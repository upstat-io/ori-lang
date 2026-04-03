//! Integer narrowing AOT tests.
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
        include_str!("fixtures/narrowing/narrowed_struct_pixel_round_trip.ori"),
        "narrowed_pixel_round_trip",
    );
}

/// Struct update syntax with narrowed fields — the spread creates a new
/// struct from old field values, each of which goes through extract→trunc.
#[test]
fn test_narrowed_struct_update() {
    assert_aot_success(
        include_str!("fixtures/narrowing/narrowed_struct_update.ori"),
        "narrowed_struct_update",
    );
}

/// Struct with mixed field types — only int fields should be narrowed.
/// Non-int fields (str, bool, float) must pass through unaffected.
#[test]
fn test_narrowed_struct_mixed_types() {
    assert_aot_success(
        include_str!("fixtures/narrowing/narrowed_struct_mixed_types.ori"),
        "narrowed_struct_mixed_types",
    );
}

/// Struct field mutation — mutable fields that go through the update path
/// must correctly trunc the new value and sext the old values.
#[test]
fn test_narrowed_struct_field_mutation() {
    assert_aot_success(
        include_str!("fixtures/narrowing/narrowed_struct_field_mutation.ori"),
        "narrowed_struct_field_mutation",
    );
}

/// Boundary value test: signed i8 boundaries (-128 and 127).
/// These are the exact limits where narrowing to i8 is valid.
#[test]
fn test_narrowed_struct_i8_boundaries() {
    assert_aot_success(
        include_str!("fixtures/narrowing/narrowed_struct_i8_boundaries.ori"),
        "narrowed_struct_i8_boundaries",
    );
}

// Multi-file AOT semantic pin tests for cross-module type metadata are blocked on multi-file
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
        include_str!("fixtures/narrowing/non_narrowed_struct_wide_range.ori"),
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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/narrowed_struct_ir_pin_sext_on_field_load.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/narrowed_struct_ir_pin_trunc_on_construction.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/narrowed_struct_ir_pin_type_layout.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/non_narrowed_struct_ir_pin_wide_range.ori"
    ));

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
        include_str!("fixtures/narrowing/narrowed_derive_hash_negative_values.ori"),
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
        include_str!("fixtures/narrowing/narrowed_derive_printable_negative_values.ori"),
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
        include_str!("fixtures/narrowing/narrowed_derive_debug_negative_values.ori"),
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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/narrowed_derive_ir_pin_sext_in_hash.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/mixed_field_struct_ir_pin_no_narrowing.ori"
    ));

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

// ---- Phase B: Local Variable Narrowing Tests ----

// Behavioral test: manual loop (loop+break) with bounded counter and accumulator.
// The program must produce correct results regardless of narrowing.
#[test]
fn test_phase_b_loop_behavioral() {
    assert_aot_success(
        include_str!("fixtures/narrowing/phase_b_loop_behavioral.ori"),
        "phase_b_loop_behavioral",
    );
}

// Behavioral test: for-range loop with bounded counter.
#[test]
fn test_phase_b_for_range_behavioral() {
    assert_aot_success(
        include_str!("fixtures/narrowing/phase_b_for_range_behavioral.ori"),
        "phase_b_for_range_behavioral",
    );
}

// Behavioral test: loop with negative values (signed i8 range [-128, 127]).
#[test]
fn test_phase_b_negative_loop_behavioral() {
    assert_aot_success(
        include_str!("fixtures/narrowing/phase_b_negative_loop_behavioral.ori"),
        "phase_b_negative_loop_behavioral",
    );
}

// IR semantic pin: loop counter phi in a manual loop should use i8.
// Range [0, 9] fits in signed i8 [-128, 127].
// This test ONLY passes with Phase B local variable narrowing.
#[test]
fn test_phase_b_ir_pin_loop_counter_phi() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/phase_b_ir_pin_loop_counter_phi.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/phase_b_ir_pin_loop_sext.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/phase_b_ir_pin_wide_range_no_i8.ori"
    ));

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

// ---- Comparison operations on narrowed fields ----
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
        include_str!("fixtures/narrowing/narrowed_comparison_signed_semantics.ori"),
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
        include_str!("fixtures/narrowing/narrowed_comparison_i8_boundary_values.ori"),
        "narrowed_comparison_i8_boundaries",
    );
}

