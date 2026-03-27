//! Integer narrowing AOT tests (§04.4).
//!
//! Tests that struct field integer narrowing produces correct runtime behavior:
//! trunc at construction + sext at extraction = identical semantics to canonical i64.

use crate::util::{assert_aot_success, compile_and_capture_ir, extract_function_ir};

/// Semantic pin: struct with all fields in [-128, 127] range.
/// Field values must survive trunc (i64→i8 at construction) and
/// sext (i8→i64 at extraction) without data corruption.
#[test]
fn test_narrowed_struct_pixel_round_trip() {
    assert_aot_success(
        r"
type Pixel = { r: int, g: int, b: int, a: int }

@main () -> int = {
    let p = Pixel { r: -128, g: 0, b: 127, a: 42 };
    let sum = p.r + p.g + p.b + p.a;
    // -128 + 0 + 127 + 42 = 41
    if sum == 41 then 0 else 1
}
",
        "narrowed_pixel_round_trip",
    );
}

/// Struct update syntax with narrowed fields — the spread creates a new
/// struct from old field values, each of which goes through extract→trunc.
#[test]
fn test_narrowed_struct_update() {
    assert_aot_success(
        r"
type Point = { x: int, y: int }

@main () -> int = {
    let p1 = Point { x: 10, y: 20 };
    let p2 = Point { ...p1, x: 30 };
    if p2.x == 30 && p2.y == 20 then 0 else 1
}
",
        "narrowed_struct_update",
    );
}

/// Struct with mixed field types — only int fields should be narrowed.
/// Non-int fields (str, bool, float) must pass through unaffected.
#[test]
fn test_narrowed_struct_mixed_types() {
    assert_aot_success(
        r#"
type Record = { count: int, name: str, active: bool }

@main () -> int = {
    let r = Record { count: 42, name: "hello", active: true };
    let ok_count = r.count == 42;
    let ok_name = r.name == "hello";
    let ok_active = r.active;
    if ok_count && ok_name && ok_active then 0 else 1
}
"#,
        "narrowed_struct_mixed_types",
    );
}

/// Struct field mutation — mutable fields that go through the update path
/// must correctly trunc the new value and sext the old values.
#[test]
fn test_narrowed_struct_field_mutation() {
    assert_aot_success(
        r"
type Counter = { value: int, step: int }

@main () -> int = {
    let c = Counter { value: 0, step: 5 };
    let c = Counter { ...c, value: c.value + c.step };
    let c = Counter { ...c, value: c.value + c.step };
    // 0 + 5 + 5 = 10
    if c.value == 10 then 0 else 1
}
",
        "narrowed_struct_field_mutation",
    );
}

/// Boundary value test: signed i8 boundaries (-128 and 127).
/// These are the exact limits where narrowing to i8 is valid.
#[test]
fn test_narrowed_struct_i8_boundaries() {
    assert_aot_success(
        r"
type Bounds = { lo: int, hi: int }

@main () -> int = {
    let b = Bounds { lo: -128, hi: 127 };
    let ok1 = b.lo == -128;
    let ok2 = b.hi == 127;
    let ok3 = b.hi - b.lo == 255;
    if ok1 && ok2 && ok3 then 0 else 1
}
",
        "narrowed_struct_i8_boundaries",
    );
}

// CROSS-04-015 multi-file AOT semantic pin tests are blocked on multi-file
// AOT compilation being incomplete (roadmap Section 4: Modules). The ARC IR
// emitter currently cannot resolve cross-module function calls. The plumbing
// for ExportedTypeMetadata is verified by:
// 1. Unit tests in compiler/oric/src/commands/build/tests.rs
//    (collect_imported_type_metadata correctness)
// 2. Existing ori_repr tests (imported metadata prevents narrowing)
// 3. Compilation succeeds with the new parameter threading

/// Negative semantic pin: struct with Top-range fields must NOT be narrowed.
/// Fields used with values spanning the full i64 range stay at i64.
#[test]
fn test_non_narrowed_struct_wide_range() {
    assert_aot_success(
        r"
type Wide = { a: int, b: int }

@main () -> int = {
    let w = Wide { a: 1_000_000_000, b: -1_000_000_000 };
    if w.a + w.b == 0 then 0 else 1
}
",
        "non_narrowed_struct_wide_range",
    );
}

// ---- IR semantic pin tests (IR-PIN-04-018) ----
//
// These tests inspect the actual LLVM IR to verify narrowing produces
// the expected type layouts and trunc/sext boundary instructions.
// Unlike the runtime-only tests above, these pin the codegen output
// so a regression that silently disables narrowing is caught.

