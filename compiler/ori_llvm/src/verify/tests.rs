//! Unit tests for the codegen audit pass using synthetic inkwell IR.

use inkwell::context::Context;
use inkwell::AddressSpace;

use super::report::{FindingKind, Severity};
use super::{audit_module, audit_module_with_options, AuditOptions, AuditReport};

/// Baseline: an empty module produces no findings.
#[test]
fn empty_module_clean() {
    let ctx = Context::create();
    let module = ctx.create_module("test_empty");
    let report = audit_module(&module);
    assert!(
        report.findings.is_empty(),
        "empty module should have no findings, got: {:?}",
        report.findings
    );
}

/// Baseline: a module with a simple function (no RC) produces no findings.
#[test]
fn simple_function_clean() {
    let ctx = Context::create();
    let module = ctx.create_module("test_simple");

    let i64_type = ctx.i64_type();
    let fn_type = i64_type.fn_type(&[i64_type.into()], false);
    let func = module.add_function("add_one", fn_type, None);
    let entry = ctx.append_basic_block(func, "entry");
    let builder = ctx.create_builder();
    builder.position_at_end(entry);

    let param = func.get_nth_param(0).unwrap().into_int_value();
    let one = i64_type.const_int(1, false);
    let result = builder.build_int_add(param, one, "result").unwrap();
    builder.build_return(Some(&result)).unwrap();

    let report = audit_module(&module);
    assert!(
        report.findings.is_empty(),
        "simple add function should be clean, got: {:?}",
        report.findings
    );
}

// ── ABI checks ──────────────────────────────────────────────────────

/// 04.3A: Large aggregate load (>16 bytes) produces a warning.
#[test]
fn large_aggregate_load_detected() {
    let ctx = Context::create();
    let module = ctx.create_module("test_large_load");

    // Create a struct with 3 i64 fields = 24 bytes
    let i64_type = ctx.i64_type();
    let big_struct = ctx.struct_type(&[i64_type.into(), i64_type.into(), i64_type.into()], false);

    let fn_type = big_struct.fn_type(&[], false);
    let func = module.add_function("load_big", fn_type, None);
    let entry = ctx.append_basic_block(func, "entry");
    let builder = ctx.create_builder();
    builder.position_at_end(entry);

    // alloca + load of the big struct
    let alloca = builder.build_alloca(big_struct, "tmp").unwrap();
    let _loaded = builder.build_load(big_struct, alloca, "big_val").unwrap();
    let undef = big_struct.get_undef();
    builder.build_return(Some(&undef)).unwrap();

    let report = audit_module(&module);
    let large_loads: Vec<_> = report
        .findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::LargeAggregateLoad { .. }))
        .collect();
    assert!(
        !large_loads.is_empty(),
        "should detect large aggregate load"
    );
    assert_eq!(large_loads[0].severity, Severity::Warning);
    if let FindingKind::LargeAggregateLoad { type_size_bytes } = &large_loads[0].kind {
        assert_eq!(*type_size_bytes, 24, "3 x i64 = 24 bytes");
    }
}

/// 04.3A: Small struct load (<=16 bytes) produces no warning.
#[test]
fn small_struct_load_clean() {
    let ctx = Context::create();
    let module = ctx.create_module("test_small_load");

    // Create a struct with 2 i64 fields = 16 bytes (at threshold)
    let i64_type = ctx.i64_type();
    let small_struct = ctx.struct_type(&[i64_type.into(), i64_type.into()], false);

    let fn_type = small_struct.fn_type(&[], false);
    let func = module.add_function("load_small", fn_type, None);
    let entry = ctx.append_basic_block(func, "entry");
    let builder = ctx.create_builder();
    builder.position_at_end(entry);

    let alloca = builder.build_alloca(small_struct, "tmp").unwrap();
    let _loaded = builder
        .build_load(small_struct, alloca, "small_val")
        .unwrap();
    let undef = small_struct.get_undef();
    builder.build_return(Some(&undef)).unwrap();

    let report = audit_module(&module);
    let large_loads: Vec<_> = report
        .findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::LargeAggregateLoad { .. }))
        .collect();
    assert!(
        large_loads.is_empty(),
        "16-byte struct should not trigger warning, got: {large_loads:?}"
    );
}

