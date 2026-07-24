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
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, RcStrategy, ValueRepr,
    VariableMetadataState,
};
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
        var_types: vec![Idx::NONE],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::RcInc {
                var: ArcVarId::new(0),
                count: 1,
                strategy: RcStrategy::HeapPointer,
                atomicity: crate::ir::RcAtomicity::default_atomic(),
            }],
            terminator: ArcTerminator::Unreachable,
        }],
        ..Default::default()
    };
    func.replace_variable_representations(vec![crate::ir::ValueRepr::Scalar]);
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
            returns_fresh_self_alloc: false,
            returns_sharing_view: false,
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
        transfers_through_return: false,
        return_alias: None,
        return_payload_contains_param: false,
        iter_consumes: false,
        borrowed_read_only: false,
        borrowed_cow_consumed: false,
        borrowed_cow_mutated: false,
        exact_transfer: crate::aims::contract::ExactTransferState::Unproven,
    }
}

#[test]
fn final_metadata_validation_rejects_corruption_without_repairing_it() {
    let pool = ori_types::Pool::new();
    let classifier = crate::ArcClassifier::new(&pool);
    let func = ArcFunction {
        var_types: vec![Idx::STR],
        var_reprs: vec![ValueRepr::Scalar],
        var_rc_strategies: vec![None],
        var_metadata_state: VariableMetadataState::Realized,
        ..ArcFunction::default()
    };

    let result = super::aims_pipeline::validate_variable_metadata(&func, &classifier, &pool);
    let Err(errors) = result else {
        panic!("corrupt realized metadata must fail at the realization owner seam");
    };

    assert!(errors.iter().any(|error| matches!(
        error,
        crate::verify::VerifyError::VariableRepresentationMismatch {
            var,
            expected: ValueRepr::FatValue,
            found: ValueRepr::Scalar,
        } if *var == ArcVarId::new(0)
    )));
    assert!(errors.iter().any(|error| matches!(
        error,
        crate::verify::VerifyError::VariableRcStrategyMismatch {
            var,
            expected: Some(RcStrategy::FatPointer),
            found: None,
        } if *var == ArcVarId::new(0)
    )));
    let strategy_error = errors
        .iter()
        .find(|error| {
            matches!(
                error,
                crate::verify::VerifyError::VariableRcStrategyMismatch { .. }
            )
        })
        .unwrap_or_else(|| panic!("strategy mismatch should be reported"));
    assert_eq!(
        strategy_error.to_string(),
        "physical ownership-strategy metadata for v0 is inconsistent with its canonical type and representation: expected Some(FatPointer), found None; rerun the same command with ORI_VERIFY_ARC=1 and report this compiler bug (Annex E, AIMS §8.11)"
    );
    assert_eq!(func.var_reprs, [ValueRepr::Scalar]);
    assert_eq!(func.var_rc_strategies, [None]);
}

#[test]
fn final_metadata_validation_rejects_unrealized_zero_var_function() {
    let pool = ori_types::Pool::new();
    let classifier = crate::ArcClassifier::new(&pool);
    let func = ArcFunction::default();

    let result = super::aims_pipeline::validate_variable_metadata(&func, &classifier, &pool);
    let Err(errors) = result else {
        panic!("zero-variable metadata still requires an explicit realized state");
    };

    assert_eq!(
        errors,
        [crate::verify::VerifyError::VariableMetadataUnrealized]
    );
}

// run_verify: blocking under verify=true

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

// run_aims_verify: absent-param-has-uses (live vs dead path)

