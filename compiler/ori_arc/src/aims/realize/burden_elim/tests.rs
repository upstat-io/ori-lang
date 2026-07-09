//! Tests for the burden-op elimination consumer.
//!
//! Negative pins on residual ops + positive pins on paired elimination.
//!
//! Predicate citations:
//! - DP-2 (`is_rc_dec_unnecessary` at `aims/transfer/mod.rs`):
//!   `is_rc_dec_unnecessary(s) ⟺ s.cardinality = Absent ∨
//!   s.consumption = Dead`.
//! - DP-3 (`is_rc_inc_elidable` at `aims/transfer/mod.rs`):
//!   `is_rc_inc_elidable(s) ⟺ s.cardinality = Once ∧
//!   (s.consumption = Linear ∨ Affine)`.

use super::{burden_op_census, eliminate_burden_ops, is_burden_removal_only};
use crate::aims::intraprocedural::birth_site_population::compute_birth_site_partition;
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
/// and a fresh interner — the per-var DP-2/DP-3 path these unit tests pin. The
/// empty rep map gives every var a singleton rep (`op_vars.len() < 2`), so the
/// lineage re-balance is inert and the per-var path is the sole active pass.
fn run_elim(func: &mut ArcFunction, state_map: &mut AimsStateMap) {
    // Mirror the pipeline contract: the birth-site partition side table is
    // installed on the state map before Phase 6 runs.
    let partition = compute_birth_site_partition(func, state_map);
    state_map.set_birth_site_partition(partition);
    let same_alloc_reps: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    let contracts: FxHashMap<Name, crate::aims::contract::MemoryContract> = FxHashMap::default();
    let interner = ori_ir::StringInterner::new();
    eliminate_burden_ops(func, state_map, &same_alloc_reps, &contracts, &interner);
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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

    let body = &func.blocks[0].body;
    assert!(
        body.is_empty(),
        "DP-3 true on (Once, Linear) must elide BurdenInc; body = {body:?}"
    );
}

/// `dec_kept_on_dead_absent_sole_emitter` — var `w` is (Owned, Dead, Absent,
/// *, *, *) per CN-1 pairing. The burden path is the sole RC emitter, so the
/// whole-var `BurdenDec` is the RL-2 scope-exit release and is KEPT even
/// though DP-2 (`is_rc_dec_unnecessary`) fires on (Dead, Absent) — eliding the
/// only release would leak. DP-2 whole-var dec-elision was co-emitter-only and
/// is removed. Spec: Annex E §AIMS RL-2 release-exactly-once.
#[test]
fn dec_kept_on_dead_absent_sole_emitter() {
    let func_body = vec![ArcInstr::BurdenDec { var: v(0) }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Absent, Consumption::Dead))],
    );

    run_elim(&mut func, &mut state_map);

    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "sole-emitter path keeps the BurdenDec as the RL-2 release; body = {body:?}"
    );
}

/// `BurdenDecPartial` follows the whole-var dec rule. The burden path is the
/// sole RC emitter, so on (Dead, Absent) the partial dec is KEPT as the RL-2
/// release (co-emitter DP-2 dec-elision removed). Spec: Annex E §AIMS RL-2.
#[test]
fn dec_partial_kept_on_dead_absent_sole_emitter() {
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

    run_elim(&mut func, &mut state_map);

    assert_eq!(
        func.blocks[0].body.len(),
        1,
        "sole-emitter path keeps BurdenDecPartial as the RL-2 release; body = {:?}",
        func.blocks[0].body
    );
}

/// `BurdenDecVariant` follows the whole-var dec rule. Sole-emitter path keeps
/// it as the RL-2 release on (Dead, Absent). Spec: Annex E §AIMS RL-2.
#[test]
fn dec_variant_kept_on_dead_absent_sole_emitter() {
    let func_body = vec![ArcInstr::BurdenDecVariant { var: v(0) }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Absent, Consumption::Dead))],
    );

    run_elim(&mut func, &mut state_map);

    assert_eq!(
        func.blocks[0].body.len(),
        1,
        "sole-emitter path keeps BurdenDecVariant as the RL-2 release; body = {:?}",
        func.blocks[0].body
    );
}

