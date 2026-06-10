//! Tests for the burden-op elimination consumer.
//!
//! Negative pins on residual ops + positive pins on paired elimination.
//!
//! Predicate citations:
//! - DP-2 (`is_rc_dec_unnecessary` at `aims/transfer/mod.rs:403`):
//!   `is_rc_dec_unnecessary(s) ⟺ s.cardinality = Absent ∨
//!   s.consumption = Dead`.
//! - DP-3 (`is_rc_inc_elidable` at `aims/transfer/mod.rs:411`):
//!   `is_rc_inc_elidable(s) ⟺ s.cardinality = Once ∧
//!   (s.consumption = Linear ∨ Affine)`.

use super::{burden_op_census, eliminate_burden_ops, is_burden_removal_only};
use crate::aims::intraprocedural::AimsStateMap;
use crate::aims::lattice::{
    AccessClass, AimsState, Cardinality, Consumption, EffectClass, Locality, ShapeClass, Uniqueness,
};
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    CtorKind,
};
use crate::ownership::Ownership;
use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::FxHashMap;

// Helpers

fn v(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

fn block_id(n: u32) -> ArcBlockId {
    ArcBlockId::new(n)
}

fn ty(n: u32) -> Idx {
    Idx::from_raw(n)
}

fn name(n: u32) -> Name {
    Name::from_raw(n)
}

/// Build a single-block `ArcFunction` with `body` as the block body.
/// Allocates `num_vars` typed slots so `v(0)..v(num_vars-1)` are valid.
fn one_block_func(num_vars: u32, body: Vec<ArcInstr>) -> ArcFunction {
    let var_types: Vec<Idx> = (0..num_vars).map(ty).collect();
    ArcFunction {
        name: name(1),
        return_type: ty(0),
        var_types,
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: Vec::new(),
            body,
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        ..Default::default()
    }
}

/// Build an `AimsState` from cardinality+consumption with the remaining
/// dimensions defaulted to a plausible owned-value shape. The elimination
/// predicates DP-2 and DP-3 only consult `cardinality` and `consumption`
/// per the decision-predicate truth-table appendix, so other dimensions
/// are unconstrained.
fn owned_state(cardinality: Cardinality, consumption: Consumption) -> AimsState {
    AimsState {
        access: AccessClass::Owned,
        consumption,
        cardinality,
        uniqueness: Uniqueness::Unique,
        locality: Locality::BlockLocal,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::NONE,
    }
}

/// Seed `state_map`'s block-exit map with `(var, state)` entries for the
/// given block. The elimination pass queries `var_state_at_block_exit`, so
/// this is the canonical place to set up per-test states.
fn seed_exit_state(
    state_map: &mut AimsStateMap,
    block: ArcBlockId,
    entries: &[(ArcVarId, AimsState)],
) {
    let mut map: FxHashMap<ArcVarId, AimsState> = FxHashMap::default();
    for (var, state) in entries {
        map.insert(*var, *state);
    }
    state_map.update_block_exit(block, map);
}

/// Run the elimination pass with an empty same-alloc rep map, empty contracts,
/// and a fresh interner — the per-var DP-2/DP-3 path these unit tests pin. Every
/// test uses `predicate_stack_rc_disabled = false`, so the lineage re-balance
/// (gated on the burden-only path) is inert; the empty maps + fresh interner are
/// correct.
fn run_elim(func: &mut ArcFunction, state_map: &AimsStateMap, predicate_stack_rc_disabled: bool) {
    let same_alloc_reps: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    let contracts: FxHashMap<Name, crate::aims::contract::MemoryContract> = FxHashMap::default();
    let interner = ori_ir::StringInterner::new();
    eliminate_burden_ops(
        func,
        state_map,
        &same_alloc_reps,
        &contracts,
        &interner,
        predicate_stack_rc_disabled,
    );
}

/// Per-burden-op-kind census `[BurdenInc, BurdenDec, BurdenDecPartial,
/// BurdenDecField, BurdenDecVariant]` over a function — for asserting the
/// post-elimination op shape.
fn census(func: &ArcFunction) -> [usize; 5] {
    burden_op_census(func)
}

// ITEM-3 — Negative pins (matrix-clamping regression-resistance).

/// State (Linear, Once) — DP-2 returns FALSE per `DP-2`
/// truth table. `BurdenDec` must NOT be removed.
#[test]
fn dp2_false_preserves_burden_dec_linear_once() {
    let func_body = vec![ArcInstr::BurdenDec { var: v(0) }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    // (Linear, Once) — value still has one demanded use; dec is required.
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Once, Consumption::Linear))],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "DP-2 false on (Once, Linear) must preserve BurdenDec; body = {body:?}"
    );
    assert!(
        matches!(body[0], ArcInstr::BurdenDec { var } if var == v(0)),
        "expected BurdenDec(v0) preserved, got {:?}",
        body[0]
    );
}

/// State (Linear, Many) — DP-3 returns FALSE per `DP-3`
/// truth table (cardinality ≠ Once). `BurdenInc` must NOT be removed.
#[test]
fn dp3_false_preserves_burden_inc_linear_many() {
    let func_body = vec![ArcInstr::BurdenInc { var: v(0) }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    // (Many, Linear) — multiple uses; the inc creates the second ref.
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Many, Consumption::Linear))],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "DP-3 false on (Many, Linear) must preserve BurdenInc; body = {body:?}"
    );
    assert!(
        matches!(body[0], ArcInstr::BurdenInc { var } if var == v(0)),
        "expected BurdenInc(v0) preserved, got {:?}",
        body[0]
    );
}

// ITEM-4 — Positive pins (semantic pins).
// One per predicate because DP-2 + DP-3 fire on mutually-exclusive states
// per `CN-1` Dead↔Absent bidirectional rule.

/// `elide_inc_on_linear_once` — var `v` is (Owned, Linear, Once, Unique,
/// `BlockLocal`, `NonReusable`); DP-3 (`is_rc_inc_elidable`) returns `true`;
/// `burden_elim` removes the `BurdenInc` instruction.
///
/// Per `DP-3`: `is_rc_inc_elidable(s) ⟺ s.cardinality =
/// Once ∧ (s.consumption = Linear ∨ Affine)`. This test pins the Linear arm.
#[test]
fn elide_inc_on_linear_once() {
    let func_body = vec![ArcInstr::BurdenInc { var: v(0) }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Once, Consumption::Linear))],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert!(
        body.is_empty(),
        "DP-3 true on (Once, Linear) must elide BurdenInc; body = {body:?}"
    );
}

/// `elide_dec_on_dead_absent` — var `w` is (Owned, Dead, Absent, *, *, *)
/// per CN-1 pairing; DP-2 (`is_rc_dec_unnecessary`) returns `true`;
/// `burden_elim` removes the `BurdenDec` instruction.
///
/// Per `DP-2`: `is_rc_dec_unnecessary(s) ⟺ s.cardinality
/// = Absent ∨ s.consumption = Dead`. CN-1 makes Dead↔Absent
/// bidirectional, so any (Dead, Absent) state trivially satisfies both
/// disjuncts.
#[test]
fn elide_dec_on_dead_absent() {
    let func_body = vec![ArcInstr::BurdenDec { var: v(0) }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Absent, Consumption::Dead))],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert!(
        body.is_empty(),
        "DP-2 true on (Dead, Absent) must elide BurdenDec; body = {body:?}"
    );
}

/// `BurdenDecPartial` follows the DP-2 rule on whole-var state. With
/// (Dead, Absent), elimination must remove `BurdenDecPartial` too.
#[test]
fn elide_dec_partial_on_dead_absent() {
    let func_body = vec![ArcInstr::BurdenDecPartial {
        var: v(0),
        skip_fields: vec![1],
    }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Absent, Consumption::Dead))],
    );

    run_elim(&mut func, &state_map, false);

    assert!(
        func.blocks[0].body.is_empty(),
        "DP-2 true must elide BurdenDecPartial; body = {:?}",
        func.blocks[0].body
    );
}

/// `BurdenDecVariant` follows the DP-2 rule on whole-var state.
#[test]
fn elide_dec_variant_on_dead_absent() {
    let func_body = vec![ArcInstr::BurdenDecVariant { var: v(0) }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Absent, Consumption::Dead))],
    );

    run_elim(&mut func, &state_map, false);

    assert!(
        func.blocks[0].body.is_empty(),
        "DP-2 true must elide BurdenDecVariant; body = {:?}",
        func.blocks[0].body
    );
}

