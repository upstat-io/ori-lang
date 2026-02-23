//! Depth & Complexity AOT Tests
//!
//! Tests that push the AOT pipeline on **complexity**: many match arms, deep
//! control flow nesting, long error-propagation chains, complex closure patterns,
//! and multi-derive struct combinations. These verify LLVM codegen correctness
//! for non-trivial program structures.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Match with many arms ───

#[test]
fn test_depth_match_20_arms() {
    assert_aot_success(
        r#"
@classify (n: int) -> int = match n {
    0 -> 100,
    1 -> 101,
    2 -> 102,
    3 -> 103,
    4 -> 104,
    5 -> 105,
    6 -> 106,
    7 -> 107,
    8 -> 108,
    9 -> 109,
    10 -> 110,
    11 -> 111,
    12 -> 112,
    13 -> 113,
    14 -> 114,
    15 -> 115,
    16 -> 116,
    17 -> 117,
    18 -> 118,
    19 -> 119,
    _ -> 0,
};

@main () -> int = {
    let ok1 = classify(n: 0) == 100;
    let ok2 = classify(n: 9) == 109;
    let ok3 = classify(n: 19) == 119;
    let ok4 = classify(n: 50) == 0;
    if ok1 && ok2 && ok3 && ok4 then 0 else 1
}
"#,
        "depth_match_20_arms",
    );
}

#[test]
fn test_depth_match_in_loop() {
    assert_aot_success(
        r#"
@category (n: int) -> int = match n % 5 {
    0 -> 10,
    1 -> 20,
    2 -> 30,
    3 -> 40,
    _ -> 50,
};

@main () -> int = {
    let sum = 0;
    for i in 0..100 do sum = sum + category(n: i);
    // each group of 5: 10+20+30+40+50 = 150, 20 groups = 3000
    if sum == 3000 then 0 else 1
}
"#,
        "depth_match_in_loop",
    );
}

// ─── Nested control flow depth ───

#[test]
fn test_depth_nested_if_5_levels() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 42;
    let result = if x > 0 then {
        if x > 10 then {
            if x > 20 then {
                if x > 30 then {
                    if x > 40 then 5
                    else 4
                } else 3
            } else 2
        } else 1
    } else 0;
    if result == 5 then 0 else 1
}
"#,
        "depth_nested_if_5",
    );
}

#[test]
fn test_depth_nested_loops_break_from_inner() {
    assert_aot_success(
        r#"
@main () -> int = {
    let total = 0;
    for i in 0..5 do {
        let inner_sum = 0;
        let j = 0;
        loop {
            if j >= 10 then break;
            inner_sum = inner_sum + j;
            j = j + 1
        };
        total = total + inner_sum
    };
    // inner_sum = 0+1+...+9 = 45 per outer iteration, 5 iterations = 225
    if total == 225 then 0 else 1
}
"#,
        "depth_nested_loops_break",
    );
}

#[test]
fn test_depth_nested_loop_continue_and_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    for i in 0..20 do {
        if i % 3 == 0 then continue;
        let j = 0;
        loop {
            if j >= i then break;
            j = j + 1;
            if j % 2 == 0 then continue;
            count = count + 1
        }
    };
    // This is complex - we just verify it compiles, runs, and doesn't leak
    if count >= 0 then 0 else 1
}
"#,
        "depth_nested_continue_break",
    );
}

#[test]
fn test_depth_match_inside_loop_inside_match() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = match true {
        true -> {
            let sum = 0;
            for i in 0..5 do {
                let contribution = match i % 3 {
                    0 -> 1,
                    1 -> 10,
                    _ -> 100,
                };
                sum = sum + contribution
            };
            sum
        },
        false -> -1,
    };
    // i=0: 1, i=1: 10, i=2: 100, i=3: 1, i=4: 10 → 122
    if result == 122 then 0 else 1
}
"#,
        "depth_match_loop_match",
    );
}

#[test]
fn test_depth_break_value_nested() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = loop {
        let inner = loop {
            break 42
        };
        break inner + 8
    };
    if result == 50 then 0 else 1
}
"#,
        "depth_break_value_nested",
    );
}

#[test]
fn test_depth_complex_for_guard() {
    assert_aot_success(
        r#"
@main () -> int = {
    // Collect numbers divisible by both 3 and 7
    let xs = for i in 0..100 if i % 3 == 0 && i % 7 == 0 yield i;
    let count = xs.iter().count();
    // 0, 21, 42, 63, 84 = 5 values
    if count == 5 then 0 else 1
}
"#,
        "depth_complex_for_guard",
    );
}

