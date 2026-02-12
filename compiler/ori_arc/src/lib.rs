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
//! - **Ownership annotations** ([`Ownership`], [`DerivedOwnership`],
//!   [`AnnotatedParam`], [`AnnotatedSig`]) —
//!   borrow inference output that drives RC insertion decisions.
//!   [`DerivedOwnership`] extends ownership tracking to all local variables,
//!   not just function parameters.
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
pub mod fbip;
mod graph;
pub mod ir;
pub mod liveness;
pub mod lower;
pub mod ownership;
pub mod rc_elim;
pub mod rc_insert;
pub mod reset_reuse;

#[cfg(test)]
pub(crate) mod test_helpers;

use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::FxHashMap;

pub use borrow::{apply_borrows, infer_borrows, infer_derived_ownership};
pub use classify::ArcClassifier;
pub use decision_tree::{
    DecisionTree, FlatPattern, PathInstruction, PatternMatrix, PatternRow, ScrutineePath, TestKind,
    TestValue,
};
pub use drop::{
    collect_drop_infos, compute_closure_env_drop, compute_drop_info, DropInfo, DropKind,
};
pub use expand_reuse::expand_reset_reuse;
pub use graph::DominatorTree;
pub use ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    CtorKind, LitValue, PrimOp,
};
pub use liveness::{
    compute_liveness, compute_refined_liveness, BlockLiveness, LiveSet, RefinedLiveness,
};
pub use lower::{lower_function_can, ArcProblem};
pub use ownership::{AnnotatedParam, AnnotatedSig, DerivedOwnership, Ownership};
pub use rc_elim::eliminate_rc_ops_dataflow;
pub use rc_insert::insert_rc_ops_with_ownership;
pub use reset_reuse::detect_reset_reuse_cfg;

/// Run the full ARC optimization pipeline on a single function.
///
/// Pipeline order: ownership inference → dominator tree → refined liveness
/// (includes standard liveness) → RC insertion → reset/reuse detection →
/// expansion → RC elimination.
///
/// This is the canonical pass ordering. All consumers should call this function
/// instead of manually sequencing passes, which avoids duplicating ordering
/// knowledge across crate boundaries.
#[expect(clippy::implicit_hasher, reason = "callee functions require FxHashMap")]
pub fn run_arc_pipeline(
    func: &mut ArcFunction,
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, AnnotatedSig>,
) {
    let ownership = borrow::infer_derived_ownership(func, sigs);
    let dom_tree = graph::DominatorTree::build(func);
    let (refined, liveness) = liveness::compute_refined_liveness(func, classifier);
    rc_insert::insert_rc_ops_with_ownership(func, classifier, &liveness, &ownership, sigs);
    reset_reuse::detect_reset_reuse_cfg(func, classifier, &dom_tree, &refined);
    expand_reuse::expand_reset_reuse(func, classifier);
    rc_elim::eliminate_rc_ops_dataflow(func, &ownership);
}

/// Run the full ARC pipeline on all functions, including borrow application.
///
/// This is the batch entry point for the entire ARC optimization pass:
/// 1. Apply borrow inference results to function parameters
/// 2. Run the per-function pipeline on each function
///
/// Consumers should call this instead of manually calling [`apply_borrows`]
/// followed by a per-function loop over [`run_arc_pipeline`].
#[expect(clippy::implicit_hasher, reason = "callee functions require FxHashMap")]
pub fn run_arc_pipeline_all(
    functions: &mut [ArcFunction],
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, AnnotatedSig>,
) {
    borrow::apply_borrows(functions, sigs);
    for func in functions {
        run_arc_pipeline(func, classifier, sigs);
    }
}

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

// Pipeline integration tests

#[cfg(test)]
mod pipeline_tests {
    use ori_types::{Idx, Pool};

    use ori_ir::Name;

    use crate::ir::{ArcBlock, ArcFunction, ArcInstr, ArcParam, ArcTerminator, CtorKind};
    use crate::ownership::Ownership;
    use crate::test_helpers::{b, count_rc_ops, make_func, v};
    use rustc_hash::FxHashMap;

    use crate::{
        compute_liveness, compute_refined_liveness, expand_reset_reuse, ArcClassifier,
        DominatorTree,
    };

    /// Run the full ARC pipeline via the public orchestration function.
    fn run_full_pipeline(func: &mut ArcFunction, classifier: &dyn crate::ArcClassification) {
        let sigs = FxHashMap::default();
        crate::run_arc_pipeline(func, classifier, &sigs);
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

        // Run pipeline in correct order (skipping detect — IR has pre-placed Reset/Reuse).
        // Uses classic functions directly (pub(crate)) to test ordering invariant.
        let mut func_correct = func.clone();
        {
            let liveness = compute_liveness(&func_correct, &classifier);
            crate::rc_insert::insert_rc_ops(&mut func_correct, &classifier, &liveness);
            // detect_reset_reuse skipped: IR already contains Reset/Reuse from setup
            expand_reset_reuse(&mut func_correct, &classifier);
            crate::rc_elim::eliminate_rc_ops(&mut func_correct);
        }

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
        crate::rc_insert::insert_rc_ops(&mut func_wrong, &classifier, &liveness);
        crate::rc_elim::eliminate_rc_ops(&mut func_wrong); // wrong: runs too early
                                                           // detect_reset_reuse skipped: IR already contains Reset/Reuse from setup
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

    /// Full enhanced pipeline on raw IR with reuse pattern.
    ///
    /// Exercises the production pipeline infrastructure:
    /// - `infer_derived_ownership` (per-variable ownership)
    /// - `DominatorTree` (for cross-block reset/reuse)
    /// - `compute_refined_liveness` (for aliasing checks)
    /// - `insert_rc_ops_with_ownership` (ownership-aware RC insertion)
    /// - `detect_reset_reuse_cfg` (intra + cross-block detection)
    /// - `eliminate_rc_ops_dataflow` (full-CFG elimination)
    /// - `analyze_fbip` (FBIP diagnostic report)
    #[test]
    fn full_pipeline_on_reuse_pattern() {
        use crate::fbip::analyze_fbip;

        // Raw IR: Project fields, Apply transform, Construct result.
        // No pre-placed Reset/Reuse — detection passes discover the pattern.
        //
        // fn foo(x: str) -> str
        //   head = Project(x, 0)
        //   tail = Project(x, 1)
        //   new_head = Apply(f, [head])
        //   result = Construct(Struct, [new_head, tail])
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
                    ArcInstr::Construct {
                        dst: v(4),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![v(3), v(2)],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(4) },
            }],
            vec![
                Idx::STR, // v0: param
                Idx::STR, // v1: head
                Idx::STR, // v2: tail
                Idx::STR, // v3: new_head
                Idx::STR, // v4: result
            ],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);

        let mut func = func;
        run_full_pipeline(&mut func, &classifier);

        // No unexpanded Reset/Reuse should remain
        let has_unexpanded = func
            .blocks
            .iter()
            .flat_map(|bl| bl.body.iter())
            .any(|i| matches!(i, ArcInstr::Reset { .. } | ArcInstr::Reuse { .. }));
        assert!(
            !has_unexpanded,
            "no Reset/Reuse should remain after expansion"
        );

        // Run FBIP analysis on the result
        let dom_tree = DominatorTree::build(&func);
        let (refined, _) = compute_refined_liveness(&func, &classifier);
        let _fbip_report = analyze_fbip(&func, &classifier, &dom_tree, &refined);
    }
}