/// `BurdenDecField` queries DP-2 against `base`'s WHOLE-VAR state.
#[test]
fn elide_dec_field_on_dead_absent_base() {
    let func_body = vec![ArcInstr::BurdenDecField {
        base: v(0),
        field: 2,
    }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Absent, Consumption::Dead))],
    );

    run_elim(&mut func, &state_map, false);

    assert!(
        func.blocks[0].body.is_empty(),
        "DP-2 true on base must elide BurdenDecField; body = {:?}",
        func.blocks[0].body
    );
}

/// Negative pin for the partial-drop variants: with a state where DP-2
/// returns false (e.g., (Many, Unrestricted)), `BurdenDecPartial` must be
/// preserved.
#[test]
fn preserve_dec_partial_on_many_unrestricted() {
    let func_body = vec![ArcInstr::BurdenDecPartial {
        var: v(0),
        skip_fields: vec![0],
    }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(
            v(0),
            owned_state(Cardinality::Many, Consumption::Unrestricted),
        )],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "DP-2 false on (Many, Unrestricted) must preserve BurdenDecPartial"
    );
    assert!(matches!(
        body[0],
        ArcInstr::BurdenDecPartial { var, .. } if var == v(0)
    ));
}

// ITEM-5 — matrix completion.
//
// Per-(forward state × backward demand) × Burden* variant matrix: 5 states
// × 5 variants = 25 cells. Eight cells covered by the ITEM-3 + ITEM-4 pins
// above (`dp2_false_preserves_burden_dec_linear_once`,
// `dp3_false_preserves_burden_inc_linear_many`, `elide_inc_on_linear_once`,
// `elide_dec_on_dead_absent`, `elide_dec_partial_on_dead_absent`,
// `elide_dec_variant_on_dead_absent`, `elide_dec_field_on_dead_absent_base`,
// `preserve_dec_partial_on_many_unrestricted`). Remaining 17 cells follow.
//
// Per the decision-predicate truth-table appendix:
// - DP-2 true ⟺ `cardinality = Absent ∨ consumption = Dead`. Per CN-1
//   (Dead ↔ Absent bidirectional), only (Absent, Dead) satisfies DP-2 in
//   a canonicalized state — the other four feasible states all preserve
//   BurdenDec*.
// - DP-3 true ⟺ `cardinality = Once ∧ (consumption = Linear ∨ Affine)`. Both
//   (Once, Linear) and (Once, Affine) satisfy DP-3 — the remaining feasible
//   states all preserve BurdenInc.
//
// Each cell pins the variant-specific code path even when the predicted
// outcome duplicates another cell; intentional matrix coverage
// (regression-resistance against per-variant drift).

// (Once, Linear) × { BurdenDecPartial, BurdenDecVariant, BurdenDecField }

/// State (Once, Linear) × `BurdenDecPartial` — DP-2 false on (Once, Linear)
/// per `DP-2` truth table (neither cardinality = Absent
/// nor consumption = Dead); partial-drop must NOT be removed.
#[test]
fn preserve_dec_partial_on_linear_once() {
    let func_body = vec![ArcInstr::BurdenDecPartial {
        var: v(0),
        skip_fields: vec![0],
    }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Once, Consumption::Linear))],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "DP-2 false on (Once, Linear) must preserve BurdenDecPartial; body = {body:?}"
    );
    assert!(matches!(
        body[0],
        ArcInstr::BurdenDecPartial { var, .. } if var == v(0)
    ));
}

/// State (Once, Linear) × `BurdenDecVariant` — DP-2 false per
/// `DP-2`; variant-drop must NOT be removed.
#[test]
fn preserve_dec_variant_on_linear_once() {
    let func_body = vec![ArcInstr::BurdenDecVariant { var: v(0) }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Once, Consumption::Linear))],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "DP-2 false on (Once, Linear) must preserve BurdenDecVariant; body = {body:?}"
    );
    assert!(matches!(
        body[0],
        ArcInstr::BurdenDecVariant { var } if var == v(0)
    ));
}

/// State (Once, Linear) × `BurdenDecField` — DP-2 queried against base's
/// whole-var state per `DP-2`; DP-2 false on (Once,
/// Linear) base means the field dec must NOT be removed.
#[test]
fn preserve_dec_field_on_linear_once_base() {
    let func_body = vec![ArcInstr::BurdenDecField {
        base: v(0),
        field: 2,
    }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Once, Consumption::Linear))],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "DP-2 false on (Once, Linear) base must preserve BurdenDecField; body = {body:?}"
    );
    assert!(matches!(
        body[0],
        ArcInstr::BurdenDecField { base, field } if base == v(0) && field == 2
    ));
}

// (Many, Linear) × { BurdenDec, BurdenDecPartial, BurdenDecVariant, BurdenDecField }

/// State (Many, Linear) × `BurdenDec` — DP-2 false (neither cardinality =
/// Absent nor consumption = Dead); dec must NOT be removed.
#[test]
fn preserve_dec_on_linear_many() {
    let func_body = vec![ArcInstr::BurdenDec { var: v(0) }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Many, Consumption::Linear))],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "DP-2 false on (Many, Linear) must preserve BurdenDec; body = {body:?}"
    );
    assert!(matches!(
        body[0],
        ArcInstr::BurdenDec { var } if var == v(0)
    ));
}

/// State (Many, Linear) × `BurdenDecPartial` — DP-2 false per
/// `DP-2`; partial-drop must NOT be removed.
#[test]
fn preserve_dec_partial_on_linear_many() {
    let func_body = vec![ArcInstr::BurdenDecPartial {
        var: v(0),
        skip_fields: vec![1, 2],
    }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Many, Consumption::Linear))],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "DP-2 false on (Many, Linear) must preserve BurdenDecPartial; body = {body:?}"
    );
    assert!(matches!(
        body[0],
        ArcInstr::BurdenDecPartial { var, .. } if var == v(0)
    ));
}

/// State (Many, Linear) × `BurdenDecVariant` — DP-2 false per
/// `DP-2`; variant-drop must NOT be removed.
#[test]
fn preserve_dec_variant_on_linear_many() {
    let func_body = vec![ArcInstr::BurdenDecVariant { var: v(0) }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Many, Consumption::Linear))],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "DP-2 false on (Many, Linear) must preserve BurdenDecVariant; body = {body:?}"
    );
    assert!(matches!(
        body[0],
        ArcInstr::BurdenDecVariant { var } if var == v(0)
    ));
}

/// State (Many, Linear) × `BurdenDecField` — DP-2 queried against base's
/// whole-var state per `DP-2`; DP-2 false on (Many,
/// Linear) base means the field dec must NOT be removed.
#[test]
fn preserve_dec_field_on_linear_many_base() {
    let func_body = vec![ArcInstr::BurdenDecField {
        base: v(0),
        field: 3,
    }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Many, Consumption::Linear))],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "DP-2 false on (Many, Linear) base must preserve BurdenDecField; body = {body:?}"
    );
    assert!(matches!(
        body[0],
        ArcInstr::BurdenDecField { base, field } if base == v(0) && field == 3
    ));
}

// (Once, Affine) × { BurdenInc, BurdenDec, BurdenDecPartial, BurdenDecVariant, BurdenDecField }

/// State (Once, Affine) × `BurdenInc` — DP-3 TRUE per `Once ∧ (Linear ∨ Affine)`
/// (`AimsProof.Decision.is_rc_inc_elidable` + `DP3_is_rc_inc_elidable_table`); a
/// single-use value borrowed (Affine) is not duplicated, so the inc is elided.
#[test]
fn elide_inc_on_affine_once() {
    let func_body = vec![ArcInstr::BurdenInc { var: v(0) }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Once, Consumption::Affine))],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert!(
        body.is_empty(),
        "DP-3 true on (Once, Affine) must elide BurdenInc; body = {body:?}"
    );
}

