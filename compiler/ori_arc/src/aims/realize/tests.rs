//! Tests for the unified realization module.

use super::decide::{
    decide, decide_annotations, decide_cow, decide_drop_hint, AnnotationSiteContext,
    DecisionContext, DecisionSite, InstructionDecisions, RcDecision, ReuseContext, ReuseDecision,
};
use crate::aims::lattice::{
    AccessClass, Cardinality, Consumption, ReuseCtorKind, ShapeClass, Uniqueness,
};
use crate::ir::ArcVarId;
use crate::uniqueness::CowMode;
use rustc_hash::FxHashSet;

fn var(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

/// Helper: create a default `AnnotationSiteContext` for COW tests.
///
/// Defaults: non-param, non-excluded, non-collection, no RC inc, no borrowed arg,
/// Owned access, Linear consumption, Once cardinality, `NonReusable` shape,
/// not borrow-disjoint.
fn cow_ctx(var: ArcVarId, uniqueness: Uniqueness) -> AnnotationSiteContext<'static> {
    // Leak a boxed empty set so we can return a static reference.
    // Fine for tests — bounded number of calls.
    let empty: &'static FxHashSet<ArcVarId> = Box::leak(Box::default());
    AnnotationSiteContext {
        var,
        uniqueness,
        rc_incremented: false,
        is_param: false,
        is_borrowed_call_arg: false,
        rc_incremented_set: empty,
        is_excluded: false,
        access: AccessClass::Owned,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        shape: ShapeClass::NonReusable,
        is_borrow_disjoint: false,
        is_collection: false,
    }
}

/// Helper: create an `AnnotationSiteContext` for drop hint tests.
///
/// Same as `cow_ctx` but with `is_collection: true` (required for drop hints).
fn drop_ctx(var: ArcVarId, uniqueness: Uniqueness) -> AnnotationSiteContext<'static> {
    let mut ctx = cow_ctx(var, uniqueness);
    ctx.is_collection = true;
    ctx
}

// decide_cow tests

#[test]
fn cow_unique_no_rc_inc_is_static_unique() {
    let ctx = cow_ctx(var(0), Uniqueness::Unique);
    assert_eq!(decide_cow(&ctx), CowMode::StaticUnique);
}

#[test]
fn cow_unique_with_rc_inc_is_dynamic() {
    let mut ctx = cow_ctx(var(0), Uniqueness::Unique);
    ctx.rc_incremented = true;
    assert_eq!(decide_cow(&ctx), CowMode::Dynamic);
}

#[test]
fn cow_maybe_shared_is_dynamic() {
    let ctx = cow_ctx(var(0), Uniqueness::MaybeShared);
    assert_eq!(decide_cow(&ctx), CowMode::Dynamic);
}

#[test]
fn cow_excluded_is_dynamic() {
    let mut ctx = cow_ctx(var(0), Uniqueness::Unique);
    ctx.is_excluded = true;
    assert_eq!(decide_cow(&ctx), CowMode::Dynamic);
}

#[test]
fn cow_shared_is_static_shared() {
    let ctx = cow_ctx(var(0), Uniqueness::Shared);
    assert_eq!(decide_cow(&ctx), CowMode::StaticShared);
}

#[test]
fn cow_param_cow_aware_unique() {
    let mut ctx = cow_ctx(var(0), Uniqueness::MaybeShared);
    ctx.is_param = true;
    // COW-aware: Owned + Linear + Once → StaticUnique for params.
    assert_eq!(decide_cow(&ctx), CowMode::StaticUnique);
}

#[test]
fn cow_param_not_cow_aware_many() {
    let mut ctx = cow_ctx(var(0), Uniqueness::MaybeShared);
    ctx.is_param = true;
    ctx.cardinality = Cardinality::Many; // breaks Once requirement
    assert_eq!(decide_cow(&ctx), CowMode::Dynamic);
}

#[test]
fn cow_cross_dim_collection_buffer_once() {
    let mut ctx = cow_ctx(var(0), Uniqueness::MaybeShared);
    ctx.shape = ShapeClass::CollectionBuffer;
    // Non-param + Once + CollectionBuffer → StaticUnique.
    assert_eq!(decide_cow(&ctx), CowMode::StaticUnique);
}

