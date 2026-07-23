//! Tests for interprocedural AIMS analysis.

use ori_ir::{BinaryOp, Name};
use ori_registry::{OpStrategy, PrimitiveAllocationEffect, RuntimeOperator};
use ori_types::Idx;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::{
    CalleeOwnerDemand, ContextRegion, FipContract, MemoryAccessClass, MemoryContract,
};
use crate::aims::intraprocedural::analyze_function;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    ArgOwnership, CtorKind, LitValue, PrimOp, PrimitiveFact,
};
use crate::ownership::Ownership;
use crate::test_helpers::{make_apply, make_block};
use crate::ArcClass;

use super::super::lattice::{AccessClass, Cardinality, Uniqueness};
use super::*;

// Test helpers

struct TestClassifier {
    scalars: Vec<bool>,
    builtin_tags: Vec<Option<ori_registry::TypeTag>>,
}

impl TestClassifier {
    fn all_ref(count: usize) -> Self {
        Self {
            scalars: vec![false; count],
            builtin_tags: vec![None; count],
        }
    }

    fn with_scalar(mut self, idx: usize) -> Self {
        if idx < self.scalars.len() {
            self.scalars[idx] = true;
        }
        self
    }

    fn with_builtin_tag(mut self, idx: usize, tag: ori_registry::TypeTag) -> Self {
        if idx < self.builtin_tags.len() {
            self.builtin_tags[idx] = Some(tag);
        }
        self
    }
}

impl crate::ArcClassification for TestClassifier {
    fn arc_class(&self, idx: Idx) -> ArcClass {
        if self
            .scalars
            .get(idx.raw() as usize)
            .copied()
            .unwrap_or(false)
        {
            ArcClass::Scalar
        } else {
            ArcClass::DefiniteRef
        }
    }

    fn builtin_type_tag(&self, idx: Idx) -> Option<ori_registry::TypeTag> {
        self.builtin_tags.get(idx.raw() as usize).copied().flatten()
    }
}

fn block_id(n: u32) -> ArcBlockId {
    ArcBlockId::new(n)
}

fn var(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

fn ty(n: u32) -> Idx {
    Idx::from_raw(n)
}

fn name(n: u32) -> Name {
    Name::from_raw(n)
}

// Extract contract from a single function (no interprocedural context)

#[test]
fn extract_contract_literal_return() {
    // fn f() -> int { return 42 }
    // v0 = literal 42; return v0
    let func = ArcFunction {
        name: name(1),
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: var(0),
                ty: ty(0),
                value: ArcValue::Literal(LitValue::Int(42)),
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1).with_scalar(0);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &func,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &ori_ir::StringInterner::new(),
    );

    assert!(contract.params.is_empty());
    assert_eq!(contract.return_info.uniqueness, Uniqueness::Unique);
}

#[test]
fn primitive_allocation_facts_reach_rl30_through_the_final_contract() {
    for (runtime, expected_allocation) in [
        (
            RuntimeOperator::StringConcat,
            PrimitiveAllocationEffect::MayAllocate,
        ),
        (
            RuntimeOperator::ListConcat,
            PrimitiveAllocationEffect::StrategyDependent,
        ),
    ] {
        let mut func = ArcFunction {
            name: name(90),
            params: vec![
                ArcParam {
                    var: var(0),
                    ty: ty(0),
                    ownership: Ownership::Owned,
                },
                ArcParam {
                    var: var(1),
                    ty: ty(0),
                    ownership: Ownership::Owned,
                },
            ],
            var_types: vec![ty(0), ty(0), ty(0)],
            blocks: vec![ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: var(2),
                    ty: ty(0),
                    value: ArcValue::PrimOp {
                        op: PrimOp::Binary(BinaryOp::Add),
                        args: vec![var(0), var(1)],
                    },
                }],
                terminator: ArcTerminator::Return { value: var(2) },
            }],
            ..ArcFunction::default()
        };
        let fact = PrimitiveFact::resolve(OpStrategy::RuntimeCall(runtime), 2)
            .unwrap_or_else(|| panic!("{runtime:?} must have a binary descriptor"));
        assert_eq!(fact.descriptor.allocation, expected_allocation);
        func.primitive_facts.insert(var(2), fact);

        let classifier = TestClassifier::all_ref(1);
        let sigs = FxHashMap::default();
        let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
        let contract = extract_contract(
            &func,
            &state_map,
            &classifier,
            &sigs,
            &FxHashSet::default(),
            &[],
            &ori_ir::StringInterner::new(),
        );

        assert!(
            contract.effects.may_allocate,
            "{runtime:?} allocation descriptor must reach the final contract"
        );
        assert_eq!(
            contract.function_effect_facts(&func).memory_access(),
            MemoryAccessClass::ReadWrite,
            "{runtime:?} allocation descriptor must prevent an RL-30 ReadOnly proof"
        );
    }
}

#[test]
fn extract_contract_param_used_once() {
    // fn f(x: str) -> str { return x }
    // param v0: str; return v0
    let func = ArcFunction {
        name: name(2),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &func,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &ori_ir::StringInterner::new(),
    );

    assert_eq!(contract.params.len(), 1);
    // Param returned directly → used once.
    assert_eq!(contract.params[0].cardinality, Cardinality::Once);
    // Returning a param → preserves freshness.
    assert!(contract.return_info.preserves_freshness);
}

#[test]
fn list_concat_return_is_unique_but_not_frozen_as_self_allocated() {
    let mut func = ArcFunction {
        name: name(3),
        params: vec![
            ArcParam {
                var: var(0),
                ty: ty(0),
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: var(1),
                ty: ty(0),
                ownership: Ownership::Owned,
            },
        ],
        return_type: ty(0),
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: var(2),
                ty: ty(0),
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Add),
                    args: vec![var(0), var(1)],
                },
            }],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };
    let Some(fact) =
        PrimitiveFact::resolve(OpStrategy::RuntimeCall(RuntimeOperator::ListConcat), 2)
    else {
        panic!("expected a list-concat descriptor");
    };
    assert!(func.primitive_facts.insert(var(2), fact).is_none());

    let classifier = TestClassifier::all_ref(1);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &func,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &ori_ir::StringInterner::new(),
    );

    assert_eq!(contract.return_info.uniqueness, Uniqueness::Unique);
    assert!(contract
        .params
        .iter()
        .all(|parameter| parameter.access == AccessClass::Owned));
    assert!(contract.return_info.preserves_freshness);
    assert!(contract.effects.may_allocate);
    assert!(!contract.return_info.returns_fresh_self_alloc);
    assert!(!contract.fresh_self_allocation_facts().is_proven());
}

