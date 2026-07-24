//! Unit tests for `assert_no_unresolved_type_vars` PC-2 seam.
//!
//! Matrix layout:
//! - Cells 1–9 exercise `var_types[*]` coverage (9 cells).
//! - Cells 10–12 cover three additional type-bearing positions on
//!   `ArcFunction` (`params[*].ty`, `return_type`, `blocks[*].params[*].1`).
//! - Cells (m), (n), (o), (p), (q) live in the `body_walker` submodule;
//!   they cover the instruction-operand axis (`blocks[*].body[*].ty` for
//!   `Construct` / `Apply` / `Project`), the thin-helper path
//!   (`assert_no_unresolved_idx`), and the terminator-operand axis
//!   (`blocks[*].terminator.ty` for `Invoke`).
//! - Two behavioral tests sit outside the core matrix:
//!     * `test_lambda_with_tag_var_in_capture_environment_fails` —
//!       closure-captured types in the walk.
//!     * `test_primary_seam_empty_exempt_set_invariant_pin` — semantic
//!       pin for empty exempt set at the primary seam.
//! - Two legacy tests retained for provenance:
//!     * `test_pc2_assertion_fires_on_synthetic_leak` — negative pin for
//!       the PC-2 assertion itself (guards against INVERTED-TDD weakening).
//!     * `test_pc2_assertion_silent_on_clean_function` — positive pin
//!       (dual-guard ensuring the assertion is not replaced by an
//!       unconditional error).
//!
//! Guards against INVERTED-TDD weakening of the validator.
//!
//! # Test Fixture Strategy
//!
//! Every test consumes helpers from `crate::test_helpers`. No inline
//! `ArcFunction { ... }` construction. Helper exceptions (if any) are
//! documented inline with a citation back to this strategy note.

use std::collections::HashSet;

use ori_ir::StringInterner;
use ori_types::{build_exempt_var_ids, Idx, Pool, Tag, VarState};
use rustc_hash::FxHashSet;

use super::{assert_no_unresolved_type_vars, UnresolvedTypeVar};
use crate::ir::{ArcBlock, ArcTerminator, ArcVarId};
use crate::test_helpers::{
    allocate_pool_var_with_id, b, make_func, make_func_named, minimal_block, owned_param, v,
};

mod body_walker;

// Copy / type-shape regression guard — compile-time pin: removing `Copy`
// from `UnresolvedTypeVar` fails this constant's type-check, no runtime
// test body needed.
const _: () = {
    const fn assert_copy<T: Copy>() {}
    assert_copy::<UnresolvedTypeVar>();
};

// Cells 1–9 — `var_types[*]` coverage (core matrix)

/// Matrix cell 1 (`var_types` empty case): `var_types` is empty, exempt set is empty →
/// `Ok(())`. Establishes the lower boundary — zero axes to walk, no violations
/// possible.
#[test]
fn test_empty_var_types_passes() {
    let interner = StringInterner::new();
    let pool = Pool::new();

    let func = make_func(Vec::new(), Idx::UNIT, vec![minimal_block(0)], Vec::new());

    let exempt: HashSet<u32> = HashSet::new();
    let result = assert_no_unresolved_type_vars(&pool, &func, &interner, &exempt);
    assert!(
        result.is_ok(),
        "empty var_types must pass PC-2; got {result:?}"
    );
}

/// Matrix cell 2 (all-resolved primitives): every `var_types[*]` position carries a
/// fully-resolved concrete primitive; exempt set empty → `Ok(())`. Dual of
/// cell 3 — proves a clean function with populated `var_types` passes.
#[test]
fn test_all_resolved_primitives_pass() {
    let interner = StringInterner::new();
    let pool = Pool::new();

    let func = make_func(
        Vec::new(),
        Idx::INT,
        vec![minimal_block(0)],
        vec![Idx::INT, Idx::FLOAT, Idx::BOOL, Idx::STR],
    );

    let exempt: HashSet<u32> = HashSet::new();
    let result = assert_no_unresolved_type_vars(&pool, &func, &interner, &exempt);
    assert!(
        result.is_ok(),
        "all-primitives var_types must pass PC-2; got {result:?}"
    );
}

