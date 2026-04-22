//! Unit tests for `assert_no_unresolved_type_vars` (§04.2 PC-2 seam).
//!
//! Negative pin (§04.2.B Semantic Pins): prove the assertion fires on a
//! handcrafted ARC IR carrying a raw `Tag::Var` leaf. Guards against
//! INVERTED-TDD weakening of the validator per CLAUDE.md §NEVER Reason
//! Out of TPR Findings and `impl-hygiene.md §INVERTED-TDD`.
//!
//! Full 12-cell matrix (§04.4 deliverable) is tracked separately.

use std::collections::HashSet;

use ori_ir::StringInterner;
use ori_types::{Idx, Pool};

use super::{assert_no_unresolved_type_vars, UnresolvedTypeVar};
use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcParam, ArcTerminator, ArcVarId, Ownership};

#[test]
fn unresolved_type_var_is_copy() {
    fn assert_copy<T: Copy>() {}
    assert_copy::<UnresolvedTypeVar>();
}

/// Negative pin (§04.2.B Semantic Pins): confirms the PC-2 assertion
/// (`assert_no_unresolved_type_vars`) fires on a handcrafted ARC IR
/// whose function body carries a raw `Tag::Var` leaf.
///
/// This guards against INVERTED-TDD weakening of the validator
/// (`impl-hygiene.md §INVERTED-TDD`): any future change that gates the
/// assertion off, widens the exemption set, or otherwise neuters the
/// check on the exact inputs it is designed to catch MUST cause this
/// test to fail.
#[test]
fn test_pc2_assertion_fires_on_synthetic_leak() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    // Handcraft an unbound Tag::Var — the exact shape that would
    // trigger the §04.2 seam at codegen.
    let leak_var_ty: Idx = pool.fresh_var();

    // Build a minimal function whose entry-block parameter is the leak.
    // The parameter position is one of the four PC-2-checked axes
    // (`validate.rs` covers var_types, params[*].ty, return_type, and
    // CFG-block params).
    let leak_var_id = ArcVarId::new(0);
    let func = ArcFunction {
        name: interner.intern("leaky"),
        params: vec![ArcParam {
            var: leak_var_id,
            ty: leak_var_ty,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::UNIT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![(leak_var_id, leak_var_ty)],
            body: Vec::new(),
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![leak_var_ty],
        ..Default::default()
    };

    // Exempt set is EMPTY — monomorphized functions carry no scheme
    // vars. A leak under an empty exemption set MUST fire.
    let exempt: HashSet<u32> = HashSet::new();
    let result = assert_no_unresolved_type_vars(&pool, &func, &interner, &exempt);
    let Err(err) = result else {
        panic!(
            "PC-2 assertion MUST fire on raw Tag::Var in function body — \
             gating this check off on any axis is INVERTED-TDD \
             (impl-hygiene.md §INVERTED-TDD); see CLAUDE.md §NEVER Reason \
             Out of TPR Findings"
        );
    };

    // Verify the error targets the correct variable and tag — not a
    // coincidental false-positive from some other path.
    assert_eq!(
        err.function, func.name,
        "error should name the leaking function"
    );
    assert_eq!(
        err.tag,
        ori_types::Tag::Var,
        "error should identify Tag::Var (not Projection)"
    );
    assert_eq!(
        pool.resolve_fully(err.idx),
        pool.resolve_fully(leak_var_ty),
        "error should point at the handcrafted leak var (post-resolve_fully)"
    );
}

/// Dual to `test_pc2_assertion_fires_on_synthetic_leak`: confirms the
/// assertion does NOT spuriously fire on a well-formed function where
/// every position carries a fully-resolved concrete type. Without this
/// positive-no-fire pin, a future weakening that replaced the check
/// with `Err(...)` unconditionally would still pass the negative pin.
#[test]
fn test_pc2_assertion_silent_on_clean_function() {
    let interner = StringInterner::new();
    let pool = Pool::new();

    // All primitive Idx values — no Tag::Var anywhere.
    let clean_var_id = ArcVarId::new(0);
    let func = ArcFunction {
        name: interner.intern("clean"),
        params: vec![ArcParam {
            var: clean_var_id,
            ty: Idx::INT,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::INT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![(clean_var_id, Idx::INT)],
            body: Vec::new(),
            terminator: ArcTerminator::Return {
                value: clean_var_id,
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::INT],
        ..Default::default()
    };

    let exempt: HashSet<u32> = HashSet::new();
    let result = assert_no_unresolved_type_vars(&pool, &func, &interner, &exempt);
    assert!(
        result.is_ok(),
        "clean function with fully-resolved concrete types MUST pass PC-2; got {result:?}"
    );
}
