//! Variable Mutation & Reassignment AOT Tests
//!
//! Tests for variable reassignment semantics in various contexts:
//! simple reassignment, loops, match arms, conditionals, and
//! accumulator patterns.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Simple reassignment ───

#[test]
fn test_mut_simple_reassign() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 10;
    x = 20;
    if x == 20 then 0 else 1
}
"#,
        "mut_simple",
    );
}

#[test]
fn test_mut_reassign_multiple() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 1;
    x = 2;
    x = 3;
    x = 4;
    if x == 4 then 0 else 1
}
"#,
        "mut_multiple",
    );
}

#[test]
fn test_mut_reassign_self_reference() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 10;
    x = x + 5;
    x = x * 2;
    // 10 -> 15 -> 30
    if x == 30 then 0 else 1
}
"#,
        "mut_self_ref",
    );
}

// ─── Loop accumulator patterns ───

#[test]
fn test_mut_loop_counter() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    for i in 0..10 do {
        count = count + 1;
    };
    if count == 10 then 0 else 1
}
"#,
        "mut_loop_counter",
    );
}

#[test]
fn test_mut_loop_accumulator() {
    assert_aot_success(
        r#"
@main () -> int = {
    let sum = 0;
    for i in 1..=5 do {
        sum = sum + i;
    };
    // 1+2+3+4+5 = 15
    if sum == 15 then 0 else 1
}
"#,
        "mut_loop_accum",
    );
}

#[test]
fn test_mut_loop_product() {
    assert_aot_success(
        r#"
@main () -> int = {
    let product = 1;
    for i in 1..=5 do {
        product = product * i;
    };
    // 1*2*3*4*5 = 120
    if product == 120 then 0 else 1
}
"#,
        "mut_loop_product",
    );
}

#[test]
fn test_mut_loop_conditional_accumulator() {
    assert_aot_success(
        r#"
@main () -> int = {
    let sum = 0;
    for i in 0..10 do {
        if i % 2 == 0 then {
            sum = sum + i;
        }
    };
    // 0+2+4+6+8 = 20
    if sum == 20 then 0 else 1
}
"#,
        "mut_loop_cond_accum",
    );
}

// ─── Loop with break ───

#[test]
fn test_mut_loop_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    loop {
        count = count + 1;
        if count >= 5 then break
    };
    if count == 5 then 0 else 1
}
"#,
        "mut_loop_break",
    );
}

#[test]
fn test_mut_while_pattern() {
    assert_aot_success(
        r#"
@main () -> int = {
    let n = 100;
    let steps = 0;
    loop {
        if n <= 1 then break;
        n = if n % 2 == 0 then n / 2 else n * 3 + 1;
        steps = steps + 1;
    };
    // Collatz sequence for 100: should terminate
    if steps > 0 then 0 else 1
}
"#,
        "mut_while_pattern",
    );
}

// ─── Reassignment in conditionals ───

#[test]
fn test_mut_if_reassign() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 10;
    if x > 5 then {
        x = x * 2;
    };
    if x == 20 then 0 else 1
}
"#,
        "mut_if_reassign",
    );
}

#[test]
fn test_mut_if_else_reassign() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 0;
    let flag = true;
    if flag then {
        x = 100;
    } else {
        x = 200;
    };
    if x == 100 then 0 else 1
}
"#,
        "mut_if_else_reassign",
    );
}

// ─── Reassignment with different types ───

#[test]
fn test_mut_reassign_string() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello";
    s = s + " world";
    if s == "hello world" then 0 else 1
}
"#,
        "mut_reassign_string",
    );
}

#[test]
fn test_mut_reassign_bool() {
    assert_aot_success(
        r#"
@main () -> int = {
    let found = false;
    for i in 0..10 do {
        if i == 7 then {
            found = true;
        }
    };
    if found then 0 else 1
}
"#,
        "mut_reassign_bool",
    );
}

// ─── Multiple variables ───

#[test]
fn test_mut_swap_values() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 10;
    let b = 20;
    let temp = a;
    a = b;
    b = temp;
    if a == 20 && b == 10 then 0 else 1
}
"#,
        "mut_swap",
    );
}

#[test]
fn test_mut_fibonacci() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 0;
    let b = 1;
    for i in 0..10 do {
        let next = a + b;
        a = b;
        b = next;
    };
    // fib(10) = 55
    if a == 55 then 0 else 1
}
"#,
        "mut_fibonacci",
    );
}

#[test]
fn test_mut_min_max_tracking() {
    assert_aot_success(
        r#"
@main () -> int = {
    let min = 999;
    let max = -999;
    for x in [3, 7, 1, 9, 2, 8, 4] do {
        if x < min then min = x;
        if x > max then max = x;
    };
    if min == 1 && max == 9 then 0 else 1
}
"#,
        "mut_min_max",
    );
}

// ─── Nested loops with mutation ───

#[test]
fn test_mut_nested_loop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let total = 0;
    for i in 0..3 do {
        for j in 0..3 do {
            total = total + 1;
        };
    };
    if total == 9 then 0 else 1
}
"#,
        "mut_nested_loop",
    );
}

#[test]
fn test_mut_outer_from_inner() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = 0;
    for i in 1..=3 do {
        for j in 1..=3 do {
            if i == j then {
                result = result + i;
            }
        };
    };
    // diagonal: 1+2+3 = 6
    if result == 6 then 0 else 1
}
"#,
        "mut_outer_from_inner",
    );
}

// ─── String building ───

#[test]
fn test_mut_string_builder() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "";
    for i in 0..5 do {
        s = s + i.to_str();
    };
    if s == "01234" then 0 else 1
}
"#,
        "mut_string_builder",
    );
}

// ─── Reassignment with function calls ───

#[test]
fn test_mut_function_result() {
    assert_aot_success(
        r#"
@double (x: int) -> int = x * 2;

@main () -> int = {
    let x = 5;
    x = double(x: x);
    x = double(x: x);
    // 5 -> 10 -> 20
    if x == 20 then 0 else 1
}
"#,
        "mut_function_result",
    );
}

#[test]
fn test_mut_accumulate_function() {
    assert_aot_success(
        r#"
@square (n: int) -> int = n * n;

@main () -> int = {
    let sum = 0;
    for i in 1..=4 do {
        sum = sum + square(n: i);
    };
    // 1+4+9+16 = 30
    if sum == 30 then 0 else 1
}
"#,
        "mut_accum_func",
    );
}
