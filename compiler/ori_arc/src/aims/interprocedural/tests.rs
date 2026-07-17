//! Tests for interprocedural AIMS analysis.

use ori_ir::{BinaryOp, Name};
use ori_registry::{OpStrategy, PrimitiveAllocationEffect, RuntimeOperator};
use ori_types::Idx;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::{ContextRegion, FipContract, MemoryAccessClass, MemoryContract};
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
    // fn wrapper(words: [str]) -> int { iterate_words(words) }
    // where iterate_words.iter_consumes[0] == true (computed callee-first per
    // IC-1). The wrapper forwards its param to the iter-consuming callee, so the
    // wrapper's param contract MUST inherit `iter_consumes` (RL-2 transitive
    // inward transfer). A borrow-read callee (iter_consumes=false) would NOT
    // propagate.
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
fn extract_contract_project_return_alias_propagates_through_forwarder() {
    // fn unbox(b: Box<[int]>) -> [int] = unwrap(b)
    // where unwrap.params[0].return_alias == Project { field: 0 } (the callee
    // returns `b.value`, a borrow-view of param field 0). The forwarder returns
    // that borrow-view UNCHANGED via an Invoke terminator (the real @unbox
    // shape), so the forwarder's param b MUST inherit Project { field: 0 } —
    // forwarder-transitivity of the same-allocation-identity relation (proven
    // net-0 single-release: scratch ForwardedProjectReturn
    // forwarded_joint_release_exactly_once; governing RL-2 RL2_release_exactly_once
    // + TF-4 borrow-view). Without it, @main drops the Box before the projected
    // list's last use — the BUG-floor box_list_int_unwrap UAF.
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
    // fn f(xs: [int]) -> [int] = { let w = Wrap { items: xs }; w.items }
    // The param is moved into a struct Construct then projected back out as the
    // Return value. `Project (Construct args) field == args[field]` (TF-3 + TF-4),
    // so the return ALIASES the param — Direct. The construct-project round-trip
    // resolver must record Direct + transfers_through_return (the caller defers
    // its premature param drop; the in-callee container suppression defers the
    // callee's). Proven net-0:
    // `ConstructProjectRoundtrip.cure_restores_balance`.
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
    // fn fwd(xs: [int]) -> int { read_only(xs) }  where read_only's param is
    // Borrowed AND borrowed_read_only (a pure borrow-read callee). The param flows
    // ONLY to a borrowed, read-only-forwarded position → fwd's param is
    // `borrowed_read_only` (the caller carve-out may un-exclude a fresh-local
    // collection passed here). Spec: Annex E §AIMS RL-2.
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
    let contract = extract_contract_with_call_ownership(
        &func,
        &state_map,
        &classifier,
        &sigs,
        &FxHashSet::default(),
        &[],
        &interner,
        &builtins,
        &exact_callables,
    );

    assert!(
        !contract.params[0].may_share,
        "an exact local callable must not inherit same-spelled builtin ownership"
    );
}

