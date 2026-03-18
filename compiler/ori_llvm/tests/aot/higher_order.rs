//! Higher-Order Function AOT Tests
//!
//! Tests for functions as arguments, return values, closures with
//! captures, closure composition, multi-parameter HOFs, function
//! chaining patterns, and callbacks in loops.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Functions as arguments ───

#[test]
fn test_hof_apply_identity() {
    assert_aot_success(
        r#"
@identity (x: int) -> int = x;
@apply (f: (int) -> int, x: int) -> int = f(x);

@main () -> int = {
    if apply(f: identity, x: 42) == 42 then 0 else 1
}
"#,
        "hof_apply_identity",
    );
}

#[test]
fn test_hof_apply_named_function() {
    assert_aot_success(
        r#"
@double (x: int) -> int = x * 2;
@apply (f: (int) -> int, x: int) -> int = f(x);

@main () -> int = {
    if apply(f: double, x: 21) == 42 then 0 else 1
}
"#,
        "hof_apply_named",
    );
}

#[test]
fn test_hof_apply_lambda() {
    assert_aot_success(
        r#"
@apply (f: (int) -> int, x: int) -> int = f(x);

@main () -> int = {
    let triple = (n: int) -> n * 3;
    if apply(f: triple, x: 14) == 42 then 0 else 1
}
"#,
        "hof_apply_lambda",
    );
}

#[test]
fn test_hof_two_function_args() {
    assert_aot_success(
        r#"
@combine (f: (int) -> int, g: (int) -> int, x: int) -> int = f(x) + g(x);

@main () -> int = {
    let dbl = (n: int) -> n * 2;
    let inc = (n: int) -> n + 1;
    // combine(3) = 6 + 4 = 10
    if combine(f: dbl, g: inc, x: 3) == 10 then 0 else 1
}
"#,
        "hof_two_fn_args",
    );
}

#[test]
fn test_hof_apply_twice() {
    assert_aot_success(
        r#"
@apply_twice (f: (int) -> int, x: int) -> int = f(f(x));

@main () -> int = {
    let add10 = (n: int) -> n + 10;
    // 0 + 10 + 10 = 20
    if apply_twice(f: add10, x: 0) == 20 then 0 else 1
}
"#,
        "hof_apply_twice",
    );
}

// ─── Functions returning functions ───

#[test]
fn test_hof_make_adder() {
    assert_aot_success(
        r#"
@make_adder (base: int) -> (int) -> int = {
    (n: int) -> base + n
};

@main () -> int = {
    let add5 = make_adder(base: 5);
    let add10 = make_adder(base: 10);
    if add5(3) == 8 && add10(3) == 13 then 0 else 1
}
"#,
        "hof_make_adder",
    );
}

#[test]
fn test_hof_make_multiplier() {
    assert_aot_success(
        r#"
@make_multiplier (factor: int) -> (int) -> int = {
    (n: int) -> factor * n
};

@main () -> int = {
    let times3 = make_multiplier(factor: 3);
    let times7 = make_multiplier(factor: 7);
    if times3(10) == 30 && times7(6) == 42 then 0 else 1
}
"#,
        "hof_make_multiplier",
    );
}

#[test]
fn test_hof_make_predicate() {
    assert_aot_success(
        r#"
@make_gt (threshold: int) -> (int) -> bool = {
    (n: int) -> n > threshold
};

@main () -> int = {
    let gt5 = make_gt(threshold: 5);
    if gt5(10) && !gt5(3) then 0 else 1
}
"#,
        "hof_make_predicate",
    );
}

// ─── Composition ───

#[test]
fn test_hof_compose() {
    assert_aot_success(
        r#"
@compose (f: (int) -> int, g: (int) -> int, x: int) -> int = f(g(x));

@main () -> int = {
    let dbl = (n: int) -> n * 2;
    let inc = (n: int) -> n + 1;
    // compose(5) = (5+1)*2 = 12
    if compose(f: dbl, g: inc, x: 5) == 12 then 0 else 1
}
"#,
        "hof_compose",
    );
}

#[test]
fn test_hof_compose_reverse_order() {
    assert_aot_success(
        r#"
@compose (f: (int) -> int, g: (int) -> int, x: int) -> int = f(g(x));

@main () -> int = {
    let dbl = (n: int) -> n * 2;
    let inc = (n: int) -> n + 1;
    // compose(5) = (5*2)+1 = 11
    if compose(f: inc, g: dbl, x: 5) == 11 then 0 else 1
}
"#,
        "hof_compose_rev",
    );
}