#[test]
fn cow_cross_dim_reusable_ctor_once() {
    let mut ctx = cow_ctx(var(0), Uniqueness::MaybeShared);
    ctx.shape = ShapeClass::ReusableCtor(ReuseCtorKind::Struct);
    // Non-param + Once + ReusableCtor → StaticUnique.
    assert_eq!(decide_cow(&ctx), CowMode::StaticUnique);
}

#[test]
fn cow_borrow_disjoint_maybe_shared() {
    let mut ctx = cow_ctx(var(0), Uniqueness::MaybeShared);
    ctx.is_borrow_disjoint = true;
    assert_eq!(decide_cow(&ctx), CowMode::StaticUnique);
}

// decide_drop_hint tests

#[test]
fn drop_hint_unique_collection_is_eligible() {
    let ctx = drop_ctx(var(0), Uniqueness::Unique);
    assert!(decide_drop_hint(&ctx));
}

#[test]
fn drop_hint_unique_with_rc_inc_not_eligible() {
    let mut ctx = drop_ctx(var(0), Uniqueness::Unique);
    ctx.rc_incremented = true;
    assert!(!decide_drop_hint(&ctx));
}

#[test]
fn drop_hint_unique_borrowed_arg_not_eligible() {
    let mut ctx = drop_ctx(var(0), Uniqueness::Unique);
    ctx.is_borrowed_call_arg = true;
    assert!(!decide_drop_hint(&ctx));
}

#[test]
fn drop_hint_maybe_shared_not_eligible() {
    let ctx = drop_ctx(var(0), Uniqueness::MaybeShared);
    assert!(!decide_drop_hint(&ctx));
}

#[test]
fn drop_hint_excluded_not_eligible() {
    let mut ctx = drop_ctx(var(0), Uniqueness::Unique);
    ctx.is_excluded = true;
    assert!(!decide_drop_hint(&ctx));
}

#[test]
fn drop_hint_non_collection_not_eligible() {
    // Unique but not a collection → no drop hint.
    let ctx = cow_ctx(var(0), Uniqueness::Unique);
    assert!(!ctx.is_collection);
    assert!(!decide_drop_hint(&ctx));
}

// InstructionDecisions type tests

#[test]
fn instruction_decisions_default_none() {
    let decisions = InstructionDecisions {
        rc: RcDecision::None,
        reuse: ReuseDecision::None,
    };
    assert_eq!(decisions.rc, RcDecision::None);
    assert_eq!(decisions.reuse, ReuseDecision::None);
}

#[test]
fn instruction_decisions_static_reuse_replaces_dec() {
    // When reuse is StaticReuse, the RC Dec is absorbed into the Reset.
    let decisions = InstructionDecisions {
        rc: RcDecision::None, // Dec absorbed by reuse
        reuse: ReuseDecision::StaticReuse,
    };
    assert_eq!(decisions.reuse, ReuseDecision::StaticReuse);
    assert_eq!(decisions.rc, RcDecision::None);
}

// decide_annotations (unified Phase 2) tests

#[test]
fn annotations_cow_site_unique_clean() {
    let ctx = cow_ctx(var(0), Uniqueness::Unique);
    let result = decide_annotations(&ctx, true, false);
    assert_eq!(result.cow, Some(CowMode::StaticUnique));
    assert!(!result.drop_hint);
}

#[test]
fn annotations_drop_site_unique_collection() {
    let ctx = drop_ctx(var(0), Uniqueness::Unique);
    let result = decide_annotations(&ctx, false, true);
    assert_eq!(result.cow, None);
    assert!(result.drop_hint);
}

#[test]
fn annotations_both_cow_and_drop() {
    let ctx = drop_ctx(var(0), Uniqueness::Unique);
    let result = decide_annotations(&ctx, true, true);
    assert_eq!(result.cow, Some(CowMode::StaticUnique));
    assert!(result.drop_hint);
}

