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
            mono_instance_id: None,
        }],
    );

    let contracts = derive_param_contracts(&func);
    assert_eq!(contracts.len(), 1);
    assert_eq!(contracts[0].access, AccessClass::Owned);
    assert_eq!(contracts[0].consumption, Consumption::Linear);
    assert!(!contracts[0].may_share);
}

#[test]
fn derive_param_unrestricted_when_rc_inc_present() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![ArcInstr::RcInc {
            var: v(0),
            count: 1,
            strategy: RcStrategy::HeapPointer,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
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
            atomicity: crate::ir::RcAtomicity::default_atomic(),
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
    assert!(!contracts[0].may_share);
}

// verify_coherence tests

#[test]
fn oracle_accepts_matching_contract() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![ArcInstr::RcDec {
            var: v(0),
            strategy: RcStrategy::HeapPointer,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
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
        transfers_through_return: false,
        return_alias: None,
        return_payload_contains_param: false,
        return_payload_contains_param_all_paths: false,
        iter_consumes: false,
        borrowed_read_only: false,
        borrowed_cow_consumed: false,
        capture_variant_return_project: None,
        iter_consumes_projected_field: None,
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
            mono_instance_id: None,
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
        transfers_through_return: false,
        return_alias: None,
        return_payload_contains_param: false,
        return_payload_contains_param_all_paths: false,
        iter_consumes: false,
        borrowed_read_only: false,
        borrowed_cow_consumed: false,
        capture_variant_return_project: None,
        iter_consumes_projected_field: None,
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
                atomicity: crate::ir::RcAtomicity::default_atomic(),
            },
            ArcInstr::RcDec {
                var: v(0),
                strategy: RcStrategy::HeapPointer,
                atomicity: crate::ir::RcAtomicity::default_atomic(),
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
        transfers_through_return: false,
        return_alias: None,
        return_payload_contains_param: false,
        return_payload_contains_param_all_paths: false,
        iter_consumes: false,
        borrowed_read_only: false,
        borrowed_cow_consumed: false,
        capture_variant_return_project: None,
        iter_consumes_projected_field: None,
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
/// 05.PRE TDD: now passes with aliasing-aware oracle (05.1 rewrite).
#[test]
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
                atomicity: crate::ir::RcAtomicity::default_atomic(),
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
            atomicity: crate::ir::RcAtomicity::default_atomic(),
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
/// 05.PRE TDD: now passes with ownership-aware oracle (05.1 rewrite).
#[test]
fn oracle_accounts_for_arg_ownership_transfer() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![ArcInstr::Apply {
            dst: v(1),
            ty: Idx::UNIT,
            func: ori_ir::Name::from_raw(100),
            args: vec![v(0)],
            arg_ownership: vec![ArgOwnership::Owned], // ownership transfer!
            mono_instance_id: None,
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
            atomicity: crate::ir::RcAtomicity::default_atomic(),
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
        transfers_through_return: false,
        return_alias: None,
        return_payload_contains_param: false,
        return_payload_contains_param_all_paths: false,
        iter_consumes: false,
        borrowed_read_only: false,
        borrowed_cow_consumed: false,
        capture_variant_return_project: None,
        iter_consumes_projected_field: None,
    }]);

    let mismatches = verify_coherence(&func, &inferred, 0);
    // The oracle SHOULD detect that inferred.may_share=false but realized
    // has rc_incs > 0 (meaning may_share should be true). Currently the
    // oracle does not check may_share at all -- verify_coherence returns
    // NO mismatches for this case, which is the bug.
    //
    // After 05.1 rewrite: oracle now checks may_share via ParamMayShare.
    assert!(
        mismatches
            .iter()
            .any(|m| matches!(m, CoherenceMismatch::ParamMayShare { .. })),
        "oracle should detect may_share mismatch (inferred=false, realized has RcInc)"
    );
}

/// `RcDec` after a non-RC use should be `Linear`, not `Affine`.
/// Bug: current oracle derives `Affine` for any "`RcDec` only" pattern,
/// regardless of whether there was a prior non-RC use.
/// 05.PRE TDD: now passes with aggregate-count derivation (05.1 rewrite).
#[test]
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
                mono_instance_id: None,
            },
            // Then: RcDec (cleanup after use)
            ArcInstr::RcDec {
                var: v(0),
                strategy: RcStrategy::HeapPointer,
                atomicity: crate::ir::RcAtomicity::default_atomic(),
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

// Alias propagation across Jump→block-param→Let chains

/// Regression: alias introduced via Let AFTER a Jump block-param propagation.
/// Bug: the Let pass ran outside the fixpoint loop, so Let bindings in blocks
/// reached by Jump were never resolved when the source was a block param.
/// Fix: both Let and Jump propagation are now inside the fixpoint loop.
#[test]
fn oracle_tracks_alias_through_jump_then_let() {
    // Block 0: Jump to block 1, passing param0 as arg
    // Block 1: block param bp0 = v(100), then Let { dst: v(101), value: Var(bp0) }
    //          then RcInc on v(101) — should be detected as param0 alias
    let func = make_func(
        vec![owned_param(0, Idx::UNIT)],
        Idx::UNIT,
        vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: vec![v(0)], // pass param0 to block 1
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![(v(100), Idx::UNIT)], // bp0 = v(100) aliases param0
                body: vec![
                    ArcInstr::Let {
                        dst: v(101),
                        ty: Idx::UNIT,
                        value: crate::ir::ArcValue::Var(v(100)), // alias of bp0
                    },
                    ArcInstr::RcInc {
                        var: v(101), // alias chain: param0 → bp0 → v(101)
                        count: 1,
                        strategy: RcStrategy::HeapPointer,
                        atomicity: crate::ir::RcAtomicity::default_atomic(),
                    },
                ],
                terminator: ArcTerminator::Return { value: v(999) },
            },
        ],
        vec![Idx::UNIT; 1000],
    );

    let contracts = derive_param_contracts(&func);
    assert_eq!(
        contracts[0].access,
        AccessClass::Owned,
        "RcInc on Jump→Let alias of param0 should detect as Owned"
    );
    assert_eq!(contracts[0].consumption, Consumption::Unrestricted);
    assert!(
        contracts[0].may_share,
        "RcInc on Jump→Let alias should detect may_share"
    );
}

