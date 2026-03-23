//! IR Quality Tests: Function Attributes
//!
//! Verify correct `nounwind`, `noreturn`, and `noundef` attribute placement
//! on LLVM IR function declarations and definitions.

use crate::util::{compile_and_capture_ir, extract_function_ir};

// Pre-banner ignored tests (nounwind + dead block pruning)

/// Nounwind-only program: no dead unreachable blocks from invoke splitting.
///
/// A trivial `@main` that does pure arithmetic has no calls that can
/// unwind, so there should be no unwind landing pads and no dead
/// blocks. Overflow panic paths correctly use `call @ori_panic_cstr`
/// followed by `unreachable` (noreturn), which is expected — only
/// dead/orphan unreachable blocks are a defect.
#[test]
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

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let main_ir = extract_function_ir(&ir, "_ori_main");

    // No invoke instructions — all calls are nounwind
    assert!(
        !main_ir.contains("invoke"),
        "expected no `invoke` in nounwind-only _ori_main.\nIR:\n{main_ir}"
    );

    // No landingpad — no unwind handling needed
    assert!(
        !main_ir.contains("landingpad"),
        "expected no `landingpad` in nounwind-only _ori_main.\nIR:\n{main_ir}"
    );

    // Every `unreachable` must follow a noreturn call (overflow panic).
    // A standalone `unreachable` (not preceded by a call) would indicate
    // a dead block from invoke splitting that wasn't cleaned up.
    for (i, line) in main_ir.lines().enumerate() {
        if line.trim() == "unreachable" {
            let prev = main_ir.lines().nth(i.wrapping_sub(1)).unwrap_or("");
            assert!(
                prev.contains("ori_panic"),
                "found standalone `unreachable` not preceded by panic call \
                 (dead unwind block?) at line {i}.\nIR:\n{main_ir}"
            );
        }
    }
}

/// Nounwind generic call: identity function should produce no dead blocks.
///
/// `identity<T>` is trivially nounwind (just returns its argument).
/// After two-pass nounwind analysis, the invoke should be downgraded to
/// call, and the former unwind block should not be emitted at all.
#[test]
fn test_nounwind_generic_call_no_unreachable() {
    let ir = compile_and_capture_ir(
        r"
@identity <T> (x: T) -> T = x;

@main () -> int = identity(x: 42);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let main_ir = extract_function_ir(&ir, "_ori_main");

    assert!(
        !main_ir.contains("unreachable"),
        "expected zero `unreachable` blocks in _ori_main calling nounwind generic, \
         but found some.\nIR:\n{main_ir}"
    );
}

/// Mixed nounwind/may-unwind: no dead unreachable blocks from invoke splitting.
///
/// `add` is nounwind (pure arithmetic), `may_panic` can unwind (calls panic).
/// After codegen-purity improvements, `_ori_main` uses `call` for both
/// functions because there is no cleanup work on the unwind path. The
/// key property: no dead blocks from invoke splitting remain.
#[test]
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

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let main_ir = extract_function_ir(&ir, "_ori_main");

    // No dead unreachable blocks — any `unreachable` must follow a noreturn call
    for (i, line) in main_ir.lines().enumerate() {
        if line.trim() == "unreachable" {
            let prev = main_ir.lines().nth(i.wrapping_sub(1)).unwrap_or("");
            assert!(
                prev.contains("ori_panic"),
                "found standalone `unreachable` not preceded by panic call \
                 (dead unwind block?) at line {i}.\nIR:\n{main_ir}"
            );
        }
    }

    // Nounwind `add` uses `call`, not `invoke`
    assert!(
        main_ir.contains("call fastcc i64 @_ori_add"),
        "nounwind `add` should use `call`, not `invoke`.\nIR:\n{main_ir}"
    );
}

