//! AOT Derive Trait Codegen Tests
//!
//! End-to-end tests verifying that `#[derive(...)]` generates correct native code
//! through the LLVM backend. Each test compiles Ori source to a native binary,
//! runs it, and checks the exit code (0 = success).
//!
//! Covers roadmap Section 3.5: Derive Traits (Eq, Clone, Hashable, Printable).

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// 3.5.1: Derive Eq

#[test]
fn test_aot_derive_eq_basic() {
    assert_aot_success(
        r#"
#[derive(Eq)]
type Point = { x: int, y: int }

@main () -> int = run(
    let a = Point { x: 1, y: 2 },
    let b = Point { x: 1, y: 2 },
    let c = Point { x: 3, y: 4 },
    if a.eq(other: b) && !a.eq(other: c) then 0 else 1
)
"#,
        "derive_eq_basic",
    );
}

#[test]
fn test_aot_derive_eq_with_strings() {
    assert_aot_success(
        r#"
#[derive(Eq)]
type Config = { name: str }

@main () -> int = run(
    let a = Config { name: "hello" },
    let b = Config { name: "hello" },
    let c = Config { name: "world" },
    if a.eq(other: b) && !a.eq(other: c) then 0 else 1
)
"#,
        "derive_eq_with_strings",
    );
}

#[test]
fn test_aot_derive_eq_mixed_types() {
    assert_aot_success(
        r#"
#[derive(Eq)]
type Record = { id: int, active: bool, score: float }

@main () -> int = run(
    let a = Record { id: 1, active: true, score: 3.14 },
    let b = Record { id: 1, active: true, score: 3.14 },
    let c = Record { id: 1, active: false, score: 3.14 },
    if a.eq(other: b) && !a.eq(other: c) then 0 else 1
)
"#,
        "derive_eq_mixed_types",
    );
}

#[test]
fn test_aot_derive_eq_single_field() {
    assert_aot_success(
        r#"
#[derive(Eq)]
type Wrapper = { value: int }

@main () -> int = run(
    let a = Wrapper { value: 42 },
    let b = Wrapper { value: 42 },
    let c = Wrapper { value: 99 },
    if a.eq(other: b) && !a.eq(other: c) then 0 else 1
)
"#,
        "derive_eq_single_field",
    );
}

// 3.5.2: Derive Clone

#[test]
fn test_aot_derive_clone_basic() {
    assert_aot_success(
        r#"
#[derive(Eq, Clone)]
type Point = { x: int, y: int }

@main () -> int = run(
    let a = Point { x: 10, y: 20 },
    let b = a.clone(),
    if a.eq(other: b) then 0 else 1
)
"#,
        "derive_clone_basic",
    );
}

#[test]
fn test_aot_derive_clone_large_struct() {
    assert_aot_success(
        r#"
#[derive(Eq, Clone)]
type Big = { a: int, b: int, c: int }

@main () -> int = run(
    let x = Big { a: 1, b: 2, c: 3 },
    let y = x.clone(),
    if x.eq(other: y) then 0 else 1
)
"#,
        "derive_clone_large_struct",
    );
}

// 3.5.3: Derive Hashable

#[test]
fn test_aot_derive_hash_equal_values() {
    assert_aot_success(
        r#"
#[derive(Hashable)]
type Point = { x: int, y: int }

@main () -> int = run(
    let a = Point { x: 1, y: 2 },
    let b = Point { x: 1, y: 2 },
    if a.hash() == b.hash() then 0 else 1
)
"#,
        "derive_hash_equal_values",
    );
}

#[test]
fn test_aot_derive_hash_different_values() {
    assert_aot_success(
        r#"
#[derive(Hashable)]
type Point = { x: int, y: int }

@main () -> int = run(
    let a = Point { x: 1, y: 2 },
    let b = Point { x: 3, y: 4 },
    if a.hash() != b.hash() then 0 else 1
)
"#,
        "derive_hash_different_values",
    );
}

// 3.5.4: Derive Printable

#[test]
fn test_aot_derive_printable_basic() {
    assert_aot_success(
        r#"
#[derive(Printable)]
type Point = { x: int, y: int }

@main () -> int = run(
    let p = Point { x: 1, y: 2 },
    let s = p.to_str(),
    if s.len() > 0 then 0 else 1
)
"#,
        "derive_printable_basic",
    );
}

// 3.5.5: Derive Default

