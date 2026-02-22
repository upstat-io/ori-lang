//! ARC Memory Management AOT Tests
//!
//! End-to-end tests verifying that ARC reference counting correctly frees
//! memory at runtime. Each test compiles an Ori program that creates RC'd
//! objects, lets them go out of scope, and verifies the drop chain runs
//! without crashing.
//!
//! These tests are slow (compile → link → execute per test) but essential
//! for verifying the full ARC pipeline works end-to-end.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Basic struct creation and drop ───

#[test]
fn test_arc_struct_basic_drop() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@main () -> int = {
    let p = Point { x: 1, y: 2 };
    if p.x + p.y == 3 then 0 else 1
}
"#,
        "arc_struct_basic_drop",
    );
}

#[test]
fn test_arc_struct_with_string_field() {
    assert_aot_success(
        r#"
type Named = { name: str, value: int }

@main () -> int = {
    let n = Named { name: "hello", value: 42 };
    if n.value == 42 then 0 else 1
}
"#,
        "arc_struct_with_string_field",
    );
}

#[test]
fn test_arc_nested_struct_drop() {
    assert_aot_success(
        r#"
type Inner = { a: int, b: int }
type Outer = { inner: Inner, c: int }

@main () -> int = {
    let i = Inner { a: 1, b: 2 };
    let o = Outer { inner: i, c: 3 };
    if o.inner.a + o.inner.b + o.c == 6 then 0 else 1
}
"#,
        "arc_nested_struct_drop",
    );
}

// ─── Struct sharing (refcount > 1) ───

#[test]
fn test_arc_shared_struct() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@main () -> int = {
    let p = Point { x: 10, y: 20 };
    let q = p;
    if p.x + q.y == 30 then 0 else 1
}
"#,
        "arc_shared_struct",
    );
}

// ─── Function passing (ownership transfer) ───

#[test]
fn test_arc_struct_passed_to_function() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@sum (p: Point) -> int = p.x + p.y;

@main () -> int = {
    let p = Point { x: 3, y: 4 };
    if sum(p) == 7 then 0 else 1
}
"#,
        "arc_struct_passed_to_function",
    );
}

#[test]
fn test_arc_struct_returned_from_function() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@make_point (x: int, y: int) -> Point = Point { x: x, y: y }

@main () -> int = {
    let p = make_point(5, 6);
    if p.x + p.y == 11 then 0 else 1
}
"#,
        "arc_struct_returned_from_function",
    );
}

// ─── Enum drop ───

#[test]
#[ignore = "LLVM codegen: enum variant constructors not yet implemented"]
fn test_arc_enum_basic_drop() {
    assert_aot_success(
        r#"
type Shape = Circle(radius: int) | Rectangle(width: int, height: int);

@main () -> int = {
    let c = Circle(radius: 5);
    let r = Rectangle(width: 3, height: 4);
    0
}
"#,
        "arc_enum_basic_drop",
    );
}

#[test]
#[ignore = "LLVM codegen: enum variant constructors not yet implemented"]
fn test_arc_enum_with_string_payload() {
    assert_aot_success(
        r#"
type Outcome = Good(value: int) | Bad(reason: str);

@main () -> int = {
    let ok = Good(value: 42);
    let err = Bad(reason: "oops");
    0
}
"#,
        "arc_enum_with_string_payload",
    );
}

// ─── Loop allocation (stress test for drops) ───

#[test]
fn test_arc_loop_allocation() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@main () -> int = {
    let sum = 0;
    for i in 0..100 do {
        let p = Point { x: i, y: i * 2 };
        sum = sum + p.x + p.y
    };
    if sum == 14850 then 0 else 1
}
"#,
        "arc_loop_allocation",
    );
}

#[test]
fn test_arc_loop_string_allocation() {
    assert_aot_success(
        r#"
type Named = { name: str, id: int }

@main () -> int = {
    let count = 0;
    for i in 0..50 do {
        let n = Named { name: "test", id: i };
        count = count + n.id - n.id + 1
    };
    if count == 50 then 0 else 1
}
"#,
        "arc_loop_string_allocation",
    );
}

// ─── List with RC'd elements ───

#[test]
fn test_arc_list_of_ints() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3, 4, 5];
    let sum = 0;
    for x in xs do sum = sum + x;
    if sum == 15 then 0 else 1
}
"#,
        "arc_list_of_ints",
    );
}

// ─── Multiple scopes ───

#[test]
fn test_arc_block_scope_drop() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@main () -> int = {
    let a = {
        let p = Point { x: 1, y: 2 };
        p.x + p.y
    };
    let b = {
        let q = Point { x: 3, y: 4 };
        q.x + q.y
    };
    if a + b == 10 then 0 else 1
}
"#,
        "arc_block_scope_drop",
    );
}

// ─── String operations (RC'd strings) ───

#[test]
fn test_arc_string_concat_drop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "hello";
    let b = " world";
    let c = a + b;
    if c == "hello world" then 0 else 1
}
"#,
        "arc_string_concat_drop",
    );
}

#[test]
fn test_arc_string_loop_concat() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "";
    for _ in 0..10 do s = s + "x";
    if s == "xxxxxxxxxx" then 0 else 1
}
"#,
        "arc_string_loop_concat",
    );
}