/// 04.3B: Calling a runtime function with wrong arg count produces an error.
#[test]
fn runtime_arg_count_mismatch_detected() {
    let ctx = Context::create();
    let module = ctx.create_module("test_arg_count");

    let ptr_type = ctx.ptr_type(AddressSpace::default());
    let i64_type = ctx.i64_type();

    // ori_rc_alloc expects (i64, i64) -> ptr.
    // Declare it correctly...
    let correct_fn_type = ptr_type.fn_type(&[i64_type.into(), i64_type.into()], false);
    let _rc_alloc = module.add_function("ori_rc_alloc", correct_fn_type, None);

    // ... but call it with wrong arity from a test function.
    // Declare a wrapper with 3 params to build a bad call.
    let wrong_fn_type =
        ptr_type.fn_type(&[i64_type.into(), i64_type.into(), i64_type.into()], false);
    let bad_alloc = module.add_function("ori_rc_alloc_bad", wrong_fn_type, None);

    // Create a caller that calls ori_rc_alloc with 3 args
    // We need a variadic function type or to abuse the calling convention.
    // Instead: declare ori_rc_alloc with the wrong type to force arg count mismatch.
    // Simpler approach: just create the module where ori_rc_alloc has wrong param count.
    let _ = bad_alloc;

    // Actually, we need to build a call instruction to ori_rc_alloc with wrong arg count.
    // The simplest way: redeclare with wrong signature, build the call.
    // inkwell won't let us add a second function with the same name. Instead,
    // let's use a module where the function itself has the wrong signature.

    // Start fresh with a mismatched declaration
    let module2 = ctx.create_module("test_arg_count2");
    let wrong_type = ptr_type.fn_type(&[i64_type.into(), i64_type.into(), i64_type.into()], false);
    let wrong_alloc = module2.add_function("ori_rc_alloc", wrong_type, None);

    let caller_type = ctx.void_type().fn_type(&[], false);
    let caller = module2.add_function("test_caller", caller_type, None);
    let entry = ctx.append_basic_block(caller, "entry");
    let builder = ctx.create_builder();
    builder.position_at_end(entry);

    let one = i64_type.const_int(1, false);
    builder
        .build_call(wrong_alloc, &[one.into(), one.into(), one.into()], "ptr")
        .unwrap();
    builder.build_return(None).unwrap();

    let report = audit_module(&module2);
    let mismatches: Vec<_> = report
        .findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::RuntimeArgCountMismatch { .. }))
        .collect();
    assert!(!mismatches.is_empty(), "should detect arg count mismatch");
    assert_eq!(mismatches[0].severity, Severity::Error);
}

/// 04.3C: nounwind function called via invoke produces a warning.
#[test]
fn nounwind_invoke_detected() {
    let ctx = Context::create();
    let module = ctx.create_module("test_nounwind_invoke");

    let ptr_type = ctx.ptr_type(AddressSpace::default());

    // ori_rc_inc is nounwind. Declare it.
    let rc_inc_type = ctx.void_type().fn_type(&[ptr_type.into()], false);
    let rc_inc = module.add_function("ori_rc_inc", rc_inc_type, None);

    // Create a caller that uses `invoke` instead of `call`
    let caller_type = ctx.void_type().fn_type(&[ptr_type.into()], false);
    let caller = module.add_function("test_caller", caller_type, None);
    let entry = ctx.append_basic_block(caller, "entry");
    let normal = ctx.append_basic_block(caller, "normal");
    let unwind = ctx.append_basic_block(caller, "unwind");

    let builder = ctx.create_builder();
    builder.position_at_end(entry);
    let param = caller.get_nth_param(0).unwrap().into_pointer_value();
    builder
        .build_invoke(rc_inc, &[param.into()], normal, unwind, "")
        .unwrap();

    builder.position_at_end(normal);
    builder.build_return(None).unwrap();

    // Build a landing pad for the unwind block
    builder.position_at_end(unwind);
    // Need an EH personality for the landing pad
    let personality_type = ctx.i32_type().fn_type(&[ctx.i32_type().into()], false);
    let personality = module.add_function("rust_eh_personality", personality_type, None);
    caller.set_personality_function(personality);
    let lp_type = ctx.struct_type(&[ptr_type.into(), ctx.i32_type().into()], false);
    let _lp = builder
        .build_landing_pad(lp_type, personality, &[], false, "lp")
        .unwrap();
    builder.build_return(None).unwrap();

    let report = audit_module(&module);
    let nounwind_invokes: Vec<_> = report
        .findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::NounwindCalledWithInvoke { .. }))
        .collect();
    assert!(
        !nounwind_invokes.is_empty(),
        "should detect nounwind function called via invoke"
    );
    assert_eq!(nounwind_invokes[0].severity, Severity::Warning);
}