#[test]
fn borrowed_read_only_false_for_param_at_owned_consumer_position() {
    // fn fwd(xs: [int]) -> int { consume(xs) }  where consume's param is Owned
    // (a COW-mutating / transferring callee — the Owned analogue of `xs.push(v)`).
    // The param flows to an OWNED position → fwd's param is NOT borrowed_read_only,
    // so the caller carve-out keeps a collection passed here EXCLUDED (un-excluding
    // a COW-shared buffer would double-free). This is the load-bearing over-fire
    // guard. Spec: Annex E §AIMS RL-2 (a COW-mutated param is NOT
    // ApplyToBorrowedParam).
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
    // callee: fn g(x: T) -> T { return x }  — pure, no alloc, no share
    // caller: fn f(a: T) -> T { let r = g(a); return r }
    //
    // Since callee is pure (may_share=false), borrowed args preserve uniqueness.
    // The caller should pass `a` as Borrowed (callee only uses once) and the
    // callee's contract should have may_share=false.
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
    // Two mutually recursive functions:
    // fn a(x: T) -> T { let r = b(x); return r }
    // fn b(x: T) -> T { let r = a(x); return r }
    //
    // Neither allocates, neither shares — effects should converge to
    // may_allocate=false, may_share=false through the SCC fixpoint.
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
    // callee(p0: T) -> T: { return p0 }
    // caller(): { v0 = Construct; v1 = callee(v0); return v1 }
    //
    // caller passes a freshly constructed value (Owned, Linear, Once)
    // to callee's param 0. Since this is the ONLY caller, the all-callers
    // condition is satisfied → callee.params[0].uniqueness should be Unique.
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
    // callee(p0: T) -> T: { return p0 }
    // caller_a(): { v0 = Construct; v1 = callee(v0); return v1 }
    // caller_b(): { v0 = Construct; v1 = callee(v0); return v1 }
    //
    // Both callers pass Owned+Linear+Once → callee param should be Unique.
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
    // callee(p0: T) -> T: { return p0 }
    // caller_good(): { v0 = Construct; v1 = callee(v0); return v1 }
    // caller_bad(p0: T): { v1 = callee(p0); v2 = callee(p0); return v2 }
    //
    // caller_bad passes p0 twice (cardinality=Many) → all-callers condition
    // NOT satisfied → callee param stays MaybeShared.
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

/// BUG-04-069: a forwarded Owned param is NOT provably single-owner. `Owned+Linear+Once`
/// proves no-future-duplication, NOT no-existing-alias (the removed-IC-8/DP-10
/// fallacy). The forwarded-param Case 2 use-count heuristic is dropped; only a
/// fresh `Construct` (one logical owner by TF-3) tightens. This is the negative soundness pin.
#[test]
fn demand_propagation_forwarded_param_stays_maybe_shared() {
    // callee(p0: T) -> T: { return p0 }
    // caller(p0: T) -> T: { v1 = callee(p0); return v1 }
    //
    // caller forwards its own parameter to callee. `p0` carries MaybeShared
    // (extract.rs param default); forwarding does NOT make it Unique. The
    // callee param MUST stay MaybeShared — tightening would be the DP-10 fallacy.
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

/// BUG-04-069: a SINGLE fresh-`Construct` variable passed to the SAME
/// (callee, `param_idx`) at >=2 call sites may have another owner at the non-first use. The
/// per-(callee,param) AND-fold must NOT tighten to Unique — the `count_var_uses
/// == 1` guard on Case 1 (fresh Construct used exactly once) keeps it
/// `MaybeShared`. Closes the DP-10-class hole on the Construct path that an
/// unguarded `construct_vars.contains` membership check would leave open.
#[test]
fn demand_propagation_construct_var_at_two_sites_stays_maybe_shared() {
    // callee(p0: T) -> T: { return p0 }
    // caller(): { v0 = Construct; v1 = callee(v0); v2 = callee(v0); return v2 }
    //
    // v0 starts with one fresh owner but is passed to the
    // SAME (callee, param 0) twice → count_var_uses(v0) == 2 → not Unique.
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
    // callee(p0: T) -> T: { return p0 }
    // caller(): { v0 = Construct; v1 = callee(v0); BurdenInc(v0); return v1 }
    //
    // v0 starts with one fresh owner but is used twice: once as the Apply
    // arg, once as a BurdenInc operand → count_var_uses(v0) == 2 → not Unique.
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
    // fn f(x: T, y: int, z: T) -> T { v3 = Construct; return v3 }
    // x (v0) is consumed non-scalar → requires unique.
    // y (v1) is scalar → excluded.
    // z (v2) is consumed non-scalar → requires unique.
    // 1 Construct, 2 consumed params → balanced → Conditional with [true, false, true].
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
    // Simulate a TRMC candidate: function builds Construct(T, [field0, rec_call_result])
    // where the rec call result fills field 1 (the "hole").
    //
    // fn map(xs: [T]) -> [T] {
    //   v1 = Construct(Cons, [head, map(tail)])  // context region
    //   return v1
    // }
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

// As-compiled impl-method contracts (compute_impl_method_contracts)

fn impl_method_with_int_primops(site_count: usize) -> ArcFunction {
    assert!((1..=2).contains(&site_count));
    let Ok(return_var) = u32::try_from(1 + site_count) else {
        panic!("primitive-site count must fit in an ARC variable identifier");
    };
    let mut body = vec![ArcInstr::Let {
        dst: var(2),
        ty: Idx::INT,
        value: ArcValue::PrimOp {
            op: PrimOp::Binary(BinaryOp::Add),
            args: vec![var(0), var(1)],
        },
    }];
    if site_count == 2 {
        body.push(ArcInstr::Let {
            dst: var(3),
            ty: Idx::INT,
            value: ArcValue::PrimOp {
                op: PrimOp::Binary(BinaryOp::Add),
                args: vec![var(2), var(1)],
            },
        });
    }
    ArcFunction {
        name: name(12),
        params: vec![
            ArcParam {
                var: var(0),
                ty: Idx::INT,
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: var(1),
                ty: Idx::INT,
                ownership: Ownership::Owned,
            },
        ],
        return_type: Idx::INT,
        var_types: vec![Idx::INT; 2 + site_count],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body,
            terminator: ArcTerminator::Return {
                value: var(return_var),
            },
        }],
        ..ArcFunction::default()
    }
}

#[test]
fn impl_method_contract_freezes_builtin_primitive_facts_before_analysis() {
    let pool = ori_types::Pool::new();
    let classifier = crate::ArcClassifier::new(&pool);
    let interner = ori_ir::StringInterner::new();
    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    let mut func = impl_method_with_int_primops(1);

    let Ok(_contracts) = compute_impl_method_contracts(
        std::slice::from_mut(&mut func),
        &classifier,
        &builtins,
        &interner,
    ) else {
        panic!("the AIMS input seam should freeze the int-add descriptor");
    };

    assert_eq!(
        func.primitive_facts.get(var(2)).map(|fact| fact.strategy),
        Some(OpStrategy::SignedInteger)
    );
}

#[test]
fn impl_method_contract_rejects_malformed_frozen_primitive_fact_without_repair() {
    let pool = ori_types::Pool::new();
    let classifier = crate::ArcClassifier::new(&pool);
    let interner = ori_ir::StringInterner::new();
    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    let mut func = impl_method_with_int_primops(1);
    let Some(malformed) =
        PrimitiveFact::resolve(OpStrategy::RuntimeCall(RuntimeOperator::ListConcat), 2)
    else {
        panic!("list concat should have a binary descriptor");
    };
    assert!(func.primitive_facts.insert(var(2), malformed).is_none());

    let Err(errors) = compute_impl_method_contracts(
        std::slice::from_mut(&mut func),
        &classifier,
        &builtins,
        &interner,
    ) else {
        panic!("a frozen descriptor cannot be recomputed or repaired");
    };

    assert!(matches!(
        errors.as_slice(),
        [crate::verify::VerifyError::PrimitiveFactInvalid { dst, .. }] if *dst == var(2)
    ));
    assert_eq!(func.primitive_facts.get(var(2)), Some(malformed));
}

#[test]
fn impl_method_contract_rejects_partial_frozen_primitive_facts_without_repair() {
    let pool = ori_types::Pool::new();
    let classifier = crate::ArcClassifier::new(&pool);
    let interner = ori_ir::StringInterner::new();
    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    let mut func = impl_method_with_int_primops(2);
    let Some(frozen) = PrimitiveFact::resolve(OpStrategy::SignedInteger, 2) else {
        panic!("integer addition should have a descriptor");
    };
    assert!(func.primitive_facts.insert(var(2), frozen).is_none());

    let Err(errors) = compute_impl_method_contracts(
        std::slice::from_mut(&mut func),
        &classifier,
        &builtins,
        &interner,
    ) else {
        panic!("a non-empty fact table must validate exactly");
    };

    assert!(errors.iter().any(|error| matches!(
        error,
        crate::verify::VerifyError::PrimitiveFactInvalid { dst, .. } if *dst == var(3)
    )));
    assert_eq!(func.primitive_facts.iter().count(), 1);
    assert_eq!(func.primitive_facts.get(var(2)), Some(frozen));
    assert_eq!(func.primitive_facts.get(var(3)), None);
}

/// Forwarder `f(x: ref) -> ref = x` — the structural Direct return-flow pair
/// is published; every other dimension stays at the conservative default
/// (the contract describes the method AS COMPILED on the immediate-emit
/// path, which never applies its own contract).
#[test]
fn impl_method_contract_forwarder_publishes_direct_ttr_pair_only() {
    let mut func = ArcFunction {
        name: name(10),
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
    let interner = ori_ir::StringInterner::new();
    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    let Ok(out) = compute_impl_method_contracts(
        std::slice::from_mut(&mut func),
        &classifier,
        &builtins,
        &interner,
    ) else {
        panic!("impl-method facts should resolve");
    };

    let contract = &out[&name(10)];
    let p = &contract.params[0];
    assert!(p.transfers_through_return, "Direct return-flow published");
    assert_eq!(
        p.return_alias,
        Some(crate::aims::contract::ReturnAliasShape::Direct)
    );

    // Every other field matches the conservative default.
    let conservative = MemoryContract::conservative(1);
    let cp = &conservative.params[0];
    assert_eq!(p.access, cp.access, "access stays conservative (Owned)");
    assert_eq!(p.consumption, cp.consumption);
    assert_eq!(p.cardinality, cp.cardinality);
    assert_eq!(p.uniqueness, cp.uniqueness);
    assert_eq!(p.iter_consumes, cp.iter_consumes);
    assert_eq!(p.borrowed_read_only, cp.borrowed_read_only);
    assert_eq!(
        p.return_payload_contains_param,
        cp.return_payload_contains_param
    );
    assert_eq!(contract.return_info, conservative.return_info);
    assert_eq!(contract.effects, conservative.effects);
    assert_eq!(contract.fip, conservative.fip);
}

/// Non-forwarder (returns a literal, param read-only) — the published
/// contract is FULLY conservative: no ttr, no return alias.
#[test]
fn impl_method_contract_non_forwarder_is_fully_conservative() {
    let mut func = ArcFunction {
        name: name(11),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(1),
        var_types: vec![ty(0), ty(1)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: var(1),
                ty: ty(1),
                value: ArcValue::Literal(LitValue::Int(7)),
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2).with_scalar(1);
    let interner = ori_ir::StringInterner::new();
    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    let Ok(out) = compute_impl_method_contracts(
        std::slice::from_mut(&mut func),
        &classifier,
        &builtins,
        &interner,
    ) else {
        panic!("impl-method facts should resolve");
    };

    let contract = &out[&name(11)];
    assert!(!contract.params[0].transfers_through_return);
    assert_eq!(contract.params[0].return_alias, None);
}

// Per-function caller-side binding (augment_contracts_with_impl_callees)

fn forwarder_contract_with_ttr() -> MemoryContract {
    let mut c = MemoryContract::conservative(1);
    c.params[0].transfers_through_return = true;
    c.params[0].return_alias = Some(crate::aims::contract::ReturnAliasShape::Direct);
    c
}

/// Caller with one `Apply @m(%0)` whose receiver type matches the impl key
/// binds the bare name to the impl-method contract.
fn caller_with_apply(
    caller_name: Name,
    callee: Name,
    recv_ty: Idx,
    args: Vec<ArcVarId>,
) -> ArcFunction {
    ArcFunction {
        name: caller_name,
        var_types: vec![recv_ty, recv_ty],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(1),
                ty: recv_ty,
                func: callee,
                args,
                arg_ownership: Vec::new(),
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    }
}

#[test]
fn augment_binds_receiver_resolved_impl_callee() {
    let pool = ori_types::Pool::default();
    let recv = Idx::STR;
    let callee = name(20);
    let func = caller_with_apply(name(21), callee, recv, vec![var(0)]);

    let base: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let mut impl_contracts = FxHashMap::default();
    impl_contracts.insert((recv, callee), forwarder_contract_with_ttr());

    let augmented = augment_contracts_with_impl_callees(&func, &base, &impl_contracts, &pool)
        .unwrap_or_else(|| panic!("binding fires"));
    assert!(augmented[&callee].params[0].transfers_through_return);
}

#[test]
fn augment_declines_when_base_already_has_name() {
    let pool = ori_types::Pool::default();
    let recv = Idx::STR;
    let callee = name(20);
    let func = caller_with_apply(name(21), callee, recv, vec![var(0)]);

    let mut base: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    base.insert(callee, MemoryContract::conservative(1));
    let mut impl_contracts = FxHashMap::default();
    impl_contracts.insert((recv, callee), forwarder_contract_with_ttr());

    assert!(
        augment_contracts_with_impl_callees(&func, &base, &impl_contracts, &pool).is_none(),
        "free-function / seeded name keeps its existing contract"
    );
}

#[test]
fn augment_declines_self_recursive_name() {
    let pool = ori_types::Pool::default();
    let recv = Idx::STR;
    let callee = name(22);
    // Caller IS the callee (self-recursive impl method shape).
    let func = caller_with_apply(callee, callee, recv, vec![var(0)]);

    let base: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let mut impl_contracts = FxHashMap::default();
    impl_contracts.insert((recv, callee), forwarder_contract_with_ttr());

    assert!(augment_contracts_with_impl_callees(&func, &base, &impl_contracts, &pool).is_none());
}

#[test]
fn augment_declines_receiver_type_mismatch() {
    let pool = ori_types::Pool::default();
    let callee = name(20);
    // Call-site receiver is INT; the impl key is STR — ambiguous name.
    let func = caller_with_apply(name(21), callee, Idx::INT, vec![var(0)]);

    let base: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let mut impl_contracts = FxHashMap::default();
    impl_contracts.insert((Idx::STR, callee), forwarder_contract_with_ttr());

    assert!(augment_contracts_with_impl_callees(&func, &base, &impl_contracts, &pool).is_none());
}

#[test]
fn augment_declines_no_receiver_args() {
    let pool = ori_types::Pool::default();
    let callee = name(20);
    let func = caller_with_apply(name(21), callee, Idx::STR, vec![]);

    let base: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let mut impl_contracts = FxHashMap::default();
    impl_contracts.insert((Idx::STR, callee), forwarder_contract_with_ttr());

    assert!(augment_contracts_with_impl_callees(&func, &base, &impl_contracts, &pool).is_none());
}

#[test]
fn augment_declines_divergent_receivers_same_name() {
    let pool = ori_types::Pool::default();
    let callee = name(20);
    // Two sites, receivers STR and INT, BOTH resolving impl keys — one
    // bare name cannot carry two contracts; the name is poisoned.
    let mut func = caller_with_apply(name(21), callee, Idx::STR, vec![var(0)]);
    func.var_types.push(Idx::INT);
    func.var_types.push(Idx::INT);
    func.blocks[0].body.push(ArcInstr::Apply {
        dst: var(3),
        ty: Idx::INT,
        func: callee,
        args: vec![var(2)],
        arg_ownership: Vec::new(),
        mono_instance_id: None,
    });

    let base: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let mut impl_contracts = FxHashMap::default();
    impl_contracts.insert((Idx::STR, callee), forwarder_contract_with_ttr());
    impl_contracts.insert((Idx::INT, callee), forwarder_contract_with_ttr());

    assert!(augment_contracts_with_impl_callees(&func, &base, &impl_contracts, &pool).is_none());
}

#[test]
fn augment_no_op_on_empty_impl_contracts() {
    let pool = ori_types::Pool::default();
    let func = caller_with_apply(name(21), name(20), Idx::STR, vec![var(0)]);
    let base: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let impl_contracts: FxHashMap<(Idx, Name), MemoryContract> = FxHashMap::default();

    assert!(augment_contracts_with_impl_callees(&func, &base, &impl_contracts, &pool).is_none());
}

// Own-contract binding (augment_contracts_with_impl_callees — own-name entry)

/// An impl method compiling ITSELF (receiver-resolved by its own self param,
/// no call sites carrying its own name) binds its as-compiled contract as its
/// OWN entry, so Phase-5's own-contract consumers (RL-2 transfer-source-dec
/// strip, RL-1/RL-34 forwarder-identity alias transparency) see the same
/// structural ttr pair its callers see.
#[test]
fn augment_binds_own_contract_for_impl_method_self_compilation() {
    let pool = ori_types::Pool::default();
    let recv = Idx::STR;
    let method = name(30);
    let func = ArcFunction {
        name: method,
        params: vec![crate::ir::ArcParam {
            var: var(0),
            ty: recv,
            ownership: crate::ownership::Ownership::Owned,
        }],
        var_types: vec![recv],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let base: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let mut impl_contracts = FxHashMap::default();
    impl_contracts.insert((recv, method), forwarder_contract_with_ttr());

    let augmented = augment_contracts_with_impl_callees(&func, &base, &impl_contracts, &pool)
        .unwrap_or_else(|| panic!("own-contract binding fires"));
    assert!(
        augmented[&method].params[0].transfers_through_return,
        "the method's own entry MUST carry the structural ttr pair",
    );
}

/// Own-binding declines when the method's self-param receiver type matches no
/// impl-contract key (a different type's same-named method or a free shape).
#[test]
fn augment_own_contract_declines_on_receiver_key_miss() {
    let pool = ori_types::Pool::default();
    let method = name(31);
    let func = ArcFunction {
        name: method,
        params: vec![crate::ir::ArcParam {
            var: var(0),
            ty: Idx::INT,
            ownership: crate::ownership::Ownership::Owned,
        }],
        var_types: vec![Idx::INT],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let base: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let mut impl_contracts = FxHashMap::default();
    impl_contracts.insert((Idx::STR, method), forwarder_contract_with_ttr());

    assert!(
        augment_contracts_with_impl_callees(&func, &base, &impl_contracts, &pool).is_none(),
        "a receiver key miss MUST decline the own-contract binding",
    );
}

/// Own-binding declines when ANY call site in the function carries the
/// function's own bare name (the receiver could resolve to a DIFFERENT
/// type's same-named method — conservative status quo).
#[test]
fn augment_own_contract_declines_when_own_name_called_in_body() {
    let pool = ori_types::Pool::default();
    let recv = Idx::STR;
    let method = name(32);
    let mut func = caller_with_apply(method, method, recv, vec![var(0)]);
    func.params = vec![crate::ir::ArcParam {
        var: var(0),
        ty: recv,
        ownership: crate::ownership::Ownership::Owned,
    }];

    let base: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let mut impl_contracts = FxHashMap::default();
    impl_contracts.insert((recv, method), forwarder_contract_with_ttr());

    assert!(
        augment_contracts_with_impl_callees(&func, &base, &impl_contracts, &pool).is_none(),
        "an own-name call site MUST decline the own-contract binding",
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

#[test]
fn loop_threaded_push_rebuild_return_certifies_fresh_self_alloc() {
    // fn f() -> [T] {
    //   bb0: %0 = Construct List(); %4 = 1 (cond); Jump bb1(%0)
    //   bb1(%1): Branch %4 ? bb2 : bb3
    //   bb2: %2 = Apply push(%1, %5); %5 = lit; Jump bb1(%2)   [backedge]
    //   bb3: Return %1
    // }
    // The loop-threaded rebuild: %1's feeders are {%0 fresh, %2 = push(%1)} —
    // the greatest-fixpoint fresh-lineage trace keeps the self-consistent
    // cycle, so the threaded return certifies `returns_fresh_self_alloc`.
    let interner = ori_ir::StringInterner::new();
    let push = interner.intern("push");
    let func = ArcFunction {
        name: name(90),
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
        contract.return_info.returns_fresh_self_alloc,
        "loop-threaded push rebuild of a fresh list is a fresh self-alloc return"
    );
}

#[test]
fn loop_threaded_param_rooted_return_stays_conservative() {
    // Same CFG shape, but bb0 threads the function PARAM (caller-visible)
    // instead of a fresh Construct — the fresh-lineage trace must evict the
    // whole cycle (one non-member external feeder) and the return stays
    // uncertified.
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