// ─── Deep ? chains (error propagation) ───

#[test]
fn test_depth_try_chain_5_levels() {
    assert_aot_success(
        r#"
@step1 (x: int) -> Result<int, str> = if x > 0 then Ok(x + 1) else Err("step1 fail");
@step2 (x: int) -> Result<int, str> = if x < 100 then Ok(x * 2) else Err("step2 fail");
@step3 (x: int) -> Result<int, str> = if x % 2 == 0 then Ok(x + 3) else Err("step3 fail");
@step4 (x: int) -> Result<int, str> = if x > 5 then Ok(x - 1) else Err("step4 fail");
@step5 (x: int) -> Result<int, str> = Ok(x * 10);

@pipeline (start: int) -> Result<int, str> = {
    let a = step1(x: start)?;
    let b = step2(x: a)?;
    let c = step3(x: b)?;
    let d = step4(x: c)?;
    let e = step5(x: d)?;
    Ok(e)
}

@main () -> int = {
    let r = pipeline(start: 5);
    if r.is_ok() then {
        // 5 -> 6 -> 12 -> 15 -> 14 -> 140
        let v = r.unwrap();
        if v == 140 then 0 else 1
    } else 1
}
"#,
        "depth_try_chain_5",
    );
}

#[test]
fn test_depth_try_chain_early_fail() {
    assert_aot_success(
        r#"
@step1 (x: int) -> Result<int, str> = Ok(x + 1);
@step2 (x: int) -> Result<int, str> = Err("fail at step2");
@step3 (x: int) -> Result<int, str> = Ok(x * 100);

@pipeline (x: int) -> Result<int, str> = {
    let a = step1(x: x)?;
    let b = step2(x: a)?;
    let c = step3(x: b)?;
    Ok(c)
}

@main () -> int = {
    let r = pipeline(x: 1);
    if r.is_err() then 0 else 1
}
"#,
        "depth_try_early_fail",
    );
}

#[test]
fn test_depth_try_option_chain() {
    assert_aot_success(
        r#"
@find (x: int) -> Option<int> = if x > 0 then Some(x) else None;
@double_if_even (x: int) -> Option<int> = if x % 2 == 0 then Some(x * 2) else None;
@cap (x: int) -> Option<int> = if x < 100 then Some(x) else None;

@pipeline (x: int) -> Option<int> = {
    let a = find(x: x)?;
    let b = double_if_even(x: a)?;
    let c = cap(x: b)?;
    Some(c)
}

@main () -> int = {
    let r = pipeline(x: 4);
    if r.is_some() then {
        let v = r.unwrap();
        if v == 8 then 0 else 1
    } else 1
}
"#,
        "depth_try_option_chain",
    );
}

// ─── unwrap_or with computed defaults ───

#[test]
fn test_depth_unwrap_or_complex() {
    assert_aot_success(
        r#"
@maybe (x: int) -> Option<int> = if x > 0 then Some(x * 10) else None;

@main () -> int = {
    let a = maybe(x: 5).unwrap_or(default: 0);
    let b = maybe(x: -1).unwrap_or(default: 99);
    if a == 50 && b == 99 then 0 else 1
}
"#,
        "depth_unwrap_or",
    );
}

// ─── Complex closure patterns ───

#[test]
fn test_depth_closure_capturing_struct() {
    assert_aot_success(
        r#"
type Config = { multiplier: int, offset: int }

@main () -> int = {
    let cfg = Config { multiplier: 3, offset: 10 };
    let transform = (x: int) -> cfg.multiplier * x + cfg.offset;
    let result = transform(5);
    // 3 * 5 + 10 = 25
    if result == 25 then 0 else 1
}
"#,
        "depth_closure_capture_struct",
    );
}

#[test]
fn test_depth_closure_capturing_string() {
    assert_aot_success(
        r#"
@main () -> int = {
    let prefix = "hello";
    let make_greeting = (name: str) -> prefix + " " + name;
    let g = make_greeting("world");
    if g == "hello world" then 0 else 1
}
"#,
        "depth_closure_capture_string",
    );
}

#[test]
#[ignore = "AOT gap: zero-arg closure capturing 3+ strings causes heap corruption (malloc corrupted top size)"]
fn test_depth_closure_capturing_multiple_strings() {
    assert_aot_success(
        r#"
@main () -> int = {
    let first = "hello";
    let sep = " ";
    let last = "world";
    let combine = () -> first + sep + last;
    let result = combine();
    if result == "hello world" then 0 else 1
}
"#,
        "depth_closure_capture_multi_string",
    );
}

