//! Tuple AOT Tests
//!
//! Tests for tuple construction, field access, destructuring, nested tuples,
//! tuples as function parameters/returns, tuples in collections, and
//! tuple interaction with other language features.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Construction ───

#[test]
fn test_tuple_pair_int() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (10, 20);
    if t.0 == 10 && t.1 == 20 then 0 else 1
}
"#,
        "tuple_pair_int",
    );
}

#[test]
fn test_tuple_triple_int() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (1, 2, 3);
    if t.0 + t.1 + t.2 == 6 then 0 else 1
}
"#,
        "tuple_triple_int",
    );
}

#[test]
fn test_tuple_mixed_types() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (42, true, "hello");
    if t.0 == 42 && t.1 then 0 else 1
}
"#,
        "tuple_mixed_types",
    );
}

#[test]
fn test_tuple_single_element() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (42,);
    if t.0 == 42 then 0 else 1
}
"#,
        "tuple_single",
    );
}

#[test]
fn test_tuple_bool_pair() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (true, false);
    if t.0 && !t.1 then 0 else 1
}
"#,
        "tuple_bool_pair",
    );
}

// ─── Destructuring ───

#[test]
fn test_tuple_destructure_pair() {
    assert_aot_success(
        r#"
@main () -> int = {
    let (a, b) = (10, 20);
    if a == 10 && b == 20 then 0 else 1
}
"#,
        "tuple_destr_pair",
    );
}

#[test]
fn test_tuple_destructure_triple() {
    assert_aot_success(
        r#"
@main () -> int = {
    let (a, b, c) = (1, 2, 3);
    if a + b + c == 6 then 0 else 1
}
"#,
        "tuple_destr_triple",
    );
}

#[test]
fn test_tuple_destructure_mixed() {
    assert_aot_success(
        r#"
@main () -> int = {
    let (n, flag) = (42, true);
    if n == 42 && flag then 0 else 1
}
"#,
        "tuple_destr_mixed",
    );
}

#[test]
fn test_tuple_destructure_from_variable() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (100, 200);
    let (a, b) = t;
    if a == 100 && b == 200 then 0 else 1
}
"#,
        "tuple_destr_from_var",
    );
}

// ─── Field access ───

#[test]
fn test_tuple_field_access_4() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (10, 20, 30, 40);
    if t.0 == 10 && t.3 == 40 then 0 else 1
}
"#,
        "tuple_field_4",
    );
}

#[test]
fn test_tuple_field_in_expression() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (3, 4);
    let sum_sq = t.0 * t.0 + t.1 * t.1;
    // 9 + 16 = 25
    if sum_sq == 25 then 0 else 1
}
"#,
        "tuple_field_expr",
    );
}

#[test]
fn test_tuple_field_as_function_arg() {
    assert_aot_success(
        r#"
@add (a: int, b: int) -> int = a + b;

@main () -> int = {
    let t = (10, 20);
    if add(a: t.0, b: t.1) == 30 then 0 else 1
}
"#,
        "tuple_field_fn_arg",
    );
}

// ─── As function parameter/return ───

#[test]
fn test_tuple_as_param() {
    assert_aot_success(
        r#"
@sum_pair (p: (int, int)) -> int = p.0 + p.1;

@main () -> int = {
    let t = (15, 27);
    if sum_pair(p: t) == 42 then 0 else 1
}
"#,
        "tuple_as_param",
    );
}

#[test]
fn test_tuple_as_return() {
    assert_aot_success(
        r#"
@make_pair (a: int, b: int) -> (int, int) = (a, b);

@main () -> int = {
    let (x, y) = make_pair(a: 3, b: 7);
    if x == 3 && y == 7 then 0 else 1
}
"#,
        "tuple_as_return",
    );
}

#[test]
fn test_tuple_return_and_access() {
    assert_aot_success(
        r#"
@min_max (a: int, b: int) -> (int, int) = {
    if a < b then (a, b) else (b, a)
};

@main () -> int = {
    let result = min_max(a: 7, b: 3);
    if result.0 == 3 && result.1 == 7 then 0 else 1
}
"#,
        "tuple_return_access",
    );
}