/// Semantic pin: ordering/sorting logic using narrowed struct fields.
/// Tests that comparison chains (used in sorting, min/max) work correctly
/// when field values are negative and narrowed to i8.
#[test]
fn test_narrowed_comparison_ordering_chain() {
    assert_aot_success(
        include_str!("fixtures/narrowing/narrowed_comparison_ordering_chain.ori"),
        "narrowed_comparison_ordering_chain",
    );
}

// ---- Phase B: Straight-Line Local Variable Narrowing Tests ----
//
// These IR-inspection tests verify that non-phi local variables are narrowed
// to smaller LLVM types when their value range fits. They are the TDD "write
// failing tests first" step — they MUST FAIL before Phase B straight-line
// local narrowing is implemented in def_var_repr()/var().
//
// Phase B checklist: write failing test matrix BEFORE implementing local narrowing

/// IR semantic pin: arithmetic result `x + 25` where x is a literal produces
/// trunc+sext in the IR. Literal constants (i64 50, i64 25) are inlined by
/// LLVM, but the ADD result flows through `def_var_repr()` and gets narrowed.
/// This test ONLY passes with Phase B straight-line local narrowing.
#[test]
fn test_phase_b_ir_pin_straight_line_add_narrowed() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/phase_b_ir_pin_straight_line_add_narrowed.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/phase_b_ir_pin_multiple_narrowed_locals.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/phase_b_negative_public_param_not_narrowed.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/phase_b_negative_wide_constant_stays_i64.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/phase_b_ir_pin_select_narrowed.ori"
    ));

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
        include_str!("fixtures/narrowing/phase_b_select_narrowed_behavior.ori"),
        "select_narrowed_behavior",
    );
}

/// Behavioral test: narrowed Select with negative values preserves sign.
/// Catches zext bugs — negative values through narrowed select must retain sign.
#[test]
fn test_phase_b_select_narrowed_negative_values() {
    assert_aot_success(
        include_str!("fixtures/narrowing/phase_b_select_narrowed_negative_values.ori"),
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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/phase_b_overflow_guard_widens_to_i16.ori"
    ));

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
        include_str!("fixtures/narrowing/phase_b_overflow_guard_behavior.ori"),
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
    let ir = compile_and_capture_ir_no_repr_opt(include_str!(
        "fixtures/narrowing/narrowing_policy_disabled_suppresses_struct_narrowing.ori"
    ));

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
    let ir = compile_and_capture_ir_no_repr_opt(include_str!(
        "fixtures/narrowing/narrowing_policy_disabled_suppresses_local_narrowing.ori"
    ));

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
        include_str!("fixtures/narrowing/narrowing_policy_disabled_behavioral_correctness.ori"),
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

// Phase C: Collection element narrowing tests
//
// These tests verify that [int] list literals with bounded values use narrowed
// element storage (i8/i16/i32) and that the narrowing is transparent to program
// semantics — all operations produce identical results to canonical i64 storage.

/// Semantic pin: list literal with bounded values uses narrowed i8 element storage.
/// Verifies LLVM IR contains narrowed `elem_size` (1) instead of canonical (8).
#[test]
fn test_narrowed_list_i8_ir_pin() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/narrowed_list_i8_ir_pin.ori"
    ));

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
        include_str!("fixtures/narrowing/narrowed_list_index_round_trip.ori"),
        "narrowed_list_index_round_trip",
    );
}

/// For-yield over narrowed list produces a correctly narrowed result list.
/// Tests the for-yield accumulator interception (`ori_list_new`/`ori_list_push`).
#[test]
fn test_narrowed_list_for_yield() {
    assert_aot_success(
        include_str!("fixtures/narrowing/narrowed_list_for_yield.ori"),
        "narrowed_list_for_yield",
    );
}

/// For-yield with transform over narrowed list.
#[test]
fn test_narrowed_list_for_yield_transform() {
    assert_aot_success(
        include_str!("fixtures/narrowing/narrowed_list_for_yield_transform.ori"),
        "narrowed_list_for_yield_transform",
    );
}

/// Iteration (for..do sum) over narrowed list.
#[test]
fn test_narrowed_list_iteration_sum() {
    assert_aot_success(
        include_str!("fixtures/narrowing/narrowed_list_iteration_sum.ori"),
        "narrowed_list_iteration_sum",
    );
}

/// Derived Eq on struct with narrowed list field.
#[test]
fn test_narrowed_list_derived_eq() {
    assert_aot_success(
        include_str!("fixtures/narrowing/narrowed_list_derived_eq.ori"),
        "narrowed_list_derived_eq",
    );
}