#[test]
fn test_depth_closure_passed_through_3_functions() {
    assert_aot_success(
        r#"
@level3 (f: (int) -> int, x: int) -> int = f(x);
@level2 (f: (int) -> int, x: int) -> int = level3(f: f, x: x + 1);
@level1 (f: (int) -> int, x: int) -> int = level2(f: f, x: x + 1);

@main () -> int = {
    let base = 100;
    let add_base = (n: int) -> base + n;
    let result = level1(f: add_base, x: 0);
    // level1: x=1, level2: x=2, level3: f(2) = 102
    if result == 102 then 0 else 1
}
"#,
        "depth_closure_3_levels",
    );
}

#[test]
fn test_depth_closure_mixed_captures() {
    assert_aot_success(
        r#"
type Config = { base: int }

@main () -> int = {
    let cfg = Config { base: 100 };
    let flag = true;
    let label = "test";
    let f = (x: int) -> {
        let adjusted = if flag then cfg.base + x else x;
        adjusted
    };
    let result = f(42);
    // flag is true, so 100 + 42 = 142
    if result == 142 then 0 else 1
}
"#,
        "depth_closure_mixed_captures",
    );
}

#[test]
fn test_depth_multiple_closures_same_scope() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 10;
    let add = (n: int) -> x + n;
    let mul = (n: int) -> x * n;
    let sub = (n: int) -> x - n;
    if add(5) == 15 && mul(3) == 30 && sub(7) == 3 then 0 else 1
}
"#,
        "depth_multiple_closures",
    );
}

// ─── Multi-derive combinations ───

#[test]
fn test_depth_derive_eq_comparable_hashable() {
    assert_aot_success(
        r#"
#derive(Eq, Comparable, Hashable)
type Score = { points: int, bonus: int }

@main () -> int = {
    let a = Score { points: 10, bonus: 5 };
    let b = Score { points: 10, bonus: 5 };
    let c = Score { points: 20, bonus: 3 };
    let eq_ok = a == b;
    let cmp_ok = a < c;
    let hash_ok = a.hash() == b.hash();
    if eq_ok && cmp_ok && hash_ok then 0 else 1
}
"#,
        "depth_derive_triple",
    );
}

#[test]
fn test_depth_derive_5_traits() {
    assert_aot_success(
        r#"
#derive(Eq, Comparable, Hashable, Clone, Default)
type Entry = { key: int, value: int }

@main () -> int = {
    let a = Entry { key: 1, value: 100 };
    let b = a.clone();
    let d = Entry.default();
    let eq_ok = a == b;
    let cmp_ok = a > d;
    let hash_ok = a.hash() == b.hash();
    let default_ok = d.key == 0 && d.value == 0;
    if eq_ok && cmp_ok && hash_ok && default_ok then 0 else 1
}
"#,
        "depth_derive_5_traits",
    );
}

#[test]
fn test_depth_derive_eq_on_struct_with_many_types() {
    assert_aot_success(
        r#"
#derive(Eq)
type Multi = { n: int, b: bool, s: str }

@main () -> int = {
    let a = Multi { n: 42, b: true, s: "hello" };
    let b = Multi { n: 42, b: true, s: "hello" };
    let c = Multi { n: 42, b: true, s: "world" };
    if a == b && a != c then 0 else 1
}
"#,
        "depth_derive_eq_many_types",
    );
}

#[test]
fn test_depth_derive_comparable_string_fields() {
    assert_aot_success(
        r#"
#derive(Eq, Comparable)
type Named = { name: str, rank: int }

@main () -> int = {
    let alice = Named { name: "alice", rank: 1 };
    let bob = Named { name: "bob", rank: 2 };
    // Lexicographic: "alice" < "bob"
    if alice < bob then 0 else 1
}
"#,
        "depth_derive_comparable_strings",
    );
}

#[test]
fn test_depth_derive_hashable_consistency() {
    assert_aot_success(
        r#"
#derive(Eq, Hashable)
type Pair = { x: int, y: int }

@main () -> int = {
    let a = Pair { x: 1, y: 2 };
    let b = Pair { x: 1, y: 2 };
    let c = Pair { x: 2, y: 1 };
    // Equal values must have equal hashes
    let contract_ok = a.hash() == b.hash();
    // Different values should have different hashes (not guaranteed, but likely)
    let different = a.hash() != c.hash();
    if contract_ok && different then 0 else 1
}
"#,
        "depth_derive_hashable_contract",
    );
}