/// Regression: effect derivation must detect `PartialApply` as allocation source.
/// Bug: `derive_effects()` only checked Construct, missing closure env allocation.
#[test]
fn oracle_detects_may_allocate_from_partial_apply() {
    let func = func_with_body(
        vec![],
        vec![ArcInstr::PartialApply {
            dst: v(1),
            ty: Idx::UNIT,
            func: ori_ir::Name::from_raw(200),
            args: vec![v(10)],
        }],
    );

    let effects = derive_effects(&func, 0);
    assert!(
        effects.may_allocate,
        "PartialApply present -> may_allocate=true (closure env allocation)"
    );
}

// --- 05.1.3 Additional matrix tests ---

/// Transitive alias chain: param0 -> v1 -> v2, `RcInc` on v2 detected.
#[test]
fn oracle_tracks_transitive_alias_chain() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![
            ArcInstr::Let {
                dst: v(1),
                ty: Idx::UNIT,
                value: crate::ir::ArcValue::Var(v(0)),
            },
            ArcInstr::Let {
                dst: v(2),
                ty: Idx::UNIT,
                value: crate::ir::ArcValue::Var(v(1)),
            },
            ArcInstr::RcInc {
                var: v(2), // two-hop alias of param0
                count: 1,
                strategy: RcStrategy::HeapPointer,
                atomicity: crate::ir::RcAtomicity::default_atomic(),
            },
        ],
    );

    let contracts = derive_param_contracts(&func);
    assert_eq!(
        contracts[0].access,
        AccessClass::Owned,
        "RcInc on two-hop alias should detect as Owned"
    );
    assert_eq!(contracts[0].consumption, Consumption::Unrestricted);
    assert!(contracts[0].may_share);
}

/// Ownership transfer via `ApplyIndirect` (closure call with owned arg).
#[test]
fn oracle_detects_owned_transfer_via_apply_indirect() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![ArcInstr::ApplyIndirect {
            dst: v(2),
            ty: Idx::UNIT,
            closure: v(10), // closure is always borrowed
            args: vec![v(0)],
            arg_ownership: vec![ArgOwnership::Owned],
        }],
    );

    let contracts = derive_param_contracts(&func);
    assert_eq!(
        contracts[0].access,
        AccessClass::Owned,
        "param passed as Owned in ApplyIndirect should be Owned"
    );
    assert_eq!(contracts[0].consumption, Consumption::Linear);
}

/// Ownership transfer via `Invoke` terminator (unwind-capable call).
#[test]
fn oracle_detects_owned_transfer_via_invoke() {
    let func = make_func(
        vec![owned_param(0, Idx::UNIT)],
        Idx::UNIT,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Invoke {
                dst: v(1),
                ty: Idx::UNIT,
                func: ori_ir::Name::from_raw(100),
                args: vec![v(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
                normal: ArcBlockId::new(1),
                unwind: ArcBlockId::new(2),
            },
        }],
        vec![Idx::UNIT; 1000],
    );

    let contracts = derive_param_contracts(&func);
    assert_eq!(
        contracts[0].access,
        AccessClass::Owned,
        "param passed as Owned in Invoke terminator should be Owned"
    );
}

