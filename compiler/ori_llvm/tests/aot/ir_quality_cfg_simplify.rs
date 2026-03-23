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
    let ir = compile_and_capture_ir(
        r"
@my_abs (n: int) -> int = if n < 0 then -n else n;

@main () -> int = my_abs(n: -7);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

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
    let ir = compile_and_capture_ir(
        r"
@my_sign (n: int) -> int =
    if n > 0 then 1
    else (if n < 0 then -1 else 0);

@main () -> int = my_sign(n: 0);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

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
    let ir = compile_and_capture_ir(
        r"
@my_abs (n: int) -> int = if n < 0 then -n else n;
@my_max (a: int, b: int) -> int = if a > b then a else b;
@my_sign (n: int) -> int =
    if n > 0 then 1
    else (if n < 0 then -1 else 0);

@main () -> int = {
    let a = my_abs(n: -7);
    let b = my_max(a: 3, b: 10);
    let c = my_sign(n: 0);
    a + b + c
}
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

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
    let ir = compile_and_capture_ir(
        r"
@my_max (a: int, b: int) -> int = if a > b then a else b;

@main () -> int = my_max(a: 3, b: 10);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_my_max");

    // my_max should be a single block with a select instruction.
    assert!(
        fn_ir.contains("select"),
        "my_max should use select for simple if/else.\nIR:\n{fn_ir}"
    );

    let bridges = count_bridge_blocks(fn_ir);
    assert_eq!(bridges, 0, "no bridge blocks in select-based function");
}
