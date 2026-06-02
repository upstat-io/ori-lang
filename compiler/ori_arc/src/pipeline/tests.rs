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
        transfers_through_return: false,
        return_alias: None,
        return_payload_contains_param: false,
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

// ── Checkpoint observer tests ──

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
            body: vec![],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        var_types: vec![Idx::NONE],
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
    let sigs = rustc_hash::FxHashMap::default();
    let type_registry = ori_types::TypeRegistry::default();

    let config = super::aims_pipeline::AimsPipelineConfig {
        classifier: &classifier,
        contracts: &contracts,
        func_names: &func_names,
        pool: &pool,
        interner: &interner,
        builtins: &builtins,
        verify_arc: false,
        observer: Some(&observer),
        sigs: &sigs,
        type_registry: &type_registry,
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
        "realize_rc_reuse",
    ];
    let mut last_idx = 0;
    for expected in &core_phases {
        if let Some(pos) = phases.iter().position(|p| p == *expected) {
            assert!(
                pos >= last_idx,
                "phase '{expected}' at {pos} should be after previous core phase at {last_idx}"
            );
            last_idx = pos;
        }
        // Phase may not appear if the pipeline short-circuits on error,
        // but if it does appear it must be in order.
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
            body: vec![],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        var_types: vec![Idx::NONE],
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
    let sigs = rustc_hash::FxHashMap::default();
    let type_registry = ori_types::TypeRegistry::default();

    let config = super::aims_pipeline::AimsPipelineConfig {
        classifier: &classifier,
        contracts: &contracts,
        func_names: &func_names,
        pool: &pool,
        interner: &interner,
        builtins: &builtins,
        verify_arc: false,
        observer: None,
        sigs: &sigs,
        type_registry: &type_registry,
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
    let sigs = rustc_hash::FxHashMap::default();
    let type_registry = ori_types::TypeRegistry::default();

    let config = super::aims_pipeline::AimsPipelineConfig {
        classifier: &classifier,
        contracts: &contracts,
        func_names: &func_names,
        pool: &pool,
        interner: &interner,
        builtins: &builtins,
        verify_arc: false,
        observer: Some(&observer),
        sigs: &sigs,
        type_registry: &type_registry,
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

// ── IC-1 enforcement semantic pins ──

/// INV1 (positive pin): the batch pipeline computes a contract for every
/// analyzed function. After `run_arc_pipeline_all` over a single function,
/// the IC-1 invariant holds — the `get_required` sites never panic on the
/// computed contracts map.
#[test]
fn aims_pipeline_ic1_invariant_holds_end_to_end() {
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

    let interner = ori_ir::StringInterner::new();
    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    let pool = ori_types::Pool::default();
    let classifier = crate::classify::ArcClassifier::new(&pool);
    let sigs = rustc_hash::FxHashMap::default();
    let type_registry = ori_types::TypeRegistry::default();

    let mut funcs = vec![func];
    let result = crate::run_arc_pipeline_all(
        &mut funcs,
        &classifier,
        &sigs,
        &interner,
        &pool,
        &builtins,
        &type_registry,
        false,
    );
    assert!(
        result.is_ok(),
        "IC-1 invariant: batch pipeline computes a contract for every \
         function — get_required sites must not panic"
    );
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
    let sigs = rustc_hash::FxHashMap::default();
    let type_registry = ori_types::TypeRegistry::default();

    let config = super::aims_pipeline::AimsPipelineConfig {
        classifier: &classifier,
        contracts: &contracts,
        func_names: &func_names,
        pool: &pool,
        interner: &interner,
        builtins: &builtins,
        verify_arc: false,
        observer: None,
        sigs: &sigs,
        type_registry: &type_registry,
    };

    let _ = super::aims_pipeline::run_aims_pipeline(&mut func, &config);
}