#[test]
fn test_tuple_return_triple() {
    assert_aot_success(
        r#"
@stats (a: int, b: int, c: int) -> (int, int, int) = {
    let min = if a < b then { if a < c then a else c } else { if b < c then b else c };
    let max = if a > b then { if a > c then a else c } else { if b > c then b else c };
    let sum = a + b + c;
    (min, max, sum)
};

@main () -> int = {
    let (mn, mx, s) = stats(a: 5, b: 2, c: 8);
    if mn == 2 && mx == 8 && s == 15 then 0 else 1
}
"#,
        "tuple_return_triple",
    );
}

// ─── Nested tuples ───

#[test]
#[ignore = "Parser gap: chained tuple field access t.0.0 lexed as float (Section 05 § 5.7 open item)"]
fn test_tuple_nested_pair_of_pairs() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = ((1, 2), (3, 4));
    if t.0.0 == 1 && t.0.1 == 2 && t.1.0 == 3 && t.1.1 == 4 then 0 else 1
}
"#,
        "tuple_nested_pairs",
    );
}

#[test]
fn test_tuple_nested_destructure() {
    assert_aot_success(
        r#"
@main () -> int = {
    let ((a, b), (c, d)) = ((10, 20), (30, 40));
    if a + b + c + d == 100 then 0 else 1
}
"#,
        "tuple_nested_destr",
    );
}

#[test]
#[ignore = "Parser gap: chained tuple field access t.1.0 lexed as float (Section 05 § 5.7 open item)"]
fn test_tuple_nested_mixed() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (1, (2, 3));
    if t.0 == 1 && t.1.0 == 2 && t.1.1 == 3 then 0 else 1
}
"#,
        "tuple_nested_mixed",
    );
}

// ─── Tuples with strings ───

#[test]
fn test_tuple_string_field() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = ("hello", 42);
    if t.1 == 42 then 0 else 1
}
"#,
        "tuple_str_field",
    );
}

#[test]
fn test_tuple_two_strings() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = ("hello", "world");
    if t.0 == "hello" && t.1 == "world" then 0 else 1
}
"#,
        "tuple_two_strs",
    );
}

// ─── Tuples in control flow ───

#[test]
fn test_tuple_from_if_expression() {
    assert_aot_success(
        r#"
@main () -> int = {
    let flag = true;
    let t = if flag then (1, 2) else (3, 4);
    if t.0 == 1 && t.1 == 2 then 0 else 1
}
"#,
        "tuple_from_if",
    );
}

#[test]
#[ignore = "AOT gap: iterating list of tuples causes heap corruption (malloc assertion failure)"]
fn test_tuple_in_loop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let sum = 0;
    for t in [(1, 10), (2, 20), (3, 30)] do {
        sum = sum + t.0 + t.1;
    };
    // (1+10) + (2+20) + (3+30) = 66
    if sum == 66 then 0 else 1
}
"#,
        "tuple_in_loop",
    );
}

#[test]
#[ignore = "Parser regression: for (a, b) in ... rejected with E1013 'for pattern requires named properties' (Section 00)"]
fn test_tuple_destructure_in_loop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let sum = 0;
    for (a, b) in [(1, 10), (2, 20), (3, 30)] do {
        sum = sum + a + b;
    };
    if sum == 66 then 0 else 1
}
"#,
        "tuple_destr_loop",
    );
}

// ─── Tuple with closures ───

#[test]
fn test_tuple_closure_capture() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (10, 20);
    let sum_it = (extra: int) -> t.0 + t.1 + extra;
    if sum_it(12) == 42 then 0 else 1
}
"#,
        "tuple_closure_capture",
    );
}

#[test]
fn test_tuple_returned_from_closure() {
    assert_aot_success(
        r#"
@main () -> int = {
    let make = (x: int) -> (x, x * 2);
    let (a, b) = make(5);
    if a == 5 && b == 10 then 0 else 1
}
"#,
        "tuple_from_closure",
    );
}

// ─── Tuple comparison ───

#[test]
#[ignore = "AOT gap: tuple == compiles but returns wrong result (comparison codegen incorrect)"]
fn test_tuple_equality() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = (1, 2);
    let b = (1, 2);
    let c = (1, 3);
    if a == b && a != c then 0 else 1
}
"#,
        "tuple_equality",
    );
}

#[test]
#[ignore = "AOT gap: tuple == compiles but returns wrong result (comparison codegen incorrect)"]
fn test_tuple_equality_triple() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = (1, 2, 3);
    let b = (1, 2, 3);
    let c = (1, 2, 4);
    if a == b && a != c then 0 else 1
}
"#,
        "tuple_eq_triple",
    );
}
