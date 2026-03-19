//! F13: Derived Eq — equality comparison on types containing fat pointer fields.
//!
//! Tests the `$eq` derived method codegen for structs and sum types whose fields
//! include strings, lists, and other fat pointer types.

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// T4/T5: Eq on struct with str field
#[test]
fn test_fm_eq_struct_str() {
    assert_aot_success(
        r#"
#derive(Eq)
type Named = { name: str, id: int }

@main () -> int = {
    let a = Named { name: "alice", id: 1 };
    let b = Named { name: "alice", id: 1 };
    let c = Named { name: "bob", id: 1 };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_struct_str",
    );
}

// T6: Eq on struct with list of scalars
#[test]
fn test_fm_eq_struct_list_scalar() {
    assert_aot_success(
        r#"
#derive(Eq)
type Container = { items: [int] }

@main () -> int = {
    let a = Container { items: [1, 2, 3] };
    let b = Container { items: [1, 2, 3] };
    let c = Container { items: [1, 2, 4] };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_struct_list_scalar",
    );
}

// T7: Eq on struct with list of strings
#[test]
fn test_fm_eq_struct_list_fat() {
    assert_aot_success(
        r#"
#derive(Eq)
type Words = { items: [str] }

@main () -> int = {
    let a = Words { items: ["hello", "world"] };
    let b = Words { items: ["hello", "world"] };
    let c = Words { items: ["hello", "other"] };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_struct_list_fat",
    );
}

// T9: Eq on struct with nested fat fields
#[test]
fn test_fm_eq_nested_fat() {
    assert_aot_success(
        r#"
#derive(Eq)
type Inner = { name: str }
#derive(Eq)
type Outer = { inner: Inner, count: int }

@main () -> int = {
    let a = Outer { inner: Inner { name: "alice" }, count: 5 };
    let b = Outer { inner: Inner { name: "alice" }, count: 5 };
    let c = Outer { inner: Inner { name: "bob" }, count: 5 };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_nested_fat",
    );
}

// Sum type: Eq on Option<str>
#[test]
#[ignore = "BUG-04-03: Option<str> == uses ARC IR primop icmp on aggregate — needs runtime comparison"]
fn test_fm_eq_option_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = Some("hello");
    let b = Some("hello");
    let c = Some("world");
    let d: Option<str> = None;
    if a == b then {
        if a != c then {
            if a != d then 0 else 1
        } else 2
    } else 3
}
"#,
        "fm_eq_option_str",
    );
}

// Direct str comparison
#[test]
fn test_fm_eq_str_direct() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "hello";
    let b = "hello";
    let c = "world";
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_str_direct",
    );
}

// Eq on struct with multiple fat fields
#[test]
fn test_fm_eq_multi_fat_fields() {
    assert_aot_success(
        r#"
#derive(Eq)
type Person = { first: str, last: str }

@main () -> int = {
    let a = Person { first: "alice", last: "smith" };
    let b = Person { first: "alice", last: "smith" };
    let c = Person { first: "alice", last: "jones" };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_multi_fat_fields",
    );
}

// Eq with heap strings (>23 bytes)
#[test]
fn test_fm_eq_heap_str() {
    assert_aot_success(
        r#"
#derive(Eq)
type Doc = { content: str }

@main () -> int = {
    let a = Doc { content: "abcdefghijklmnopqrstuvwxyz1234" };
    let b = Doc { content: "abcdefghijklmnopqrstuvwxyz1234" };
    let c = Doc { content: "abcdefghijklmnopqrstuvwxyz9999" };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_heap_str",
    );
}
