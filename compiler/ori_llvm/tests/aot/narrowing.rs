//! Integer narrowing AOT tests (§04.4).
//!
//! Tests that struct field integer narrowing produces correct runtime behavior:
//! trunc at construction + sext at extraction = identical semantics to canonical i64.

use crate::util::assert_aot_success;

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