/// Ownership transfer via `Construct` (all constructor args are owned).
#[test]
fn oracle_detects_owned_transfer_via_construct() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![ArcInstr::Construct {
            dst: v(1),
            ty: Idx::UNIT,
            ctor: crate::ir::CtorKind::Tuple,
            args: vec![v(0), v(10)],
        }],
    );

    let contracts = derive_param_contracts(&func);
    assert_eq!(
        contracts[0].access,
        AccessClass::Owned,
        "param consumed by Construct should be Owned"
    );
    assert_eq!(contracts[0].consumption, Consumption::Linear);
}

/// Ownership transfer via `PartialApply` (closure capture — all args owned).
#[test]
fn oracle_detects_owned_transfer_via_partial_apply() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![ArcInstr::PartialApply {
            dst: v(1),
            ty: Idx::UNIT,
            func: ori_ir::Name::from_raw(200),
            args: vec![v(0)],
        }],
    );

    let contracts = derive_param_contracts(&func);
    assert_eq!(
        contracts[0].access,
        AccessClass::Owned,
        "param captured by PartialApply should be Owned"
    );
    assert_eq!(contracts[0].consumption, Consumption::Linear);
}

// --- 05.2 Effect derivation tests ---

/// `may_allocate` detected from `Construct` instruction in the function.
#[test]
fn oracle_detects_may_allocate_from_construct() {
    let func = func_with_body(
        vec![],
        vec![ArcInstr::Construct {
            dst: v(1),
            ty: Idx::UNIT,
            ctor: crate::ir::CtorKind::Tuple,
            args: vec![v(10), v(11)],
        }],
    );

    let effects = derive_effects(&func, 0);
    assert!(
        effects.may_allocate,
        "Construct present -> may_allocate=true"
    );
    assert!(
        !effects.may_deallocate,
        "missed_reuses=0 -> may_deallocate=false"
    );
    assert!(!effects.may_share, "no RcInc -> may_share=false");
}

/// Function-level `may_share` detects `RcInc` on a local variable (not a param).
#[test]
fn oracle_detects_may_share_from_local_rc_inc() {
    // v(10) is a local variable, not a function parameter.
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![ArcInstr::RcInc {
            var: v(10), // local, not param
            count: 1,
            strategy: RcStrategy::HeapPointer,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
        }],
    );

    let effects = derive_effects(&func, 0);
    assert!(
        effects.may_share,
        "RcInc on local variable -> function-level may_share=true"
    );
}

/// Conservative `may_share`: inferred true but realized false → no mismatch (safe direction).
#[test]
fn oracle_accepts_conservative_may_share() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        // No RcInc on param0 → realized may_share = false
        vec![ArcInstr::Apply {
            dst: v(1),
            ty: Idx::UNIT,
            func: ori_ir::Name::from_raw(100),
            args: vec![v(0)],
            arg_ownership: vec![],
            mono_instance_id: None,
        }],
    );

    // Inferred claims may_share=true (conservative) but realized has no RcInc
    let contract = make_contract(vec![ParamContract {
        access: AccessClass::Owned,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        may_escape: false,
        may_share: true, // conservative: claims sharing
        locality_bound: Locality::Unknown,
        uniqueness: Uniqueness::MaybeShared,
        transfers_through_return: false,
        return_alias: None,
        return_payload_contains_param: false,
        return_payload_contains_param_all_paths: false,
        iter_consumes: false,
        borrowed_read_only: false,
        borrowed_cow_consumed: false,
        capture_variant_return_project: None,
        iter_consumes_projected_field: None,
    }]);

    let mismatches = verify_coherence(&func, &contract, 0);
    assert!(
        mismatches.is_empty(),
        "conservative may_share (inferred=true, realized=false) should not produce mismatches: {mismatches:?}"
    );
}

/// `may_share` effect mismatch detected when inferred says no sharing but
/// the function has `RcInc` on a local variable.
#[test]
fn oracle_coherence_catches_function_level_may_share_mismatch() {
    let func = func_with_body(
        vec![],
        vec![ArcInstr::RcInc {
            var: v(10), // local variable
            count: 1,
            strategy: RcStrategy::HeapPointer,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
        }],
    );

    let mut contract = make_contract(vec![]);
    contract.effects.may_share = false; // claims no sharing

    let mismatches = verify_coherence(&func, &contract, 0);
    assert!(
        mismatches.iter().any(|m| matches!(
            m,
            CoherenceMismatch::EffectMismatch {
                field: "may_share",
                ..
            }
        )),
        "local RcInc should trigger function-level may_share mismatch"
    );
}

