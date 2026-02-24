//! Struct AOT Tests
//!
//! Tests for struct construction, field access, update syntax, nested structs,
//! structs as function parameters/returns, struct with various field types,
//! and struct interaction with closures and control flow.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Basic construction & field access ───

#[test]
fn test_struct_two_fields() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };

@main () -> int = {
    let p = Point { x: 3, y: 4 };
    if p.x == 3 && p.y == 4 then 0 else 1
}
"#,
        "struct_two_fields",
    );
}

#[test]
fn test_struct_three_fields() {
    assert_aot_success(
        r#"
type Vec3 = { x: int, y: int, z: int };

@main () -> int = {
    let v = Vec3 { x: 1, y: 2, z: 3 };
    if v.x + v.y + v.z == 6 then 0 else 1
}
"#,
        "struct_three_fields",
    );
}

#[test]
fn test_struct_single_field() {
    assert_aot_success(
        r#"
type Wrapper = { value: int };

@main () -> int = {
    let w = Wrapper { value: 42 };
    if w.value == 42 then 0 else 1
}
"#,
        "struct_single_field",
    );
}

#[test]
fn test_struct_bool_fields() {
    assert_aot_success(
        r#"
type Flags = { enabled: bool, visible: bool };

@main () -> int = {
    let f = Flags { enabled: true, visible: false };
    if f.enabled && !f.visible then 0 else 1
}
"#,
        "struct_bool_fields",
    );
}

#[test]
fn test_struct_mixed_fields() {
    assert_aot_success(
        r#"
type Person = { age: int, active: bool };

@main () -> int = {
    let p = Person { age: 30, active: true };
    if p.age == 30 && p.active then 0 else 1
}
"#,
        "struct_mixed_fields",
    );
}

// ─── String fields ───

#[test]
fn test_struct_string_field() {
    assert_aot_success(
        r#"
type Named = { name: str, id: int };

@main () -> int = {
    let n = Named { name: "alice", id: 1 };
    if n.name == "alice" && n.id == 1 then 0 else 1
}
"#,
        "struct_str_field",
    );
}

#[test]
fn test_struct_two_string_fields() {
    assert_aot_success(
        r#"
type FullName = { first: str, last: str };

@main () -> int = {
    let n = FullName { first: "Alice", last: "Smith" };
    if n.first == "Alice" && n.last == "Smith" then 0 else 1
}
"#,
        "struct_two_strs",
    );
}

#[test]
fn test_struct_string_field_method() {
    assert_aot_success(
        r#"
type Named = { name: str };

@main () -> int = {
    let n = Named { name: "hello" };
    if n.name.length() == 5 then 0 else 1
}
"#,
        "struct_str_method",
    );
}

// ─── Update syntax ───

#[test]
fn test_struct_update_one_field() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };

@main () -> int = {
    let p1 = Point { x: 1, y: 2 };
    let p2 = Point { ...p1, x: 10 };
    if p2.x == 10 && p2.y == 2 then 0 else 1
}
"#,
        "struct_update_one",
    );
}

#[test]
fn test_struct_update_all_fields() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };

@main () -> int = {
    let p1 = Point { x: 1, y: 2 };
    let p2 = Point { ...p1, x: 10, y: 20 };
    if p2.x == 10 && p2.y == 20 then 0 else 1
}
"#,
        "struct_update_all",
    );
}

#[test]
fn test_struct_update_preserves_original() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };

@main () -> int = {
    let p1 = Point { x: 1, y: 2 };
    let p2 = Point { ...p1, x: 10 };
    // p1 should be unchanged
    if p1.x == 1 && p1.y == 2 && p2.x == 10 then 0 else 1
}
"#,
        "struct_update_preserves",
    );
}

#[test]
fn test_struct_update_chain() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };

@main () -> int = {
    let p1 = Point { x: 0, y: 0 };
    let p2 = Point { ...p1, x: 5 };
    let p3 = Point { ...p2, y: 10 };
    if p3.x == 5 && p3.y == 10 then 0 else 1
}
"#,
        "struct_update_chain",
    );
}

#[test]
fn test_struct_update_with_string() {
    assert_aot_success(
        r#"
type Entry = { key: str, value: int };

@main () -> int = {
    let e1 = Entry { key: "x", value: 1 };
    let e2 = Entry { ...e1, value: 42 };
    if e2.key == "x" && e2.value == 42 then 0 else 1
}
"#,
        "struct_update_str",
    );
}

// ─── Nested structs ───

#[test]
fn test_struct_nested_basic() {
    assert_aot_success(
        r#"
type Inner = { value: int };
type Outer = { inner: Inner, extra: int };

@main () -> int = {
    let o = Outer { inner: Inner { value: 42 }, extra: 10 };
    if o.inner.value == 42 && o.extra == 10 then 0 else 1
}
"#,
        "struct_nested_basic",
    );
}

#[test]
fn test_struct_nested_three_levels() {
    assert_aot_success(
        r#"
type A = { val: int };
type B = { a: A };
type C = { b: B };

@main () -> int = {
    let c = C { b: B { a: A { val: 99 } } };
    if c.b.a.val == 99 then 0 else 1
}
"#,
        "struct_nested_3",
    );
}

#[test]
fn test_struct_nested_with_string() {
    assert_aot_success(
        r#"
type Inner = { name: str };
type Outer = { inner: Inner, count: int };

@main () -> int = {
    let o = Outer { inner: Inner { name: "hello" }, count: 5 };
    if o.inner.name == "hello" && o.count == 5 then 0 else 1
}
"#,
        "struct_nested_str",
    );
}