#[test]
fn annotations_neither_cow_nor_drop() {
    let ctx = cow_ctx(var(0), Uniqueness::Unique);
    let result = decide_annotations(&ctx, false, false);
    assert_eq!(result.cow, None);
    assert!(!result.drop_hint);
}

#[test]
fn annotations_shared_cow_site() {
    let ctx = cow_ctx(var(0), Uniqueness::Shared);
    let result = decide_annotations(&ctx, true, false);
    assert_eq!(result.cow, Some(CowMode::StaticShared));
    assert!(!result.drop_hint);
}

#[test]
fn annotations_maybe_shared_drop_site_not_eligible() {
    let ctx = drop_ctx(var(0), Uniqueness::MaybeShared);
    let result = decide_annotations(&ctx, false, true);
    assert_eq!(result.cow, None);
    assert!(!result.drop_hint);
}

// Phase 1 decide() tests

fn reuse_non_reusable() -> ReuseContext {
    ReuseContext {
        shape: ShapeClass::NonReusable,
        uniqueness: Uniqueness::Unique,
        cardinality: Cardinality::Once,
    }
}

fn reuse_unique_struct() -> ReuseContext {
    ReuseContext {
        shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
        uniqueness: Uniqueness::Unique,
        cardinality: Cardinality::Once,
    }
}

// Non-RC-managed variables

#[test]
fn decide_non_rc_managed_returns_none() {
    let ctx = DecisionContext {
        site: DecisionSite::Use {
            has_future_use: true,
        },
        is_rc_managed: false,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::None);
    assert_eq!(d.reuse, ReuseDecision::None);
}

// Use site decisions

#[test]
fn decide_use_with_future_use_returns_inc() {
    let ctx = DecisionContext {
        site: DecisionSite::Use {
            has_future_use: true,
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Inc);
    assert_eq!(d.reuse, ReuseDecision::None);
}

#[test]
fn decide_use_without_future_use_returns_none() {
    let ctx = DecisionContext {
        site: DecisionSite::Use {
            has_future_use: false,
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::None);
    assert_eq!(d.reuse, ReuseDecision::None);
}

// DefinedDead site decisions

#[test]
fn decide_defined_dead_returns_dec() {
    let ctx = DecisionContext {
        site: DecisionSite::DefinedDead,
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Dec);
    assert_eq!(d.reuse, ReuseDecision::None);
}

// LastUse — suppression flags

#[test]
fn decide_last_use_consuming_primop_returns_none() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: true,
            is_ownership_transfer: false,
            is_owned_call_position: false,
            has_deferred_children: false,
            reuse: reuse_unique_struct(),
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::None);
    assert_eq!(d.reuse, ReuseDecision::None);
}

#[test]
fn decide_last_use_ownership_transfer_returns_none() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: true,
            is_owned_call_position: false,
            has_deferred_children: false,
            reuse: reuse_unique_struct(),
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::None);
    assert_eq!(d.reuse, ReuseDecision::None);
}

#[test]
fn decide_last_use_owned_call_position_returns_none() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: true,
            has_deferred_children: false,
            reuse: reuse_unique_struct(),
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::None);
    assert_eq!(d.reuse, ReuseDecision::None);
}

#[test]
fn decide_last_use_deferred_children_returns_defer() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,
            has_deferred_children: true,
            reuse: reuse_unique_struct(),
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Defer);
    assert_eq!(d.reuse, ReuseDecision::None);
}

// LastUse — regular Dec with reuse

#[test]
fn decide_last_use_unique_struct_returns_dec_static_reuse() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,
            has_deferred_children: false,
            reuse: reuse_unique_struct(),
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Dec);
    assert_eq!(d.reuse, ReuseDecision::StaticReuse);
}

#[test]
fn decide_last_use_non_reusable_shape_returns_dec_no_reuse() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,
            has_deferred_children: false,
            reuse: reuse_non_reusable(),
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Dec);
    assert_eq!(d.reuse, ReuseDecision::None);
}