#[test]
fn test_hof_pipeline_three() {
    assert_aot_success(
        r#"
@pipe3 (f: (int) -> int, g: (int) -> int, h: (int) -> int, x: int) -> int = h(g(f(x)));

@main () -> int = {
    let add1 = (n: int) -> n + 1;
    let dbl = (n: int) -> n * 2;
    let sub3 = (n: int) -> n - 3;
    // 5 → 6 → 12 → 9
    if pipe3(f: add1, g: dbl, h: sub3, x: 5) == 9 then 0 else 1
}
"#,
        "hof_pipe3",
    );
}

// ─── Closures with captures ───

#[test]
fn test_hof_closure_capture_computation() {
    assert_aot_success(
        r#"
@main () -> int = {
    let base = 100;
    let offset = 23;
    let compute = (x: int) -> base + offset + x;
    if compute(0) == 123 then 0 else 1
}
"#,
        "hof_closure_compute",
    );
}

#[test]
fn test_hof_closure_capture_in_loop() {
    assert_aot_success(
        r#"
@apply (f: (int) -> int, x: int) -> int = f(x);

@main () -> int = {
    let multiplier = 10;
    let scale = (n: int) -> n * multiplier;
    let sum = 0;
    for i in 1..=3 do {
        sum = sum + apply(f: scale, x: i);
    };
    // 10 + 20 + 30 = 60
    if sum == 60 then 0 else 1
}
"#,
        "hof_closure_in_loop",
    );
}

#[test]
fn test_hof_closure_bool_capture() {
    assert_aot_success(
        r#"
@main () -> int = {
    let flag = true;
    let check = (x: int) -> if flag then x * 2 else x;
    if check(21) == 42 then 0 else 1
}
"#,
        "hof_closure_bool",
    );
}

// ─── Callbacks ───

#[test]
fn test_hof_callback_pattern() {
    assert_aot_success(
        r#"
@process (value: int, on_success: (int) -> int) -> int = {
    if value > 0 then on_success(value) else -1
}

@main () -> int = {
    let r1 = process(value: 5, on_success: (x: int) -> x * 10);
    let r2 = process(value: -1, on_success: (x: int) -> x * 10);
    if r1 == 50 && r2 == -1 then 0 else 1
}
"#,
        "hof_callback",
    );
}

#[test]
fn test_hof_predicate_filter_count() {
    assert_aot_success(
        r#"
@count_matching (pred: (int) -> bool) -> int = {
    let count = 0;
    for i in 0..10 do {
        if pred(i) then {
            count = count + 1;
        }
    };
    count
}

@main () -> int = {
    let is_even = (n: int) -> n % 2 == 0;
    let c = count_matching(pred: is_even);
    // 0,2,4,6,8 = 5 even numbers
    if c == 5 then 0 else 1
}
"#,
        "hof_pred_filter",
    );
}

// ─── Multiple return closure usage ───

#[test]
fn test_hof_two_closures_same_capture() {
    assert_aot_success(
        r#"
@main () -> int = {
    let base = 10;
    let add = (x: int) -> base + x;
    let mul = (x: int) -> base * x;
    if add(5) == 15 && mul(5) == 50 then 0 else 1
}
"#,
        "hof_two_closures",
    );
}

// ─── Nested closures ───

#[test]
fn test_hof_nested_closure_deep() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 1;
    let f = (b: int) -> {
        let g = (c: int) -> {
            let h = (d: int) -> a + b + c + d;
            h(4)
        };
        g(3)
    };
    // 1 + 2 + 3 + 4 = 10
    if f(2) == 10 then 0 else 1
}
"#,
        "hof_nested_deep",
    );
}

// ─── Named functions as values ───

#[test]
fn test_hof_named_fn_as_arg() {
    assert_aot_success(
        r#"
@square (x: int) -> int = x * x;
@cube (x: int) -> int = x * x * x;
@apply (f: (int) -> int, x: int) -> int = f(x);

@main () -> int = {
    let s = apply(f: square, x: 5);
    let c = apply(f: cube, x: 3);
    if s == 25 && c == 27 then 0 else 1
}
"#,
        "hof_named_fn_arg",
    );
}

// ─── Closure in conditional ───

