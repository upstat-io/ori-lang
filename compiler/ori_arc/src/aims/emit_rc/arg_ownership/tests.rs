//! AIMS entry-point tests for call-site ownership annotation.

use ori_ir::StringInterner;
use ori_types::{Idx, Pool};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::{MemoryContract, ParamContract};
use crate::aims::lattice::{AccessClass, Cardinality, Consumption, Locality, Uniqueness};
use crate::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership};
use crate::test_helpers::make_func_named;
use crate::BuiltinOwnershipSets;

fn make_param_contract(access: AccessClass) -> ParamContract {
    ParamContract {
        access,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        may_escape: false,
        may_share: false,
        locality_bound: Locality::FunctionLocal,
        uniqueness: Uniqueness::MaybeShared,
        transfers_through_return: false,
        return_alias: None,
        return_payload_contains_param: false,
        iter_consumes: false,
        borrowed_read_only: false,
        borrowed_cow_consumed: false,
        borrowed_cow_mutated: false,
        iter_consumes_projected_field: None,
    }
}

#[test]
fn direct_call_reuses_monomorphized_contract_merge() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = BuiltinOwnershipSets::empty();

    let target_name = interner.intern("apply");
    let mono_name = interner.intern("apply$m$Lint");
    let func_name = interner.intern("caller");

    // Only the monomorphized contract exists. Direct call annotation resolves
    // the original name through the conservative monomorphized merge.
    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![ArcInstr::Apply {
            dst: ArcVarId::new(1),
            ty: Idx::INT,
            func: target_name,
            args: vec![ArcVarId::new(0)],
            arg_ownership: vec![],
            mono_instance_id: None,
        }],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(1),
        },
    }];

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 2]);

    let mut contracts = FxHashMap::default();
    let mut mono_contract = MemoryContract::conservative(1);
    mono_contract.params[0] = make_param_contract(AccessClass::Owned);
    contracts.insert(mono_name, mono_contract);

    let Ok(()) = super::emit_arg_ownership(
        &mut func,
        &contracts,
        &interner,
        &builtins,
        &pool,
        &FxHashSet::default(),
    ) else {
        panic!("direct call ownership annotation should be total");
    };

    if let ArcInstr::Apply { arg_ownership, .. } = &func.blocks[0].body[0] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Owned],
            "the monomorphized contract should merge under the direct target name"
        );
    } else {
        panic!("expected Apply");
    }
}

#[test]
fn indirect_call_keeps_uniform_borrowed_abi() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = BuiltinOwnershipSets::empty();

    let target_name = interner.intern("apply");
    let mono_name = interner.intern("apply$m$Lint");
    let func_name = interner.intern("caller");

    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![
            ArcInstr::PartialApply {
                dst: ArcVarId::new(0),
                ty: Idx::NONE,
                func: target_name,
                args: vec![],
            },
            ArcInstr::ApplyIndirect {
                dst: ArcVarId::new(2),
                ty: Idx::INT,
                closure: ArcVarId::new(0),
                args: vec![ArcVarId::new(1)],
                arg_ownership: vec![],
            },
        ],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(2),
        },
    }];

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 3]);
    let mut contracts = FxHashMap::default();
    let mut contract = MemoryContract::conservative(1);
    contract.params[0] = make_param_contract(AccessClass::Owned);
    contracts.insert(mono_name, contract);

    let Ok(()) = super::emit_arg_ownership(
        &mut func,
        &contracts,
        &interner,
        &builtins,
        &pool,
        &FxHashSet::default(),
    ) else {
        panic!("indirect call ownership annotation should be total");
    };

    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[0].body[1] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Borrowed],
            "indirect explicit arguments use the target-independent borrowed ABI"
        );
    } else {
        panic!("expected ApplyIndirect");
    }
}

#[test]
fn total_ownership_gate_rejects_unannotated_indirect_args() {
    let interner = StringInterner::new();
    let caller = interner.intern("caller");
    let target = interner.intern("target");
    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![
            ArcInstr::PartialApply {
                dst: ArcVarId::new(0),
                ty: Idx::NONE,
                func: target,
                args: vec![],
            },
            ArcInstr::ApplyIndirect {
                dst: ArcVarId::new(2),
                ty: Idx::INT,
                closure: ArcVarId::new(0),
                args: vec![ArcVarId::new(1)],
                arg_ownership: vec![],
            },
        ],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(2),
        },
    }];
    let func = make_func_named(caller, vec![], Idx::NONE, blocks, vec![Idx::INT; 3]);

    assert_eq!(
        crate::verify::check_total_arg_ownership(&func),
        vec![crate::verify::VerifyError::ArgOwnershipLenMismatch {
            block: ArcBlockId::new(0),
            expected: 1,
            actual: 0,
        }]
    );
}

#[test]
fn exact_external_alias_named_like_builtin_keeps_producer_contract() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let builtins = BuiltinOwnershipSets::new(&interner);
    let push = interner.intern("push");
    let caller = interner.intern("caller");
    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![ArcInstr::Apply {
            dst: ArcVarId::new(1),
            ty: list_int,
            func: push,
            args: vec![ArcVarId::new(0)],
            arg_ownership: vec![ArgOwnership::Owned],
            mono_instance_id: None,
        }],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(1),
        },
    }];
    let mut function = make_func_named(caller, vec![], list_int, blocks, vec![list_int; 2]);
    let mut contract = MemoryContract::conservative(1);
    contract.params[0] = make_param_contract(AccessClass::Borrowed);
    let contracts = [(push, contract)].into_iter().collect();
    let exact_callables: FxHashSet<_> = [push].into_iter().collect();

    let Ok(()) = super::emit_arg_ownership(
        &mut function,
        &contracts,
        &interner,
        &builtins,
        &pool,
        &exact_callables,
    ) else {
        panic!("direct call ownership annotation should be total");
    };

    let ArcInstr::Apply { arg_ownership, .. } = &function.blocks[0].body[0] else {
        panic!("expected Apply");
    };
    assert_eq!(
        arg_ownership,
        &[ArgOwnership::Borrowed],
        "producer-owned callable facts must outrank the builtin push heuristic"
    );
}
