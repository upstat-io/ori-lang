//! F20: Derived Clone — cloning types containing fat pointer fields.
//!
//! Tests Clone codegen for structs and sum types whose fields include strings,
//! lists, and other fat pointer types. Clone must correctly RC-increment all
//! heap-allocated fields.

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// T4/T5: Clone struct with str field
#[test]
fn test_fm_clone_struct_str() {
    assert_aot_success(
        r#"
#derive(Eq, Clone)
type Named = { name: str, id: int }

@main () -> int = {
    let a = Named { name: "alice", id: 1 };
    let b = a.clone();
    if a == b then {
        if b.name.length() == 5 then 0 else 1
    } else 2
}
"#,
        "fm_clone_struct_str",
    );
}

// T5: Clone struct with heap str field
#[test]
fn test_fm_clone_struct_str_heap() {
    assert_aot_success(
        r#"
#derive(Eq, Clone)
type Doc = { content: str }

@main () -> int = {
    let a = Doc { content: "abcdefghijklmnopqrstuvwxyz1234" };
    let b = a.clone();
    if a == b then {
        if b.content.length() == 30 then 0 else 1
    } else 2
}
"#,
        "fm_clone_struct_str_heap",
    );
}

// T6: Clone struct with list of scalars
#[test]
fn test_fm_clone_struct_list_scalar() {
    assert_aot_success(
        r#"
#derive(Eq, Clone)
type Container = { items: [int] }

@main () -> int = {
    let a = Container { items: [1, 2, 3] };
    let b = a.clone();
    if a == b then {
        if b.items.length() == 3 then 0 else 1
    } else 2
}
"#,
        "fm_clone_struct_list_scalar",
    );
}

// T7: Clone struct with list of fat pointers
#[test]
fn test_fm_clone_struct_list_fat() {
    assert_aot_success(
        r#"
#derive(Eq, Clone)
type Words = { items: [str] }

@main () -> int = {
    let a = Words { items: ["hello", "world"] };
    let b = a.clone();
    if a == b then {
        if b.items.length() == 2 then 0 else 1
    } else 2
}
"#,
        "fm_clone_struct_list_fat",
    );
}

// T9: Clone struct with nested fat fields
#[test]
fn test_fm_clone_nested_fat() {
    assert_aot_success(
        r#"
#derive(Eq, Clone)
type Inner = { name: str }
#derive(Eq, Clone)
type Outer = { inner: Inner, count: int }

@main () -> int = {
    let a = Outer { inner: Inner { name: "alice" }, count: 5 };
    let b = a.clone();
    if a == b then {
        if b.inner.name.length() == 5 then 0 else 1
    } else 2
}
"#,
        "fm_clone_nested_fat",
    );
}

// Multiple fat fields — both must be cloned
#[test]
fn test_fm_clone_multi_fat_fields() {
    assert_aot_success(
        r#"
#derive(Eq, Clone)
type Person = { first: str, last: str }

@main () -> int = {
    let a = Person { first: "alice", last: "smith" };
    let b = a.clone();
    if a == b then {
        if b.first.length() + b.last.length() == 10 then 0 else 1
    } else 2
}
"#,
        "fm_clone_multi_fat_fields",
    );
}

// Clone used after original is consumed (RC independence)
#[test]
fn test_fm_clone_independence() {
    assert_aot_success(
        r#"
#derive(Clone)
type Named = { name: str, id: int }

@consume (n: Named) -> int = n.name.length();

@main () -> int = {
    let a = Named { name: "hello", id: 1 };
    let b = a.clone();
    let len_a = consume(n: a);
    let len_b = b.name.length();
    if len_a == 5 then {
        if len_b == 5 then 0 else 1
    } else 2
}
"#,
        "fm_clone_independence",
    );
}

// Clone with map field
#[test]
fn test_fm_clone_map_field() {
    assert_aot_success(
        r#"
#derive(Eq, Clone)
type Config = { settings: {str: int} }

@main () -> int = {
    let a = Config { settings: {"x": 1, "y": 2} };
    let b = a.clone();
    if a == b then {
        if b.settings.length() == 2 then 0 else 1
    } else 2
}
"#,
        "fm_clone_map_field",
    );
}