/// Matrix cell 3 (first-var `Tag::Var)`: `var_types[0]` carries `Tag::Var`; empty
/// exempt set → `Err` with `var_id: ArcVarId(0)`. The `var_id` field names the
/// SSA position (index 0 into `var_types`), NOT the pool var id.
#[test]
fn test_first_var_unresolved_returns_error_with_var_id_zero() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let leak = pool.fresh_var();
    let func = make_func(Vec::new(), Idx::UNIT, vec![minimal_block(0)], vec![leak]);

    let exempt: HashSet<u32> = HashSet::new();
    let Err(err) = assert_no_unresolved_type_vars(&pool, &func, &interner, &exempt) else {
        panic!("Tag::Var at var_types[0] must fail PC-2");
    };

    assert_eq!(err.var_id, v(0), "reports SSA position of leak");
    assert_eq!(err.tag, Tag::Var);
}

/// Matrix cell 4 (second-var `Tag::Var)`: `var_types[0]` resolved, `var_types[1]` is
/// `Tag::Var`; empty exempt → `Err` with `var_id: ArcVarId(1)`. Confirms the
/// walk iterates positions in order and reports the SECOND SSA position
/// (not the pool var id) when the first is clean.
#[test]
fn test_second_var_unresolved_names_that_arcvarid() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let leak = pool.fresh_var();
    let func = make_func(
        Vec::new(),
        Idx::UNIT,
        vec![minimal_block(0)],
        vec![Idx::INT, leak],
    );

    let exempt: HashSet<u32> = HashSet::new();
    let Err(err) = assert_no_unresolved_type_vars(&pool, &func, &interner, &exempt) else {
        panic!("Tag::Var at var_types[1] must fail PC-2");
    };

    assert_eq!(err.var_id, v(1), "reports SSA position 1");
    assert_eq!(err.tag, Tag::Var);
}

/// Matrix cell 5 (all-vars `Tag::Var`, first-violator determinism): ALL `var_types` positions are `Tag::Var` with
/// increasing pool `var_ids`; empty exempt → `Err` at the FIRST position
/// (`ArcVarId(0)`). Confirms deterministic first-violator reporting under
/// multiple violations.
#[test]
fn test_all_vars_unresolved_returns_first_violator_deterministic() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    // Distinct fresh vars → distinct pool var_ids in increasing order.
    let leaks: Vec<Idx> = (0..4).map(|_| pool.fresh_var()).collect();
    let func = make_func(Vec::new(), Idx::UNIT, vec![minimal_block(0)], leaks);

    let exempt: HashSet<u32> = HashSet::new();
    let Err(err) = assert_no_unresolved_type_vars(&pool, &func, &interner, &exempt) else {
        panic!("all-Tag::Var var_types must fail PC-2");
    };

    assert_eq!(
        err.var_id,
        v(0),
        "first-violator rule — lowest SSA position wins"
    );
}

/// Matrix cell 6 (`Tag::Var` with exempt `var_id)`: `var_types[0]` is `Tag::Var` whose underlying
/// pool `var_id` is 42; exempt set `{42}` → `Ok(())`. The validator's
/// exemption key is the POOL var id (`pool.data(resolved)` per
/// `validate.rs`), not the SSA position; `{42}` matches the pool id.
#[test]
fn test_tag_var_with_exempt_var_id_passes() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let var_42 = allocate_pool_var_with_id(&mut pool, 42);

    let func = make_func(Vec::new(), Idx::UNIT, vec![minimal_block(0)], vec![var_42]);

    let mut exempt: HashSet<u32> = HashSet::new();
    exempt.insert(42);
    let result = assert_no_unresolved_type_vars(&pool, &func, &interner, &exempt);
    assert!(
        result.is_ok(),
        "Tag::Var whose pool var_id is in exempt set must pass PC-2; got {result:?}"
    );
}

