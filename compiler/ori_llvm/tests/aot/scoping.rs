//! Variable Scoping & Block Expression AOT Tests
//!
//! Tests for let bindings, variable shadowing, block expressions as values,
//! nested scopes, if/match as expression in value position, and scope
//! interaction with control flow.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Basic let bindings ───

#[test]
fn test_scope_let_basic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 42;
    if x == 42 then 0 else 1
}
"#,
        "scope_let_basic",
    );
}

#[test]
fn test_scope_let_type_annotation() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x: int = 42;
    let f: float = 3.14;
    let b: bool = true;
    if x == 42 && f > 3.0 && b then 0 else 1
}
"#,
        "scope_let_type_ann",
    );
}

#[test]
fn test_scope_let_chain() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 1;
    let b = a + 1;
    let c = b + 1;
    let d = c + 1;
    let e = d + 1;
    if e == 5 then 0 else 1
}
"#,
        "scope_let_chain",
    );
}

// ─── Variable shadowing ───

#[test]
fn test_scope_shadow_same_type() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 10;
    let x = 20;
    if x == 20 then 0 else 1
}
"#,
        "scope_shadow_same",
    );
}

#[test]
fn test_scope_shadow_different_type() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 42;
    let x = "hello";
    if x == "hello" then 0 else 1
}
"#,
        "scope_shadow_diff_type",
    );
}

#[test]
fn test_scope_shadow_uses_previous() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 10;
    let x = x + 5;
    let x = x * 2;
    // 10 -> 15 -> 30
    if x == 30 then 0 else 1
}
"#,
        "scope_shadow_uses_prev",
    );
}

#[test]
fn test_scope_shadow_in_nested_block() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 10;
    let y = {
        let x = 20;
        x + 1
    };
    // x is still 10 outside, y = 21
    if x == 10 && y == 21 then 0 else 1
}
"#,
        "scope_shadow_nested",
    );
}

#[test]
fn test_scope_shadow_three_levels() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 1;
    let a = {
        let x = 2;
        let b = {
            let x = 3;
            x
        };
        x + b
    };
    // a = 2 + 3 = 5, x still 1
    if x == 1 && a == 5 then 0 else 1
}
"#,
        "scope_shadow_three",
    );
}

// ─── Block expressions as values ───

#[test]
fn test_scope_block_as_value() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = {
        let a = 10;
        let b = 20;
        a + b
    };
    if x == 30 then 0 else 1
}
"#,
        "scope_block_value",
    );
}

#[test]
fn test_scope_block_single_expression() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = { 42 };
    if x == 42 then 0 else 1
}
"#,
        "scope_block_single",
    );
}

#[test]
fn test_scope_nested_blocks_as_values() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = {
        let inner = {
            let deep = 5;
            deep * 2
        };
        inner + 3
    };
    // 5 * 2 = 10, + 3 = 13
    if x == 13 then 0 else 1
}
"#,
        "scope_nested_blocks",
    );
}

#[test]
fn test_scope_block_with_side_effects() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = {
        let a = 10;
        let b = a * 2;
        let c = b - 3;
        c
    };
    if result == 17 then 0 else 1
}
"#,
        "scope_block_side_effects",
    );
}

// ─── If-else as expression ───

#[test]
fn test_scope_if_else_value() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = if true then 42 else 0;
    if x == 42 then 0 else 1
}
"#,
        "scope_if_value",
    );
}

#[test]
fn test_scope_if_else_computed() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 10;
    let x = if a > 5 then a * 2 else a * 3;
    if x == 20 then 0 else 1
}
"#,
        "scope_if_computed",
    );
}

#[test]
fn test_scope_nested_if_expression() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 15;
    let category = if x > 20 then 3
                   else if x > 10 then 2
                   else if x > 0 then 1
                   else 0;
    if category == 2 then 0 else 1
}
"#,
        "scope_nested_if_expr",
    );
}

#[test]
fn test_scope_if_block_branches() {
    assert_aot_success(
        r#"
@main () -> int = {
    let condition = true;
    let result = if condition then {
        let a = 10;
        let b = 20;
        a + b
    } else {
        let c = 100;
        c
    };
    if result == 30 then 0 else 1
}
"#,
        "scope_if_block_branches",
    );
}

// ─── Match as expression ───

#[test]
fn test_scope_match_expression_value() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 2;
    let result = match x {
        1 -> 10,
        2 -> 20,
        3 -> 30,
        _ -> 0,
    };
    if result == 20 then 0 else 1
}
"#,
        "scope_match_value",
    );
}

#[test]
fn test_scope_match_in_let() {
    assert_aot_success(
        r#"
@main () -> int = {
    let label = match 42 {
        0 -> "zero",
        1 -> "one",
        _ -> "other",
    };
    if label == "other" then 0 else 1
}
"#,
        "scope_match_in_let",
    );
}

#[test]
fn test_scope_match_block_arms() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 3;
    let result = match x {
        1 -> {
            let a = 10;
            a + 1
        },
        2 -> {
            let a = 20;
            a + 2
        },
        3 -> {
            let a = 30;
            a + 3
        },
        _ -> 0,
    };
    if result == 33 then 0 else 1
}
"#,
        "scope_match_block_arms",
    );
}