#[test]
fn extract_contract_iter_consume_propagates_through_forwarding_wrapper() {
    // A wrapper forwarding its parameter to an iter-consuming callee inherits
    // `iter_consumes`; a borrow-only callee would not propagate it.
    let iterate_words = name(7);
    let mut callee_contract = MemoryContract::conservative(1);
    callee_contract.params[0].iter_consumes = true;
    let mut sigs = FxHashMap::default();
    sigs.insert(iterate_words, callee_contract);

    let wrapper = ArcFunction {
        name: name(8),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Borrowed,
        }],
        return_type: ty(1),
        var_types: vec![ty(0), ty(1)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(1),
                ty: ty(1),
                func: iterate_words,
                args: vec![var(0)],
                arg_ownership: vec![ArgOwnership::Borrowed],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let state_map = analyze_function(&wrapper, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &wrapper,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &ori_ir::StringInterner::new(),
    );
    assert!(
        contract.params[0].iter_consumes,
        "forwarding a param to an iter-consuming callee propagates iter_consumes"
    );

    // Negative: a callee whose param iter_consumes=false does NOT propagate.
    let mut sigs_borrow = FxHashMap::default();
    sigs_borrow.insert(iterate_words, MemoryContract::conservative(1));
    let state_map_b = analyze_function(&wrapper, &classifier, &sigs_borrow, &[], Vec::new());
    let contract_b = extract_contract(
        &wrapper,
        &state_map_b,
        &classifier,
        &sigs_borrow,
        &FxHashSet::default(),
        &[],
        &ori_ir::StringInterner::new(),
    );
    assert!(
        !contract_b.params[0].iter_consumes,
        "forwarding to a NON-iter-consuming (borrow-read) callee does NOT propagate"
    );
}

