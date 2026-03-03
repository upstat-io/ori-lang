//! AOT Spec Conformance Tests
//!
//! End-to-end tests that compile Ori programs through the full AOT pipeline
//! (compile → link → execute) and verify correct behavior.
//!
//! These tests mirror patterns from `tests/spec/` but run through AOT instead
//! of the interpreter or JIT backends.
//!
//! These tests can run in parallel - each test uses unique temp files via
//! atomic counters, and the AOT compiler uses `tempfile::TempDir` for
//! intermediate object files.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{assert_aot_success, compile_and_run_capture};

#[test]
fn test_aot_let_binding_basic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 42;
    if x == 42 then 0 else 1
}
"#,
        "let_binding_basic",
    );
}

#[test]
fn test_aot_let_binding_annotated() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x: int = 42;
    let y: bool = true;
    if x == 42 && y then 0 else 1
}
"#,
        "let_binding_annotated",
    );
}

#[test]
fn test_aot_let_shadowing() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 1;
    let x = x + 1;
    let x = x * 2;
    if x == 4 then 0 else 1
}
"#,
        "let_shadowing",
    );
}

#[test]
fn test_aot_if_then_else() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = if true then 1 else 0;
    let b = if false then 0 else 2;
    if a == 1 && b == 2 then 0 else 1
}
"#,
        "if_then_else",
    );
}

#[test]
fn test_aot_nested_conditionals() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = if true then if true then 1 else 2 else 3;
    let y = if false then 1 else if true then 2 else 3;
    if x == 1 && y == 2 then 0 else 1
}
"#,
        "nested_conditionals",
    );
}

#[test]
fn test_aot_comparison_conditions() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 10;
    let a = if x > 5 then 1 else 0;
    let b = if x < 20 then 1 else 0;
    let c = if x == 10 then 1 else 0;
    let d = if x != 5 then 1 else 0;
    if a == 1 && b == 1 && c == 1 && d == 1 then 0 else 1
}
"#,
        "comparison_conditions",
    );
}

#[test]
fn test_aot_arithmetic_add_sub() {
    assert_aot_success(
        r#"
@main () -> int = {
    let add = 3 + 4;
    let sub = 10 - 3;
    if add == 7 && sub == 7 then 0 else 1
}
"#,
        "arithmetic_add_sub",
    );
}

#[test]
fn test_aot_arithmetic_mul_div() {
    assert_aot_success(
        r#"
@main () -> int = {
    let mul = 6 * 7;
    let div_result = 42 / 6;
    if mul == 42 && div_result == 7 then 0 else 1
}
"#,
        "arithmetic_mul_div",
    );
}

#[test]
fn test_aot_arithmetic_modulo() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m1 = 17 % 5;
    let m2 = 10 % 3;
    if m1 == 2 && m2 == 1 then 0 else 1
}
"#,
        "arithmetic_modulo",
    );
}

#[test]
fn test_aot_arithmetic_negation() {
    assert_aot_success(
        r#"
@main () -> int = {
    let neg = -5;
    let double_neg = -(-10);
    if neg == -5 && double_neg == 10 then 0 else 1
}
"#,
        "arithmetic_negation",
    );
}

#[test]
fn test_aot_arithmetic_precedence() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 2 + 3 * 4;
    let b = (2 + 3) * 4;
    if a == 14 && b == 20 then 0 else 1
}
"#,
        "arithmetic_precedence",
    );
}

#[test]
fn test_aot_boolean_and() {
    assert_aot_success(
        r#"
@main () -> int = {
    let tt = true && true;
    let tf = true && false;
    let ft = false && true;
    let ff = false && false;
    if tt && !tf && !ft && !ff then 0 else 1
}
"#,
        "boolean_and",
    );
}

#[test]
fn test_aot_boolean_or() {
    assert_aot_success(
        r#"
@main () -> int = {
    let tt = true || true;
    let tf = true || false;
    let ft = false || true;
    let ff = false || false;
    if tt && tf && ft && !ff then 0 else 1
}
"#,
        "boolean_or",
    );
}

#[test]
fn test_aot_boolean_not() {
    assert_aot_success(
        r#"
@main () -> int = {
    let not_true = !true;
    let not_false = !false;
    if !not_true && not_false then 0 else 1
}
"#,
        "boolean_not",
    );
}

#[test]
fn test_aot_function_call() {
    assert_aot_success(
        r#"
@double (n: int) -> int = n * 2;

@main () -> int = {
    let result = double(n: 21);
    if result == 42 then 0 else 1
}
"#,
        "function_call",
    );
}

#[test]
fn test_aot_function_multiple_params() {
    assert_aot_success(
        r#"
@add (a: int, b: int) -> int = a + b;

@main () -> int = {
    let result = add(a: 35, b: 7);
    if result == 42 then 0 else 1
}
"#,
        "function_multiple_params",
    );
}

#[test]
fn test_aot_function_recursion() {
    assert_aot_success(
        r#"
@factorial (n: int) -> int = if n <= 1 then 1 else n * factorial(n: n - 1);

@main () -> int = {
    let f5 = factorial(n: 5);
    if f5 == 120 then 0 else 1
}
"#,
        "function_recursion",
    );
}

#[test]
fn test_aot_function_nested_calls() {
    assert_aot_success(
        r#"
@double (n: int) -> int = n * 2;
@add_one (n: int) -> int = n + 1;

@main () -> int = {
    let result = double(n: add_one(n: 20));
    if result == 42 then 0 else 1
}
"#,
        "function_nested_calls",
    );
}

#[test]
fn test_aot_comparison_equality() {
    assert_aot_success(
        r#"
@main () -> int = {
    let eq = 42 == 42;
    let neq = 42 != 43;
    if eq && neq then 0 else 1
}
"#,
        "comparison_equality",
    );
}

#[test]
fn test_aot_comparison_ordering() {
    assert_aot_success(
        r#"
@main () -> int = {
    let lt = 3 < 5;
    let le1 = 5 <= 5;
    let le2 = 4 <= 5;
    let gt = 7 > 3;
    let ge1 = 7 >= 7;
    let ge2 = 8 >= 7;
    if lt && le1 && le2 && gt && ge1 && ge2 then 0 else 1
}
"#,
        "comparison_ordering",
    );
}

#[test]
fn test_aot_print_string() {
    let source = r#"@main () -> void = print(msg: "Hello AOT!");"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert_eq!(exit_code, 0, "print_string failed: {stderr}");
    assert!(
        stdout.contains("Hello AOT!"),
        "Expected output to contain 'Hello AOT!', got stdout: '{stdout}', stderr: '{stderr}'"
    );
}

