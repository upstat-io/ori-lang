//! AIMS entry-point tests for `emit_arg_ownership()` with indirect calls.

use ori_ir::{Name, StringInterner};
use ori_types::{Idx, Pool};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::{MemoryContract, ParamContract};
use crate::aims::lattice::{AccessClass, Cardinality, Consumption, Locality, Uniqueness};
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, ArgOwnership,
};
use crate::BuiltinOwnershipSets;

fn empty_builtins() -> BuiltinOwnershipSets {
    BuiltinOwnershipSets {
        borrowing: FxHashSet::default(),
        consuming_receiver: FxHashSet::default(),
        consuming_second_arg: FxHashSet::default(),
        consuming_receiver_only: FxHashSet::default(),
        protocol: FxHashMap::default(),
    }
}

fn make_func(
    name: Name,
    params: Vec<ArcParam>,
    blocks: Vec<ArcBlock>,
    var_count: usize,
) -> ArcFunction {
    ArcFunction {
        name,
        params,
        return_type: Idx::NONE,
        blocks,
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::INT; var_count],
        var_reprs: vec![],
        spans: vec![],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: crate::uniqueness::CowAnnotations::default(),
        drop_hints: crate::uniqueness::DropHints::default(),
        tail_calls: vec![],
    }
}

fn make_param_contract(access: AccessClass) -> ParamContract {
    ParamContract {
        access,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        may_escape: false,
        may_share: false,
        locality_bound: Locality::FunctionLocal,
        uniqueness: Uniqueness::MaybeShared,
    }
}

#[test]
fn test_emit_arg_ownership_indirect_reuses_monomorphized_merge() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = empty_builtins();

    let target_name = interner.intern("apply");
    let mono_name = interner.intern("apply$m$Lint");
    let func_name = interner.intern("caller");

    // Only monomorphized contract exists (no bare "apply" contract).
    // emit_arg_ownership merges mono → original name, so indirect
    // call should find it under "apply".
    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![
            ArcInstr::PartialApply {
                dst: ArcVarId::new(0),
                ty: Idx::NONE,
                func: target_name,
                args: vec![], // 0 captures
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

    let mut func = make_func(func_name, vec![], blocks, 3);

    let mut contracts = FxHashMap::default();
    let mut mono_contract = MemoryContract::conservative(1);
    mono_contract.params[0] = make_param_contract(AccessClass::Owned);
    contracts.insert(mono_name, mono_contract);

    super::emit_arg_ownership(&mut func, &contracts, &interner, &builtins, &pool);

    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[0].body[1] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Owned],
            "monomorphized name should merge to base name for indirect lookup"
        );
    } else {
        panic!("expected ApplyIndirect");
    }
}

#[test]
fn test_emit_arg_ownership_indirect_debug_assert_invariant() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = empty_builtins();

    let target_name = interner.intern("func");
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

    let mut func = make_func(func_name, vec![], blocks, 3);
    let mut contracts = FxHashMap::default();
    let mut contract = MemoryContract::conservative(1);
    contract.params[0] = make_param_contract(AccessClass::Borrowed);
    contracts.insert(target_name, contract);

    // This should not panic from the debug_assert — non-empty args get non-empty ownership.
    super::emit_arg_ownership(&mut func, &contracts, &interner, &builtins, &pool);

    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[0].body[1] {
        assert!(
            !arg_ownership.is_empty(),
            "non-empty args must have non-empty arg_ownership after annotation"
        );
    } else {
        panic!("expected ApplyIndirect");
    }
}