#[test]
fn borrowed_projected_iteration_does_not_transfer_aggregate_field_credit() {
    use ori_ir::builtin_constants::protocol::ProtocolBuiltin;

    let interner = ori_ir::StringInterner::new();
    let iter = interner.intern(ProtocolBuiltin::Iter.name());
    let iter_drop = interner.intern(ProtocolBuiltin::IterDrop.name());
    let func = ArcFunction {
        name: name(9),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Borrowed,
        }],
        return_type: ty(4),
        var_types: vec![ty(0), ty(1), ty(2), ty(3), ty(4)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Project {
                    dst: var(1),
                    ty: ty(1),
                    value: var(0),
                    field: 0,
                },
                ArcInstr::Apply {
                    dst: var(2),
                    ty: ty(2),
                    func: iter,
                    args: vec![var(1)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                ArcInstr::Apply {
                    dst: var(3),
                    ty: ty(3),
                    func: iter_drop,
                    args: vec![var(2)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                ArcInstr::Let {
                    dst: var(4),
                    ty: ty(4),
                    value: ArcValue::Literal(LitValue::Int(0)),
                },
            ],
            terminator: ArcTerminator::Return { value: var(4) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(5).with_scalar(3).with_scalar(4);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &func,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &interner,
    );

    assert!(!contract.params[0].iter_consumes);
    assert_eq!(
        contract.params[0].callee_owner_demand(),
        CalleeOwnerDemand::Borrow,
        "iterator drop consumes its own retained projection, not aggregate owner credit"
    );
}

#[test]
fn exact_projected_cow_reconstruction_transfers_the_aggregate_boundary() {
    let interner = ori_ir::StringInterner::new();
    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    let push = interner.intern("push");
    let mut sigs = FxHashMap::default();
    crate::aims::builtins::seed_builtin_contracts(&mut sigs, &builtins, &interner);

    let func = ArcFunction {
        name: name(10),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Borrowed,
        }],
        return_type: ty(0),
        var_types: vec![ty(0), ty(1), ty(2), ty(1), ty(3), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Project {
                    dst: var(1),
                    ty: ty(1),
                    value: var(0),
                    field: 0,
                },
                ArcInstr::Let {
                    dst: var(2),
                    ty: ty(2),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Apply {
                    dst: var(3),
                    ty: ty(1),
                    func: push,
                    args: vec![var(1), var(2)],
                    arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Borrowed],
                    mono_instance_id: None,
                },
                ArcInstr::Project {
                    dst: var(4),
                    ty: ty(3),
                    value: var(0),
                    field: 1,
                },
                ArcInstr::Construct {
                    dst: var(5),
                    ty: ty(0),
                    ctor: CtorKind::Struct(name(80)),
                    args: vec![var(3), var(4)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(5) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(4).with_scalar(2);
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &func,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &interner,
    );

    assert_eq!(
        contract.params[0].access,
        AccessClass::Owned,
        "an exact all-field reconstruction must transfer the aggregate owner \
         into the helper so the projected COW receiver reuses its existing credit"
    );
    assert_eq!(
        contract.params[0].callee_owner_demand(),
        CalleeOwnerDemand::WholeValue,
        "the helper boundary transfers the complete aggregate, never a synthetic \
         single projected-field credit"
    );
}

#[test]
fn registered_receiver_only_relay_transfers_aggregate_boundary() {
    let interner = ori_ir::StringInterner::new();
    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    let remove = interner.intern("remove");
    let mut sigs = FxHashMap::default();
    crate::aims::builtins::seed_builtin_contracts(&mut sigs, &builtins, &interner);

    let func = ArcFunction {
        name: name(13),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Borrowed,
        }],
        return_type: ty(0),
        var_types: vec![ty(0), ty(1), ty(2), ty(1), ty(3), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Project {
                    dst: var(1),
                    ty: ty(1),
                    value: var(0),
                    field: 0,
                },
                ArcInstr::Let {
                    dst: var(2),
                    ty: ty(2),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Apply {
                    dst: var(3),
                    ty: ty(1),
                    func: remove,
                    args: vec![var(1), var(2)],
                    arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Borrowed],
                    mono_instance_id: None,
                },
                ArcInstr::Project {
                    dst: var(4),
                    ty: ty(3),
                    value: var(0),
                    field: 1,
                },
                ArcInstr::Construct {
                    dst: var(5),
                    ty: ty(0),
                    ctor: CtorKind::Struct(name(80)),
                    args: vec![var(3), var(4)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(5) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(4).with_scalar(2);
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &func,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &interner,
    );

    assert_eq!(
        contract.params[0].access,
        AccessClass::Owned,
        "the frozen Owned receiver contract proves effective consumption"
    );
    assert_eq!(
        contract.params[0].callee_owner_demand(),
        CalleeOwnerDemand::WholeValue
    );
}

#[test]
fn registered_borrowed_relay_ignores_hardcoded_owned_annotation() {
    let contract = borrowed_relay_reconstruction_contract("registered_borrowed_relay", false);

    assert_eq!(contract.params[0].access, AccessClass::Borrowed);
    assert_eq!(
        contract.params[0].callee_owner_demand(),
        CalleeOwnerDemand::Borrow
    );
}

#[test]
fn exact_push_name_collision_ignores_hardcoded_owned_annotation() {
    let contract = borrowed_relay_reconstruction_contract("push", true);

    assert_eq!(contract.params[0].access, AccessClass::Borrowed);
    assert_eq!(
        contract.params[0].callee_owner_demand(),
        CalleeOwnerDemand::Borrow
    );
}

fn borrowed_relay_reconstruction_contract(relay_spelling: &str, exact: bool) -> MemoryContract {
    let interner = ori_ir::StringInterner::new();
    let relay = interner.intern(relay_spelling);
    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    let mut sigs = FxHashMap::default();
    sigs.insert(relay, MemoryContract::all_borrowed(2, FipContract::Never));
    let func = ArcFunction {
        name: name(14),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Borrowed,
        }],
        return_type: ty(0),
        var_types: vec![ty(0), ty(1), ty(2), ty(1), ty(3), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Project {
                    dst: var(1),
                    ty: ty(1),
                    value: var(0),
                    field: 0,
                },
                ArcInstr::Let {
                    dst: var(2),
                    ty: ty(2),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Apply {
                    dst: var(3),
                    ty: ty(1),
                    func: relay,
                    args: vec![var(1), var(2)],
                    arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Borrowed],
                    mono_instance_id: None,
                },
                ArcInstr::Project {
                    dst: var(4),
                    ty: ty(3),
                    value: var(0),
                    field: 1,
                },
                ArcInstr::Construct {
                    dst: var(5),
                    ty: ty(0),
                    ctor: CtorKind::Struct(name(80)),
                    args: vec![var(3), var(4)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(5) },
        }],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(4).with_scalar(2);
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let exact_callables = if exact {
        FxHashSet::from_iter([relay])
    } else {
        FxHashSet::default()
    };
    extract_contract_with_call_ownership(&ContractExtractionInput {
        func: &func,
        state_map: &state_map,
        classifier: &classifier,
        sigs: &sigs,
        scc_peers: &FxHashSet::default(),
        context_regions: &[],
        interner: &interner,
        builtins: &builtins,
        exact_callables: &exact_callables,
    })
}

#[test]
fn projected_cow_consume_without_exact_reconstruction_stays_borrowed() {
    let interner = ori_ir::StringInterner::new();
    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    let push = interner.intern("push");
    let mut sigs = FxHashMap::default();
    crate::aims::builtins::seed_builtin_contracts(&mut sigs, &builtins, &interner);

    let func = ArcFunction {
        name: name(11),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Borrowed,
        }],
        return_type: ty(1),
        var_types: vec![ty(0), ty(1), ty(2), ty(1)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Project {
                    dst: var(1),
                    ty: ty(1),
                    value: var(0),
                    field: 0,
                },
                ArcInstr::Let {
                    dst: var(2),
                    ty: ty(2),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Apply {
                    dst: var(3),
                    ty: ty(1),
                    func: push,
                    args: vec![var(1), var(2)],
                    arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Borrowed],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return { value: var(3) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(3).with_scalar(2);
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &func,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &interner,
    );

    assert_eq!(contract.params[0].access, AccessClass::Borrowed);
    assert_eq!(
        contract.params[0].callee_owner_demand(),
        CalleeOwnerDemand::Borrow,
        "a consumed projection alone cannot transfer or destroy its parent aggregate"
    );
}

#[test]
fn opaque_projected_relay_does_not_transfer_aggregate_boundary() {
    let interner = ori_ir::StringInterner::new();
    let opaque = interner.intern("opaque_projected_relay");
    let sigs = FxHashMap::default();
    let func = ArcFunction {
        name: name(12),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Borrowed,
        }],
        return_type: ty(0),
        var_types: vec![ty(0), ty(1), ty(2), ty(1), ty(3), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Project {
                    dst: var(1),
                    ty: ty(1),
                    value: var(0),
                    field: 0,
                },
                ArcInstr::Let {
                    dst: var(2),
                    ty: ty(2),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Apply {
                    dst: var(3),
                    ty: ty(1),
                    func: opaque,
                    args: vec![var(1), var(2)],
                    arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Borrowed],
                    mono_instance_id: None,
                },
                ArcInstr::Project {
                    dst: var(4),
                    ty: ty(3),
                    value: var(0),
                    field: 1,
                },
                ArcInstr::Construct {
                    dst: var(5),
                    ty: ty(0),
                    ctor: CtorKind::Struct(name(80)),
                    args: vec![var(3), var(4)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(5) },
        }],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(4).with_scalar(2);
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &func,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &interner,
    );

    assert_eq!(contract.params[0].access, AccessClass::Borrowed);
    assert_eq!(
        contract.params[0].callee_owner_demand(),
        CalleeOwnerDemand::Borrow,
        "an opaque Owned annotation is conservative fallback, not exact \
         aggregate-reconstruction authority"
    );
}

#[test]
fn extract_contract_project_return_alias_propagates_through_forwarder() {
    // INVARIANT: A forwarder preserves the callee's projected return alias so
    // callers retain the owning box through the view's last use.
    let unwrap = name(11);
    let mut callee_contract = MemoryContract::conservative(1);
    callee_contract.params[0].return_alias =
        Some(crate::aims::contract::ReturnAliasShape::Project { field: 0 });
    let mut sigs = FxHashMap::default();
    sigs.insert(unwrap, callee_contract);

    let unbox = ArcFunction {
        name: name(12),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Borrowed,
        }],
        return_type: ty(1),
        var_types: vec![ty(0), ty(1)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: var(1),
                    ty: ty(1),
                    func: unwrap,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    normal: block_id(1),
                    unwind: block_id(2),
                    mono_instance_id: None,
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(1) },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let state_map = analyze_function(&unbox, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &unbox,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &ori_ir::StringInterner::new(),
    );
    assert_eq!(
        contract.params[0].return_alias,
        Some(crate::aims::contract::ReturnAliasShape::Project { field: 0 }),
        "forwarding a param to a Project-return-alias callee propagates Project field 0"
    );

    // Negative pin: a callee with NO return_alias (returns a fresh value) does
    // NOT propagate any Project alias — the forwarder's param stays None, so a
    // caller never suppresses a release on a genuinely-fresh forwarded result.
    let mut sigs_fresh = FxHashMap::default();
    sigs_fresh.insert(unwrap, MemoryContract::conservative(1));
    let state_map_f = analyze_function(&unbox, &classifier, &sigs_fresh, &[], Vec::new());
    let contract_f = extract_contract(
        &unbox,
        &state_map_f,
        &classifier,
        &sigs_fresh,
        &FxHashSet::default(),
        &[],
        &ori_ir::StringInterner::new(),
    );
    assert_eq!(
        contract_f.params[0].return_alias, None,
        "forwarding to a callee with no return_alias does NOT propagate a Project alias"
    );
}

#[test]
fn extract_contract_construct_project_roundtrip_records_direct() {
    // INVARIANT: Projecting a just-constructed field back out preserves the
    // original parameter identity and transfers it through the return.
    let wrap = name(20);
    let f = ArcFunction {
        name: name(21),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Borrowed,
        }],
        return_type: ty(0),
        var_types: vec![ty(0), ty(1), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                // %1 = Construct Wrap(%0)
                ArcInstr::Construct {
                    dst: var(1),
                    ty: ty(1),
                    ctor: CtorKind::Struct(wrap),
                    args: vec![var(0)],
                },
                // %2 = Project %1.0  (== %0, the param)
                ArcInstr::Project {
                    dst: var(2),
                    ty: ty(0),
                    value: var(1),
                    field: 0,
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(3);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&f, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &f,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &ori_ir::StringInterner::new(),
    );
    assert_eq!(
        contract.params[0].return_alias,
        Some(crate::aims::contract::ReturnAliasShape::Direct),
        "construct-project round-trip records Direct on the round-tripped param"
    );
    assert!(
        contract.params[0].transfers_through_return,
        "Direct round-trip param transfers through the return (invariant paired)"
    );

    // Negative pin: a FRESH struct returned WHOLE (no project-back-to-param) does
    // NOT record a round-trip Direct — the return is the construct itself, not the
    // param, so suppressing the param's release would be unsound.
    let g = ArcFunction {
        name: name(22),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Borrowed,
        }],
        return_type: ty(1),
        var_types: vec![ty(0), ty(1)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(1),
                ty: ty(1),
                ctor: CtorKind::Struct(wrap),
                args: vec![var(0)],
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };
    let classifier_g = TestClassifier::all_ref(2);
    let state_map_g = analyze_function(&g, &classifier_g, &sigs, &[], Vec::new());
    let contract_g = extract_contract(
        &g,
        &state_map_g,
        &classifier_g,
        &sigs,
        &FxHashSet::default(),
        &[],
        &ori_ir::StringInterner::new(),
    );
    assert_ne!(
        contract_g.params[0].return_alias,
        Some(crate::aims::contract::ReturnAliasShape::Direct),
        "a fresh struct returned WHOLE is not a construct-project round-trip Direct"
    );
}

#[test]
fn borrowed_read_only_true_for_param_at_borrowed_user_call_position() {
    // A parameter forwarded only to a borrowed read-only callee remains
    // `borrowed_read_only` under Annex E §AIMS RL-2.
    let read_only = name(7);
    let mut callee = MemoryContract::conservative(1);
    callee.params[0].access = AccessClass::Borrowed;
    callee.params[0].borrowed_read_only = true;
    let mut sigs = FxHashMap::default();
    sigs.insert(read_only, callee);

    let fwd = ArcFunction {
        name: name(8),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Borrowed,
        }],
        return_type: ty(1),
        var_types: vec![ty(0), ty(1)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(1),
                ty: ty(1),
                func: read_only,
                args: vec![var(0)],
                arg_ownership: vec![ArgOwnership::Borrowed],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(2);
    let state_map = analyze_function(&fwd, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &fwd,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &ori_ir::StringInterner::new(),
    );
    assert!(
        contract.params[0].borrowed_read_only,
        "a param flowing only to a borrowed read-only callee position is borrowed_read_only"
    );
}

#[test]
fn extract_contract_publishes_borrowed_callee_sharing_on_param() {
    let sharing_callee = name(70);
    let mut callee = MemoryContract::conservative(1);
    callee.params[0].access = AccessClass::Borrowed;
    callee.params[0].may_share = true;
    callee.effects.may_share = true;
    let sigs = FxHashMap::from_iter([(sharing_callee, callee)]);

    let wrapper = ArcFunction {
        name: name(71),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Borrowed,
        }],
        return_type: ty(1),
        var_types: vec![ty(0), ty(1)],
        blocks: vec![make_block(
            block_id(0),
            vec![make_apply(
                var(1),
                ty(1),
                sharing_callee,
                vec![var(0)],
                vec![ArgOwnership::Borrowed],
            )],
            ArcTerminator::Return { value: var(1) },
        )],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(2);
    let state_map = analyze_function(&wrapper, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &wrapper,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &ori_ir::StringInterner::new(),
    );

    assert!(
        contract.params[0].may_share,
        "a wrapper must publish a borrowed callee's retained owner"
    );
}