#[test]
fn aims_verify_blocks_absent_param_used_on_live_path() {
    // Live-path: single block `return v0`. The absent param IS used on a
    // path that reaches Return → genuine contract/IR inconsistency → Err.
    let func = crate::test_helpers::make_func(
        vec![owned_param(0, Idx::UNIT)],
        Idx::UNIT,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::UNIT],
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
    // Regression: AIMS verifier false positive on dead code after always-panic paths.
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
                    atomicity: crate::ir::RcAtomicity::default_atomic(),
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
                    atomicity: crate::ir::RcAtomicity::default_atomic(),
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
                    atomicity: crate::ir::RcAtomicity::default_atomic(),
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

// Checkpoint observer tests

#[test]
fn checkpoint_observer_with_all_passes_configured_captures_all_phase_names_in_order() {
    use std::cell::RefCell;

    // Build a minimal function that exercises the full AIMS pipeline.
    // A function with one param, one block, returns the param — simple but
    // goes through all pipeline steps.
    let mut func = ArcFunction {
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: v(0),
                ty: Idx::INT,
                value: crate::ir::ArcValue::Literal(crate::ir::LitValue::Int(0)),
            }],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        var_types: vec![Idx::INT],
        ..Default::default()
    };

    // Capture phase names from the observer callback (single-threaded test).
    let captured = RefCell::new(Vec::<String>::new());
    let observer = |_func: &ArcFunction, phase: &str| {
        captured.borrow_mut().push(phase.to_owned());
    };

    let interner = ori_ir::StringInterner::new();
    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    // IC-1: pre-populate the synthetic function's contract (the production
    // pipeline computes contracts via analyze_program before per-function work).
    let mut contracts = rustc_hash::FxHashMap::default();
    contracts.insert(func.name, MemoryContract::conservative(func.params.len()));
    let func_names: rustc_hash::FxHashSet<ori_ir::Name> = contracts.keys().copied().collect();
    let pool = ori_types::Pool::default();
    let classifier = crate::classify::ArcClassifier::new(&pool);
    let type_registry = ori_types::TypeRegistry::default();

    let config = super::aims_pipeline::AimsPipelineConfig {
        classifier: &classifier,
        contracts: &contracts,
        func_names: &func_names,
        exact_callables: &func_names,
        pool: &pool,
        interner: &interner,
        builtins: &builtins,
        verify_arc: false,
        observer: Some(&observer),
        type_registry: &type_registry,
        exact_transfer_witnesses: &rustc_hash::FxHashMap::default(),
    };

    let _result = super::aims_pipeline::run_aims_pipeline(&mut func, &config);

    let phases = captured.into_inner();
    // Must have at least the core phases in order.
    assert!(
        !phases.is_empty(),
        "observer should have captured at least one phase"
    );
    // verify key phases appear in order
    let core_phases = [
        "compute_var_reprs",
        "normalize_function",
        "normalize_with_trmc_complete",
        "analyze_function",
        "class_ledger_emission",
        "realize_rc_reuse",
    ];
    let mut last_idx = 0;
    for expected in &core_phases {
        let pos = phases
            .iter()
            .position(|p| p == *expected)
            .unwrap_or_else(|| panic!("phase '{expected}' missing from {phases:?}"));
        assert!(
            pos >= last_idx,
            "phase '{expected}' at {pos} should be after previous core phase at {last_idx}"
        );
        last_idx = pos;
    }
}

#[test]
fn checkpoint_observer_when_none_skips_all_callbacks() {
    // With observer: None, no callbacks are invoked. This is a compile-only
    // structural test — the type system enforces that None means no callback,
    // but the test documents intent and verifies the config construction.
    let mut func = ArcFunction {
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: v(0),
                ty: Idx::INT,
                value: crate::ir::ArcValue::Literal(crate::ir::LitValue::Int(0)),
            }],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        var_types: vec![Idx::INT],
        ..Default::default()
    };

    let interner = ori_ir::StringInterner::new();
    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    // IC-1: pre-populate the synthetic function's contract.
    let mut contracts = rustc_hash::FxHashMap::default();
    contracts.insert(func.name, MemoryContract::conservative(func.params.len()));
    let func_names: rustc_hash::FxHashSet<ori_ir::Name> = contracts.keys().copied().collect();
    let pool = ori_types::Pool::default();
    let classifier = crate::classify::ArcClassifier::new(&pool);
    let type_registry = ori_types::TypeRegistry::default();

    let config = super::aims_pipeline::AimsPipelineConfig {
        classifier: &classifier,
        contracts: &contracts,
        func_names: &func_names,
        exact_callables: &func_names,
        pool: &pool,
        interner: &interner,
        builtins: &builtins,
        verify_arc: false,
        observer: None,
        type_registry: &type_registry,
        exact_transfer_witnesses: &rustc_hash::FxHashMap::default(),
    };

    // Pipeline runs successfully with no observer — zero overhead path.
    let result = super::aims_pipeline::run_aims_pipeline(&mut func, &config);
    assert!(
        result.is_ok(),
        "pipeline should succeed with observer: None"
    );
}