/// State (Once, Affine) × `BurdenDec` — DP-2 false (neither cardinality =
/// Absent nor consumption = Dead); dec must NOT be removed.
#[test]
fn preserve_dec_on_affine_once() {
    let func_body = vec![ArcInstr::BurdenDec { var: v(0) }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Once, Consumption::Affine))],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "DP-2 false on (Once, Affine) must preserve BurdenDec; body = {body:?}"
    );
    assert!(matches!(
        body[0],
        ArcInstr::BurdenDec { var } if var == v(0)
    ));
}

/// State (Once, Affine) × `BurdenDecPartial` — DP-2 false per
/// `DP-2`; partial-drop must NOT be removed.
#[test]
fn preserve_dec_partial_on_affine_once() {
    let func_body = vec![ArcInstr::BurdenDecPartial {
        var: v(0),
        skip_fields: vec![0, 2],
    }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Once, Consumption::Affine))],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "DP-2 false on (Once, Affine) must preserve BurdenDecPartial; body = {body:?}"
    );
    assert!(matches!(
        body[0],
        ArcInstr::BurdenDecPartial { var, .. } if var == v(0)
    ));
}

/// State (Once, Affine) × `BurdenDecVariant` — DP-2 false per
/// `DP-2`; variant-drop must NOT be removed.
#[test]
fn preserve_dec_variant_on_affine_once() {
    let func_body = vec![ArcInstr::BurdenDecVariant { var: v(0) }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Once, Consumption::Affine))],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "DP-2 false on (Once, Affine) must preserve BurdenDecVariant; body = {body:?}"
    );
    assert!(matches!(
        body[0],
        ArcInstr::BurdenDecVariant { var } if var == v(0)
    ));
}

/// State (Once, Affine) × `BurdenDecField` — DP-2 queried against base's
/// whole-var state per `DP-2`; DP-2 false on (Once,
/// Affine) base means the field dec must NOT be removed.
#[test]
fn preserve_dec_field_on_affine_once_base() {
    let func_body = vec![ArcInstr::BurdenDecField {
        base: v(0),
        field: 1,
    }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Once, Consumption::Affine))],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "DP-2 false on (Once, Affine) base must preserve BurdenDecField; body = {body:?}"
    );
    assert!(matches!(
        body[0],
        ArcInstr::BurdenDecField { base, field } if base == v(0) && field == 1
    ));
}

// (Many, Unrestricted) × { BurdenInc, BurdenDec, BurdenDecVariant, BurdenDecField }
// — BurdenDecPartial covered by `preserve_dec_partial_on_many_unrestricted`.

/// State (Many, Unrestricted) × `BurdenInc` — DP-3 false per
/// `DP-3` (cardinality ≠ Once); inc must NOT be removed.
#[test]
fn preserve_inc_on_unrestricted_many() {
    let func_body = vec![ArcInstr::BurdenInc { var: v(0) }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(
            v(0),
            owned_state(Cardinality::Many, Consumption::Unrestricted),
        )],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "DP-3 false on (Many, Unrestricted) must preserve BurdenInc; body = {body:?}"
    );
    assert!(matches!(
        body[0],
        ArcInstr::BurdenInc { var } if var == v(0)
    ));
}

/// State (Many, Unrestricted) × `BurdenDec` — DP-2 false per
/// `DP-2`; dec must NOT be removed.
#[test]
fn preserve_dec_on_unrestricted_many() {
    let func_body = vec![ArcInstr::BurdenDec { var: v(0) }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(
            v(0),
            owned_state(Cardinality::Many, Consumption::Unrestricted),
        )],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "DP-2 false on (Many, Unrestricted) must preserve BurdenDec; body = {body:?}"
    );
    assert!(matches!(
        body[0],
        ArcInstr::BurdenDec { var } if var == v(0)
    ));
}

/// State (Many, Unrestricted) × `BurdenDecVariant` — DP-2 false per
/// `DP-2`; variant-drop must NOT be removed.
#[test]
fn preserve_dec_variant_on_unrestricted_many() {
    let func_body = vec![ArcInstr::BurdenDecVariant { var: v(0) }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(
            v(0),
            owned_state(Cardinality::Many, Consumption::Unrestricted),
        )],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "DP-2 false on (Many, Unrestricted) must preserve BurdenDecVariant; body = {body:?}"
    );
    assert!(matches!(
        body[0],
        ArcInstr::BurdenDecVariant { var } if var == v(0)
    ));
}

/// State (Many, Unrestricted) × `BurdenDecField` — DP-2 queried against
/// base's whole-var state per `DP-2`; DP-2 false on
/// (Many, Unrestricted) base means the field dec must NOT be removed.
#[test]
fn preserve_dec_field_on_unrestricted_many_base() {
    let func_body = vec![ArcInstr::BurdenDecField {
        base: v(0),
        field: 4,
    }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(
            v(0),
            owned_state(Cardinality::Many, Consumption::Unrestricted),
        )],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "DP-2 false on (Many, Unrestricted) base must preserve BurdenDecField; body = {body:?}"
    );
    assert!(matches!(
        body[0],
        ArcInstr::BurdenDecField { base, field } if base == v(0) && field == 4
    ));
}

// (Absent, Dead) × { BurdenInc }
// — remaining BurdenDec* variants on (Absent, Dead) covered above.

/// State (Absent, Dead) × `BurdenInc` — DP-3 false (cardinality ≠ Once); inc
/// must NOT be removed. Per CN-1 the
/// (Absent, Dead) state is the only canonicalized state satisfying DP-2,
/// but `BurdenInc` consults DP-3 which fails on cardinality = Absent.
#[test]
fn preserve_inc_on_dead_absent() {
    let func_body = vec![ArcInstr::BurdenInc { var: v(0) }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Absent, Consumption::Dead))],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "DP-3 false on (Absent, Dead) must preserve BurdenInc; body = {body:?}"
    );
    assert!(matches!(
        body[0],
        ArcInstr::BurdenInc { var } if var == v(0)
    ));
}

// Coexistence handshake pins.
//
// These pins target the predicate-stack / burden-walk handshake: when an
// SSA-alias class is fully burden-covered, `decide()` returns
// `RcDecision::None` (predicate stack defers); when ANY member lacks
// burden coverage, `class_covered` is false and predicate stack runs
// unchanged. The unit-level test exercises `decide()` directly with the
// `class_covered` flag set/unset (mirroring what the realize walks
// compute from `state_map.is_class_covered`); the integration shape
// (populating burden_emitted + class_covered + invoking decide) lands
// in the full pipeline path.

/// Positive pin: `decide()` with `class_covered: true`
/// returns `RcDecision::None` regardless of the underlying `DecisionSite`.
/// Predicate-stack realization SHALL emit zero RC ops on this site —
/// burden walk owns the inc/dec.
#[test]
fn class_fully_covered_predicate_stack_skips() {
    use crate::aims::realize::decide::{
        decide, DecisionContext, DecisionSite, RcDecision, ReuseContext, ReuseDecision,
        UseSemantics,
    };

    // Use-site that would normally emit Inc (future use, Normal semantics).
    let decision = decide(&DecisionContext {
        site: DecisionSite::Use {
            has_future_use: true,
            semantics: UseSemantics::Normal,
        },
        is_rc_managed: true,
        class_covered: true,
    });
    assert_eq!(
        decision.rc,
        RcDecision::None,
        "class_covered=true must force RcDecision::None on Use site (got {:?})",
        decision.rc
    );

    // Defined-dead site that would normally emit Dec.
    let decision = decide(&DecisionContext {
        site: DecisionSite::DefinedDead,
        is_rc_managed: true,
        class_covered: true,
    });
    assert_eq!(
        decision.rc,
        RcDecision::None,
        "class_covered=true must force RcDecision::None on DefinedDead (got {:?})",
        decision.rc
    );

    // Last-use site that would normally emit Dec.
    let decision = decide(&DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,
            has_deferred_children: false,
            reuse: ReuseContext {
                shape: ShapeClass::NonReusable,
                uniqueness: Uniqueness::Unique,
                cardinality: Cardinality::Once,
            },
        },
        is_rc_managed: true,
        class_covered: true,
    });
    assert_eq!(
        decision.rc,
        RcDecision::None,
        "class_covered=true must force RcDecision::None on LastUse (got {:?})",
        decision.rc
    );
    assert_eq!(
        decision.reuse,
        ReuseDecision::None,
        "class_covered=true must skip reuse (burden walk owns disposal)"
    );
}

