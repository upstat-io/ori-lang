//! IR Quality Tests: Function Attributes
//!
//! Verify correct `nounwind`, `noreturn`, and `noundef` attribute placement
//! on LLVM IR function declarations and definitions.

use crate::util::{
    compile_and_capture_ir, compile_and_capture_ir_no_repr_opt, extract_function_ir,
    resolve_derived_function_name, resolve_function_attrs,
};

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/nounwind_program_has_no_unreachable_blocks.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/nounwind_generic_call_no_unreachable.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/mixed_calls_no_dead_unreachable.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/constant_main_minimal_ir.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/trivial_main_wrapper_has_nounwind.ori"
    ));

    assert_fn_has_attr(&ir, "main", "nounwind");
}

/// `@main` that may panic should NOT have `nounwind` on C `main()` wrapper.
///
/// The wrapper calls `_ori_main` which may unwind — the C `main` must not
/// be marked `nounwind` or LLVM would treat unwinding as UB.
#[test]
fn test_panicking_main_wrapper_lacks_nounwind() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/panicking_main_wrapper_lacks_nounwind.ori"
    ));

    assert_fn_lacks_attr(&ir, "main", "nounwind");
}

/// Regression: a same-frame `catch(expr: 1 / 0)` makes the
/// enclosing function may-unwind. The checked div-by-zero panic is emitted as
/// `invoke @ori_panic_cstr` to a catch landing pad, so `_ori_main` MUST NOT be
/// marked `nounwind` and MUST carry a `landingpad` + the panic `invoke`. The
/// exit-code cells cannot observe the nounwind attribute / IR shape — this pins
/// the landing pad surviving to codegen (no nounwind-strips-landingpad regress).
#[test]
fn test_checked_op_catch_fn_not_nounwind() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/checked_op_catch_not_nounwind.ori"
    ));

    // The function carrying the catch must not be nounwind (it may unwind via
    // the checked-op invoke).
    assert_fn_lacks_attr(&ir, "_ori_main", "nounwind");

    let main_ir = extract_function_ir(&ir, "_ori_main");

    // The catch landing pad survived to codegen.
    assert!(
        main_ir.contains("landingpad"),
        "expected a `landingpad` in _ori_main with a same-frame checked-op catch.\nIR:\n{main_ir}"
    );

    // The checked div-by-zero panic is an `invoke` (caught), not a plain `call`
    // + `unreachable` (which would escape the catch and abort).
    assert!(
        main_ir.contains("invoke void @ori_panic_cstr"),
        "expected `invoke @ori_panic_cstr` routing the checked-op panic to the \
         catch landing pad.\nIR:\n{main_ir}"
    );
}

/// Regression: same as [`test_checked_op_catch_fn_not_nounwind`], for `byte`.
/// Pins the `Tag::is_checked_int_arithmetic()` widening (`Int | Byte |
/// Duration | Size`): byte checked ops inside a same-frame `catch(expr:)`
/// register in `catch_scoped_checked_ops` and route panics to the landing pad.
#[test]
fn test_checked_op_catch_fn_not_nounwind_byte() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/checked_op_catch_not_nounwind_byte.ori"
    ));

    assert_fn_lacks_attr(&ir, "_ori_main", "nounwind");

    let main_ir = extract_function_ir(&ir, "_ori_main");

    assert!(
        main_ir.contains("landingpad"),
        "expected a `landingpad` in _ori_main with a same-frame checked-op catch.\nIR:\n{main_ir}"
    );

    assert!(
        main_ir.contains("invoke void @ori_panic_cstr"),
        "expected `invoke @ori_panic_cstr` routing the checked-op panic to the \
         catch landing pad.\nIR:\n{main_ir}"
    );
}