#[test]
fn test_hof_closure_conditional_select() {
    assert_aot_success(
        r#"
@apply (f: (int) -> int, x: int) -> int = f(x);

@main () -> int = {
    let use_double = true;
    let dbl = (n: int) -> n * 2;
    let triple = (n: int) -> n * 3;
    let f = if use_double then dbl else triple;
    if apply(f: f, x: 7) == 14 then 0 else 1
}
"#,
        "hof_cond_select",
    );
}

// ─── Accumulator with HOF ───

#[test]
fn test_hof_fold_manual() {
    assert_aot_success(
        r#"
@fold (init: int, f: (int, int) -> int) -> int = {
    let acc = init;
    for i in 1..=5 do {
        acc = f(acc, i);
    };
    acc
}

@main () -> int = {
    let sum = fold(init: 0, f: (a: int, b: int) -> a + b);
    // 0+1+2+3+4+5 = 15
    if sum == 15 then 0 else 1
}
"#,
        "hof_fold_manual",
    );
}

#[test]
fn test_hof_fold_product() {
    assert_aot_success(
        r#"
@fold (init: int, f: (int, int) -> int) -> int = {
    let acc = init;
    for i in 1..=5 do {
        acc = f(acc, i);
    };
    acc
}

@main () -> int = {
    let product = fold(init: 1, f: (a: int, b: int) -> a * b);
    // 1*2*3*4*5 = 120
    if product == 120 then 0 else 1
}
"#,
        "hof_fold_product",
    );
}

// ─── Chained closures ───

#[test]
fn test_hof_chain_closures() {
    assert_aot_success(
        r#"
@main () -> int = {
    let step1 = (x: int) -> x + 10;
    let step2 = (x: int) -> x * 2;
    let step3 = (x: int) -> x - 5;
    let result = step3(step2(step1(0)));
    // 0 → 10 → 20 → 15
    if result == 15 then 0 else 1
}
"#,
        "hof_chain_closures",
    );
}

// ─── Closure with complex capture ───

#[test]
fn test_hof_closure_capture_from_function() {
    assert_aot_success(
        r#"
@compute (x: int) -> int = x * x + 1;

@main () -> int = {
    let val = compute(x: 5);
    let use_val = (offset: int) -> val + offset;
    // compute(5) = 26, 26 + 4 = 30
    if use_val(4) == 30 then 0 else 1
}
"#,
        "hof_capture_from_fn",
    );
}

// ─── Lambda returning bool ───

#[test]
fn test_hof_bool_lambda() {
    assert_aot_success(
        r#"
@check_pred (p: (int) -> bool, x: int) -> bool = p(x);

@main () -> int = {
    let is_positive = (n: int) -> n > 0;
    let r1 = check_pred(p: is_positive, x: 5);
    let r2 = check_pred(p: is_positive, x: -3);
    if r1 && !r2 then 0 else 1
}
"#,
        "hof_bool_lambda",
    );
}

// ─── Multi-param lambda ───

#[test]
fn test_hof_multi_param_lambda() {
    assert_aot_success(
        r#"
@apply2 (f: (int, int) -> int, a: int, b: int) -> int = f(a, b);

@main () -> int = {
    let add = (x: int, y: int) -> x + y;
    let mul = (x: int, y: int) -> x * y;
    let s = apply2(f: add, a: 3, b: 4);
    let p = apply2(f: mul, a: 3, b: 4);
    if s == 7 && p == 12 then 0 else 1
}
"#,
        "hof_multi_param",
    );
}

// ─── Closure return value chaining ───

#[test]
fn test_hof_closure_return_chain() {
    assert_aot_success(
        r#"
@main () -> int = {
    let scale = (x: int) -> x * 10;
    let total = scale(1) + scale(2) + scale(3);
    if total == 60 then 0 else 1
}
"#,
        "hof_closure_return_chain",
    );
}

// ─── C1: Mixed closure types across functions ───
// Bug: lambda names are per-function (`__lambda_0` in each), but stored
// in a global HashMap. Lambdas in different functions collide.

#[test]
fn test_hof_cross_function_lambda_collision() {
    // C1 bug: lambda in @main collides with lambda in @make_adder
    // Both produce __lambda_0 → HashMap collision → wrong calling convention
    assert_aot_success(
        r#"
@apply (f: (int) -> int, x: int) -> int = f(x);
@make_adder (n: int) -> (int) -> int = (x: int) -> x + n;

@main () -> int = {
    let $double = (x: int) -> x * 2;
    let $a = apply(f: double, x: 5);
    let $add10 = make_adder(n: 10);
    let $b = add10(7);
    if a == 10 && b == 17 then 0 else 1
}
"#,
        "hof_cross_function_collision",
    );
}