/// Negative pin: when `class_covered: false` (mixed
/// coverage in the class), `decide()` runs as today and produces the
/// normal predicate-stack decisions. This pin proves the coexistence
/// handshake is a STRICT all-or-nothing gate — no partial-class skipping.
#[test]
fn mixed_coverage_predicate_stack_runs() {
    use crate::aims::realize::decide::{
        decide, DecisionContext, DecisionSite, RcDecision, UseSemantics,
    };

    // Use-site with future use, class_covered=false → predicate stack
    // emits the normal RcInc decision.
    let decision = decide(&DecisionContext {
        site: DecisionSite::Use {
            has_future_use: true,
            semantics: UseSemantics::Normal,
        },
        is_rc_managed: true,
        class_covered: false,
    });
    assert_eq!(
        decision.rc,
        RcDecision::Inc,
        "class_covered=false must NOT suppress RcInc (got {:?})",
        decision.rc
    );

    // Defined-dead site with class_covered=false → predicate stack emits Dec.
    let decision = decide(&DecisionContext {
        site: DecisionSite::DefinedDead,
        is_rc_managed: true,
        class_covered: false,
    });
    assert_eq!(
        decision.rc,
        RcDecision::Dec,
        "class_covered=false must NOT suppress DefinedDead Dec (got {:?})",
        decision.rc
    );
}

/// Helper test: `AimsStateMap::is_class_covered` reads
/// the set installed by `set_class_covered`. The full `populate_class_covered`
/// fixed-point semantics are exercised via the pipeline path; here we pin
/// the accessor contract a class id mapping in/out of the set.
#[test]
fn class_covered_accessor_reads_installed_set() {
    let func = one_block_func(1, vec![]);
    let mut state_map = AimsStateMap::new(&func);
    assert!(
        !state_map.is_class_covered(0),
        "empty class_covered must report false for any class id"
    );

    let mut covered: rustc_hash::FxHashSet<u32> = rustc_hash::FxHashSet::default();
    covered.insert(7);
    state_map.set_class_covered(covered);
    assert!(
        state_map.is_class_covered(7),
        "installed class 7 must report covered"
    );
    assert!(
        !state_map.is_class_covered(8),
        "non-installed class 8 must report not covered"
    );
    assert_eq!(state_map.class_covered_count(), 1);
}

/// Helper test: `ArcFunction::burden_emitted` records the
/// vars touched by burden-op emission. Pin proves the populate pass sets
/// the bit for each Burden* instruction's target var.
#[test]
fn burden_emitted_records_emitted_vars() {
    let body = vec![
        ArcInstr::BurdenInc { var: v(0) },
        ArcInstr::BurdenDec { var: v(1) },
        ArcInstr::BurdenDecField {
            base: v(2),
            field: 0,
        },
    ];
    let func = one_block_func(4, body);
    // Mimic what `populate_burden_emitted` does (private helper —
    // exercised at the pipeline level; here we directly set the field
    // to pin the accessor shape).
    let mut emitted = vec![false; 4];
    emitted[0] = true;
    emitted[1] = true;
    emitted[2] = true;
    let mut func2 = func;
    func2.burden_emitted = emitted;
    assert!(func2.burden_emitted[0], "v0 emitted by BurdenInc");
    assert!(func2.burden_emitted[1], "v1 emitted by BurdenDec");
    assert!(func2.burden_emitted[2], "v2 emitted by BurdenDecField base");
    assert!(!func2.burden_emitted[3], "v3 not emitted");
}

/// Mixed-instruction body: per paired-elimination preserving VF-1
/// intraprocedural balance, a var's Inc + whole-var Dec ops elide together
/// ONLY when DP-3 fires on every Inc AND DP-2 fires on every Dec — otherwise every op
/// for that var is retained so the intraprocedural net stays zero.
///
/// v0 state (Once, Linear): DP-3 fires on its Inc → inc ELIDED; DP-2 does
/// NOT fire on its Dec → the RL-2 scope-exit Dec is KEPT (decoupled inc-only
/// elision — the alloc/callee-return supplies the +1, the surviving Dec brings
/// it to 0, RC-balanced per `RL1_duplication_balanced`).
/// v1 state (Many, Unrestricted): neither DP-2 nor DP-3 fire → its Dec
/// retained. Non-burden `RcInc` preserved verbatim + relative order kept.
#[test]
fn mixed_body_preserves_non_burden_and_relative_order() {
    let func_body = vec![
        ArcInstr::BurdenInc { var: v(0) },
        ArcInstr::RcInc {
            var: v(1),
            count: 1,
            strategy: crate::ir::RcStrategy::HeapPointer,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
        },
        ArcInstr::BurdenDec { var: v(1) },
        ArcInstr::BurdenDec { var: v(0) },
    ];
    let mut func = one_block_func(2, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[
            (v(0), owned_state(Cardinality::Once, Consumption::Linear)),
            (
                v(1),
                owned_state(Cardinality::Many, Consumption::Unrestricted),
            ),
        ],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        3,
        "decoupled inc-only elim: v0 BurdenInc elided (DP-3), its Dec + v1 ops kept; body = {body:?}"
    );
    assert!(matches!(body[0], ArcInstr::RcInc { var, .. } if var == v(1)));
    assert!(matches!(body[1], ArcInstr::BurdenDec { var } if var == v(1)));
    assert!(matches!(body[2], ArcInstr::BurdenDec { var } if var == v(0)));
}

/// Paired-elim positive case: same var has `BurdenInc` + `BurdenDec`, both
/// states elidable per their predicates. With both Inc and Dec elidable, both elide
/// together when DP-3 fires on Inc AND DP-2 fires on Dec.
///
/// Two distinct states is impossible for one var in one block-exit
/// lookup; this test uses two separate vars to demonstrate the pairing
/// independence — v0 (Once, Linear) pair elides, v1 (Absent, Dead) pair
/// elides, but mixing requires same var.
///
/// For a single var, DP-2 + DP-3 cannot BOTH fire on the same canonical
/// state per CN-1 (Dead ↔ Absent mutually exclusive with Once/Linear).
/// Therefore the same-var paired-elim path NEVER fires in practice — it
/// is a soundness backstop preventing VF-1 imbalance, not an
/// optimization. The pin demonstrates that the backstop correctly
/// retains BOTH ops in every case where they could not BOTH elide.
#[test]
fn paired_elim_unmatched_inc_retains_both() {
    let func_body = vec![
        ArcInstr::BurdenInc { var: v(0) },
        ArcInstr::BurdenDec { var: v(0) },
    ];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Absent, Consumption::Dead))],
    );

    run_elim(&mut func, &state_map, false);

    let body = &func.blocks[0].body;
    // (Absent, Dead): DP-2 fires on Dec but DP-3 FAILS on Inc (needs
    // Once + Linear). Per VF-1 paired-elim contract, retain both to
    // keep `Σ Inc - Σ Dec = 0` across the block.
    assert_eq!(
        body.len(),
        2,
        "paired-elim must retain unmatched Inc + matching Dec to preserve VF-1 balance; body = {body:?}"
    );
    assert!(matches!(body[0], ArcInstr::BurdenInc { var } if var == v(0)));
    assert!(matches!(body[1], ArcInstr::BurdenDec { var } if var == v(0)));
}

/// Paired-elim negative case: a var with only `BurdenInc` and no matching
/// `BurdenDec` elides the Inc per DP-3 directly (no Dec to pin against).
/// The pre-paired-elim contract for solitary-Inc preservation is
/// retained — `elide_inc_on_linear_once` already covers this; this pin
/// makes the asymmetric case explicit.
#[test]
fn paired_elim_solo_inc_elidable_state_elides() {
    let func_body = vec![ArcInstr::BurdenInc { var: v(0) }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Once, Consumption::Linear))],
    );

    run_elim(&mut func, &state_map, false);

    assert!(
        func.blocks[0].body.is_empty(),
        "solitary Inc with DP-3 firing elides (no matching Dec to pin against); body = {:?}",
        func.blocks[0].body
    );
}

