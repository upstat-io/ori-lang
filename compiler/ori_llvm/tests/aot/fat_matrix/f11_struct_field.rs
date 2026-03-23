//! F11: Struct Field — fat pointer types stored in struct fields.
//!
//! Tests GEP-based field access, aggregate construction, and RC handling
//! when fat pointer values are stored in and extracted from struct fields.

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// T4/T5: Struct with str field
#[test]
fn test_fm_struct_field_str() {
    assert_aot_success(
        r#"
type Wrapper = { value: str }

@main () -> int = {
    let w = Wrapper { value: "hello" };
    if w.value.length() == 5 then 0 else 1
}
"#,
        "fm_struct_field_str",
    );
}

// T5: Struct with heap str field
#[test]
fn test_fm_struct_field_str_heap() {
    assert_aot_success(
        r#"
type Wrapper = { value: str }

@main () -> int = {
    let w = Wrapper { value: "abcdefghijklmnopqrstuvwxyz1234" };
    if w.value.length() == 30 then 0 else 1
}
"#,
        "fm_struct_field_str_heap",
    );
}

// T6: Struct with list of scalars field
#[test]
fn test_fm_struct_field_list_scalar() {
    assert_aot_success(
        r#"
type Container = { items: [int] }

@main () -> int = {
    let c = Container { items: [10, 20, 30] };
    if c.items.length() == 3 then 0 else 1
}
"#,
        "fm_struct_field_list_scalar",
    );
}

// T7: Struct with list of fat pointers field
#[test]
fn test_fm_struct_field_list_fat() {
    assert_aot_success(
        r#"
type Container = { items: [str] }

@main () -> int = {
    let c = Container { items: ["hello", "world"] };
    if c.items.length() == 2 then 0 else 1
}
"#,
        "fm_struct_field_list_fat",
    );
}

// T9: Nested struct with fat fields
#[test]
fn test_fm_struct_field_nested_fat() {
    assert_aot_success(
        r#"
type Inner = { name: str }
type Outer = { inner: Inner, count: int }

@main () -> int = {
    let o = Outer { inner: Inner { name: "alice" }, count: 5 };
    if o.inner.name.length() + o.count == 10 then 0 else 1
}
"#,
        "fm_struct_field_nested_fat",
    );
}

// Multiple fat fields in same struct
#[test]
fn test_fm_struct_field_multi_fat() {
    assert_aot_success(
        r#"
type Person = { first: str, last: str, age: int }

@main () -> int = {
    let p = Person { first: "alice", last: "smith", age: 30 };
    if p.first.length() + p.last.length() + p.age == 40 then 0 else 1
}
"#,
        "fm_struct_field_multi_fat",
    );
}

// Struct field passed to function
#[test]
fn test_fm_struct_field_passed() {
    assert_aot_success(
        r#"
type Named = { name: str, id: int }

@get_name_len (s: str) -> int = s.length();

@main () -> int = {
    let n = Named { name: "hello", id: 1 };
    if get_name_len(s: n.name) == 5 then 0 else 1
}
"#,
        "fm_struct_field_passed",
    );
}

// Struct with map field
#[test]
fn test_fm_struct_field_map() {
    assert_aot_success(
        r#"
type Config = { settings: {str: int}, name: str }

@main () -> int = {
    let c = Config { settings: {"a": 1}, name: "cfg" };
    if c.settings.length() + c.name.length() == 4 then 0 else 1
}
"#,
        "fm_struct_field_map",
    );
}
