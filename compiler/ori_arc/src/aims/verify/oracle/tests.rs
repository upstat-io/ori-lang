//! Tests for the contract coherence oracle.

use super::*;

use crate::aims::contract::{
    ContextBehavior, EffectSummary, FipContract, MemoryContract, ParamContract, ReturnContract,
};
use crate::aims::lattice::{AccessClass, Cardinality, Consumption, Locality, Uniqueness};
use crate::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcTerminator, RcStrategy};
use crate::test_helpers::{borrowed_param, make_func, owned_param, v};
use crate::ArgOwnership;
use ori_types::Idx;

mod evidence;

/// Build a one-block `ArcFunction` with the given params and body.
fn func_with_body(params: Vec<crate::ir::ArcParam>, body: Vec<ArcInstr>) -> crate::ir::ArcFunction {
    let num_vars = 1000;
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

fn verify_isolated(
    func: &crate::ir::ArcFunction,
    contract: &MemoryContract,
    missed_reuses: u32,
) -> Vec<CoherenceMismatch> {
    verify_coherence(
        func,
        contract,
        &FxHashMap::default(),
        &StringInterner::default(),
        missed_reuses,
    )
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
fn derive_param_credit_is_sharing_evidence_not_semantic_demand() {
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
    assert_eq!(contracts[0].access, AccessClass::Borrowed);
    assert_eq!(contracts[0].consumption, Consumption::Dead);
    assert!(contracts[0].may_share);
}

#[test]
fn derive_param_unfunded_release_requires_owned_without_adding_demand() {
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
    assert_eq!(contracts[0].consumption, Consumption::Dead);
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
        iter_consumes: false,
        borrowed_read_only: false,
        borrowed_cow_consumed: false,
        borrowed_cow_mutated: false,
    }]);

    let mismatches = verify_isolated(&func, &contract, 0);
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
        iter_consumes: false,
        borrowed_read_only: false,
        borrowed_cow_consumed: false,
        borrowed_cow_mutated: false,
    }]);

    let mismatches = verify_isolated(&func, &contract, 0);
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
            ArcInstr::RcDec {
                var: v(0),
                strategy: RcStrategy::HeapPointer,
                atomicity: crate::ir::RcAtomicity::default_atomic(),
            },
            ArcInstr::RcInc {
                var: v(0),
                count: 1,
                strategy: RcStrategy::HeapPointer,
                atomicity: crate::ir::RcAtomicity::default_atomic(),
            },
            ArcInstr::Apply {
                dst: v(1),
                ty: Idx::UNIT,
                func: ori_ir::Name::from_raw(100),
                args: vec![v(0)],
                arg_ownership: vec![ArgOwnership::Borrowed],
                mono_instance_id: None,
            },
            ArcInstr::Apply {
                dst: v(2),
                ty: Idx::UNIT,
                func: ori_ir::Name::from_raw(101),
                args: vec![v(0)],
                arg_ownership: vec![ArgOwnership::Borrowed],
                mono_instance_id: None,
            },
        ],
    );

    // The later credit cannot fund the preceding release, and two semantic
    // reads on one path compose to Unrestricted.
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
        iter_consumes: false,
        borrowed_read_only: false,
        borrowed_cow_consumed: false,
        borrowed_cow_mutated: false,
    }]);

    let mismatches = verify_isolated(&func, &contract, 0);
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

    let mismatches = verify_isolated(&func, &contract, 3);
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

// Alias-aware, ownership-aware, and count-aware oracle derivation

/// The oracle detects RC operations on parameter aliases created via `Let` bindings.
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
                // Variable 1 aliases parameter 0.
                var: v(1),
                count: 1,
                strategy: RcStrategy::HeapPointer,
                atomicity: crate::ir::RcAtomicity::default_atomic(),
            },
        ],
    );

    let contracts = derive_param_contracts(&func);
    // A positive credit is independent sharing evidence but does not by itself
    // require incoming ownership or add semantic demand.
    assert_eq!(
        contracts[0].access,
        AccessClass::Borrowed,
        "an explicit alias credit should preserve Borrowed access"
    );
    assert_eq!(
        contracts[0].consumption,
        Consumption::Dead,
        "an RC event is evidence, not a semantic demand"
    );
    assert!(contracts[0].may_share);
}