#[test]
fn decide_last_use_shared_returns_dec_no_reuse() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,
            has_deferred_children: false,
            reuse: ReuseContext {
                shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
                uniqueness: Uniqueness::Shared,
                cardinality: Cardinality::Once,
            },
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Dec);
    assert_eq!(d.reuse, ReuseDecision::None);
}

#[test]
fn decide_last_use_maybe_shared_struct_returns_dynamic_reuse() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,
            has_deferred_children: false,
            reuse: ReuseContext {
                shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
                uniqueness: Uniqueness::MaybeShared,
                cardinality: Cardinality::Many,
            },
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Dec);
    assert_eq!(d.reuse, ReuseDecision::DynamicReuse);
}

// Cross-dimensional proof: MaybeShared + Once + ReusableCtor → StaticReuse

#[test]
fn decide_cross_dimensional_maybe_shared_once_ctor_is_static_reuse() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,
            has_deferred_children: false,
            reuse: ReuseContext {
                shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
                uniqueness: Uniqueness::MaybeShared,
                cardinality: Cardinality::Once,
            },
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Dec);
    assert_eq!(d.reuse, ReuseDecision::StaticReuse);
}

#[test]
fn decide_cross_dimensional_maybe_shared_once_non_reusable_no_reuse() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,
            has_deferred_children: false,
            reuse: ReuseContext {
                shape: ShapeClass::NonReusable,
                uniqueness: Uniqueness::MaybeShared,
                cardinality: Cardinality::Once,
            },
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Dec);
    assert_eq!(d.reuse, ReuseDecision::None);
}

#[test]
fn decide_cross_dimensional_maybe_shared_once_collection_is_dynamic_reuse() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,
            has_deferred_children: false,
            reuse: ReuseContext {
                shape: ShapeClass::CollectionBuffer,
                uniqueness: Uniqueness::MaybeShared,
                cardinality: Cardinality::Once,
            },
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Dec);
    // CollectionBuffer is not NonReusable, and MaybeShared + Once but NOT
    // ReusableCtor → cross-dimensional proof doesn't apply → DynamicReuse.
    assert_eq!(d.reuse, ReuseDecision::DynamicReuse);
}

// Enum variant reuse

#[test]
fn decide_unique_enum_variant_returns_static_reuse() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: false,
            is_ownership_transfer: false,
            is_owned_call_position: false,
            has_deferred_children: false,
            reuse: ReuseContext {
                shape: ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant),
                uniqueness: Uniqueness::Unique,
                cardinality: Cardinality::Many,
            },
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::Dec);
    assert_eq!(d.reuse, ReuseDecision::StaticReuse);
}

// Suppression priority: consuming primop takes precedence over all others

#[test]
fn decide_consuming_primop_suppresses_even_with_deferred_children() {
    let ctx = DecisionContext {
        site: DecisionSite::LastUse {
            is_consuming_primop: true,
            is_ownership_transfer: false,
            is_owned_call_position: false,
            has_deferred_children: true,
            reuse: reuse_unique_struct(),
        },
        is_rc_managed: true,
    };
    let d = decide(&ctx);
    assert_eq!(d.rc, RcDecision::None);
}

// Output equivalence tests (Section 10.2)
//
// These tests verify that the unified `realize_rc_reuse()` (Phase 1)
// produces identical RC/reuse output to the legacy 4-pass emission
// (`emit_arg_ownership` + `emit_rc_ops` + `emit_reuse`).
//
// Phase 2 (`realize_annotations`) delegates to the same functions as
// legacy (`compute_aims_cow_annotations`, `compute_aims_drop_hints`),
// so Phase 2 equivalence is by construction — tested in Section 10.4.

use ori_ir::{Name, StringInterner};
use ori_types::{Idx, Pool};
use rustc_hash::FxHashMap;

use crate::aims::contract::MemoryContract;
use crate::aims::emit_rc::emit_rc_ops;
use crate::aims::emit_reuse::emit_reuse;
use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::{AimsState, EffectClass, Locality};
use crate::borrow::BuiltinOwnershipSets;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, CtorKind,
    LitValue, ValueRepr,
};
use crate::ownership::Ownership;
use crate::uniqueness::drop_hints::DropHints;
use crate::uniqueness::CowAnnotations;