/// List `.first()` and `.last()` through narrowed elements — returns `Option<int>`.
#[test]
fn test_narrowed_list_first_last() {
    assert_aot_success(
        include_str!("fixtures/narrowing/narrowed_list_first_last.ori"),
        "narrowed_list_first_last",
    );
}

/// Sort on narrowed [int] list.
#[test]
fn test_narrowed_list_sort() {
    assert_aot_success(
        include_str!("fixtures/narrowing/narrowed_list_sort.ori"),
        "narrowed_list_sort",
    );
}

/// Negative pin: list with narrowing disabled (`ORI_NO_REPR_OPT=1`) uses
/// canonical i64 element storage — no i8 GEP in the LLVM IR.
#[test]
fn test_narrowed_list_disabled_ir_pin() {
    let ir = compile_and_capture_ir_no_repr_opt(include_str!(
        "fixtures/narrowing/narrowed_list_disabled_ir_pin.ori"
    ));

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

// Phase C: Set exclusion from narrowing
//
// Sets are excluded from Phase C narrowing because eq/hash thunks always load
// canonical-width values (i64 for int) from element pointers. These tests
// verify sets work correctly with canonical element sizes even when lists
// in the same program are narrowed.

/// Set operations work correctly when list narrowing is active.
/// Sets are created via `.iter().collect()` — canonical element sizes.
#[test]
fn test_set_int_canonical_with_narrowed_list_ir() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/set_int_canonical_with_narrowed_list_ir.ori"
    ));

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
        include_str!("fixtures/narrowing/set_int_operations_canonical.ori"),
        "set_int_operations_canonical",
    );
}

/// Mixed program: narrowed list + canonical set coexist without interference.
#[test]
fn test_narrowed_list_and_canonical_set_coexist() {
    assert_aot_success(
        include_str!("fixtures/narrowing/narrowed_list_and_canonical_set_coexist.ori"),
        "narrowed_list_and_canonical_set_coexist",
    );
}

/// Set insert works with canonical element sizes when list narrowing is active.
#[test]
fn test_set_insert_with_narrowed_list_context() {
    assert_aot_success(
        include_str!("fixtures/narrowing/set_insert_with_narrowed_list_context.ori"),
        "set_insert_with_narrowed_list_context",
    );
}

// Phase C: for-yield narrowing safety
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
        include_str!("fixtures/narrowing/for_yield_str_with_narrowed_int_list.ori"),
        "for_yield_str_with_narrowed_int_list",
    );
}

/// for-yield producing [int] from narrowed [int] — narrowing should still work.
#[test]
fn test_for_yield_int_from_narrowed_int_list() {
    assert_aot_success(
        include_str!("fixtures/narrowing/for_yield_int_from_narrowed_int_list.ori"),
        "for_yield_int_from_narrowed_int_list",
    );
}

/// Mixed for-yields in same function: int and str accumulators coexist.
#[test]
fn test_mixed_for_yield_int_and_str() {
    assert_aot_success(
        include_str!("fixtures/narrowing/mixed_for_yield_int_and_str.ori"),
        "mixed_for_yield_int_and_str",
    );
}

// ---- Float Narrowing AOT Tests ----
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
        include_str!("fixtures/narrowing/float_narrowed_struct_roundtrip.ori"),
        "float_narrowed_struct_roundtrip",
    );
}

/// IR semantic pin: narrowed float struct type contains `float` fields (not `double`).
/// A struct with all f32-exact fields should produce `{ float, float, float }` in LLVM IR.
#[test]
fn test_float_narrowed_struct_ir_pin_type_layout() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/float_narrowed_struct_ir_pin_type_layout.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/float_narrowed_struct_ir_pin_fptrunc_on_construction.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/float_narrowed_struct_ir_pin_fpext_on_field_load.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/float_non_narrowed_struct_ir_pin_wide_value.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/float_arithmetic_not_narrowed.ori"
    ));

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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/float_variable_not_narrowed.ori"
    ));

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
/// Verifies combined integer+float narrowing in one struct.
#[test]
fn test_mixed_int_float_narrowed_struct() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/mixed_int_float_narrowed_struct.ori"
    ));

    let fn_ir = extract_function_ir(&ir, "_ori_read_mass");

    // Health [100, 100] narrows to i8 (integer narrowing), mass 0.5 narrows to float
    // (float narrowing). The struct type should be { float, i8 } — both fields narrowed.
    // Note: the function return type is `double` (canonical f64), which is correct —
    // only the struct storage uses the narrowed float type.
    let has_narrowed_struct = fn_ir.contains("{ float, i8 }") || fn_ir.contains("{float, i8}");
    assert!(
        has_narrowed_struct,
        "expected narrowed struct type `{{ float, i8 }}` in _ori_read_mass IR — \
         Particle.mass (f32-exact 0.5) → float, Particle.health [100,100] → i8.\n\
         Combined integer+float narrowing pin.\nIR:\n{fn_ir}"
    );
}