// ── RC Balance checks ───────────────────────────────────────────────

/// 04.1: RC alloc without dec produces a leak warning.
#[test]
fn rc_leak_detected() {
    let ctx = Context::create();
    let module = ctx.create_module("test_rc_leak");

    let ptr_type = ctx.ptr_type(AddressSpace::default());
    let i64_type = ctx.i64_type();

    // Declare ori_rc_alloc
    let alloc_type = ptr_type.fn_type(&[i64_type.into(), i64_type.into()], false);
    let rc_alloc = module.add_function("ori_rc_alloc", alloc_type, None);

    // Function that allocs but never decs
    let fn_type = ctx.void_type().fn_type(&[], false);
    let func = module.add_function("leaky", fn_type, None);
    let entry = ctx.append_basic_block(func, "entry");
    let builder = ctx.create_builder();
    builder.position_at_end(entry);

    let one = i64_type.const_int(1, false);
    let _alloc = builder
        .build_call(rc_alloc, &[one.into(), one.into()], "leaked_ptr")
        .unwrap();
    builder.build_return(None).unwrap();

    let report = audit_module(&module);
    let leaks: Vec<_> = report
        .findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::RcLeak { .. }))
        .collect();
    assert!(!leaks.is_empty(), "should detect RC leak");
    assert_eq!(leaks[0].severity, Severity::Warning);
}

/// 04.1: Balanced alloc + dec produces no findings.
#[test]
fn rc_balanced_clean() {
    let ctx = Context::create();
    let module = ctx.create_module("test_rc_balanced");

    let ptr_type = ctx.ptr_type(AddressSpace::default());
    let i64_type = ctx.i64_type();

    // Declare ori_rc_alloc and ori_rc_dec
    let alloc_type = ptr_type.fn_type(&[i64_type.into(), i64_type.into()], false);
    let rc_alloc = module.add_function("ori_rc_alloc", alloc_type, None);

    let dec_type = ctx
        .void_type()
        .fn_type(&[ptr_type.into(), ptr_type.into()], false);
    let rc_dec = module.add_function("ori_rc_dec", dec_type, None);

    // Function that allocs and properly decs
    let fn_type = ctx.void_type().fn_type(&[], false);
    let func = module.add_function("balanced", fn_type, None);
    let entry = ctx.append_basic_block(func, "entry");
    let builder = ctx.create_builder();
    builder.position_at_end(entry);

    let one = i64_type.const_int(1, false);
    let alloc_result = builder
        .build_call(rc_alloc, &[one.into(), one.into()], "ptr")
        .unwrap();

    // Build dec call using the allocated pointer
    let ptr_val = alloc_result
        .try_as_basic_value()
        .basic()
        .unwrap()
        .into_pointer_value();
    let null = ptr_type.const_null();
    builder
        .build_call(rc_dec, &[ptr_val.into(), null.into()], "")
        .unwrap();
    builder.build_return(None).unwrap();

    let report = audit_module(&module);
    let rc_findings: Vec<_> = report
        .findings
        .iter()
        .filter(|f| {
            matches!(
                f.kind,
                FindingKind::RcLeak { .. } | FindingKind::RcDecAfterCow { .. }
            )
        })
        .collect();
    assert!(
        rc_findings.is_empty(),
        "balanced alloc+dec should be clean, got: {rc_findings:?}"
    );
}