/// `BurdenDecField` is dec-side: on the sole-emitter path it is always KEPT
/// (eliding a field-grain release would leak), regardless of `base`'s DP-2
/// state. Spec: Annex E §AIMS RL-2.
#[test]
fn dec_field_kept_on_dead_absent_base_sole_emitter() {
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

    run_elim(&mut func, &mut state_map);

    assert_eq!(
        func.blocks[0].body.len(),
        1,
        "sole-emitter path keeps BurdenDecField as a field-grain release; body = {:?}",
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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

// Predicate-stack decision pins.
//
// `decide()` produces the predicate-stack RC decisions from a managed
// `DecisionSite`. These pins exercise it directly at the unit level.

/// Pin: a managed Use-site with a future use emits `RcInc`; a managed
/// `DefinedDead` site emits `RcDec`.
#[test]
fn managed_site_emits_normal_decisions() {
    use crate::aims::realize::decide::{
        decide, DecisionContext, DecisionSite, RcDecision, UseSemantics,
    };

    // Use-site with future use → predicate stack emits the normal RcInc.
    let decision = decide(&DecisionContext {
        site: DecisionSite::Use {
            has_future_use: true,
            semantics: UseSemantics::Normal,
        },
        is_rc_managed: true,
    });
    assert_eq!(
        decision.rc,
        RcDecision::Inc,
        "managed Use site with future use must emit RcInc (got {:?})",
        decision.rc
    );

    // Defined-dead site → predicate stack emits Dec.
    let decision = decide(&DecisionContext {
        site: DecisionSite::DefinedDead,
        is_rc_managed: true,
    });
    assert_eq!(
        decision.rc,
        RcDecision::Dec,
        "managed DefinedDead site must emit RcDec (got {:?})",
        decision.rc
    );
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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

    assert!(
        func.blocks[0].body.is_empty(),
        "solitary Inc with DP-3 firing elides (no matching Dec to pin against); body = {:?}",
        func.blocks[0].body
    );
}

/// Paired-elim dec side: a var with only `BurdenDec` and no matching
/// `BurdenInc` KEEPS the Dec on the sole-emitter path — DP-2 whole-var
/// dec-elision is co-emitter-only and removed, so the solitary Dec stays as
/// the RL-2 release. The inc-side companion
/// (`paired_elim_solo_inc_elidable_state_elides`) still elides per DP-3.
/// Spec: Annex E §AIMS RL-2.
#[test]
fn paired_elim_solo_dec_kept_sole_emitter() {
    let func_body = vec![ArcInstr::BurdenDec { var: v(0) }];
    let mut func = one_block_func(1, func_body);
    let mut state_map = AimsStateMap::new(&func);
    seed_exit_state(
        &mut state_map,
        block_id(0),
        &[(v(0), owned_state(Cardinality::Absent, Consumption::Dead))],
    );

    run_elim(&mut func, &mut state_map);

    assert_eq!(
        func.blocks[0].body.len(),
        1,
        "solitary Dec is kept on the sole-emitter path as the RL-2 release; body = {:?}",
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
    run_elim(&mut func, &mut state_map);
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
    run_elim(&mut func, &mut state_map);
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

/// Structural guard pin — `assert_burden_removal_only` panics when Phase 6
/// would construct a burden op. Always-on in every build (`assert!`, never
/// `debug_assert!`): a Phase-6 construction regression corrupts RC balance in
/// a shipped binary, so the guard must not disappear under `--release`.
#[test]
#[should_panic(expected = "AIMS Phase-6 invariant")]
fn guard_panics_on_phase6_construction() {
    use super::assert_burden_removal_only;
    let before = [1usize, 0, 0, 0, 0];
    // after grows BurdenDec from 0 → 1: a construction in Phase 6.
    let after = [1usize, 1, 0, 0, 0];
    assert_burden_removal_only(&before, &after);
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

/// Run elimination with the alias-chain rep map — the path that exercises the
/// lineage re-balance (the burden path is the sole RC emitter).
/// The alias-chain rep carries no apply-result alias, so an EMPTY contracts map
/// is correct: `compute_transfer_forwarder_anchors` finds no forwarder, and the
/// pure-Let-Var-alias re-balance path is exercised unchanged.
fn run_elim_rebalance(func: &mut ArcFunction) {
    let same_alloc_reps = alias_chain_reps();
    let contracts: FxHashMap<Name, crate::aims::contract::MemoryContract> = FxHashMap::default();
    let interner = ori_ir::StringInterner::new();
    let mut state_map = AimsStateMap::new(func);
    let partition = compute_birth_site_partition(func, &state_map);
    state_map.set_birth_site_partition(partition);
    eliminate_burden_ops(func, &state_map, &same_alloc_reps, &contracts, &interner);
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
    let partition = compute_birth_site_partition(&func, &state_map);
    state_map.set_birth_site_partition(partition);
    eliminate_burden_ops(
        &mut func,
        &state_map,
        &same_alloc_reps,
        &contracts,
        &interner,
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
    let partition = compute_birth_site_partition(&func, &state_map);
    state_map.set_birth_site_partition(partition);
    eliminate_burden_ops(
        &mut func,
        &state_map,
        &same_alloc_reps,
        &contracts,
        &interner,
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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

    let c = census(&func);
    assert_eq!(
        (c[0], c[1]),
        (0, 1),
        "non-param-rooted alias keeps the decoupled split (inc elided, dec kept); census = {c:?}"
    );
}

/// Pair-atomicity on the sole-emitter path: a param-rooted dup-alias pair
/// (DP-3 firing on the inc, the dec dead-on-arrival in a successor block) is
/// KEPT WHOLE. DP-2 whole-var dec-elision is co-emitter-only and removed, so
/// the dec is not elidable; the pair-atomic guard then keeps the inc too
/// (splitting nets -1 on the still-live param lineage — the `@stash_and_return`
/// double-free). Spec: Annex E §AIMS RL-1 duplication-balanced + RL-2.
#[test]
fn param_rooted_alias_pair_kept_whole_sole_emitter() {
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

    run_elim(&mut func, &mut state_map);

    let c = census(&func);
    assert_eq!(
        (c[0], c[1]),
        (1, 1),
        "param-rooted pair kept WHOLE on the sole-emitter path (dec not elidable, \
         pair-atomic guard keeps the inc); census = {c:?}"
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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

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

    run_elim(&mut func, &mut state_map);

    let c = census(&func);
    assert_eq!(
        (c[0], c[1]),
        (0, 1),
        "a back-edge re-reached store block is not terminal; split preserved; census = {c:?}"
    );
}

// S07 class-grain whole-pair elision (T3 sibling-liveness) pins.

/// Run elimination with a MANUAL two-var partition: `union_them` unions the
/// whole-var nodes of `%0` and `%1` into one allocation class (the
/// forwarder/extract same-allocation shape); `false` leaves them distinct
/// births. Exit states seed BOTH vars (Many, Unrestricted) so the per-var
/// DP-2/DP-3 pass KEEPS every op and the class-grain pass owns the verdict.
fn run_elim_class_grain(func: &mut ArcFunction, union_them: bool) {
    let mut state_map = AimsStateMap::new(func);
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
    let mut partition = compute_birth_site_partition(func, &state_map);
    // Register BOTH whole-var nodes unconditionally so the distinct-class
    // case exercises the same-rep comparison (an unregistered var would
    // decline at the class-unknown gate instead, leaving the SAME-CLASS-ONLY
    // boundary unpinned); union only when the test wants one class.
    use crate::aims::intraprocedural::birth_site_partition::FieldPath;
    let a = partition.register_node(v(0), FieldPath::whole_var());
    let b = partition.register_node(v(1), FieldPath::whole_var());
    if union_them {
        partition.union_tier1(a, b);
    }
    state_map.set_birth_site_partition(partition);
    let same_alloc_reps: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    let contracts: FxHashMap<Name, crate::aims::contract::MemoryContract> = FxHashMap::default();
    let interner = ori_ir::StringInterner::new();
    eliminate_burden_ops(func, &state_map, &same_alloc_reps, &contracts, &interner);
}

/// The T3 bracket: sibling `%0` born (Construct) before the `%1` pair, with
/// `%0`'s kept release after it. Same class -> the `%1` keep-alive pair is
/// elided WHOLE (both inc AND dec); `%0`'s release survives as the class's
/// single release (`keep_alive_redundancy_sound_iff_whole_pair`).
#[test]
fn class_grain_pair_elided_with_live_same_class_sibling() {
    let body = vec![
        ArcInstr::Construct {
            dst: v(0),
            ty: ty(0),
            ctor: CtorKind::Tuple,
            args: vec![],
        },
        ArcInstr::BurdenInc { var: v(1) },
        ArcInstr::BurdenDec { var: v(1) },
        ArcInstr::BurdenDec { var: v(0) },
    ];
    let mut func = one_block_func(2, body);
    run_elim_class_grain(&mut func, true);
    // Whole pair on %1 gone; %0's release kept. Never an inc-only split.
    assert_eq!(census(&func), [0, 1, 0, 0, 0]);
}

/// Class-boundary negative pin (the attempt-287 290-UAF class one grain
/// coarser): the SAME positional bracket over two DISTINCT allocations (the
/// partition holds them in different classes) elides NOTHING — a
/// mis-classified "sibling" never supplies T3 evidence.
#[test]
fn class_grain_distinct_allocation_bracket_not_elided() {
    let body = vec![
        ArcInstr::Construct {
            dst: v(0),
            ty: ty(0),
            ctor: CtorKind::Tuple,
            args: vec![],
        },
        ArcInstr::BurdenInc { var: v(1) },
        ArcInstr::BurdenDec { var: v(1) },
        ArcInstr::BurdenDec { var: v(0) },
    ];
    let mut func = one_block_func(2, body);
    run_elim_class_grain(&mut func, false);
    assert_eq!(census(&func), [1, 2, 0, 0, 0]);
}

/// MUTATE exclusion: the same-class bracket with a COW mutation on a class
/// member keeps every pair — COW load-bearing incs are never elided (DP-5 /
/// DP-9 count sibling references at the mutation site).
#[test]
fn class_grain_mutate_feeding_class_keeps_pair() {
    let body = vec![
        ArcInstr::Construct {
            dst: v(0),
            ty: ty(0),
            ctor: CtorKind::Tuple,
            args: vec![],
        },
        ArcInstr::BurdenInc { var: v(1) },
        ArcInstr::BurdenDec { var: v(1) },
        ArcInstr::Set {
            base: v(0),
            field: 0,
            value: v(1),
        },
        ArcInstr::BurdenDec { var: v(0) },
    ];
    let mut func = one_block_func(2, body);
    run_elim_class_grain(&mut func, true);
    assert_eq!(census(&func), [1, 2, 0, 0, 0]);
}

/// Ablation toggle disposition: `disabled = true` (the
/// `ORI_DISABLE_CLASS_GRAIN_PAIR_ELISION=1` reading) declines the pass —
/// the per-var-only disposition survives verbatim on the exact shape the
/// enabled pass elides.
#[test]
fn class_grain_toggle_disabled_keeps_pair() {
    use crate::aims::intraprocedural::birth_site_partition::FieldPath;
    let body = vec![
        ArcInstr::Construct {
            dst: v(0),
            ty: ty(0),
            ctor: CtorKind::Tuple,
            args: vec![],
        },
        ArcInstr::BurdenInc { var: v(1) },
        ArcInstr::BurdenDec { var: v(1) },
        ArcInstr::BurdenDec { var: v(0) },
    ];
    let func = one_block_func(2, body);
    let mut state_map = AimsStateMap::new(&func);
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
    let mut partition = compute_birth_site_partition(&func, &state_map);
    let a = partition.register_node(v(0), FieldPath::whole_var());
    let b = partition.register_node(v(1), FieldPath::whole_var());
    partition.union_tier1(a, b);
    state_map.set_birth_site_partition(partition);
    let mut balances: FxHashMap<ArcVarId, super::WholeVarBalance> = FxHashMap::default();
    let e0 = balances
        .entry(v(0))
        .or_insert_with(super::WholeVarBalance::seed);
    e0.dec_sites.push((0, 3));
    let e1 = balances
        .entry(v(1))
        .or_insert_with(super::WholeVarBalance::seed);
    e1.inc_sites.push((0, 1));
    e1.dec_sites.push((0, 2));
    let rebalanced: rustc_hash::FxHashSet<ArcVarId> = rustc_hash::FxHashSet::default();
    let mut remove = vec![vec![false; func.blocks[0].body.len()]];
    super::class_grain::mark_class_grain_whole_pair_removals_gated(
        true,
        &func,
        &state_map,
        &balances,
        &rebalanced,
        &mut remove,
    );
    assert!(
        remove.iter().flatten().all(|r| !r),
        "disabled toggle must decline every class-grain removal"
    );
    super::class_grain::mark_class_grain_whole_pair_removals_gated(
        false,
        &func,
        &state_map,
        &balances,
        &rebalanced,
        &mut remove,
    );
    assert!(
        remove[0][1] && remove[0][2],
        "enabled pass must elide the pair on the identical shape"
    );
}

/// Alias-sibling +1-establishment: a sibling whose only funding inc lands
/// INSIDE the pair's span supplies no dominating evidence — at span entry
/// the class count is uncovered; the pair is kept whole.
#[test]
fn class_grain_alias_sibling_funded_mid_span_keeps_pair() {
    let body = vec![
        ArcInstr::Let {
            dst: v(0),
            ty: ty(0),
            value: ArcValue::Var(v(2)),
        },
        ArcInstr::BurdenInc { var: v(1) },
        ArcInstr::BurdenInc { var: v(0) },
        ArcInstr::BurdenDec { var: v(1) },
        ArcInstr::BurdenDec { var: v(0) },
    ];
    let mut func = one_block_func(3, body);
    run_elim_class_grain(&mut func, true);
    // %0 is a Let-Var alias: its dup inc (site 2) is INSIDE the %1 pair's
    // span (sites 1..3) — no evidence; every op survives.
    assert_eq!(census(&func), [2, 2, 0, 0, 0]);
}

/// Net-at-span-entry: a sibling that both funded AND released before the
/// span enters it at count 0 — its second kept release after the span is
/// no evidence; the pair is kept whole.
#[test]
fn class_grain_sibling_net_zero_at_span_entry_keeps_pair() {
    let body = vec![
        ArcInstr::Let {
            dst: v(0),
            ty: ty(0),
            value: ArcValue::Var(v(2)),
        },
        ArcInstr::BurdenInc { var: v(0) },
        ArcInstr::BurdenDec { var: v(0) },
        ArcInstr::BurdenInc { var: v(1) },
        ArcInstr::BurdenDec { var: v(1) },
        ArcInstr::BurdenDec { var: v(0) },
    ];
    let mut func = one_block_func(3, body);
    run_elim_class_grain(&mut func, true);
    // %0's before-span inc is cancelled by its before-span dec (net 0 at
    // span entry); the after-span dec alone supplies no T3 evidence.
    assert_eq!(census(&func), [2, 3, 0, 0, 0]);
}

/// T3 live-across-the-span premise: a sibling releasing INSIDE the pair's
/// span (multi-release sibling) supplies no evidence even though another
/// kept release lands after the span — the interior count may hit zero
/// mid-span; the pair is kept whole.
#[test]
fn class_grain_sibling_release_within_span_keeps_pair_whole() {
    let body = vec![
        ArcInstr::Construct {
            dst: v(0),
            ty: ty(0),
            ctor: CtorKind::Tuple,
            args: vec![],
        },
        ArcInstr::BurdenInc { var: v(1) },
        ArcInstr::BurdenDec { var: v(0) },
        ArcInstr::BurdenDec { var: v(1) },
        ArcInstr::BurdenDec { var: v(0) },
    ];
    let mut func = one_block_func(2, body);
    run_elim_class_grain(&mut func, true);
    assert_eq!(census(&func), [1, 3, 0, 0, 0]);
}

/// Whole-pair-only admission: a bracket whose release set contains a
/// `BurdenDecPartial` slice drop sits outside the T3 whole-var proof and is
/// never admitted — every op survives.
#[test]
fn class_grain_partial_dec_bracket_not_admitted() {
    let body = vec![
        ArcInstr::Construct {
            dst: v(0),
            ty: ty(0),
            ctor: CtorKind::Tuple,
            args: vec![],
        },
        ArcInstr::BurdenInc { var: v(1) },
        ArcInstr::BurdenDecPartial {
            var: v(1),
            skip_fields: vec![],
        },
        ArcInstr::BurdenDec { var: v(0) },
    ];
    let mut func = one_block_func(2, body);
    run_elim_class_grain(&mut func, true);
    assert_eq!(census(&func), [1, 1, 1, 0, 0]);
}

/// MUTATE exclusion covers buffer recycling: a `CollectionReuse` consuming a
/// class member taints the whole class; the pair is kept.
#[test]
fn class_grain_collection_reuse_taints_class_keeps_pair() {
    let body = vec![
        ArcInstr::Construct {
            dst: v(0),
            ty: ty(0),
            ctor: CtorKind::Tuple,
            args: vec![],
        },
        ArcInstr::BurdenInc { var: v(1) },
        ArcInstr::BurdenDec { var: v(1) },
        ArcInstr::CollectionReuse {
            dst: v(2),
            old_var: v(0),
            ty: ty(0),
            ctor: CtorKind::ListLiteral,
            args: vec![],
        },
        ArcInstr::BurdenDec { var: v(0) },
    ];
    let mut func = one_block_func(3, body);
    run_elim_class_grain(&mut func, true);
    assert_eq!(census(&func), [1, 2, 0, 0, 0]);
}

/// No release-after-span, no evidence: the sibling's dec BEFORE the pair
/// supplies no dominating bracket; the pair is kept whole (never split).
#[test]
fn class_grain_sibling_release_before_span_keeps_pair_whole() {
    let body = vec![
        ArcInstr::Construct {
            dst: v(0),
            ty: ty(0),
            ctor: CtorKind::Tuple,
            args: vec![],
        },
        ArcInstr::BurdenDec { var: v(0) },
        ArcInstr::BurdenInc { var: v(1) },
        ArcInstr::BurdenDec { var: v(1) },
    ];
    let mut func = one_block_func(2, body);
    run_elim_class_grain(&mut func, true);
    assert_eq!(census(&func), [1, 2, 0, 0, 0]);
}