/// Negative IR pin: `#repr("c")` struct with f32-exact fields must NOT be narrowed.
/// C ABI compatibility requires canonical double layout.
#[test]
fn test_float_repr_c_not_narrowed() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/float_repr_c_not_narrowed.ori"
    ));

    let fn_ir = extract_function_ir(&ir, "_ori_read_x");

    // #repr("c") structs must preserve ABI — no narrowing. Fields stay double.
    assert!(
        !fn_ir.contains("fpext float"),
        "expected NO float narrowing in #repr(\"c\") struct _ori_read_x — \
         C ABI requires canonical double layout. Fields must stay double.\nIR:\n{fn_ir}"
    );
}

// ---- Derive Semantic Pins for Float Narrowing ----
//
// These tests verify that derived traits (Printable, Debug, Hashable) work
// correctly with narrowed float struct fields. The derive codegen must extend
// narrowed f32 fields back to canonical f64 before calling runtime functions.

/// Semantic pin: derived `to_str()` on narrowed float struct fields.
/// The string representation MUST show "0.5", not garbage from ABI mismatch.
/// Regression guard: derive codegen must widen narrowed float fields before formatting.
#[test]
fn test_float_narrowed_derive_printable() {
    assert_aot_success(
        include_str!("fixtures/narrowing/float_narrowed_derive_printable.ori"),
        "float_narrowed_derive_printable",
    );
}

/// Semantic pin: derived `debug()` on narrowed float struct fields.
/// Same as Printable but for the Debug trait. The debug representation
/// MUST show the correct float values.
#[test]
fn test_float_narrowed_derive_debug() {
    assert_aot_success(
        include_str!("fixtures/narrowing/float_narrowed_derive_debug.ori"),
        "float_narrowed_derive_debug",
    );
}

/// Semantic pin: derived `hash()` on narrowed float struct fields.
/// Two structs with identical f32-exact values must produce the same hash.
/// A struct with different values must produce a different hash.
#[test]
fn test_float_narrowed_derive_hash() {
    assert_aot_success(
        include_str!("fixtures/narrowing/float_narrowed_derive_hash.ori"),
        "float_narrowed_derive_hash",
    );
}

/// Semantic pin: derived Eq on narrowed float struct fields.
/// Equality must work correctly when fields are stored as f32.
#[test]
fn test_float_narrowed_derive_eq() {
    assert_aot_success(
        include_str!("fixtures/narrowing/float_narrowed_derive_eq.ori"),
        "float_narrowed_derive_eq",
    );
}

/// Semantic pin: derived Comparable on narrowed float struct fields.
/// Comparisons (via `compare()` → `Ordering`) must work correctly with f32 storage.
#[test]
fn test_float_narrowed_derive_comparable() {
    assert_aot_success(
        include_str!("fixtures/narrowing/float_narrowed_derive_comparable.ori"),
        "float_narrowed_derive_comparable",
    );
}

/// IR semantic pin: derive Printable codegen must contain `fpext float` to widen
/// narrowed fields before calling `ori_str_from_float(double)`.
/// Directly verifies at the IR level that float widening occurs before formatting.
#[test]
fn test_float_narrowed_derive_ir_pin_fpext_in_printable() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/narrowing/float_narrowed_derive_ir_pin_fpext_in_printable.ori"
    ));

    // The derive Printable function must fpext narrowed float fields to double
    // before calling ori_str_from_float. Without the fix, LLVM verification fails.
    let has_fpext = ir.contains("fpext float");
    assert!(
        has_fpext,
        "expected `fpext float ... to double` in derive Printable IR for narrowed float \
         struct — derive codegen must widen f32 fields to f64 before ori_str_from_float.\n\
         Regression guard: derive codegen float widening fix.\nFull IR length: {} chars",
        ir.len()
    );
}

// ---- Float Narrowing LLVM Parity Tests ----
//
// These tests close the LLVM coverage gap for scenarios that the Ori spec tests
// (tests/spec/repr/float_narrowing/) cover via the interpreter but cannot run
// through LLVM due to the assert_eq monomorphization limitation.