/// The oracle counts batched `RcInc.count` rather than incrementing by 1
/// per instruction.
#[test]
fn oracle_counts_batched_rc_inc() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![
            ArcInstr::RcInc {
                var: v(0),
                count: 3,
                strategy: RcStrategy::HeapPointer,
                atomicity: crate::ir::RcAtomicity::default_atomic(),
            },
            ArcInstr::Construct {
                dst: v(1),
                ty: Idx::UNIT,
                ctor: crate::ir::CtorKind::Tuple,
                args: vec![v(0), v(0), v(0)],
            },
        ],
    );

    let contracts = derive_param_contracts(&func);
    // All three transfers are funded. Treating the batched credit as one would
    // make the third transfer underflow.
    assert_eq!(contracts[0].access, AccessClass::Borrowed);
    assert_eq!(contracts[0].consumption, Consumption::Unrestricted);
}

/// The oracle detects ownership transfers via `arg_ownership` on `Apply`.
#[test]
fn oracle_accounts_for_arg_ownership_transfer() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![ArcInstr::Apply {
            dst: v(1),
            ty: Idx::UNIT,
            func: ori_ir::Name::from_raw(100),
            args: vec![v(0)],
            // The call transfers ownership.
            arg_ownership: vec![ArgOwnership::Owned],
            mono_instance_id: None,
        }],
    );

    let contracts = derive_param_contracts(&func);
    // param0 passed as an Owned arg to the callee makes access=Owned
    // (ownership was transferred).
    assert_eq!(
        contracts[0].access,
        AccessClass::Owned,
        "param passed as Owned arg should make access=Owned"
    );
}

/// The oracle derives `may_share` from `rc_incs > 0`.
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
    let inferred = make_contract(vec![ParamContract {
        access: AccessClass::Owned,
        consumption: Consumption::Unrestricted,
        cardinality: Cardinality::Many,
        may_escape: false,
        // The contract denies sharing despite the realized `RcInc`.
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
    }]);

    let mismatches = verify_isolated(&func, &inferred, 0);
    // The oracle detects that inferred.may_share=false but realized has
    // rc_incs > 0 (meaning may_share should be true) via ParamMayShare.
    assert!(
        mismatches
            .iter()
            .any(|m| matches!(m, CoherenceMismatch::ParamMayShare { .. })),
        "oracle should detect may_share mismatch (inferred=false, realized has RcInc)"
    );
}

/// `RcDec` after a non-RC use should be `Linear`, not `Affine`.
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
                // Empty ownership metadata defaults to borrowed.
                arg_ownership: vec![],
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
    // A prior non-RC use (the Apply) means the value is consumed at use,
    // then dropped, so the correct derivation is Linear — Affine means
    // "dropped WITHOUT use", which is not the case here.
    assert_eq!(
        contracts[0].consumption,
        Consumption::Linear,
        "RcDec after non-RC use should be Linear, not Affine"
    );
}

// Alias propagation across Jump→block-param→Let chains

/// An alias introduced via `Let` after a Jump block-param propagation is
/// resolved — both `Let` and Jump propagation run inside the fixpoint loop.
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
                    // Parameter 0 flows into block 1.
                    args: vec![v(0)],
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                // Block parameter 100 aliases parameter 0.
                params: vec![(v(100), Idx::UNIT)],
                body: vec![
                    ArcInstr::Let {
                        dst: v(101),
                        ty: Idx::UNIT,
                        // The assigned value aliases block parameter 100.
                        value: crate::ir::ArcValue::Var(v(100)),
                    },
                    ArcInstr::RcInc {
                        // Variable 101 completes the alias chain from parameter 0.
                        var: v(101),
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
        AccessClass::Borrowed,
        "the alias credit should preserve Borrowed access"
    );
    assert_eq!(contracts[0].consumption, Consumption::Dead);
    assert!(
        contracts[0].may_share,
        "RcInc on Jump→Let alias should detect may_share"
    );
}