/// Regression: same as [`test_checked_op_catch_fn_not_nounwind`], for `Size`.
/// Pins the `Tag::is_checked_int_arithmetic()` widening for Size the same
/// way as byte.
#[test]
fn test_checked_op_catch_fn_not_nounwind_size() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/checked_op_catch_not_nounwind_size.ori"
    ));

    assert_fn_lacks_attr(&ir, "_ori_main", "nounwind");

    let main_ir = extract_function_ir(&ir, "_ori_main");

    assert!(
        main_ir.contains("landingpad"),
        "expected a `landingpad` in _ori_main with a same-frame checked-op catch.\nIR:\n{main_ir}"
    );

    assert!(
        main_ir.contains("invoke void @ori_panic_cstr"),
        "expected `invoke @ori_panic_cstr` routing the checked-op panic to the \
         catch landing pad.\nIR:\n{main_ir}"
    );
}

/// Regression: an uncaught checked-arithmetic overflow with no other calls
/// and no same-frame catch must not mark the enclosing function `nounwind`.
/// The overflow panic (`call @ori_panic_cstr`) is an LLVM-emission-time-only
/// artifact of the checked-add intrinsic lowering — invisible to the ARC IR
/// nounwind scan, which only inspects `Apply`/`Invoke`/`RcDec` instructions.
/// A leaf function whose sole instruction is checked arithmetic must be
/// conservatively treated as may-unwind, exactly like an indirect call.
#[test]
fn test_uncaught_checked_arith_overflow_not_nounwind() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/uncaught_checked_arith_overflow_not_nounwind.ori"
    ));

    assert_fn_lacks_attr(&ir, "_ori_main", "nounwind");
    assert_fn_lacks_attr(&ir, "main", "nounwind");
}

// nounwind propagation through builtin methods and protocols

/// Function calling builtin method (str.length) via Invoke terminator gets nounwind.
///
/// The ARC IR lowers `.length()` to `Invoke @length(...)`, and the nounwind
/// analysis must recognize `@length` as an intercepted builtin that always
/// emits `call` (never `invoke`). Without this, the function and its callers
/// would incorrectly lose the nounwind attribute. The fixture bodies contain
/// no arithmetic — isolating this concern from the (separately pinned)
/// checked-arithmetic-taints-nounwind behavior.
#[test]
fn test_function_calling_builtin_method_gets_nounwind() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/function_calling_builtin_method_gets_nounwind.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/closure_call_gets_nounwind_via_posthoc.ori"
    ));

    assert_fn_has_attr(&ir, "_ori_check_capture", "nounwind");
    assert_fn_has_attr(&ir, "_ori_main", "nounwind");
    assert_fn_has_attr(&ir, "main", "nounwind");
}

/// Generic functions with may-unwind bodies must not be treated as intercepted.
/// A `mono_dispatch` match takes precedence over the builtin method heuristic
/// even when the first argument has a builtin type such as `str`.
#[test]
fn test_generic_call_with_builtin_arg_not_treated_as_intercepted() {
    let ir = compile_and_capture_ir(
        include_str!("fixtures/ir_quality_attributes/generic_call_with_builtin_arg_not_treated_as_intercepted.ori"),
    );

    // `mono_dispatch` identifies `might_panic` as a may-unwind generic call even
    // though its first argument has the builtin `str` type.
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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/pure_derived_methods_have_nounwind.ori"
    ));

    for method in ["eq", "compare", "hash"] {
        let symbol = resolve_derived_function_name(&ir, method);
        assert_fn_has_attr(&ir, symbol, "nounwind");
    }
}

/// Derived equality should test cheap scalar fields before managed fields.
#[test]
fn test_derived_eq_checks_scalar_fields_before_managed_fields() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/derived_eq_scalar_field_first.ori"
    ));
    let eq_symbol = resolve_derived_function_name(&ir, "eq");
    let eq_ir = extract_function_ir(&ir, eq_symbol);
    let scalar_projection = eq_ir
        .find("%proj.1")
        .expect("derived Eq should project the scalar field");

    let managed_projection = eq_ir
        .find("%proj.0")
        .expect("derived Eq should project the managed field");

    assert!(
        scalar_projection < managed_projection,
        "derived Eq should compare the scalar field before the managed field:\n{eq_ir}"
    );
}

