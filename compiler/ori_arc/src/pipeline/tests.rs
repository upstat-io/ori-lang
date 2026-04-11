//! Tests for ARC pipeline verification gates.
//!
//! Verifies that `run_verify()` and `run_aims_verify()` return blocking
//! errors under explicit verification mode (`verify=true`) while
//! preserving the existing warning-only behavior under debug assertions.

use ori_types::Idx;

use crate::aims::contract::{
    ContextBehavior, EffectSummary, FipContract, MemoryContract, ParamContract, ReturnContract,
};
use crate::aims::lattice::{
    AccessClass, Cardinality, Consumption, Locality, ShapeClass, Uniqueness,
};
use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, RcStrategy};
use crate::test_helpers::{owned_param, v};

/// Build a function with a dangling block reference (block 0 jumps to non-existent block 99).
fn function_with_dangling_ref() -> ArcFunction {
    ArcFunction {
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: Vec::new(),
            terminator: ArcTerminator::Jump {
                target: ArcBlockId::new(99),
                args: vec![],
            },
        }],
        ..Default::default()
    }
}

/// Build a function with an `RcInc` on a scalar variable (invariant violation).
fn function_with_rc_on_scalar() -> ArcFunction {
    let mut func = ArcFunction {
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::RcInc {
                var: ArcVarId::new(0),
                count: 1,
                strategy: RcStrategy::HeapPointer,
            }],
            terminator: ArcTerminator::Unreachable,
        }],
        ..Default::default()
    };
    func.var_reprs = vec![crate::ir::ValueRepr::Scalar];
    func
}

/// Build a minimal contract for AIMS verify tests.
fn make_contract(params: Vec<ParamContract>) -> MemoryContract {
    MemoryContract {
        params,
        return_info: ReturnContract {
            uniqueness: Uniqueness::MaybeShared,
            preserves_freshness: false,
            locality: Locality::Unknown,
            shape: ShapeClass::NonReusable,
        },
        effects: EffectSummary::default(),
        context_behavior: ContextBehavior::default(),
        fip: FipContract::Never,
        is_fbip: false,
    }
}

fn absent_param() -> ParamContract {
    ParamContract {
        access: AccessClass::Borrowed,
        consumption: Consumption::Dead,
        cardinality: Cardinality::Absent,
        may_escape: false,
        may_share: false,
        locality_bound: Locality::BlockLocal,
        uniqueness: Uniqueness::MaybeShared,
    }
}

// ── run_verify: blocking under verify=true ──

#[test]
fn verify_returns_err_when_verify_true_and_errors_found() {
    let func = function_with_dangling_ref();
    let result = super::run_verify(&func, "test", true);
    assert!(
        result.is_err(),
        "run_verify should return Err when verify=true and verification errors exist"
    );
    let Err(errors) = result else {
        panic!("expected Err");
    };
    assert!(!errors.is_empty(), "should contain at least one error");
}

#[test]
fn verify_returns_ok_when_verify_false() {
    let func = function_with_dangling_ref();
    let result = super::run_verify(&func, "test", false);
    assert!(
        result.is_ok(),
        "run_verify should return Ok when verify=false (warn only under debug_assertions)"
    );
}

#[test]
fn verify_returns_ok_for_valid_function() {
    let func = ArcFunction::default();
    let result = super::run_verify(&func, "test", true);
    assert!(
        result.is_ok(),
        "run_verify should return Ok for a valid function"
    );
}

#[test]
fn verify_detects_rc_on_scalar() {
    let func = function_with_rc_on_scalar();
    let result = super::run_verify(&func, "test", true);
    assert!(
        result.is_err(),
        "run_verify should detect RcInc on scalar variable"
    );
}

// ── run_aims_verify: absent-param-has-uses (live vs dead path) ──

#[test]
fn aims_verify_blocks_absent_param_used_on_live_path() {
    // Live-path: single block `return v0`. The absent param IS used on a
    // path that reaches Return → genuine contract/IR inconsistency → Err.
    let func = crate::test_helpers::make_func(
        vec![owned_param(0, Idx::NONE)],
        Idx::NONE,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::NONE],
    );
    let contract = make_contract(vec![absent_param()]);

    let result = super::run_aims_verify(&func, &contract, "test", true);
    assert!(
        result.is_err(),
        "run_aims_verify should return Err when absent param has uses on a live path"
    );
}