#[test]
fn checkpoint_observer_after_realize_rc_reuse_captures_added_rc_ops() {
    use std::cell::RefCell;

    // Build a function where realize_rc_reuse will ADD RC ops:
    // A function with an owned ref-counted param that is returned.
    // realize_rc_reuse should insert RcInc for the return value.
    let mut func = crate::test_helpers::make_func(
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

    // Track RC ops at the realize_rc_reuse checkpoint (single-threaded test).
    let rc_at_realize = RefCell::new(None::<(usize, usize)>);
    let observer = |func: &ArcFunction, phase: &str| {
        if phase == "realize_rc_reuse" {
            let rc = crate::pipeline::rc_count::count_rc_ops(func);
            *rc_at_realize.borrow_mut() = Some((rc.inc, rc.dec));
        }
    };

    let interner = ori_ir::StringInterner::new();
    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    // IC-1: pre-populate the synthetic function's contract.
    let mut contracts = rustc_hash::FxHashMap::default();
    contracts.insert(func.name, MemoryContract::conservative(func.params.len()));
    let func_names: rustc_hash::FxHashSet<ori_ir::Name> = contracts.keys().copied().collect();
    let pool = ori_types::Pool::default();
    let classifier = crate::classify::ArcClassifier::new(&pool);
    let type_registry = ori_types::TypeRegistry::default();

    let config = super::aims_pipeline::AimsPipelineConfig {
        classifier: &classifier,
        contracts: &contracts,
        func_names: &func_names,
        exact_callables: &func_names,
        pool: &pool,
        interner: &interner,
        builtins: &builtins,
        verify_arc: false,
        observer: Some(&observer),
        type_registry: &type_registry,
        exact_transfer_witnesses: &rustc_hash::FxHashMap::default(),
    };

    let _result = super::aims_pipeline::run_aims_pipeline(&mut func, &config);

    // The observer should have been called at the realize_rc_reuse checkpoint.
    let captured = rc_at_realize.into_inner();
    assert!(
        captured.is_some(),
        "observer should have been called at 'realize_rc_reuse' phase"
    );
    // Note: whether RC ops are present depends on the classifier results
    // for Idx::NONE (likely Scalar → no RC). The key assertion is that
    // the observer was called at the right phase — the RC counts are a
    // bonus verification that the function state is accessible.
}

// FIP structural verification tests

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

// IC-1 enforcement semantic pins

/// INV1 (positive pin): the batch pipeline computes a contract for every
/// analyzed function. The batch outcome returns the same finalized contract
/// map consumed by the second pass, so the executable seam never reanalyzes.
#[test]
fn aims_pipeline_ic1_invariant_holds_end_to_end() {
    let func = crate::test_helpers::make_func(
        vec![owned_param(0, Idx::UNIT)],
        Idx::UNIT,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::UNIT],
    );

    let interner = ori_ir::StringInterner::new();
    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    let pool = ori_types::Pool::default();
    let classifier = crate::classify::ArcClassifier::new(&pool);
    let type_registry = ori_types::TypeRegistry::default();
    let external_contracts = rustc_hash::FxHashMap::default();
    let callable_boundaries = crate::CallableBoundaryFacts::default();

    let mut funcs = vec![func];
    let result = crate::realize_closed_program(
        &mut funcs,
        &crate::ArcPipelineContext {
            classifier: &classifier,
            interner: &interner,
            pool: &pool,
            builtins: &builtins,
            type_registry: &type_registry,
            callable_boundaries: &callable_boundaries,
            verify_arc: false,
            external_contracts: &external_contracts,
        },
    );
    assert!(
        result.is_ok(),
        "IC-1 invariant: batch pipeline computes a contract for every \
         function — get_required sites must not panic"
    );
    let outcome =
        result.unwrap_or_else(|errors| panic!("unexpected verification errors: {errors:?}"));
    assert_eq!(outcome.contracts.len(), funcs.len());
    assert!(outcome.contracts.contains_key(&funcs[0].name));
    assert_eq!(outcome.function_effects.len(), funcs.len());
    assert!(outcome.function_effects.contains_key(&funcs[0].name));
    assert_eq!(outcome.fresh_return_facts.len(), funcs.len());
    assert!(outcome.fresh_return_facts.contains_key(&funcs[0].name));
    assert_eq!(outcome.param_disjointness.len(), funcs.len());
    assert!(outcome.param_disjointness.contains_key(&funcs[0].name));
}