/// Matrix cell 7 (`Tag::Var` outside exempt set): `var_types[0]` is `Tag::Var` whose underlying
/// pool `var_id` is 42; exempt set `{7}` → `Err` with `var_id: ArcVarId(0)`
/// (the SSA position of the param, not the pool var id). Confirms exempt
/// lookup miss, and confirms reported `var_id` is the SSA slot.
#[test]
fn test_tag_var_outside_exempt_set_fails() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let var_42 = allocate_pool_var_with_id(&mut pool, 42);

    let func = make_func(Vec::new(), Idx::UNIT, vec![minimal_block(0)], vec![var_42]);

    let mut exempt: HashSet<u32> = HashSet::new();
    exempt.insert(7);
    let Err(err) = assert_no_unresolved_type_vars(&pool, &func, &interner, &exempt) else {
        panic!("pool var_id 42 not in exempt {{7}} must fail PC-2");
    };

    assert_eq!(
        err.var_id,
        v(0),
        "reported var_id is SSA position (0), NOT pool var_id (42)"
    );
    assert_eq!(err.tag, Tag::Var);
}

/// Matrix cell 8 (`Tag::Var` resolves via `resolve_fully)`: `var_types[0]` is a `Tag::Var` that resolves
/// via `VarState::Link` to a concrete primitive; empty exempt → `Ok(())`.
/// Confirms `pool.resolve_fully()` is load-bearing — a var that appears as
/// `Tag::Var` by tag alone but is a link to concrete must PASS.
#[test]
fn test_linked_var_resolves_via_pool_resolve_fully() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    // Allocate var_id 0 and link it to INT.
    pool.ensure_var_capacity(1);
    *pool.var_state_mut(0) = VarState::Link { target: Idx::INT };
    let linked_var = pool.intern(Tag::Var, 0);

    let func = make_func(
        Vec::new(),
        Idx::UNIT,
        vec![minimal_block(0)],
        vec![linked_var],
    );

    let exempt: HashSet<u32> = HashSet::new();
    let result = assert_no_unresolved_type_vars(&pool, &func, &interner, &exempt);
    assert!(
        result.is_ok(),
        "Tag::Var linked to concrete must pass PC-2 via resolve_fully; got {result:?}"
    );
}

/// Matrix cell 9 (`Tag::Projection)`: `var_types[0]` is `Tag::Projection` (an
/// unresolved associated-type reference); empty exempt → `Err` with
/// `tag: Tag::Projection`. Confirms the validator rejects both `Tag::Var`
/// AND `Tag::Projection` — the two residual pre-codegen unresolved states.
#[test]
fn test_unresolved_projection_returns_error() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    // Construct a Tag::Projection entry directly; the validator treats any
    // residual Projection as a violation (per validate.rs).
    let proj = pool.intern(Tag::Projection, 0);

    let func = make_func(Vec::new(), Idx::UNIT, vec![minimal_block(0)], vec![proj]);

    let exempt: HashSet<u32> = HashSet::new();
    let Err(err) = assert_no_unresolved_type_vars(&pool, &func, &interner, &exempt) else {
        panic!("Tag::Projection must fail PC-2");
    };

    assert_eq!(
        err.tag,
        Tag::Projection,
        "error tag is Projection (not Var)"
    );
    assert_eq!(err.var_id, v(0));
}

// Cells 10–12 — Additional type-bearing positions on `ArcFunction`