// ── COW Rule checks ─────────────────────────────────────────────────

/// 04.2 Rule 3: Dec before COW produces an error.
#[test]
fn cow_dec_before_call_detected() {
    let ctx = Context::create();
    let module = ctx.create_module("test_cow_dec_before");

    let ptr_type = ctx.ptr_type(AddressSpace::default());
    let i64_type = ctx.i64_type();

    // Declare ori_rc_dec and ori_list_push_cow
    let dec_type = ctx
        .void_type()
        .fn_type(&[ptr_type.into(), ptr_type.into()], false);
    let rc_dec = module.add_function("ori_rc_dec", dec_type, None);

    // ori_list_push_cow: (ptr, i64, i64, ptr, i64, i64, ptr) -> void
    let cow_type = ctx.void_type().fn_type(
        &[
            ptr_type.into(),
            i64_type.into(),
            i64_type.into(),
            ptr_type.into(),
            i64_type.into(),
            i64_type.into(),
            ptr_type.into(),
        ],
        false,
    );
    let push_cow = module.add_function("ori_list_push_cow", cow_type, None);

    // Function: dec then cow
    let fn_type = ctx.void_type().fn_type(&[ptr_type.into()], false);
    let func = module.add_function("bad_cow", fn_type, None);
    let entry = ctx.append_basic_block(func, "entry");
    let builder = ctx.create_builder();
    builder.position_at_end(entry);

    let data_ptr = func.get_nth_param(0).unwrap().into_pointer_value();
    data_ptr.set_name("data_ptr");
    let null = ptr_type.const_null();
    let one = i64_type.const_int(1, false);

    // Dec first (bad!)
    builder
        .build_call(rc_dec, &[data_ptr.into(), null.into()], "")
        .unwrap();

    // Then COW call with the decremented pointer
    builder
        .build_call(
            push_cow,
            &[
                data_ptr.into(),
                one.into(),
                one.into(),
                null.into(),
                one.into(),
                one.into(),
                null.into(),
            ],
            "",
        )
        .unwrap();
    builder.build_return(None).unwrap();

    let report = audit_module(&module);
    let cow_findings: Vec<_> = report
        .findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::CowInputDecBeforeCall { .. }))
        .collect();
    assert!(!cow_findings.is_empty(), "should detect dec before COW");
    assert_eq!(cow_findings[0].severity, Severity::Error);
}

// ── Safety check density ────────────────────────────────────────────

/// 04.4: Empty function produces no safety findings.
#[test]
fn safety_empty_function_clean() {
    let ctx = Context::create();
    let module = ctx.create_module("test_safety_empty");

    let fn_type = ctx.void_type().fn_type(&[], false);
    let func = module.add_function("empty_fn", fn_type, None);
    let entry = ctx.append_basic_block(func, "entry");
    let builder = ctx.create_builder();
    builder.position_at_end(entry);
    builder.build_return(None).unwrap();

    let report = audit_module(&module);
    let safety: Vec<_> = report
        .findings
        .iter()
        .filter(|f| {
            matches!(
                f.kind,
                FindingKind::SafetyCheckCall { .. }
                    | FindingKind::SafetyCheckBranch { .. }
                    | FindingKind::SafetyCheckSummary { .. }
            )
        })
        .collect();
    assert!(
        safety.is_empty(),
        "empty function should have no safety findings, got: {safety:?}"
    );
}