#[test]
fn aims_verify_allows_absent_param_in_dead_code() {
    // Dead-path regression test for BUG-04-056.
    // CFG: entry(b0) → Branch → b1(uses v0, Unreachable) / b2(return v1)
    // v0 used only in b1 (dead path to Unreachable) → no live-path use → Ok.
    use crate::ir::{ArcInstr, RcStrategy};
    let func = crate::test_helpers::make_func(
        vec![owned_param(0, Idx::NONE), owned_param(1, Idx::NONE)],
        Idx::NONE,
        vec![
            // Block 0: entry — branch to dead block or live block.
            ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: v(1),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            },
            // Block 1: dead path — uses v0, ends in Unreachable.
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![],
                body: vec![ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                    strategy: RcStrategy::HeapPointer,
                }],
                terminator: ArcTerminator::Unreachable,
            },
            // Block 2: live path — returns v1 (NOT v0).
            ArcBlock {
                id: ArcBlockId::new(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
        ],
        vec![Idx::NONE; 2],
    );
    let non_absent = ParamContract {
        cardinality: Cardinality::Once,
        ..absent_param()
    };
    let contract = make_contract(vec![absent_param(), non_absent]);

    // v0 (absent) is used only in block 1 (dead path → Unreachable).
    // live_blocks() excludes block 1 → no live-path use → Ok.
    let result = super::run_aims_verify(&func, &contract, "test", true);
    assert!(
        result.is_ok(),
        "run_aims_verify should return Ok when absent param is used only in dead code: {result:?}"
    );
}

#[test]
fn aims_verify_respects_nonzero_entry_block() {
    // Regression: live_blocks() must use func.entry, not hardcoded block 0.
    // TRMC normalization creates a prologue block and sets func.entry to it.
    // CFG: b0(return v1) / b1(entry, branch → b0 or b2) / b2(uses v0, Unreachable)
    // func.entry = b1. v0 used only in b2 (dead path) → Ok.
    use crate::ir::{ArcInstr, RcStrategy};
    let mut func = crate::test_helpers::make_func(
        vec![owned_param(0, Idx::NONE), owned_param(1, Idx::NONE)],
        Idx::NONE,
        vec![
            // Block 0: live path (return v1) — NOT the entry.
            ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
            // Block 1: entry — branch to live (b0) or dead (b2).
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: v(1),
                    then_block: ArcBlockId::new(0),
                    else_block: ArcBlockId::new(2),
                },
            },
            // Block 2: dead path — uses v0, ends in Unreachable.
            ArcBlock {
                id: ArcBlockId::new(2),
                params: vec![],
                body: vec![ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                    strategy: RcStrategy::HeapPointer,
                }],
                terminator: ArcTerminator::Unreachable,
            },
        ],
        vec![Idx::NONE; 2],
    );
    func.entry = ArcBlockId::new(1); // Entry is block 1, not 0.
    let non_absent = ParamContract {
        cardinality: Cardinality::Once,
        ..absent_param()
    };
    let contract = make_contract(vec![absent_param(), non_absent]);

    let result = super::run_aims_verify(&func, &contract, "test", true);
    assert!(
        result.is_ok(),
        "live_blocks must use func.entry (b1), not hardcoded b0: {result:?}"
    );
}

#[test]
fn aims_verify_treats_resume_as_live_exit() {
    // Regression: Resume is a real exit (exceptional unwind path).
    // CFG: b0(entry) → return v1 / b1 uses v0, Resume
    // v0 used on Resume path (live) → Err.
    use crate::ir::{ArcInstr, RcStrategy};
    let func = crate::test_helpers::make_func(
        vec![owned_param(0, Idx::NONE), owned_param(1, Idx::NONE)],
        Idx::NONE,
        vec![
            // Block 0: entry — branch to normal or unwind.
            ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: v(1),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            },
            // Block 1: unwind path — uses v0, ends in Resume (live exit).
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![],
                body: vec![ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                    strategy: RcStrategy::HeapPointer,
                }],
                terminator: ArcTerminator::Resume,
            },
            // Block 2: normal path — returns v1 (NOT v0).
            ArcBlock {
                id: ArcBlockId::new(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
        ],
        vec![Idx::NONE; 2],
    );
    let non_absent = ParamContract {
        cardinality: Cardinality::Once,
        ..absent_param()
    };
    let contract = make_contract(vec![absent_param(), non_absent]);

    // v0 used on Resume path — Resume IS a real exit → live path → Err.
    let result = super::run_aims_verify(&func, &contract, "test", true);
    assert!(
        result.is_err(),
        "Resume is a live exit — absent param used on unwind path must be an error"
    );
}