// ─── Struct as function param/return ───

#[test]
fn test_struct_as_param() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };

@manhattan (p: Point) -> int = {
    let ax = if p.x < 0 then -p.x else p.x;
    let ay = if p.y < 0 then -p.y else p.y;
    ax + ay
};

@main () -> int = {
    let p = Point { x: -3, y: 4 };
    if manhattan(p: p) == 7 then 0 else 1
}
"#,
        "struct_as_param",
    );
}

#[test]
fn test_struct_as_return() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };

@origin () -> Point = Point { x: 0, y: 0 };

@main () -> int = {
    let p = origin();
    if p.x == 0 && p.y == 0 then 0 else 1
}
"#,
        "struct_as_return",
    );
}

#[test]
fn test_struct_param_and_return() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };

@translate (p: Point, dx: int, dy: int) -> Point = {
    Point { x: p.x + dx, y: p.y + dy }
};

@main () -> int = {
    let p = Point { x: 1, y: 2 };
    let p2 = translate(p: p, dx: 10, dy: 20);
    if p2.x == 11 && p2.y == 22 then 0 else 1
}
"#,
        "struct_param_return",
    );
}

#[test]
fn test_struct_multiple_params() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };

@distance_sq (a: Point, b: Point) -> int = {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
};

@main () -> int = {
    let p1 = Point { x: 0, y: 0 };
    let p2 = Point { x: 3, y: 4 };
    // 9 + 16 = 25
    if distance_sq(a: p1, b: p2) == 25 then 0 else 1
}
"#,
        "struct_multi_param",
    );
}

// ─── Struct in control flow ───

#[test]
fn test_struct_from_if() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };

@main () -> int = {
    let flag = true;
    let p = if flag then Point { x: 1, y: 2 } else Point { x: 3, y: 4 };
    if p.x == 1 && p.y == 2 then 0 else 1
}
"#,
        "struct_from_if",
    );
}

#[test]
fn test_struct_in_loop() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };

@main () -> int = {
    let sum = 0;
    for i in 0..5 do {
        let p = Point { x: i, y: i * 2 };
        sum = sum + p.x + p.y;
    };
    // (0+0) + (1+2) + (2+4) + (3+6) + (4+8) = 30
    if sum == 30 then 0 else 1
}
"#,
        "struct_in_loop",
    );
}

// ─── Struct with closures ───

#[test]
fn test_struct_closure_field_access() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };

@main () -> int = {
    let p = Point { x: 10, y: 20 };
    let get_sum = (extra: int) -> p.x + p.y + extra;
    if get_sum(12) == 42 then 0 else 1
}
"#,
        "struct_closure_field",
    );
}

#[test]
fn test_struct_returned_from_closure() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };

@main () -> int = {
    let make = (n: int) -> Point { x: n, y: n * 2 };
    let p = make(5);
    if p.x == 5 && p.y == 10 then 0 else 1
}
"#,
        "struct_from_closure",
    );
}

// ─── Struct equality ───

#[test]
fn test_struct_derived_eq() {
    assert_aot_success(
        r#"
#[derive(Eq)]
type Point = { x: int, y: int }

@main () -> int = {
    let a = Point { x: 1, y: 2 };
    let b = Point { x: 1, y: 2 };
    let c = Point { x: 1, y: 3 };
    if a == b && a != c then 0 else 1
}
"#,
        "struct_derived_eq",
    );
}

#[test]
fn test_struct_derived_eq_string() {
    assert_aot_success(
        r#"
#[derive(Eq)]
type Named = { name: str, id: int }

@main () -> int = {
    let a = Named { name: "alice", id: 1 };
    let b = Named { name: "alice", id: 1 };
    let c = Named { name: "bob", id: 1 };
    if a == b && a != c then 0 else 1
}
"#,
        "struct_eq_string",
    );
}

// ─── Multiple struct types ───

#[test]
fn test_struct_multiple_types() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };
type Rect = { width: int, height: int };

@area (r: Rect) -> int = r.width * r.height;

@main () -> int = {
    let p = Point { x: 1, y: 2 };
    let r = Rect { width: 3, height: 4 };
    if p.x + p.y == 3 && area(r: r) == 12 then 0 else 1
}
"#,
        "struct_multi_types",
    );
}

// ─── Struct with list field ───

#[test]
fn test_struct_list_field() {
    assert_aot_success(
        r#"
type Container = { items: [int], label: str };

@main () -> int = {
    let c = Container { items: [1, 2, 3], label: "test" };
    if c.items.length() == 3 && c.label == "test" then 0 else 1
}
"#,
        "struct_list_field",
    );
}

// ─── Struct field computation ───

#[test]
fn test_struct_computed_fields() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };

@main () -> int = {
    let a = 5;
    let b = 10;
    let p = Point { x: a * 2, y: b - 3 };
    if p.x == 10 && p.y == 7 then 0 else 1
}
"#,
        "struct_computed",
    );
}

#[test]
fn test_struct_field_from_function() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };
@double (n: int) -> int = n * 2;

@main () -> int = {
    let p = Point { x: double(n: 5), y: double(n: 10) };
    if p.x == 10 && p.y == 20 then 0 else 1
}
"#,
        "struct_field_from_fn",
    );
}