/// Matrix cell 10 (entry-block param `Tag::Var)`: `params[0].ty` carries `Tag::Var`;
/// `var_types[*]` fully resolved; empty exempt → `Err` with
/// `var_id: params[0].var` (the param's SSA var id). Confirms the validator
/// walks entry-block parameter types.
///
/// `var_types[*]` is populated with `Idx::INT` (no leak) so the walk passes
/// the `var_types` axis and proceeds to `params[*].ty`, which is where the
/// leak lives. The reported `var_id` is `params[0].var = ArcVarId(3)` per
/// `validate.rs` (`check_idx(param.ty, param.var)`).
#[test]
fn test_unresolved_var_in_entry_param_fails() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let leak = pool.fresh_var();
    let param_var_id: u32 = 3;
    let func = make_func(
        vec![owned_param(param_var_id, leak)],
        Idx::UNIT,
        vec![minimal_block(0)],
        // var_types[*] is CLEAN — the only leak is on params[0].ty. This
        // isolates the params-walk coverage from the var_types-walk coverage
        // (cells 3–5) and confirms the walk reaches params after var_types.
        vec![Idx::INT, Idx::INT, Idx::INT, Idx::INT],
    );

    let exempt: HashSet<u32> = HashSet::new();
    let Err(err) = assert_no_unresolved_type_vars(&pool, &func, &interner, &exempt) else {
        panic!("Tag::Var on params[0].ty must fail PC-2");
    };
    assert_eq!(err.tag, Tag::Var);
    assert_eq!(
        err.var_id,
        v(param_var_id),
        "reported var_id matches params[0].var (ArcVarId(3)) per assert_no_unresolved_type_vars"
    );
}

/// Matrix cell 11 (`return_type` `Tag::Var`, sentinel `ArcVarId::INVALID)`: `return_type` carries `Tag::Var`;
/// `var_types[*]` + `params[*]` fully resolved; empty exempt → `Err` with
/// `var_id: ArcVarId::INVALID` (the sentinel — no owning SSA var for the
/// return-type position). Confirms return-type coverage.
#[test]
fn test_unresolved_var_in_return_type_fails_with_sentinel_id() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let leak = pool.fresh_var();
    let func = make_func(Vec::new(), leak, vec![minimal_block(0)], Vec::new());

    let exempt: HashSet<u32> = HashSet::new();
    let Err(err) = assert_no_unresolved_type_vars(&pool, &func, &interner, &exempt) else {
        panic!("Tag::Var on return_type must fail PC-2");
    };
    assert_eq!(err.tag, Tag::Var);
    assert_eq!(
        err.var_id,
        ArcVarId::INVALID,
        "return_type position uses INVALID sentinel — no owning SSA var"
    );
}

/// Matrix cell 12 (non-entry-block param `Tag::Var)`: `blocks[1].params[0].1` (tuple `.1` = `Idx`)
/// carries `Tag::Var`; `var_types[*]` + `params[*]` + `return_type` clean;
/// empty exempt → `Err` with `var_id: blocks[1].params[0].0` (tuple `.0` =
/// `ArcVarId`). Confirms non-entry block params are walked. Entry block
/// (`blocks[0]`) is skipped since it mirrors `func.params`.
#[test]
fn test_unresolved_var_in_non_entry_block_param_fails() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let leak = pool.fresh_var();
    let block_param_var = v(5);
    // Non-entry block carries the leak on block.params[0].1. The tight
    // inline ArcBlock follows the test fixture strategy — helpers cover
    // ArcFunction-level skeleton; block-level params are local to this test.
    let non_entry_block = ArcBlock {
        id: b(1),
        params: vec![(block_param_var, leak)],
        body: Vec::new(),
        terminator: ArcTerminator::Unreachable,
    };
    let func = make_func(
        Vec::new(),
        Idx::UNIT,
        vec![minimal_block(0), non_entry_block],
        Vec::new(),
    );

    let exempt: HashSet<u32> = HashSet::new();
    let Err(err) = assert_no_unresolved_type_vars(&pool, &func, &interner, &exempt) else {
        panic!("Tag::Var on non-entry block param must fail PC-2");
    };
    assert_eq!(err.tag, Tag::Var);
    assert_eq!(
        err.var_id, block_param_var,
        "reported var_id matches block param tuple.0 (ArcVarId(5))"
    );
}