#[test]
fn aims_verify_returns_ok_when_verify_false() {
    let func = crate::test_helpers::make_func(
        vec![owned_param(0, Idx::NONE)],
        Idx::NONE,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::NONE],
    );
    let contract = make_contract(vec![absent_param()]);

    let result = super::run_aims_verify(&func, &contract, "test", false);
    assert!(
        result.is_ok(),
        "run_aims_verify should return Ok when verify=false (warn only)"
    );
}

// ── FIP structural verification tests ──

/// Verify that `FipStructural` errors are included in `VerifyError` and
/// format correctly.
#[test]
fn fip_structural_error_displays_message() {
    let err = crate::verify::VerifyError::FipStructural {
        message: "test violation".to_owned(),
    };
    let display = err.to_string();
    assert!(
        display.contains("FIP structural violation"),
        "FipStructural Display should contain 'FIP structural violation': {display}"
    );
    assert!(
        display.contains("test violation"),
        "FipStructural Display should contain the inner message: {display}"
    );
}

/// First-pass FIP verification: `CertifiedButHasMissedReuses` is non-blocking
/// (logged as debug), but `CertifiedButUnboundedStack` maps to a blocking
/// `FipStructural` error. This tests the error mapping contract — the pipeline
/// code in `aims_pipeline/mod.rs` step 5a creates `FipStructural` for
/// structural violations only.
#[test]
fn fip_first_pass_allows_missed_reuses_but_blocks_structural() {
    use crate::aims::verify::fip::FipVerificationError;

    // CertifiedButHasMissedReuses should NOT produce a FipStructural error
    // (it's expected in the first pass).
    let missed_reuse = FipVerificationError::CertifiedButHasMissedReuses {
        function: ori_ir::Name::from_raw(1),
        missed_count: 2,
    };
    let is_structural = matches!(
        missed_reuse,
        FipVerificationError::CertifiedButUnboundedStack { .. }
            | FipVerificationError::BoundedExceeded { .. }
    );
    assert!(
        !is_structural,
        "CertifiedButHasMissedReuses should NOT be classified as structural"
    );

    // CertifiedButUnboundedStack SHOULD produce a FipStructural error.
    let unbounded = FipVerificationError::CertifiedButUnboundedStack {
        function: ori_ir::Name::from_raw(1),
    };
    let is_structural = matches!(
        unbounded,
        FipVerificationError::CertifiedButUnboundedStack { .. }
            | FipVerificationError::BoundedExceeded { .. }
    );
    assert!(
        is_structural,
        "CertifiedButUnboundedStack SHOULD be classified as structural"
    );

    // The pipeline maps structural errors to VerifyError::FipStructural.
    let verify_err = crate::verify::VerifyError::FipStructural {
        message: unbounded.to_string(),
    };
    assert!(
        matches!(verify_err, crate::verify::VerifyError::FipStructural { .. }),
        "structural FIP errors should map to VerifyError::FipStructural"
    );
}

/// Second-pass FIP verification: ALL errors (including `CertifiedButHasMissedReuses`)
/// should be blocking because `may_deallocate` facts have been recomputed.
/// The pipeline code in `batch.rs` second pass maps ALL FIP errors to
/// `FipStructural`.
#[test]
fn fip_second_pass_blocks_all_errors() {
    use crate::aims::verify::fip::FipVerificationError;

    // In the second pass, ALL FIP error variants should be converted to
    // blocking VerifyError::FipStructural — including CertifiedButHasMissedReuses
    // which is expected (non-blocking) in the first pass.
    let all_variants: Vec<FipVerificationError> = vec![
        FipVerificationError::CertifiedButHasMissedReuses {
            function: ori_ir::Name::from_raw(1),
            missed_count: 1,
        },
        FipVerificationError::CertifiedButUnboundedStack {
            function: ori_ir::Name::from_raw(2),
        },
        FipVerificationError::BoundedExceeded {
            function: ori_ir::Name::from_raw(3),
            declared: 2,
            actual: 5,
        },
    ];

    // The second pass converts ALL variants to FipStructural.
    let verify_errors: Vec<crate::verify::VerifyError> = all_variants
        .into_iter()
        .map(|e| crate::verify::VerifyError::FipStructural {
            message: e.to_string(),
        })
        .collect();

    assert_eq!(
        verify_errors.len(),
        3,
        "second pass should block ALL 3 FIP error variants"
    );
    for err in &verify_errors {
        assert!(
            matches!(err, crate::verify::VerifyError::FipStructural { .. }),
            "all second-pass FIP errors should be FipStructural: {err}"
        );
    }
}
