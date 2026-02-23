//! Pattern Matching Extension AOT Tests
//!
//! Tests for advanced pattern matching features beyond basic literal/wildcard:
//! or-patterns, guard clauses, tuple patterns, struct patterns, and binding
//! patterns in match expressions.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Or-patterns in match ───

#[test]
fn test_pattern_or_int_literals() {
    assert_aot_success(
        r#"
@classify (n: int) -> int = match n {
    1 | 2 | 3 -> 10,
    4 | 5 | 6 -> 20,
    7 | 8 | 9 -> 30,
    _ -> 0,
};

@main () -> int = {
    let a = classify(n: 2);
    let b = classify(n: 5);
    let c = classify(n: 8);
    let d = classify(n: 100);
    if a == 10 && b == 20 && c == 30 && d == 0 then 0 else 1
}
"#,
        "pattern_or_int",
    );
}

#[test]
fn test_pattern_or_char_literals() {
    assert_aot_success(
        r#"
@is_vowel (c: char) -> bool = match c {
    'a' | 'e' | 'i' | 'o' | 'u' -> true,
    _ -> false,
};

@main () -> int = {
    let ok1 = is_vowel(c: 'a');
    let ok2 = is_vowel(c: 'e');
    let ok3 = !is_vowel(c: 'x');
    let ok4 = !is_vowel(c: 'z');
    if ok1 && ok2 && ok3 && ok4 then 0 else 1
}
"#,
        "pattern_or_char",
    );
}

#[test]
fn test_pattern_or_bool() {
    assert_aot_success(
        r#"
@main () -> int = {
    // Trivial or-pattern on bool (always matches first)
    let x = match true {
        true | false -> 42,
    };
    if x == 42 then 0 else 1
}
"#,
        "pattern_or_bool",
    );
}

#[test]
fn test_pattern_or_in_loop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    for i in 0..20 do {
        let x = match i % 4 {
            0 | 2 -> 1,
            _ -> 0,
        };
        count = count + x
    };
    // i%4 == 0 or 2: at i=0,2,4,6,8,10,12,14,16,18 → 10 values
    if count == 10 then 0 else 1
}
"#,
        "pattern_or_in_loop",
    );
}

// ─── Guard clauses ───

#[test]
fn test_pattern_guard_basic() {
    assert_aot_success(
        r#"
@classify (n: int) -> int = match n {
    x if x > 100 -> 3,
    x if x > 10 -> 2,
    x if x > 0 -> 1,
    _ -> 0,
};

@main () -> int = {
    let a = classify(n: 200);
    let b = classify(n: 50);
    let c = classify(n: 5);
    let d = classify(n: -1);
    if a == 3 && b == 2 && c == 1 && d == 0 then 0 else 1
}
"#,
        "pattern_guard_basic",
    );
}

#[test]
fn test_pattern_guard_with_binding() {
    assert_aot_success(
        r#"
@abs_classify (n: int) -> int = match n {
    x if x > 0 -> x,
    x if x < 0 -> -x,
    _ -> 0,
};

@main () -> int = {
    let a = abs_classify(n: 5);
    let b = abs_classify(n: -5);
    let c = abs_classify(n: 0);
    if a == 5 && b == 5 && c == 0 then 0 else 1
}
"#,
        "pattern_guard_binding",
    );
}

#[test]
fn test_pattern_guard_complex_condition() {
    assert_aot_success(
        r#"
@in_range (n: int) -> bool = match n {
    x if x >= 10 && x <= 20 -> true,
    _ -> false,
};

@main () -> int = {
    let ok1 = in_range(n: 15);
    let ok2 = !in_range(n: 5);
    let ok3 = in_range(n: 10);
    let ok4 = in_range(n: 20);
    let ok5 = !in_range(n: 21);
    if ok1 && ok2 && ok3 && ok4 && ok5 then 0 else 1
}
"#,
        "pattern_guard_complex",
    );
}

#[test]
fn test_pattern_guard_in_loop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let sum = 0;
    for i in 0..30 do {
        let contribution = match i {
            x if x % 15 == 0 -> 100,
            x if x % 5 == 0 -> 50,
            x if x % 3 == 0 -> 30,
            _ -> 1,
        };
        sum = sum + contribution
    };
    // FizzBuzz-like: 0,15 → 100; 5,10,20,25 → 50; 3,6,9,12,18,21,24,27 → 30
    // rest (1,2,4,7,8,11,13,14,16,17,19,22,23,26,28,29) → 1
    // 2*100 + 4*50 + 8*30 + 16*1 = 200 + 200 + 240 + 16 = 656
    if sum == 656 then 0 else 1
}
"#,
        "pattern_guard_loop",
    );
}

// ─── Tuple patterns in match ───

#[test]
fn test_pattern_tuple_basic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (1, 2);
    let r = match t {
        (1, y) -> y,
        (x, 2) -> x * 10,
        _ -> 0,
    };
    // First arm matches: y = 2
    if r == 2 then 0 else 1
}
"#,
        "pattern_tuple_basic",
    );
}

#[test]
fn test_pattern_tuple_second_arm() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (5, 2);
    let r = match t {
        (1, y) -> y,
        (x, 2) -> x * 10,
        _ -> 0,
    };
    // Second arm matches: x = 5, result = 50
    if r == 50 then 0 else 1
}
"#,
        "pattern_tuple_second",
    );
}

