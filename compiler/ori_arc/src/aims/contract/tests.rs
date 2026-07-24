//! Tests for memory contract types.

use super::*;
use crate::aims::lattice::Cardinality;
use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId};
use crate::ownership::Ownership;
use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::FxHashMap;

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

fn read_only_function() -> ArcFunction {
    ArcFunction {
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: Vec::new(),
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..ArcFunction::default()
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
        uniqueness: Uniqueness::Unique,
        transfers_through_return: false,
        return_alias: None,
        return_payload_contains_param: false,
        iter_consumes: false,
        borrowed_read_only: false,
        borrowed_cow_consumed: false,
        borrowed_cow_mutated: false,
        exact_transfer: crate::aims::contract::ExactTransferState::Unproven,
    };
    let b = ParamContract {
        access: AccessClass::Borrowed,
        consumption: Consumption::Affine,
        cardinality: Cardinality::Many,
        may_escape: false,
        may_share: true,
        locality_bound: Locality::HeapEscaping,
        uniqueness: Uniqueness::MaybeShared,
        transfers_through_return: false,
        return_alias: None,
        return_payload_contains_param: false,
        iter_consumes: false,
        borrowed_read_only: false,
        borrowed_cow_consumed: false,
        borrowed_cow_mutated: false,
        exact_transfer: crate::aims::contract::ExactTransferState::Unproven,
    };
    assert_eq!(a.join(&b), b.join(&a));
}

fn exact_transfer(fields: &[(u32, ExactFieldTransferKind)]) -> ExactTransferState {
    let fields = fields
        .iter()
        .map(|&(field, kind)| ExactFieldTransfer {
            path: ExactFieldPath::single(field),
            kind,
        })
        .collect();
    let proof = ExactAggregateTransfer::new(
        fields,
        ResidualDisposition::FullyReconstructed,
        CleanupAuthority::OrdinaryCleanupProven,
    );
    let Some(proof) = proof else {
        panic!("fixture fields must be unique");
    };
    ExactTransferState::exact(proof)
}

#[test]
fn exact_transfer_constructor_canonicalizes_field_order() {
    let left = exact_transfer(&[
        (2, ExactFieldTransferKind::EffectiveOwnedRelay),
        (0, ExactFieldTransferKind::DirectMove),
    ]);
    let right = exact_transfer(&[
        (0, ExactFieldTransferKind::DirectMove),
        (2, ExactFieldTransferKind::EffectiveOwnedRelay),
    ]);

    assert_eq!(left, right);
}

#[test]
fn exact_transfer_constructor_rejects_duplicate_field_credit() {
    let duplicate = vec![
        ExactFieldTransfer {
            path: ExactFieldPath::single(1),
            kind: ExactFieldTransferKind::DirectMove,
        },
        ExactFieldTransfer {
            path: ExactFieldPath::single(1),
            kind: ExactFieldTransferKind::EffectiveOwnedRelay,
        },
    ];

    assert!(ExactAggregateTransfer::new(
        duplicate,
        ResidualDisposition::FullyReconstructed,
        CleanupAuthority::OrdinaryCleanupProven,
    )
    .is_none());
}

#[test]
fn exact_transfer_join_is_flat_associative_and_idempotent() {
    let a = exact_transfer(&[(0, ExactFieldTransferKind::DirectMove)]);
    let same_a = exact_transfer(&[(0, ExactFieldTransferKind::DirectMove)]);
    let b = exact_transfer(&[(1, ExactFieldTransferKind::EffectiveOwnedRelay)]);

    assert_eq!(a.join(&same_a), a);
    assert_eq!(a.join(&b), ExactTransferState::Unproven);
    assert_eq!(
        a.join(&same_a).join(&b),
        a.join(&same_a.join(&b)),
        "a = same_a != b must reach Unproven under either grouping"
    );
    assert_eq!(
        ExactTransferState::Optimistic.join(&a),
        a.join(&ExactTransferState::Optimistic)
    );
}

