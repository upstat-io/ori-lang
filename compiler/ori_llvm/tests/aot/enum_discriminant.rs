//! AOT tests for §07.1 discriminant narrowing.
//!
//! Verifies that enum tags are emitted as narrowed integer types in LLVM IR
//! (i8 instead of i64 for ≤256 variants) and that behavioral correctness
//! is preserved.

use crate::util::{assert_aot_success, compile_and_run_capture, compile_to_llvm_ir};

/// Semantic pin: all-unit enum LLVM type is `{ i8 }` (not `{ i64 }`).
#[test]
fn test_all_unit_enum_tag_is_i8() {
    let ir = compile_to_llvm_ir(
        r"
type Color = Red | Green | Blue;

@pick (c: Color) -> int = match c {
    Red -> 1,
    Green -> 2,
    Blue -> 3,
}

@main () -> int = pick(c: Green);
",
    )
    .expect("compilation failed");

    // The Color enum type should contain i8 (narrowed tag), not i64
    assert!(
        ir.contains("type { i8 }"),
        "All-unit enum should use i8 tag, not i64. IR snippet:\n{}",
        ir.lines()
            .filter(|l: &&str| l.contains("Color") || l.contains("type {"))
            .take(5)
            .collect::<Vec<_>>()
            .join("\n")
    );
    // Negative pin: must NOT contain i64 tag for this enum
    assert!(
        !ir.contains("%Color = type { i64 }"),
        "All-unit enum should NOT use i64 tag"
    );
}

/// Documents: Option/Result keep i64 tags for runtime (`ori_rt`) compatibility.
/// User-defined enums get i8 tags via `resolve_enum()`.
/// Narrowing Option/Result tags requires coordinated `ori_rt` update.
#[test]
fn test_option_keeps_i64_tag_for_runtime_compat() {
    let ir = compile_to_llvm_ir(
        r#"
@get () -> Option<str> = Some("hello");

@main () -> int = {
    let o = get();
    match o {
        Some(_) -> 0,
        None -> 1,
    }
}
"#,
    )
    .expect("compilation failed");

    // Option<str> uses {i64, {i64, i64, ptr}} — i64 tag for runtime compatibility
    // The runtime (ori_list_first, ori_map_get, etc.) writes i64 tags to sret ptrs
    assert!(
        ir.contains("{ i64,"),
        "Option should use i64 tag for runtime compatibility. IR snippet:\n{}",
        ir.lines()
            .filter(|l: &&str| l.contains("sret") || l.contains("{ i64"))
            .take(5)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Behavioral correctness: all-unit enum match produces correct values.
#[test]
fn test_all_unit_enum_match_correctness() {
    assert_aot_success(
        r"
type Dir = North | South | East | West;

@dir_val (d: Dir) -> int = match d {
    North -> 1,
    South -> 2,
    East -> 3,
    West -> 4,
}

@main () -> int = {
    let sum = dir_val(d: North) + dir_val(d: South) + dir_val(d: East) + dir_val(d: West);
    if sum == 10 then 0 else 1
}
",
        "all_unit_enum_match",
    );
}

/// Behavioral correctness: Option match with narrowed tag.
#[test]
fn test_option_match_narrowed_tag() {
    let (exit_code, stdout, stderr) = compile_and_run_capture(
        r"
@main () -> void = {
    let some_val = Some(42);
    let none_val: Option<int> = None;
    let r1 = match some_val { Some(v) -> v, None -> 0 };
    let r2 = match none_val { Some(v) -> v, None -> -1 };
    print(msg: `{r1},{r2}`)
}
",
    );
    assert_eq!(exit_code, 0, "option match failed: {stderr}");
    assert_eq!(stdout.trim(), "42,-1");
}

/// Behavioral correctness: Result match with narrowed tag.
#[test]
fn test_result_match_narrowed_tag() {
    let (exit_code, stdout, stderr) = compile_and_run_capture(
        r#"
@main () -> void = {
    let ok_val: Result<int, str> = Ok(100);
    let err_val: Result<int, str> = Err("fail");
    let r1 = match ok_val { Ok(v) -> v, Err(_) -> -1 };
    let r2 = match err_val { Ok(_) -> -1, Err(msg) -> len(collection: msg) };
    print(msg: `{r1},{r2}`)
}
"#,
    );
    assert_eq!(exit_code, 0, "result match failed: {stderr}");
    assert_eq!(stdout.trim(), "100,4");
}

/// Behavioral + leak check: enum with RC payload and narrowed tag.
#[test]
fn test_enum_rc_payload_narrowed_tag() {
    assert_aot_success(
        r#"
type Container = Empty | WithStr(val: str) | WithList(items: [int]);

@extract (c: Container) -> str = match c {
    Empty -> "empty",
    WithStr(val:) -> val,
    WithList(items:) -> `list of {len(collection: items)}`,
}

@main () -> int = {
    let c1 = extract(c: Empty);
    let c2 = extract(c: WithStr(val: "hello"));
    let c3 = extract(c: WithList(items: [1, 2, 3]));
    if c1 == "empty" then {
        if c2 == "hello" then {
            if c3 == "list of 3" then 0 else 3
        } else 2
    } else 1
}
"#,
        "enum_rc_payload_narrowed",
    );
}