#[test]
fn test_hof_journey5_reproduction() {
    // Exact Journey 5 reproduction — AOT should compute 27
    assert_aot_success(
        r#"
@apply (f: (int) -> int, x: int) -> int = f(x);
@make_adder (n: int) -> (int) -> int = (x: int) -> x + n;

@main () -> int = {
    let $double = (x: int) -> x * 2;
    let $a = apply(f: double, x: 5);
    let $add10 = make_adder(n: 10);
    let $b = add10(7);
    if a + b == 27 then 0 else 1
}
"#,
        "hof_journey5",
    );
}

#[test]
fn test_hof_multiple_functions_with_lambdas() {
    // Three separate functions each containing a lambda
    assert_aot_success(
        r#"
@double_it (x: int) -> int = {
    let $f = (n: int) -> n * 2;
    f(x)
}

@triple_it (x: int) -> int = {
    let $f = (n: int) -> n * 3;
    f(x)
}

@add_ten (x: int) -> int = {
    let $offset = 10;
    let $f = (n: int) -> n + offset;
    f(x)
}

@main () -> int = {
    let $a = double_it(x: 5);
    let $b = triple_it(x: 5);
    let $c = add_ten(x: 5);
    if a == 10 && b == 15 && c == 15 then 0 else 1
}
"#,
        "hof_multiple_fn_lambdas",
    );
}

#[test]
fn test_hof_two_noncapturing_in_different_functions() {
    // Non-capturing lambdas in different functions — tests name collision
    // even when both are non-capturing (same phantom env convention)
    assert_aot_success(
        r#"
@apply_double (x: int) -> int = {
    let $f = (n: int) -> n * 2;
    f(x)
}

@apply_square (x: int) -> int = {
    let $f = (n: int) -> n * n;
    f(x)
}

@main () -> int = {
    let $a = apply_double(x: 5);
    let $b = apply_square(x: 5);
    if a == 10 && b == 25 then 0 else 1
}
"#,
        "hof_two_noncapturing_diff_fns",
    );
}

// Closure captures of non-scalar (fat pointer) types.
//
// These tests verify that lambdas capturing heap-allocated values (str, [T])
// have correct RC management. The root cause of the bug was that
// declare_and_process_lambda() did not apply AIMS param ownership to lambda
// params before running the ARC pipeline, causing collect_all_borrowed_defs()
// to miss borrowed params and their Let aliases. Edge cleanup then emitted
// spurious RcDec for borrowed-param aliases, causing double-free.

/// Closure capturing a heap-allocated str (>23 bytes, exceeds SSO).
/// Without the fix, `ori_rc_dec` is called on already-freed allocation.
#[test]
fn test_closure_capture_heap_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "this is a long string that exceeds SSO threshold of twenty three bytes";
    let f = () -> s.length();
    if f() == 70 then 0 else 1
}
"#,
        "closure_capture_heap_str",
    );
}

/// Closure capturing `[int]` and calling `.length()`.
#[test]
fn test_closure_capture_list() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3, 4, 5];
    let f = () -> xs.length();
    if f() == 5 then 0 else 1
}
"#,
        "closure_capture_list",
    );
}

/// Closure with str capture and a str parameter — the J17 pattern.
#[test]
fn test_closure_capture_str_with_param() {
    assert_aot_success(
        r#"
@main () -> int = {
    let prefix = "hello world prefix that exceeds SSO limit by far";
    let f = s -> prefix.length() + s.length();
    if f("world") == 53 then 0 else 1
}
"#,
        "closure_capture_str_with_param",
    );
}

/// Closure passed as argument with a heap str capture — higher-order with
/// fat pointer capture.
#[test]
fn test_closure_passed_with_str_capture() {
    assert_aot_success(
        r#"
@apply (f: (str) -> int, s: str) -> int = f(s);

@main () -> int = {
    let prefix = "a very long prefix string that exceeds SSO threshold";
    let f = s -> prefix.length() + s.length();
    if apply(f: f, s: "world") == 57 then 0 else 1
}
"#,
        "closure_passed_with_str_capture",
    );
}

/// Multiple non-scalar captures (str + [int]).
#[test]
fn test_closure_multi_capture() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello world with extra padding to exceed SSO threshold";
    let xs = [1, 2, 3];
    let f = () -> s.length() + xs.length();
    if f() == 57 then 0 else 1
}
"#,
        "closure_multi_capture",
    );
}