#[test]
fn param_join_carries_exact_transfer_lattice() {
    let exact = exact_transfer(&[(0, ExactFieldTransferKind::DirectMove)]);
    let mut left = ParamContract::OPTIMISTIC;
    left.exact_transfer = exact.clone();
    let mut right = ParamContract::OPTIMISTIC;
    right.exact_transfer = exact;

    assert!(matches!(
        left.join(&right).exact_transfer,
        ExactTransferState::Exact(_)
    ));
    assert_eq!(
        left.join(&ParamContract::CONSERVATIVE).exact_transfer,
        ExactTransferState::Unproven
    );
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
    assert!(!a.join(b).preserves_freshness);
}

#[test]
fn return_join_uniqueness_weakens() {
    let unique = ReturnContract::OPTIMISTIC;
    let shared = ReturnContract::CONSERVATIVE;
    let joined = unique.join(shared);
    assert_eq!(joined.uniqueness, Uniqueness::MaybeShared);
}

#[test]
fn fresh_self_allocation_facts_require_the_stronger_return_proof() {
    let mut fresh = MemoryContract::conservative(0);
    fresh.return_info.preserves_freshness = true;
    fresh.return_info.returns_fresh_self_alloc = true;
    assert!(fresh.fresh_self_allocation_facts().is_proven());

    let mut forwarded_or_consumed = fresh;
    forwarded_or_consumed.return_info.returns_fresh_self_alloc = false;
    assert!(forwarded_or_consumed.return_info.preserves_freshness);
    assert!(
        !forwarded_or_consumed
            .fresh_self_allocation_facts()
            .is_proven(),
        "freshness preservation cannot certify caller-owned or consumed storage"
    );
}

// EffectSummary join

#[test]
fn effect_join_or_semantics() {
    let a = EffectSummary {
        may_allocate: true,
        alloc_only_on_slow_path: true,
        may_deallocate: false,
        may_share: false,
        may_throw: false,
        has_unbounded_stack: false,
    };
    let b = EffectSummary {
        may_allocate: false,
        alloc_only_on_slow_path: false,
        may_deallocate: true,
        may_share: true,
        may_throw: false,
        has_unbounded_stack: false,
    };
    let joined = a.join(b);
    assert!(joined.may_allocate);
    assert!(!joined.alloc_only_on_slow_path);
    assert!(joined.may_deallocate);
    assert!(joined.may_share);
    assert!(!joined.may_throw);
    assert!(!joined.has_unbounded_stack);
}

#[test]
fn effect_join_unbounded_stack_or_semantics() {
    let bounded = EffectSummary {
        has_unbounded_stack: false,
        ..EffectSummary::OPTIMISTIC
    };
    let unbounded = EffectSummary {
        has_unbounded_stack: true,
        ..EffectSummary::OPTIMISTIC
    };
    assert!(bounded.join(unbounded).has_unbounded_stack);
    assert!(unbounded.join(bounded).has_unbounded_stack);
    assert!(!bounded.join(bounded).has_unbounded_stack);
    assert!(unbounded.join(unbounded).has_unbounded_stack);
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
            uniqueness: Uniqueness::MaybeShared,
            transfers_through_return: false,
            return_alias: None,
            return_payload_contains_param: false,
            iter_consumes: false,
            borrowed_read_only: false,
            borrowed_cow_consumed: false,
            borrowed_cow_mutated: false,
            exact_transfer: crate::aims::contract::ExactTransferState::Unproven,
        }],
        return_info: ReturnContract::CONSERVATIVE,
        effects: EffectSummary::default(),
        context_behavior: ContextBehavior::default(),
        fip: FipContract::Never,
        is_fbip: false,
    };
    let func_params = vec![arc_param(0, 1)];
    let sig = contract.to_annotated_sig(&func_params, Idx::UNIT);

    assert_eq!(sig.params[0].ownership, Ownership::Borrowed);
}

// ContextBehavior