#[test]
fn extract_contract_publishes_borrowed_alias_credit_for_construct() {
    let func = ArcFunction {
        name: name(72),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Borrowed,
        }],
        return_type: ty(0),
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![make_block(
            block_id(0),
            vec![
                ArcInstr::Let {
                    dst: var(1),
                    ty: ty(0),
                    value: ArcValue::Var(var(0)),
                },
                ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(0),
                    ctor: CtorKind::Struct(name(73)),
                    args: vec![var(1)],
                },
            ],
            ArcTerminator::Return { value: var(2) },
        )],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(1);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &func,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &ori_ir::StringInterner::new(),
    );

    assert_eq!(contract.params[0].access, AccessClass::Borrowed);
    assert!(
        contract.params[0].may_share,
        "a borrowed alias moved into constructed storage needs a published credit"
    );
}

#[test]
fn extract_contract_publishes_typed_cow_credit_for_live_borrowed_param() {
    let interner = ori_ir::StringInterner::new();
    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    let mut sigs = FxHashMap::default();
    crate::aims::builtins::seed_builtin_contracts(&mut sigs, &builtins, &interner);
    let push = interner.intern("push");
    let len = interner.intern("len");
    let func = ArcFunction {
        name: name(74),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Borrowed,
        }],
        return_type: ty(1),
        var_types: vec![ty(0), ty(0), ty(1), ty(0), ty(1)],
        blocks: vec![make_block(
            block_id(0),
            vec![
                ArcInstr::Let {
                    dst: var(1),
                    ty: ty(0),
                    value: ArcValue::Var(var(0)),
                },
                ArcInstr::Let {
                    dst: var(2),
                    ty: ty(1),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                make_apply(var(3), ty(0), push, vec![var(1), var(2)], Vec::new()),
                make_apply(var(4), ty(1), len, vec![var(0)], Vec::new()),
            ],
            ArcTerminator::Return { value: var(4) },
        )],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(2)
        .with_scalar(1)
        .with_builtin_tag(0, ori_registry::TypeTag::List)
        .with_builtin_tag(1, ori_registry::TypeTag::Int);
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &func,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &interner,
    );

    assert_eq!(contract.params[0].access, AccessClass::Borrowed);
    assert!(
        contract.params[0].may_share,
        "a typed owned COW handoff must publish its borrowed-root credit"
    );
}

