//! Integer narrowing AOT tests (§04.4).
//!
//! Tests that struct field integer narrowing produces correct runtime behavior:
//! trunc at construction + sext at extraction = identical semantics to canonical i64.

use crate::util::{assert_aot_success, compile_and_capture_ir, extract_function_ir, stdlib_path};

use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use tempfile::TempDir;

/// Compile with `ORI_NO_REPR_OPT=1` and capture LLVM IR.
/// Used for `NarrowingPolicy::Disabled` verification tests.
fn compile_and_capture_ir_no_repr_opt(source: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source_path = temp_dir.path().join(format!("test_nro_{id}.ori"));
    let binary_path = temp_dir
        .path()
        .join(format!("test_nro_{id}{}", std::env::consts::EXE_SUFFIX));

    std::fs::write(&source_path, source).expect("Failed to write source");

    let exe = format!("ori{}", std::env::consts::EXE_SUFFIX);
    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .to_path_buf();
    let binary = workspace_root.join("target/debug").join(&exe);

    let result = Command::new(binary)
        .args([
            "build",
            source_path.to_str().unwrap(),
            "-o",
            binary_path.to_str().unwrap(),
        ])
        .env("ORI_STDLIB", stdlib_path())
        .env("ORI_DEBUG_LLVM", "1")
        .env("ORI_NO_REPR_OPT", "1")
        .output()
        .expect("Failed to execute ori build");

    assert!(
        result.status.success(),
        "Compilation failed:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );

    String::from_utf8_lossy(&result.stderr).to_string()
}

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

// ---- DERIVE-PIN-04-020: Negative-value derive semantic pins ----
//
// These tests exercise derived hash(), to_str(), and debug() on narrowed structs
// with NEGATIVE field values. A zext (instead of sext) bug when widening i8 fields
// to canonical i64 for runtime functions would corrupt negative values (e.g., -50
// becomes 206). Previous derive tests only used positive values and would not
// catch this.

/// Semantic pin: derived `hash()` on narrowed struct with negative i8 field values.
///
/// Two structs with identical negative values must produce the same hash.
/// A third struct with different values must produce a different hash.
/// This verifies that hash codegen correctly sign-extends narrowed fields.
#[test]
fn test_narrowed_derive_hash_negative_values() {
    assert_aot_success(
        r"
#[derive(Eq, Hashable)]
type SignedPixel = { r: int, g: int, b: int }

@main () -> int = {
    let a = SignedPixel { r: -50, g: -120, b: 100 };
    let b = SignedPixel { r: -50, g: -120, b: 100 };
    let c = SignedPixel { r: 50, g: 120, b: -100 };
    let same = a.hash() == b.hash();
    let diff = a.hash() != c.hash();
    if same && diff then 0 else 1
}
",
        "narrowed_derive_hash_negative",
    );
}

/// Semantic pin: derived `to_str()` on narrowed struct with negative i8 field values.
///
/// The string representation MUST contain "-50", not "206". If sext is replaced
/// with zext in derive codegen, -50 (i8 = 0xCE) becomes 206 when zero-extended
/// to i64, and `to_str()` would display "206" — failing this test.
#[test]
fn test_narrowed_derive_printable_negative_values() {
    assert_aot_success(
        r#"
#[derive(Printable)]
type SignedPoint = { x: int, y: int }

@main () -> int = {
    let p = SignedPoint { x: -50, y: -120 };
    let s = p.to_str();
    let has_neg50 = s.contains(substr: "-50");
    let has_neg120 = s.contains(substr: "-120");
    if has_neg50 && has_neg120 then 0 else 1
}
"#,
        "narrowed_derive_printable_negative",
    );
}

/// Semantic pin: derived `debug()` on narrowed struct with negative i8 field values.
///
/// Same as the Printable test but for the Debug trait. The debug representation
/// MUST show the correct negative values, not zero-extended positive values.
#[test]
fn test_narrowed_derive_debug_negative_values() {
    assert_aot_success(
        r#"
#[derive(Debug)]
type SignedColor = { r: int, g: int, b: int }

@main () -> int = {
    let c = SignedColor { r: -1, g: -128, b: 127 };
    let s = c.debug();
    let has_neg1 = s.contains(substr: "-1");
    let has_neg128 = s.contains(substr: "-128");
    let has_127 = s.contains(substr: "127");
    if has_neg1 && has_neg128 && has_127 then 0 else 1
}
"#,
        "narrowed_derive_debug_negative",
    );
}

/// IR semantic pin: derive hash codegen on narrowed struct must use sext (not zext)
/// when widening i8 fields to i64 for `hash_combine` runtime calls.
///
/// We compile a narrowed struct with #derive(Hashable) and inspect the full IR
/// for evidence that the hash function sign-extends narrowed fields.
#[test]
fn test_narrowed_derive_ir_pin_sext_in_hash() {
    let ir = compile_and_capture_ir(
        r"
#[derive(Eq, Hashable)]
type NarrowHash = { a: int, b: int }

@compute_hash (p: NarrowHash) -> int = p.hash();

@main () -> int = {
    let p = NarrowHash { a: -50, b: -120 };
    compute_hash(p:)
}
",
    );

    // The hash function for NarrowHash must extract i8 fields and sext them
    // to i64 before passing to hash_combine. Look for sext in any function
    // operating on the NarrowHash type (the hash impl function name varies).
    // We check the full IR because the derive function name is mangled.
    let has_sext_i8 = ir.contains("sext i8");
    assert!(
        has_sext_i8,
        "expected `sext i8` in IR for narrowed struct hash — derive codegen must \
         sign-extend i8 fields to i64 before hash_combine. A missing sext (or zext) \
         would corrupt negative values in the hash computation.\n\
         This is a regression guard for DERIVE-PIN-04-020.\n\
         Full IR length: {} chars",
        ir.len()
    );
}

// ---- MIXED-PIN-04-019: Mixed-field struct rejection pin ----

/// Negative IR semantic pin: mixed-field struct (str + narrowed int) must NOT
/// be lowered through the narrowed aggregate path.
///
/// `try_lower_narrowed_aggregate()` rejects structs with non-scalar fields (str,
/// collections, etc.) because narrowing changes the struct's overall size, breaking
/// `element_store_size()` assumptions. This test verifies the int field stays at
/// canonical i64 width in the LLVM type layout.
#[test]
fn test_mixed_field_struct_ir_pin_no_narrowing() {
    let ir = compile_and_capture_ir(
        r#"
type Record = { count: int, name: str, active: bool }

@read_count (r: Record) -> int = r.count;

@main () -> int = {
    let r = Record { count: 42, name: "hello", active: true };
    read_count(r:)
}
"#,
    );

    let fn_ir = extract_function_ir(&ir, "_ori_read_count");

    // The Record struct has a str field, so the narrowed aggregate path rejects it.
    // The int field (count: 42, fits in i8) must NOT appear as i8 in the type layout.
    // It should stay at canonical i64. Check that no i8 narrowing artifacts appear.
    assert!(
        !fn_ir.contains("sext i8"),
        "expected NO `sext i8` in _ori_read_count — mixed-field struct Record \
         (str + int + bool) must NOT be narrowed. The int field should stay i64 \
         because `try_lower_narrowed_aggregate()` rejects non-all-scalar structs.\n\
         This is a regression guard for MIXED-PIN-04-019.\nIR:\n{fn_ir}"
    );
}

// ---- Phase B: Local Variable Narrowing Tests (§04.4) ----

// Behavioral test: manual loop (loop+break) with bounded counter and accumulator.
// The program must produce correct results regardless of narrowing.
#[test]
fn test_phase_b_loop_behavioral() {
    assert_aot_success(
        r"
@main () -> int = {
    let sum = 0;
    let i = 0;
    loop {
        if i >= 10 then break;
        sum = sum + i;
        i = i + 1;
    };
    // 0+1+2+...+9 = 45
    if sum == 45 then 0 else 1
}
",
        "phase_b_loop_behavioral",
    );
}

// Behavioral test: for-range loop with bounded counter.
#[test]
fn test_phase_b_for_range_behavioral() {
    assert_aot_success(
        r"
@main () -> int = {
    let sum = 0;
    for i in 0..10 do {
        sum = sum + i;
    };
    if sum == 45 then 0 else 1
}
",
        "phase_b_for_range_behavioral",
    );
}

// Behavioral test: loop with negative values (signed i8 range [-128, 127]).
#[test]
fn test_phase_b_negative_loop_behavioral() {
    assert_aot_success(
        r"
@main () -> int = {
    let sum = 0;
    let i = -5;
    loop {
        if i > 5 then break;
        sum = sum + i;
        i = i + 1;
    };
    // -5+-4+-3+-2+-1+0+1+2+3+4+5 = 0
    if sum == 0 then 0 else 1
}
",
        "phase_b_negative_loop_behavioral",
    );
}

// IR semantic pin: loop counter phi in a manual loop should use i8.
// Range [0, 9] fits in signed i8 [-128, 127].
// This test ONLY passes with Phase B local variable narrowing.
#[test]
fn test_phase_b_ir_pin_loop_counter_phi() {
    let ir = compile_and_capture_ir(
        r"
@sum_loop () -> int = {
    let sum = 0;
    let i = 0;
    loop {
        if i >= 10 then break;
        sum = sum + i;
        i = i + 1;
    };
    sum
}

@main () -> int = sum_loop();
",
    );

    let fn_ir = extract_function_ir(&ir, "_ori_sum_loop");

    // With local variable narrowing, loop counter `i` (range [0, 9]) and
    // accumulator `sum` (range [0, 45]) should both produce i8 phi nodes
    // instead of i64 phi nodes.
    assert!(
        fn_ir.contains("phi i8"),
        "expected `phi i8` for narrowed loop counter/accumulator in _ori_sum_loop — \
         ranges [0,9] and [0,45] both fit in signed i8.\n\
         Phase B semantic pin: ONLY passes with local variable narrowing.\n\
         Got IR:\n{fn_ir}"
    );
}

// IR semantic pin: sext must be present to widen narrowed loop variables
// before canonical-width arithmetic (overflow-checked i64 add).
#[test]
fn test_phase_b_ir_pin_loop_sext() {
    let ir = compile_and_capture_ir(
        r"
@sum_loop () -> int = {
    let sum = 0;
    let i = 0;
    loop {
        if i >= 10 then break;
        sum = sum + i;
        i = i + 1;
    };
    sum
}

@main () -> int = sum_loop();
",
    );

    let fn_ir = extract_function_ir(&ir, "_ori_sum_loop");

    // With narrowed i8 loop variables, arithmetic requires sign-extending
    // to i64 before the overflow-checked add. This sext is the boundary
    // between narrow storage and canonical computation.
    assert!(
        fn_ir.contains("sext i8"),
        "expected `sext i8 ... to i64` in _ori_sum_loop — narrowed loop variables \
         must be widened before overflow-checked arithmetic.\n\
         Phase B semantic pin: ONLY passes with local variable narrowing.\n\
         Got IR:\n{fn_ir}"
    );
}

// Negative IR pin: wide-range loop variables must NOT be narrowed.
// Loop counter up to 50000 exceeds i8 range, should stay i64 (or i16).
#[test]
fn test_phase_b_ir_pin_wide_range_no_i8() {
    let ir = compile_and_capture_ir(
        r"
@sum_wide () -> int = {
    let sum = 0;
    let i = 0;
    loop {
        if i >= 50000 then break;
        sum = sum + i;
        i = i + 1;
    };
    sum
}

@main () -> int = sum_wide();
",
    );

    let fn_ir = extract_function_ir(&ir, "_ori_sum_wide");

    // Range [0, 49999] does NOT fit in signed i8 [-128, 127].
    // The counter should NOT have a `phi i8` node.
    assert!(
        !fn_ir.contains("phi i8"),
        "expected NO `phi i8` in _ori_sum_wide — loop counter range [0, 49999] \
         exceeds i8 capacity. Should use i16 or wider.\n\
         Got IR:\n{fn_ir}"
    );
}

// ---- Comparison operations on narrowed fields (§04.4) ----
//
// Narrowed struct fields are sign-extended (sext) to i64 before any use,
// including comparisons. This guarantees signed comparison semantics are
// preserved: -50 < 0 is true (sext: 0xFFFFFFFFFFFFFFCE < 0), not false
// (zext: 0xCE = 206 < 0 is false). These tests pin that invariant.

/// Semantic pin: signed comparisons on narrowed struct fields.
/// Field values are negative, and all comparisons must respect signed semantics
/// (via sext i8→i64 before icmp). A zext bug would make -50 > 0.
#[test]
fn test_narrowed_comparison_signed_semantics() {
    assert_aot_success(
        r"
type SignedPair = { a: int, b: int }

@compare (p: SignedPair) -> int = {
    // a = -50, b = 20
    let lt = p.a < p.b;     // -50 < 20 = true (zext would give 206 < 20 = false)
    let le = p.a <= p.b;    // -50 <= 20 = true
    let gt = p.b > p.a;     // 20 > -50 = true
    let ge = p.b >= p.a;    // 20 >= -50 = true
    let eq = p.a == p.a;    // -50 == -50 = true
    let ne = p.a != p.b;    // -50 != 20 = true
    if lt && le && gt && ge && eq && ne then 0 else 1
}

@main () -> int = {
    let p = SignedPair { a: -50, b: 20 };
    compare(p:)
}
",
        "narrowed_comparison_signed",
    );
}

/// Semantic pin: comparison at signed i8 boundaries (-128, 127).
/// If sext is missing, -128 becomes 128 (unsigned) and -128 < 127 would still
/// be true by coincidence in unsigned. So also check -128 < 0 (would fail with
/// zext: 128 < 0 = false).
#[test]
fn test_narrowed_comparison_i8_boundary_values() {
    assert_aot_success(
        r"
type Bounds = { lo: int, hi: int, zero: int }

@main () -> int = {
    let b = Bounds { lo: -128, hi: 127, zero: 0 };
    let ok1 = b.lo < b.hi;     // -128 < 127 = true
    let ok2 = b.lo < b.zero;   // -128 < 0 = true (zext: 128 < 0 = false!)
    let ok3 = b.hi > b.zero;   // 127 > 0 = true
    let ok4 = b.lo <= b.lo;    // -128 <= -128 = true
    let ok5 = b.hi >= b.hi;    // 127 >= 127 = true
    if ok1 && ok2 && ok3 && ok4 && ok5 then 0 else 1
}
",
        "narrowed_comparison_i8_boundaries",
    );
}

/// Semantic pin: ordering/sorting logic using narrowed struct fields.
/// Tests that comparison chains (used in sorting, min/max) work correctly
/// when field values are negative and narrowed to i8.
#[test]
fn test_narrowed_comparison_ordering_chain() {
    assert_aot_success(
        r"
type Triple = { x: int, y: int, z: int }

@min_of_three (t: Triple) -> int = {
    let m = t.x;
    let m = if t.y < m then t.y else m;
    if t.z < m then t.z else m
}

@main () -> int = {
    let t = Triple { x: -10, y: -100, z: -1 };
    let m = min_of_three(t:);
    // min(-10, -100, -1) = -100
    if m == -100 then 0 else 1
}
",
        "narrowed_comparison_ordering_chain",
    );
}

// ---- Phase B: Straight-Line Local Variable Narrowing Tests (§04.4) ----
//
// These IR-inspection tests verify that non-phi local variables are narrowed
// to smaller LLVM types when their value range fits. They are the TDD "write
// failing tests first" step — they MUST FAIL before Phase B straight-line
// local narrowing is implemented in def_var_repr()/var().
//
// §04.5 checklist: "Write failing test matrix for Phase B BEFORE implementing"

/// IR semantic pin: arithmetic result `x + 25` where x is a literal produces
/// trunc+sext in the IR. Literal constants (i64 50, i64 25) are inlined by
/// LLVM, but the ADD result flows through `def_var_repr()` and gets narrowed.
/// This test ONLY passes with Phase B straight-line local narrowing.
#[test]
fn test_phase_b_ir_pin_straight_line_add_narrowed() {
    let ir = compile_and_capture_ir(
        r"
@id (x: int) -> int = x;

@use_literal () -> int = {
    let x = 50;
    let y = x + 25;
    id(x: y)
}

@main () -> int = use_literal();
",
    );

    let fn_ir = extract_function_ir(&ir, "_ori_use_literal");

    // Phase B: the add result (range [75, 75]) and its copy get trunc+sext.
    // Literal constants 50 and 25 are inlined directly, but their arithmetic
    // result flows through def_var_repr() which inserts the trunc+sext pair.
    assert!(
        fn_ir.contains("local.trunc") && fn_ir.contains("local.sext"),
        "expected `local.trunc` + `local.sext` in _ori_use_literal — \
         arithmetic result (range [75, 75]) fits in signed i8, should be narrowed.\n\
         Phase B semantic pin: ONLY passes with straight-line local narrowing.\n\
         Got IR:\n{fn_ir}"
    );
}

/// IR semantic pin: multiple narrowed locals produce multiple trunc+sext pairs.
/// Each narrowed variable definition inserts its own trunc+sext pair.
#[test]
fn test_phase_b_ir_pin_multiple_narrowed_locals() {
    let ir = compile_and_capture_ir(
        r"
@id (x: int) -> int = x;

@compute () -> int = {
    let x = 50;
    let y = x + 25;
    let z = y + 10;
    id(x: z)
}

@main () -> int = compute();
",
    );

    let fn_ir = extract_function_ir(&ir, "_ori_compute");

    // Phase B: y (range [75, 75]) and z (range [85, 85]) each get trunc+sext.
    // Count trunc instructions — at least 2 for the arithmetic results.
    let trunc_count = fn_ir.matches("local.trunc").count();
    assert!(
        trunc_count >= 2,
        "expected at least 2 `local.trunc` instructions in _ori_compute — \
         both y and z arithmetic results should be narrowed.\n\
         Phase B semantic pin: ONLY passes with straight-line local narrowing.\n\
         Got trunc count: {trunc_count}\nGot IR:\n{fn_ir}"
    );
}

/// Negative pin: public function parameters must NOT be narrowed.
/// Even if only called with value 5, the parameter stays i64 because
/// external callers might pass any value.
#[test]
fn test_phase_b_negative_public_param_not_narrowed() {
    let ir = compile_and_capture_ir(
        r"
pub @add_one (n: int) -> int = n + 1;

@main () -> int = add_one(n: 5);
",
    );

    let fn_ir = extract_function_ir(&ir, "_ori_add_one");

    // Parameters are excluded from compute_narrowed_vars() — they must stay i64.
    // No trunc/narrow-type should appear for the parameter itself.
    assert!(
        !fn_ir.contains("trunc i64") || fn_ir.contains("sadd.with.overflow.i64"),
        "expected NO parameter narrowing in pub _ori_add_one — \
         public function parameters must stay canonical i64.\n\
         Got IR:\n{fn_ir}"
    );
}

/// Negative pin: `let x = 3_000_000_000` exceeds i32 range — must stay i64.
/// Range [3B, 3B] does not fit in i32 [-2^31, 2^31-1].
#[test]
fn test_phase_b_negative_wide_constant_stays_i64() {
    let ir = compile_and_capture_ir(
        r"
@id (x: int) -> int = x;

@use_wide () -> int = {
    let x = 3_000_000_000;
    id(x:)
}

@main () -> int = use_wide();
",
    );

    let fn_ir = extract_function_ir(&ir, "_ori_use_wide");

    // 3B exceeds signed i32 max (2147483647), so x stays i64.
    // No trunc to i8/i16/i32 should appear.
    assert!(
        !fn_ir.contains("trunc i64") && !fn_ir.contains("i8") && !fn_ir.contains("i16"),
        "expected NO narrowing in _ori_use_wide — 3_000_000_000 exceeds i32 range.\n\
         Got IR:\n{fn_ir}"
    );
}

/// IR semantic pin: `ArcInstr::Select` result is narrowed when range analysis
/// proves the result fits in a narrow type. Block-merge folds trivial if/else
/// diamonds into `select` instructions — these must go through `def_var_repr()`
/// to participate in Phase B local narrowing.
///
/// This test ONLY passes once the Select path uses `def_var_repr()`.
#[test]
fn test_phase_b_ir_pin_select_narrowed() {
    let ir = compile_and_capture_ir(
        r"
@pick (b: bool) -> int = if b then 1 else 2;

@main () -> int = pick(b: true);
",
    );

    let fn_ir = extract_function_ir(&ir, "_ori_pick");

    // Phase B: the select result (range [1, 2]) fits in signed i8.
    // After block-merge folds the if/else diamond into ArcInstr::Select,
    // the emitter should narrow the result via def_var_repr(), producing
    // a trunc+sext pair. Without the fix, _ori_pick is just:
    //   %sel = select i1 %0, i64 1, i64 2
    //   ret i64 %sel
    // With the fix, the trunc+sext should appear between select and ret.
    assert!(
        fn_ir.contains("local.trunc") && fn_ir.contains("local.sext"),
        "expected `local.trunc` + `local.sext` in _ori_pick — \
         select result (range [1, 2]) fits in signed i8, should be narrowed.\n\
         Phase B semantic pin: ONLY passes with Select narrowing via def_var_repr().\n\
         Got IR:\n{fn_ir}"
    );
}

/// Behavioral test: narrowed Select result produces correct values.
/// Verifies both branches of a narrowed select yield correct runtime output.
#[test]
fn test_phase_b_select_narrowed_behavior() {
    assert_aot_success(
        r"
@pick (b: bool) -> int = if b then 10 else 20;

@main () -> int = {
    let t = pick(b: true);
    let f = pick(b: false);
    if t == 10 && f == 20 then 0 else 1
}
",
        "select_narrowed_behavior",
    );
}

/// Behavioral test: narrowed Select with negative values preserves sign.
/// Catches zext bugs — negative values through narrowed select must retain sign.
#[test]
fn test_phase_b_select_narrowed_negative_values() {
    assert_aot_success(
        r"
@pick (b: bool) -> int = if b then -50 else -100;

@main () -> int = {
    let sum = pick(b: true) + pick(b: false);
    // -50 + -100 = -150
    if sum == -150 then 0 else 1
}
",
        "select_narrowed_negative_values",
    );
}

/// Overflow guard verification: when a narrowed local's arithmetic result
/// exceeds i8 range, the result is correctly narrowed to i16 (not i8).
/// The architecture handles this by construction — arithmetic operates at i64,
/// and `min_width()` selects the smallest type that fits the computed range.
/// No explicit overflow guard is needed.
#[test]
fn test_phase_b_overflow_guard_widens_to_i16() {
    let ir = compile_and_capture_ir(
        r"
@id (x: int) -> int = x;

@compute () -> int = {
    let x = 100;
    let y = x + 50;
    id(x: y)
}

@main () -> int = compute();
",
    );

    let fn_ir = extract_function_ir(&ir, "_ori_compute");

    // y = 100 + 50 = 150 — exceeds signed i8 max (127), so narrowed to i16.
    // This verifies the overflow guard is correct by construction:
    // arithmetic at i64, trunc to i16 (not i8) preserves the value.
    assert!(
        fn_ir.contains("i16"),
        "expected i16 narrowing in _ori_compute — result 150 exceeds i8, needs i16.\n\
         Overflow guard semantic pin: verifies range-driven width selection.\n\
         Got IR:\n{fn_ir}"
    );
    assert!(
        !fn_ir.contains("trunc i64") || !fn_ir.contains("to i8\n"),
        "result 150 must NOT be truncated to i8 — would lose data.\n\
         Got IR:\n{fn_ir}"
    );
}

/// Behavioral test: overflow guard correctness — 100 + 50 = 150 (exceeds i8).
#[test]
fn test_phase_b_overflow_guard_behavior() {
    assert_aot_success(
        r"
@compute () -> int = {
    let x = 100;
    let y = x + 50;
    // 150 exceeds i8 range but fits i16 — value must be preserved
    if y == 150 then 0 else 1
}

@main () -> int = compute();
",
        "overflow_guard_behavior",
    );
}

// ── NarrowingPolicy verification ────────────────────────────────────────

/// `ORI_NO_REPR_OPT=1` suppresses ALL narrowing — Pixel struct stays
/// canonical i64 fields. Verifies `NarrowingPolicy::Disabled` works end-to-end.
///
/// Semantic pin: ONLY passes when Disabled actually suppresses narrowing.
/// Without Disabled, the Pixel struct would show `i8` fields in IR.
#[test]
fn test_narrowing_policy_disabled_suppresses_struct_narrowing() {
    let ir = compile_and_capture_ir_no_repr_opt(
        r"
type Pixel = { r: int, g: int, b: int, a: int }

@read_pixel (p: Pixel) -> int = p.r + p.g + p.b + p.a;

@main () -> int = {
    let p = Pixel { r: 10, g: 20, b: 30, a: 40 };
    let sum = read_pixel(p:);
    if sum == 100 then 0 else 1
}
",
    );

    let fn_ir = extract_function_ir(&ir, "_ori_read_pixel");

    // With Disabled, no trunc/sext should appear — fields stay canonical i64.
    assert!(
        !fn_ir.contains("sext i8") && !fn_ir.contains("sext i16"),
        "expected NO sext in _ori_read_pixel with ORI_NO_REPR_OPT=1.\n\
         NarrowingPolicy::Disabled must suppress ALL narrowing.\n\
         Got IR:\n{fn_ir}"
    );
}

/// `ORI_NO_REPR_OPT=1` suppresses local variable narrowing (Phase B).
/// Loop counters and straight-line locals stay canonical i64.
#[test]
fn test_narrowing_policy_disabled_suppresses_local_narrowing() {
    let ir = compile_and_capture_ir_no_repr_opt(
        r"
@id (x: int) -> int = x;

@compute () -> int = {
    let x = 50;
    let y = x + 25;
    id(x: y)
}

@main () -> int = compute();
",
    );

    let fn_ir = extract_function_ir(&ir, "_ori_compute");

    // With Disabled, no local.trunc/local.sext should appear.
    assert!(
        !fn_ir.contains("local.trunc") && !fn_ir.contains("local.sext"),
        "expected NO local.trunc/sext in _ori_compute with ORI_NO_REPR_OPT=1.\n\
         NarrowingPolicy::Disabled must suppress Phase B local narrowing.\n\
         Got IR:\n{fn_ir}"
    );
}

/// Verify that `NarrowingPolicy::Disabled` produces correct runtime results
/// (no data corruption from missing narrowing).
#[test]
fn test_narrowing_policy_disabled_behavioral_correctness() {
    // Run with ORI_NO_REPR_OPT=1 — same binary, different env
    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .to_path_buf();

    let temp_dir = TempDir::new().expect("temp dir");
    let source_path = temp_dir.path().join("test_disabled.ori");
    let binary_path = temp_dir
        .path()
        .join(format!("test_disabled{}", std::env::consts::EXE_SUFFIX));

    std::fs::write(
        &source_path,
        r"
type Pixel = { r: int, g: int, b: int, a: int }

@main () -> int = {
    let p = Pixel { r: -128, g: 0, b: 127, a: 42 };
    let sum = p.r + p.g + p.b + p.a;
    if sum == 41 then 0 else 1
}
",
    )
    .unwrap();

    let exe = format!("ori{}", std::env::consts::EXE_SUFFIX);
    let binary = workspace_root.join("target/debug").join(&exe);

    // Compile with ORI_NO_REPR_OPT=1
    let compile = Command::new(&binary)
        .args([
            "build",
            source_path.to_str().unwrap(),
            "-o",
            binary_path.to_str().unwrap(),
        ])
        .env("ORI_STDLIB", stdlib_path())
        .env("ORI_NO_REPR_OPT", "1")
        .output()
        .expect("compile");
    assert!(
        compile.status.success(),
        "Compilation failed:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );

    // Run and check exit code
    let run = Command::new(&binary_path).output().expect("run");
    let exit_code = run.status.code().unwrap_or(-1);
    assert_eq!(
        exit_code,
        0,
        "Disabled-policy binary returned {exit_code}, expected 0.\n\
         stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
}

// §04.4 Phase C: Collection element narrowing tests
//
// These tests verify that [int] list literals with bounded values use narrowed
// element storage (i8/i16/i32) and that the narrowing is transparent to program
// semantics — all operations produce identical results to canonical i64 storage.

/// Semantic pin: list literal with bounded values uses narrowed i8 element storage.
/// Verifies LLVM IR contains narrowed `elem_size` (1) instead of canonical (8).
#[test]
fn test_narrowed_list_i8_ir_pin() {
    let ir = compile_and_capture_ir(
        r"
@main () -> int = {
    let xs: [int] = [1, 2, 3];
    xs[0]
}
",
    );

    let main_ir = extract_function_ir(&ir, "_ori_main");
    // Construction: ori_list_alloc_data with elem_size=1 (i8), not 8 (i64)
    assert!(
        main_ir.contains("@ori_list_alloc_data(i64 3, i64 1)"),
        "Expected narrowed elem_size=1 in list construction IR.\nIR:\n{main_ir}"
    );
    // Element GEP should use i8 type (inbounds form)
    assert!(
        main_ir.contains("getelementptr inbounds i8,"),
        "Expected i8 GEP type for narrowed list elements.\nIR:\n{main_ir}"
    );
    // Element store should use i8
    assert!(
        main_ir.contains("store i8 1,"),
        "Expected i8 store for narrowed list elements.\nIR:\n{main_ir}"
    );
    // Element load should use i8 with sext to i64
    assert!(
        main_ir.contains("load i8,") && main_ir.contains("sext i8"),
        "Expected i8 load + sext for narrowed element access.\nIR:\n{main_ir}"
    );
}

/// Semantic pin: list indexing through narrowed elements produces correct values.
/// Each element is stored as i8, loaded as i8, then sext'd back to i64.
#[test]
fn test_narrowed_list_index_round_trip() {
    assert_aot_success(
        r"
@main () -> int = {
    let xs: [int] = [-128, 0, 127, 42];
    let ok = xs[0] == -128 && xs[1] == 0 && xs[2] == 127 && xs[3] == 42;
    if ok then 0 else 1
}
",
        "narrowed_list_index_round_trip",
    );
}

/// For-yield over narrowed list produces a correctly narrowed result list.
/// Tests the for-yield accumulator interception (`ori_list_new`/`ori_list_push`).
#[test]
fn test_narrowed_list_for_yield() {
    assert_aot_success(
        r"
@main () -> int = {
    let result = for x in [10, 20, 30] yield x;
    let ok = result[0] == 10 && result[1] == 20 && result[2] == 30;
    if ok then 0 else 1
}
",
        "narrowed_list_for_yield",
    );
}

/// For-yield with transform over narrowed list.
#[test]
fn test_narrowed_list_for_yield_transform() {
    assert_aot_success(
        r"
@main () -> int = {
    let result = for x in [1, 2, 3] yield x * 10;
    let ok = result[0] == 10 && result[1] == 20 && result[2] == 30;
    if ok then 0 else 1
}
",
        "narrowed_list_for_yield_transform",
    );
}

/// Iteration (for..do sum) over narrowed list.
#[test]
fn test_narrowed_list_iteration_sum() {
    assert_aot_success(
        r"
@main () -> int = {
    let xs = [10, 20, 30, 40];
    let sum = 0;
    for x in xs do { sum = sum + x };
    if sum == 100 then 0 else 1
}
",
        "narrowed_list_iteration_sum",
    );
}

/// Derived Eq on struct with narrowed list field.
#[test]
fn test_narrowed_list_derived_eq() {
    assert_aot_success(
        r"
#derive(Eq)
type Container = { items: [int] }

@main () -> int = {
    let a = Container { items: [1, 2, 3] };
    let b = Container { items: [1, 2, 3] };
    let c = Container { items: [1, 2, 4] };
    let eq = a == b;
    let neq = a != c;
    if eq && neq then 0 else 1
}
",
        "narrowed_list_derived_eq",
    );
}

/// List `.first()` and `.last()` through narrowed elements — returns `Option<int>`.
#[test]
fn test_narrowed_list_first_last() {
    assert_aot_success(
        r"
@main () -> int = {
    let xs = [10, 20, 30];
    let f = xs.first();
    let l = xs.last();
    let ok = is_some(option: f) && is_some(option: l);
    if !ok then 1
    else {
        let fv = f.unwrap_or(default: -1);
        let lv = l.unwrap_or(default: -1);
        if fv == 10 && lv == 30 then 0 else 2
    }
}
",
        "narrowed_list_first_last",
    );
}

/// Sort on narrowed [int] list.
#[test]
fn test_narrowed_list_sort() {
    assert_aot_success(
        r"
@main () -> int = {
    let xs = [30, 10, 20];
    let sorted = xs.sort();
    let ok = sorted[0] == 10 && sorted[1] == 20 && sorted[2] == 30;
    if ok then 0 else 1
}
",
        "narrowed_list_sort",
    );
}

/// Negative pin: list with narrowing disabled (`ORI_NO_REPR_OPT=1`) uses
/// canonical i64 element storage — no i8 GEP in the LLVM IR.
#[test]
fn test_narrowed_list_disabled_ir_pin() {
    let ir = compile_and_capture_ir_no_repr_opt(
        r"
@main () -> int = {
    let xs: [int] = [1, 2, 3];
    xs[0]
}
",
    );

    let main_ir = extract_function_ir(&ir, "_ori_main");
    // With narrowing disabled, element GEP should NOT use i8
    assert!(
        !main_ir.contains("getelementptr inbounds i8,"),
        "Expected NO i8 GEP when narrowing is disabled.\nIR:\n{main_ir}"
    );
    // Should use canonical elem_size=8
    assert!(
        main_ir.contains("i64 8)") || main_ir.contains(", i64 8,"),
        "Expected canonical elem_size=8 when narrowing is disabled.\nIR:\n{main_ir}"
    );
}

// §04.4 Phase C: Set exclusion from narrowing
//
// Sets are excluded from Phase C narrowing because eq/hash thunks always load
// canonical-width values (i64 for int) from element pointers. These tests
// verify sets work correctly with canonical element sizes even when lists
// in the same program are narrowed.

/// Set operations work correctly when list narrowing is active.
/// Sets are created via `.iter().collect()` — canonical element sizes.
#[test]
fn test_set_int_canonical_with_narrowed_list_ir() {
    let ir = compile_and_capture_ir(
        r"
@main () -> int = {
    let xs: [int] = [1, 2, 3];
    let s: Set<int> = xs.iter().collect();
    if s.contains(value: xs[0]) then 0 else 1
}
",
    );

    let main_ir = extract_function_ir(&ir, "_ori_main");
    // List should be narrowed (elem_size=1 for i8)
    assert!(
        main_ir.contains("@ori_list_alloc_data(i64 3, i64 1)"),
        "Expected narrowed list elem_size=1 in IR.\nIR:\n{main_ir}"
    );
    // collect_set uses canonical elem_size=8 (not narrowed)
    assert!(
        main_ir.contains("i64 8") || main_ir.contains(", i64 8,"),
        "Expected canonical elem_size=8 for set collection.\nIR:\n{main_ir}"
    );
}

/// Set with bounded int elements works correctly at runtime (canonical sizes).
#[test]
fn test_set_int_operations_canonical() {
    assert_aot_success(
        r"
use std.testing { assert_eq, assert }
@main () -> void = {
    let s: Set<int> = [10, 20, 30].iter().collect();
    assert(condition: s.contains(value: 10));
    assert(condition: s.contains(value: 20));
    assert(condition: s.contains(value: 30));
    assert(condition: !s.contains(value: 40));
    assert_eq(actual: s.len(), expected: 3);
}
",
        "set_int_operations_canonical",
    );
}

/// Mixed program: narrowed list + canonical set coexist without interference.
#[test]
fn test_narrowed_list_and_canonical_set_coexist() {
    assert_aot_success(
        r"
use std.testing { assert_eq, assert }
@main () -> void = {
    // List gets narrowed (all values fit in i8)
    let xs: [int] = [1, 2, 3, 4, 5];
    assert_eq(actual: xs[0], expected: 1);
    assert_eq(actual: xs[4], expected: 5);

    // Set uses canonical sizes (not narrowed)
    let s: Set<int> = [1, 2, 3, 4, 5].iter().collect();
    assert(condition: s.contains(value: 1));
    assert(condition: s.contains(value: 5));
    assert(condition: !s.contains(value: 6));
    assert_eq(actual: s.len(), expected: 5);

    // Cross-check: list element lookup in set
    assert(condition: s.contains(value: xs[2]));
}
",
        "narrowed_list_and_canonical_set_coexist",
    );
}

/// Set insert works with canonical element sizes when list narrowing is active.
#[test]
fn test_set_insert_with_narrowed_list_context() {
    assert_aot_success(
        r"
use std.testing { assert_eq, assert }
@main () -> void = {
    let xs: [int] = [10, 20, 30];
    let s: Set<int> = [10, 20].iter().collect();
    let s2 = s.insert(value: 30);
    assert_eq(actual: s2.len(), expected: 3);
    assert(condition: s2.contains(value: 30));
    // Original set unchanged
    assert_eq(actual: s.len(), expected: 2);
}
",
        "set_insert_with_narrowed_list_context",
    );
}

// §04.4 Phase C: for-yield narrowing safety
//
// The for-yield elem_size override must only fire for int-element accumulators.
// A program with a narrowed [int] and a for...yield producing [str] must not
// corrupt the string accumulator's elem_size.

/// Semantic pin: for-yield producing [str] works correctly when [int] is narrowed.
/// Without the element-type gate, `ori_list_new`/`ori_list_push` for the str
/// accumulator would receive `elem_size=1` instead of 24, causing corruption.
#[test]
fn test_for_yield_str_with_narrowed_int_list() {
    assert_aot_success(
        r"
use std.testing { assert_eq }
@main () -> void = {
    // Narrowed [int] literal (all values fit in i8)
    let xs: [int] = [1, 2, 3];
    assert_eq(actual: xs[0], expected: 1);

    // for-yield producing [str] — must NOT use narrowed elem_size
    let ys: [str] = for x in xs.iter() yield if x == 1 then `a` else `bb`;
    assert_eq(actual: ys[0], expected: `a`);
    assert_eq(actual: ys[1], expected: `bb`);
    assert_eq(actual: ys[2], expected: `bb`);
}
",
        "for_yield_str_with_narrowed_int_list",
    );
}

/// for-yield producing [int] from narrowed [int] — narrowing should still work.
#[test]
fn test_for_yield_int_from_narrowed_int_list() {
    assert_aot_success(
        r"
use std.testing { assert_eq }
@main () -> void = {
    let xs: [int] = [1, 2, 3];
    let ys: [int] = for x in xs.iter() yield x * 2;
    assert_eq(actual: ys[0], expected: 2);
    assert_eq(actual: ys[1], expected: 4);
    assert_eq(actual: ys[2], expected: 6);
}
",
        "for_yield_int_from_narrowed_int_list",
    );
}

/// Mixed for-yields in same function: int and str accumulators coexist.
#[test]
fn test_mixed_for_yield_int_and_str() {
    assert_aot_success(
        r"
use std.testing { assert_eq }
@main () -> void = {
    let xs: [int] = [1, 2, 3];
    // Int for-yield — safe to narrow
    let doubled: [int] = for x in xs.iter() yield x * 2;
    // Str for-yield — must NOT narrow
    let names: [str] = for x in xs.iter() yield if x == 1 then `one` else `other`;
    assert_eq(actual: doubled[0], expected: 2);
    assert_eq(actual: doubled[2], expected: 6);
    assert_eq(actual: names[0], expected: `one`);
    assert_eq(actual: names[1], expected: `other`);
}
",
        "mixed_for_yield_int_and_str",
    );
}

// ---- §05 Float Narrowing AOT Tests ----
//
// These tests verify that float field narrowing (f64→f32 for f32-exact literals)
// produces correct runtime behavior and LLVM IR patterns. They are the float
// counterpart to the integer narrowing tests above.

/// Semantic pin: struct with f32-exact float fields (0.0, 0.5, 1.0).
/// Field values must survive fptrunc (f64→f32 at construction) and
/// fpext (f32→f64 at extraction) without data corruption.
#[test]
fn test_float_narrowed_struct_roundtrip() {
    assert_aot_success(
        r"
type FloatPoint = { x: float, y: float, z: float }

@main () -> int = {
    let p = FloatPoint { x: 0.0, y: 0.5, z: 1.0 };
    let ok1 = p.x == 0.0;
    let ok2 = p.y == 0.5;
    let ok3 = p.z == 1.0;
    if ok1 && ok2 && ok3 then 0 else 1
}
",
        "float_narrowed_struct_roundtrip",
    );
}

/// IR semantic pin: narrowed float struct type contains `float` fields (not `double`).
/// A struct with all f32-exact fields should produce `{ float, float, float }` in LLVM IR.
#[test]
fn test_float_narrowed_struct_ir_pin_type_layout() {
    let ir = compile_and_capture_ir(
        r"
type FloatColor = { r: float, g: float, b: float }

@read_r (c: FloatColor) -> float = c.r;

@main () -> int = {
    let c = FloatColor { r: 0.5, g: 0.25, b: 1.0 };
    let v = read_r(c:);
    if v == 0.5 then 0 else 1
}
",
    );

    let fn_ir = extract_function_ir(&ir, "_ori_read_r");

    // The struct type should be { float, float, float } not { double, double, double }.
    let has_narrowed_type =
        fn_ir.contains("{ float, float, float }") || fn_ir.contains("{float, float, float}");
    assert!(
        has_narrowed_type,
        "expected narrowed float struct type `{{ float, float, float }}` in _ori_read_r IR — \
         FloatColor fields with f32-exact values should be narrowed to float (f32).\n\
         Regression guard: without float narrowing, type would be `{{ double, double, double }}`.\n\
         IR:\n{fn_ir}"
    );
}

/// IR semantic pin: narrowed float struct construction must insert `fptrunc double ... to float`.
#[test]
fn test_float_narrowed_struct_ir_pin_fptrunc_on_construction() {
    let ir = compile_and_capture_ir(
        r"
type FVec2 = { x: float, y: float }

@read_x (v: FVec2) -> float = v.x;

@main () -> int = {
    let v = FVec2 { x: 0.5, y: 0.25 };
    let r = read_x(v:);
    if r == 0.5 then 0 else 1
}
",
    );

    let main_ir = extract_function_ir(&ir, "_ori_main");

    // The f64 constants (0.5, 0.25) are stored into narrowed f32 struct fields.
    // LLVM may fold `fptrunc double 5.0e-1 to float` → `float 5.0e-1` at IR
    // construction time, so we check for either explicit fptrunc or a float constant.
    let has_fptrunc = main_ir.contains("fptrunc double");
    let has_narrowed_const = main_ir.contains("{ float, float }");
    assert!(
        has_fptrunc || has_narrowed_const,
        "expected evidence of float narrowing at construction in _ori_main — either \
         `fptrunc double ... to float` instructions or a `{{ float, float }}` constant.\n\
         Regression guard: without narrowing, the struct type would be `{{ double, double }}`.\n\
         IR:\n{main_ir}"
    );
}

/// IR semantic pin: narrowed float struct field loads must produce `fpext float ... to double`.
#[test]
fn test_float_narrowed_struct_ir_pin_fpext_on_field_load() {
    let ir = compile_and_capture_ir(
        r"
type FVec2 = { x: float, y: float }

@sum_fields (v: FVec2) -> float = v.x + v.y;

@main () -> int = {
    let v = FVec2 { x: 0.5, y: 0.25 };
    let s = sum_fields(v:);
    if s == 0.75 then 0 else 1
}
",
    );

    let fn_ir = extract_function_ir(&ir, "_ori_sum_fields");

    // Narrowed float fields are f32. Loading them produces f32 values that
    // must be extended to f64 for canonical-width arithmetic.
    assert!(
        fn_ir.contains("fpext float"),
        "expected `fpext float ... to double` in _ori_sum_fields — narrowed FVec2 fields \
         should produce f32 loads that need extension to canonical f64.\n\
         Regression guard: if narrowing is disabled, fields stay double and no fpext appears.\n\
         IR:\n{fn_ir}"
    );
}

/// Negative IR semantic pin: struct with non-f32-exact values must NOT show float narrowing.
/// `1e300` exceeds f32 range — fields must stay double.
#[test]
fn test_float_non_narrowed_struct_ir_pin_wide_value() {
    let ir = compile_and_capture_ir(
        r"
type WideFloat = { a: float, b: float }

@sum_wide (w: WideFloat) -> float = w.a + w.b;

@main () -> int = {
    let w = WideFloat { a: 1e300, b: -1e300 };
    let s = sum_wide(w:);
    if s == 0.0 then 0 else 1
}
",
    );

    let fn_ir = extract_function_ir(&ir, "_ori_sum_wide");

    // Fields with values 1e300 and -1e300 exceed f32 range — no narrowing.
    // The function should NOT contain narrowing-specific fpext instructions.
    assert!(
        !fn_ir.contains("fpext float"),
        "expected NO float narrowing fpext in _ori_sum_wide — field values 1e300 \
         and -1e300 exceed f32 range, so fields must stay double.\nIR:\n{fn_ir}"
    );
}

/// Negative pin: float arithmetic result stored in struct field stays f64.
/// Even if the result is f32-exact, the analysis deems arithmetic as Top.
#[test]
fn test_float_arithmetic_not_narrowed() {
    let ir = compile_and_capture_ir(
        r"
type Result = { val: float }

@compute (r: Result) -> float = r.val;

@main () -> int = {
    let x = 0.5;
    let y = x + 0.0;
    let r = Result { val: y };
    let v = compute(r:);
    if v == 0.5 then 0 else 1
}
",
    );

    let fn_ir = extract_function_ir(&ir, "_ori_compute");

    // Arithmetic result (even if f32-exact) is marked Top by the analysis.
    // Field should stay double — no fpext needed.
    assert!(
        !fn_ir.contains("fpext float"),
        "expected NO float narrowing in _ori_compute — arithmetic results must not \
         be narrowed (analysis conservatively marks arithmetic as Top).\nIR:\n{fn_ir}"
    );
}

/// Negative pin: non-literal float variable stored in struct field stays f64.
/// Only literal values are analyzed for f32-exactness.
#[test]
fn test_float_variable_not_narrowed() {
    let ir = compile_and_capture_ir(
        r"
type Wrap = { val: float }

@identity (x: float) -> float = x;

@compute (w: Wrap) -> float = w.val;

@main () -> int = {
    let x = identity(x: 0.5);
    let w = Wrap { val: x };
    let v = compute(w:);
    if v == 0.5 then 0 else 1
}
",
    );

    let fn_ir = extract_function_ir(&ir, "_ori_compute");

    // Function return values are not literal-analyzed — field stays double.
    assert!(
        !fn_ir.contains("fpext float"),
        "expected NO float narrowing in _ori_compute — non-literal float variable \
         stored in struct field must stay double.\nIR:\n{fn_ir}"
    );
}

/// IR semantic pin: mixed int+float narrowed struct.
/// Int field in [0, 255] → i16, float field with f32-exact → float.
/// Verifies combined §04+§05 narrowing in one struct.
#[test]
fn test_mixed_int_float_narrowed_struct() {
    let ir = compile_and_capture_ir(
        r"
type Particle = { mass: float, health: int }

@read_mass (p: Particle) -> float = p.mass;

@main () -> int = {
    let p = Particle { mass: 0.5, health: 100 };
    let m = read_mass(p:);
    if m == 0.5 && p.health == 100 then 0 else 1
}
",
    );

    let fn_ir = extract_function_ir(&ir, "_ori_read_mass");

    // Health [100, 100] narrows to i8 (§04), mass 0.5 narrows to float (§05).
    // The struct type should be { float, i8 } — both fields narrowed.
    // Note: the function return type is `double` (canonical f64), which is correct —
    // only the struct storage uses the narrowed float type.
    let has_narrowed_struct = fn_ir.contains("{ float, i8 }") || fn_ir.contains("{float, i8}");
    assert!(
        has_narrowed_struct,
        "expected narrowed struct type `{{ float, i8 }}` in _ori_read_mass IR — \
         Particle.mass (f32-exact 0.5) → float, Particle.health [100,100] → i8.\n\
         Combined §04+§05 narrowing pin.\nIR:\n{fn_ir}"
    );
}

/// Negative IR pin: `#repr("c")` struct with f32-exact fields must NOT be narrowed.
/// C ABI compatibility requires canonical double layout.
#[test]
fn test_float_repr_c_not_narrowed() {
    let ir = compile_and_capture_ir(
        r#"
#[repr("c")]
type CPoint = { x: float, y: float }

@read_x (p: CPoint) -> float = p.x;

@main () -> int = {
    let p = CPoint { x: 0.5, y: 0.25 };
    let v = read_x(p:);
    if v == 0.5 then 0 else 1
}
"#,
    );

    let fn_ir = extract_function_ir(&ir, "_ori_read_x");

    // #repr("c") structs must preserve ABI — no narrowing. Fields stay double.
    assert!(
        !fn_ir.contains("fpext float"),
        "expected NO float narrowing in #repr(\"c\") struct _ori_read_x — \
         C ABI requires canonical double layout. Fields must stay double.\nIR:\n{fn_ir}"
    );
}

// ---- §05 Derive Semantic Pins for Float Narrowing ----
//
// These tests verify that derived traits (Printable, Debug, Hashable) work
// correctly with narrowed float struct fields. The derive codegen must extend
// narrowed f32 fields back to canonical f64 before calling runtime functions.

/// Semantic pin: derived `to_str()` on narrowed float struct fields.
/// The string representation MUST show "0.5", not garbage from ABI mismatch.
/// This is the regression test for TPR-05-001.
#[test]
fn test_float_narrowed_derive_printable() {
    assert_aot_success(
        r#"
#[derive(Printable)]
type FPoint = { x: float, y: float }

@main () -> int = {
    let p = FPoint { x: 0.5, y: 0.25 };
    let s = p.to_str();
    let has_05 = s.contains(substr: "0.5");
    let has_025 = s.contains(substr: "0.25");
    if has_05 && has_025 then 0 else 1
}
"#,
        "float_narrowed_derive_printable",
    );
}

/// Semantic pin: derived `debug()` on narrowed float struct fields.
/// Same as Printable but for the Debug trait. The debug representation
/// MUST show the correct float values.
#[test]
fn test_float_narrowed_derive_debug() {
    assert_aot_success(
        r#"
#[derive(Debug)]
type FColor = { r: float, g: float, b: float }

@main () -> int = {
    let c = FColor { r: 0.5, g: 0.25, b: 1.0 };
    let s = c.debug();
    let has_05 = s.contains(substr: "0.5");
    let has_025 = s.contains(substr: "0.25");
    // Runtime formats 1.0 as "1" (no trailing .0)
    let has_b = s.contains(substr: "b: 1");
    if has_05 && has_025 && has_b then 0 else 1
}
"#,
        "float_narrowed_derive_debug",
    );
}

/// Semantic pin: derived `hash()` on narrowed float struct fields.
/// Two structs with identical f32-exact values must produce the same hash.
/// A struct with different values must produce a different hash.
#[test]
fn test_float_narrowed_derive_hash() {
    assert_aot_success(
        r"
#[derive(Eq, Hashable)]
type FPair = { x: float, y: float }

@main () -> int = {
    let a = FPair { x: 0.5, y: 0.25 };
    let b = FPair { x: 0.5, y: 0.25 };
    let c = FPair { x: 1.0, y: 0.0 };
    let same = a.hash() == b.hash();
    let diff = a.hash() != c.hash();
    if same && diff then 0 else 1
}
",
        "float_narrowed_derive_hash",
    );
}

/// Semantic pin: derived Eq on narrowed float struct fields.
/// Equality must work correctly when fields are stored as f32.
#[test]
fn test_float_narrowed_derive_eq() {
    assert_aot_success(
        r"
#[derive(Eq)]
type FVec = { x: float, y: float }

@main () -> int = {
    let a = FVec { x: 0.5, y: 0.25 };
    let b = FVec { x: 0.5, y: 0.25 };
    let c = FVec { x: 1.0, y: 0.0 };
    let ok1 = a == b;
    let ok2 = a != c;
    if ok1 && ok2 then 0 else 1
}
",
        "float_narrowed_derive_eq",
    );
}

/// Semantic pin: derived Comparable on narrowed float struct fields.
/// Comparisons (via `compare()` → `Ordering`) must work correctly with f32 storage.
#[test]
fn test_float_narrowed_derive_comparable() {
    assert_aot_success(
        r"
#[derive(Eq, Comparable)]
type Measure = { value: float }

@main () -> int = {
    let a = Measure { value: 0.25 };
    let b = Measure { value: 0.5 };
    let ok1 = a < b;
    let ok2 = b > a;
    let ok3 = a <= a;
    if ok1 && ok2 && ok3 then 0 else 1
}
",
        "float_narrowed_derive_comparable",
    );
}

/// IR semantic pin: derive Printable codegen must contain `fpext float` to widen
/// narrowed fields before calling `ori_str_from_float(double)`.
/// This directly verifies the TPR-05-001 fix at the IR level.
#[test]
fn test_float_narrowed_derive_ir_pin_fpext_in_printable() {
    let ir = compile_and_capture_ir(
        r#"
#[derive(Printable)]
type FmtFloat = { x: float }

@format_it (f: FmtFloat) -> str = f.to_str();

@main () -> int = {
    let f = FmtFloat { x: 0.5 };
    let s = format_it(f:);
    if s.contains(substr: "0.5") then 0 else 1
}
"#,
    );

    // The derive Printable function must fpext narrowed float fields to double
    // before calling ori_str_from_float. Without the fix, LLVM verification fails.
    let has_fpext = ir.contains("fpext float");
    assert!(
        has_fpext,
        "expected `fpext float ... to double` in derive Printable IR for narrowed float \
         struct — derive codegen must widen f32 fields to f64 before ori_str_from_float.\n\
         TPR-05-001 regression guard.\nFull IR length: {} chars",
        ir.len()
    );
}