#[test]
fn context_behavior_default_is_conservative() {
    let cb = ContextBehavior::default();
    assert!(!cb.preserves_context);
    assert!(!cb.consumes_hole);
    assert!(cb.requires_unique_context);
    assert!(!cb.may_resume_nonlinearly);
}

#[test]
fn context_behavior_join_is_conservative() {
    let a = ContextBehavior {
        preserves_context: true,
        consumes_hole: true,
        requires_unique_context: false,
        may_resume_nonlinearly: false,
    };
    let b = ContextBehavior {
        preserves_context: false,
        consumes_hole: true,
        requires_unique_context: true,
        may_resume_nonlinearly: true,
    };
    let joined = a.join(&b);
    assert!(!joined.preserves_context);
    assert!(joined.consumes_hole);
    assert!(joined.requires_unique_context);
    assert!(joined.may_resume_nonlinearly);
}

#[test]
fn context_behavior_join_is_commutative() {
    let a = ContextBehavior {
        preserves_context: true,
        consumes_hole: false,
        requires_unique_context: false,
        may_resume_nonlinearly: true,
    };
    let b = ContextBehavior {
        preserves_context: false,
        consumes_hole: true,
        requires_unique_context: true,
        may_resume_nonlinearly: false,
    };
    assert_eq!(a.join(&b), b.join(&a));
}

#[test]
fn context_behavior_conservative_constructor_safe() {
    let c = MemoryContract::conservative(1);
    assert!(c.context_behavior.requires_unique_context);
    assert!(!c.context_behavior.preserves_context);
    assert!(!c.context_behavior.consumes_hole);
    assert!(!c.context_behavior.may_resume_nonlinearly);
}

// ContractMapExt — AIMS IC-1 coverage enforcement.

#[test]
fn test_get_required_returns_contract_when_present() {
    let mut map: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let name = Name::from_raw(1);
    let contract = MemoryContract::conservative(0);
    map.insert(name, contract.clone());

    let got = map.get_required(&name, "test_site");
    assert_eq!(got, &contract);
}

#[test]
#[should_panic(expected = "test_site_tag_marker")]
fn test_get_required_panics_on_missing_with_site_tag() {
    let map: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let _ = map.get_required(&Name::from_raw(2), "test_site_tag_marker");
}

#[test]
#[should_panic(expected = "AIMS Invariant IC-1")]
fn test_get_required_panics_message_includes_invariant_label() {
    let map: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let _ = map.get_required(&Name::from_raw(3), "any_site");
}

#[test]
fn test_get_mut_required_returns_mut_ref_when_present() {
    let mut map: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let name = Name::from_raw(4);
    map.insert(name, MemoryContract::conservative(0));

    let got = map.get_mut_required(&name, "test_mut_site");
    got.effects.may_deallocate = true;
    assert!(map.get(&name).is_some_and(|c| c.effects.may_deallocate));
}

#[test]
#[should_panic(expected = "mut_test_site_tag_marker")]
fn test_get_mut_required_panics_on_missing_with_site_tag() {
    let mut map: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let _ = map.get_mut_required(&Name::from_raw(5), "mut_test_site_tag_marker");
}

#[test]
#[should_panic(expected = "AIMS Invariant IC-1")]
fn test_get_mut_required_panics_message_includes_invariant_label() {
    let mut map: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let _ = map.get_mut_required(&Name::from_raw(6), "any_mut_site");
}

// Sharing-view joins (Spec: Annex E §AIMS §12).

#[test]
fn return_contract_join_ands_returns_sharing_view() {
    let view = ReturnContract {
        returns_sharing_view: true,
        ..ReturnContract::CONSERVATIVE
    };
    let non_view = ReturnContract::CONSERVATIVE;
    assert!(view.join(view).returns_sharing_view);
    assert!(!view.join(non_view).returns_sharing_view);
    assert!(!non_view.join(view).returns_sharing_view);
    assert!(!non_view.join(non_view).returns_sharing_view);
}