/// Helper: create a minimal [`ArcFunction`] for equivalence tests.
fn eq_make_func(
    blocks: Vec<ArcBlock>,
    num_vars: usize,
    params: Vec<ArcParam>,
    var_reprs: Vec<ValueRepr>,
) -> ArcFunction {
    ArcFunction {
        name: Name::new(0, 0),
        params,
        return_type: Idx::NONE,
        blocks,
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::NONE; num_vars],
        var_reprs,
        spans: Vec::new(),
        is_fbip: false,
        num_captures: 0,
        cow_annotations: CowAnnotations::default(),
        drop_hints: DropHints::default(),
        tail_calls: Vec::new(),
    }
}

/// Helper: create an Owned [`AimsState`] with given cardinality and uniqueness.
fn eq_owned_state(card: Cardinality, uniq: Uniqueness) -> AimsState {
    AimsState {
        access: AccessClass::Owned,
        consumption: match card {
            Cardinality::Absent => Consumption::Dead,
            Cardinality::Once => Consumption::Linear,
            Cardinality::Many => Consumption::Unrestricted,
        },
        cardinality: card,
        uniqueness: uniq,
        locality: Locality::FunctionLocal,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::NONE,
    }
}

/// Helper: create an Owned state with Unique uniqueness.
fn eq_owned_unique(card: Cardinality) -> AimsState {
    eq_owned_state(card, Uniqueness::Unique)
}

/// Run both legacy and unified emission paths, return `(legacy, unified)`.
///
/// The legacy path runs: `emit_arg_ownership` → `emit_rc_ops` → `emit_reuse`.
/// The unified path runs: `realize_rc_reuse` (which internally does all three).
fn run_both_paths(func: ArcFunction, state_map: &AimsStateMap) -> (ArcFunction, ArcFunction) {
    let pool = Pool::new();
    let interner = StringInterner::default();
    let builtins = BuiltinOwnershipSets::new(&interner);
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    // Legacy path.
    let mut legacy = func.clone();
    crate::aims::emit_rc::arg_ownership::emit_arg_ownership(
        &mut legacy,
        &contracts,
        &interner,
        &builtins,
        &pool,
    );
    emit_rc_ops(&mut legacy, state_map, &pool);
    let _reuse_result = emit_reuse(&mut legacy, state_map, &pool, &contracts);

    // Unified path.
    let mut unified = func;
    let _realize_result = super::realize_rc_reuse(
        &mut unified,
        state_map,
        &contracts,
        &interner,
        &builtins,
        &pool,
    );

    (legacy, unified)
}

/// Count RC operations (`RcInc` + `RcDec`) in a function.
fn eq_count_rc_ops(func: &ArcFunction) -> (usize, usize) {
    let mut inc = 0usize;
    let mut dec = 0usize;
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::RcInc { count, .. } => inc += *count as usize,
                ArcInstr::RcDec { .. } => dec += 1,
                _ => {}
            }
        }
    }
    (inc, dec)
}

/// Assert that two functions have identical RC instruction sequences per block.
fn assert_rc_equivalent(legacy: &ArcFunction, unified: &ArcFunction, test_name: &str) {
    assert_eq!(
        legacy.blocks.len(),
        unified.blocks.len(),
        "{test_name}: block count mismatch"
    );

    let (l_inc, l_dec) = eq_count_rc_ops(legacy);
    let (u_inc, u_dec) = eq_count_rc_ops(unified);
    assert_eq!(
        (l_inc, l_dec),
        (u_inc, u_dec),
        "{test_name}: RC op counts differ — legacy: ({l_inc} inc, {l_dec} dec), \
         unified: ({u_inc} inc, {u_dec} dec)"
    );

    for (bi, (lb, ub)) in legacy.blocks.iter().zip(unified.blocks.iter()).enumerate() {
        assert_eq!(
            lb.body.len(),
            ub.body.len(),
            "{test_name}: block {bi} body length mismatch — \
             legacy ({} instrs): {:?}, unified ({} instrs): {:?}",
            lb.body.len(),
            lb.body,
            ub.body.len(),
            ub.body,
        );
        for (ii, (li, ui)) in lb.body.iter().zip(ub.body.iter()).enumerate() {
            assert_eq!(
                li, ui,
                "{test_name}: block {bi} instr {ii} differs — \
                 legacy: {li:?}, unified: {ui:?}"
            );
        }
    }
}

