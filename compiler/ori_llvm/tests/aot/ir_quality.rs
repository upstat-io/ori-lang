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
    compile_and_capture_ir, count_bridge_blocks, count_dead_phis, count_single_pred_phis,
    extract_function_ir,
};

/// Nounwind-only program: zero `unreachable` terminators in the IR.
///
/// A trivial `@main` that does pure arithmetic has no calls that can
/// unwind, so there should be no unwind landing pads and therefore
/// no dead blocks with `unreachable`.
#[test]
#[ignore = "codegen-purity plan, sections 02 + 06: nounwind invoke→call + dead block pruning"]
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
#[ignore = "codegen-purity plan, sections 02 + 06: nounwind generic invoke→call + dead block pruning"]
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
#[ignore = "codegen-purity plan, sections 02 + 06: mixed nounwind/may-unwind dead block pruning"]
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
#[ignore = "codegen-purity plan, sections 02 + 06: constant main should emit minimal IR"]
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

// ── nounwind on C main Wrapper (Codegen Purity §02.2) ───────────────

/// Trivial `@main` should have `nounwind` on the C `main()` wrapper.
///
/// A `@main () -> int = 42` is pure arithmetic — the nounwind analysis
/// marks `_ori_main` as `nounwind`, and the C main wrapper inherits it.
#[test]
fn test_trivial_main_wrapper_has_nounwind() {
    let ir = compile_and_capture_ir(
        r"
@main () -> int = 42;
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    assert_fn_has_attr(&ir, "main", "nounwind");
}

/// `@main` that may panic should NOT have `nounwind` on C `main()` wrapper.
///
/// The wrapper calls `_ori_main` which may unwind — the C `main` must not
/// be marked `nounwind` or LLVM would treat unwinding as UB.
#[test]
fn test_panicking_main_wrapper_lacks_nounwind() {
    let ir = compile_and_capture_ir(
        r#"
@main () -> int = {
    let x = 42;
    if x == 0 then panic(msg: "zero");
    x
}
"#,
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    assert_fn_lacks_attr(&ir, "main", "nounwind");
}

// ── nounwind on Derived Trait Methods (Codegen Purity §02.3) ────────

/// Pure derived methods ($eq, $compare, $hash) should have `nounwind`.
///
/// These methods only do field comparisons, hashing, and arithmetic —
/// no string allocation or user code calls. They are provably nounwind.
#[test]
fn test_pure_derived_methods_have_nounwind() {
    let ir = compile_and_capture_ir(
        r"
#derive(Eq, Comparable, Hashable)
type Shape = { sides: int, area: float };

@main () -> int = {
    let a = Shape { sides: 4, area: 16.0 };
    let b = Shape { sides: 4, area: 16.0 };
    let c = a.compare(other: b);
    let h = a.hash();
    if a == b then 1 else 0
}
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    assert_fn_has_attr(&ir, "_ori_Shape$eq", "nounwind");
    assert_fn_has_attr(&ir, "_ori_Shape$compare", "nounwind");
    assert_fn_has_attr(&ir, "_ori_Shape$hash", "nounwind");
}

/// Impure derived methods (`$to_str`, `$debug`) should NOT have `nounwind`.
///
/// These methods allocate strings and call non-nounwind runtime functions.
#[test]
fn test_impure_derived_methods_lack_nounwind() {
    let ir = compile_and_capture_ir(
        r"
#derive(Printable, Debug)
type Point = { x: int, y: int };

@main () -> int = {
    let p = Point { x: 1, y: 2 };
    let s = p.to_str();
    let d = p.debug();
    0
}
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    assert_fn_lacks_attr(&ir, "_ori_Point$to_str", "nounwind");
    assert_fn_lacks_attr(&ir, "_ori_Point$debug", "nounwind");
}

// ── noreturn on Panic Functions (Codegen Purity §02.1) ──────────────

/// Panic function declarations should have the `noreturn` attribute.
///
/// `ori_panic_cstr` (compile-time constant messages, e.g. overflow) and
/// `ori_panic` (dynamic string messages) never return to their caller.
/// LLVM needs `noreturn` to eliminate dead code after panic calls.
///
/// This test uses arithmetic (triggers `ori_panic_cstr` for overflow)
/// and explicit `panic()` (triggers `ori_panic` for dynamic messages).
///
/// LLVM IR uses attribute groups (`#N = { ... }`) — the `declare` line
/// references the group number, not the attributes directly.
#[test]
fn test_panic_declarations_have_noreturn() {
    let ir = compile_and_capture_ir(
        r#"
@main () -> int = {
    let x = 42;
    if x == 0 then panic(msg: "zero");
    x + 1
}
"#,
    );

    // Skip if release binary (no IR output)
    if !ir.contains("declare ") && !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    // Check ori_panic_cstr has noreturn via its attribute group
    assert_fn_has_attr(&ir, "ori_panic_cstr", "noreturn");
    assert_fn_lacks_attr(&ir, "ori_panic_cstr", "nounwind");

    // Check ori_panic has noreturn via its attribute group
    assert_fn_has_attr(&ir, "ori_panic", "noreturn");
    assert_fn_lacks_attr(&ir, "ori_panic", "nounwind");
}

/// Assert that a function declaration in the IR has a specific attribute
/// (resolved through LLVM's `#N = { ... }` attribute groups).
fn assert_fn_has_attr(ir: &str, func_name: &str, attr: &str) {
    let attrs = resolve_fn_attrs(ir, func_name);
    assert!(
        attrs.contains(attr),
        "{func_name} should have `{attr}` attribute.\n\
         Resolved attributes: {attrs}"
    );
}

/// Assert that a function declaration does NOT have a specific attribute.
fn assert_fn_lacks_attr(ir: &str, func_name: &str, attr: &str) {
    let attrs = resolve_fn_attrs(ir, func_name);
    assert!(
        !attrs.contains(attr),
        "{func_name} must NOT have `{attr}` attribute.\n\
         Resolved attributes: {attrs}"
    );
}

/// Resolve a function's attributes by following its `#N` attribute group
/// reference in the LLVM IR.
///
/// Searches both `declare` and `define` lines. Handles both plain names
/// (`@main(`) and quoted names (`@"_ori_Shape$eq"(`).
fn resolve_fn_attrs(ir: &str, func_name: &str) -> String {
    // LLVM quotes names with special characters: @"_ori_Shape$eq"(
    let search_plain = format!("@{func_name}(");
    let search_quoted = format!("@\"{func_name}\"(");
    let decl_line = ir
        .lines()
        .find(|l| {
            (l.contains("declare") || l.contains("define"))
                && (l.contains(&search_plain) || l.contains(&search_quoted))
        })
        .unwrap_or_else(|| panic!("{func_name} should be declared/defined in IR"));

    // Extract attribute group reference (e.g., "#2" from the declaration).
    // For `define`, strip trailing ` {` first.
    let line = decl_line.trim_end_matches('{').trim();
    let group_ref = line
        .rsplit_once('#')
        .map(|(_, num)| format!("#{}", num.trim()))
        .unwrap_or_default();

    if group_ref.is_empty() {
        return String::new();
    }

    // Find the attribute group definition: `attributes #2 = { cold noreturn }`
    let group_prefix = format!("attributes {group_ref} = ");
    ir.lines()
        .find(|l| l.starts_with(&group_prefix))
        .map(|l| l[group_prefix.len()..].to_string())
        .unwrap_or_default()
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

// ── Break Bridge Block Elimination Tests (Section 01.4) ─────────────

/// Single-break loop: exit block should have no bridge blocks or dead phis.
///
/// `loop { if cond then break value }` with a single break path has
/// a single-predecessor exit block. Phase 5 converts params to Let
/// bindings and Phase 4 merges the blocks.
#[test]
fn test_single_break_loop_clean_exit() {
    let ir = compile_and_capture_ir(
        r"
@sum_loop (n: int) -> int = {
    let i = 0;
    let total = 0;
    loop {
        if i >= n then break total;
        total += i + 1;
        i += 1
    }
};

@main () -> int = sum_loop(n: 5);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_sum_loop");

    // Single-break loop: Phase 5 should handle single-predecessor exit.
    let single_pred = count_single_pred_phis(fn_ir);
    assert_eq!(
        single_pred, 0,
        "expected zero single-predecessor phis in _ori_sum_loop.\nIR:\n{fn_ir}"
    );

    let dead = count_dead_phis(fn_ir);
    assert_eq!(
        dead, 0,
        "expected zero dead phis in _ori_sum_loop.\nIR:\n{fn_ir}"
    );
}

/// Multi-break loop: exit block should have no dead phis.
///
/// Two `break` paths create a multi-predecessor exit block. The exit
/// block params for mutable variables (`i`, `total`) that are unused
/// after the loop should be eliminated by dead-param analysis.
#[test]
fn test_multi_break_loop_no_dead_phis() {
    let ir = compile_and_capture_ir(
        r"
@multi_break (n: int) -> int = {
    let i = 0;
    let total = 0;
    loop {
        if i >= n then break total;
        if total > 100 then break -1;
        total += i + 1;
        i += 1
    }
};

@main () -> int = multi_break(n: 5);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_multi_break");

    let dead = count_dead_phis(fn_ir);
    assert_eq!(
        dead, 0,
        "expected zero dead phis in _ori_multi_break, but found {dead}.\n\
         Dead phis arise from unused mutable variable params on multi-predecessor \
         exit blocks.\nIR:\n{fn_ir}"
    );
}

/// Multi-break loop with post-loop variable use: only truly dead params eliminated.
///
/// When a mutable variable IS used after the loop, its exit block param
/// must be preserved. Only unused params should be eliminated.
#[test]
fn test_multi_break_loop_preserves_live_params() {
    let ir = compile_and_capture_ir(
        r"
@search (n: int) -> int = {
    let i = 0;
    let found = loop {
        if i >= n then break -1;
        if i * i > n then break i;
        i += 1
    };
    found + i
};

@main () -> int = search(n: 10);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_search");

    // `i` is used after the loop (`found + i`), so its exit param is live.
    // The `found` param (break value) is also live.
    // No dead phis should exist.
    let dead = count_dead_phis(fn_ir);
    assert_eq!(
        dead, 0,
        "expected zero dead phis in _ori_search.\nIR:\n{fn_ir}"
    );
}

// ── noundef on Scalar Parameters (Codegen Purity §02.6) ─────────────

/// Scalar parameters (`int`, `float`, `bool`) should have `noundef` in IR.
///
/// Ori's type system guarantees all scalar values are initialized, so LLVM
/// can assume passing undef/poison is UB. This enables range analysis,
/// dead argument elimination, and other scalar optimizations.
#[test]
fn test_scalar_params_have_noundef() {
    let ir = compile_and_capture_ir(
        r"
@add (a: int, b: int) -> int = a + b;

@scale (x: float, factor: float) -> float = x * factor;

@main () -> int = {
    let s = scale(x: 3.0, factor: 2.0);
    add(a: 1, b: 2)
}
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    // _ori_add: both int params and int return should have noundef
    let add_decl = ir
        .lines()
        .find(|l| l.contains("@_ori_add") || l.contains("@\"_ori_add\""))
        .expect("_ori_add should be in IR");
    assert_eq!(
        add_decl.matches("noundef").count(),
        3,
        "expected 3 noundef (2 int params + int return) on _ori_add:\n{add_decl}"
    );

    // _ori_scale: both float params and float return should have noundef
    let scale_decl = ir
        .lines()
        .find(|l| l.contains("@_ori_scale") || l.contains("@\"_ori_scale\""))
        .expect("_ori_scale should be in IR");
    assert_eq!(
        scale_decl.matches("noundef").count(),
        3,
        "expected 3 noundef (2 float params + float return) on _ori_scale:\n{scale_decl}"
    );
}

/// Aggregate parameters (str) and pointer params should NOT have `noundef`.
///
/// §02.6 conservative policy: only scalar primitives get `noundef`.
/// Aggregates and pointers require additional proof obligations.
#[test]
fn test_aggregate_params_lack_noundef() {
    let ir = compile_and_capture_ir(
        r#"
@greet (name: str) -> str = `Hello, {name}`;

@main () -> void = {
    let msg = greet(name: "world");
    print(msg: msg)
}
"#,
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    // _ori_greet has str param (aggregate, >16 bytes, Indirect → ptr) and str
    // return (Sret → void). Neither should have noundef.
    let greet_decl = ir
        .lines()
        .find(|l| {
            (l.contains("@_ori_greet") || l.contains("@\"_ori_greet\""))
                && (l.contains("define") || l.contains("declare"))
        })
        .expect("_ori_greet should be in IR");
    assert!(
        !greet_decl.contains("noundef"),
        "str param/return should NOT have noundef on _ori_greet:\n{greet_decl}"
    );
}

// ── Sum Type Payload Extraction Tests (Codegen Purity §05) ──────────

/// Enum payload extraction should use `extractvalue`, not alloca+store+GEP+load.
///
/// A match arm that destructures a 2-field sum type variant currently
/// spills the entire enum to the stack via alloca+store, then uses GEP
/// to index into the payload array. This costs 5 instructions per field.
/// With `extractvalue`, it's 2 instructions per field (extract payload
/// array, then extract element) plus an optional bitcast for non-i64 types.
#[test]
fn test_enum_payload_uses_extractvalue() {
    let ir = compile_and_capture_ir(
        r"
type Shape = Circle(radius: float) | Rect(width: float, height: float);

@extract (s: Shape) -> float = match s {
    Circle(radius) -> radius,
    Rect(width, height) -> width + height,
};

@main () -> int = {
    let c = Circle(radius: 3.14);
    let r = Rect(width: 2.0, height: 5.0);
    let x = extract(s: c);
    let y = extract(s: r);
    0
}
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_extract");

    // Match arm blocks should NOT contain `proj.alloca` (the alloca+store pattern
    // for enum payload extraction). The extractvalue path avoids stack spill.
    assert!(
        !fn_ir.contains("proj.alloca"),
        "expected no `proj.alloca` in _ori_extract — enum payload extraction should use \
         `extractvalue` instead of alloca+store+GEP+load.\nIR:\n{fn_ir}"
    );

    // Should contain extractvalue for payload access.
    assert!(
        fn_ir.contains("extractvalue"),
        "expected `extractvalue` instructions for enum payload extraction.\nIR:\n{fn_ir}"
    );
}

/// Enum with int fields: extractvalue needs no bitcast (i64 → i64 is identity).
#[test]
fn test_enum_int_payload_extractvalue() {
    let ir = compile_and_capture_ir(
        r"
type IntEnum = A(x: int) | B(x: int, y: int);

@extract_b (e: IntEnum) -> int = match e {
    A(x) -> x,
    B(x, y) -> x + y,
};

@main () -> int = extract_b(e: B(x: 10, y: 20));
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_extract_b");

    assert!(
        !fn_ir.contains("proj.alloca"),
        "expected no `proj.alloca` in _ori_extract_b — int payload should use extractvalue.\n\
         IR:\n{fn_ir}"
    );
}

/// Boxed (recursive) enum fields must still use alloca+GEP+load.
///
/// Recursive types are heap-allocated behind RC pointers. The payload
/// slot contains a pointer, not a value. Extractvalue can get the raw
/// pointer bits, but the subsequent dereference requires a load from
/// heap memory — which only works through a pointer, not extractvalue.
#[test]
fn test_boxed_enum_field_uses_alloca() {
    let ir = compile_and_capture_ir(
        r"
type Tree = Leaf(value: int) | Node(left: Tree, right: Tree);

@left_val (t: Tree) -> int = match t {
    Leaf(value) -> value,
    Node(left, right) -> match left {
        Leaf(value) -> value,
        _ -> -1,
    },
};

@main () -> int = {
    let t = Node(left: Leaf(value: 42), right: Leaf(value: 0));
    left_val(t: t)
}
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_left_val");

    // Boxed enum fields (recursive types) MUST still use alloca path —
    // the RC pointer needs a load from heap, which extractvalue cannot do.
    // The function should contain proj.alloca for the Node arm's left/right extraction.
    assert!(
        fn_ir.contains("proj.alloca") || fn_ir.contains("proj.") && fn_ir.contains(".ptr"),
        "expected alloca-based extraction for boxed (recursive) enum fields in _ori_left_val.\n\
         Boxed fields store RC pointers in payload slots — extractvalue alone cannot dereference heap.\n\
         IR:\n{fn_ir}"
    );
}