// Behavioral tests — blind spots and semantic pins

/// Blind Spot #5: closure-captured types must be caught. `num_captures > 0`
/// indicates the leading `var_types` entries hold the capture environment's
/// SSA slot types (per `ArcFunction.num_captures` doc at `ir/mod.rs`).
/// A `Tag::Var` in a capture slot SHALL fail PC-2 like any other SSA position.
///
/// Uses the `make_func` + mutate pattern (Test Fixture Strategy option
/// (b)) to set `num_captures` without extending the shared helper's signature —
/// `make_func` defaults to `num_captures: 0`, which is correct for every
/// other consumer in `borrow/`, `liveness/`, `aims/`, and `pipeline/` tests.
#[test]
fn test_lambda_with_tag_var_in_capture_environment_fails() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let leak = pool.fresh_var();
    // num_captures = 1 — the first var_types slot is the capture env's SSA
    // slot type. The validator walks var_types uniformly; a Tag::Var in a
    // capture slot must fire the check exactly like any other SSA position.
    let mut func = make_func(
        Vec::new(),
        Idx::UNIT,
        vec![minimal_block(0)],
        vec![leak, Idx::INT],
    );
    func.num_captures = 1;

    let exempt: HashSet<u32> = HashSet::new();
    let Err(err) = assert_no_unresolved_type_vars(&pool, &func, &interner, &exempt) else {
        panic!("Tag::Var in closure capture env must fail PC-2");
    };
    assert_eq!(err.tag, Tag::Var);
    assert_eq!(
        err.var_id,
        v(0),
        "capture slot is SSA position 0 (leading slot when num_captures=1)"
    );
}

/// Semantic pin for the primary-seam design decision: at the PRIMARY seam
/// (`process_arc_function` / `declare_and_process_lambda`), `exempt_var_ids`
/// SHALL be empty — the seam fires on ALL `Tag::Var`s regardless of the
/// owning function's `scheme_var_ids`. This pin models a synthetic
/// `FunctionSig.scheme_var_ids = [1, 2, 3]`, calls `build_exempt_var_ids` to
/// produce the set the SECONDARY sites would build, then invokes the
/// assertion at the primary seam with the CANONICAL empty `FxHashSet` — the
/// exact shape used at Hook 1 / Hook 2 in `define_phase.rs`. Guards against
/// a future refactor that routes non-empty `scheme_var_ids` into the primary
/// seam — closes that spot without changing the primary-seam architecture.
#[test]
fn test_primary_seam_empty_exempt_set_invariant_pin() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    // Model the upstream substitution gap: a Tag::Var whose pool var_id
    // matches one of the synthetic scheme_var_ids. At a secondary site,
    // build_exempt_var_ids({1,2,3}) would exempt this var; at the PRIMARY
    // seam, the exempt set is empty and the assertion MUST fire.
    let scheme_var = allocate_pool_var_with_id(&mut pool, 1);

    let func = make_func(
        Vec::new(),
        Idx::UNIT,
        vec![minimal_block(0)],
        vec![scheme_var],
    );

    // Build the exempt set that a SECONDARY site would use, to make
    // the contrast explicit. Must be non-empty for the invariant contrast
    // to be meaningful.
    let secondary_exempt = build_exempt_var_ids(&pool, &[1, 2, 3]);
    assert!(
        !secondary_exempt.is_empty(),
        "secondary-site exempt set must be non-empty for the contrast to \
         be meaningful; scheme_var_ids=[1,2,3] should produce at least one \
         entry"
    );
    assert!(
        secondary_exempt.contains(&1),
        "secondary-site exempt set MUST contain scheme_var_id 1 (the leak's \
         pool var_id) — otherwise the contrast against the primary seam is \
         empty"
    );

    // PRIMARY seam: empty FxHashSet (Hook 1 / Hook 2 in define_phase.rs).
    let primary_exempt: FxHashSet<u32> = FxHashSet::default();
    let Err(err) = assert_no_unresolved_type_vars(&pool, &func, &interner, &primary_exempt) else {
        panic!(
            "PRIMARY seam empty exempt_var_ids is LOAD-BEARING: the seam \
             MUST fire on ALL Tag::Vars regardless of scheme metadata; \
             any refactor that routes non-empty scheme_var_ids into the \
             primary seam is a contract violation caught here"
        );
    };
    assert_eq!(err.tag, Tag::Var);
}