/// 04.4: Direct call to `ori_panic` is detected as a safety check.
#[test]
fn safety_panic_call_detected() {
    let ctx = Context::create();
    let module = ctx.create_module("test_safety_panic");

    let ptr_type = ctx.ptr_type(AddressSpace::default());

    // Declare ori_panic
    let panic_type = ctx.void_type().fn_type(&[ptr_type.into()], false);
    let ori_panic = module.add_function("ori_panic", panic_type, None);

    // Function that calls ori_panic
    let fn_type = ctx.void_type().fn_type(&[ptr_type.into()], false);
    let func = module.add_function("panicking", fn_type, None);
    let entry = ctx.append_basic_block(func, "entry");
    let builder = ctx.create_builder();
    builder.position_at_end(entry);

    let param = func.get_nth_param(0).unwrap().into_pointer_value();
    builder.build_call(ori_panic, &[param.into()], "").unwrap();
    builder.build_return(None).unwrap();

    let report = audit_module(&module);
    let calls: Vec<_> = report
        .findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::SafetyCheckCall { .. }))
        .collect();
    assert!(!calls.is_empty(), "should detect ori_panic call");
    assert_eq!(calls[0].severity, Severity::Note);
    if let FindingKind::SafetyCheckCall { callee } = &calls[0].kind {
        assert_eq!(callee, "ori_panic");
    }

    // Should also have a summary
    let summaries: Vec<_> = report
        .findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::SafetyCheckSummary { .. }))
        .collect();
    assert!(!summaries.is_empty(), "should have safety summary");
}

/// 04.4: `ori_assert_eq_int` is detected as a safety check.
#[test]
fn safety_assert_eq_int_detected() {
    let ctx = Context::create();
    let module = ctx.create_module("test_safety_assert");

    let i64_type = ctx.i64_type();

    // Declare ori_assert_eq_int
    let assert_type = ctx
        .void_type()
        .fn_type(&[i64_type.into(), i64_type.into()], false);
    let assert_fn = module.add_function("ori_assert_eq_int", assert_type, None);

    // Function that calls ori_assert_eq_int
    let fn_type = ctx.void_type().fn_type(&[], false);
    let func = module.add_function("asserting", fn_type, None);
    let entry = ctx.append_basic_block(func, "entry");
    let builder = ctx.create_builder();
    builder.position_at_end(entry);

    let one = i64_type.const_int(1, false);
    builder
        .build_call(assert_fn, &[one.into(), one.into()], "")
        .unwrap();
    builder.build_return(None).unwrap();

    let report = audit_module(&module);
    let calls: Vec<_> = report
        .findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::SafetyCheckCall { .. }))
        .collect();
    assert!(!calls.is_empty(), "should detect ori_assert_eq_int");
    if let FindingKind::SafetyCheckCall { callee } = &calls[0].kind {
        assert_eq!(callee, "ori_assert_eq_int");
    }
}

/// 04.4: Conditional branch to a panic block is detected.
#[test]
fn safety_conditional_branch_to_panic_block() {
    let ctx = Context::create();
    let module = ctx.create_module("test_safety_branch");

    let ptr_type = ctx.ptr_type(AddressSpace::default());

    // Declare ori_panic
    let panic_type = ctx.void_type().fn_type(&[ptr_type.into()], false);
    let ori_panic = module.add_function("ori_panic", panic_type, None);

    // Function: branch cond ? normal : panic_bb
    let fn_type = ctx
        .void_type()
        .fn_type(&[ctx.bool_type().into(), ptr_type.into()], false);
    let func = module.add_function("guarded", fn_type, None);
    let entry = ctx.append_basic_block(func, "entry");
    let panic_bb = ctx.append_basic_block(func, "panic_bb");
    let cont_bb = ctx.append_basic_block(func, "continue");

    let builder = ctx.create_builder();

    // Entry: conditional branch
    builder.position_at_end(entry);
    let cond = func.get_nth_param(0).unwrap().into_int_value();
    let msg = func.get_nth_param(1).unwrap().into_pointer_value();
    builder
        .build_conditional_branch(cond, cont_bb, panic_bb)
        .unwrap();

    // Panic block: call ori_panic + unreachable
    builder.position_at_end(panic_bb);
    builder.build_call(ori_panic, &[msg.into()], "").unwrap();
    builder.build_unreachable().unwrap();

    // Continue: return
    builder.position_at_end(cont_bb);
    builder.build_return(None).unwrap();

    let report = audit_module(&module);

    // Should detect the branch to panic_bb
    let branches: Vec<_> = report
        .findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::SafetyCheckBranch { .. }))
        .collect();
    assert!(
        !branches.is_empty(),
        "should detect conditional branch to panic block"
    );
    assert_eq!(branches[0].severity, Severity::Note);
    if let FindingKind::SafetyCheckBranch { panic_block } = &branches[0].kind {
        assert_eq!(panic_block, "panic_bb");
    }
}