/// Param count mismatch: inferred has 2 params but function has 1.
/// Oracle should handle gracefully via `zip` (checks only the shared prefix).
#[test]
fn oracle_handles_param_count_mismatch_gracefully() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![ArcInstr::RcDec {
            var: v(0),
            strategy: RcStrategy::HeapPointer,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
        }],
    );

    // Contract claims 2 params but function only has 1.
    let contract = make_contract(vec![
        ParamContract {
            access: AccessClass::Owned,
            consumption: Consumption::Affine,
            cardinality: Cardinality::Once,
            may_escape: false,
            may_share: false,
            locality_bound: Locality::Unknown,
            uniqueness: Uniqueness::MaybeShared,
            transfers_through_return: false,
            return_alias: None,
            return_payload_contains_param: false,
            return_payload_contains_param_all_paths: false,
            iter_consumes: false,
            borrowed_read_only: false,
            borrowed_cow_consumed: false,
            capture_variant_return_project: None,
            iter_consumes_projected_field: None,
        },
        ParamContract {
            access: AccessClass::Owned,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            may_escape: false,
            may_share: false,
            locality_bound: Locality::Unknown,
            uniqueness: Uniqueness::MaybeShared,
            transfers_through_return: false,
            return_alias: None,
            return_payload_contains_param: false,
            return_payload_contains_param_all_paths: false,
            iter_consumes: false,
            borrowed_read_only: false,
            borrowed_cow_consumed: false,
            capture_variant_return_project: None,
            iter_consumes_projected_field: None,
        },
    ]);

    // Should not panic — zip handles length mismatch by truncating to shorter.
    let mismatches = verify_coherence(&func, &contract, 0);
    // The matching param (index 0) should not produce mismatches since
    // inferred=Owned/Affine matches realized=Owned/Affine.
    assert!(
        mismatches.is_empty(),
        "param count mismatch should be handled gracefully: {mismatches:?}"
    );
}

/// Param count mismatch (reverse): function has 2 params, contract has 1.
/// Extra function params beyond the contract are silently ignored by zip.
#[test]
fn oracle_handles_extra_function_params_gracefully() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT), owned_param(1, Idx::UNIT)],
        vec![],
    );

    let contract = make_contract(vec![ParamContract {
        access: AccessClass::Borrowed,
        consumption: Consumption::Dead,
        cardinality: Cardinality::Absent,
        may_escape: false,
        may_share: false,
        locality_bound: Locality::Unknown,
        uniqueness: Uniqueness::MaybeShared,
        transfers_through_return: false,
        return_alias: None,
        return_payload_contains_param: false,
        return_payload_contains_param_all_paths: false,
        iter_consumes: false,
        borrowed_read_only: false,
        borrowed_cow_consumed: false,
        capture_variant_return_project: None,
        iter_consumes_projected_field: None,
    }]);

    // Should not panic — zip truncates to the shorter (1 param from contract).
    let mismatches = verify_coherence(&func, &contract, 0);
    assert!(
        mismatches.is_empty(),
        "extra function params should be handled gracefully: {mismatches:?}"
    );
}

/// Conservative effect: inferred claims `may_allocate=true` but no Construct exists.
/// Should not produce a mismatch (safe direction).
#[test]
fn oracle_accepts_conservative_may_allocate_effect() {
    let func = func_with_body(vec![], vec![]);

    let mut contract = make_contract(vec![]);
    contract.effects.may_allocate = true; // conservative: claims allocation

    let mismatches = verify_coherence(&func, &contract, 0);
    assert!(
        mismatches.is_empty(),
        "conservative may_allocate (inferred=true, realized=false) should not produce mismatches: {mismatches:?}"
    );
}

// --- Display impl tests ---

/// Display for `ParamAccess` mismatch produces actionable diagnostic text.
#[test]
fn display_param_access_mismatch_includes_index_and_direction() {
    let mismatch = CoherenceMismatch::ParamAccess {
        param_index: 2,
        param_var: v(7),
        inferred: AccessClass::Borrowed,
        realized: AccessClass::Owned,
    };
    let text = format!("{mismatch}");
    assert!(text.contains("param 2"), "should include param index");
    assert!(text.contains("access"), "should identify the dimension");
    assert!(
        text.contains("Borrowed") && text.contains("Owned"),
        "should include both directions: {text}"
    );
}

/// Display for `EffectMismatch` includes the field name.
#[test]
fn display_effect_mismatch_includes_field_name() {
    let mismatch = CoherenceMismatch::EffectMismatch {
        field: "may_deallocate",
        inferred: false,
        realized: true,
    };
    let text = format!("{mismatch}");
    assert!(text.contains("may_deallocate"), "should include field name");
    assert!(
        text.contains("false") && text.contains("true"),
        "should include both values: {text}"
    );
}
