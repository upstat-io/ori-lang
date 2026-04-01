//! IR Quality Tests: CFG Simplification
//!
//! Verify that the post-emission CFG simplification pass eliminates
//! empty trampoline blocks and redundant conditional branches.

use crate::util::{compile_and_capture_ir, count_bridge_blocks, extract_function_ir};

/// J2 branching: `my_abs` should have no bridge blocks after CFG simplification.
///
/// Before CFG simplification, the overflow check in negation created an empty
/// trampoline block. The pass should eliminate it.
#[test]
fn test_branching_my_abs_no_bridge_blocks() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_cfg_simplify/branching_my_abs_no_bridge_blocks.ori"
    ));

    let fn_ir = extract_function_ir(&ir, "_ori_my_abs");
    let bridges = count_bridge_blocks(fn_ir);

    assert_eq!(
        bridges, 0,
        "my_abs should have no bridge blocks after CFG simplification.\nIR:\n{fn_ir}"
    );
}

/// J2 branching: nested if/else (`my_sign`) should have no bridge blocks.
#[test]
fn test_branching_my_sign_no_bridge_blocks() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_cfg_simplify/branching_my_sign_no_bridge_blocks.ori"
    ));

    let fn_ir = extract_function_ir(&ir, "_ori_my_sign");
    let bridges = count_bridge_blocks(fn_ir);

    assert_eq!(
        bridges, 0,
        "my_sign should have no bridge blocks.\nIR:\n{fn_ir}"
    );
}

/// Full J2 branching journey: all functions should have zero bridge blocks
/// and produce the correct result (exit code 17).
#[test]
fn test_j2_branching_correct_output_and_clean_cfg() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_cfg_simplify/j2_branching_correct_output_and_clean_cfg.ori"
    ));

    // Verify no bridge blocks in any function.
    for func_name in ["_ori_my_abs", "_ori_my_max", "_ori_my_sign", "_ori_main"] {
        let fn_ir = extract_function_ir(&ir, func_name);
        let bridges = count_bridge_blocks(fn_ir);
        assert_eq!(
            bridges, 0,
            "{func_name} should have zero bridge blocks.\nIR:\n{fn_ir}"
        );
    }
}

/// If/else with only constants should use `select`, not branches.
/// The CFG simplifier shouldn't break select-lowered patterns.
#[test]
fn test_select_lowering_preserved() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_cfg_simplify/select_lowering_preserved.ori"
    ));

    let fn_ir = extract_function_ir(&ir, "_ori_my_max");

    // my_max should be a single block with a select instruction.
    assert!(
        fn_ir.contains("select"),
        "my_max should use select for simple if/else.\nIR:\n{fn_ir}"
    );

    let bridges = count_bridge_blocks(fn_ir);
    assert_eq!(bridges, 0, "no bridge blocks in select-based function");
}