/// 04.4: Density calculation is correct.
#[test]
fn safety_density_calculation() {
    let ctx = Context::create();
    let module = ctx.create_module("test_safety_density");

    let ptr_type = ctx.ptr_type(AddressSpace::default());

    // Declare ori_panic
    let panic_type = ctx.void_type().fn_type(&[ptr_type.into()], false);
    let ori_panic = module.add_function("ori_panic", panic_type, None);

    // Function with known instruction count: alloca + call + ret = 3 instructions
    // 1 safety check → density = 1/3 * 100 ≈ 33.3
    let fn_type = ctx.void_type().fn_type(&[], false);
    let func = module.add_function("density_test", fn_type, None);
    let entry = ctx.append_basic_block(func, "entry");
    let builder = ctx.create_builder();
    builder.position_at_end(entry);

    let alloca = builder.build_alloca(ptr_type, "tmp").unwrap();
    builder.build_call(ori_panic, &[alloca.into()], "").unwrap();
    builder.build_return(None).unwrap();

    let report = audit_module(&module);
    let summaries: Vec<_> = report
        .findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::SafetyCheckSummary { .. }))
        .collect();
    assert!(!summaries.is_empty(), "should have density summary");
    if let FindingKind::SafetyCheckSummary {
        check_count,
        instruction_count,
    } = &summaries[0].kind
    {
        assert_eq!(*check_count, 1);
        assert_eq!(*instruction_count, 3); // alloca + call + ret
                                           // density = 1/3 * 100 ≈ 33.3 — verified via description string
        assert!(
            summaries[0].description.contains("33.3"),
            "density description should contain 33.3, got: {}",
            summaries[0].description
        );
    }
}

/// 04.4: Function filter applies to safety checks.
#[test]
fn safety_function_filter_applies() {
    let ctx = Context::create();
    let module = ctx.create_module("test_safety_filter");

    let ptr_type = ctx.ptr_type(AddressSpace::default());

    // Declare ori_panic
    let panic_type = ctx.void_type().fn_type(&[ptr_type.into()], false);
    let ori_panic = module.add_function("ori_panic", panic_type, None);

    // Two functions with panic calls
    for name in &["target_fn", "other_fn"] {
        let fn_type = ctx.void_type().fn_type(&[ptr_type.into()], false);
        let func = module.add_function(name, fn_type, None);
        let entry = ctx.append_basic_block(func, "entry");
        let builder = ctx.create_builder();
        builder.position_at_end(entry);

        let param = func.get_nth_param(0).unwrap().into_pointer_value();
        builder.build_call(ori_panic, &[param.into()], "").unwrap();
        builder.build_return(None).unwrap();
    }

    // With filter: only target_fn
    let opts = AuditOptions {
        strict: false,
        function_filter: Some("target".to_owned()),
    };
    let report = audit_module_with_options(&module, &opts);
    let calls: Vec<_> = report
        .findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::SafetyCheckCall { .. }))
        .collect();
    assert_eq!(calls.len(), 1, "filter should only match target_fn");
    assert_eq!(calls[0].function_name, "target_fn");
}

/// 04.4: Unconditional branch to a panic block is NOT a safety check branch.
#[test]
fn safety_unconditional_branch_not_detected() {
    let ctx = Context::create();
    let module = ctx.create_module("test_safety_uncond");

    let ptr_type = ctx.ptr_type(AddressSpace::default());

    // Declare ori_panic
    let panic_type = ctx.void_type().fn_type(&[ptr_type.into()], false);
    let ori_panic = module.add_function("ori_panic", panic_type, None);

    // Function: entry unconditionally jumps to panic block
    let fn_type = ctx.void_type().fn_type(&[ptr_type.into()], false);
    let func = module.add_function("always_panic", fn_type, None);
    let entry = ctx.append_basic_block(func, "entry");
    let panic_bb = ctx.append_basic_block(func, "panic_bb");

    let builder = ctx.create_builder();

    // Entry: unconditional branch to panic block
    builder.position_at_end(entry);
    builder.build_unconditional_branch(panic_bb).unwrap();

    // Panic block: call ori_panic + unreachable
    builder.position_at_end(panic_bb);
    let param = func.get_nth_param(0).unwrap().into_pointer_value();
    builder.build_call(ori_panic, &[param.into()], "").unwrap();
    builder.build_unreachable().unwrap();

    let report = audit_module(&module);

    // Should detect the call but NOT a branch finding
    let branches: Vec<_> = report
        .findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::SafetyCheckBranch { .. }))
        .collect();
    assert!(
        branches.is_empty(),
        "unconditional branch should NOT be a safety check branch, got: {branches:?}"
    );

    // But should still detect the call itself
    let calls: Vec<_> = report
        .findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::SafetyCheckCall { .. }))
        .collect();
    assert!(
        !calls.is_empty(),
        "should still detect the call to ori_panic"
    );
}