#[test]
fn test_aot_complex_expression() {
    assert_aot_success(
        r#"
@max (a: int, b: int) -> int = if a > b then a else b;
@min (a: int, b: int) -> int = if a < b then a else b;
@clamp (value: int, lo: int, hi: int) -> int = max(a: lo, b: min(a: value, b: hi));

@main () -> int = {
    let c1 = clamp(value: 5, lo: 0, hi: 10);
    let c2 = clamp(value: -5, lo: 0, hi: 10);
    let c3 = clamp(value: 15, lo: 0, hi: 10);
    if c1 == 5 && c2 == 0 && c3 == 10 then 0 else 1
}
"#,
        "complex_expression",
    );
}

#[test]
fn test_aot_fibonacci() {
    assert_aot_success(
        r#"
@fib (n: int) -> int = if n <= 1 then n else fib(n: n - 1) + fib(n: n - 2);

@main () -> int = {
    let f0 = fib(n: 0);
    let f1 = fib(n: 1);
    let f5 = fib(n: 5);
    let f10 = fib(n: 10);
    if f0 == 0 && f1 == 1 && f5 == 5 && f10 == 55 then 0 else 1
}
"#,
        "fibonacci",
    );
}

// Duration and Size Literals

#[test]
fn test_aot_duration_literals() {
    assert_aot_success(
        r#"
@main () -> int = {
    let ns_ok = 100ns == 100ns;
    let us_ok = 1us == 1000ns;
    let ms_ok = 1ms == 1000us;
    let s_ok = 1s == 1000ms;
    let m_ok = 1m == 60s;
    let h_ok = 1h == 60m;
    if ns_ok && us_ok && ms_ok && s_ok && m_ok && h_ok then 0 else 1
}
"#,
        "duration_literals",
    );
}

#[test]
fn test_aot_duration_negative() {
    assert_aot_success(
        r#"
@main () -> int = {
    let neg = -(1s);
    let neg_ok = neg == -1s;
    let double_neg = -(-(500ms));
    let double_neg_ok = double_neg == 500ms;
    if neg_ok && double_neg_ok then 0 else 1
}
"#,
        "duration_negative",
    );
}

#[test]
fn test_aot_size_literals() {
    assert_aot_success(
        r#"
@main () -> int = {
    let b_ok = 100b == 100b;
    let kb_ok = 1kb == 1000b;
    let mb_ok = 1mb == 1000kb;
    let gb_ok = 1gb == 1000mb;
    let tb_ok = 1tb == 1000gb;
    if b_ok && kb_ok && mb_ok && gb_ok && tb_ok then 0 else 1
}
"#,
        "size_literals",
    );
}

// Duration and Size Arithmetic

#[test]
fn test_aot_duration_arithmetic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let add_ok = 1s + 500ms == 1500ms;
    let sub_ok = 2s - 1s == 1s;
    let mul_ok = 100ms * 3 == 300ms;
    let int_mul_ok = 2 * 500ms == 1s;
    let div_ok = 1s / 2 == 500ms;
    let mod_ok = 1500ms % 1s == 500ms;
    if add_ok && sub_ok && mul_ok && int_mul_ok && div_ok && mod_ok then 0 else 1
}
"#,
        "duration_arithmetic",
    );
}

#[test]
fn test_aot_duration_comparison() {
    assert_aot_success(
        r#"
@main () -> int = {
    let lt = 500ms < 1s;
    let le = 1s <= 1000ms;
    let gt = 2s > 1s;
    let ge = 1s >= 1000ms;
    let eq = 1s == 1000ms;
    let ne = 1s != 2s;
    if lt && le && gt && ge && eq && ne then 0 else 1
}
"#,
        "duration_comparison",
    );
}

#[test]
fn test_aot_size_arithmetic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let add_ok = 1kb + 500b == 1500b;
    let sub_ok = 2kb - 1kb == 1kb;
    let mul_ok = 100b * 3 == 300b;
    let int_mul_ok = 2 * 500b == 1kb;
    let div_ok = 1kb / 2 == 500b;
    let mod_ok = 1500b % 1kb == 500b;
    if add_ok && sub_ok && mul_ok && int_mul_ok && div_ok && mod_ok then 0 else 1
}
"#,
        "size_arithmetic",
    );
}

#[test]
fn test_aot_size_comparison() {
    assert_aot_success(
        r#"
@main () -> int = {
    let lt = 500b < 1kb;
    let le = 1kb <= 1000b;
    let gt = 2kb > 1kb;
    let ge = 1kb >= 1000b;
    let eq = 1kb == 1000b;
    let ne = 1kb != 2kb;
    if lt && le && gt && ge && eq && ne then 0 else 1
}
"#,
        "size_comparison",
    );
}

// Float Primitives

#[test]
fn test_aot_float_literals() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 3.14;
    let b = 0.5;
    let c = 1.5e2;
    let ok1 = a == 3.14;
    let ok2 = b == 0.5;
    let ok3 = c == 150.0;
    if ok1 && ok2 && ok3 then 0 else 1
}
"#,
        "float_literals",
    );
}

#[test]
fn test_aot_float_arithmetic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let add = 2.5 + 3.5;
    let sub = 10.0 - 3.75;
    let mul = 3.0 * 4.0;
    let quotient = 15.0 / 2.0;
    if add == 6.0 && sub == 6.25 && mul == 12.0 && quotient == 7.5 then 0 else 1
}
"#,
        "float_arithmetic",
    );
}

#[test]
fn test_aot_float_comparison() {
    assert_aot_success(
        r#"
@main () -> int = {
    let lt = 1.5 < 2.5;
    let le = 3.0 <= 3.0;
    let gt = 5.5 > 4.5;
    let ge = 7.0 >= 7.0;
    let eq = 1.0 == 1.0;
    let ne = 1.0 != 2.0;
    if lt && le && gt && ge && eq && ne then 0 else 1
}
"#,
        "float_comparison",
    );
}

#[test]
fn test_aot_float_negation() {
    assert_aot_success(
        r#"
@main () -> int = {
    let neg = -5.0;
    let double_neg = -(-3.5);
    if neg == -5.0 && double_neg == 3.5 then 0 else 1
}
"#,
        "float_negation",
    );
}

// Char Primitives

#[test]
fn test_aot_char_literals() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 'a';
    let b = 'b';
    let eq = a == 'a';
    let ne = a != b;
    if eq && ne then 0 else 1
}
"#,
        "char_literals",
    );
}

#[test]
fn test_aot_char_comparison() {
    assert_aot_success(
        r#"
@main () -> int = {
    let lt = 'a' < 'b';
    let le = 'a' <= 'a';
    let gt = 'z' > 'a';
    let ge = 'z' >= 'z';
    if lt && le && gt && ge then 0 else 1
}
"#,
        "char_comparison",
    );
}