#[test]
fn test_aot_derive_default_basic() {
    assert_aot_success(
        r#"
#[derive(Default)]
type Point = { x: int, y: int }

@main () -> int = run(
    let p = Point.default(),
    if p.x == 0 && p.y == 0 then 0 else 1
)
"#,
        "derive_default_basic",
    );
}

#[test]
fn test_aot_derive_default_mixed_types() {
    assert_aot_success(
        r#"
#[derive(Default)]
type Config = { count: int, enabled: bool, score: float }

@main () -> int = run(
    let c = Config.default(),
    if c.count == 0 && c.enabled == false && c.score == 0.0 then 0 else 1
)
"#,
        "derive_default_mixed_types",
    );
}

#[test]
fn test_aot_derive_default_eq_integration() {
    assert_aot_success(
        r#"
#[derive(Default, Eq)]
type Point = { x: int, y: int }

@main () -> int = run(
    let a = Point.default(),
    let b = Point.default(),
    if a.eq(other: b) then 0 else 1
)
"#,
        "derive_default_eq_integration",
    );
}

// 3.7: Clone trait on primitives (built-in identity clone)

#[test]
fn test_aot_clone_int() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let x = 42,
    let y = x.clone(),
    if y == 42 then 0 else 1
)
"#,
        "clone_int",
    );
}

#[test]
fn test_aot_clone_float() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let x = 3.14,
    let y = x.clone(),
    if y == 3.14 then 0 else 1
)
"#,
        "clone_float",
    );
}

#[test]
fn test_aot_clone_bool() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let a = true.clone(),
    let b = false.clone(),
    if a && !b then 0 else 1
)
"#,
        "clone_bool",
    );
}

#[test]
fn test_aot_clone_str() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let s = "hello",
    let s2 = s.clone(),
    if s2 == "hello" then 0 else 1
)
"#,
        "clone_str",
    );
}

// 3.7: Clone on collections

#[test]
fn test_aot_clone_list_int() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let items = [1, 2, 3],
    let items2 = items.clone(),
    if items2.len() == 3 then 0 else 1
)
"#,
        "clone_list_int",
    );
}

#[test]
fn test_aot_clone_list_empty() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let items: [int] = [],
    let items2 = items.clone(),
    if items2.len() == 0 then 0 else 1
)
"#,
        "clone_list_empty",
    );
}

// 3.7: Clone on Option

#[test]
fn test_aot_clone_option_some() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let opt = Some(42),
    let opt2 = opt.clone(),
    if (opt2 ?? 0) == 42 then 0 else 1
)
"#,
        "clone_option_some",
    );
}

#[test]
fn test_aot_clone_option_none() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let opt: Option<int> = None,
    let opt2 = opt.clone(),
    if (opt2 ?? -1) == -1 then 0 else 1
)
"#,
        "clone_option_none",
    );
}

// 3.7: Clone on Result

#[test]
fn test_aot_clone_result_ok() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let r: Result<int, str> = Ok(42),
    let r2 = r.clone(),
    if r2.is_ok() then 0 else 1
)
"#,
        "clone_result_ok",
    );
}

#[test]
fn test_aot_clone_result_err() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let r: Result<int, str> = Err("fail"),
    let r2 = r.clone(),
    if r2.is_err() then 0 else 1
)
"#,
        "clone_result_err",
    );
}

// 3.7: Clone on tuples

#[test]
fn test_aot_clone_tuple_pair() {
    // Tuple destructuring not yet implemented in AOT codegen,
    // so we verify clone compiles and the value is usable.
    assert_aot_success(
        r#"
@main () -> int = run(
    let t = (42, 99),
    let _t2 = t.clone(),
    0
)
"#,
        "clone_tuple_pair",
    );
}

#[test]
fn test_aot_clone_tuple_triple() {
    // Tuple destructuring not yet implemented in AOT codegen,
    // so we verify clone compiles and the value is usable.
    assert_aot_success(
        r#"
@main () -> int = run(
    let t = (1, 2, 3),
    let _t2 = t.clone(),
    0
)
"#,
        "clone_tuple_triple",
    );
}

// 3.5.6: Multiple derives on one type

#[test]
fn test_aot_derive_multiple_traits() {
    assert_aot_success(
        r#"
#[derive(Eq, Clone)]
type Pair = { x: int, y: int }

@main () -> int = run(
    let a = Pair { x: 5, y: 10 },
    let b = a.clone(),
    if a.eq(other: b) then 0 else 1
)
"#,
        "derive_multiple_traits",
    );
}
