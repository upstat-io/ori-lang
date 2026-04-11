//! Tests for the contract coherence oracle.

use super::*;

use crate::aims::contract::{
    ContextBehavior, EffectSummary, FipContract, MemoryContract, ParamContract, ReturnContract,
};
use crate::aims::lattice::{AccessClass, Cardinality, Consumption, Locality, Uniqueness};
use crate::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcTerminator, RcStrategy};
use crate::test_helpers::{make_func, owned_param, v};
use ori_types::Idx;

/// Build a one-block `ArcFunction` with the given params and body.
fn func_with_body(params: Vec<crate::ir::ArcParam>, body: Vec<ArcInstr>) -> crate::ir::ArcFunction {
    let num_vars = 1000; // large enough for any test var id
    make_func(
        params,
        Idx::UNIT,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body,
            terminator: ArcTerminator::Return { value: v(999) },
        }],
        vec![Idx::UNIT; num_vars],
    )
}

fn make_contract(param_contracts: Vec<ParamContract>) -> MemoryContract {
    MemoryContract {
        params: param_contracts,
        return_info: ReturnContract::OPTIMISTIC,
        effects: EffectSummary::OPTIMISTIC,
        context_behavior: ContextBehavior::default(),
        fip: FipContract::Never,
        is_fbip: false,
    }
}

// derive_param_contracts tests

#[test]
fn derive_param_linear_when_no_rc_ops() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![ArcInstr::Apply {
            dst: v(1),
            ty: Idx::UNIT,
            func: ori_ir::Name::from_raw(100),
            args: vec![v(0)],
            arg_ownership: vec![],
        }],
    );

    let contracts = derive_param_contracts(&func);
    assert_eq!(contracts.len(), 1);
    assert_eq!(contracts[0].access, AccessClass::Borrowed);
    assert_eq!(contracts[0].consumption, Consumption::Linear);
    assert_eq!(contracts[0].cardinality, Cardinality::Once);
}

#[test]
fn derive_param_unrestricted_when_rc_inc_present() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![ArcInstr::RcInc {
            var: v(0),
            count: 1,
            strategy: RcStrategy::HeapPointer,
        }],
    );

    let contracts = derive_param_contracts(&func);
    assert_eq!(contracts[0].access, AccessClass::Owned);
    assert_eq!(contracts[0].consumption, Consumption::Unrestricted);
}

#[test]
fn derive_param_affine_when_only_rc_dec() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![ArcInstr::RcDec {
            var: v(0),
            strategy: RcStrategy::HeapPointer,
        }],
    );

    let contracts = derive_param_contracts(&func);
    assert_eq!(contracts[0].access, AccessClass::Owned);
    assert_eq!(contracts[0].consumption, Consumption::Affine);
}

#[test]
fn derive_param_dead_when_unused() {
    let func = func_with_body(vec![owned_param(0, Idx::UNIT)], vec![]);

    let contracts = derive_param_contracts(&func);
    assert_eq!(contracts[0].access, AccessClass::Borrowed);
    assert_eq!(contracts[0].consumption, Consumption::Dead);
    assert_eq!(contracts[0].cardinality, Cardinality::Absent);
}

// verify_coherence tests

#[test]
fn oracle_accepts_matching_contract() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![ArcInstr::RcDec {
            var: v(0),
            strategy: RcStrategy::HeapPointer,
        }],
    );

    let contract = make_contract(vec![ParamContract {
        access: AccessClass::Owned,
        consumption: Consumption::Affine,
        cardinality: Cardinality::Once,
        may_escape: false,
        may_share: false,
        locality_bound: Locality::Unknown,
        uniqueness: Uniqueness::MaybeShared,
    }]);

    let mismatches = verify_coherence(&func, &contract, 0);
    assert!(
        mismatches.is_empty(),
        "expected no mismatches: {mismatches:?}"
    );
}

#[test]
fn oracle_accepts_conservative_inference() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![ArcInstr::Apply {
            dst: v(1),
            ty: Idx::UNIT,
            func: ori_ir::Name::from_raw(100),
            args: vec![v(0)],
            arg_ownership: vec![],
        }],
    );

    // Inferred says Owned + Unrestricted (conservative) but realized is Borrowed + Linear
    let contract = make_contract(vec![ParamContract {
        access: AccessClass::Owned,
        consumption: Consumption::Unrestricted,
        cardinality: Cardinality::Many,
        may_escape: true,
        may_share: true,
        locality_bound: Locality::Unknown,
        uniqueness: Uniqueness::MaybeShared,
    }]);

    let mismatches = verify_coherence(&func, &contract, 0);
    assert!(
        mismatches.is_empty(),
        "conservative inference should not produce mismatches: {mismatches:?}"
    );
}

#[test]
fn oracle_rejects_unsafe_optimistic_inference() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![
            ArcInstr::RcInc {
                var: v(0),
                count: 1,
                strategy: RcStrategy::HeapPointer,
            },
            ArcInstr::RcDec {
                var: v(0),
                strategy: RcStrategy::HeapPointer,
            },
        ],
    );

    // Inferred says Borrowed + Linear but realized has both RcInc and RcDec
    let contract = make_contract(vec![ParamContract {
        access: AccessClass::Borrowed,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        may_escape: false,
        may_share: false,
        locality_bound: Locality::BlockLocal,
        uniqueness: Uniqueness::Unique,
    }]);

    let mismatches = verify_coherence(&func, &contract, 0);
    assert!(
        !mismatches.is_empty(),
        "unsafe optimistic inference should produce mismatches"
    );
    assert!(
        mismatches
            .iter()
            .any(|m| matches!(m, CoherenceMismatch::ParamAccess { .. })),
        "should detect access mismatch"
    );
    assert!(
        mismatches
            .iter()
            .any(|m| matches!(m, CoherenceMismatch::ParamConsumption { .. })),
        "should detect consumption mismatch"
    );
}

#[test]
fn oracle_detects_may_deallocate_mismatch() {
    let func = func_with_body(vec![], vec![]);

    let mut contract = make_contract(vec![]);
    contract.effects.may_deallocate = false;

    let mismatches = verify_coherence(&func, &contract, 3);
    assert!(
        mismatches.iter().any(|m| matches!(
            m,
            CoherenceMismatch::EffectMismatch {
                field: "may_deallocate",
                ..
            }
        )),
        "should detect may_deallocate mismatch when missed_reuses > 0"
    );
}