#[test]
fn exact_callable_named_like_cow_builtin_does_not_publish_credit() {
    let interner = ori_ir::StringInterner::new();
    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    let local_push = interner.intern("push");
    let sigs = FxHashMap::from_iter([(
        local_push,
        MemoryContract::all_borrowed(1, FipContract::Never),
    )]);
    let func = ArcFunction {
        name: name(75),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Borrowed,
        }],
        return_type: ty(1),
        var_types: vec![ty(0), ty(1)],
        blocks: vec![make_block(
            block_id(0),
            vec![make_apply(
                var(1),
                ty(1),
                local_push,
                vec![var(0)],
                Vec::new(),
            )],
            ArcTerminator::Return { value: var(1) },
        )],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(2)
        .with_scalar(1)
        .with_builtin_tag(0, ori_registry::TypeTag::List)
        .with_builtin_tag(1, ori_registry::TypeTag::Int);
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let exact_callables = FxHashSet::from_iter([local_push]);
    let contract = extract_contract_with_call_ownership(&ContractExtractionInput {
        func: &func,
        state_map: &state_map,
        classifier: &classifier,
        sigs: &sigs,
        scc_peers: &FxHashSet::default(),
        context_regions: &[],
        interner: &interner,
        builtins: &builtins,
        exact_callables: &exact_callables,
    });

    assert!(
        !contract.params[0].may_share,
        "an exact local callable must not inherit same-spelled builtin ownership"
    );
}

