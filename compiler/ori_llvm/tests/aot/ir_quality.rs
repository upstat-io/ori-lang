//! IR Quality Tests
//!
//! Verify that generated LLVM IR is clean and minimal at `-O0`:
//! - No dead `unreachable` blocks from nounwind invoke→call downgrade
//! - Minimal `declare` statements (only what's actually called)
//! - No redundant single-instruction `br` blocks in match arms
//! - Trivial if/else expressions emit `select`, not branch+phi diamonds
//!
//! These properties are optimized away at `-O1`+, but clean `-O0` output
//! aids debugging and makes IR inspection feasible during development.

use crate::util::{
    compile_and_capture_ir, count_bridge_blocks, count_single_pred_phis, extract_function_ir,
};

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

// ── Match Arm Bridge-Block Tests ───────────────────────────────────

/// Match with 5 pure-value arms should emit `select` chains, not branch+phi.
///
/// When every arm produces a simple value (constant, variable) with no side
/// effects, the decision tree + select lowering should emit a single basic
/// block with chained `select` instructions. No phi nodes, no bridge blocks.
#[test]
fn test_match_pure_values_no_bridge_blocks() {
    let ir = compile_and_capture_ir(
        r"
@classify (x: int) -> int = match x {
    0 -> 10,
    1 -> 20,
    2 -> 30,
    3 -> 40,
    _ -> 50,
};

@main () -> int = classify(x: 2);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let classify_ir = extract_function_ir(&ir, "_ori_classify");
    let bridges = count_bridge_blocks(classify_ir);

    assert_eq!(
        bridges, 0,
        "expected zero bridge blocks in _ori_classify with 5 pure-value match arms, \
         but found {bridges}.\nIR:\n{classify_ir}"
    );

    // Verify select lowering: should have `select` and no `phi`
    assert!(
        classify_ir.contains("select"),
        "expected `select` instructions for pure-value match arms.\nIR:\n{classify_ir}"
    );
    assert!(
        !classify_ir.contains("phi "),
        "expected no `phi` nodes for pure-value match arms (should use select).\n\
         IR:\n{classify_ir}"
    );
}

/// Match with 3+ function-call arms should have no trivial bridge blocks
/// between arm blocks and the merge point.
///
/// Each arm block should contain the function call instruction followed by
/// a branch to merge — not a separate bridge block that only holds `br`.
#[test]
fn test_match_call_arms_no_bridge_blocks() {
    let ir = compile_and_capture_ir(
        r"
@double (x: int) -> int = x + x;
@triple (x: int) -> int = x + x + x;
@quad (x: int) -> int = x + x + x + x;

@dispatch (op: int, val: int) -> int = match op {
    0 -> double(x: val),
    1 -> triple(x: val),
    2 -> quad(x: val),
    _ -> val,
};

@main () -> int = dispatch(op: 1, val: 5);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let dispatch_ir = extract_function_ir(&ir, "_ori_dispatch");
    let bridges = count_bridge_blocks(dispatch_ir);

    // The default arm (wildcard `_ -> val`) is a structural bridge block: a
    // switch target that just passes a parameter to the merge phi.  This is
    // inherent to the switch instruction — each case needs a target label.
    // We allow at most 1 bridge block (the default arm).
    assert!(
        bridges <= 1,
        "expected at most 1 bridge block (switch default) in _ori_dispatch, \
         but found {bridges}.\nIR:\n{dispatch_ir}"
    );
}

// ── Select Lowering Tests ───────────────────────────────────────────

/// Trivial if/else with variables should emit `select`, not a 4-block diamond.
///
/// `if x > 0 then a else b` — both arms are function parameters (no side
/// effects, no arm-local definitions). The ARC block merge pass should fold
/// this into a single `select` instruction.
#[test]
fn test_trivial_if_else_emits_select() {
    let ir = compile_and_capture_ir(
        r"
@pick (x: int, a: int, b: int) -> int = if x > 0 then a else b;

@main () -> int = pick(x: 5, a: 10, b: 20);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let pick_ir = extract_function_ir(&ir, "_ori_pick");

    // Should contain select instruction.
    assert!(
        pick_ir.contains("select"),
        "expected `select` instruction for trivial if/else in _ori_pick.\nIR:\n{pick_ir}"
    );

    // Should not contain phi (for a function this simple, all control flow
    // should be folded to select).
    assert!(
        !pick_ir.contains("phi "),
        "expected no `phi` nodes for trivial if/else (should use select).\nIR:\n{pick_ir}"
    );

    let bridges = count_bridge_blocks(pick_ir);
    assert_eq!(
        bridges, 0,
        "expected zero bridge blocks for trivial if/else.\nIR:\n{pick_ir}"
    );
}

/// Non-trivial if/else with function calls should emit branch+phi diamond.
///
/// Function calls lower to `Apply`/`Invoke` which are not trivial
/// (`is_trivial_body` only accepts `Let { Literal | Var }`).
#[test]
fn test_nontrivial_if_else_emits_diamond() {
    let ir = compile_and_capture_ir(
        r"
@f (x: int) -> int = x + 1;
@g (x: int) -> int = x - 1;

@pick (x: int) -> int = if x > 0 then f(x: x) else g(x: x);

@main () -> int = pick(x: 5);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let pick_ir = extract_function_ir(&ir, "_ori_pick");

    // Should contain conditional branch (br i1).
    assert!(
        pick_ir.contains("br i1"),
        "expected conditional branch for non-trivial if/else in _ori_pick.\nIR:\n{pick_ir}"
    );
}

/// If/else with negation (`PrimOp`) should emit diamond, not select.
///
/// Negation lowers to `Let { PrimOp { Unary(Neg) } }`, which is a `Let`
/// but not in the trivial whitelist (only `Literal` and `Var`).
#[test]
fn test_if_else_with_negation_emits_diamond() {
    let ir = compile_and_capture_ir(
        r"
@pick (x: int) -> int = if x > 0 then -x else x;

@main () -> int = pick(x: 5);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let pick_ir = extract_function_ir(&ir, "_ori_pick");

    // Should contain conditional branch (negation is not select-eligible).
    assert!(
        pick_ir.contains("br i1"),
        "expected conditional branch for if/else with negation in _ori_pick.\nIR:\n{pick_ir}"
    );
}

// ── Single-Predecessor Phi Elimination Tests ─────────────────────────

/// J6 pattern: enum match on Status should have no single-predecessor phis.
///
/// The enum match produces merge blocks; after block merge Phase 5,
/// any single-predecessor phis should be eliminated.
#[test]
fn test_enum_match_no_single_pred_phis() {
    let ir = compile_and_capture_ir(
        r"
type Status = Active | Inactive | Pending;

@to_code (s: Status) -> int = match s {
    Active -> 1,
    Inactive -> 2,
    Pending -> 3,
};

@main () -> int = to_code(s: Active);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_to_code");
    let single_pred = count_single_pred_phis(fn_ir);

    assert_eq!(
        single_pred, 0,
        "expected zero single-predecessor phi nodes in _ori_to_code, \
         but found {single_pred}.\nIR:\n{fn_ir}"
    );
}

/// J12 pattern: `?` operator on `Option<int>` should have no
/// single-predecessor phis.
#[test]
fn test_option_propagation_no_single_pred_phis() {
    let ir = compile_and_capture_ir(
        r"
@try_div (a: int, b: int) -> Option<int> = {
    if b == 0 then None else Some(a / b)
};

@main () -> int = {
    let r = try_div(a: 10, b: 2);
    match r {
        Some(v) -> v,
        None -> -1,
    }
}
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_try_div");
    let single_pred = count_single_pred_phis(fn_ir);

    assert_eq!(
        single_pred, 0,
        "expected zero single-predecessor phi nodes in _ori_try_div, \
         but found {single_pred}.\nIR:\n{fn_ir}"
    );
}

/// Generic test: a function with a single entry merge point should use
/// direct value reference, not phi.
#[test]
fn test_single_entry_merge_uses_direct_value() {
    let ir = compile_and_capture_ir(
        r"
@pick (x: int, a: int, b: int) -> int = {
    let result = if x > 0 then a else b;
    result * 2
};

@main () -> int = pick(x: 1, a: 10, b: 20);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_pick");
    let single_pred = count_single_pred_phis(fn_ir);

    assert_eq!(
        single_pred, 0,
        "expected zero single-predecessor phi nodes in _ori_pick, \
         but found {single_pred}.\nIR:\n{fn_ir}"
    );
}

/// Synthetic test: force a pattern where Phase 4 can't fully clean up.
///
/// The if/else with side-effecting arms (function calls) prevents select
/// lowering. Phase 5 should ensure no single-predecessor phis remain.
#[test]
fn test_synthetic_single_pred_phi_eliminated() {
    let ir = compile_and_capture_ir(
        r"
@inc (x: int) -> int = x + 1;
@dec (x: int) -> int = x - 1;

@synthetic (x: int) -> int = {
    let a = if x > 0 then inc(x: x) else dec(x: x);
    let b = a * 2;
    b
};

@main () -> int = synthetic(x: 5);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_synthetic");
    let single_pred = count_single_pred_phis(fn_ir);

    assert_eq!(
        single_pred, 0,
        "expected zero single-predecessor phi nodes in _ori_synthetic, \
         but found {single_pred}.\nIR:\n{fn_ir}"
    );
}