// Byte Primitives

#[test]
fn test_aot_byte_basics() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a: byte = 65;
    let b: byte = 65;
    let c: byte = 0;
    let d: byte = 255;
    let eq = a == b;
    let ne = a != c;
    let bounds = c != d;
    if eq && ne && bounds then 0 else 1
}
"#,
        "byte_basics",
    );
}

// Never Type Coercion

#[test]
fn test_aot_never_panic_coercion() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x: int = if true then 42 else panic(msg: "unreachable");
    if x == 42 then 0 else 1
}
"#,
        "never_panic_coercion",
    );
}

#[test]
fn test_aot_never_conditional_branches() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a: int = if false then panic(msg: "nope") else 1;
    let b: str = if true then "hello" else panic(msg: "nope");
    let c: bool = if true then true else panic(msg: "nope");
    if a == 1 && b == "hello" && c then 0 else 1
}
"#,
        "never_conditional_branches",
    );
}

// Loop, Break, Continue — Never Type Coercion

#[test]
fn test_aot_loop_break_value() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = loop break 42;
    if result == 42 then 0 else 1
}
"#,
        "loop_break_value",
    );
}

#[test]
fn test_aot_loop_conditional_break() {
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
        "loop_conditional_break",
    );
}

#[test]
fn test_aot_loop_break_never_coercion() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    let result = loop {
        count = count + 1;
        if count > 5 then break count else count
    };
    if result == 6 then 0 else 1
}
"#,
        "loop_break_never_coercion",
    );
}

#[test]
fn test_aot_loop_continue_never_coercion() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    let sum = 0;
    loop {
        count = count + 1;
        if count > 10 then break;
        if count % 2 == 0 then continue;
        sum = sum + count
    };
    if sum == 25 then 0 else 1
}
"#,
        "loop_continue_never_coercion",
    );
}

#[test]
fn test_aot_loop_break_and_continue_combined() {
    assert_aot_success(
        r#"
@main () -> int = {
    let i = 0;
    let total = 0;
    loop {
        i = i + 1;
        if i > 20 then break;
        if i % 3 == 0 then continue;
        total = total + i
    };
    if total == 147 then 0 else 1
}
"#,
        "loop_break_and_continue_combined",
    );
}

// Result/Option Constructors and ? Operator

#[test]
fn test_aot_result_ok_unwrap() {
    assert_aot_success(
        r#"
@make_ok () -> Result<int, str> = Ok(42);

@main () -> int = {
    let r = make_ok();
    if r.is_ok() then {
        let v = r.unwrap();
        if v == 42 then 0 else 1
    } else 1
}
"#,
        "result_ok_unwrap",
    );
}

#[test]
fn test_aot_result_err_check() {
    assert_aot_success(
        r#"
@make_err () -> Result<int, str> = Err("bad");

@main () -> int = {
    let r = make_err();
    if r.is_err() then 0 else 1
}
"#,
        "result_err_check",
    );
}

/// C4 regression: Option match tag inversion — switch labels must match construction tags.
/// Construction: Some=tag 0, None=tag 1. Match must use the same mapping.
#[test]
fn test_aot_option_match_tag_correctness() {
    assert_aot_success(
        r#"
@unwrap_or (opt: Option<int>, default: int) -> int =
    match opt { Some(v) -> v, None -> default }

@main () -> int = {
    let some_val = unwrap_or(opt: Some(42), default: 0);
    let none_val = unwrap_or(opt: None, default: 99);
    // some_val should be 42 (not 0), none_val should be 99 (not garbage)
    if some_val == 42 then {
        if none_val == 99 then 0 else 1
    } else 1
}
"#,
        "option_match_tag_correctness",
    );
}

/// C4 regression: match on Option inside if/else producing Option values.
#[test]
fn test_aot_option_match_with_construction() {
    assert_aot_success(
        r#"
@safe_div (a: int, b: int) -> Option<int> =
    if b == 0 then None else Some(a / b);

@unwrap_or (opt: Option<int>, default: int) -> int =
    match opt { Some(v) -> v, None -> default }

@main () -> int = {
    let a = unwrap_or(opt: safe_div(a: 100, b: 5), default: 0);
    let b = unwrap_or(opt: safe_div(a: 100, b: 0), default: 5);
    // a should be 20, b should be 5
    if a == 20 then {
        if b == 5 then 0 else 1
    } else 1
}
"#,
        "option_match_with_construction",
    );
}

#[test]
fn test_aot_option_some_unwrap() {
    assert_aot_success(
        r#"
@make_some () -> Option<int> = Some(42);

@main () -> int = {
    let o = make_some();
    if o.is_some() then {
        let v = o.unwrap();
        if v == 42 then 0 else 1
    } else 1
}
"#,
        "option_some_unwrap",
    );
}

#[test]
fn test_aot_option_none_check() {
    assert_aot_success(
        r#"
@make_none () -> Option<int> = None;

@main () -> int = {
    let o = make_none();
    if o.is_none() then 0 else 1
}
"#,
        "option_none_check",
    );
}

#[test]
fn test_aot_try_result_ok_unwraps() {
    assert_aot_success(
        r#"
@get_value () -> Result<int, str> = Ok(21);

@double_value () -> Result<int, str> = {
    let x = get_value()?;
    Ok(x * 2)
}

@main () -> int = {
    let r = double_value();
    if r.is_ok() then {
        let v = r.unwrap();
        if v == 42 then 0 else 1
    } else 1
}
"#,
        "try_result_ok_unwraps",
    );
}

#[test]
fn test_aot_try_result_err_propagates() {
    assert_aot_success(
        r#"
@fail_early () -> Result<int, str> = Err("oops");

@try_it () -> Result<int, str> = {
    let x = fail_early()?;
    Ok(x * 2)
}

@main () -> int = {
    let r = try_it();
    if r.is_err() then 0 else 1
}
"#,
        "try_result_err_propagates",
    );
}

#[test]
fn test_aot_try_option_some_unwraps() {
    assert_aot_success(
        r#"
@find_value () -> Option<int> = Some(42);

@try_find () -> Option<int> = {
    let x = find_value()?;
    Some(x + 1)
}

@main () -> int = {
    let o = try_find();
    if o.is_some() then {
        let v = o.unwrap();
        if v == 43 then 0 else 1
    } else 1
}
"#,
        "try_option_some_unwraps",
    );
}

#[test]
fn test_aot_try_option_none_propagates() {
    assert_aot_success(
        r#"
@find_nothing () -> Option<int> = None;

@try_find () -> Option<int> = {
    let x = find_nothing()?;
    Some(x + 1)
}

@main () -> int = {
    let o = try_find();
    if o.is_none() then 0 else 1
}
"#,
        "try_option_none_propagates",
    );
}