/// Paired-elim negative case: a var with only `BurdenDec` and no matching
/// `BurdenInc` elides the Dec per DP-2 directly. Mirrors
/// `paired_elim_solo_inc_elidable_state_elides` for the dec side.
#[test]
fn paired_elim_solo_dec_unnecessary_state_elides() {
    let func_body = vec![ArcInstr::BurdenDec { var: v(0) }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Absent, Consumption::Dead))],
    );

    run_elim(&mut func, &state_map, false);

    assert!(
        func.blocks[0].body.is_empty(),
        "solitary Dec with DP-2 firing elides (no matching Inc to pin against); body = {:?}",
        func.blocks[0].body
    );
}

// Lattice consumption-mode shift (emission → elimination).
//
// The CRITICAL invariant of the Canonical RC-Emission Path:
// "`eliminate_burden_ops` consumes DP-2/DP-3 at burden-op sites. It NEVER
// constructs burden ops." Phase 6 is an OPTIMIZER over the Phase-5
// burden-emitted baseline — every burden-op kind's post-pass census is `≤`
// its pre-pass census. The pins below clamp that consumption-mode from both
// sides:
//   - semantic positive pin: the pass eliminates (census strictly shrinks on
//     an elidable op AND the op is gone) and reports removal-only;
//   - negative pin: the removal-only predicate REJECTS a constructing census
//     (after > before);
//   - debug-build guard pin: the structural guard panics when Phase 6 would
//     construct a burden op (debug builds only — `debug_assert!`).

/// Semantic positive pin — the consumption-mode is ELIMINATION.
///
/// `eliminate_burden_ops` over a body with an elidable `BurdenInc` (Once,
/// Linear → DP-3 fires) STRICTLY SHRINKS the burden census (the Inc is
/// removed) and the transition is removal-only. This pin FAILS on revert if
/// Phase 6 were reverted to a constructor (census would grow, `<` would
/// become `>`) OR if it stopped eliminating (census would stay equal, the
/// strict-shrink assertion would fail).
#[test]
fn phase6_is_elimination_census_strictly_shrinks() {
    let func_body = vec![ArcInstr::BurdenInc { var: v(0) }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Once, Consumption::Linear))],
    );

    let before = burden_op_census(&func);
    run_elim(&mut func, &state_map, false);
    let after = burden_op_census(&func);

    // BurdenInc census (index 0) strictly shrinks: the elidable op is removed.
    assert_eq!(before[0], 1, "pre-pass census records the one BurdenInc");
    assert_eq!(
        after[0], 0,
        "Phase 6 ELIMINATED the BurdenInc (census shrank)"
    );
    assert!(
        is_burden_removal_only(&before, &after),
        "Phase 6 elimination is removal-only: before = {before:?}, after = {after:?}"
    );
    assert!(
        func.blocks[0].body.is_empty(),
        "the elided BurdenInc must be gone from the body; body = {:?}",
        func.blocks[0].body
    );
}

/// Census-conservation pin — when nothing is elidable, Phase 6
/// constructs nothing and the census is unchanged (removal-only with zero
/// removals). Clamps the lower bound: the pass never grows the census.
#[test]
fn phase6_preserves_census_when_nothing_elidable() {
    let func_body = vec![
        ArcInstr::BurdenInc { var: v(0) },
        ArcInstr::BurdenDec { var: v(0) },
    ];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    // (Many, Unrestricted): neither DP-2 nor DP-3 fires → no elision.
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(
            v(0),
            owned_state(Cardinality::Many, Consumption::Unrestricted),
        )],
    );

    let before = burden_op_census(&func);
    run_elim(&mut func, &state_map, false);
    let after = burden_op_census(&func);

    assert_eq!(
        before, after,
        "Phase 6 constructs nothing AND elides nothing on a non-elidable body"
    );
    assert!(is_burden_removal_only(&before, &after));
}

/// Negative pin — the removal-only predicate REJECTS a Phase-6
/// construction attempt. For EVERY burden-op kind, a census transition where
/// `after > before` (a burden op was constructed) is rejected. This is the
/// structural enforcement that a future regression appending a burden op in
/// Phase 6 cannot pass.
#[test]
fn removal_only_predicate_rejects_construction_per_kind() {
    // Baseline: empty census.
    let zero = [0usize; 5];
    // Removal-only (no change) is accepted.
    assert!(
        is_burden_removal_only(&zero, &zero),
        "no-op census transition is removal-only"
    );
    // Removal (after < before) is accepted.
    let some = [2, 2, 2, 2, 2];
    let removed = [1, 0, 2, 1, 0];
    assert!(
        is_burden_removal_only(&some, &removed),
        "shrinking census (pure removal) is removal-only"
    );

    // Construction of ANY single kind is rejected.
    for kind in 0..5 {
        let mut constructed = zero;
        constructed[kind] = 1; // Phase 6 "appended" one op of this kind.
        assert!(
            !is_burden_removal_only(&zero, &constructed),
            "constructing kind {kind} (0 → 1) must be REJECTED by the removal-only guard"
        );
    }
}

/// Debug-build structural guard pin — `debug_assert_burden_removal_only`
/// panics when Phase 6 would construct a burden op. Debug builds only:
/// `debug_assert!` is a no-op under `--release`, so the guard (and this pin)
/// are scoped to `debug_assertions`. The companion release-safe enforcement
/// is `removal_only_predicate_rejects_construction_per_kind` above.
#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "AIMS Phase-6 invariant")]
fn debug_guard_panics_on_phase6_construction() {
    use super::debug_assert_burden_removal_only;
    let before = [1usize, 0, 0, 0, 0];
    // after grows BurdenDec from 0 → 1: a construction in Phase 6.
    let after = [1usize, 1, 0, 0, 0];
    debug_assert_burden_removal_only(&before, &after);
}

// Lineage re-balance pins (the `same_alloc_reps`-grouped alias-chain
// release-exactly-once pass, burden-only path).

/// Build a single-block alias-chain function: `%0 = "lit"` (fresh `FatValue`
/// alloc) + `%1 = Let Var(%0)` (alias) with the supplied burden `body`, returning
/// a scalar so the block is terminal. `var_reprs` marks `%0`/`%1` as `FatValue`
/// (the `fresh_rc_alloc_dst` repr gate) and the return var as `Scalar`.
fn alias_chain_func(body: Vec<ArcInstr>) -> ArcFunction {
    use crate::ir::{ArcValue, LitValue};
    use crate::ValueRepr;
    let mut full_body = vec![
        ArcInstr::Let {
            dst: v(0),
            ty: ty(0),
            value: ArcValue::Literal(LitValue::String(name(99))),
        },
        ArcInstr::Let {
            dst: v(1),
            ty: ty(0),
            value: ArcValue::Var(v(0)),
        },
    ];
    full_body.extend(body);
    let mut func = ArcFunction {
        name: name(1),
        return_type: ty(0),
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: Vec::new(),
            body: full_body,
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        ..Default::default()
    };
    // %0, %1 are FatValue (RC-tracked str); %2 (return scalar) is Scalar.
    func.var_reprs = vec![ValueRepr::FatValue, ValueRepr::FatValue, ValueRepr::Scalar];
    func
}

/// `same_alloc_reps` unioning `%1 → %0` (the Let-Var alias edge), the rep map
/// `compute_same_alloc_reps` would produce for `let b = a`.
fn alias_chain_reps() -> FxHashMap<ArcVarId, ArcVarId> {
    let mut reps = FxHashMap::default();
    reps.insert(v(0), v(0));
    reps.insert(v(1), v(0));
    reps
}

/// Run elimination on the burden-only path (`predicate_stack_rc_disabled = true`)
/// with the alias-chain rep map — the path that exercises the lineage re-balance.
/// The alias-chain rep carries no apply-result alias, so an EMPTY contracts map
/// is correct: `compute_transfer_forwarder_anchors` finds no forwarder, and the
/// pure-Let-Var-alias re-balance path is exercised unchanged.
fn run_elim_rebalance(func: &mut ArcFunction) {
    let same_alloc_reps = alias_chain_reps();
    let contracts: FxHashMap<Name, crate::aims::contract::MemoryContract> = FxHashMap::default();
    let interner = ori_ir::StringInterner::new();
    let state_map = AimsStateMap::new(func);
    eliminate_burden_ops(
        func,
        &state_map,
        &same_alloc_reps,
        &contracts,
        &interner,
        true,
    );
}

