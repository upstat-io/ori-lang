//! F17: Higher-Order — fat pointer types passed through function-typed parameters.
//!
//! Tests indirect call codegen, type erasure, and RC handling when fat pointer
//! values flow through higher-order function parameters.

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// Higher-order: function taking str -> int
#[test]
fn test_fm_higher_order_str_fn() {
    assert_aot_success(
        r#"
@apply_to_str (f: (str) -> int, s: str) -> int = f(s);

@get_len (s: str) -> int = s.length();

@main () -> int = {
    if apply_to_str(f: get_len, s: "hello") == 5 then 0 else 1
}
"#,
        "fm_higher_order_str_fn",
    );
}

// Higher-order: function taking [int] -> int
#[test]
fn test_fm_higher_order_list_fn() {
    assert_aot_success(
        r#"
@apply_to_list (f: ([int]) -> int, xs: [int]) -> int = f(xs);

@list_len (xs: [int]) -> int = xs.length();

@main () -> int = {
    if apply_to_list(f: list_len, xs: [10, 20, 30]) == 3 then 0 else 1
}
"#,
        "fm_higher_order_list_fn",
    );
}

// Higher-order: lambda with fat capture passed as argument
#[test]
fn test_fm_higher_order_lambda_fat_capture() {
    assert_aot_success(
        r#"
@apply (f: (int) -> int, x: int) -> int = f(x);

@main () -> int = {
    let prefix = "hello";
    let f = (n: int) -> int = prefix.length() + n;
    if apply(f: f, x: 5) == 10 then 0 else 1
}
"#,
        "fm_higher_order_lambda_fat_capture",
    );
}

// Higher-order: function returning int called twice (RC correctness)
#[test]
fn test_fm_higher_order_called_twice() {
    assert_aot_success(
        r#"
@apply (f: (str) -> int, s: str) -> int = f(s);

@get_len (s: str) -> int = s.length();

@main () -> int = {
    let s = "abcdefghijklmnopqrstuvwxyz1234";
    let a = apply(f: get_len, s: s);
    let b = apply(f: get_len, s: s);
    if a + b == 60 then 0 else 1
}
"#,
        "fm_higher_order_called_twice",
    );
}

// Higher-order: compose two functions on str
#[test]
fn test_fm_higher_order_compose() {
    assert_aot_success(
        r#"
@apply_then_add (f: (str) -> int, s: str, n: int) -> int = f(s) + n;

@get_len (s: str) -> int = s.length();

@main () -> int = {
    if apply_then_add(f: get_len, s: "hello", n: 5) == 10 then 0 else 1
}
"#,
        "fm_higher_order_compose",
    );
}

// Higher-order: function operating on struct with fat field
#[test]
fn test_fm_higher_order_struct_fat() {
    assert_aot_success(
        r#"
type Named = { name: str, id: int }

@apply_to_named (f: (Named) -> int, n: Named) -> int = f(n);

@name_len (n: Named) -> int = n.name.length();

@main () -> int = {
    let n = Named { name: "alice", id: 1 };
    if apply_to_named(f: name_len, n: n) == 5 then 0 else 1
}
"#,
        "fm_higher_order_struct_fat",
    );
}

// Higher-order: function operating on map
#[test]
fn test_fm_higher_order_map() {
    assert_aot_success(
        r#"
@apply_to_map (f: ({str: int}) -> int, m: {str: int}) -> int = f(m);

@map_size (m: {str: int}) -> int = m.length();

@main () -> int = {
    let m = {"a": 1, "b": 2};
    if apply_to_map(f: map_size, m: m) == 2 then 0 else 1
}
"#,
        "fm_higher_order_map",
    );
}

// Higher-order: two different functions on same fat type
#[test]
fn test_fm_higher_order_different_fns() {
    assert_aot_success(
        r#"
@apply (f: (str) -> int, s: str) -> int = f(s);

@get_len (s: str) -> int = s.length();
@first_byte (s: str) -> int = if s.length() > 0 then 1 else 0;

@main () -> int = {
    let s = "hello";
    let a = apply(f: get_len, s: s);
    let b = apply(f: first_byte, s: s);
    if a == 5 then {
        if b == 1 then 0 else 1
    } else 2
}
"#,
        "fm_higher_order_different_fns",
    );
}
