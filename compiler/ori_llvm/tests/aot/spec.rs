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