#[test]
fn test_aot_try_chained_result() {
    assert_aot_success(
        r#"
@step1 (x: int) -> Result<int, str> = {
    if x > 0 then Ok(x * 2) else Err("must be positive")
}

@step2 (x: int) -> Result<int, str> = {
    if x < 100 then Ok(x + 1) else Err("too large")
}

@pipeline (x: int) -> Result<int, str> = {
    let a = step1(x: x)?;
    let b = step2(x: a)?;
    Ok(b)
}

@main () -> int = {
    let r = pipeline(x: 5);
    if r.is_ok() then {
        let v = r.unwrap();
        if v == 11 then 0 else 1
    } else 1
}
"#,
        "try_chained_result",
    );
}

#[test]
fn test_aot_try_chained_first_fails() {
    assert_aot_success(
        r#"
@step1 (x: int) -> Result<int, str> = {
    if x > 0 then Ok(x * 2) else Err("must be positive")
}

@step2 (x: int) -> Result<int, str> = {
    if x < 100 then Ok(x + 1) else Err("too large")
}

@pipeline (x: int) -> Result<int, str> = {
    let a = step1(x: x)?;
    let b = step2(x: a)?;
    Ok(b)
}

@main () -> int = {
    let r = pipeline(x: -1);
    if r.is_err() then 0 else 1
}
"#,
        "try_chained_first_fails",
    );
}

// String Escape Sequences

#[test]
fn test_aot_string_escape_tab() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello\tworld";
    if s.length() == 11 then 0 else 1
}
"#,
        "string_escape_tab",
    );
}

#[test]
fn test_aot_string_escape_backslash() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "a\\b";
    if s.length() == 3 then 0 else 1
}
"#,
        "string_escape_backslash",
    );
}

#[test]
fn test_aot_string_escape_quote() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "say \"hi\"";
    if s.length() == 8 then 0 else 1
}
"#,
        "string_escape_quote",
    );
}

#[test]
fn test_aot_string_escape_newline() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "line1\nline2";
    if s.length() == 11 then 0 else 1
}
"#,
        "string_escape_newline",
    );
}

// Unit / Void

#[test]
fn test_aot_unit_return() {
    assert_aot_success(
        r#"
@do_nothing () -> void = ();

@main () -> int = {
    do_nothing();
    0
}
"#,
        "unit_return",
    );
}

#[test]
fn test_aot_unit_in_conditional() {
    assert_aot_success(
        r#"
@side_effect (x: int) -> void = ();

@main () -> int = {
    if true then side_effect(x: 1) else side_effect(x: 2);
    0
}
"#,
        "unit_in_conditional",
    );
}

// Match Expressions

#[test]
fn test_aot_match_int_literal() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = match 42 {
        0 -> 1,
        42 -> 0,
        _ -> 2,
    };
    x
}
"#,
        "match_int_literal",
    );
}

#[test]
fn test_aot_match_wildcard() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = match 99 {
        0 -> 1,
        1 -> 2,
        _ -> 0,
    };
    x
}
"#,
        "match_wildcard",
    );
}

#[test]
fn test_aot_match_nested_with_if() {
    assert_aot_success(
        r#"
@classify (n: int) -> int =
    match n {
        0 -> 0,
        1 -> 1,
        _ -> if n > 10 then 3 else 2,
    };

@main () -> int = {
    let a = classify(n: 0);
    let b = classify(n: 1);
    let c = classify(n: 5);
    let d = classify(n: 20);
    if a == 0 && b == 1 && c == 2 && d == 3 then 0 else 1
}
"#,
        "match_nested_with_if",
    );
}

#[test]
fn test_aot_match_bool() {
    assert_aot_success(
        r#"
@to_int (b: bool) -> int = match b {
    true -> 1,
    false -> 0,
};

@main () -> int = {
    let a = to_int(b: true);
    let b = to_int(b: false);
    if a == 1 && b == 0 then 0 else 1
}
"#,
        "match_bool",
    );
}

#[test]
fn test_aot_match_expression_valued() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = match 3 {
        1 -> 10,
        2 -> 20,
        3 -> 30,
        _ -> 0,
    };
    if result == 30 then 0 else 1
}
"#,
        "match_expression_valued",
    );
}

// Mutual Recursion

#[test]
fn test_aot_mutual_recursion() {
    assert_aot_success(
        r#"
@is_even (n: int) -> bool = if n == 0 then true else is_odd(n: n - 1);
@is_odd (n: int) -> bool = if n == 0 then false else is_even(n: n - 1);

@main () -> int = {
    let e = is_even(n: 10);
    let o = is_odd(n: 7);
    if e && o then 0 else 1
}
"#,
        "mutual_recursion",
    );
}

#[test]
fn test_aot_mutual_recursion_deeper() {
    assert_aot_success(
        r#"
@is_even (n: int) -> bool = if n == 0 then true else is_odd(n: n - 1);
@is_odd (n: int) -> bool = if n == 0 then false else is_even(n: n - 1);

@main () -> int = {
    let e50 = is_even(n: 50);
    let o49 = is_odd(n: 49);
    let e51 = is_even(n: 51);
    if e50 && o49 && !e51 then 0 else 1
}
"#,
        "mutual_recursion_deeper",
    );
}

// Nested Control Flow

#[test]
fn test_aot_nested_match_in_if() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 5;
    let result = if x > 0 then {
        match x {
            1 -> 10,
            5 -> 50,
            _ -> 0,
        }
    } else -1;
    if result == 50 then 0 else 1
}
"#,
        "nested_match_in_if",
    );
}

#[test]
fn test_aot_nested_if_in_match() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 3;
    let result = match x {
        1 -> if true then 10 else 11,
        2 -> if false then 20 else 21,
        _ -> if x > 2 then 30 else 31,
    };
    if result == 30 then 0 else 1
}
"#,
        "nested_if_in_match",
    );
}

#[test]
fn test_aot_loop_with_match() {
    assert_aot_success(
        r#"
@main () -> int = {
    let i = 0;
    let sum = 0;
    loop {
        if i >= 5 then break;
        let contribution = match i {
            0 -> 1,
            1 -> 2,
            2 -> 4,
            _ -> 8,
        };
        sum = sum + contribution;
        i = i + 1
    };
    if sum == 23 then 0 else 1
}
"#,
        "loop_with_match",
    );
}

// Deep Recursion (stress)

#[test]
fn test_aot_deep_recursion() {
    assert_aot_success(
        r#"
@count_down (n: int) -> int = if n == 0 then 0 else count_down(n: n - 1);

@main () -> int = {
    let result = count_down(n: 1000);
    if result == 0 then 0 else 1
}
"#,
        "deep_recursion",
    );
}