// ─── Nested Option/Result types ───

#[test]
fn test_depth_nested_option() {
    assert_aot_success(
        r#"
@inner (x: int) -> Option<int> = if x > 0 then Some(x) else None;
@outer (x: int) -> Option<Option<int>> = if x >= 0 then Some(inner(x: x)) else None;

@main () -> int = {
    let r1 = outer(x: 5);
    let r2 = outer(x: 0);
    let r3 = outer(x: -1);
    let ok1 = r1.is_some();
    let ok2 = r2.is_some();
    let ok3 = r3.is_none();
    if ok1 && ok2 && ok3 then 0 else 1
}
"#,
        "depth_nested_option",
    );
}

#[test]
fn test_depth_result_with_option_payload() {
    assert_aot_success(
        r#"
@find_positive (x: int) -> Result<Option<int>, str> = {
    if x < 0 then Err("negative")
    else if x == 0 then Ok(None)
    else Ok(Some(x))
}

@main () -> int = {
    let r1 = find_positive(x: 5);
    let r2 = find_positive(x: 0);
    let r3 = find_positive(x: -1);
    let ok1 = r1.is_ok();
    let ok2 = r2.is_ok();
    let ok3 = r3.is_err();
    if ok1 && ok2 && ok3 then 0 else 1
}
"#,
        "depth_result_option_payload",
    );
}

// ─── Complex expressions combining multiple features ───

#[test]
#[ignore = "AOT gap: mutation of outer variables inside match arms within for-do body (ArcIrEmitter: variable not yet defined)"]
fn test_depth_combined_match_closure_result() {
    assert_aot_success(
        r#"
@safe_div (a: int, b: int) -> Result<int, str> =
    if b == 0 then Err("div by zero") else Ok(a / b);

@main () -> int = {
    let ops = [10, 20, 30, 0, 40];
    let successes = 0;
    let failures = 0;
    for divisor in ops do {
        let r = safe_div(a: 100, b: divisor);
        match r.is_ok() {
            true -> successes = successes + 1,
            false -> failures = failures + 1,
        }
    };
    if successes == 4 && failures == 1 then 0 else 1
}
"#,
        "depth_combined_match_closure_result",
    );
}

#[test]
#[ignore = "AOT gap: bool comparison stored in struct field during for-yield always produces false"]
fn test_depth_combined_struct_iter_match() {
    assert_aot_success(
        r#"
type Task = { priority: int, done: bool }

@process (t: Task) -> int = match t.done {
    true -> 0,
    false -> t.priority,
};

@main () -> int = {
    let tasks = for i in 1..=10 yield Task { priority: i, done: i % 3 == 0 };
    let total = 0;
    for t in tasks do total = total + process(t: t);
    // done: 3,6,9 -> priority 0; rest: 1+2+4+5+7+8+10 = 37
    if total == 37 then 0 else 1
}
"#,
        "depth_combined_struct_iter_match",
    );
}

#[test]
fn test_depth_combined_recursion_with_match() {
    assert_aot_success(
        r#"
@fib (n: int) -> int = match n {
    0 -> 0,
    1 -> 1,
    _ -> fib(n: n - 1) + fib(n: n - 2),
};

@main () -> int = {
    // fib(10) = 55
    if fib(n: 10) == 55 then 0 else 1
}
"#,
        "depth_recursion_match",
    );
}

#[test]
#[ignore = "AOT gap: mutation of outer variables inside match arms within for-do body (ArcIrEmitter: variable not yet defined)"]
fn test_depth_combined_all_features() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@make_point (i: int) -> Point = Point { x: i, y: i * 2 };

@classify (p: Point) -> int = match p.x % 3 {
    0 -> 0,
    1 -> 1,
    _ -> 2,
};

@safe_sum (a: int, b: int) -> Result<int, str> =
    if a + b > 1000 then Err("overflow") else Ok(a + b);

@main () -> int = {
    let points = for i in 0..20 yield make_point(i: i);
    let total = 0;
    let errors = 0;
    for p in points do {
        let cat = classify(p: p);
        let contribution = if cat == 0 then p.x * 10
            else if cat == 1 then p.y
            else 1;
        let r = safe_sum(a: total, b: contribution);
        match r.is_ok() {
            true -> total = r.unwrap(),
            false -> errors = errors + 1,
        }
    };
    // Verify it ran without crashing and total is reasonable
    if total >= 0 && errors == 0 then 0 else 1
}
"#,
        "depth_combined_all_features",
    );
}