/// Effect derivation detects `PartialApply` as an allocation source (closure
/// env allocation), not only `Construct`.
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

// Additional matrix tests

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
                // Variable 2 is a two-hop alias of parameter 0.
                var: v(2),
                count: 1,
                strategy: RcStrategy::HeapPointer,
                atomicity: crate::ir::RcAtomicity::default_atomic(),
            },
        ],
    );

    let contracts = derive_param_contracts(&func);
    assert_eq!(
        contracts[0].access,
        AccessClass::Borrowed,
        "the transitive alias credit should preserve Borrowed access"
    );
    assert_eq!(contracts[0].consumption, Consumption::Dead);
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
            // Closure operands are always borrowed.
            closure: v(10),
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

// Effect derivation tests

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
            var: v(10),
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
        // The contract conservatively allows sharing.
        may_share: true,
        locality_bound: Locality::Unknown,
        uniqueness: Uniqueness::MaybeShared,
        transfers_through_return: false,
        return_alias: None,
        return_payload_contains_param: false,
        iter_consumes: false,
        borrowed_read_only: false,
        borrowed_cow_consumed: false,
        borrowed_cow_mutated: false,
    }]);

    let mismatches = verify_isolated(&func, &contract, 0);
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
            var: v(10),
            count: 1,
            strategy: RcStrategy::HeapPointer,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
        }],
    );

    let mut contract = make_contract(vec![]);
    // The contract denies sharing.
    contract.effects.may_share = false;

    let mismatches = verify_isolated(&func, &contract, 0);
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
/// The oracle reports the structural disagreement and still checks the prefix.
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
            iter_consumes: false,
            borrowed_read_only: false,
            borrowed_cow_consumed: false,
            borrowed_cow_mutated: false,
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
            iter_consumes: false,
            borrowed_read_only: false,
            borrowed_cow_consumed: false,
            borrowed_cow_mutated: false,
        },
    ]);

    let mismatches = verify_isolated(&func, &contract, 0);
    assert!(
        matches!(
            mismatches.as_slice(),
            [CoherenceMismatch::ParamArity {
                function_params: 1,
                inferred_params: 2
            }]
        ),
        "arity mismatch must not disappear through zip truncation: {mismatches:?}"
    );
}

/// Param count mismatch (reverse): function has 2 params, contract has 1.
/// Extra function params are an unsafe structural mismatch.
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
        iter_consumes: false,
        borrowed_read_only: false,
        borrowed_cow_consumed: false,
        borrowed_cow_mutated: false,
    }]);

    let mismatches = verify_isolated(&func, &contract, 0);
    assert!(
        matches!(
            mismatches.as_slice(),
            [CoherenceMismatch::ParamArity {
                function_params: 2,
                inferred_params: 1
            }]
        ),
        "extra function parameters must be reported: {mismatches:?}"
    );
}

/// Conservative effect: inferred claims `may_allocate=true` but no Construct exists.
/// Should not produce a mismatch (safe direction).
#[test]
fn oracle_accepts_conservative_may_allocate_effect() {
    let func = func_with_body(vec![], vec![]);

    let mut contract = make_contract(vec![]);
    // The contract conservatively allows allocation.
    contract.effects.may_allocate = true;

    let mismatches = verify_isolated(&func, &contract, 0);
    assert!(
        mismatches.is_empty(),
        "conservative may_allocate (inferred=true, realized=false) should not produce mismatches: {mismatches:?}"
    );
}

// Display impl tests

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
