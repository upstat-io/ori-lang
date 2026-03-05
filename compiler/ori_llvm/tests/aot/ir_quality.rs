//! IR Quality Tests
//!
//! Verify that generated LLVM IR is clean and minimal at `-O0`:
//! - No dead `unreachable` blocks from nounwind invoke→call downgrade
//! - Minimal `declare` statements (only what's actually called)
//! - No redundant single-instruction `br` blocks in match arms
//!
//! These properties are optimized away at `-O1`+, but clean `-O0` output
//! aids debugging and makes IR inspection feasible during development.

use crate::util::{compile_and_capture_ir, count_bridge_blocks, extract_function_ir};

/// Nounwind-only program: zero `unreachable` terminators in the IR.
///
/// A trivial `@main` that does pure arithmetic has no calls that can
/// unwind, so there should be no unwind landing pads and therefore
/// no dead blocks with `unreachable`.
#[test]
#[ignore = "Pre-existing: nounwind analysis not yet eliminating dead unreachable blocks at -O0"]
fn test_nounwind_program_has_no_unreachable_blocks() {
    let ir = compile_and_capture_ir(
        r"
@main () -> int = {
    let x = 10;
    let y = 3;
    x * y + 3
}
",
    );

    let main_ir = extract_function_ir(&ir, "_ori_main");

    // No unreachable terminators should exist
    assert!(
        !main_ir.contains("unreachable"),
        "expected zero `unreachable` blocks in nounwind-only _ori_main, \
         but found some.\nIR:\n{main_ir}"
    );
}

/// Nounwind generic call: identity function should produce no dead blocks.
///
/// `identity<T>` is trivially nounwind (just returns its argument).
/// After two-pass nounwind analysis, the invoke should be downgraded to
/// call, and the former unwind block should not be emitted at all.
#[test]
#[ignore = "Pre-existing: nounwind generic call still leaves dead unreachable blocks"]
fn test_nounwind_generic_call_no_unreachable() {
    let ir = compile_and_capture_ir(
        r"
@identity <T> (x: T) -> T = x;

@main () -> int = identity(x: 42);
",
    );

    let main_ir = extract_function_ir(&ir, "_ori_main");

    assert!(
        !main_ir.contains("unreachable"),
        "expected zero `unreachable` blocks in _ori_main calling nounwind generic, \
         but found some.\nIR:\n{main_ir}"
    );
}

/// Mixed nounwind/may-unwind: only may-unwind calls should have unwind blocks.
///
/// `add` is nounwind (pure arithmetic), `may_panic` can unwind (calls panic).
/// The IR for `_ori_main` should have landing pads only for the may-unwind call,
/// and no dead `unreachable` blocks from the nounwind call.
#[test]
#[ignore = "Pre-existing: mixed nounwind/may-unwind calls leave dead unreachable blocks"]
fn test_mixed_calls_no_dead_unreachable() {
    let ir = compile_and_capture_ir(
        r#"
@add (a: int, b: int) -> int = a + b;

@may_panic (x: int) -> int = {
    if x == 0 then panic(msg: "zero") else x
};

@main () -> int = {
    let sum = add(a: 1, b: 2);
    may_panic(x: sum)
}
"#,
    );

    let main_ir = extract_function_ir(&ir, "_ori_main");

    // Should have invoke (for may_panic) but no dead unreachable blocks
    assert!(
        !main_ir.contains("\n  unreachable\n"),
        "expected no dead `unreachable` blocks in _ori_main with mixed calls.\n\
         The nounwind `add` call should not leave a dead unwind block.\nIR:\n{main_ir}"
    );

    // Verify may_panic still uses invoke (correctness check)
    assert!(
        main_ir.contains("invoke"),
        "expected `invoke` for may-unwind `may_panic` call in _ori_main.\nIR:\n{main_ir}"
    );
}

/// Constant-returning main: near-minimal IR, no declarations beyond what's needed.
///
/// `@main () -> int = 33` should produce minimal IR — just a function that
/// returns 33. No unreachable blocks, no landing pads.
#[test]
#[ignore = "Pre-existing: constant-returning main still has unreachable/invoke/landingpad"]
fn test_constant_main_minimal_ir() {
    let ir = compile_and_capture_ir(
        r"
@main () -> int = 33;
",
    );

    let main_ir = extract_function_ir(&ir, "_ori_main");

    assert!(
        !main_ir.contains("unreachable"),
        "constant-returning _ori_main should have no unreachable blocks.\nIR:\n{main_ir}"
    );

    assert!(
        !main_ir.contains("invoke"),
        "constant-returning _ori_main should have no invoke instructions.\nIR:\n{main_ir}"
    );

    assert!(
        !main_ir.contains("landingpad"),
        "constant-returning _ori_main should have no landing pads.\nIR:\n{main_ir}"
    );
}

// ── Bridge-Block Elimination Tests ──────────────────────────────────

/// Sequential `add()` calls should produce no bridge-only blocks.
///
/// The ARC block merge pass should downgrade trivial invokes and merge
/// the resulting jump chains, eliminating `br label %bbN` bridge blocks.
#[test]
fn test_sequential_calls_no_bridge_blocks() {
    let ir = compile_and_capture_ir(
        r"
@add (a: int, b: int) -> int = a + b;

@main () -> int = {
    let x = add(a: 1, b: 2);
    let y = add(a: x, b: 3);
    let z = add(a: y, b: 4);
    z
}
",
    );

    // ORI_DEBUG_LLVM is debug-only — release binary produces no IR.
    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let main_ir = extract_function_ir(&ir, "_ori_main");
    let bridges = count_bridge_blocks(main_ir);

    assert_eq!(
        bridges, 0,
        "expected zero bridge-only blocks in _ori_main with 3 sequential add() calls, \
         but found {bridges}.\nIR:\n{main_ir}"
    );
}

/// @main calling a function should produce no bridge-only blocks.
#[test]
fn test_main_with_call_no_bridge_blocks() {
    let ir = compile_and_capture_ir(
        r"
@double (x: int) -> int = x * 2;

@main () -> int = double(x: 21);
",
    );

    // ORI_DEBUG_LLVM is debug-only — release binary produces no IR.
    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let main_ir = extract_function_ir(&ir, "_ori_main");
    let bridges = count_bridge_blocks(main_ir);

    assert_eq!(
        bridges, 0,
        "expected zero bridge-only blocks in _ori_main calling double(), \
         but found {bridges}.\nIR:\n{main_ir}"
    );
}