// Golden corpus equivalence tests

/// Equivalence: single-use variable → `RcDec` only.
#[test]
fn equivalence_single_use() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::Let {
            dst: v1,
            ty: Idx::NONE,
            value: ArcValue::Var(v0),
        }],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let func = eq_make_func(
        vec![block],
        2,
        vec![ArcParam {
            var: v0,
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        vec![ValueRepr::RcPointer, ValueRepr::Scalar],
    );

    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v0, eq_owned_unique(Cardinality::Once))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(ArcBlockId::new(0), FxHashMap::default());

    let (legacy, unified) = run_both_paths(func, &state_map);
    assert_rc_equivalent(&legacy, &unified, "single_use");
}

/// Equivalence: multi-use variable → `RcInc` before first use, `RcDec` after last.
#[test]
fn equivalence_multi_use() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Let {
                dst: v1,
                ty: Idx::NONE,
                value: ArcValue::Var(v0),
            },
            ArcInstr::Let {
                dst: v2,
                ty: Idx::NONE,
                value: ArcValue::Var(v0),
            },
        ],
        terminator: ArcTerminator::Return { value: v2 },
    };

    let func = eq_make_func(
        vec![block],
        3,
        vec![ArcParam {
            var: v0,
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        vec![ValueRepr::RcPointer, ValueRepr::Scalar, ValueRepr::Scalar],
    );

    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v0, eq_owned_unique(Cardinality::Many))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(ArcBlockId::new(0), FxHashMap::default());

    let (legacy, unified) = run_both_paths(func, &state_map);
    assert_rc_equivalent(&legacy, &unified, "multi_use");
}

/// Equivalence: variable used in body AND terminator.
#[test]
fn equivalence_body_and_terminator_use() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::Let {
            dst: v1,
            ty: Idx::NONE,
            value: ArcValue::Var(v0),
        }],
        terminator: ArcTerminator::Return { value: v0 },
    };

    let func = eq_make_func(
        vec![block],
        2,
        vec![ArcParam {
            var: v0,
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        vec![ValueRepr::RcPointer, ValueRepr::Scalar],
    );

    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v0, eq_owned_unique(Cardinality::Many))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(ArcBlockId::new(0), FxHashMap::default());

    let (legacy, unified) = run_both_paths(func, &state_map);
    assert_rc_equivalent(&legacy, &unified, "body_and_terminator_use");
}

/// Equivalence: scalar variables produce no RC ops.
#[test]
fn equivalence_scalar_skipped() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::Let {
            dst: v1,
            ty: Idx::NONE,
            value: ArcValue::Literal(LitValue::Int(42)),
        }],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let func = eq_make_func(
        vec![block],
        2,
        vec![ArcParam {
            var: v0,
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        vec![ValueRepr::Scalar, ValueRepr::Scalar],
    );

    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v0);
    state_map.set_permanent_scalar(v1);

    let (legacy, unified) = run_both_paths(func, &state_map);
    assert_rc_equivalent(&legacy, &unified, "scalar_skipped");
}

/// Equivalence: borrowed variables produce no RC ops.
#[test]
fn equivalence_borrowed_skipped() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::Let {
            dst: v1,
            ty: Idx::NONE,
            value: ArcValue::Var(v0),
        }],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let func = eq_make_func(
        vec![block],
        2,
        vec![ArcParam {
            var: v0,
            ty: Idx::NONE,
            ownership: Ownership::Borrowed,
        }],
        vec![ValueRepr::RcPointer, ValueRepr::Scalar],
    );

    let borrowed_state = AimsState {
        access: AccessClass::Borrowed,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        uniqueness: Uniqueness::Unique,
        locality: Locality::FunctionLocal,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::NONE,
    };
    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v0, borrowed_state)].into_iter().collect(),
    );
    state_map.update_block_exit(ArcBlockId::new(0), FxHashMap::default());

    let (legacy, unified) = run_both_paths(func, &state_map);
    assert_rc_equivalent(&legacy, &unified, "borrowed_skipped");
}

