//! ARC analysis for the Ori compiler.
//!
//! This crate provides:
//!
//! - **Type classification** ([`ArcClass`]) — every type is classified as
//!   [`Scalar`](ArcClass::Scalar) (no RC needed),
//!   [`DefiniteRef`](ArcClass::DefiniteRef) (always needs RC), or
//!   [`PossibleRef`](ArcClass::PossibleRef) (conservative fallback).
//!
//! - **ARC IR** ([`ArcFunction`], [`ArcBlock`], [`ArcInstr`], [`ArcTerminator`]) —
//!   a basic-block intermediate representation that all ARC analysis passes
//!   (borrow inference, RC insertion, RC elimination, constructor reuse)
//!   operate on.
//!
//! - **Ownership annotations** ([`Ownership`], [`AnnotatedParam`], [`AnnotatedSig`]) —
//!   borrow inference output that drives RC insertion decisions.
//!
//! # Design
//!
//! Inspired by Lean 4's three-way classification (`isScalar`/`isPossibleRef`/
//! `isDefiniteRef` on `IRType`) and LCNF basic-block IR. Classification is
//! **monomorphized** — it operates on concrete types after type parameter
//! substitution. This means:
//!
//! - `option[int]` → **Scalar** (tag + int, no heap pointer)
//! - `option[str]` → **`DefiniteRef`** (contains heap-allocated string)
//! - `option[T]` where `T` is unresolved → **`PossibleRef`** (conservative)
//!
//! # Crate Dependencies
//!
//! `ori_arc` depends on `ori_types` (for `Pool`/`Idx`/`Tag`) and `ori_ir`
//! (for `Name`, `BinaryOp`, `UnaryOp`, etc.). No LLVM dependency — ARC
//! analysis is backend-independent.

pub mod borrow;
mod classify;
pub mod decision_tree;
pub mod drop;
pub mod expand_reuse;
pub mod ir;
pub mod liveness;
pub mod lower;
pub mod ownership;
pub mod rc_elim;
pub mod rc_insert;
pub mod reset_reuse;

pub use borrow::{apply_borrows, infer_borrows};
pub use classify::ArcClassifier;
pub use decision_tree::{
    DecisionTree, FlatPattern, PathInstruction, PatternMatrix, PatternRow, ScrutineePath, TestKind,
    TestValue,
};
pub use drop::{
    collect_drop_infos, compute_closure_env_drop, compute_drop_info, DropInfo, DropKind,
};
pub use expand_reuse::expand_reset_reuse;
pub use ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    CtorKind, LitValue, PrimOp,
};
pub use liveness::{compute_liveness, BlockLiveness, LiveSet};
pub use lower::{lower_function, ArcProblem};
use ori_types::Idx;
pub use ownership::{AnnotatedParam, AnnotatedSig, Ownership};
pub use rc_elim::eliminate_rc_ops;
pub use rc_insert::insert_rc_ops;
pub use reset_reuse::detect_reset_reuse;

/// ARC classification for a type.
///
/// Determines whether values of this type need reference counting.
/// This classification is the foundation for all ARC optimization passes.
///
/// Inspired by Lean 4's three-way classification methods
/// (`isScalar`, `isPossibleRef`, `isDefiniteRef` on `IRType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArcClass {
    /// No reference counting needed. The value is purely stack/register.
    ///
    /// Examples: `int`, `float`, `bool`, `char`, `byte`, `unit`, `never`,
    /// `duration`, `size`, `ordering`, `option[int]`, `(int, float)`.
    Scalar,

    /// Definitely contains a reference-counted heap pointer.
    /// Every value of this type needs retain/release.
    ///
    /// Examples: `str`, `[T]`, `{K: V}`, `set[T]`, `chan<T>`,
    /// `(P) -> R`, `option[str]`, `(int, str)`.
    DefiniteRef,

    /// Might contain a reference-counted pointer depending on unresolved
    /// type variables. Conservatively treated as needing RC.
    ///
    /// Only appears for unresolved type variables before monomorphization.
    /// After monomorphization, every type classifies as either `Scalar` or
    /// `DefiniteRef` — encountering `PossibleRef` post-mono is a compiler bug.
    PossibleRef,
}

/// Classification trait for ARC analysis.
///
/// Provides the core `arc_class` query plus convenience predicates.
/// Implemented by [`ArcClassifier`], which wraps a `Pool` reference
/// with caching and cycle detection.
pub trait ArcClassification {
    /// Classify a type by its pool index.
    fn arc_class(&self, idx: Idx) -> ArcClass;

    /// Returns `true` if this type is scalar (no RC operations needed).
    fn is_scalar(&self, idx: Idx) -> bool {
        self.arc_class(idx) == ArcClass::Scalar
    }

    /// Returns `true` if this type might need reference counting.
    ///
    /// This is `true` for both `DefiniteRef` and `PossibleRef`.
    fn needs_rc(&self, idx: Idx) -> bool {
        self.arc_class(idx) != ArcClass::Scalar
    }
}

// ── Pipeline integration tests ──────────────────────────────────────

#[cfg(test)]
mod pipeline_tests {
    use ori_ir::Name;
    use ori_types::{Idx, Pool};

    use crate::ir::{
        ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, CtorKind,
    };
    use crate::ownership::Ownership;
    use crate::{
        compute_liveness, detect_reset_reuse, eliminate_rc_ops, expand_reset_reuse, insert_rc_ops,
        ArcClassifier,
    };

    fn v(n: u32) -> ArcVarId {
        ArcVarId::new(n)
    }

    fn b(n: u32) -> ArcBlockId {
        ArcBlockId::new(n)
    }

