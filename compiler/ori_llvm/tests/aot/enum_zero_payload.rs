//! AOT tests for BUG-04-008: zero-sized enum payload mismatch.
//!
//! Verifies that enum variants with void/unit payloads are correctly
//! handled in LLVM codegen — void fields should contribute 0 bytes
//! to enum payload size, not 8 bytes (the i64 placeholder).

use crate::util::{compile_and_run, compile_and_run_capture, compile_to_llvm_ir};

/// Exact failing case from BUG-04-008: `A(u: void) | B` should compile and run.
#[test]
fn test_void_payload_enum_compiles() {
    let exit_code = compile_and_run(
        r"
type E = A(u: void) | B;

@main () -> int = {
    let x = A(u: ());
    0
}
",
    );
    assert_eq!(exit_code, 0, "void-payload enum should compile and run");
}

/// Match on void-payload variant should produce correct result.
#[test]
fn test_void_payload_enum_match() {
    let (exit_code, stdout, _) = compile_and_run_capture(
        r#"
type E = UnitA(u: void) | UnitB;

@check (e: E) -> str = match e {
    UnitA(u:) -> "a",
    UnitB -> "b",
}

@main () -> void = {
    print(msg: check(e: UnitA(u: ())));
    print(msg: check(e: UnitB));
}
"#,
    );
    assert_eq!(exit_code, 0, "compilation should succeed");
    assert_eq!(
        stdout.trim(),
        "a\nb",
        "match should produce correct results"
    );
}

/// Mixed enum: real payload alongside void payload.
#[test]
fn test_mixed_payload_with_void() {
    let (exit_code, stdout, _) = compile_and_run_capture(
        r#"
type Mixed = Real(x: int) | Empty(u: void) | Plain;

@describe (m: Mixed) -> str = match m {
    Real(x:) -> `real({x})`,
    Empty(u:) -> "empty",
    Plain -> "plain",
}

@main () -> void = {
    print(msg: describe(m: Real(x: 42)));
    print(msg: describe(m: Empty(u: ())));
    print(msg: describe(m: Plain));
}
"#,
    );
    assert_eq!(exit_code, 0, "compilation should succeed");
    assert_eq!(
        stdout.trim(),
        "real(42)\nempty\nplain",
        "mixed enum with void payload should work correctly"
    );
}

/// Semantic pin: void-payload enum should have the same LLVM layout
/// as an all-unit enum (tag only, no payload array).
#[test]
fn test_void_payload_enum_layout_is_tag_only() {
    let ir = compile_to_llvm_ir(
        r"
type WithVoid = VA(u: void) | VB;

@pick (e: WithVoid) -> int = match e {
    VA(u:) -> 1,
    VB -> 2,
}

@main () -> int = pick(e: VA(u: ()));
",
    )
    .expect("compilation should succeed");

    // Void-payload enum should be tag-only `{ i8 }`, same as all-unit enum.
    // If void fields incorrectly contribute 8 bytes, the layout would be
    // `{ i8, [1 x i64] }` — which is wrong.
    assert!(
        ir.contains("type { i8 }"),
        "Void-payload enum should use tag-only layout {{ i8 }}, not {{ i8, [1 x i64] }}.\n\
         IR types:\n{}",
        ir.lines()
            .filter(|l: &&str| l.contains("WithVoid") || l.contains("type {"))
            .take(10)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// All variants with void payloads should be equivalent to all-unit enum.
#[test]
fn test_all_void_payload_variants() {
    let exit_code = compile_and_run(
        r"
type AllVoid = X(v: void) | Y(w: void) | Z;

@main () -> int = {
    let a = X(v: ());
    let b = Y(w: ());
    let c = Z;
    let ra = match a { X(v:) -> 1, Y(w:) -> 2, Z -> 3 };
    let rb = match b { X(v:) -> 1, Y(w:) -> 2, Z -> 3 };
    let rc = match c { X(v:) -> 1, Y(w:) -> 2, Z -> 3 };
    if ra == 1 then { if rb == 2 then { if rc == 3 then 0 else 1 } else 1 } else 1
}
",
    );
    assert_eq!(exit_code, 0, "all-void-payload enum should work correctly");
}