// =========================================================================
// Match: char patterns
// =========================================================================

#[test]
fn test_aot_match_char() {
    assert_aot_success(
        r#"
@classify (c: char) -> int = match c {
    'a' -> 1,
    'b' -> 2,
    'z' -> 26,
    _ -> 0,
};

@main () -> int = {
    let a = classify(c: 'a');
    let b = classify(c: 'b');
    let z = classify(c: 'z');
    let other = classify(c: 'x');
    if a == 1 && b == 2 && z == 26 && other == 0 then 0 else 1
}
"#,
        "match_char",
    );
}

// =========================================================================
// Bitwise operators
// =========================================================================

#[test]
fn test_aot_bitwise_and_or_xor() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 0xFF & 0x0F;
    let b = 0x0F | 0xF0;
    let c = 0xFF ^ 0x0F;
    if a == 15 && b == 255 && c == 240 then 0 else 1
}
"#,
        "bitwise_and_or_xor",
    );
}

#[test]
fn test_aot_bitwise_shift() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 1 << 4;
    let b = 256 >> 3;
    if a == 16 && b == 32 then 0 else 1
}
"#,
        "bitwise_shift",
    );
}

#[test]
fn test_aot_bitwise_not() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = ~0;
    if a == -1 then 0 else 1
}
"#,
        "bitwise_not",
    );
}

// =========================================================================
// String operations
// =========================================================================

#[test]
fn test_aot_string_equality() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "hello";
    let b = "hello";
    let c = "world";
    if a == b && a != c then 0 else 1
}
"#,
        "string_equality",
    );
}

#[test]
fn test_aot_string_length() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello";
    let empty = "";
    if s.length() == 5 && empty.length() == 0 then 0 else 1
}
"#,
        "string_length",
    );
}

#[test]
fn test_aot_string_concat() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "hello ";
    let b = "world";
    let c = a + b;
    if c == "hello world" then 0 else 1
}
"#,
        "string_concat",
    );
}

// =========================================================================
// Tuples
// =========================================================================

#[test]
fn test_aot_tuple_construction_destructure() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (10, 20);
    let (a, b) = t;
    if a == 10 && b == 20 then 0 else 1
}
"#,
        "tuple_construct_destruct",
    );
}

#[test]
fn test_aot_tuple_field_access() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (10, 20, 30);
    if t.0 == 10 && t.1 == 20 && t.2 == 30 then 0 else 1
}
"#,
        "tuple_field_access",
    );
}

#[test]
fn test_aot_tuple_from_function() {
    assert_aot_success(
        r#"
@pair (a: int, b: int) -> (int, int) = (a, b);

@main () -> int = {
    let p = pair(a: 3, b: 7);
    let (x, y) = p;
    if x == 3 && y == 7 then 0 else 1
}
"#,
        "tuple_from_function",
    );
}

// =========================================================================
// Structs
// =========================================================================

#[test]
fn test_aot_struct_construction() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };

@main () -> int = {
    let p = Point { x: 3, y: 4 };
    if p.x == 3 && p.y == 4 then 0 else 1
}
"#,
        "struct_construction",
    );
}

#[test]
fn test_aot_struct_update() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };

@main () -> int = {
    let p = Point { x: 3, y: 4 };
    let p2 = Point { ...p, x: 10 };
    if p2.x == 10 && p2.y == 4 then 0 else 1
}
"#,
        "struct_update",
    );
}

#[test]
fn test_aot_struct_as_parameter() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };

@sum_fields (p: Point) -> int = p.x + p.y;

@main () -> int = {
    let p = Point { x: 10, y: 20 };
    if sum_fields(p: p) == 30 then 0 else 1
}
"#,
        "struct_as_parameter",
    );
}

// =========================================================================
// Closures and higher-order functions
// =========================================================================

#[test]
fn test_aot_closure_capture() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 10;
    let add_x = (n: int) -> int = n + x;
    let result = add_x(n: 32);
    if result == 42 then 0 else 1
}
"#,
        "closure_capture",
    );
}

#[test]
fn test_aot_higher_order_function() {
    assert_aot_success(
        r#"
@apply (f: (int) -> int, x: int) -> int = f(x);

@main () -> int = {
    let double = (n: int) -> int = n * 2;
    let result = apply(f: double, x: 21);
    if result == 42 then 0 else 1
}
"#,
        "higher_order_function",
    );
}

#[test]
fn test_aot_function_returning_closure() {
    assert_aot_success(
        r#"
@make_adder (n: int) -> (int) -> int = (x: int) -> int = x + n;

@main () -> int = {
    let add5 = make_adder(n: 5);
    let result = add5(x: 37);
    if result == 42 then 0 else 1
}
"#,
        "function_returning_closure",
    );
}

#[test]
fn test_aot_closure_composition() {
    assert_aot_success(
        r#"
@apply_both (f: (int) -> int, g: (int) -> int, x: int) -> int = g(f(x));

@main () -> int = {
    let double = (n: int) -> int = n * 2;
    let inc = (n: int) -> int = n + 1;
    let result = apply_both(f: double, g: inc, x: 20);
    if result == 41 then 0 else 1
}
"#,
        "closure_composition",
    );
}

// =========================================================================
// For-in loops
// =========================================================================

#[test]
fn test_aot_for_in_range() {
    assert_aot_success(
        r#"
@main () -> int = {
    let sum = 0;
    for i in 1..=10 do {
        sum = sum + i;
    };
    if sum == 55 then 0 else 1
}
"#,
        "for_in_range",
    );
}

#[test]
fn test_aot_for_in_list() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    for i in [1, 2, 3, 4, 5] do {
        count = count + 1;
    };
    if count == 5 then 0 else 1
}
"#,
        "for_in_list",
    );
}

// =========================================================================
// Collections: list
// =========================================================================

#[test]
fn test_aot_list_literal_length() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3, 4, 5];
    if xs.length() == 5 then 0 else 1
}
"#,
        "list_literal_length",
    );
}

#[test]
fn test_aot_list_map_collect() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let doubled = xs.iter().map(x -> x * 2).collect();
    if doubled.length() == 3 then 0 else 1
}
"#,
        "list_map_collect",
    );
}

// Enum variant constructors

#[test]
fn test_aot_enum_construction() {
    assert_aot_success(
        r#"
type Shape = Circle(radius: int) | Square(side: int);

@area (s: Shape) -> int = match s {
    Circle(r) -> r * r,
    Square(s) -> s * s,
};

@main () -> int = {
    let c = Circle(radius: 5);
    let s = Square(side: 3);
    if area(s: c) == 25 && area(s: s) == 9 then 0 else 1
}
"#,
        "enum_construction",
    );
}