    fn make_func(
        params: Vec<ArcParam>,
        return_type: Idx,
        blocks: Vec<ArcBlock>,
        var_types: Vec<Idx>,
    ) -> ArcFunction {
        let span_vecs: Vec<Vec<Option<ori_ir::Span>>> =
            blocks.iter().map(|bl| vec![None; bl.body.len()]).collect();
        ArcFunction {
            name: Name::from_raw(1),
            params,
            return_type,
            blocks,
            entry: ArcBlockId::new(0),
            var_types,
            spans: span_vecs,
        }
    }

    fn count_rc_ops(func: &ArcFunction) -> usize {
        func.blocks
            .iter()
            .flat_map(|bl| bl.body.iter())
            .filter(|i| matches!(i, ArcInstr::RcInc { .. } | ArcInstr::RcDec { .. }))
            .count()
    }

    /// Run the full ARC pipeline in the documented correct order:
    /// insert (07) → detect → expand (09) → eliminate (08).
    ///
    /// This matches `rc_elim.rs` documentation: "Execution order: 07 → 09 → 08"
    /// and Lean 4's pipeline: `explicitRC` → `expandResetReuse`.
    fn run_full_pipeline(func: &mut ArcFunction, classifier: &dyn crate::ArcClassification) {
        let liveness = compute_liveness(func, classifier);
        insert_rc_ops(func, classifier, &liveness);
        detect_reset_reuse(func, classifier);
        expand_reset_reuse(func, classifier);
        eliminate_rc_ops(func);
    }

    /// Verifies the correct pipeline order: expand BEFORE eliminate.
    ///
    /// Creates a function with a constructor-reuse pattern. After expansion,
    /// new `RcInc`/`RcDec` instructions are generated (slow path `RcDec`, restored
    /// `RcInc`, fast path field `RcDec`). Running eliminate AFTER expansion
    /// ensures those ops are candidates for optimization.
    #[test]
    fn pipeline_order_expand_before_eliminate() {
        // fn foo(x: str) -> str
        //   head = Project(x, 0)       -- STR field
        //   tail = Project(x, 1)       -- STR field
        //   new_head = Apply(f, [head]) -- transform head
        //   Reset(x, token)
        //   result = Reuse(token, Struct, [new_head, tail])
        //   Return result
        let func = make_func(
            vec![ArcParam {
                var: v(0),
                ty: Idx::STR,
                ownership: Ownership::Owned,
            }],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Project {
                        dst: v(1),
                        ty: Idx::STR,
                        value: v(0),
                        field: 0,
                    },
                    ArcInstr::Project {
                        dst: v(2),
                        ty: Idx::STR,
                        value: v(0),
                        field: 1,
                    },
                    ArcInstr::Apply {
                        dst: v(3),
                        ty: Idx::STR,
                        func: Name::from_raw(99),
                        args: vec![v(1)],
                    },
                    ArcInstr::Reset {
                        var: v(0),
                        token: v(4),
                    },
                    ArcInstr::Reuse {
                        token: v(4),
                        dst: v(5),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![v(3), v(2)],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(5) },
            }],
            vec![
                Idx::STR, // v0: param
                Idx::STR, // v1: head
                Idx::STR, // v2: tail
                Idx::STR, // v3: new_head
                Idx::STR, // v4: token
                Idx::STR, // v5: result
            ],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);

        // Run pipeline in correct order
        let mut func_correct = func.clone();
        run_full_pipeline(&mut func_correct, &classifier);

        // No Reset/Reuse should remain after expansion
        let has_reset = func_correct
            .blocks
            .iter()
            .flat_map(|bl| bl.body.iter())
            .any(|i| matches!(i, ArcInstr::Reset { .. }));
        let has_reuse = func_correct
            .blocks
            .iter()
            .flat_map(|bl| bl.body.iter())
            .any(|i| matches!(i, ArcInstr::Reuse { .. }));
        assert!(!has_reset, "no Reset instructions should remain");
        assert!(!has_reuse, "no Reuse instructions should remain");

        // Should have expanded into multiple blocks (original + fast + slow + merge)
        assert!(
            func_correct.blocks.len() >= 3,
            "pipeline should expand into 3+ blocks, got {}",
            func_correct.blocks.len()
        );

        // Run pipeline in WRONG order (eliminate before expand) for comparison
        let mut func_wrong = func.clone();
        let liveness = compute_liveness(&func_wrong, &classifier);
        insert_rc_ops(&mut func_wrong, &classifier, &liveness);
        eliminate_rc_ops(&mut func_wrong); // wrong: runs too early
        detect_reset_reuse(&mut func_wrong, &classifier);
        expand_reset_reuse(&mut func_wrong, &classifier);

        // Wrong order should have MORE remaining RC ops (expand generated
        // new ones that eliminate already ran and couldn't clean up)
        let correct_rc_count = count_rc_ops(&func_correct);
        let wrong_rc_count = count_rc_ops(&func_wrong);
        assert!(
            correct_rc_count <= wrong_rc_count,
            "correct pipeline order should have <= RC ops ({correct_rc_count}) \
             than wrong order ({wrong_rc_count})"
        );
    }

    /// The pipeline should handle functions with no Reset/Reuse gracefully.
    #[test]
    fn pipeline_no_reuse_pattern() {
        let func = make_func(
            vec![ArcParam {
                var: v(0),
                ty: Idx::STR,
                ownership: Ownership::Owned,
            }],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);

        let mut func = func;
        run_full_pipeline(&mut func, &classifier);

        // Should still have exactly 1 block (no expansion needed)
        assert_eq!(func.blocks.len(), 1);
    }
}
