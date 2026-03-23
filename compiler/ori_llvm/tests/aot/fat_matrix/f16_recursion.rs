//! F16: Recursion — fat pointer types passed through recursive function calls.
//!
//! Tests stack frame RC handling, ensuring that fat pointer values are correctly
//! incremented/decremented across recursive call boundaries.

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// Recursive countdown with str in scope
#[test]
fn test_fm_recursion_str_in_scope() {
    assert_aot_success(
        r#"
@countdown (n: int) -> int = {
    let label = "counting";
    if n <= 0 then label.length()
    else countdown(n: n - 1)
};

@main () -> int = {
    if countdown(n: 5) == 8 then 0 else 1
}
"#,
        "fm_recursion_str_in_scope",
    );
}

// Recursive function with str parameter
#[test]
fn test_fm_recursion_str_param() {
    assert_aot_success(
        r#"
@count_down (s: str, n: int) -> int = {
    if n <= 0 then s.length()
    else count_down(s: s, n: n - 1)
};

@main () -> int = {
    if count_down(s: "hello", n: 3) == 5 then 0 else 1
}
"#,
        "fm_recursion_str_param",
    );
}

// Recursive function with list parameter
#[test]
fn test_fm_recursion_list_param() {
    assert_aot_success(
        r#"
@sum_recursive (xs: [int], idx: int) -> int = {
    if idx >= xs.length() then 0
    else xs[idx] + sum_recursive(xs: xs, idx: idx + 1)
};

@main () -> int = {
    if sum_recursive(xs: [10, 20, 30], idx: 0) == 60 then 0 else 1
}
"#,
        "fm_recursion_list_param",
    );
}

// Recursive function returning struct with fat field
#[test]
fn test_fm_recursion_struct_fat_return() {
    assert_aot_success(
        r#"
type Named = { name: str, count: int }

@build (n: int) -> Named = {
    if n <= 0 then Named { name: "done", count: 0 }
    else {
        let inner = build(n: n - 1);
        Named { name: inner.name, count: inner.count + 1 }
    }
};

@main () -> int = {
    let result = build(n: 3);
    if result.count == 3 then {
        if result.name.length() == 4 then 0 else 1
    } else 2
}
"#,
        "fm_recursion_struct_fat_return",
    );
}

// Recursive function with Option<int> return
#[test]
fn test_fm_recursion_option_return() {
    assert_aot_success(
        r#"
@find_positive (xs: [int], idx: int) -> Option<int> = {
    if idx >= xs.length() then None
    else if xs[idx] > 0 then Some(xs[idx])
    else find_positive(xs: xs, idx: idx + 1)
};

@main () -> int = {
    match find_positive(xs: [-1, -2, 5, -3], idx: 0) {
        Some(v) -> if v == 5 then 0 else 1,
        None -> 2,
    }
}
"#,
        "fm_recursion_option_return",
    );
}

// Mutual recursion with fat values
#[test]
fn test_fm_recursion_mutual_fat() {
    assert_aot_success(
        r#"
@is_even (n: int) -> int = {
    let tag = "even";
    if n == 0 then tag.length()
    else is_odd(n: n - 1)
};

@is_odd (n: int) -> int = {
    let tag = "odd";
    if n == 0 then tag.length()
    else is_even(n: n - 1)
};

@main () -> int = {
    let r = is_even(n: 4);
    if r == 4 then 0 else 1
}
"#,
        "fm_recursion_mutual_fat",
    );
}