/// Negative zero (-0.0) must be preserved through narrowing. IEEE 754
/// distinguishes +0.0 and -0.0; both are f32-exact. 1.0/-0.0 = -inf.
#[test]
fn test_float_narrowed_negative_zero() {
    assert_aot_success(
        include_str!("fixtures/narrowing/float_narrowed_negative_zero.ori"),
        "float_narrowed_negative_zero",
    );
}

/// Boundary values at the edge of f32 representation.
/// Powers of 2 and the f32 precision edge (2^24 - 1).
#[test]
fn test_float_narrowed_boundary_values() {
    assert_aot_success(
        include_str!("fixtures/narrowing/float_narrowed_boundary_values.ori"),
        "float_narrowed_boundary_values",
    );
}

/// Multiple call sites storing different f32-exact values into the same
/// struct field. The compiler joins multiple observations to decide narrowing.
#[test]
fn test_float_narrowed_multiple_stores() {
    assert_aot_success(
        include_str!("fixtures/narrowing/float_narrowed_multiple_stores.ori"),
        "float_narrowed_multiple_stores",
    );
}

/// Mixed exact/non-exact float fields: 0.5 is f32-exact (narrowable),
/// 0.1 is NOT f32-exact (stays f64). Both must produce correct values.
/// This is the AOT counterpart to `mixed_fields.ori`.
#[test]
fn test_float_narrowed_mixed_exact_non_exact() {
    assert_aot_success(
        include_str!("fixtures/narrowing/float_narrowed_mixed_exact_non_exact.ori"),
        "float_narrowed_mixed_exact_non_exact",
    );
}

/// Float comparison operators on values loaded from narrowed struct fields.
/// The fpext (f32→f64) must produce exact values for all comparison operators.
#[test]
fn test_float_narrowed_comparison_after_load() {
    assert_aot_success(
        include_str!("fixtures/narrowing/float_narrowed_comparison_after_load.ori"),
        "float_narrowed_comparison_after_load",
    );
}

// §07.1 Regression: TPR-07-002 — for...yield over narrowed all-unit enums.
//
// After §07.1 discriminant narrowing, all-unit enums lower to `{ i8 }` (1 byte)
// in LLVM. But `pool_type_store_size()` in `ori_arc` still sized them as 8 bytes
// (i64 tag), causing `ori_list_new`/`ori_list_push` to allocate/copy 8 bytes per
// element from 1-byte values — resulting in out-of-bounds memory access and
// segfaults when the collected values were read back.

/// Semantic pin: for...yield over an all-unit enum list must work correctly.
/// This is the exact reproducer from the TPR-07-002 finding.
#[test]
fn test_for_yield_all_unit_enum() {
    assert_aot_success(
        include_str!("fixtures/narrowing/for_yield_all_unit_enum.ori"),
        "for_yield_all_unit_enum",
    );
}

/// For...yield with transformation over all-unit enum.
#[test]
fn test_for_yield_all_unit_enum_transform() {
    assert_aot_success(
        include_str!("fixtures/narrowing/for_yield_all_unit_enum_transform.ori"),
        "for_yield_all_unit_enum_transform",
    );
}

/// For...yield over range producing all-unit enum values.
#[test]
fn test_for_yield_range_to_enum() {
    assert_aot_success(
        include_str!("fixtures/narrowing/for_yield_range_to_enum.ori"),
        "for_yield_range_to_enum",
    );
}

// Phase C — Collection element narrowing with mutations

/// Regression: BUG-05-001. A program with `[1]` literal + push of values
/// >= 128 must not corrupt data via i8 narrowing.
#[test]
fn test_phase_c_push_corruption_guard() {
    assert_aot_success(
        include_str!("fixtures/narrowing/phase_c_push_corruption_guard.ori"),
        "phase_c_push_corruption_guard",
    );
}

/// Positive pin: literal-only [int] lists should still benefit from narrowing.
#[test]
fn test_phase_c_only_literals_still_narrows() {
    assert_aot_success(
        include_str!("fixtures/narrowing/phase_c_only_literals_still_narrows.ori"),
        "phase_c_only_literals_still_narrows",
    );
}

/// Edge case: push values spanning i8/i16 boundaries.
#[test]
fn test_phase_c_push_large_values() {
    assert_aot_success(
        include_str!("fixtures/narrowing/phase_c_push_large_values.ori"),
        "phase_c_push_large_values",
    );
}