#[test]
fn test_aot_enum_unit_variants() {
    assert_aot_success(
        r#"
type Color = Red | Green | Blue;

@to_int (c: Color) -> int = match c {
    Red -> 0,
    Green -> 1,
    Blue -> 2,
};

@main () -> int = {
    let r = Red;
    let g = Green;
    let b = Blue;
    if to_int(c: r) == 0 && to_int(c: g) == 1 && to_int(c: b) == 2 then 0 else 1
}
"#,
        "enum_unit_variants",
    );
}

#[test]
fn test_aot_enum_mixed_variants() {
    assert_aot_success(
        r#"
type Value = Nothing | Single(x: int) | Pair(x: int, y: int);

@sum (v: Value) -> int = match v {
    Nothing -> 0,
    Single(x) -> x,
    Pair(x, y) -> x + y,
};

@main () -> int = {
    let a = Nothing;
    let b = Single(x: 10);
    let c = Pair(x: 3, y: 7);
    if sum(v: a) == 0 && sum(v: b) == 10 && sum(v: c) == 10 then 0 else 1
}
"#,
        "enum_mixed_variants",
    );
}

#[test]
fn test_aot_enum_as_param_and_return() {
    assert_aot_success(
        r#"
type Dir = Left | Right;

@flip (d: Dir) -> Dir = match d {
    Left -> Right,
    Right -> Left,
};

@is_left (d: Dir) -> bool = match d {
    Left -> true,
    Right -> false,
};

@main () -> int = {
    let d = Left;
    let d2 = flip(d: d);
    if is_left(d: d) && !is_left(d: d2) then 0 else 1
}
"#,
        "enum_param_return",
    );
}

// =========================================================================
// Known AOT gaps (ignored until codegen supports them)
// =========================================================================

#[test]
fn test_aot_derive_eq_struct() {
    assert_aot_success(
        r#"
#derive(Eq)
type Point = { x: int, y: int };

@main () -> int = {
    let a = Point { x: 1, y: 2 };
    let b = Point { x: 1, y: 2 };
    let c = Point { x: 3, y: 4 };
    if a == b && a != c then 0 else 1
}
"#,
        "derive_eq_struct",
    );
}

#[test]
fn test_aot_recursive_enum_tree() {
    assert_aot_success(
        r#"
type Tree = Leaf(value: int) | Node(left: Tree, right: Tree);

@tree_sum (t: Tree) -> int = match t {
    Leaf(v) -> v,
    Node(l, r) -> tree_sum(t: l) + tree_sum(t: r)
}

@main () -> int = {
    let leaf1 = Leaf(value: 5);
    let leaf2 = Leaf(value: 10);
    let tree = Node(left: leaf1, right: leaf2);
    if tree_sum(t: tree) == 15 then 0 else 1
}
"#,
        "recursive_enum_tree",
    );
}

/// Deeper recursive enum: 3 levels of nesting.
#[test]
fn test_aot_recursive_enum_tree_deep() {
    assert_aot_success(
        r#"
type Tree = Leaf(value: int) | Node(left: Tree, right: Tree);

@tree_sum (t: Tree) -> int = match t {
    Leaf(v) -> v,
    Node(l, r) -> tree_sum(t: l) + tree_sum(t: r)
}

@main () -> int = {
    let a = Leaf(value: 1);
    let b = Leaf(value: 2);
    let c = Leaf(value: 3);
    let d = Leaf(value: 4);
    let left = Node(left: a, right: b);
    let right = Node(left: c, right: d);
    let root = Node(left: left, right: right);
    if tree_sum(t: root) == 10 then 0 else 1
}
"#,
        "recursive_enum_tree_deep",
    );
}

/// Recursive enum with a single-field variant (linked list).
#[test]
fn test_aot_recursive_enum_linked_list() {
    assert_aot_success(
        r#"
type List = Nil | Cons(head: int, tail: List);

@list_sum (l: List) -> int = match l {
    Nil -> 0,
    Cons(h, t) -> h + list_sum(l: t)
}

@main () -> int = {
    let list = Cons(head: 1, tail: Cons(head: 2, tail: Cons(head: 3, tail: Nil)));
    if list_sum(l: list) == 6 then 0 else 1
}
"#,
        "recursive_enum_linked_list",
    );
}

#[test]
fn test_aot_derive_eq_enum() {
    assert_aot_success(
        r#"
#derive(Eq)
type Color = Red | Green | Blue;

@main () -> int = {
    let a = Red;
    let b = Red;
    let c = Blue;
    if a == b && a != c then 0 else 1
}
"#,
        "derive_eq_enum",
    );
}

#[test]
fn test_aot_derive_eq_enum_all_variants() {
    assert_aot_success(
        r#"
#derive(Eq)
type Dir = North | South | East | West;

@main () -> int = {
    let n1 = North;
    let n2 = North;
    let s = South;
    let e = East;
    let w = West;
    // Same variant == same variant
    let ok1 = n1 == n2;
    // Different variants != each other
    let ok2 = n1 != s;
    let ok3 = s != e;
    let ok4 = e != w;
    let ok5 = w != n1;
    if ok1 && ok2 && ok3 && ok4 && ok5 then 0 else 1
}
"#,
        "derive_eq_enum_all",
    );
}

#[test]
fn test_aot_list_index() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [10, 20, 30];
    if xs[0] == 10 && xs[1] == 20 && xs[2] == 30 then 0 else 1
}
"#,
        "list_index",
    );
}

#[test]
fn test_aot_string_interpolation() {
    assert_aot_success(
        r#"
@main () -> int = {
    let name = "world";
    let greeting = `hello {name}`;
    if greeting == "hello world" then 0 else 1
}
"#,
        "string_interpolation",
    );
}

// =========================================================================
// Section 11.1 — While-like loops (loop + conditional break)
// =========================================================================

#[test]
fn test_aot_while_pattern_basic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let i = 0;
    loop {
        if i >= 10 then break;
        i = i + 1
    };
    if i == 10 then 0 else 1
}
"#,
        "while_pattern_basic",
    );
}

#[test]
fn test_aot_while_pattern_with_accumulator() {
    assert_aot_success(
        r#"
@main () -> int = {
    let n = 10;
    let result = 0;
    loop {
        if n == 0 then break;
        result = result + n;
        n = n - 1
    };
    if result == 55 then 0 else 1
}
"#,
        "while_pattern_accumulator",
    );
}

// =========================================================================
// Section 11.1 — catch(expr:) panic recovery
// =========================================================================

#[test]
fn test_aot_catch_success() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = catch(expr: 42);
    if result.is_ok() then {
        let v = result.unwrap();
        if v == 42 then 0 else 1
    } else 1
}
"#,
        "catch_success",
    );
}