// Legacy pins retained for provenance

/// Negative pin (Semantic Pins): confirms the PC-2 assertion
/// (`assert_no_unresolved_type_vars`) fires on a handcrafted ARC IR whose
/// function body carries a raw `Tag::Var` leaf.
///
/// This guards against INVERTED-TDD weakening of the validator
/// (INVERTED-TDD): any future change that gates the
/// assertion off, widens the exemption set, or otherwise neuters the check
/// on the exact inputs it is designed to catch MUST cause this test to fail.
///
/// Uses `make_func_named` because the test asserts `err.function == func.name`
/// via `interner.intern("leaky")` to pin the function-name propagation.
#[test]
fn test_pc2_assertion_fires_on_synthetic_leak() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let leak_var_ty: Idx = pool.fresh_var();
    let leak_var_id = v(0);

    let entry_block = ArcBlock {
        id: b(0),
        params: vec![(leak_var_id, leak_var_ty)],
        body: Vec::new(),
        terminator: ArcTerminator::Unreachable,
    };
    let func = make_func_named(
        interner.intern("leaky"),
        vec![owned_param(0, leak_var_ty)],
        Idx::UNIT,
        vec![entry_block],
        vec![leak_var_ty],
    );

    let exempt: HashSet<u32> = HashSet::new();
    let Err(err) = assert_no_unresolved_type_vars(&pool, &func, &interner, &exempt) else {
        panic!(
            "PC-2 assertion MUST fire on raw Tag::Var in function body — \
             gating this check off on any axis is INVERTED-TDD"
        );
    };

    assert_eq!(
        err.function, func.name,
        "error should name the leaking function"
    );
    assert_eq!(
        err.tag,
        Tag::Var,
        "error should identify Tag::Var (not Projection)"
    );
    assert_eq!(
        pool.resolve_fully(err.idx),
        pool.resolve_fully(leak_var_ty),
        "error should point at the handcrafted leak var (post-resolve_fully)"
    );
}

/// Dual to `test_pc2_assertion_fires_on_synthetic_leak`: confirms the
/// assertion does NOT spuriously fire on a well-formed function where every
/// position carries a fully-resolved concrete type. Without this
/// positive-no-fire pin, a future weakening that replaced the check with
/// `Err(...)` unconditionally would still pass the negative pin.
#[test]
fn test_pc2_assertion_silent_on_clean_function() {
    let interner = StringInterner::new();
    let pool = Pool::new();

    let clean_var_id = v(0);
    // Entry block uses a Return terminator referencing the param — this test
    // pins that a well-typed function with an active return path passes the
    // validator. The inline ArcBlock follows the test fixture strategy —
    // tight block-level customization, helper covers ArcFunction skeleton.
    let entry_block = ArcBlock {
        id: b(0),
        params: vec![(clean_var_id, Idx::INT)],
        body: Vec::new(),
        terminator: ArcTerminator::Return {
            value: clean_var_id,
        },
    };
    let func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![entry_block],
        vec![Idx::INT],
    );

    let exempt: HashSet<u32> = HashSet::new();
    let result = assert_no_unresolved_type_vars(&pool, &func, &interner, &exempt);
    assert!(
        result.is_ok(),
        "clean function with fully-resolved concrete types MUST pass PC-2; got {result:?}"
    );
}
