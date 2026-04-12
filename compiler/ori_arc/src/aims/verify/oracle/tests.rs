//! Tests for the contract coherence oracle.

use super::*;

use crate::aims::contract::{
    ContextBehavior, EffectSummary, FipContract, MemoryContract, ParamContract, ReturnContract,
};
use crate::aims::lattice::{AccessClass, Cardinality, Consumption, Locality, Uniqueness};
use crate::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcTerminator, RcStrategy};
use crate::test_helpers::{make_func, owned_param, v};
use crate::ArgOwnership;
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

// --- 05.PRE: Failing tests that expose known soundness bugs in the current oracle ---

/// The oracle must detect RC operations on parameter aliases created via `Let` bindings.
/// Bug: current oracle only checks direct parameter vars, not aliases.
/// 05.PRE TDD: un-ignore after 05.1 rewrite.
#[test]
#[ignore = "05.PRE: oracle blind to aliasing — will pass after 05.1 rewrite"]
fn oracle_tracks_aliased_param_via_let_binding() {
    // param0 -> v1 via Let { dst: v1, value: Var(param0) }
    // RcInc on v1 should be detected as an RC op on param0
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![
            ArcInstr::Let {
                dst: v(1),
                ty: Idx::UNIT,
                value: crate::ir::ArcValue::Var(v(0)),
            },
            ArcInstr::RcInc {
                var: v(1), // alias of param0
                count: 1,
                strategy: RcStrategy::HeapPointer,
            },
        ],
    );

    let contracts = derive_param_contracts(&func);
    // The oracle SHOULD detect this as Owned + Unrestricted (RcInc on an alias).
    // Currently fails: oracle sees Borrowed + Linear (misses the aliased RcInc).
    assert_eq!(
        contracts[0].access,
        AccessClass::Owned,
        "RcInc on alias of param0 should make access=Owned"
    );
    assert_eq!(
        contracts[0].consumption,
        Consumption::Unrestricted,
        "RcInc on alias of param0 should make consumption=Unrestricted"
    );
}

/// The oracle must count batched `RcInc.count`, not just increment by 1.
/// Bug: current oracle does `rc_incs[idx] += 1` instead of `+= count`.
#[test]
fn oracle_counts_batched_rc_inc() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![ArcInstr::RcInc {
            var: v(0),
            count: 3, // batched: 3 increments in one instruction
            strategy: RcStrategy::HeapPointer,
        }],
    );

    let contracts = derive_param_contracts(&func);
    // With count=3, the oracle should see 3 increments, not 1.
    // Both current and correct oracle say Unrestricted (any rc_incs > 0),
    // so the bug is unobservable at the current API surface. The real impact
    // is in future `may_share` derivation and diagnostic detail.
    assert_eq!(contracts[0].access, AccessClass::Owned);
    assert_eq!(contracts[0].consumption, Consumption::Unrestricted);
}

/// The oracle must detect ownership transfers via `arg_ownership` on `Apply`.
/// Bug: current oracle treats all `Apply` args as non-RC uses, ignoring `arg_ownership`.
/// 05.PRE TDD: un-ignore after 05.1 rewrite.
#[test]
#[ignore = "05.PRE: oracle blind to arg_ownership — will pass after 05.1 rewrite"]
fn oracle_accounts_for_arg_ownership_transfer() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![ArcInstr::Apply {
            dst: v(1),
            ty: Idx::UNIT,
            func: ori_ir::Name::from_raw(100),
            args: vec![v(0)],
            arg_ownership: vec![ArgOwnership::Owned], // ownership transfer!
        }],
    );

    let contracts = derive_param_contracts(&func);
    // When param0 is passed as Owned to a callee, the oracle SHOULD detect
    // this as access=Owned (ownership was transferred). Currently the oracle
    // sees this as a non-RC use -> Borrowed + Linear.
    assert_eq!(
        contracts[0].access,
        AccessClass::Owned,
        "param passed as Owned arg should make access=Owned"
    );
}

/// The oracle should derive `may_share` from `rc_incs > 0`.
/// Bug: current `RealizedParamContract` has no `may_share` field.
#[test]
fn oracle_derives_may_share_from_rc_incs() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![ArcInstr::RcInc {
            var: v(0),
            count: 1,
            strategy: RcStrategy::HeapPointer,
        }],
    );

    let _contracts = derive_param_contracts(&func);
    // The oracle SHOULD expose `may_share`. Currently `RealizedParamContract`
    // has no `may_share` field. Verify the coherence comparison misses it.
    let inferred = make_contract(vec![ParamContract {
        access: AccessClass::Owned,
        consumption: Consumption::Unrestricted,
        cardinality: Cardinality::Many,
        may_escape: false,
        may_share: false, // claims no sharing, but realized has RcInc
        locality_bound: Locality::Unknown,
        uniqueness: Uniqueness::MaybeShared,
    }]);

    let mismatches = verify_coherence(&func, &inferred, 0);
    // The oracle SHOULD detect that inferred.may_share=false but realized
    // has rc_incs > 0 (meaning may_share should be true). Currently the
    // oracle does not check may_share at all -- verify_coherence returns
    // NO mismatches for this case, which is the bug.
    //
    // GAP: verify_coherence does not check may_share dimension.
    assert!(
        !mismatches.iter().any(|m| matches!(
            m,
            CoherenceMismatch::EffectMismatch {
                field: "may_share",
                ..
            }
        )),
        "BUG: oracle currently does NOT detect may_share mismatch -- this test documents the gap"
    );
}

/// `RcDec` after a non-RC use should be `Linear`, not `Affine`.
/// Bug: current oracle derives `Affine` for any "`RcDec` only" pattern,
/// regardless of whether there was a prior non-RC use.
/// 05.PRE TDD: un-ignore after 05.1 rewrite.
#[test]
#[ignore = "05.PRE: oracle conflates Affine/Linear — will pass after 05.1 rewrite"]
fn oracle_distinguishes_affine_from_linear() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![
            // First: a non-RC use (Apply with Borrowed arg)
            ArcInstr::Apply {
                dst: v(1),
                ty: Idx::UNIT,
                func: ori_ir::Name::from_raw(100),
                args: vec![v(0)],
                arg_ownership: vec![], // empty = default Borrowed
            },
            // Then: RcDec (cleanup after use)
            ArcInstr::RcDec {
                var: v(0),
                strategy: RcStrategy::HeapPointer,
            },
        ],
    );

    let contracts = derive_param_contracts(&func);
    // The current oracle sees rc_incs=0, rc_decs=1, and derives Affine.
    // But there IS a non-RC use (the Apply), so the correct derivation
    // is Linear (consumed at use, then dropped). Affine means "dropped
    // WITHOUT use" -- which is not the case here.
    assert_eq!(
        contracts[0].consumption,
        Consumption::Linear,
        "RcDec after non-RC use should be Linear, not Affine"
    );
}