/// SEMANTIC PIN: a balanced per-alias burden shape on one allocation
/// (`burden_inc %0`, `burden_inc %1`, `burden_dec %0`, `burden_dec %1`) is
/// re-balanced to EXACTLY ONE release — alloc(+1) − 1 dec = 0 (RL-2
/// `RL2_release_exactly_once`). Pre-fix the per-var pass kept both decs +
/// stripped only the alias inc → net −1 (double-free). This pins the
/// alias-chain re-balance: zero incs + exactly one dec survive.
#[test]
fn lineage_rebalance_alias_chain_keeps_one_release() {
    let mut func = alias_chain_func(vec![
        ArcInstr::BurdenInc { var: v(0) },
        ArcInstr::BurdenInc { var: v(1) },
        ArcInstr::BurdenDec { var: v(0) },
        ArcInstr::BurdenDec { var: v(1) },
    ]);
    run_elim_rebalance(&mut func);
    let c = census(&func);
    assert_eq!(
        c[0], 0,
        "lineage re-balance must elide ALL alias-chain incs; census = {c:?}"
    );
    assert_eq!(
        c[1], 1,
        "lineage re-balance must keep EXACTLY ONE BurdenDec as the RL-2 release; \
         census = {c:?}"
    );
}

/// NEGATIVE PIN: a FORWARDER-tainted rep (an `ApplyAliasSource::Direct` result
/// unioned into the lineage) is EXCLUDED from the re-balance — its apply-result
/// inc is a genuine transfer-duplication, not an alias-spurious inc; eliding it
/// would under-count the shared allocation. The per-var pass owns it, so the
/// balanced inc/dec PAIR survives unchanged (DP-2 false on `Once` → dec kept;
/// DP-3 no-fire on `Many` → inc kept). Pins the forwarder exclusion that cured
/// the generics-forwarder over-fire.
#[test]
fn lineage_rebalance_excludes_forwarder_tainted_rep() {
    use crate::aims::intraprocedural::state_map::ApplyAliasSource;
    let mut func = alias_chain_func(vec![
        ArcInstr::BurdenInc { var: v(0) },
        ArcInstr::BurdenInc { var: v(1) },
        ArcInstr::BurdenDec { var: v(0) },
        ArcInstr::BurdenDec { var: v(1) },
    ]);
    let same_alloc_reps = alias_chain_reps();
    let interner = ori_ir::StringInterner::new();
    let mut state_map = AimsStateMap::new(&func);
    // %1 is an apply-result Direct alias of %0 (a forwarder f(%0) = %0): taints
    // rep %0, so the re-balance must NOT fire on this lineage.
    let mut aliases = FxHashMap::default();
    aliases.insert(v(1), ApplyAliasSource::Direct(v(0)));
    state_map.set_apply_result_aliases(aliases);
    // (Many, Unrestricted) on both → per-var keeps incs (DP-3 no-fire) + decs
    // (DP-2 false) → all 4 ops survive when the re-balance is correctly skipped.
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[
            (
                v(0),
                owned_state(Cardinality::Many, Consumption::Unrestricted),
            ),
            (
                v(1),
                owned_state(Cardinality::Many, Consumption::Unrestricted),
            ),
        ],
    );
    // NO contract for the forwarder → `compute_transfer_forwarder_anchors`
    // produces no anchor (the `transfers_through_return` provenance is absent), so
    // the `Direct` apply-result alias stays in the EXCLUDED class: the per-var
    // pass owns it. An EMPTY contracts map is the right fixture — the exclusion
    // must hold whenever the transfer fact is unproven.
    let contracts: FxHashMap<Name, crate::aims::contract::MemoryContract> = FxHashMap::default();
    eliminate_burden_ops(
        &mut func,
        &state_map,
        &same_alloc_reps,
        &contracts,
        &interner,
        true,
    );
    let c = census(&func);
    assert_eq!(
        c[0], 2,
        "forwarder-tainted rep with NO transfers_through_return contract must NOT be \
         re-balanced — both incs survive; census = {c:?}"
    );
    assert_eq!(
        c[1], 2,
        "forwarder-tainted rep with NO transfers_through_return contract must NOT be \
         re-balanced — both decs survive; census = {c:?}"
    );
}

// COW-survivor lineage re-balance pins (the `compute_cow_mutated_lineage_reps`
// candidate-gate deferral — RL-1 keep-alive inc on a COW-shared-survives lineage).

/// Build a single-block COW-shared-survivor function: `%0 = "lit"` (fresh
/// `FatValue` alloc) + `%1 = Let Var(%0)` (alias) flowing into a COW-mutation at
/// an OWNED `Apply` arg (`push(%1)` → `%2`) + `%4 = Let Var(%0)` re-reading the
/// original after the consume. `same_alloc_reps` unions `%1`/`%4` into rep `%0`;
/// the push result `%2` is its own rep. `var_reprs` marks every collection var
/// `FatValue` (the `is_rcptr` gate) and the scalar arg/return `Scalar`.
fn cow_push_alias_func(body: Vec<ArcInstr>) -> ArcFunction {
    use crate::ir::{ArcValue, ArgOwnership, LitValue};
    use crate::ValueRepr;
    let mut full_body = vec![
        ArcInstr::Let {
            dst: v(0),
            ty: ty(0),
            value: ArcValue::Literal(LitValue::String(name(99))),
        },
        ArcInstr::Let {
            dst: v(1),
            ty: ty(0),
            value: ArcValue::Var(v(0)),
        },
        // COW mutation: `%1.push(%3)` lowers to an `Apply @push` with the
        // receiver `%1` at OWNED arg position 0 (`is_owned_position` + `is_rcptr`
        // → `consumed_owned`). `arg_ownership` explicit so the detector sees the
        // owned receiver regardless of the all-Owned default.
        ArcInstr::Apply {
            dst: v(2),
            ty: ty(0),
            func: name(50),
            args: vec![v(1), v(3)],
            arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Borrowed],
            mono_instance_id: None,
        },
        // Re-read of the original allocation AFTER the consume — the RL-1
        // duplicating-use condition that makes the fresh keep-alive inc
        // load-bearing.
        ArcInstr::Let {
            dst: v(4),
            ty: ty(0),
            value: ArcValue::Var(v(0)),
        },
    ];
    full_body.extend(body);
    let mut func = ArcFunction {
        name: name(1),
        return_type: ty(0),
        var_types: vec![ty(0), ty(0), ty(0), ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: Vec::new(),
            body: full_body,
            terminator: ArcTerminator::Return { value: v(5) },
        }],
        ..Default::default()
    };
    // %0/%1/%2/%4 are FatValue (RC-tracked); %3 (pushed int) + %5 (return) Scalar.
    func.var_reprs = vec![
        ValueRepr::FatValue,
        ValueRepr::FatValue,
        ValueRepr::FatValue,
        ValueRepr::Scalar,
        ValueRepr::FatValue,
        ValueRepr::Scalar,
    ];
    func
}

/// `same_alloc_reps` unioning the original allocation's alias chain `%1 → %0`,
/// `%4 → %0` for the COW-push fixture. `%2` (the push result) is a distinct
/// allocation rep and is intentionally NOT unioned.
fn cow_push_alias_reps() -> FxHashMap<ArcVarId, ArcVarId> {
    let mut reps = FxHashMap::default();
    reps.insert(v(0), v(0));
    reps.insert(v(1), v(0));
    reps.insert(v(4), v(0));
    reps
}

