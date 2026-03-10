//! Tests for memory contract types.

use super::*;
use crate::ir::{ArcParam, ArcVarId};
use crate::ownership::Ownership;
use ori_types::Idx;

fn var(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

fn ty(n: u32) -> Idx {
    Idx::from_raw(n)
}

fn arc_param(var_id: u32, ty_id: u32) -> ArcParam {
    ArcParam {
        var: var(var_id),
        ty: ty(ty_id),
        ownership: Ownership::Owned,
    }
}

// MemoryContract construction

#[test]
fn conservative_all_params_owned() {
    let c = MemoryContract::conservative(3);
    assert_eq!(c.params.len(), 3);
    for p in &c.params {
        assert_eq!(p.access, AccessClass::Owned);
        assert_eq!(p.consumption, Consumption::Unrestricted);
        assert_eq!(p.cardinality, Cardinality::Many);
        assert!(p.may_escape);
        assert!(p.may_share);
        assert_eq!(p.locality_bound, Locality::Unknown);
    }
    assert_eq!(c.return_info.uniqueness, Uniqueness::MaybeShared);
    assert!(!c.return_info.preserves_freshness);
    assert_eq!(c.effects, EffectSummary::CONSERVATIVE);
    assert_eq!(c.fip, FipContract::Never);
}

#[test]
fn all_borrowed_optimistic() {
    let c = MemoryContract::all_borrowed(2, FipContract::Never);
    assert_eq!(c.params.len(), 2);
    for p in &c.params {
        assert_eq!(p.access, AccessClass::Borrowed);
        assert_eq!(p.consumption, Consumption::Dead);
        assert_eq!(p.cardinality, Cardinality::Absent);
        assert!(!p.may_escape);
        assert!(!p.may_share);
        assert_eq!(p.locality_bound, Locality::BlockLocal);
    }
    assert_eq!(c.return_info.uniqueness, Uniqueness::Unique);
    assert!(c.return_info.preserves_freshness);
    assert_eq!(c.effects, EffectSummary::OPTIMISTIC);
}

#[test]
fn all_borrowed_with_certified_fip() {
    let c = MemoryContract::all_borrowed(1, FipContract::Certified);
    assert_eq!(c.fip, FipContract::Certified);
}

#[test]
fn all_borrowed_zero_params() {
    let c = MemoryContract::all_borrowed(0, FipContract::Never);
    assert!(c.params.is_empty());
}

// ParamContract join

#[test]
fn param_join_optimistic_with_conservative() {
    let result = ParamContract::OPTIMISTIC.join(&ParamContract::CONSERVATIVE);
    assert_eq!(result, ParamContract::CONSERVATIVE);
}

#[test]
fn param_join_is_idempotent() {
    let c = ParamContract::CONSERVATIVE;
    assert_eq!(c.join(&c), c);
    let o = ParamContract::OPTIMISTIC;
    assert_eq!(o.join(&o), o);
}

#[test]
fn param_join_is_commutative() {
    let a = ParamContract {
        access: AccessClass::Owned,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        may_escape: true,
        may_share: false,
        locality_bound: Locality::FunctionLocal,
    };
    let b = ParamContract {
        access: AccessClass::Borrowed,
        consumption: Consumption::Affine,
        cardinality: Cardinality::Many,
        may_escape: false,
        may_share: true,
        locality_bound: Locality::HeapEscaping,
    };
    assert_eq!(a.join(&b), b.join(&a));
}

// ReturnContract join

#[test]
fn return_join_preserves_freshness_and() {
    let a = ReturnContract {
        preserves_freshness: true,
        ..ReturnContract::OPTIMISTIC
    };
    let b = ReturnContract {
        preserves_freshness: false,
        ..ReturnContract::OPTIMISTIC
    };
    assert!(!a.join(&b).preserves_freshness);
}

#[test]
fn return_join_uniqueness_weakens() {
    let unique = ReturnContract::OPTIMISTIC;
    let shared = ReturnContract::CONSERVATIVE;
    let joined = unique.join(&shared);
    assert_eq!(joined.uniqueness, Uniqueness::MaybeShared);
}

// EffectSummary join

#[test]
fn effect_join_or_semantics() {
    let a = EffectSummary {
        may_allocate: true,
        alloc_only_on_slow_path: true,
        may_share: false,
        may_throw: false,
    };
    let b = EffectSummary {
        may_allocate: false,
        alloc_only_on_slow_path: false,
        may_share: true,
        may_throw: false,
    };
    let joined = a.join(&b);
    assert!(joined.may_allocate);
    assert!(!joined.alloc_only_on_slow_path); // AND semantics
    assert!(joined.may_share);
    assert!(!joined.may_throw);
}

// FipContract join

#[test]
fn fip_never_absorbs() {
    assert_eq!(
        FipContract::Never.join(&FipContract::Certified),
        FipContract::Never
    );
    assert_eq!(
        FipContract::Certified.join(&FipContract::Never),
        FipContract::Never
    );
}

#[test]
fn fip_certified_identity() {
    assert_eq!(
        FipContract::Certified.join(&FipContract::Certified),
        FipContract::Certified
    );
}

#[test]
fn fip_conditional_union() {
    let a = FipContract::Conditional {
        requires_unique_params: vec![true, false, false],
    };
    let b = FipContract::Conditional {
        requires_unique_params: vec![false, true, false],
    };
    let joined = a.join(&b);
    assert_eq!(
        joined,
        FipContract::Conditional {
            requires_unique_params: vec![true, true, false],
        }
    );
}

#[test]
fn fip_conditional_with_certified() {
    let cond = FipContract::Conditional {
        requires_unique_params: vec![true, false],
    };
    assert_eq!(cond.join(&FipContract::Certified), cond);
    let cond2 = FipContract::Conditional {
        requires_unique_params: vec![true, false],
    };
    assert_eq!(FipContract::Certified.join(&cond2), cond2);
}

// MemoryContract join

#[test]
fn contract_join_convergence() {
    let optimistic = MemoryContract::all_borrowed(2, FipContract::Never);
    let conservative = MemoryContract::conservative(2);
    let joined = optimistic.join(&conservative);
    assert_eq!(joined, conservative);
}

#[test]
fn contract_join_idempotent() {
    let c = MemoryContract::conservative(2);
    assert_eq!(c.join(&c), c);
}

// Conversion: MemoryContract → AnnotatedSig

#[test]
fn to_annotated_sig_owned_params() {
    let contract = MemoryContract::conservative(2);
    let func_params = vec![arc_param(0, 1), arc_param(1, 2)];
    let sig = contract.to_annotated_sig(&func_params, Idx::INT);

    assert_eq!(sig.params.len(), 2);
    assert_eq!(sig.params[0].ownership, Ownership::Owned);
    assert_eq!(sig.params[1].ownership, Ownership::Owned);
    assert_eq!(sig.return_type, Idx::INT);
}

#[test]
fn to_annotated_sig_borrowed_params() {
    let contract = MemoryContract::all_borrowed(1, FipContract::Never);
    let func_params = vec![arc_param(0, 1)];
    let sig = contract.to_annotated_sig(&func_params, Idx::STR);

    // Dead consumption → Borrowed regardless of access class.
    assert_eq!(sig.params[0].ownership, Ownership::Borrowed);
}

#[test]
fn to_annotated_sig_dead_param_is_borrowed() {
    let contract = MemoryContract {
        params: vec![ParamContract {
            access: AccessClass::Owned,
            consumption: Consumption::Dead,
            cardinality: Cardinality::Absent,
            may_escape: false,
            may_share: false,
            locality_bound: Locality::Unknown,
        }],
        return_info: ReturnContract::CONSERVATIVE,
        effects: EffectSummary::default(),
        context_behavior: ContextBehavior::default(),
        fip: FipContract::Never,
    };
    let func_params = vec![arc_param(0, 1)];
    let sig = contract.to_annotated_sig(&func_params, Idx::UNIT);

    // Dead params are treated as Borrowed even if access says Owned.
    assert_eq!(sig.params[0].ownership, Ownership::Borrowed);
}

// Conversion: MemoryContract → UniquenessSummary

#[test]
fn to_uniqueness_summary_basic() {
    let contract = MemoryContract {
        params: vec![ParamContract::CONSERVATIVE; 2],
        return_info: ReturnContract {
            uniqueness: Uniqueness::Unique,
            preserves_freshness: true,
            locality: Locality::FunctionLocal,
            shape: ShapeClass::NonReusable,
        },
        effects: EffectSummary::default(),
        context_behavior: ContextBehavior::default(),
        fip: FipContract::Never,
    };
    let summary = contract.to_uniqueness_summary();

    assert_eq!(summary.params.len(), 2);
    // Per-param uniqueness is always MaybeShared in the legacy system.
    for p in &summary.params {
        assert_eq!(*p, crate::uniqueness::Uniqueness::MaybeShared);
    }
    assert_eq!(summary.return_val, crate::uniqueness::Uniqueness::Unique);
    assert!(summary.preserves_freshness);
}

#[test]
fn to_uniqueness_summary_shared_return() {
    let contract = MemoryContract::conservative(0);
    let summary = contract.to_uniqueness_summary();
    assert_eq!(
        summary.return_val,
        crate::uniqueness::Uniqueness::MaybeShared
    );
    assert!(!summary.preserves_freshness);
}

// ContextBehavior join

#[test]
fn context_behavior_join_and() {
    let a = ContextBehavior {
        preserves_context: true,
        consumes_hole: false,
    };
    let b = ContextBehavior {
        preserves_context: false,
        consumes_hole: true,
    };
    let joined = a.join(&b);
    assert!(!joined.preserves_context);
    assert!(!joined.consumes_hole);
}