#[test]
fn test_pattern_tuple_wildcard_fallthrough() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (99, 88);
    let r = match t {
        (1, _) -> 10,
        (_, 1) -> 20,
        _ -> 0,
    };
    // Neither matches, wildcard fallthrough: 0
    if r == 0 then 0 else 1
}
"#,
        "pattern_tuple_wildcard",
    );
}

#[test]
fn test_pattern_tuple_3_elements() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (1, 2, 3);
    let r = match t {
        (1, 2, z) -> z * 100,
        _ -> 0,
    };
    if r == 300 then 0 else 1
}
"#,
        "pattern_tuple_3elem",
    );
}

#[test]
fn test_pattern_tuple_all_wildcards() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (42, 99);
    let r = match t {
        (_, _) -> 0,
    };
    r
}
"#,
        "pattern_tuple_all_wild",
    );
}

#[test]
fn test_pattern_tuple_from_function() {
    assert_aot_success(
        r#"
@make_pair (x: int) -> (int, bool) = (x, x > 0);

@classify (p: (int, bool)) -> int = match p {
    (_, true) -> 1,
    (_, false) -> 0,
};

@main () -> int = {
    let a = classify(p: make_pair(x: 5));
    let b = classify(p: make_pair(x: -1));
    if a == 1 && b == 0 then 0 else 1
}
"#,
        "pattern_tuple_from_fn",
    );
}

// ─── Binding patterns ───

#[test]
fn test_pattern_binding_capture() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 42;
    let r = match x {
        n -> n * 2,
    };
    if r == 84 then 0 else 1
}
"#,
        "pattern_binding_capture",
    );
}

#[test]
fn test_pattern_binding_with_literal_arms() {
    assert_aot_success(
        r#"
@describe (n: int) -> int = match n {
    0 -> 0,
    1 -> 1,
    x -> x + 100,
};

@main () -> int = {
    let a = describe(n: 0);
    let b = describe(n: 1);
    let c = describe(n: 5);
    if a == 0 && b == 1 && c == 105 then 0 else 1
}
"#,
        "pattern_binding_mixed",
    );
}

// ─── Combined patterns ───

#[test]
fn test_pattern_guard_with_tuple() {
    assert_aot_success(
        r#"
@main () -> int = {
    let p = (10, 20);
    let r = match p {
        (x, y) if x + y > 25 -> 1,
        (x, y) if x + y > 15 -> 2,
        _ -> 0,
    };
    // 10 + 20 = 30 > 25: first arm
    if r == 1 then 0 else 1
}
"#,
        "pattern_guard_tuple",
    );
}

#[test]
fn test_pattern_match_on_result_tag() {
    assert_aot_success(
        r#"
@safe_div (a: int, b: int) -> Result<int, str> =
    if b == 0 then Err("divide by zero") else Ok(a / b);

@main () -> int = {
    let r1 = safe_div(a: 10, b: 2);
    let r2 = safe_div(a: 10, b: 0);
    let v1 = match r1.is_ok() {
        true -> r1.unwrap(),
        false -> -1,
    };
    let v2 = match r2.is_err() {
        true -> 0,
        false -> -1,
    };
    if v1 == 5 && v2 == 0 then 0 else 1
}
"#,
        "pattern_result_dispatch",
    );
}

#[test]
fn test_pattern_nested_match() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 3;
    let y = 7;
    let r = match x {
        1 -> match y {
            1 -> 11,
            _ -> 10,
        },
        3 -> match y {
            7 -> 37,
            _ -> 30,
        },
        _ -> 0,
    };
    if r == 37 then 0 else 1
}
"#,
        "pattern_nested_match",
    );
}

#[test]
fn test_pattern_match_expression_in_function() {
    assert_aot_success(
        r#"
@fizzbuzz (n: int) -> int = match n {
    x if x % 15 == 0 -> 3,
    x if x % 5 == 0 -> 2,
    x if x % 3 == 0 -> 1,
    _ -> 0,
};

@main () -> int = {
    let fb15 = fizzbuzz(n: 15);
    let fb5 = fizzbuzz(n: 10);
    let fb3 = fizzbuzz(n: 9);
    let fb1 = fizzbuzz(n: 7);
    if fb15 == 3 && fb5 == 2 && fb3 == 1 && fb1 == 0 then 0 else 1
}
"#,
        "pattern_fizzbuzz",
    );
}

// ─── Match exhaustiveness ───

#[test]
fn test_pattern_match_all_bool_cases() {
    assert_aot_success(
        r#"
@to_int (b: bool) -> int = match b {
    true -> 1,
    false -> 0,
};

@main () -> int = {
    if to_int(b: true) == 1 && to_int(b: false) == 0 then 0 else 1
}
"#,
        "pattern_exhaust_bool",
    );
}

#[test]
fn test_pattern_match_many_char_literals() {
    assert_aot_success(
        r#"
@char_val (c: char) -> int = match c {
    'a' -> 1,
    'b' -> 2,
    'c' -> 3,
    'd' -> 4,
    'e' -> 5,
    'f' -> 6,
    'g' -> 7,
    'h' -> 8,
    'i' -> 9,
    'j' -> 10,
    _ -> 0,
};

@main () -> int = {
    let sum = char_val(c: 'a') + char_val(c: 'e') + char_val(c: 'j') + char_val(c: 'z');
    // 1 + 5 + 10 + 0 = 16
    if sum == 16 then 0 else 1
}
"#,
        "pattern_many_chars",
    );
}