/// Report summary methods work correctly.
#[test]
fn report_summary() {
    let mut report = AuditReport::new();
    assert!(!report.has_errors());
    assert_eq!(report.error_count(), 0);

    report.add(super::report::AuditFinding {
        function_name: "test".to_owned(),
        severity: Severity::Warning,
        kind: FindingKind::RcLeak {
            alloc_site: "ptr".to_owned(),
        },
        description: "test warning".to_owned(),
    });
    assert!(!report.has_errors());

    report.add(super::report::AuditFinding {
        function_name: "test".to_owned(),
        severity: Severity::Error,
        kind: FindingKind::RuntimeArgCountMismatch {
            fn_name: "ori_rc_alloc".to_owned(),
            expected: 2,
            actual: 3,
        },
        description: "test error".to_owned(),
    });
    assert!(report.has_errors());
    assert_eq!(report.error_count(), 1);
    assert_eq!(report.findings.len(), 2);
}

// ── Strict mode checks ────────────────────────────────────────────

/// Strict mode: COW + dec produces `RcDoubleDec` error (not just `RcDecAfterCow` warning).
#[test]
fn strict_mode_cow_then_dec_is_double_dec() {
    let ctx = Context::create();
    let module = ctx.create_module("test_strict_cow");

    let ptr_type = ctx.ptr_type(AddressSpace::default());
    let i64_type = ctx.i64_type();

    // Declare ori_rc_alloc, ori_rc_dec, and a COW function
    let alloc_type = ptr_type.fn_type(&[i64_type.into(), i64_type.into()], false);
    let rc_alloc = module.add_function("ori_rc_alloc", alloc_type, None);

    let dec_type = ctx
        .void_type()
        .fn_type(&[ptr_type.into(), ptr_type.into()], false);
    let rc_dec = module.add_function("ori_rc_dec", dec_type, None);

    let cow_type = ctx.void_type().fn_type(
        &[
            ptr_type.into(),
            i64_type.into(),
            i64_type.into(),
            ptr_type.into(),
            i64_type.into(),
            i64_type.into(),
            ptr_type.into(),
        ],
        false,
    );
    let push_cow = module.add_function("ori_list_push_cow", cow_type, None);

    // Function: alloc → COW → dec (the dec is a double-free in strict mode)
    let fn_type = ctx.void_type().fn_type(&[], false);
    let func = module.add_function("strict_test", fn_type, None);
    let entry = ctx.append_basic_block(func, "entry");
    let builder = ctx.create_builder();
    builder.position_at_end(entry);

    let one = i64_type.const_int(1, false);
    let alloc_result = builder
        .build_call(rc_alloc, &[one.into(), one.into()], "data_ptr")
        .unwrap();
    let data_ptr = alloc_result
        .try_as_basic_value()
        .basic()
        .unwrap()
        .into_pointer_value();

    let null = ptr_type.const_null();

    // COW consumes the pointer
    builder
        .build_call(
            push_cow,
            &[
                data_ptr.into(),
                one.into(),
                one.into(),
                null.into(),
                one.into(),
                one.into(),
                null.into(),
            ],
            "",
        )
        .unwrap();

    // Dec after COW
    builder
        .build_call(rc_dec, &[data_ptr.into(), null.into()], "")
        .unwrap();
    builder.build_return(None).unwrap();

    // Normal mode: RcDecAfterCow warning
    let report = audit_module(&module);
    let cow_warnings: Vec<_> = report
        .findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::RcDecAfterCow { .. }))
        .collect();
    assert!(
        !cow_warnings.is_empty(),
        "normal mode should warn about dec after COW"
    );
    assert_eq!(cow_warnings[0].severity, Severity::Warning);

    // Strict mode: RcDoubleDec error
    let strict_opts = AuditOptions {
        strict: true,
        function_filter: None,
    };
    let strict_report = audit_module_with_options(&module, &strict_opts);
    let double_decs: Vec<_> = strict_report
        .findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::RcDoubleDec { .. }))
        .collect();
    assert!(
        !double_decs.is_empty(),
        "strict mode should detect double-dec after COW"
    );
    assert_eq!(double_decs[0].severity, Severity::Error);
}

