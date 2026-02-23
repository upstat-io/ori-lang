//! Error Handling AOT Tests
//!
//! Tests for ? operator, Result/Option chaining, error propagation through
//! multiple function calls, error handling in loops, and combined
//! Result+Option patterns.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Result: basic patterns ───

#[test]
fn test_err_result_ok_unwrap() {
    assert_aot_success(
        r#"
@main () -> int = {
    let r: Result<int, str> = Ok(42);
    if r.is_ok() && r.unwrap() == 42 then 0 else 1
}
"#,
        "err_result_ok",
    );
}

#[test]
fn test_err_result_err_check() {
    assert_aot_success(
        r#"
@main () -> int = {
    let r: Result<int, str> = Err("fail");
    if r.is_err() then 0 else 1
}
"#,
        "err_result_err",
    );
}

#[test]
fn test_err_result_unwrap_or_ok() {
    assert_aot_success(
        r#"
@main () -> int = {
    let r: Result<int, str> = Ok(42);
    let v = r.unwrap_or(0);
    if v == 42 then 0 else 1
}
"#,
        "err_result_unwrap_or_ok",
    );
}

#[test]
fn test_err_result_unwrap_or_err() {
    assert_aot_success(
        r#"
@main () -> int = {
    let r: Result<int, str> = Err("fail");
    let v = r.unwrap_or(99);
    if v == 99 then 0 else 1
}
"#,
        "err_result_unwrap_or_err",
    );
}

// ─── Option: basic patterns ───

#[test]
fn test_err_option_some_unwrap() {
    assert_aot_success(
        r#"
@main () -> int = {
    let o: Option<int> = Some(42);
    if o.is_some() && o.unwrap() == 42 then 0 else 1
}
"#,
        "err_option_some",
    );
}

#[test]
fn test_err_option_none_check() {
    assert_aot_success(
        r#"
@main () -> int = {
    let o: Option<int> = None;
    if o.is_none() then 0 else 1
}
"#,
        "err_option_none",
    );
}

#[test]
fn test_err_option_unwrap_or_some() {
    assert_aot_success(
        r#"
@main () -> int = {
    let o: Option<int> = Some(42);
    let v = o.unwrap_or(0);
    if v == 42 then 0 else 1
}
"#,
        "err_option_unwrap_or_some",
    );
}

#[test]
fn test_err_option_unwrap_or_none() {
    assert_aot_success(
        r#"
@main () -> int = {
    let o: Option<int> = None;
    let v = o.unwrap_or(99);
    if v == 99 then 0 else 1
}
"#,
        "err_option_unwrap_or_none",
    );
}

// ─── ? operator: Result ───

#[test]
fn test_err_try_result_ok() {
    assert_aot_success(
        r#"
@parse (x: int) -> Result<int, str> = Ok(x * 2);

@process () -> Result<int, str> = {
    let v = parse(x: 21)?;
    Ok(v)
}

@main () -> int = {
    let r = process();
    if r.is_ok() && r.unwrap() == 42 then 0 else 1
}
"#,
        "err_try_result_ok",
    );
}

#[test]
fn test_err_try_result_err() {
    assert_aot_success(
        r#"
@fail () -> Result<int, str> = Err("broken");

@process () -> Result<int, str> = {
    let v = fail()?;
    Ok(v + 1)
}

@main () -> int = {
    let r = process();
    if r.is_err() then 0 else 1
}
"#,
        "err_try_result_err",
    );
}

#[test]
fn test_err_try_result_chain() {
    assert_aot_success(
        r#"
@step1 (x: int) -> Result<int, str> = Ok(x + 10);
@step2 (x: int) -> Result<int, str> = Ok(x * 2);

@pipeline (x: int) -> Result<int, str> = {
    let a = step1(x: x)?;
    let b = step2(x: a)?;
    Ok(b)
}

@main () -> int = {
    let r = pipeline(x: 5);
    // 5 + 10 = 15, 15 * 2 = 30
    if r.is_ok() && r.unwrap() == 30 then 0 else 1
}
"#,
        "err_try_chain",
    );
}

#[test]
fn test_err_try_result_early_exit() {
    assert_aot_success(
        r#"
@step1 () -> Result<int, str> = Ok(10);
@step2 () -> Result<int, str> = Err("fail at step 2");
@step3 () -> Result<int, str> = Ok(30);

@pipeline () -> Result<int, str> = {
    let a = step1()?;
    let b = step2()?;
    let c = step3()?;
    Ok(a + b + c)
}

@main () -> int = {
    let r = pipeline();
    // step2 fails, so step3 is never reached
    if r.is_err() then 0 else 1
}
"#,
        "err_try_early_exit",
    );
}

// ─── ? operator: Option ───