#[test]
fn borrowed_read_only_false_for_param_at_owned_consumer_position() {
    // Forwarding to an owned, COW-mutating position must clear the RL-2
    // read-only fact, preventing the caller carve-out from admitting a shared
    // buffer.
    let consume = name(7);
    let mut callee = MemoryContract::conservative(1);
    callee.params[0].access = AccessClass::Owned;
    callee.params[0].borrowed_read_only = false;
    let mut sigs = FxHashMap::default();
    sigs.insert(consume, callee);

    let fwd = ArcFunction {
        name: name(8),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Borrowed,
        }],
        return_type: ty(1),
        var_types: vec![ty(0), ty(1)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(1),
                ty: ty(1),
                func: consume,
                args: vec![var(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(2);
    let state_map = analyze_function(&fwd, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &fwd,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &ori_ir::StringInterner::new(),
    );
    assert!(
        !contract.params[0].borrowed_read_only,
        "a param flowing to an OWNED consumer position is NOT borrowed_read_only"
    );
}

#[test]
fn extract_contract_construct_return_is_unique() {
    // fn f() -> Point { return Point { x: 1, y: 2 } }
    // v0 = literal 1; v1 = literal 2; v2 = Construct(Point, [v0, v1]); return v2
    let func = ArcFunction {
        name: name(3),
        return_type: ty(2),
        var_types: vec![ty(0), ty(0), ty(2)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Let {
                    dst: var(1),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(2)),
                },
                ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(2),
                    ctor: CtorKind::Struct(name(10)),
                    args: vec![var(0), var(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(3).with_scalar(0);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &func,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &ori_ir::StringInterner::new(),
    );

    assert_eq!(contract.return_info.uniqueness, Uniqueness::Unique);
    assert!(contract.return_info.preserves_freshness);
}

#[test]
fn analyze_program_single_function() {
    // fn f(x: str) -> str { return x }
    let func = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(&[func], &classifier, &builtins, &interner);
    assert!(contracts.contains_key(&name(1)));

    let contract = &contracts[&name(1)];
    assert_eq!(contract.params.len(), 1);
    assert_eq!(contract.params[0].cardinality, Cardinality::Once);
}

#[test]
fn analyze_program_callee_before_caller() {
    // fn callee() -> T { Construct(T) }
    // fn caller() -> T { Apply(callee) }
    // Callee returns Unique → caller's return should also be Unique.
    let callee = ArcFunction {
        name: name(1),
        return_type: ty(1),
        var_types: vec![ty(1)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(0),
                ty: ty(1),
                ctor: CtorKind::Struct(name(10)),
                args: vec![],
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let caller = ArcFunction {
        name: name(2),
        return_type: ty(1),
        var_types: vec![ty(1)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(0),
                ty: ty(1),
                func: name(1),
                args: vec![],
                arg_ownership: vec![],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(&[callee, caller], &classifier, &builtins, &interner);

    // Callee constructs → Unique.
    assert_eq!(
        contracts[&name(1)].return_info.uniqueness,
        Uniqueness::Unique
    );
    // Caller calls callee with Unique return → also Unique.
    assert_eq!(
        contracts[&name(2)].return_info.uniqueness,
        Uniqueness::Unique
    );
}

// Effect Activation tests

#[test]
fn pure_function_call_preserves_caller_uniqueness() {
    // A pure identity callee borrows its once-used argument without widening
    // uniqueness or reporting sharing.
    let callee = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let caller = ArcFunction {
        name: name(2),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(1),
                ty: ty(0),
                func: name(1),
                args: vec![var(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(&[callee, caller], &classifier, &builtins, &interner);

    // Callee is pure: no allocations, no sharing.
    let callee_contract = &contracts[&name(1)];
    assert!(
        !callee_contract.effects.may_share,
        "pure callee should have may_share=false"
    );
    assert!(
        !callee_contract.effects.may_allocate,
        "pure callee should have may_allocate=false"
    );
    assert!(callee_contract.is_fbip, "pure callee should be FBIP");

    // Caller calls a pure callee → caller's effects propagate from callee.
    let caller_contract = &contracts[&name(2)];
    assert!(
        !caller_contract.effects.may_share,
        "caller of pure function should have may_share=false"
    );
}

#[test]
fn function_without_allocations_is_fbip() {
    // fn f(x: T) -> T { return x }
    // No Construct, no PartialApply → may_allocate=false → is_fbip=true.
    let func = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(&[func], &classifier, &builtins, &interner);
    let contract = &contracts[&name(1)];

    assert!(
        !contract.effects.may_allocate,
        "no Construct → no allocation"
    );
    assert!(contract.is_fbip, "non-allocating function should be FBIP");

    // Contrast: a function WITH a Construct should NOT be FBIP.
    let allocating_func = ArcFunction {
        name: name(2),
        return_type: ty(1),
        var_types: vec![ty(0), ty(1)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Construct {
                    dst: var(1),
                    ty: ty(1),
                    ctor: CtorKind::Struct(name(10)),
                    args: vec![var(0)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let contracts = analyze_program(&[allocating_func], &classifier, &builtins, &interner);
    let alloc_contract = &contracts[&name(2)];

    assert!(
        alloc_contract.effects.may_allocate,
        "Construct → may_allocate"
    );
    assert!(
        !alloc_contract.is_fbip,
        "allocating function should NOT be FBIP"
    );
}

#[test]
fn effect_propagation_through_scc_converges() {
    // Two pure mutually recursive identity functions should converge to no
    // allocation and no sharing through the SCC fixed point.
    let func_a = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(1),
                ty: ty(0),
                func: name(2), // calls b
                args: vec![var(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let func_b = ArcFunction {
        name: name(2),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(1),
                ty: ty(0),
                func: name(1), // calls a
                args: vec![var(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(&[func_a, func_b], &classifier, &builtins, &interner);

    // Both functions exist in the result.
    assert!(
        contracts.contains_key(&name(1)),
        "func_a should have contract"
    );
    assert!(
        contracts.contains_key(&name(2)),
        "func_b should have contract"
    );

    // Neither allocates → may_allocate converges to false.
    let a_contract = &contracts[&name(1)];
    let b_contract = &contracts[&name(2)];

    assert!(
        !a_contract.effects.may_allocate,
        "SCC with no Construct should converge to may_allocate=false"
    );
    assert!(
        !b_contract.effects.may_allocate,
        "SCC with no Construct should converge to may_allocate=false"
    );

    // Both are FBIP (no allocations).
    assert!(
        a_contract.is_fbip,
        "non-allocating SCC member should be FBIP"
    );
    assert!(
        b_contract.is_fbip,
        "non-allocating SCC member should be FBIP"
    );

    // Effects are consistent across the SCC.
    assert_eq!(
        a_contract.effects, b_contract.effects,
        "symmetric SCC members should have identical effects"
    );
}

// Demand propagation: linear consumption tightens callee uniqueness

#[test]
fn demand_propagation_single_caller_owned_linear_once() {
    // The sole caller passes one fresh, linear value to the identity callee,
    // satisfying the all-callers uniqueness condition.
    let callee = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let caller = ArcFunction {
        name: name(2),
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: name(1),
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(&[callee, caller], &classifier, &builtins, &interner);

    let callee_contract = &contracts[&name(1)];
    assert_eq!(
        callee_contract.params[0].uniqueness,
        Uniqueness::Unique,
        "single caller passing Owned+Linear+Once → callee param uniqueness should be Unique"
    );
}

#[test]
fn demand_propagation_multiple_callers_all_satisfy() {
    // Every caller passes one fresh, linear value, so the callee parameter is
    // unique under the all-callers condition.
    let callee = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let make_caller = |caller_name: u32| ArcFunction {
        name: name(caller_name),
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: name(1),
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let caller_a = make_caller(2);
    let caller_b = make_caller(3);

    let classifier = TestClassifier::all_ref(2);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(
        &[callee, caller_a, caller_b],
        &classifier,
        &builtins,
        &interner,
    );

    let callee_contract = &contracts[&name(1)];
    assert_eq!(
        callee_contract.params[0].uniqueness,
        Uniqueness::Unique,
        "all callers pass Owned+Linear+Once → callee param uniqueness should be Unique"
    );
}

#[test]
fn demand_propagation_one_caller_violates() {
    // One caller passes the same value twice, so the all-callers uniqueness
    // condition fails and the callee parameter remains maybe-shared.
    let callee = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let caller_good = ArcFunction {
        name: name(2),
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: name(1),
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    // caller_bad passes the same param to callee twice (cardinality=Many).
    let caller_bad = ArcFunction {
        name: name(3),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: name(1),
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                ArcInstr::Apply {
                    dst: var(2),
                    ty: ty(0),
                    func: name(1),
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(3);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(
        &[callee, caller_good, caller_bad],
        &classifier,
        &builtins,
        &interner,
    );

    let callee_contract = &contracts[&name(1)];
    assert_eq!(
        callee_contract.params[0].uniqueness,
        Uniqueness::MaybeShared,
        "one caller violates the condition → callee param stays MaybeShared"
    );
}

#[test]
fn demand_propagation_no_callers_stays_maybe_shared() {
    // callee(p0: T) -> T: { return p0 }
    // No other function calls callee → no demand info → stays MaybeShared.
    let callee = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(&[callee], &classifier, &builtins, &interner);

    let callee_contract = &contracts[&name(1)];
    assert_eq!(
        callee_contract.params[0].uniqueness,
        Uniqueness::MaybeShared,
        "no callers → no demand propagation → stays MaybeShared"
    );
}

/// Verifies that forwarding an owned parameter cannot prove that no aliases exist.
#[test]
fn demand_propagation_forwarded_param_stays_maybe_shared() {
    let callee = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let caller = ArcFunction {
        name: name(2),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(1),
                ty: ty(0),
                func: name(1),
                args: vec![var(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(&[callee, caller], &classifier, &builtins, &interner);

    let callee_contract = &contracts[&name(1)];
    assert_eq!(
        callee_contract.params[0].uniqueness,
        Uniqueness::MaybeShared,
        "forwarded Owned param is not provably single-owner → callee param stays MaybeShared (DP-10/IC-8 removed)"
    );
}

/// A SINGLE fresh-`Construct` variable passed to the SAME
/// (callee, `param_idx`) at >=2 call sites may have another owner at the non-first use. The
/// per-(callee,param) AND-fold must NOT tighten to Unique — the `count_var_uses
/// == 1` guard on Case 1 (fresh Construct used exactly once) keeps it
/// `MaybeShared`. Closes the DP-10-class hole on the Construct path that an
/// unguarded `construct_vars.contains` membership check would leave open.
#[test]
fn demand_propagation_construct_var_at_two_sites_stays_maybe_shared() {
    // Passing one fresh value to the same callee parameter twice disqualifies
    // uniqueness despite its single initial owner.
    let callee = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let caller = ArcFunction {
        name: name(2),
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: name(1),
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                ArcInstr::Apply {
                    dst: var(2),
                    ty: ty(0),
                    func: name(1),
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(&[callee, caller], &classifier, &builtins, &interner);

    let callee_contract = &contracts[&name(1)];
    assert_eq!(
        callee_contract.params[0].uniqueness,
        Uniqueness::MaybeShared,
        "fresh Construct passed to the same (callee, param) at two sites has multiple owner credits at the second use, so it stays MaybeShared"
    );
}

/// A fresh `Construct` used once as a normal call argument and once as a
/// `BurdenInc` operand has multiple logical owner credits. `instr_use_count` counts the
/// burden-op operand as a real use. `count_var_uses == 2` → the
/// `count_var_uses == 1` guard on Case 1 keeps the callee param `MaybeShared`,
/// NOT `Unique`. Pins that the `Burden*` family (`BurdenInc` / `BurdenDec` /
/// `BurdenDecPartial` / `BurdenDecVariant` / `BurdenDecField`) participates in the
/// use-count that gates uniqueness-tightening; a regression that stopped
/// counting a burden operand would silently make a non-unique value appear
/// unique, which would make ownership-event elision unsound.
#[test]
fn demand_propagation_construct_var_with_burden_op_use_stays_maybe_shared() {
    // A fresh value used by both an apply and a burden increment has multiple
    // uses and cannot prove callee-parameter uniqueness.
    let callee = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let caller = ArcFunction {
        name: name(2),
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: name(1),
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                ArcInstr::BurdenInc { var: var(0) },
            ],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(&[callee, caller], &classifier, &builtins, &interner);

    let callee_contract = &contracts[&name(1)];
    assert_eq!(
        callee_contract.params[0].uniqueness,
        Uniqueness::MaybeShared,
        "fresh Construct also used as a BurdenInc operand has multiple owner credits, so count_var_uses == 2 and it stays MaybeShared"
    );
}

// FIP contract classification

#[test]
fn extract_contract_fbip_still_certified() {
    // fn f(x: T) -> T { return x }
    // No allocations → FBIP → FipContract::Certified.
    let func = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(&[func], &classifier, &builtins, &interner);
    let contract = &contracts[&name(1)];

    assert_eq!(
        contract.fip,
        FipContract::Certified,
        "FBIP function should be FipContract::Certified"
    );
    assert!(contract.is_fbip);
}

#[test]
fn extract_contract_token_balanced_produces_conditional() {
    // fn f(x: T) -> T { v1 = Construct(T, []); return v1 }
    // 1 Construct + 1 consumed param → token balanced.
    // But param needs uniqueness for reuse → FipContract::Conditional.
    let func = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(1),
                ty: ty(0),
                ctor: CtorKind::Struct(name(10)),
                args: vec![],
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &func,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &ori_ir::StringInterner::new(),
    );

    // Param v0 is consumed (Dead — never used after entry) and non-scalar.
    // 1 Construct balanced by 1 consumed param.
    // Since param requires uniqueness → Conditional.
    assert!(
        matches!(contract.fip, FipContract::Conditional { .. }),
        "token-balanced with consumed param should produce Conditional, got {:?}",
        contract.fip
    );

    if let FipContract::Conditional {
        requires_unique_params,
    } = &contract.fip
    {
        assert_eq!(requires_unique_params.len(), 1);
        assert!(
            requires_unique_params[0],
            "consumed non-scalar param should require uniqueness"
        );
    }
}

#[test]
fn extract_contract_net_positive_produces_bounded() {
    // fn f(x: T) -> T { v1 = Construct; v2 = Construct; return v2 }
    // 2 Constructs + 1 consumed param → net = 1 → Bounded(1).
    let func = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: var(1),
                    ty: ty(0),
                    ctor: CtorKind::Struct(name(10)),
                    args: vec![],
                },
                ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(0),
                    ctor: CtorKind::Struct(name(10)),
                    args: vec![],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(3);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &func,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &ori_ir::StringInterner::new(),
    );

    // 2 Constructs - 1 consumed param = net 1 → Bounded(1).
    assert_eq!(
        contract.fip,
        FipContract::Bounded(1),
        "net positive allocation should produce Bounded"
    );
}

#[test]
fn extract_contract_conditional_requires_unique_vector() {
    // One allocation balances two consumed non-scalar parameters; the scalar
    // middle parameter is excluded from the conditional uniqueness mask.
    let func = ArcFunction {
        name: name(1),
        params: vec![
            ArcParam {
                var: var(0),
                ty: ty(0),
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: var(1),
                ty: ty(1), // scalar
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: var(2),
                ty: ty(0),
                ownership: Ownership::Owned,
            },
        ],
        return_type: ty(0),
        var_types: vec![ty(0), ty(1), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(3),
                ty: ty(0),
                ctor: CtorKind::Struct(name(10)),
                args: vec![],
            }],
            terminator: ArcTerminator::Return { value: var(3) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(4).with_scalar(1);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &func,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &ori_ir::StringInterner::new(),
    );

    // Token balanced: 1 construct, 2 consumed non-scalar params (surplus).
    // requires_unique_params: [true(x), false(y=scalar), true(z)].
    if let FipContract::Conditional {
        requires_unique_params,
    } = &contract.fip
    {
        assert_eq!(requires_unique_params.len(), 3);
        assert!(requires_unique_params[0], "x should require uniqueness");
        assert!(
            !requires_unique_params[1],
            "y (scalar) should not require uniqueness"
        );
        assert!(requires_unique_params[2], "z should require uniqueness");
    } else {
        panic!("expected Conditional, got {:?}", contract.fip);
    }
}

// ContextBehavior interprocedural inference

#[test]
fn extract_contract_no_trmc_has_default_context_behavior() {
    // fn f(x: T) -> T { return x }
    // No TRMC candidate → default ContextBehavior.
    let func = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &func,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[], // no context regions
        &ori_ir::StringInterner::new(),
    );

    let cb = contract.context_behavior;
    assert!(!cb.preserves_context, "no TRMC → no preservation");
    assert!(!cb.consumes_hole, "no TRMC → no hole consumption");
    assert!(cb.requires_unique_context, "default requires uniqueness");
    assert!(!cb.may_resume_nonlinearly, "pure function → no non-linear");
}

#[test]
fn extract_contract_with_trmc_computes_context_behavior() {
    // Simulate TRMC by placing a recursive call result into field 1 of a
    // returned constructor context.
    let func = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0), ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                // v1 = head (scalar for simplicity)
                ArcInstr::Let {
                    dst: var(1),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(0)),
                },
                // v2 = recursive call to self
                ArcInstr::Apply {
                    dst: var(2),
                    ty: ty(0),
                    func: name(1),
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                // v3 = Construct(Cons, [v1, v2]) — context: v2 fills hole at field 1
                ArcInstr::Construct {
                    dst: var(3),
                    ty: ty(0),
                    ctor: CtorKind::Struct(name(10)),
                    args: vec![var(1), var(2)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(3) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(4);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    // Provide a ContextRegion that matches the TRMC pattern.
    let regions = vec![ContextRegion {
        open_block: block_id(0),
        open_instr: 2, // Construct at index 2
        context_var: var(3),
        hole_field: 1,
        close_block: block_id(0),
        close_instr: 1, // Apply at index 1
        hole_var: var(2),
    }];

    let contract = extract_contract(
        &func,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &regions,
        &ori_ir::StringInterner::new(),
    );

    let cb = contract.context_behavior;
    // Context var (v3) is returned → preserves context.
    assert!(cb.preserves_context, "context var returned → preserves");
    // Hole is filled by recursive call → consumes hole.
    assert!(cb.consumes_hole, "TRMC region → consumes hole");
    // Modulo-cons always requires uniqueness.
    assert!(cb.requires_unique_context, "modulo-cons → requires unique");
    // TRMC functions return a Construct → HeapEscaping → may_share = true,
    // so the HeapEscaping → may_share rule makes every TRMC candidate
    // trigger may_resume_nonlinearly.
    assert!(
        cb.may_resume_nonlinearly,
        "HeapEscaping return → may_share → non-linear"
    );
}

// Payload containment (find_payload_containment_params)

/// `@wrap_ok (m: T) -> Result<T, E> = Ok(m)` — the single return wraps the
/// param, so the containment bit publishes.
#[test]
fn extract_contract_single_return_wrap_publishes_containment() {
    let func = ArcFunction {
        name: name(30),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Borrowed,
        }],
        return_type: ty(1),
        var_types: vec![ty(0), ty(1)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(1),
                ty: ty(1),
                ctor: CtorKind::EnumVariant {
                    enum_name: Name::from_raw(20),
                    variant: 0,
                },
                args: vec![var(0)],
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &func,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &ori_ir::StringInterner::new(),
    );

    let p = &contract.params[0];
    assert!(p.return_payload_contains_param, "wrapped on some path");
}

/// `@maybe_wrap (m: T) -> Result<T, E> = if c then Ok(m) else Err(0)` — the
/// OR containment bit publishes even when only one return path wraps.
#[test]
fn extract_contract_branchy_wrap_publishes_containment() {
    let func = ArcFunction {
        name: name(31),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Borrowed,
        }],
        return_type: ty(1),
        var_types: vec![ty(0), ty(1), ty(1), ty(2)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: var(3),
                    ty: ty(2),
                    value: ArcValue::Literal(LitValue::Bool(true)),
                }],
                terminator: ArcTerminator::Branch {
                    cond: var(3),
                    then_block: block_id(1),
                    else_block: block_id(2),
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![ArcInstr::Construct {
                    dst: var(1),
                    ty: ty(1),
                    ctor: CtorKind::EnumVariant {
                        enum_name: Name::from_raw(20),
                        variant: 0,
                    },
                    args: vec![var(0)],
                }],
                terminator: ArcTerminator::Return { value: var(1) },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(1),
                    ctor: CtorKind::EnumVariant {
                        enum_name: Name::from_raw(20),
                        variant: 1,
                    },
                    args: vec![],
                }],
                terminator: ArcTerminator::Return { value: var(2) },
            },
        ],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(4).with_scalar(3);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &func,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &ori_ir::StringInterner::new(),
    );

    let p = &contract.params[0];
    assert!(p.return_payload_contains_param, "wrapped on SOME path");
}

fn loop_threaded_push_rebuild_contract() -> MemoryContract {
    // INVARIANT: A loop parameter fed only by a fresh seed and its COW rebuild
    // remains in the fresh-allocation fixed point.
    let interner = ori_ir::StringInterner::new();
    let push = interner.intern("push");
    let func = ArcFunction {
        name: interner.intern("loop_threaded_push_rebuild"),
        params: vec![],
        return_type: ty(0),
        var_types: vec![ty(0), ty(0), ty(0), ty(0), ty(1), ty(1)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![
                    ArcInstr::Construct {
                        dst: var(0),
                        ty: ty(0),
                        ctor: CtorKind::ListLiteral,
                        args: vec![],
                    },
                    ArcInstr::Let {
                        dst: var(4),
                        ty: ty(1),
                        value: ArcValue::Literal(LitValue::Int(1)),
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: block_id(1),
                    args: vec![var(0)],
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![(var(1), ty(0))],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: var(4),
                    then_block: block_id(2),
                    else_block: block_id(3),
                },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![
                    ArcInstr::Let {
                        dst: var(5),
                        ty: ty(1),
                        value: ArcValue::Literal(LitValue::Int(7)),
                    },
                    ArcInstr::Apply {
                        dst: var(2),
                        ty: ty(0),
                        func: push,
                        args: vec![var(1), var(5)],
                        arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Owned],
                        mono_instance_id: None,
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: block_id(1),
                    args: vec![var(2)],
                },
            },
            ArcBlock {
                id: block_id(3),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(1) },
            },
        ],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(6).with_scalar(4).with_scalar(5);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    extract_contract(
        &func,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &interner,
    )
}

#[test]
fn loop_threaded_push_rebuild_return_certifies_fresh_self_alloc() {
    let contract = loop_threaded_push_rebuild_contract();

    assert!(
        contract.return_info.returns_fresh_self_alloc,
        "loop-threaded push rebuild of a fresh list is a fresh self-alloc return"
    );
}

crate::test_helpers::ablation_env_event_test!(
    fresh_lineage_return_trace_reproduces_conservative_return_contract,
    "ORI_DISABLE_FRESH_LINEAGE_RETURN_TRACE",
    "decline loop-threaded fresh-lineage return certification",
    || {
        let contract = loop_threaded_push_rebuild_contract();
        assert!(
            !contract.return_info.returns_fresh_self_alloc,
            "the ablation must decline the loop-threaded freshness proof"
        );
        true
    },
);

#[test]
fn loop_threaded_param_rooted_return_stays_conservative() {
    // Threading a caller-visible parameter rather than a fresh allocation into
    // the same cycle must evict the lineage and leave the return uncertified.
    let interner = ori_ir::StringInterner::new();
    let push = interner.intern("push");
    let func = ArcFunction {
        name: name(91),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0), ty(0), ty(0), ty(0), ty(1), ty(1)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: var(4),
                    ty: ty(1),
                    value: ArcValue::Literal(LitValue::Int(1)),
                }],
                terminator: ArcTerminator::Jump {
                    target: block_id(1),
                    args: vec![var(0)],
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![(var(1), ty(0))],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: var(4),
                    then_block: block_id(2),
                    else_block: block_id(3),
                },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![
                    ArcInstr::Let {
                        dst: var(5),
                        ty: ty(1),
                        value: ArcValue::Literal(LitValue::Int(7)),
                    },
                    ArcInstr::Apply {
                        dst: var(2),
                        ty: ty(0),
                        func: push,
                        args: vec![var(1), var(5)],
                        arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Owned],
                        mono_instance_id: None,
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: block_id(1),
                    args: vec![var(2)],
                },
            },
            ArcBlock {
                id: block_id(3),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(1) },
            },
        ],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(6).with_scalar(4).with_scalar(5);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(
        &func,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &interner,
    );

    assert!(
        !contract.return_info.returns_fresh_self_alloc,
        "a param-rooted threaded return is caller-visible, never a fresh self-alloc"
    );
}