/// Strict mode: leak on function pointer parameter produces an error.
#[test]
fn strict_mode_param_leak_detected() {
    let ctx = Context::create();
    let module = ctx.create_module("test_strict_param_leak");

    let ptr_type = ctx.ptr_type(AddressSpace::default());

    // Function takes a pointer param but never dec's it
    let fn_type = ctx.void_type().fn_type(&[ptr_type.into()], false);
    let func = module.add_function("leaky_param", fn_type, None);
    let entry = ctx.append_basic_block(func, "entry");
    let builder = ctx.create_builder();
    builder.position_at_end(entry);

    let param = func.get_nth_param(0).unwrap().into_pointer_value();
    param.set_name("param_ptr");
    builder.build_return(None).unwrap();

    // Normal mode: no findings (params not tracked)
    let report = audit_module(&module);
    assert!(
        report.findings.is_empty(),
        "normal mode should not track params, got: {:?}",
        report.findings
    );

    // Strict mode: param leak detected
    let strict_opts = AuditOptions {
        strict: true,
        function_filter: None,
    };
    let strict_report = audit_module_with_options(&module, &strict_opts);
    let leaks: Vec<_> = strict_report
        .findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::RcLeak { .. }))
        .collect();
    assert!(!leaks.is_empty(), "strict mode should detect param leak");
    assert_eq!(leaks[0].severity, Severity::Error);
}

// ── Function filter checks ─────────────────────────────────────────

/// Function filter: only audits matching functions.
#[test]
fn function_filter_skips_non_matching() {
    let ctx = Context::create();
    let module = ctx.create_module("test_filter");

    let ptr_type = ctx.ptr_type(AddressSpace::default());
    let i64_type = ctx.i64_type();

    // Declare ori_rc_alloc
    let alloc_type = ptr_type.fn_type(&[i64_type.into(), i64_type.into()], false);
    let rc_alloc = module.add_function("ori_rc_alloc", alloc_type, None);

    // Two leaky functions: "target_fn" and "other_fn"
    let fn_type = ctx.void_type().fn_type(&[], false);
    let one = i64_type.const_int(1, false);

    for name in &["target_fn", "other_fn"] {
        let func = module.add_function(name, fn_type, None);
        let entry = ctx.append_basic_block(func, "entry");
        let builder = ctx.create_builder();
        builder.position_at_end(entry);
        let _alloc = builder
            .build_call(rc_alloc, &[one.into(), one.into()], "leaked")
            .unwrap();
        builder.build_return(None).unwrap();
    }

    // Without filter: both leaks detected
    let report = audit_module(&module);
    let leaks: Vec<_> = report
        .findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::RcLeak { .. }))
        .collect();
    assert_eq!(leaks.len(), 2, "both functions should report leaks");

    // With filter for "target": only target_fn leak
    let filtered_opts = AuditOptions {
        strict: false,
        function_filter: Some("target".to_owned()),
    };
    let filtered_report = audit_module_with_options(&module, &filtered_opts);
    let filtered_leaks: Vec<_> = filtered_report
        .findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::RcLeak { .. }))
        .collect();
    assert_eq!(
        filtered_leaks.len(),
        1,
        "filter should only match target_fn"
    );
    assert_eq!(filtered_leaks[0].function_name, "target_fn");
}