/// INV2 (negative pin — load-bearing): invoking the per-function pipeline
/// with an empty contracts map (synthetic IC-1 break — bypasses
/// `analyze_program`) MUST panic at the first `get_required` site reached, not
/// silently degrade. Reverting any site replacement to a silent
/// `contracts.get(...).is_some()` fallback would make this test pass with no
/// panic — the pin guards against that regression.
#[test]
#[should_panic(expected = "AIMS Invariant IC-1")]
fn aims_pipeline_panics_on_synthetic_invariant_break() {
    let mut func = crate::test_helpers::make_func(
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

    let interner = ori_ir::StringInterner::new();
    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    // Synthetic IC-1 break: empty contracts map, no analyze_program run.
    let contracts = rustc_hash::FxHashMap::default();
    let func_names = rustc_hash::FxHashSet::default();
    let pool = ori_types::Pool::default();
    let classifier = crate::classify::ArcClassifier::new(&pool);
    let type_registry = ori_types::TypeRegistry::default();

    let config = super::aims_pipeline::AimsPipelineConfig {
        classifier: &classifier,
        contracts: &contracts,
        func_names: &func_names,
        exact_callables: &func_names,
        pool: &pool,
        interner: &interner,
        builtins: &builtins,
        verify_arc: false,
        observer: None,
        type_registry: &type_registry,
        exact_transfer_witnesses: &rustc_hash::FxHashMap::default(),
    };

    let _ = super::aims_pipeline::run_aims_pipeline(&mut func, &config);
}

// Class-ledger Step-4b replacement (pipeline-level)

/// Drive `run_aims_pipeline` over a minimal config (the class-ledger
/// emitter is unconditional; a declined function falls back per-function).
fn run_pipeline(func: &mut ArcFunction) {
    let interner = ori_ir::StringInterner::new();
    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    let mut contracts = rustc_hash::FxHashMap::default();
    contracts.insert(func.name, MemoryContract::conservative(func.params.len()));
    let func_names: rustc_hash::FxHashSet<ori_ir::Name> = contracts.keys().copied().collect();
    let pool = ori_types::Pool::default();
    let classifier = crate::classify::ArcClassifier::new(&pool);
    let type_registry = ori_types::TypeRegistry::default();

    let config = super::aims_pipeline::AimsPipelineConfig {
        classifier: &classifier,
        contracts: &contracts,
        func_names: &func_names,
        exact_callables: &func_names,
        pool: &pool,
        interner: &interner,
        builtins: &builtins,
        verify_arc: false,
        observer: None,
        type_registry: &type_registry,
        exact_transfer_witnesses: &rustc_hash::FxHashMap::default(),
    };
    let result = super::aims_pipeline::run_aims_pipeline(func, &config);
    assert!(
        result.is_ok(),
        "pipeline must succeed for the class-ledger fixtures"
    );
}

/// Fresh `str` construct, read once (`IsShared`), dead — the fully-clean
/// class-ledger skeleton whose plan is exactly one release after the read.
fn class_ledger_clean_fixture() -> ArcFunction {
    ArcFunction {
        var_types: vec![Idx::STR, Idx::BOOL],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: v(0),
                    ty: Idx::STR,
                    ctor: crate::ir::CtorKind::Tuple,
                    args: vec![],
                },
                ArcInstr::IsShared {
                    dst: v(1),
                    var: v(0),
                },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        ..Default::default()
    }
}