/// Constant-returning main: near-minimal IR, no declarations beyond what's needed.
///
/// `@main () -> int = 33` should produce minimal IR — just a function that
/// returns 33. No unreachable blocks, no landing pads.
#[test]
fn test_constant_main_minimal_ir() {
    let ir = compile_and_capture_ir(
        r"
@main () -> int = 33;
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

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

// nounwind on C main wrapper

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

// nounwind propagation through builtin methods and protocols

/// Function calling builtin method (str.length) via Invoke terminator gets nounwind.
///
/// The ARC IR lowers `.length()` to `Invoke @length(...)`, and the nounwind
/// analysis must recognize `@length` as an intercepted builtin that always
/// emits `call` (never `invoke`). Without this, the function and its callers
/// would incorrectly lose the nounwind attribute.
#[test]
fn test_function_calling_builtin_method_gets_nounwind() {
    let ir = compile_and_capture_ir(
        r#"
@count_chars (words: [str]) -> int = {
    let total = 0;
    for w in words do total += w.length();
    total
}

@total_items (xs: [str]) -> int = xs.length();

@main () -> int = {
    let words = ["hello", "world", "12345"];
    let a = count_chars(words: words);
    let b = total_items(xs: words);
    a + b
}
"#,
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    assert_fn_has_attr(&ir, "_ori_count_chars", "nounwind");
    assert_fn_has_attr(&ir, "_ori_total_items", "nounwind");
    assert_fn_has_attr(&ir, "_ori_main", "nounwind");
    assert_fn_has_attr(&ir, "main", "nounwind");
}

/// Function with indirect call (closure) still gets nounwind via post-hoc pass.
///
/// The ARC IR produces `ApplyIndirect` for closure calls, which the two-pass
/// analysis conservatively treats as may-unwind. The post-hoc pass detects
/// that the emitted LLVM IR has no `invoke` instructions and marks the function
/// nounwind. The post-hoc pass must use fixed-point iteration so call chains
/// (`main` → `check_capture` → closure) all propagate correctly regardless of
/// `HashMap` iteration order.
#[test]
fn test_closure_call_gets_nounwind_via_posthoc() {
    let ir = compile_and_capture_ir(
        r#"
@check_capture () -> int = {
    let prefix = "hello";
    let f = s -> prefix.length() + s.length();
    f("world")
}

@main () -> int = check_capture();
"#,
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    assert_fn_has_attr(&ir, "_ori_check_capture", "nounwind");
    assert_fn_has_attr(&ir, "_ori_main", "nounwind");
    assert_fn_has_attr(&ir, "main", "nounwind");
}

/// Generic function with may-unwind body must NOT be treated as intercepted.
///
/// Regression test for TPR-01-003: `is_callee_intercepted()` previously fell
/// through to the builtin method heuristic for generic calls with builtin-typed
/// first args. A call like `might_panic(s)` where `s: str` would be treated as
/// an intercepted builtin (nounwind), even though the monomorphized function
/// may unwind via `panic()`. The fix adds a `mono_dispatch` check before the
/// builtin heuristic.
#[test]
fn test_generic_call_with_builtin_arg_not_treated_as_intercepted() {
    let ir = compile_and_capture_ir(
        r#"
@might_panic<T> (x: T) -> T = {
    if true then panic(msg: "boom");
    x
}

@main () -> int = {
    let s = might_panic(x: "hello");
    s.length()
}
"#,
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    // `might_panic` contains `panic()` — it MUST NOT be nounwind.
    // Before the fix, mono_dispatch was not checked, so `might_panic(x: "hello")`
    // would be classified as an intercepted builtin (str receiver), making main
    // appear nounwind despite calling a may-unwind generic function.
    assert_fn_lacks_attr(&ir, "_ori_main", "nounwind");
    assert_fn_lacks_attr(&ir, "main", "nounwind");
}

// nounwind on derived trait methods

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

// noreturn on panic functions

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

// noundef on scalar parameters

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

/// Indirect pointer params (str) should have `noundef` on the pointer value.
///
/// The pointer itself is always a valid, defined address in Ori — never
/// poison or undef. Sret return is void in the LLVM signature, so no
/// return `noundef`.
#[test]
fn test_indirect_params_have_noundef() {
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

    // _ori_greet: str param (Indirect → ptr noundef), str return (Sret → void).
    // The pointer param gets noundef; the sret pointer does NOT get noundef
    // (sret is a special ABI parameter, not a user value).
    let greet_decl = ir
        .lines()
        .find(|l| {
            (l.contains("@_ori_greet") || l.contains("@\"_ori_greet\""))
                && (l.contains("define") || l.contains("declare"))
        })
        .expect("_ori_greet should be in IR");
    assert!(
        greet_decl.contains("noundef"),
        "Indirect str param should have noundef on _ori_greet:\n{greet_decl}"
    );
}

// nonnull + dereferenceable on indirect params

/// Indirect pointer params (str, list) should have `nonnull` attribute.
///
/// Ori never passes null pointers to functions — all Indirect/Reference
/// params point to valid, initialized memory. LLVM uses `nonnull` to
/// eliminate null checks and enable speculative loads.
#[test]
fn test_indirect_params_have_nonnull() {
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

    let greet_decl = ir
        .lines()
        .find(|l| {
            (l.contains("@_ori_greet") || l.contains("@\"_ori_greet\""))
                && (l.contains("define") || l.contains("declare"))
        })
        .expect("_ori_greet should be in IR");
    assert!(
        greet_decl.contains("nonnull"),
        "Indirect str param should have nonnull on _ori_greet:\n{greet_decl}"
    );
}

/// Indirect pointer params should have `dereferenceable(N)` where N is
/// the ABI size of the pointed-to type.
///
/// For `str` (`OriStr`: {ptr, len, cap} = 24 bytes), the pointer should
/// have `dereferenceable(24)`. This enables LLVM to perform speculative
/// loads without null/bounds checks.
#[test]
fn test_indirect_params_have_dereferenceable() {
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

    let greet_decl = ir
        .lines()
        .find(|l| {
            (l.contains("@_ori_greet") || l.contains("@\"_ori_greet\""))
                && (l.contains("define") || l.contains("declare"))
        })
        .expect("_ori_greet should be in IR");
    // str = OriStr = {ptr, len, cap} = 24 bytes
    assert!(
        greet_decl.contains("dereferenceable(24)"),
        "Indirect str param should have dereferenceable(24) on _ori_greet:\n{greet_decl}"
    );
}

/// Direct scalar params should NOT have nonnull or dereferenceable — those
/// attributes only apply to pointer params (Indirect/Reference).
#[test]
fn test_direct_params_lack_nonnull() {
    let ir = compile_and_capture_ir(
        r"
@add (a: int, b: int) -> int = a + b;

@main () -> int = add(a: 1, b: 2);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let add_decl = ir
        .lines()
        .find(|l| {
            (l.contains("@_ori_add") || l.contains("@\"_ori_add\""))
                && (l.contains("define") || l.contains("declare"))
        })
        .expect("_ori_add should be in IR");
    assert!(
        !add_decl.contains("nonnull"),
        "Direct int params should NOT have nonnull on _ori_add:\n{add_decl}"
    );
    assert!(
        !add_decl.contains("dereferenceable"),
        "Direct int params should NOT have dereferenceable on _ori_add:\n{add_decl}"
    );
}

// Helpers

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

// Iterator option wrapping elimination

/// For-loop iterator codegen should NOT build a `{i64, T}` wrapper struct.
///
/// The ARC IR `__iter_next` protocol returns a decomposed `(tag, scratch_ptr)`
/// pair. The LLVM emission should use the tag directly and load the element
/// from the scratch buffer — no `insertvalue` to build a wrapper struct.
#[test]
fn test_iter_next_no_wrapper_struct() {
    let ir = compile_and_capture_ir(
        r#"
@count (words: [str]) -> int = {
    let total = 0;
    for w in words do total += w.length();
    total
}

@main () -> int = count(words: ["hello", "world", "test"]);
"#,
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let count_ir = extract_function_ir(&ir, "_ori_count");

    // The wrapper struct `{i64, {i64, i64, ptr}}` required insertvalue.
    // After optimization, no insertvalue should appear for iter_next results.
    let insertvalue_count = count_ir.matches("insertvalue").count();
    assert_eq!(
        insertvalue_count, 0,
        "expected 0 insertvalue in @count (iter_next decomposed), got {insertvalue_count}.\nIR:\n{count_ir}"
    );

    // ori_str_len should read from iter_next.scratch directly, not a separate alloca.
    assert!(
        count_ir.contains("call i64 @ori_str_len(ptr %iter_next.scratch)"),
        "expected ori_str_len to receive iter_next.scratch directly.\nIR:\n{count_ir}"
    );
}

/// Semantic pin: for-loop over `[str]` with `.length()` returns correct total.
#[test]
fn test_iter_for_loop_str_length_correctness() {
    let exit = crate::util::compile_and_run(
        r#"
@main () -> int = {
    let words = ["hello", "world"];
    let total = 0;
    for w in words do total += w.length();
    total
}
"#,
    );
    assert_eq!(exit, 10, "expected total length 10 (5+5)");
}

/// Semantic pin: for-loop over `[[int]]` with `.length()` returns correct total.
#[test]
fn test_iter_for_loop_list_length_correctness() {
    let exit = crate::util::compile_and_run(
        r"
@main () -> int = {
    let lists = [[1, 2, 3], [4, 5]];
    let total = 0;
    for xs in lists do total += xs.length();
    total
}
",
    );
    assert_eq!(exit, 5, "expected total length 5 (3+2)");
}

/// Scalar elements (int) should not be affected by the optimization.
#[test]
fn test_iter_for_loop_scalar_element() {
    let exit = crate::util::compile_and_run(
        r"
@main () -> int = {
    let nums = [3, 7, 2];
    let total = 0;
    for n in nums do total += n;
    total
}
",
    );
    assert_eq!(exit, 12, "expected sum 12 (3+7+2)");
}

/// For-loop with break: element must be valid up to break point.
#[test]
fn test_iter_for_loop_with_break() {
    let exit = crate::util::compile_and_run(
        r#"
@main () -> int = {
    let words = ["hello", "world", "test"];
    let total = 0;
    for w in words do {
        if w.length() == 5 then total += 1;
        if total == 2 then break
    };
    total
}
"#,
    );
    assert_eq!(exit, 2, "expected 2 (break after two 5-char words)");
}

/// Nested for-loops: outer element used in inner loop body.
#[test]
fn test_iter_nested_for_loops() {
    let exit = crate::util::compile_and_run(
        r"
@main () -> int = {
    let lists = [[1, 2], [3]];
    let total = 0;
    for xs in lists do {
        for x in xs do total += x
    };
    total
}
",
    );
    assert_eq!(exit, 6, "expected 6 (1+2+3)");
}

/// Struct element field access in for-loop body.
///
/// Iterates over `[Point]` and accesses `p.x` — the element is a struct
/// passed by value. Tests that the scratch buffer optimization correctly
/// handles struct elements with field projection.
#[test]
fn test_iter_for_loop_struct_field_access() {
    let exit = crate::util::compile_and_run(
        r"
type Point = { x: int, y: int };

@main () -> int = {
    let points = [Point { x: 3, y: 4 }, Point { x: 7, y: 1 }];
    let total = 0;
    for p in points do total += p.x;
    total
}
",
    );
    assert_eq!(exit, 10, "expected 10 (3+7)");
}

/// Element stored into collection via push during for-loop.
///
/// `for w in words do result = result.push(value: w)` — the element must
/// be a valid copy when pushed. `ori_list_push` copies immediately, so
/// scratch buffer forwarding is safe. Verify by reading back the pushed
/// elements and summing their lengths.
#[test]
fn test_iter_for_loop_push_element() {
    let exit = crate::util::compile_and_run(
        r#"
@main () -> int = {
    let words = ["hello", "world"];
    let result: [str] = [];
    for w in words do result = result.push(value: w);
    let total = 0;
    for r in result do total += r.length();
    total
}
"#,
    );
    assert_eq!(
        exit, 10,
        "expected 10 (5+5) — pushed elements must be valid copies"
    );
}

/// Guarded for-loop: element used in both guard and body.
///
/// `for w in words if w.length() > 3 do total += w.length()` — the guard
/// and body both access the element via `Project(next_result, 1)`. Both
/// must read from the same scratch pointer, which is correct since the
/// buffer isn't overwritten between guard and body evaluation.
#[test]
fn test_iter_for_loop_guarded() {
    let exit = crate::util::compile_and_run(
        r#"
@main () -> int = {
    let words = ["hi", "hello", "go", "world"];
    let total = 0;
    for w in words if w.length() > 3 do total += w.length();
    total
}
"#,
    );
    assert_eq!(
        exit, 10,
        "expected 10 (hello=5 + world=5, hi and go filtered)"
    );
}

/// For-yield (list comprehension): element transformed and collected.
///
/// `for w in words yield w.length()` uses the same `__iter_next` path as
/// for-do. The yielded value is the body result (`w.length()` → int), but
/// the element `w` still comes from the scratch buffer via Project.
#[test]
fn test_iter_for_yield_lengths() {
    let exit = crate::util::compile_and_run(
        r#"
@main () -> int = {
    let words = ["hello", "world"];
    let lengths = for w in words yield w.length();
    let total = 0;
    for n in lengths do total += n;
    total
}
"#,
    );
    assert_eq!(
        exit, 10,
        "expected 10 (5+5) — for-yield must work with scratch forwarding"
    );
}

/// Element passed by value to a function — must be a copy, not the scratch buffer.
///
/// `process(s: w)` passes `w` by value. The function receives a valid copy
/// of the string, not a pointer into the scratch buffer. If the scratch
/// buffer were forwarded as the argument, the function might see stale data
/// on subsequent iterations.
#[test]
fn test_iter_for_loop_element_passed_to_function() {
    let exit = crate::util::compile_and_run(
        r#"
@process (s: str) -> int = s.length() + 1;

@main () -> int = {
    let words = ["hello", "world"];
    let total = 0;
    for w in words do total += process(s: w);
    total
}
"#,
    );
    assert_eq!(
        exit, 12,
        "expected 12 (6+6) — element must be a valid copy in callee"
    );
}

/// Element passed to two runtime calls in same iteration.
///
/// Both `w.length()` calls must see the correct string value. If the scratch
/// buffer were mutated between calls (it shouldn't be — only `ori_iter_next`
/// overwrites it), one call would see corrupt data.
#[test]
fn test_iter_for_loop_two_calls_same_element() {
    let exit = crate::util::compile_and_run(
        r#"
@main () -> int = {
    let words = ["hello", "world"];
    let total = 0;
    for w in words do {
        let a = w.length();
        let b = w.length();
        total += a + b
    };
    total
}
"#,
    );
    assert_eq!(
        exit, 20,
        "expected 20 — both calls must see correct element value"
    );
}

/// Semantic pin: for-yield over string list returns correct mapped lengths.
///
/// `for w in ["a", "bb", "ccc"] yield w.length()` must produce `[1, 2, 3]`.
/// Verifies both the list length (3) and sum (6) to pin the for-yield
/// semantics with the scratch buffer optimization.
#[test]
fn test_iter_for_yield_semantic_pin() {
    let exit = crate::util::compile_and_run(
        r#"
@main () -> int = {
    let lengths = for w in ["a", "bb", "ccc"] yield w.length();
    if lengths.length() == 3 then {
        let total = 0;
        for n in lengths do total += n;
        total
    } else 0
}
"#,
    );
    assert_eq!(exit, 6, "expected 6 (1+2+3) — for-yield semantic pin");
}
