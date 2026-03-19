//! F10: Generic Instantiation — fat pointer types used as generic type parameters.
//!
//! Tests monomorphization of generic functions when instantiated with fat pointer
//! types. The compiler must correctly specialize ABI, RC handling, and type layout
//! for each concrete instantiation.

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// T4: Generic identity with SSO string
#[test]
fn test_fm_generic_str_sso() {
    assert_aot_success(
        r#"
@identity<T> (x: T) -> T = x;

@main () -> int = {
    let s = identity(x: "hello");
    if s.length() == 5 then 0 else 1
}
"#,
        "fm_generic_str_sso",
    );
}

// T5: Generic identity with heap string
#[test]
fn test_fm_generic_str_heap() {
    assert_aot_success(
        r#"
@identity<T> (x: T) -> T = x;

@main () -> int = {
    let s = identity(x: "abcdefghijklmnopqrstuvwxyz1234");
    if s.length() == 30 then 0 else 1
}
"#,
        "fm_generic_str_heap",
    );
}

// T6: Generic with list of scalars
#[test]
fn test_fm_generic_list_scalar() {
    assert_aot_success(
        r#"
@identity<T> (x: T) -> T = x;

@main () -> int = {
    let xs = identity(x: [10, 20, 30]);
    if xs.length() == 3 then 0 else 1
}
"#,
        "fm_generic_list_scalar",
    );
}

// T7: Generic with list of fat pointers
#[test]
fn test_fm_generic_list_fat() {
    assert_aot_success(
        r#"
@identity<T> (x: T) -> T = x;

@main () -> int = {
    let xs = identity(x: ["hello", "world"]);
    if xs.length() == 2 then 0 else 1
}
"#,
        "fm_generic_list_fat",
    );
}

// T8: Generic with struct (scalar fields)
#[test]
fn test_fm_generic_struct_scalar() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@identity<T> (x: T) -> T = x;

@main () -> int = {
    let p = identity(x: Point { x: 10, y: 20 });
    if p.x + p.y == 30 then 0 else 1
}
"#,
        "fm_generic_struct_scalar",
    );
}

// T9: Generic with struct (fat fields)
#[test]
fn test_fm_generic_struct_fat() {
    assert_aot_success(
        r#"
type Named = { name: str, id: int }

@identity<T> (x: T) -> T = x;

@main () -> int = {
    let n = identity(x: Named { name: "alice", id: 1 });
    if n.name.length() + n.id == 6 then 0 else 1
}
"#,
        "fm_generic_struct_fat",
    );
}

// T15: Generic with Option<int>
#[test]
fn test_fm_generic_option_int() {
    assert_aot_success(
        r#"
@identity<T> (x: T) -> T = x;

@main () -> int = {
    let opt = identity(x: Some(42));
    if is_some(option: opt) then 0 else 1
}
"#,
        "fm_generic_option_int",
    );
}

// T17: Generic with map
#[test]
fn test_fm_generic_map() {
    assert_aot_success(
        r#"
@identity<T> (x: T) -> T = x;

@main () -> int = {
    let m = identity(x: {"a": 1, "b": 2});
    if m.length() == 2 then 0 else 1
}
"#,
        "fm_generic_map",
    );
}

// Generic function called with multiple fat types (monomorphized separately)
#[test]
fn test_fm_generic_multi_instantiation() {
    assert_aot_success(
        r#"
@identity<T> (x: T) -> T = x;

@main () -> int = {
    let s = identity(x: "hello");
    let xs = identity(x: [1, 2, 3]);
    if s.length() + xs.length() == 8 then 0 else 1
}
"#,
        "fm_generic_multi_instantiation",
    );
}

// Generic function with fat constraint (uses length)
#[test]
#[ignore = "BUG-04-01: generic indirect call leaks RC — monomorphized apply<T> with indirect f(x) inside"]
fn test_fm_generic_with_operation() {
    assert_aot_success(
        r#"
@get_len (xs: [int]) -> int = xs.length();

@apply<T> (f: (T) -> int, x: T) -> int = f(x);

@main () -> int = {
    let result = apply(f: get_len, x: [10, 20, 30]);
    if result == 3 then 0 else 1
}
"#,
        "fm_generic_with_operation",
    );
}