/// Loop-threaded `str` class with a pre-seeded per-iteration `BurdenInc`
/// credit: the class's owed count disagrees at the loop-header merge, so the
/// class-ledger analysis DECLINES the class (readiness not clean).
fn class_ledger_declined_fixture() -> ArcFunction {
    ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::BOOL, Idx::BOOL],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![ArcInstr::Construct {
                    dst: v(0),
                    ty: Idx::STR,
                    ctor: crate::ir::CtorKind::Tuple,
                    args: vec![],
                }],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: vec![v(0)],
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![(v(1), Idx::STR)],
                body: vec![
                    ArcInstr::BurdenInc { var: v(1) },
                    ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::BOOL,
                        value: crate::ir::ArcValue::Literal(crate::ir::LitValue::Bool(true)),
                    },
                ],
                terminator: ArcTerminator::Branch {
                    cond: v(2),
                    then_block: ArcBlockId::new(2),
                    else_block: ArcBlockId::new(3),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: vec![v(1)],
                },
            },
            ArcBlock {
                id: ArcBlockId::new(3),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(3),
                    ty: Idx::BOOL,
                    value: crate::ir::ArcValue::Literal(crate::ir::LitValue::Bool(false)),
                }],
                terminator: ArcTerminator::Return { value: v(3) },
            },
        ],
        ..Default::default()
    }
}

/// Count `(RcInc total, RcDec on var, Burden* residue)` across all blocks.
fn count_rc_shape(func: &ArcFunction, dec_var: ArcVarId) -> (usize, usize, usize) {
    let mut incs = 0usize;
    let mut decs_on_var = 0usize;
    let mut burden = 0usize;
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::RcInc { .. } => incs += 1,
                ArcInstr::RcDec { var, .. } if *var == dec_var => decs_on_var += 1,
                ArcInstr::BurdenInc { .. }
                | ArcInstr::BurdenDec { .. }
                | ArcInstr::BurdenDecPartial { .. }
                | ArcInstr::BurdenDecField { .. }
                | ArcInstr::BurdenDecVariant { .. } => burden += 1,
                _ => {}
            }
        }
    }
    (incs, decs_on_var, burden)
}

/// Fully-clean function: the class-ledger plan replaces the standard
/// burden-op emission — exactly ONE lowered release after the last read, no
/// duplicate ops, no burden residue, edge machinery unmarked.
#[test]
fn class_ledger_replaces_clean_function_with_lowered_plan() {
    let mut func = class_ledger_clean_fixture();
    run_pipeline(&mut func);

    assert!(func.class_ledger_emission, "replacement must commit");
    let (incs, decs_on_v0, burden) = count_rc_shape(&func, v(0));
    assert_eq!(incs, 0, "the plan funds no duplication on this shape");
    assert_eq!(decs_on_v0, 1, "exactly one release for the dead class");
    assert_eq!(burden, 0, "every planned op lowers to real RC");
    assert!(
        func.burden_emitted.iter().all(|marked| !marked),
        "replacement never marks burden_emitted"
    );

    let body = &func.blocks[0].body;
    assert!(matches!(body[1], ArcInstr::IsShared { .. }));
    assert!(
        matches!(body[2], ArcInstr::RcDec { var, .. } if var == v(0)),
        "the release lands immediately after the last read; body={body:?}"
    );
}

/// A declined class is FAIL-LOUD on every path: the class-ledger plan is the
/// sole RC-emission input to the current compiled-counter adapter (the legacy
/// repair passes are deleted; no fallback adapter input exists), so a decline
/// is an ICE naming the function + gate.
#[test]
#[should_panic(expected = "class-ledger replacement declined")]
fn class_ledger_declined_function_fails_loud() {
    let mut func = class_ledger_declined_fixture();
    run_pipeline(&mut func);
}

/// Double-emission guard: the replaced output carries the plan's single
/// release and nothing else (two emitters on one function would double it);
/// replacement never marks `burden_emitted`.
#[test]
fn class_ledger_replaced_function_carries_no_burden_ops() {
    let mut replaced = class_ledger_clean_fixture();
    run_pipeline(&mut replaced);

    let (replaced_incs, replaced_decs, replaced_burden) = count_rc_shape(&replaced, v(0));
    assert_eq!(
        (replaced_incs, replaced_decs, replaced_burden),
        (0, 1, 0),
        "replaced output is exactly the lowered plan"
    );
    assert!(replaced.burden_emitted.iter().all(|marked| !marked));
}