/// POSITIVE PIN: a COW-shared-survives lineage (fresh `%0` aliased into an owned
/// `push` arg AND re-read after) DEFERS to the COW-aware per-var pass — the
/// lineage re-balance candidate gate excludes it (`cow_mutated_reps.contains`).
/// The per-var pass KEEPS the fresh keep-alive inc on `(Many, Unrestricted)`
/// (DP-3 no-fire), so it survives the elimination. Pre-fix the COW-blind
/// re-balance elided ALL incs → the runtime rc stayed at 1 → the COW protocol
/// mutated the shared buffer in place → double-free. FAILS if the
/// `cow_mutated_reps.contains(rep)` gate term is reverted (the rep is then
/// re-balanced and the inc is wrongly elided to 0). Spec: Annex E §AIMS RL-1.
#[test]
fn lineage_rebalance_defers_on_cow_mutated_shared_survivor() {
    let mut func = cow_push_alias_func(vec![
        ArcInstr::BurdenInc { var: v(0) },
        ArcInstr::BurdenInc { var: v(1) },
        ArcInstr::BurdenDec { var: v(0) },
        ArcInstr::BurdenDec { var: v(1) },
    ]);
    let same_alloc_reps = cow_push_alias_reps();
    let contracts: FxHashMap<Name, crate::aims::contract::MemoryContract> = FxHashMap::default();
    let interner = ori_ir::StringInterner::new();
    let mut state_map = AimsStateMap::new(&func);
    // (Many, Unrestricted) on the lineage vars → DP-3 no-fire, so the COW-aware
    // per-var pass KEEPS the load-bearing keep-alive inc when the re-balance
    // correctly defers.
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[
            (
                v(0),
                owned_state(Cardinality::Many, Consumption::Unrestricted),
            ),
            (
                v(1),
                owned_state(Cardinality::Many, Consumption::Unrestricted),
            ),
        ],
    );
    eliminate_burden_ops(
        &mut func,
        &state_map,
        &same_alloc_reps,
        &contracts,
        &interner,
        true,
    );
    let c = census(&func);
    assert!(
        c[0] >= 1,
        "COW-shared-survivor lineage must DEFER to the per-var pass — the fresh \
         keep-alive inc (RL-1 load-bearing) must survive, not be elided by the \
         re-balance; census = {c:?}"
    );
}

/// NEGATIVE / over-fire guard PIN: a PURE alias chain (`let b = a; a == b`) with
/// NO COW-mutation operand is STILL re-balanced — the COW exclusion must NOT
/// disable the legitimate release-exactly-once collapse. Zero incs + exactly one
/// dec survive (alloc(+1) − 1 = 0, RL-2). Mirrors
/// `lineage_rebalance_alias_chain_keeps_one_release` but exists as the explicit
/// guard that the COW gate term does not over-fire on a non-COW lineage. FAILS
/// if `cow_mutated_reps` wrongly flags a comparison/borrow-read lineage.
#[test]
fn lineage_rebalance_still_fires_on_pure_alias_chain() {
    use crate::ir::{ArcValue, PrimOp};
    use ori_ir::BinaryOp;
    // `%2 = %0 == %1` — a comparison BORROW-READS its operands (NOT a COW
    // consume); the alias chain must still be re-balanced.
    let mut func = alias_chain_func(vec![
        ArcInstr::Let {
            dst: v(2),
            ty: ty(0),
            value: ArcValue::PrimOp {
                op: PrimOp::Binary(BinaryOp::Eq),
                args: vec![v(0), v(1)],
            },
        },
        ArcInstr::BurdenInc { var: v(0) },
        ArcInstr::BurdenInc { var: v(1) },
        ArcInstr::BurdenDec { var: v(0) },
        ArcInstr::BurdenDec { var: v(1) },
    ]);
    run_elim_rebalance(&mut func);
    let c = census(&func);
    assert_eq!(
        c[0], 0,
        "pure alias chain (no COW operand) must STILL elide all incs — the COW \
         exclusion must not over-fire on a borrow-read comparison; census = {c:?}"
    );
    assert_eq!(
        c[1], 1,
        "pure alias chain must STILL keep exactly one RL-2 release; census = {c:?}"
    );
}

/// NEGATIVE / over-fire guard PIN: an aggregate transfer-forwarder rep is STILL
/// re-balanced (when its `transfers_through_return` provenance is proven) — the
/// COW exclusion must NOT suppress the forwarder re-balance path. This fixture
/// carries no COW-mutation operand, so the COW gate term is inert and the
/// forwarder lineage collapses to one release exactly as before. FAILS if the
/// COW gate term wrongly intercepts a non-COW forwarder lineage.
#[test]
fn lineage_rebalance_still_fires_on_transfer_forwarder() {
    // A pure alias chain with no COW operand stands in for the non-COW lineage
    // class the forwarder path also belongs to: the COW gate is inert (empty
    // `cow_mutated_reps`), so the re-balance collapses it to one release.
    let mut func = alias_chain_func(vec![
        ArcInstr::BurdenInc { var: v(0) },
        ArcInstr::BurdenInc { var: v(1) },
        ArcInstr::BurdenDec { var: v(0) },
        ArcInstr::BurdenDec { var: v(1) },
    ]);
    run_elim_rebalance(&mut func);
    let c = census(&func);
    assert_eq!(
        c[0], 0,
        "non-COW (forwarder-class) lineage must STILL elide all incs — the COW \
         exclusion is inert when no COW operand is present; census = {c:?}"
    );
    assert_eq!(
        c[1], 1,
        "non-COW (forwarder-class) lineage must STILL keep exactly one release; \
         census = {c:?}"
    );
}

// RL-1 duplication-pair coupling for param-rooted aliases
// (`collect_pair_atomic_alias_dsts` consumed by `mark_whole_var_removals`)

/// One-block func whose body aliases param `%0` into `%1` and carries the
/// given burden ops. The alias chain roots at a function param, so the
/// pair-coupling gate governs `%1`'s ops.
fn param_alias_func(extra_alias_hop: bool, ops: Vec<ArcInstr>) -> ArcFunction {
    let mut body = vec![ArcInstr::Let {
        dst: v(1),
        ty: ty(0),
        value: ArcValue::Var(v(0)),
    }];
    if extra_alias_hop {
        body.push(ArcInstr::Let {
            dst: v(2),
            ty: ty(0),
            value: ArcValue::Var(v(1)),
        });
    }
    body.extend(ops);
    ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: v(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: (0..3).map(ty).collect(),
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: Vec::new(),
            body,
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        ..Default::default()
    }
}

/// DP-3 fires on the alias's own (Once, Linear) state but DP-2 does not — the
/// decoupled split (inc elided, dec kept) nets -1 on the still-live param
/// lineage. The pair is ATOMIC: BOTH ops retained.
#[test]
fn param_rooted_alias_pair_kept_whole_when_dp2_fails() {
    let mut func = param_alias_func(
        false,
        vec![
            ArcInstr::BurdenInc { var: v(1) },
            ArcInstr::BurdenDec { var: v(1) },
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(1), owned_state(Cardinality::Once, Consumption::Linear))],
    );

    run_elim(&mut func, &state_map, false);

    let c = census(&func);
    assert_eq!(
        (c[0], c[1]),
        (1, 1),
        "param-rooted alias pair must be kept WHOLE (no inc-only split); census = {c:?}"
    );
}

/// Inc-only param-rooted alias (its dec was transfer-suppressed at Phase 5;
/// the move-out consumer owns the release): the inc backs the consumer's
/// cross-var release and must NEVER be elided, even though DP-3 fires.
#[test]
fn param_rooted_alias_inc_only_never_elided() {
    let mut func = param_alias_func(false, vec![ArcInstr::BurdenInc { var: v(1) }]);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(1), owned_state(Cardinality::Once, Consumption::Linear))],
    );

    run_elim(&mut func, &state_map, false);

    let c = census(&func);
    assert_eq!(
        c[0], 1,
        "inc-only param-rooted alias backs a cross-var release; census = {c:?}"
    );
}

/// Transitive chain `%2 = %1 = %0(param)`: the root walk resolves multi-hop
/// chains, so `%2`'s pair is coupled exactly like a direct alias of the param.
#[test]
fn param_rooted_alias_pair_coupling_is_transitive() {
    let mut func = param_alias_func(
        true,
        vec![
            ArcInstr::BurdenInc { var: v(2) },
            ArcInstr::BurdenDec { var: v(2) },
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(2), owned_state(Cardinality::Once, Consumption::Linear))],
    );

    run_elim(&mut func, &state_map, false);

    let c = census(&func);
    assert_eq!(
        (c[0], c[1]),
        (1, 1),
        "multi-hop param-rooted alias pair must be kept WHOLE; census = {c:?}"
    );
}