/// Impure derived methods (`$to_str`, `$debug`) should NOT have `nounwind`.
///
/// These methods allocate strings and call non-nounwind runtime functions.
#[test]
fn test_impure_derived_methods_lack_nounwind() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/impure_derived_methods_lack_nounwind.ori"
    ));

    for method in ["to_str", "debug"] {
        let symbol = resolve_derived_function_name(&ir, method);
        assert_fn_lacks_attr(&ir, symbol, "nounwind");
    }
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
    let ir = compile_and_capture_ir_no_repr_opt(include_str!(
        "fixtures/ir_quality_attributes/panic_declarations_have_noreturn.ori"
    ));

    assert_fn_has_attr(&ir, "ori_panic_cstr", "noreturn");
    assert_fn_lacks_attr(&ir, "ori_panic_cstr", "nounwind");

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/scalar_params_have_noundef.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/indirect_str_param_attributes.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/indirect_str_param_attributes.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/indirect_str_param_attributes.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/direct_params_lack_nonnull.ori"
    ));

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
    let attrs = resolve_function_attrs(ir, func_name);
    assert!(
        attrs.contains(attr),
        "{func_name} should have `{attr}` attribute.\n\
         Resolved attributes: {attrs}"
    );
}

/// Assert that a function declaration does NOT have a specific attribute.
fn assert_fn_lacks_attr(ir: &str, func_name: &str, attr: &str) {
    let attrs = resolve_function_attrs(ir, func_name);
    assert!(
        !attrs.contains(attr),
        "{func_name} must NOT have `{attr}` attribute.\n\
         Resolved attributes: {attrs}"
    );
}

// Iterator option wrapping elimination

/// For-loop iterator codegen should NOT build a `{i64, T}` wrapper struct.
///
/// The ARC IR `__iter_next` protocol returns a decomposed `(tag, scratch_ptr)`
/// pair. The LLVM emission should use the tag directly and load the element
/// from the scratch buffer — no `insertvalue` to build a wrapper struct.
#[test]
fn test_iter_next_no_wrapper_struct() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/iter_next_no_wrapper_struct.ori"
    ));

    let count_ir = extract_function_ir(&ir, "_ori_count");

    // The wrapper struct `{i64, {i64, i64, ptr}}` required insertvalue.
    // After optimization, no insertvalue should appear for iter_next results.
    let insertvalue_count = count_ir.matches("insertvalue").count();
    assert_eq!(
        insertvalue_count, 0,
        "expected 0 insertvalue in @count (iter_next decomposed), got {insertvalue_count}.\nIR:\n{count_ir}"
    );

    let scratch_ptr = count_ir
        .lines()
        .find_map(|line| {
            let (_, arguments) = line.split_once("@ori_iter_next(")?;
            let (_, scratch_and_tail) = arguments.split_once(", ptr ")?;
            scratch_and_tail.split_once(',').map(|(scratch, _)| scratch)
        })
        .expect("expected ori_iter_next call with a scratch pointer");

    let scratch_alloca = format!("{scratch_ptr} = alloca {{ i64, i64, ptr }}");
    assert!(
        count_ir.contains(&scratch_alloca),
        "expected iter_next scratch argument to name its element alloca.\nIR:\n{count_ir}"
    );

    // The element consumer must read from the same scratch buffer passed to iter_next.
    let str_len_call = format!("call i64 @ori_str_len(ptr {scratch_ptr})");
    assert!(
        count_ir.contains(&str_len_call),
        "expected ori_str_len to receive the iter_next scratch pointer directly.\nIR:\n{count_ir}"
    );
}

/// Semantic pin: for-loop over `[str]` with `.length()` returns correct total.
#[test]
fn test_iter_for_loop_str_length_correctness() {
    let exit = crate::util::compile_and_run(include_str!(
        "fixtures/ir_quality_attributes/iter_for_loop_str_length_correctness.ori"
    ));
    assert_eq!(exit, 10, "expected total length 10 (5+5)");
}

/// Semantic pin: for-loop over `[[int]]` with `.length()` returns correct total.
#[test]
fn test_iter_for_loop_list_length_correctness() {
    let exit = crate::util::compile_and_run(include_str!(
        "fixtures/ir_quality_attributes/iter_for_loop_list_length_correctness.ori"
    ));
    assert_eq!(exit, 5, "expected total length 5 (3+2)");
}