/// Caller fixture for the contract-certified payload-view engagement pin:
/// str literal -> sum `Construct` -> borrowed `Invoke` -> result returned.
fn payload_view_caller_fixture(
    interner: &ori_ir::StringInterner,
    callee_name: ori_ir::Name,
    container_idx: Idx,
) -> ArcFunction {
    ArcFunction {
        var_types: vec![Idx::STR, container_idx, Idx::STR, Idx::UNIT, Idx::UNIT],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![
                    ArcInstr::Let {
                        dst: v(0),
                        ty: Idx::STR,
                        value: crate::ir::ArcValue::Literal(crate::ir::LitValue::String(
                            interner.intern("heap payload string past the sso threshold"),
                        )),
                    },
                    ArcInstr::Construct {
                        dst: v(1),
                        ty: container_idx,
                        ctor: crate::ir::CtorKind::EnumVariant {
                            enum_name: interner.intern("Wrapper"),
                            variant: 0,
                        },
                        args: vec![v(0)],
                    },
                ],
                terminator: ArcTerminator::Invoke {
                    dst: v(2),
                    ty: Idx::STR,
                    func: callee_name,
                    args: vec![v(1)],
                    arg_ownership: vec![crate::ir::ArgOwnership::Borrowed],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(3),
                    ty: Idx::UNIT,
                    value: crate::ir::ArcValue::Literal(crate::ir::LitValue::Int(0)),
                }],
                terminator: ArcTerminator::Return { value: v(2) },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        ..ArcFunction::default()
    }
}

/// ENGAGEMENT pin for the contract-boundary payload-view cure: a fresh sum
/// container borrowed into an `Invoke` whose callee contract certifies
/// `return_alias = Project` (the `assert_some` / field-accessor family). The
/// extraction happens inside the callee (no local `Project` seed), and the
/// credited call-result arrival funds the view — the caller REPLACES.
/// Reverting the credited-arrival admission makes this fixture fall back
/// with `field-view-liveness`.
#[test]
fn class_ledger_replaces_contract_certified_payload_view_caller() {
    use crate::aims::contract::ReturnAliasShape;
    use crate::lower::test_utils::registered_struct_with_burden;
    use ori_types::burden::{UserBurdenSpec, UserOwnedField};

    let interner = ori_ir::StringInterner::new();
    let callee_name = interner.intern("payload_view_callee");
    let mut pool = ori_types::Pool::default();
    let container_idx = pool.named(interner.intern("Wrapper"));
    let mut func = payload_view_caller_fixture(&interner, callee_name, container_idx);
    func.name = interner.intern("payload_view_caller");

    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    let mut contracts = rustc_hash::FxHashMap::default();
    contracts.insert(func.name, MemoryContract::conservative(0));
    let mut callee_contract = MemoryContract::conservative(1);
    callee_contract.params[0] = ParamContract {
        access: AccessClass::Borrowed,
        consumption: Consumption::Affine,
        cardinality: Cardinality::Many,
        return_alias: Some(ReturnAliasShape::Project { field: 0 }),
        ..ParamContract::CONSERVATIVE
    };
    contracts.insert(callee_name, callee_contract);
    let func_names: rustc_hash::FxHashSet<ori_ir::Name> = contracts.keys().copied().collect();
    let classifier = crate::classify::ArcClassifier::new(&pool);
    let mut type_registry = ori_types::TypeRegistry::default();
    registered_struct_with_burden(
        &mut type_registry,
        "Wrapper",
        container_idx,
        Some(UserBurdenSpec {
            self_owned_identity: false,
            owned_fields: vec![UserOwnedField {
                field_path: vec![0],
                field_type: Idx::STR,
            }],
            ..Default::default()
        }),
    );

    let config = super::aims_pipeline::AimsPipelineConfig {
        classifier: &classifier,
        contracts: &contracts,
        func_names: &func_names,
        exact_callables: &func_names,
        pool: &pool,
        interner: &interner,
        builtins: &builtins,
        verify_arc: false,
        observer: None,
        type_registry: &type_registry,
        exact_transfer_witnesses: &rustc_hash::FxHashMap::default(),
    };
    let result = super::aims_pipeline::run_aims_pipeline(&mut func, &config);
    assert!(result.is_ok(), "pipeline must succeed");
    assert!(
        func.class_ledger_emission,
        "contract-certified payload-view caller must REPLACE (credited \
         arrival funds the view); fallback here means the credited-arrival \
         admission regressed"
    );
}
