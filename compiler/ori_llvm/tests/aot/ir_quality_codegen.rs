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

/// Struct passed whole to another function must load all fields.
///
/// When a struct param is passed directly as an argument to another call
/// (not via Project), all fields must be loaded — the callee may use any.
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

    // `forward` passes `p` whole to `sum_big` — all fields must be loaded.
    // Big has 4 fields, so we need at least 4 loads.
    let load_count = fn_ir.matches("= load ").count();
    assert!(
        load_count >= 4,
        "expected at least 4 loads in _ori_forward (whole passthrough of 4-field struct), \
         but found {load_count}.\nIR:\n{fn_ir}"
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

    // Find the line that calls ori_panic.
    let lines: Vec<&str> = fn_ir.lines().collect();
    let panic_line_idx = lines
        .iter()
        .position(|l| {
            l.contains("call") && l.contains("ori_panic") && !l.contains("ori_panic_cstr")
        })
        .expect("expected call to ori_panic in _ori_may_panic");

    // The very next non-empty line after the panic call should be `unreachable`.
    let next_meaningful = lines[panic_line_idx + 1..]
        .iter()
        .find(|l| !l.trim().is_empty())
        .expect("expected instruction after ori_panic call");

    assert!(
        next_meaningful.trim() == "unreachable",
        "expected `unreachable` immediately after `call @ori_panic`, \
         but found: `{}`.\n\
         No RC cleanup or other code should follow a noreturn call.\nIR:\n{fn_ir}",
        next_meaningful.trim()
    );
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