#[test]
fn returns_sharing_view_optimistic_init_clears_via_join() {
    const { assert!(!ReturnContract::CONSERVATIVE.returns_sharing_view) };
    const { assert!(ReturnContract::OPTIMISTIC.returns_sharing_view) };
    let extracted_non_view = ReturnContract::CONSERVATIVE;
    assert!(
        !ReturnContract::OPTIMISTIC
            .join(extracted_non_view)
            .returns_sharing_view
    );
}

#[test]
fn function_effect_facts_classify_only_proven_no_write_contracts_read_only() {
    let mut contract = MemoryContract::all_borrowed(1, FipContract::Never);
    contract.params[0].cardinality = Cardinality::Once;
    contract.effects = EffectSummary::OPTIMISTIC;

    let facts = contract.function_effect_facts(&read_only_function());

    assert_eq!(facts.effects(), EffectSummary::OPTIMISTIC);
    assert!(!facts.may_write_inaccessible());
    assert_eq!(facts.memory_access(), MemoryAccessClass::ReadOnly);
}

#[test]
fn function_effect_facts_fail_closed_for_every_write_source() {
    let baseline = MemoryContract::all_borrowed(1, FipContract::Never);
    let mut cases = Vec::new();

    let mut allocation = baseline.clone();
    allocation.effects.may_allocate = true;
    cases.push(allocation);

    let mut deallocation = baseline.clone();
    deallocation.effects.may_deallocate = true;
    cases.push(deallocation);

    let mut sharing_effect = baseline.clone();
    sharing_effect.effects.may_share = true;
    cases.push(sharing_effect);

    let mut throwing = baseline.clone();
    throwing.effects.may_throw = true;
    cases.push(throwing);

    let mut owned_param = baseline.clone();
    owned_param.params[0].cardinality = Cardinality::Once;
    owned_param.params[0].access = AccessClass::Owned;
    cases.push(owned_param);

    let mut sharing_param = baseline;
    sharing_param.params[0].cardinality = Cardinality::Once;
    sharing_param.params[0].may_share = true;
    cases.push(sharing_param);

    for contract in cases {
        assert_eq!(
            contract
                .function_effect_facts(&read_only_function())
                .memory_access(),
            MemoryAccessClass::ReadWrite
        );
    }
}

#[test]
fn function_effect_facts_fail_closed_for_untyped_calls() {
    let contract = MemoryContract::all_borrowed(0, FipContract::Never);
    let interner = ori_ir::StringInterner::new();

    for symbol in [
        "known_internal_function",
        "unknown_external_function",
        "ori_print",
        "ori_panic",
        "ori_tls_set",
    ] {
        let mut function = read_only_function();
        function.blocks[0].body.push(ArcInstr::Apply {
            dst: var(0),
            ty: Idx::UNIT,
            func: interner.intern(symbol),
            args: Vec::new(),
            arg_ownership: Vec::new(),
            mono_instance_id: None,
        });

        let facts = contract.function_effect_facts(&function);
        assert!(
            facts.may_write_inaccessible(),
            "untyped call to {symbol} must fail closed"
        );
        assert_eq!(
            facts.memory_access(),
            MemoryAccessClass::ReadWrite,
            "untyped call to {symbol} must not acquire a ReadOnly proof"
        );
    }

    let mut indirect = read_only_function();
    indirect.blocks[0].body.push(ArcInstr::ApplyIndirect {
        dst: var(0),
        ty: Idx::UNIT,
        closure: var(1),
        args: Vec::new(),
        arg_ownership: Vec::new(),
    });
    let indirect_facts = contract.function_effect_facts(&indirect);
    assert!(indirect_facts.may_write_inaccessible());
    assert_eq!(indirect_facts.memory_access(), MemoryAccessClass::ReadWrite);

    let mut resume = read_only_function();
    resume.blocks[0].terminator = ArcTerminator::Resume;
    let resume_facts = contract.function_effect_facts(&resume);
    assert!(resume_facts.may_write_inaccessible());
    assert_eq!(resume_facts.memory_access(), MemoryAccessClass::ReadWrite);
}
