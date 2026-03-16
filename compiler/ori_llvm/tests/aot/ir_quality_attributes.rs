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