#[test]
#[ignore = "AOT gap: inline panic in catch — invoke only intercepts callee-function panics, not same-function inline code"]
fn test_aot_catch_panic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = catch(expr: panic(msg: "test error"));
    if result.is_err() then 0 else 1
}
"#,
        "catch_panic",
    );
}

#[test]
#[ignore = "AOT gap: inline panic in catch — invoke only intercepts callee-function panics, not same-function inline code"]
fn test_aot_catch_div_by_zero() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = catch(expr: 1 / 0);
    if result.is_err() then 0 else 1
}
"#,
        "catch_div_by_zero",
    );
}

// =========================================================================
// Section 11.1 — Generic functions
// =========================================================================

#[test]
fn test_aot_generic_identity() {
    assert_aot_success(
        r#"
@identity <T> (x: T) -> T = x;

@main () -> int = {
    let a = identity(x: 42);
    let b = identity(x: true);
    if a == 42 && b then 0 else 1
}
"#,
        "generic_identity",
    );
}

#[test]
fn test_aot_generic_pair() {
    assert_aot_success(
        r#"
@make_pair <A, B> (a: A, b: B) -> (A, B) = (a, b);

@main () -> int = {
    let p = make_pair(a: 1, b: true);
    let (x, y) = p;
    if x == 1 && y then 0 else 1
}
"#,
        "generic_pair",
    );
}

#[test]
fn test_aot_generic_three_type_params() {
    assert_aot_success(
        r#"
@triple <A, B, C> (a: A, b: B, c: C) -> (A, B, C) = (a, b, c);

@main () -> int = {
    let t = triple(a: 1, b: true, c: 42);
    let (x, y, z) = t;
    if x == 1 && y && z == 42 then 0 else 1
}
"#,
        "generic_three_params",
    );
}

#[test]
fn test_aot_generic_calling_non_generic() {
    assert_aot_success(
        r#"
@double (n: int) -> int = n * 2;
@apply_double <T> (x: T, n: int) -> int = double(n: n);

@main () -> int = {
    let result = apply_double(x: true, n: 21);
    if result == 42 then 0 else 1
}
"#,
        "generic_calling_non_generic",
    );
}

#[test]
fn test_aot_generic_two_specializations() {
    assert_aot_success(
        r#"
@identity <T> (x: T) -> T = x;

@main () -> int = {
    let a = identity(x: 42);
    let b = identity(x: true);
    if a == 42 && b then 0 else 1
}
"#,
        "generic_two_specializations",
    );
}

// =========================================================================
// Section 11.1 — Map collection operations
// =========================================================================

#[test]
fn test_aot_map_literal_length() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"a": 1, "b": 2, "c": 3};
    if m.length() == 3 then 0 else 1
}
"#,
        "map_literal_length",
    );
}

#[test]
fn test_aot_map_is_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"key": 42};
    let empty: {str: int} = {};
    if !m.is_empty() && empty.is_empty() then 0 else 1
}
"#,
        "map_is_empty",
    );
}

// =========================================================================
// Section 11.1 — List operations
// =========================================================================

#[test]
fn test_aot_list_push() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let ys = xs.push(4);
    if ys.length() == 4 then 0 else 1
}
"#,
        "list_push",
    );
}

#[test]
fn test_aot_list_concat() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = [1, 2];
    let b = [3, 4];
    let c = a.concat(b);
    if c.length() == 4 then 0 else 1
}
"#,
        "list_concat",
    );
}

#[test]
fn test_aot_list_first_last() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [10, 20, 30];
    let f = xs.first();
    let l = xs.last();
    if f.is_some() && l.is_some() then {
        let fv = f.unwrap();
        let lv = l.unwrap();
        if fv == 10 && lv == 30 then 0 else 1
    } else 1
}
"#,
        "list_first_last",
    );
}

#[test]
fn test_aot_list_empty_operations() {
    assert_aot_success(
        r#"
@main () -> int = {
    let empty: [int] = [];
    let ok1 = empty.is_empty();
    let ok2 = empty.first().is_none();
    let ok3 = empty.last().is_none();
    if ok1 && ok2 && ok3 then 0 else 1
}
"#,
        "list_empty_operations",
    );
}

// =========================================================================
// Section 11.1 — Struct with RC fields (ARC stress)
// =========================================================================

#[test]
fn test_aot_struct_with_list_field() {
    assert_aot_success(
        r#"
type Container = { items: [int], label: str }

@main () -> int = {
    let c = Container { items: [1, 2, 3], label: "test" };
    if c.items.length() == 3 then 0 else 1
}
"#,
        "struct_with_list_field",
    );
}

#[test]
fn test_aot_list_of_strings() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = ["hello", "world", "foo"];
    let count = 0;
    for s in xs do count = count + s.length();
    if count == 13 then 0 else 1
}
"#,
        "list_of_strings",
    );
}

#[test]
fn test_aot_struct_with_string_fields_shared() {
    assert_aot_success(
        r#"
type Person = { name: str, role: str }

@greet (p: Person) -> str = p.name + " (" + p.role + ")";

@main () -> int = {
    let p = Person { name: "Alice", role: "dev" };
    let g = greet(p: p);
    if g.length() > 0 then 0 else 1
}
"#,
        "struct_string_fields_shared",
    );
}

// =========================================================================
// Section 11.1 — Closures: zero capture, multiple capture, nested
// =========================================================================

#[test]
fn test_aot_closure_zero_capture() {
    assert_aot_success(
        r#"
@main () -> int = {
    let add = (a: int, b: int) -> int = a + b;
    if add(a: 3, b: 4) == 7 then 0 else 1
}
"#,
        "closure_zero_capture",
    );
}

#[test]
fn test_aot_closure_capturing_closure() {
    assert_aot_success(
        r#"
@main () -> int = {
    let base = 10;
    let make_adder = (n: int) -> (int) -> int = {
        (x: int) -> int = base + n + x
    };
    let add15 = make_adder(n: 5);
    if add15(x: 2) == 17 then 0 else 1
}
"#,
        "closure_capturing_closure",
    );
}

// =========================================================================
// Section 11.1 — Enumerate iterator (produces tuples)
// =========================================================================

#[test]
fn test_aot_iter_enumerate() {
    assert_aot_success(
        r#"
@main () -> int = {
    let c = [10, 20, 30].iter().enumerate().count();
    if c == 3 then 0 else 1
}
"#,
        "iter_enumerate",
    );
}

// =========================================================================
// Section 11.1 — Deep nesting stress
// =========================================================================

#[test]
fn test_aot_match_inside_loop_inside_if() {
    assert_aot_success(
        r#"
@main () -> int = {
    let sum = 0;
    if true then {
        let i = 0;
        loop {
            if i >= 5 then break;
            let contribution = match i % 3 {
                0 -> 1,
                1 -> 2,
                _ -> 3,
            };
            sum = sum + contribution;
            i = i + 1
        }
    } else ();
    if sum == 9 then 0 else 1
}
"#,
        "match_inside_loop_inside_if",
    );
}

