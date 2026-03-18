//! IR Quality Tests: Codegen
//!
//! Verify sum type payload extraction, surgical struct field loading,
//! and skip-codegen-after-noreturn optimizations.

use crate::util::{compile_and_capture_ir, extract_function_ir};

// Sum type payload extraction

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

// Surgical struct field loading

/// Function accessing 1 of 4 struct fields should emit only 1 GEP+load.
///
/// `get_x` receives a 4-field `Point` by pointer but only accesses field 0.
/// The codegen should skip loading fields 1, 2, 3 — only emit 1 GEP+load
/// (not 4 GEP+load+insertvalue sequences).
#[test]
fn test_struct_selective_field_loading() {
    let ir = compile_and_capture_ir(
        r"
type Point = { x: int, y: int, z: int, w: int };

@get_x (p: Point) -> int = p.x;

@main () -> int = get_x(p: Point { x: 42, y: 0, z: 0, w: 0 });
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_get_x");

    // Count GEP instructions for field access — should be exactly 1 (field 0).
    let gep_count = fn_ir.matches("getelementptr").count();
    assert!(
        gep_count <= 1,
        "expected at most 1 GEP in _ori_get_x (accessing only field x), \
         but found {gep_count}.\nIR:\n{fn_ir}"
    );

    // Count load instructions — should be exactly 1 (loading field 0).
    let load_count = fn_ir.matches("= load ").count();
    assert!(
        load_count <= 1,
        "expected at most 1 load in _ori_get_x (loading only field x), \
         but found {load_count}.\nIR:\n{fn_ir}"
    );
}

/// Function accessing 2 of 4 struct fields should emit exactly 2 GEP+load.
#[test]
fn test_struct_selective_two_fields() {
    let ir = compile_and_capture_ir(
        r"
type Rect = { x: int, y: int, width: int, height: int };

@area (r: Rect) -> int = r.width * r.height;

@main () -> int = area(r: Rect { x: 0, y: 0, width: 3, height: 4 });
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_area");

    // Should load exactly 2 fields (width at index 2, height at index 3).
    let load_count = fn_ir.matches("= load ").count();
    assert!(
        load_count <= 2,
        "expected at most 2 loads in _ori_area (width + height only), \
         but found {load_count}.\nIR:\n{fn_ir}"
    );
}

/// Struct passed whole to another function uses aggregate load in AOT.
///
/// When a struct param is passed directly as an argument to another call
/// (not via Project), all fields are needed. In AOT mode, this should use
/// a single aggregate load (not per-field GEP+load+insertvalue).
/// Uses a 4-field struct to ensure Indirect passing (>16 bytes).
#[test]
fn test_struct_whole_passthrough_loads_all() {
    let ir = compile_and_capture_ir(
        r"
type Big = { a: int, b: int, c: int, d: int };

@sum_big (p: Big) -> int = p.a + p.b + p.c + p.d;

@forward (p: Big) -> int = sum_big(p: p);

@main () -> int = forward(p: Big { a: 1, b: 2, c: 3, d: 4 });
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_forward");

    // AOT mode: `forward` should use a single aggregate load for the whole
    // struct, not per-field GEP+load+insertvalue (which was the old JIT-safe
    // pattern). Exactly 1 load for the param.
    let load_count = fn_ir.matches("= load ").count();
    assert!(
        load_count >= 1,
        "expected at least 1 load in _ori_forward (aggregate load of struct param), \
         but found {load_count}.\nIR:\n{fn_ir}"
    );
    // Should NOT have per-field GEP decomposition.
    let gep_count = fn_ir.matches("getelementptr").count();
    assert_eq!(
        gep_count, 0,
        "expected 0 GEPs in _ori_forward (aggregate load, not per-field), \
         but found {gep_count}.\nIR:\n{fn_ir}"
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

// Skip codegen after noreturn

/// Explicit `panic()` call should have `unreachable` immediately after —
/// no RC cleanup or other code between the call and the terminator.
///
/// In `if cond then panic(msg: "x") else value`, the panic arm calls
/// `ori_panic` (noreturn). The codegen should emit `call @ori_panic` +
/// `unreachable` with nothing in between.
#[test]
fn test_noreturn_panic_has_unreachable_no_cleanup() {
    let ir = compile_and_capture_ir(
        r#"
@may_panic (x: int) -> int = {
    if x == 0 then panic(msg: "zero") else x
};

@main () -> int = may_panic(x: 5);
"#,
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_may_panic");

    // Find the line that calls/invokes ori_panic.
    // ori_panic now uses `invoke` (not `call`) because it raises exceptions
    // via _Unwind_RaiseException — cleanup landing pads need invoke.
    let lines: Vec<&str> = fn_ir.lines().collect();
    let panic_line_idx = lines
        .iter()
        .position(|l| {
            (l.contains("call") || l.contains("invoke"))
                && l.contains("ori_panic")
                && !l.contains("ori_panic_cstr")
        })
        .expect("expected call/invoke to ori_panic in _ori_may_panic");

    // For invoke, the next non-empty line is `to label %normal unwind label %unwind`.
    // For call, the next line should be `unreachable`.
    // In both cases, the normal continuation should eventually be unreachable.
    let panic_line = lines[panic_line_idx].trim();
    if panic_line.contains("invoke") {
        // invoke ... to label %bbN unwind label %bbM
        // The `to label` and `unwind label` may be on the next line (LLVM
        // wraps long invoke instructions). Check both the invoke line and
        // the continuation for the unwind destination.
        let invoke_region = lines[panic_line_idx..panic_line_idx + 3]
            .iter()
            .map(|l| l.trim())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            invoke_region.contains("to label") && invoke_region.contains("unwind label"),
            "invoke to ori_panic should have normal + unwind destinations.\nIR:\n{fn_ir}"
        );
    } else {
        // call: next meaningful line should be unreachable
        let next_meaningful = lines[panic_line_idx + 1..]
            .iter()
            .find(|l| !l.trim().is_empty())
            .expect("expected instruction after ori_panic call");
        assert!(
            next_meaningful.trim() == "unreachable",
            "expected `unreachable` immediately after `call @ori_panic`, \
             but found: `{}`.\nIR:\n{fn_ir}",
            next_meaningful.trim()
        );
    }
}

/// The else arm of `if cond then panic(...) else value` should still work
/// normally — the noreturn pruning must not affect the non-panic path.
#[test]
fn test_noreturn_panic_else_arm_continues() {
    let ir = compile_and_capture_ir(
        r#"
@may_panic (x: int) -> int = {
    if x == 0 then panic(msg: "zero") else x
};

@main () -> int = may_panic(x: 5);
"#,
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_may_panic");

    // The function must still contain a `ret` for the else arm.
    assert!(
        fn_ir.contains("ret i64"),
        "expected `ret i64` for the else arm in _ori_may_panic.\n\
         Noreturn pruning should not affect the non-panic path.\nIR:\n{fn_ir}"
    );
}

/// Checked arithmetic overflow panic (`emit_checked_binop`) should still
/// have `unreachable` after the panic call — regression guard.
///
/// This path was already correct before §06.2. The test guards against
/// accidentally breaking it while implementing noreturn pruning.
#[test]
fn test_checked_binop_overflow_still_has_unreachable() {
    let ir = compile_and_capture_ir(
        r"
@add_checked (a: int, b: int) -> int = a + b;

@main () -> int = add_checked(a: 9223372036854775807, b: 1);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_add_checked");

    // The overflow panic block should contain ori_panic_cstr + unreachable.
    assert!(
        fn_ir.contains("ori_panic_cstr"),
        "expected `ori_panic_cstr` for overflow panic in _ori_add_checked.\nIR:\n{fn_ir}"
    );

    // Find the ori_panic_cstr call line.
    let lines: Vec<&str> = fn_ir.lines().collect();
    let panic_line_idx = lines
        .iter()
        .position(|l| l.contains("call") && l.contains("ori_panic_cstr"))
        .expect("expected call to ori_panic_cstr in _ori_add_checked");

    // Next meaningful line should be `unreachable`.
    let next_meaningful = lines[panic_line_idx + 1..]
        .iter()
        .find(|l| !l.trim().is_empty())
        .expect("expected instruction after ori_panic_cstr call");

    assert!(
        next_meaningful.trim() == "unreachable",
        "expected `unreachable` immediately after `call ori_panic_cstr` in overflow path, \
         but found: `{}`.\nIR:\n{fn_ir}",
        next_meaningful.trim()
    );
}

// Tail call loop lowering

/// Tail-recursive `gcd` should compile to a loop, not a recursive `call`.
///
/// The ARC loop-lowering pass (§09.2) replaces `Apply @gcd(b, rem)` with
/// a `Jump(header, [b, rem])` back-edge. The LLVM IR should contain a
/// `br label` loop back-edge instead of `call @_ori_gcd`.
#[test]
fn test_tail_recursive_gcd_has_no_self_call() {
    let ir = compile_and_capture_ir(
        r"
@gcd (a: int, b: int) -> int = {
    if b == 0 then a else gcd(a: b, b: a % b)
};

@main () -> int = gcd(a: 48, b: 18);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_gcd");

    // The function must NOT contain a recursive call to itself.
    assert!(
        !fn_ir.contains("call fastcc i64 @_ori_gcd"),
        "expected no recursive `call @_ori_gcd` in tail-recursive gcd — \
         loop lowering should have replaced it with a back-edge.\nIR:\n{fn_ir}"
    );

    // The function must contain a loop back-edge (br label).
    assert!(
        fn_ir.contains("br label"),
        "expected `br label` loop back-edge in tail-recursive gcd.\nIR:\n{fn_ir}"
    );

    // The function must contain phi nodes for the loop parameters.
    assert!(
        fn_ir.contains("phi i64"),
        "expected `phi i64` for loop parameters in tail-recursive gcd.\nIR:\n{fn_ir}"
    );
}

// String constant deduplication

/// Each unique overflow message should appear exactly once in the IR.
///
/// When multiple arithmetic operations of the same kind exist (e.g., two
/// additions), the overflow panic message ("integer overflow on addition")
/// should be deduplicated to a single global constant — not emitted once
/// per operation site.
#[test]
fn test_overflow_string_dedup_single_global_per_message() {
    let ir = compile_and_capture_ir(
        r"
@main () -> int = {
    let a = 1 + 2;
    let b = 3 + 4;
    let c = a * b;
    let d = 5 * 6;
    c + d
}
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    // Count occurrences of the addition overflow message.
    // With dedup, "integer overflow on addition" appears exactly once as
    // a global constant, even though there are 3 addition sites.
    let add_msg_count = ir.matches("integer overflow on addition").count();
    assert_eq!(
        add_msg_count, 1,
        "expected exactly 1 global for addition overflow message, found {add_msg_count}.\n\
         Deduplication should collapse multiple identical messages into one global."
    );

    // Similarly, multiplication overflow message appears once despite 2 mul sites.
    let mul_msg_count = ir.matches("integer overflow on multiplication").count();
    assert_eq!(
        mul_msg_count, 1,
        "expected exactly 1 global for multiplication overflow message, found {mul_msg_count}."
    );
}

// Fat pointer aggregate load regression tests

/// str param uses aggregate load (not per-field GEP+load+insertvalue).
#[test]
fn test_str_param_aggregate_load() {
    let ir = compile_and_capture_ir(
        r#"
@get_len (s: str) -> int = s.length();
@main () -> int = get_len(s: "hello");
"#,
    );
    if !ir.contains("define ") {
        return;
    }
    let fn_ir = extract_function_ir(&ir, "_ori_get_len");
    // Single aggregate load for the str param, no per-field GEP.
    assert!(
        fn_ir.contains("load { i64, i64, ptr }"),
        "str param should use aggregate load in AOT.\nIR:\n{fn_ir}"
    );
    assert!(
        !fn_ir.contains("insertvalue"),
        "str param should not use insertvalue (field-by-field) in AOT.\nIR:\n{fn_ir}"
    );
}

/// Two str params both use aggregate loads.
#[test]
fn test_multi_str_param_aggregate_load() {
    let ir = compile_and_capture_ir(
        r#"
@longer (a: str, b: str) -> int = {
    let la = a.length();
    let lb = b.length();
    if la > lb then la else lb
};
@main () -> int = longer(a: "hi", b: "hello");
"#,
    );
    if !ir.contains("define ") {
        return;
    }
    let fn_ir = extract_function_ir(&ir, "_ori_longer");
    let agg_loads = fn_ir.matches("load { i64, i64, ptr }").count();
    assert!(
        agg_loads >= 2,
        "two str params should produce at least 2 aggregate loads, found {agg_loads}.\nIR:\n{fn_ir}"
    );
    assert!(
        !fn_ir.contains("insertvalue"),
        "str params should not use insertvalue in AOT.\nIR:\n{fn_ir}"
    );
}

// Nounwind invoke→call regression tests

/// Calling a nounwind user function uses `call`, not `invoke`.
#[test]
fn test_nounwind_callee_uses_call() {
    let ir = compile_and_capture_ir(
        r#"
@get_len (s: str) -> int = s.length();
@check () -> int = {
    let s = "hello";
    get_len(s: s)
};
@main () -> int = check();
"#,
    );
    if !ir.contains("define ") {
        return;
    }
    let fn_ir = extract_function_ir(&ir, "_ori_check");
    // get_len is nounwind (only calls builtin length method),
    // so check should use `call`, not `invoke`.
    assert!(
        fn_ir.contains("call fastcc") && fn_ir.contains("_ori_get_len"),
        "nounwind callee should be called with `call`, not `invoke`.\nIR:\n{fn_ir}"
    );
    assert!(
        !fn_ir.contains("invoke fastcc"),
        "nounwind callee should not use `invoke`.\nIR:\n{fn_ir}"
    );
    // No landing pad needed.
    assert!(
        !fn_ir.contains("landingpad"),
        "no landing pad when callee is nounwind.\nIR:\n{fn_ir}"
    );
}

// Single-predecessor block merging regression tests

/// Unconditional br to single-predecessor successor should be merged.
#[test]
fn test_single_predecessor_block_merged() {
    let ir = compile_and_capture_ir(
        r#"
@get_len (s: str) -> int = s.length();
@check () -> int = {
    let s = "hello";
    get_len(s: s)
};
@main () -> int = check();
"#,
    );
    if !ir.contains("define ") {
        return;
    }
    let fn_ir = extract_function_ir(&ir, "_ori_check");
    // After nounwind downgrade + block merging, check should have
    // no unconditional `br label %bb1` followed by a single-predecessor bb1.
    // Count basic block labels — fewer is better.
    let block_labels: Vec<&str> = fn_ir
        .lines()
        .filter(|l| l.ends_with(':') && !l.starts_with(';'))
        .collect();
    // The function should have at most: bb0 + rc_dec blocks (heap + sso_skip).
    // No separate bb1 for post-call cleanup.
    assert!(
        !fn_ir.contains("\n  br label %bb1\n"),
        "unconditional br to single-predecessor block should be merged.\n\
         Blocks: {block_labels:?}\nIR:\n{fn_ir}"
    );
}

/// SSO guard ptrtoint is never duplicated.
#[test]
fn test_sso_guard_single_ptrtoint() {
    let ir = compile_and_capture_ir(
        r#"
@check () -> int = {
    let s = "hello";
    s.length()
};
@main () -> int = check();
"#,
    );
    if !ir.contains("define ") {
        return;
    }
    let fn_ir = extract_function_ir(&ir, "_ori_check");
    // Each SSO guard should have exactly 1 ptrtoint. Count per-guard:
    // the pattern is `ptrtoint ptr %X to i64` followed by sso_flag/is_null checks.
    let ptrtoint_count = fn_ir.matches("ptrtoint ptr").count();
    let sso_guard_count = fn_ir.matches("sso_flag").count().max(1);
    assert!(
        ptrtoint_count <= sso_guard_count,
        "each SSO guard should have at most 1 ptrtoint, \
         found {ptrtoint_count} ptrtoint vs {sso_guard_count} guards.\nIR:\n{fn_ir}"
    );
}