/// A NON-param-rooted alias (`%1 = %0` where `%0` is a plain local) stays on
/// the decoupled path: its lineage carries a birth-site `+1` the kept dec
/// releases, so the inc-only split (DP-3 fires, DP-2 does not) is preserved
/// behavior.
#[test]
fn non_param_rooted_alias_keeps_decoupled_split() {
    let body = vec![
        ArcInstr::Let {
            dst: v(1),
            ty: ty(0),
            value: ArcValue::Var(v(0)),
        },
        ArcInstr::BurdenInc { var: v(1) },
        ArcInstr::BurdenDec { var: v(1) },
    ];
    let mut func = one_block_func(2, body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(1), owned_state(Cardinality::Once, Consumption::Linear))],
    );

    run_elim(&mut func, &state_map, false);

    let c = census(&func);
    assert_eq!(
        (c[0], c[1]),
        (0, 1),
        "non-param-rooted alias keeps the decoupled split (inc elided, dec kept); census = {c:?}"
    );
}

/// Pair-atomic removal stays available: when DP-3 fires on the inc AND DP-2
/// fires on the dec (dead-on-arrival alias pair split across blocks), the
/// param-rooted pair is removed WHOLE on the co-emitter path.
#[test]
fn param_rooted_alias_pair_removed_whole_when_both_predicates_fire() {
    let mut func = param_alias_func(false, vec![ArcInstr::BurdenInc { var: v(1) }]);
    // Move the dec into a successor block whose exit state is Dead/Absent.
    func.blocks[0].terminator = ArcTerminator::Jump {
        target: block_id(1),
        args: Vec::new(),
    };
    func.blocks.push(ArcBlock {
        id: block_id(1),
        params: Vec::new(),
        body: vec![ArcInstr::BurdenDec { var: v(1) }],
        terminator: ArcTerminator::Return { value: v(0) },
    });
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(1), owned_state(Cardinality::Once, Consumption::Linear))],
    );
    seed_exit_state(
        &mut state_map,
        block_id(1),
        &[(v(1), owned_state(Cardinality::Absent, Consumption::Dead))],
    );

    run_elim(&mut func, &state_map, false);

    let c = census(&func);
    assert_eq!(
        (c[0], c[1]),
        (0, 0),
        "pair removed WHOLE when DP-3 and DP-2 both fire (co-emitter path); census = {c:?}"
    );
}

// RL-1 duplication-pair coupling for local-Construct-rooted terminal-store
// lineages (`collect_pair_atomic_alias_dsts` Construct-root admission)

/// Construct an aggregate-store instruction consuming `arg`.
fn store_consuming(dst: u32, arg: u32) -> ArcInstr {
    ArcInstr::Construct {
        dst: v(dst),
        ty: ty(0),
        ctor: CtorKind::Tuple,
        args: vec![v(arg)],
    }
}

/// One-block func: `%0 = Construct; %1 = %0 [pair ops]; %2 = %0;
/// %3 = Construct([%2])` — the lineage's terminal use is the aggregate store,
/// so the read-alias `%1`'s pair is ATOMIC (kept whole; splitting nets -1 —
/// the local terminal-move-store double-free).
#[test]
fn construct_rooted_terminal_store_alias_pair_kept_whole() {
    let body = vec![
        ArcInstr::Construct {
            dst: v(0),
            ty: ty(0),
            ctor: CtorKind::Tuple,
            args: Vec::new(),
        },
        ArcInstr::Let {
            dst: v(1),
            ty: ty(0),
            value: ArcValue::Var(v(0)),
        },
        ArcInstr::BurdenInc { var: v(1) },
        ArcInstr::BurdenDec { var: v(1) },
        ArcInstr::Let {
            dst: v(2),
            ty: ty(0),
            value: ArcValue::Var(v(0)),
        },
        store_consuming(3, 2),
    ];
    let mut func = one_block_func(4, body);
    func.blocks[0].terminator = ArcTerminator::Return { value: v(3) };
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(1), owned_state(Cardinality::Once, Consumption::Linear))],
    );

    run_elim(&mut func, &state_map, false);

    let c = census(&func);
    assert_eq!(
        (c[0], c[1]),
        (1, 1),
        "terminal-store Construct-rooted alias pair must be kept WHOLE; census = {c:?}"
    );
}

/// Same lineage but a read-alias is DEFINED (and its pair ops sit) AFTER the
/// store — the lineage is used past the store (the local genuine-dup shape),
/// so the decoupled split is preserved behavior (its -1 compensates the kept
/// FRESH-site inc until that over-emission's own cycle).
#[test]
fn construct_rooted_alias_used_after_store_keeps_split() {
    let body = vec![
        ArcInstr::Construct {
            dst: v(0),
            ty: ty(0),
            ctor: CtorKind::Tuple,
            args: Vec::new(),
        },
        ArcInstr::Let {
            dst: v(2),
            ty: ty(0),
            value: ArcValue::Var(v(0)),
        },
        store_consuming(3, 2),
        ArcInstr::Let {
            dst: v(1),
            ty: ty(0),
            value: ArcValue::Var(v(0)),
        },
        ArcInstr::BurdenInc { var: v(1) },
        ArcInstr::BurdenDec { var: v(1) },
    ];
    let mut func = one_block_func(4, body);
    func.blocks[0].terminator = ArcTerminator::Return { value: v(3) };
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(1), owned_state(Cardinality::Once, Consumption::Linear))],
    );

    run_elim(&mut func, &state_map, false);

    let c = census(&func);
    assert_eq!(
        (c[0], c[1]),
        (0, 1),
        "a lineage used past the store keeps the decoupled split; census = {c:?}"
    );
}

/// A Construct-rooted lineage with NO aggregate-store consume stays on the
/// decoupled path (the borrowed-call-arg compensation arrangement).
#[test]
fn construct_rooted_no_store_keeps_split() {
    let body = vec![
        ArcInstr::Construct {
            dst: v(0),
            ty: ty(0),
            ctor: CtorKind::Tuple,
            args: Vec::new(),
        },
        ArcInstr::Let {
            dst: v(1),
            ty: ty(0),
            value: ArcValue::Var(v(0)),
        },
        ArcInstr::BurdenInc { var: v(1) },
        ArcInstr::BurdenDec { var: v(1) },
    ];
    let mut func = one_block_func(2, body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(1), owned_state(Cardinality::Once, Consumption::Linear))],
    );

    run_elim(&mut func, &state_map, false);

    let c = census(&func);
    assert_eq!(
        (c[0], c[1]),
        (0, 1),
        "a store-free Construct-rooted lineage keeps the decoupled split; census = {c:?}"
    );
}

/// Loop back-edge re-reach counts as use-after-store: a store inside a loop
/// whose header re-reads the lineage stays DECOUPLED (mirrors the Phase-5
/// reachability discriminator).
#[test]
fn construct_rooted_store_in_loop_with_reread_keeps_split() {
    // bb0: %0 = Construct; %1 = %0 [pair]; Jump bb1
    // bb1: %2 = %0; store(%3, [%2]); Branch -> bb1 | bb2  (back edge re-reaches the store block)
    // bb2: Return
    let mut func = one_block_func(5, Vec::new());
    func.blocks[0].body = vec![
        ArcInstr::Construct {
            dst: v(0),
            ty: ty(0),
            ctor: CtorKind::Tuple,
            args: Vec::new(),
        },
        ArcInstr::Let {
            dst: v(1),
            ty: ty(0),
            value: ArcValue::Var(v(0)),
        },
        ArcInstr::BurdenInc { var: v(1) },
        ArcInstr::BurdenDec { var: v(1) },
    ];
    func.blocks[0].terminator = ArcTerminator::Jump {
        target: block_id(1),
        args: Vec::new(),
    };
    func.blocks.push(ArcBlock {
        id: block_id(1),
        params: Vec::new(),
        body: vec![
            ArcInstr::Let {
                dst: v(2),
                ty: ty(0),
                value: ArcValue::Var(v(0)),
            },
            store_consuming(3, 2),
        ],
        terminator: ArcTerminator::Branch {
            cond: v(4),
            then_block: block_id(1),
            else_block: block_id(2),
        },
    });
    func.blocks.push(ArcBlock {
        id: block_id(2),
        params: Vec::new(),
        body: Vec::new(),
        terminator: ArcTerminator::Return { value: v(3) },
    });
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(1), owned_state(Cardinality::Once, Consumption::Linear))],
    );

    run_elim(&mut func, &state_map, false);

    let c = census(&func);
    assert_eq!(
        (c[0], c[1]),
        (0, 1),
        "a back-edge re-reached store block is not terminal; split preserved; census = {c:?}"
    );
}