/// IR semantic pin: narrowed struct field loads must produce `sext i8 ... to i64`.
///
/// A separate function taking a Pixel parameter forces the struct type and
/// field extraction to appear in the function's IR (not folded away in main).
#[test]
fn test_narrowed_struct_ir_pin_sext_on_field_load() {
    let ir = compile_and_capture_ir(
        r"
type Pixel = { r: int, g: int, b: int, a: int }

@sum_channels (p: Pixel) -> int = p.r + p.g + p.b + p.a;

@main () -> int = {
    let p = Pixel { r: 10, g: 20, b: 30, a: 40 };
    sum_channels(p:)
}
",
    );

    let fn_ir = extract_function_ir(&ir, "_ori_sum_channels");

    // Narrowed Pixel fields are i8. Loading them produces i8 values that
    // must be sign-extended to i64 for canonical-width arithmetic.
    assert!(
        fn_ir.contains("sext i8"),
        "expected `sext i8 ... to i64` in _ori_sum_channels — narrowed Pixel fields \
         should produce i8 loads that need sign extension to canonical i64.\n\
         This test is a regression guard: if narrowing is disabled, fields stay i64 \
         and no sext appears.\nIR:\n{fn_ir}"
    );
}

/// IR semantic pin: narrowed struct construction must insert `trunc i64 ... to i8`.
///
/// When constructing a narrowed struct from canonical-width (i64) values,
/// the codegen must truncate each value to the narrowed field width.
/// Construction happens in `@main` with known-bounded constants so the
/// field range analysis produces bounded ranges (interprocedural range
/// propagation for function parameters is not yet implemented).
#[test]
fn test_narrowed_struct_ir_pin_trunc_on_construction() {
    let ir = compile_and_capture_ir(
        r"
type Color = { r: int, g: int, b: int }

@read_r (c: Color) -> int = c.r;

@main () -> int = {
    let c = Color { r: 10, g: 20, b: 30 };
    read_r(c:)
}
",
    );

    let main_ir = extract_function_ir(&ir, "_ori_main");

    // The i64 constants (10, 20, 30) are stored into narrowed i8 struct
    // fields. For constants, LLVM folds `trunc i64 10 to i8` → `i8 10`
    // at IR construction time, so we may see either an explicit `trunc`
    // instruction or a constant struct with i8 values. Either proves the
    // narrowed struct type is in use.
    let has_trunc = main_ir.contains("trunc i64");
    let has_narrowed_store = main_ir.contains("{ i8, i8, i8 }");
    assert!(
        has_trunc || has_narrowed_store,
        "expected evidence of narrowed struct construction in _ori_main — either \
         `trunc i64 ... to i8` instructions or a `{{ i8, i8, i8 }}` constant store.\n\
         This test is a regression guard: without narrowing, the struct type would be \
         `{{ i64, i64, i64 }}`.\nIR:\n{main_ir}"
    );
}

/// IR semantic pin: narrowed struct type contains i8 fields.
///
/// Verifies the LLVM struct type for a narrowed Pixel is `{ i8, i8, i8, i8 }`
/// rather than the canonical `{ i64, i64, i64, i64 }`.
#[test]
fn test_narrowed_struct_ir_pin_type_layout() {
    let ir = compile_and_capture_ir(
        r"
type Pixel = { r: int, g: int, b: int, a: int }

@read_pixel (p: Pixel) -> int = p.r;

@main () -> int = {
    let p = Pixel { r: 42, g: 0, b: 0, a: 0 };
    read_pixel(p:)
}
",
    );

    let fn_ir = extract_function_ir(&ir, "_ori_read_pixel");

    // The function IR should reference the narrowed struct type. With 4 int
    // fields all in [-128, 127], the type is { i8, i8, i8, i8 }. The GEP
    // instruction reveals the struct type it operates on.
    let has_narrowed_type =
        fn_ir.contains("{ i8, i8, i8, i8 }") || fn_ir.contains("{i8, i8, i8, i8}");
    assert!(
        has_narrowed_type,
        "expected narrowed struct type `{{ i8, i8, i8, i8 }}` in _ori_read_pixel IR — \
         Pixel fields with range [-128, 127] should be narrowed to i8.\n\
         This test is a regression guard: without narrowing, the type would be \
         `{{ i64, i64, i64, i64 }}`.\nIR:\n{fn_ir}"
    );
}

/// Negative IR semantic pin: wide-range struct must NOT show narrowing patterns.
///
/// When struct field ranges exceed i32 bounds, fields stay at canonical i64.
/// The function IR must NOT contain `sext i8/i16/i32` or `trunc i64` patterns
/// that would indicate incorrect narrowing.
#[test]
fn test_non_narrowed_struct_ir_pin_wide_range() {
    let ir = compile_and_capture_ir(
        r"
type Wide = { a: int, b: int }

@sum_wide (w: Wide) -> int = w.a + w.b;

@main () -> int = {
    let w = Wide { a: 3_000_000_000, b: -3_000_000_000 };
    sum_wide(w:)
}
",
    );

    let fn_ir = extract_function_ir(&ir, "_ori_sum_wide");

    // Fields with values 3B and -3B exceed i32 range — no narrowing.
    // The function should NOT contain narrowing-specific sext instructions.
    assert!(
        !fn_ir.contains("sext i8") && !fn_ir.contains("sext i16") && !fn_ir.contains("sext i32"),
        "expected NO narrowing sext in _ori_sum_wide — field values 3_000_000_000 \
         and -3_000_000_000 exceed i32 range, so fields must stay i64.\nIR:\n{fn_ir}"
    );
}