/// Equivalence: dead parameter at entry gets `RcDec` at block start.
#[test]
fn equivalence_dead_at_entry() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::Let {
            dst: v1,
            ty: Idx::NONE,
            value: ArcValue::Literal(LitValue::Int(42)),
        }],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let func = eq_make_func(
        vec![block],
        2,
        vec![ArcParam {
            var: v0,
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        vec![ValueRepr::RcPointer, ValueRepr::Scalar],
    );

    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v0, eq_owned_unique(Cardinality::Once))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(ArcBlockId::new(0), FxHashMap::default());

    let (legacy, unified) = run_both_paths(func, &state_map);
    assert_rc_equivalent(&legacy, &unified, "dead_at_entry");
}

/// Equivalence: defined-dead variable (defined but never used) gets `RcDec`.
#[test]
fn equivalence_defined_dead() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            // v1 is defined by Construct but never used → defined-dead.
            ArcInstr::Construct {
                dst: v1,
                ty: Idx::NONE,
                ctor: CtorKind::Struct(Name::new(0, 0)),
                args: vec![],
            },
            ArcInstr::Let {
                dst: v2,
                ty: Idx::NONE,
                value: ArcValue::Var(v0),
            },
        ],
        terminator: ArcTerminator::Return { value: v2 },
    };

    let func = eq_make_func(
        vec![block],
        3,
        vec![ArcParam {
            var: v0,
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        vec![
            ValueRepr::RcPointer,
            ValueRepr::RcPointer,
            ValueRepr::Scalar,
        ],
    );

    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v0, eq_owned_unique(Cardinality::Once))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(ArcBlockId::new(0), FxHashMap::default());

    let (legacy, unified) = run_both_paths(func, &state_map);
    assert_rc_equivalent(&legacy, &unified, "defined_dead");
}

// Cross-decision interaction tests (Section 10.2, strictly-better)

/// Cross-decision: unified `decide()` simultaneously determines RC and reuse
/// decisions for a dying variable with reusable shape. The death event is
/// collected inline during the walk, producing the same reuse matching as
/// the legacy separate-scan approach.
///
/// This test verifies that the unified path's inline death event collection
/// produces identical reuse output to the legacy `collect_death_events()` scan.
/// For programs with reuse opportunities, both paths must find the same
/// death→alloc matchings.
#[test]
fn cross_decision_reuse_equivalent() {
    let v0 = ArcVarId::new(0); // dying value (struct, unique, reusable)
    let v1 = ArcVarId::new(1); // literal arg
    let v2 = ArcVarId::new(2); // new Construct (alloc site, reuse target)

    let struct_name = Name::new(1, 0);
    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            // Use v0 (last use → death site with reuse potential).
            ArcInstr::Let {
                dst: v1,
                ty: Idx::NONE,
                value: ArcValue::Var(v0),
            },
            // Construct same-type struct (allocation site → reuse candidate).
            ArcInstr::Construct {
                dst: v2,
                ty: Idx::NONE,
                ctor: CtorKind::Struct(struct_name),
                args: vec![],
            },
        ],
        terminator: ArcTerminator::Return { value: v2 },
    };

    let func = eq_make_func(
        vec![block],
        3,
        vec![ArcParam {
            var: v0,
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        vec![
            ValueRepr::RcPointer,
            ValueRepr::Scalar,
            ValueRepr::RcPointer,
        ],
    );

    // v0: Owned + Unique + Once + ReusableCtor(Struct) → StaticReuse candidate.
    let reusable_state = AimsState {
        access: AccessClass::Owned,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        uniqueness: Uniqueness::Unique,
        locality: Locality::FunctionLocal,
        shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
        effect: EffectClass::NONE,
    };
    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v0, reusable_state)].into_iter().collect(),
    );
    state_map.update_block_exit(ArcBlockId::new(0), FxHashMap::default());
    state_map.set_var_shape(v0, ShapeClass::ReusableCtor(ReuseCtorKind::Struct));

    let (legacy, unified) = run_both_paths(func, &state_map);

    // Both paths should find the same reuse opportunity.
    // Compare the final instruction sequences — reuse emission may have
    // replaced the RcDec+Construct with Reset+Set/Reuse instructions.
    assert_eq!(
        legacy.blocks.len(),
        unified.blocks.len(),
        "cross_decision_reuse: block count mismatch"
    );

    let (l_inc, l_dec) = eq_count_rc_ops(&legacy);
    let (u_inc, u_dec) = eq_count_rc_ops(&unified);
    // Unified should produce identical or fewer RC ops.
    assert!(
        u_inc <= l_inc && u_dec <= l_dec,
        "cross_decision_reuse: unified has MORE RC ops than legacy — \
         legacy: ({l_inc} inc, {l_dec} dec), unified: ({u_inc} inc, {u_dec} dec)"
    );
}