// =========================================================================
// Section 11.1 — Comparison operators on structs (via trait dispatch)
// =========================================================================

#[test]
fn test_aot_derive_eq_struct_not_equal() {
    assert_aot_success(
        r#"
#derive(Eq)
type Vec2 = { x: int, y: int };

@main () -> int = {
    let a = Vec2 { x: 1, y: 2 };
    let b = Vec2 { x: 3, y: 4 };
    if a != b then 0 else 1
}
"#,
        "derive_eq_struct_neq",
    );
}

#[test]
fn test_aot_derive_eq_struct_with_strings() {
    assert_aot_success(
        r#"
#derive(Eq)
type Named = { name: str, value: int };

@main () -> int = {
    let a = Named { name: "hello", value: 42 };
    let b = Named { name: "hello", value: 42 };
    let c = Named { name: "world", value: 42 };
    if a == b && a != c then 0 else 1
}
"#,
        "derive_eq_struct_strings",
    );
}

#[test]
fn test_aot_derive_comparable_struct() {
    assert_aot_success(
        r#"
#derive(Eq, Comparable)
type Score = { points: int, bonus: int };

@main () -> int = {
    let a = Score { points: 10, bonus: 5 };
    let b = Score { points: 20, bonus: 3 };
    let c = Score { points: 10, bonus: 5 };
    if a < b && b > a && a <= c && a >= c then 0 else 1
}
"#,
        "derive_comparable_struct",
    );
}

// =========================================================================
// Section 11.1 — Panic and error handling (non-catch)
// =========================================================================

#[test]
fn test_aot_panic_basic() {
    let source = r#"
@main () -> int = {
    let x = 42;
    if x == 42 then 0 else panic(message: "unreachable")
}
"#;
    // Should succeed (panic branch not taken)
    assert_aot_success(source, "panic_basic");
}

#[test]
fn test_aot_option_unwrap_some() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = Some(42);
    let val = x.unwrap();
    if val == 42 then 0 else 1
}
"#,
        "option_unwrap_some",
    );
}

#[test]
fn test_aot_result_unwrap_ok() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x: Result<int, str> = Ok(42);
    let val = x.unwrap();
    if val == 42 then 0 else 1
}
"#,
        "result_unwrap_ok",
    );
}

// =========================================================================
// Section 11.1 — ARC: collections of RC'd values (more patterns)
// =========================================================================

#[test]
fn test_aot_struct_with_list_and_string() {
    assert_aot_success(
        r#"
type Config = { name: str, values: [int] };

@main () -> int = {
    let c = Config { name: "test", values: [1, 2, 3] };
    if c.name.length() == 4 then 0 else 1
}
"#,
        "struct_with_list_and_string",
    );
}

#[test]
fn test_aot_nested_struct_with_strings() {
    assert_aot_success(
        r#"
type Inner = { label: str };
type Outer = { inner: Inner, count: int };

@main () -> int = {
    let o = Outer { inner: Inner { label: "ok" }, count: 5 };
    if o.count == 5 then 0 else 1
}
"#,
        "nested_struct_with_strings",
    );
}

// =========================================================================
// Section 11.1 — For-yield with complex expressions
// =========================================================================

#[test]
fn test_aot_for_yield_with_filter() {
    assert_aot_success(
        r#"
@main () -> int = {
    let evens = for x in 0..10 if x % 2 == 0 yield x;
    let count = evens.iter().count();
    if count == 5 then 0 else 1
}
"#,
        "for_yield_with_filter",
    );
}

#[test]
fn test_aot_for_yield_transform() {
    assert_aot_success(
        r#"
@main () -> int = {
    let squares = for x in 1..=5 yield x * x;
    let sum = squares.iter().fold(start: 0, f: (acc: int, x: int) -> int = acc + x);
    if sum == 55 then 0 else 1
}
"#,
        "for_yield_transform",
    );
}

// =========================================================================
// Prelude builtin functions (str, int, float, byte, hash_combine)
// =========================================================================

#[test]
fn test_aot_str_from_int() {
    let (exit_code, stdout, stderr) =
        compile_and_run_capture(r#"@main () -> void = print(msg: str(42));"#);
    assert_eq!(exit_code, 0, "str_from_int failed: {stderr}");
    assert!(
        stdout.contains("42"),
        "Expected '42' in output, got: '{stdout}'"
    );
}

#[test]
fn test_aot_str_from_bool() {
    let (exit_code, stdout, stderr) =
        compile_and_run_capture(r#"@main () -> void = print(msg: str(true));"#);
    assert_eq!(exit_code, 0, "str_from_bool failed: {stderr}");
    assert!(
        stdout.contains("true"),
        "Expected 'true' in output, got: '{stdout}'"
    );
}

#[test]
fn test_aot_str_from_float() {
    let (exit_code, stdout, stderr) =
        compile_and_run_capture(r#"@main () -> void = print(msg: str(3.14));"#);
    assert_eq!(exit_code, 0, "str_from_float failed: {stderr}");
    assert!(
        stdout.contains("3.14"),
        "Expected '3.14' in output, got: '{stdout}'"
    );
}

#[test]
fn test_aot_str_from_str() {
    let (exit_code, stdout, stderr) =
        compile_and_run_capture(r#"@main () -> void = print(msg: str("hello"));"#);
    assert_eq!(exit_code, 0, "str_from_str failed: {stderr}");
    assert!(
        stdout.contains("hello"),
        "Expected 'hello' in output, got: '{stdout}'"
    );
}

#[test]
fn test_aot_int_from_float() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = int(3.7);
    if x == 3 then 0 else 1
}
"#,
        "int_from_float",
    );
}

#[test]
fn test_aot_int_from_bool() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = int(true);
    let f = int(false);
    if t == 1 && f == 0 then 0 else 1
}
"#,
        "int_from_bool",
    );
}

#[test]
fn test_aot_float_from_int() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = float(42);
    if x == 42.0 then 0 else 1
}
"#,
        "float_from_int",
    );
}

#[test]
fn test_aot_byte_from_int() {
    assert_aot_success(
        r#"
@main () -> int = {
    let b = byte(65);
    let back = b.to_int();
    if back == 65 then 0 else 1
}
"#,
        "byte_from_int",
    );
}

#[test]
fn test_aot_hash_combine_basic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let h = hash_combine(0, 42);
    // hash_combine should produce a non-zero value for non-zero inputs
    if h != 0 then 0 else 1
}
"#,
        "hash_combine_basic",
    );
}