/// Scalar elements (int) should not be affected by the optimization.
#[test]
fn test_iter_for_loop_scalar_element() {
    let exit = crate::util::compile_and_run(include_str!(
        "fixtures/ir_quality_attributes/iter_for_loop_scalar_element.ori"
    ));
    assert_eq!(exit, 12, "expected sum 12 (3+7+2)");
}

/// For-loop with break: element must be valid up to break point.
#[test]
fn test_iter_for_loop_with_break() {
    let exit = crate::util::compile_and_run(include_str!(
        "fixtures/ir_quality_attributes/iter_for_loop_with_break.ori"
    ));
    assert_eq!(exit, 2, "expected 2 (break after two 5-char words)");
}

/// Nested for-loops: outer element used in inner loop body.
#[test]
fn test_iter_nested_for_loops() {
    let exit = crate::util::compile_and_run(include_str!(
        "fixtures/ir_quality_attributes/iter_nested_for_loops.ori"
    ));
    assert_eq!(exit, 6, "expected 6 (1+2+3)");
}

/// Pins scratch-buffer field projection for by-value struct iterator elements.
#[test]
fn test_iter_for_loop_struct_field_access() {
    let exit = crate::util::compile_and_run(include_str!(
        "fixtures/ir_quality_attributes/iter_for_loop_struct_field_access.ori"
    ));
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
    let exit = crate::util::compile_and_run(include_str!(
        "fixtures/ir_quality_attributes/iter_for_loop_push_element.ori"
    ));
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
    let exit = crate::util::compile_and_run(include_str!(
        "fixtures/ir_quality_attributes/iter_for_loop_guarded.ori"
    ));
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
    let exit = crate::util::compile_and_run(include_str!(
        "fixtures/ir_quality_attributes/iter_for_yield_lengths.ori"
    ));
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
    let exit = crate::util::compile_and_run(include_str!(
        "fixtures/ir_quality_attributes/iter_for_loop_element_passed_to_function.ori"
    ));
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
    let exit = crate::util::compile_and_run(include_str!(
        "fixtures/ir_quality_attributes/iter_for_loop_two_calls_same_element.ori"
    ));
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
    let exit = crate::util::compile_and_run(include_str!(
        "fixtures/ir_quality_attributes/iter_for_yield_semantic_pin.ori"
    ));
    assert_eq!(exit, 6, "expected 6 (1+2+3) — for-yield semantic pin");
}

// regression: closure wrappers returning >16-byte types via sret
// must NOT add `noundef` on the hidden sret pointer parameter. The sret
// pointer is a compiler-managed ABI parameter, not a user value.

/// Verify that a capturing closure wrapper returning `str` (sret) does not
/// mark the sret pointer `noundef`. Regular params should still have `noundef`.
#[test]
fn test_closure_wrapper_sret_no_noundef() {
    let ir = crate::util::compile_and_capture_ir(include_str!(
        "fixtures/ir_quality_attributes/closure_wrapper_sret_no_noundef.ori"
    ));

    // Find any _ori_partial_* wrapper declaration — these are closure wrappers.
    // The test program captures `prefix` in a lambda returning `str` (>16 bytes),
    // which must produce an `_ori_partial_*` wrapper with an sret parameter.
    let wrapper_decl = ir.lines().find(|l| {
        (l.contains("_ori_partial_") || l.contains("\"_ori_partial_"))
            && l.contains("define ")
            && l.contains("sret(")
    });

    // The fixture's capturing closure must emit an sret wrapper.
    let decl = wrapper_decl.expect(
        "expected at least one _ori_partial_* wrapper with sret in IR — \
         the capturing closure returning str must emit a wrapper",
    );

    // INVARIANT: The first parameter segment containing sret excludes `noundef`.
    let sret_pos = decl
        .find("sret(")
        .expect("wrapper matched sret( in search but not here");
    let after_sret = &decl[sret_pos..];
    let sret_param_end = after_sret
        .find(',')
        .unwrap_or_else(|| after_sret.find(')').unwrap_or(after_sret.len()));
    let sret_param_text = &after_sret[..sret_param_end];
    assert!(
        !sret_param_text.contains("noundef"),
        "sret pointer parameter should NOT have noundef attribute:\n{decl}"
    );
}