// ─── Expressions in complex positions ───

#[test]
fn test_scope_expression_in_function_arg() {
    assert_aot_success(
        r#"
@add (a: int, b: int) -> int = a + b;

@main () -> int = {
    let result = add(a: { let x = 10; x }, b: { let y = 20; y });
    if result == 30 then 0 else 1
}
"#,
        "scope_expr_in_arg",
    );
}

#[test]
fn test_scope_expression_in_arithmetic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = { 10 } + { 20 };
    if x == 30 then 0 else 1
}
"#,
        "scope_expr_in_arith",
    );
}

#[test]
fn test_scope_expression_in_comparison() {
    assert_aot_success(
        r#"
@main () -> int = {
    let ok = { 10 + 5 } == { 3 * 5 };
    if ok then 0 else 1
}
"#,
        "scope_expr_in_cmp",
    );
}

// ─── Scope interaction with control flow ───

#[test]
fn test_scope_shadow_in_loop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 0;
    for i in 0..5 do {
        let x = i * 2;
        x
    };
    // After loop, x is still 0
    if x == 0 then 0 else 1
}
"#,
        "scope_shadow_loop",
    );
}

#[test]
fn test_scope_let_in_match_arm() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 2;
    let result = match x {
        n if n > 0 -> {
            let doubled = n * 2;
            let tripled = n * 3;
            doubled + tripled
        },
        _ -> 0,
    };
    // n=2: doubled=4, tripled=6, result=10
    if result == 10 then 0 else 1
}
"#,
        "scope_let_in_match",
    );
}

#[test]
fn test_scope_block_in_loop_body() {
    assert_aot_success(
        r#"
@main () -> int = {
    let total = 0;
    for i in 0..5 do {
        let contribution = {
            let base = i * 10;
            let bonus = 1;
            base + bonus
        };
        total = total + contribution;
    };
    // 1 + 11 + 21 + 31 + 41 = 105
    if total == 105 then 0 else 1
}
"#,
        "scope_block_in_loop",
    );
}

// ─── Complex scoping patterns ───

#[test]
fn test_scope_closure_captures_outer() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 10;
    let add_x = (y: int) -> x + y;
    let result = add_x(5);
    if result == 15 then 0 else 1
}
"#,
        "scope_closure_captures",
    );
}

#[test]
fn test_scope_shadow_before_closure() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 10;
    let x = 20;
    let get_x = () -> x;
    if get_x() == 20 then 0 else 1
}
"#,
        "scope_shadow_before_closure",
    );
}

#[test]
fn test_scope_many_lets_same_name() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 1;
    let x = x + 1;
    let x = x + 1;
    let x = x + 1;
    let x = x + 1;
    let x = x + 1;
    let x = x + 1;
    let x = x + 1;
    let x = x + 1;
    let x = x + 1;
    // 1 + 9 increments = 10
    if x == 10 then 0 else 1
}
"#,
        "scope_many_shadows",
    );
}

#[test]
fn test_scope_string_shadow() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello";
    let s = s + " world";
    if s == "hello world" then 0 else 1
}
"#,
        "scope_string_shadow",
    );
}

#[test]
fn test_scope_tuple_destructure() {
    assert_aot_success(
        r#"
@main () -> int = {
    let pair = (10, 20);
    let (a, b) = pair;
    if a == 10 && b == 20 then 0 else 1
}
"#,
        "scope_tuple_destr",
    );
}

#[test]
fn test_scope_if_else_string_value() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 5;
    let label = if x > 10 then "big" else "small";
    if label == "small" then 0 else 1
}
"#,
        "scope_if_str_value",
    );
}

#[test]
fn test_scope_block_returning_struct() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@main () -> int = {
    let p = {
        let a = 3;
        let b = 4;
        Point { x: a, y: b }
    };
    if p.x == 3 && p.y == 4 then 0 else 1
}
"#,
        "scope_block_struct",
    );
}

#[test]
fn test_scope_let_in_each_branch() {
    assert_aot_success(
        r#"
@main () -> int = {
    let flag = true;
    let result = if flag then {
        let a = 100;
        let b = 200;
        a + b
    } else {
        let c = 999;
        c
    };
    if result == 300 then 0 else 1
}
"#,
        "scope_let_each_branch",
    );
}

// ─── Match with various value types ───

#[test]
fn test_scope_match_bool_expression() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = true;
    let result = match x {
        true -> 100,
        false -> 200,
    };
    if result == 100 then 0 else 1
}
"#,
        "scope_match_bool",
    );
}

#[test]
fn test_scope_match_nested_in_if() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 5;
    let result = if x > 0 then
        match x {
            1 -> 10,
            5 -> 50,
            _ -> 0,
        }
    else 0;
    if result == 50 then 0 else 1
}
"#,
        "scope_match_in_if",
    );
}

#[test]
fn test_scope_if_in_match_arm() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 3;
    let result = match x {
        n if n > 0 -> if n > 2 then n * 10 else n,
        _ -> 0,
    };
    if result == 30 then 0 else 1
}
"#,
        "scope_if_in_match",
    );
}