#[test]
fn test_err_try_option_some() {
    assert_aot_success(
        r#"
@find (x: int) -> Option<int> = Some(x * 3);

@lookup () -> Option<int> = {
    let v = find(x: 10)?;
    Some(v + 1)
}

@main () -> int = {
    let o = lookup();
    if o.is_some() && o.unwrap() == 31 then 0 else 1
}
"#,
        "err_try_option_some",
    );
}

#[test]
fn test_err_try_option_none() {
    assert_aot_success(
        r#"
@find_nothing () -> Option<int> = None;

@lookup () -> Option<int> = {
    let v = find_nothing()?;
    Some(v + 1)
}

@main () -> int = {
    let o = lookup();
    if o.is_none() then 0 else 1
}
"#,
        "err_try_option_none",
    );
}

// ─── Result with conditional logic ───

#[test]
fn test_err_result_conditional() {
    assert_aot_success(
        r#"
@validate (x: int) -> Result<int, str> = {
    if x > 0 then Ok(x) else Err("must be positive")
}

@main () -> int = {
    let ok = validate(x: 5);
    let bad = validate(x: -1);
    if ok.is_ok() && bad.is_err() then 0 else 1
}
"#,
        "err_result_conditional",
    );
}

#[test]
fn test_err_result_chain_conditional() {
    assert_aot_success(
        r#"
@check_range (x: int) -> Result<int, str> = {
    if x < 0 then Err("too small")
    else if x > 100 then Err("too big")
    else Ok(x)
}

@main () -> int = {
    let r1 = check_range(x: 50);
    let r2 = check_range(x: -5);
    let r3 = check_range(x: 200);
    if r1.is_ok() && r2.is_err() && r3.is_err() then 0 else 1
}
"#,
        "err_chain_conditional",
    );
}

// ─── Deep ? chains ───

#[test]
fn test_err_deep_try_chain() {
    assert_aot_success(
        r#"
@a (x: int) -> Result<int, str> = Ok(x + 1);
@b (x: int) -> Result<int, str> = Ok(x + 2);
@c (x: int) -> Result<int, str> = Ok(x + 3);
@d (x: int) -> Result<int, str> = Ok(x + 4);

@deep_chain (x: int) -> Result<int, str> = {
    let v1 = a(x: x)?;
    let v2 = b(x: v1)?;
    let v3 = c(x: v2)?;
    let v4 = d(x: v3)?;
    Ok(v4)
}

@main () -> int = {
    let r = deep_chain(x: 0);
    // 0 + 1 + 2 + 3 + 4 = 10
    if r.is_ok() && r.unwrap() == 10 then 0 else 1
}
"#,
        "err_deep_try",
    );
}

// ─── Result in loops ───

#[test]
fn test_err_result_in_loop() {
    assert_aot_success(
        r#"
@validate (x: int) -> Result<int, str> = {
    if x >= 0 then Ok(x) else Err("negative")
}

@main () -> int = {
    let all_ok = true;
    for i in 0..5 do {
        let r = validate(x: i);
        if r.is_err() then {
            all_ok = false;
        }
    };
    if all_ok then 0 else 1
}
"#,
        "err_result_in_loop",
    );
}

// ─── Option patterns ───

#[test]
fn test_err_option_chain_unwrap() {
    assert_aot_success(
        r#"
@find_index (x: int) -> Option<int> = {
    if x > 0 then Some(x * 10) else None
}

@main () -> int = {
    let a = find_index(x: 3);
    let b = find_index(x: -1);
    let av = a.unwrap_or(0);
    let bv = b.unwrap_or(-1);
    if av == 30 && bv == -1 then 0 else 1
}
"#,
        "err_option_chain",
    );
}

// ─── Mixed Result + Option ───

#[test]
fn test_err_result_with_option_payload() {
    assert_aot_success(
        r#"
@find (key: int) -> Result<Option<int>, str> = {
    if key < 0 then Err("invalid key")
    else if key == 0 then Ok(None)
    else Ok(Some(key * 100))
}

@main () -> int = {
    let r1 = find(key: 5);
    let r2 = find(key: 0);
    let r3 = find(key: -1);
    let ok1 = r1.is_ok() && r1.unwrap().is_some() && r1.unwrap().unwrap() == 500;
    let ok2 = r2.is_ok() && r2.unwrap().is_none();
    let ok3 = r3.is_err();
    if ok1 && ok2 && ok3 then 0 else 1
}
"#,
        "err_result_option",
    );
}

// ─── Return type variants ───

#[test]
fn test_err_result_int_err() {
    assert_aot_success(
        r#"
@divide (a: int, b: int) -> Result<int, int> = {
    if b == 0 then Err(-1) else Ok(a / b)
}

@main () -> int = {
    let r1 = divide(a: 10, b: 2);
    let r2 = divide(a: 10, b: 0);
    if r1.is_ok() && r1.unwrap() == 5 && r2.is_err() then 0 else 1
}
"#,
        "err_result_int_err",
    );
}