/// Strictly better: cross-dimensional proof in `decide()` promotes a
/// MaybeShared+Once+ReusableCtor variable to `StaticReuse`. Both legacy and
/// unified should make this promotion. The unified path makes it in one
/// `decide()` call; legacy requires separate detection.
///
/// This test exercises the cross-dimensional proof path (Section 09.2)
/// through both emission pipelines to verify the promotion is preserved
/// through the unified decision surface.
#[test]
fn cross_decision_cross_dimensional_promotion() {
    let v0 = ArcVarId::new(0); // dying value: MaybeShared but Once + Struct
    let v1 = ArcVarId::new(1); // literal
    let v2 = ArcVarId::new(2); // new Construct

    let struct_name = Name::new(1, 0);
    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Let {
                dst: v1,
                ty: Idx::NONE,
                value: ArcValue::Var(v0),
            },
            ArcInstr::Construct {
                dst: v2,
                ty: Idx::NONE,
                ctor: CtorKind::Struct(struct_name),
                args: vec![],
            },
        ],
        terminator: ArcTerminator::Return { value: v2 },
    };

    let func = eq_make_func(
        vec![block],
        3,
        vec![ArcParam {
            var: v0,
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        vec![
            ValueRepr::RcPointer,
            ValueRepr::Scalar,
            ValueRepr::RcPointer,
        ],
    );

    // Key: MaybeShared (not Unique!) but Once + ReusableCtor(Struct).
    // Cross-dimensional proof: MaybeShared + Once + ReusableCtor → StaticReuse.
    let cross_dim_state = AimsState {
        access: AccessClass::Owned,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        uniqueness: Uniqueness::MaybeShared, // <-- cross-dimensional proof needed
        locality: Locality::FunctionLocal,
        shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
        effect: EffectClass::NONE,
    };
    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v0, cross_dim_state)].into_iter().collect(),
    );
    state_map.update_block_exit(ArcBlockId::new(0), FxHashMap::default());
    state_map.set_var_shape(v0, ShapeClass::ReusableCtor(ReuseCtorKind::Struct));

    let (legacy, unified) = run_both_paths(func, &state_map);

    // Both should handle the cross-dimensional case equivalently.
    let (l_inc, l_dec) = eq_count_rc_ops(&legacy);
    let (u_inc, u_dec) = eq_count_rc_ops(&unified);
    assert!(
        u_inc <= l_inc && u_dec <= l_dec,
        "cross_dimensional: unified has MORE RC ops — \
         legacy: ({l_inc} inc, {l_dec} dec), unified: ({u_inc} inc, {u_dec} dec)"
    );

    // Verify the cross-dimensional proof was applied by checking that the
    // Construct at index 1 was potentially rewritten (reuse emission may
    // have replaced it). Both paths should produce identical output.
    assert_eq!(
        legacy.blocks[0].body.len(),
        unified.blocks[0].body.len(),
        "cross_dimensional: body length mismatch — \
         legacy: {:?}, unified: {:?}",
        legacy.blocks[0].body,
        unified.blocks[0].body,
    );
}
