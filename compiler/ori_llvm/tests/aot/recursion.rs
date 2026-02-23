//! Recursion AOT Tests
//!
//! Tests for direct recursion, mutual recursion, recursive patterns
//! with data structures, recursion with accumulators, and recursive
//! control flow patterns.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Basic recursion ───

#[test]
fn test_rec_factorial() {
    assert_aot_success(
        r#"
@factorial (n: int) -> int = {
    if n <= 1 then 1 else n * factorial(n: n - 1)
};

@main () -> int = {
    if factorial(n: 10) == 3628800 then 0 else 1
}
"#,
        "rec_factorial",
    );
}

#[test]
fn test_rec_fibonacci() {
    assert_aot_success(
        r#"
@fib (n: int) -> int = {
    if n <= 0 then 0
    else if n == 1 then 1
    else fib(n: n - 1) + fib(n: n - 2)
};

@main () -> int = {
    // fib(10) = 55
    if fib(n: 10) == 55 then 0 else 1
}
"#,
        "rec_fibonacci",
    );
}

#[test]
fn test_rec_sum_to() {
    assert_aot_success(
        r#"
@sum_to (n: int) -> int = {
    if n <= 0 then 0 else n + sum_to(n: n - 1)
};

@main () -> int = {
    // 1+2+...+100 = 5050
    if sum_to(n: 100) == 5050 then 0 else 1
}
"#,
        "rec_sum_to",
    );
}

#[test]
fn test_rec_power() {
    assert_aot_success(
        r#"
@power (base: int, exp: int) -> int = {
    if exp == 0 then 1
    else base * power(base: base, exp: exp - 1)
};

@main () -> int = {
    // 2^10 = 1024
    if power(base: 2, exp: 10) == 1024 then 0 else 1
}
"#,
        "rec_power",
    );
}

#[test]
fn test_rec_gcd() {
    assert_aot_success(
        r#"
@gcd (a: int, b: int) -> int = {
    if b == 0 then a else gcd(a: b, b: a % b)
};

@main () -> int = {
    if gcd(a: 48, b: 18) == 6 then 0 else 1
}
"#,
        "rec_gcd",
    );
}

// ─── Tail-recursive patterns (accumulator) ───

#[test]
fn test_rec_factorial_acc() {
    assert_aot_success(
        r#"
@fact_acc (n: int, acc: int) -> int = {
    if n <= 1 then acc else fact_acc(n: n - 1, acc: acc * n)
};

@main () -> int = {
    if fact_acc(n: 10, acc: 1) == 3628800 then 0 else 1
}
"#,
        "rec_fact_acc",
    );
}

#[test]
fn test_rec_sum_acc() {
    assert_aot_success(
        r#"
@sum_acc (n: int, acc: int) -> int = {
    if n <= 0 then acc else sum_acc(n: n - 1, acc: acc + n)
};

@main () -> int = {
    if sum_acc(n: 100, acc: 0) == 5050 then 0 else 1
}
"#,
        "rec_sum_acc",
    );
}

#[test]
fn test_rec_count_digits() {
    assert_aot_success(
        r#"
@count_digits (n: int, acc: int) -> int = {
    if n < 10 then acc + 1
    else count_digits(n: n / 10, acc: acc + 1)
};

@main () -> int = {
    if count_digits(n: 12345, acc: 0) == 5 then 0 else 1
}
"#,
        "rec_count_digits",
    );
}

// ─── Mutual recursion ───

#[test]
fn test_rec_is_even_odd() {
    assert_aot_success(
        r#"
@is_even (n: int) -> bool = {
    if n == 0 then true else is_odd(n: n - 1)
};

@is_odd (n: int) -> bool = {
    if n == 0 then false else is_even(n: n - 1)
};

@main () -> int = {
    if is_even(n: 10) && is_odd(n: 7) && !is_even(n: 3) then 0 else 1
}
"#,
        "rec_even_odd",
    );
}

#[test]
fn test_rec_mutual_countdown() {
    assert_aot_success(
        r#"
@count_a (n: int) -> int = {
    if n <= 0 then 0 else 1 + count_b(n: n - 1)
};

@count_b (n: int) -> int = {
    if n <= 0 then 0 else 1 + count_a(n: n - 1)
};

@main () -> int = {
    // Both should count to 20
    if count_a(n: 20) == 20 then 0 else 1
}
"#,
        "rec_mutual_countdown",
    );
}

// ─── Recursion with Result ───

#[test]
fn test_rec_safe_divide() {
    assert_aot_success(
        r#"
@safe_div (n: int, d: int) -> Result<int, str> = {
    if d == 0 then Err("division by zero")
    else Ok(n / d)
};

@repeated_halve (n: int, times: int) -> Result<int, str> = {
    if times <= 0 then Ok(n)
    else {
        let half = safe_div(n: n, d: 2)?;
        repeated_halve(n: half, times: times - 1)
    }
};

@main () -> int = {
    let r = repeated_halve(n: 64, times: 3);
    // 64 / 2 / 2 / 2 = 8
    if r.is_ok() && r.unwrap() == 8 then 0 else 1
}
"#,
        "rec_safe_divide",
    );
}

// ─── Recursion with match ───

#[test]
fn test_rec_match_countdown() {
    assert_aot_success(
        r#"
@countdown (n: int) -> int = {
    match n {
        0 -> 0,
        _ -> 1 + countdown(n: n - 1),
    }
};

@main () -> int = {
    if countdown(n: 50) == 50 then 0 else 1
}
"#,
        "rec_match_countdown",
    );
}

#[test]
fn test_rec_match_collatz() {
    assert_aot_success(
        r#"
@collatz_steps (n: int, steps: int) -> int = {
    match n {
        1 -> steps,
        _ -> {
            let next = if n % 2 == 0 then n / 2 else n * 3 + 1;
            collatz_steps(n: next, steps: steps + 1)
        },
    }
};

@main () -> int = {
    // Collatz sequence for 27 takes 111 steps
    if collatz_steps(n: 27, steps: 0) == 111 then 0 else 1
}
"#,
        "rec_collatz",
    );
}

// ─── Recursion depth ───

#[test]
fn test_rec_depth_100() {
    assert_aot_success(
        r#"
@depth (n: int) -> int = {
    if n <= 0 then 0 else 1 + depth(n: n - 1)
};

@main () -> int = {
    if depth(n: 100) == 100 then 0 else 1
}
"#,
        "rec_depth_100",
    );
}

#[test]
fn test_rec_depth_1000() {
    assert_aot_success(
        r#"
@depth (n: int, acc: int) -> int = {
    if n <= 0 then acc else depth(n: n - 1, acc: acc + 1)
};

@main () -> int = {
    if depth(n: 1000, acc: 0) == 1000 then 0 else 1
}
"#,
        "rec_depth_1000",
    );
}

// ─── Recursion with structs ───

#[test]
fn test_rec_struct_param() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };

@move_towards_origin (p: Point) -> int = {
    if p.x == 0 && p.y == 0 then 0
    else {
        let nx = if p.x > 0 then p.x - 1 else if p.x < 0 then p.x + 1 else 0;
        let ny = if p.y > 0 then p.y - 1 else if p.y < 0 then p.y + 1 else 0;
        1 + move_towards_origin(p: Point { x: nx, y: ny })
    }
};

@main () -> int = {
    let steps = move_towards_origin(p: Point { x: 3, y: 4 });
    // max(3, 4) = 4 steps to reach origin
    if steps == 4 then 0 else 1
}
"#,
        "rec_struct_param",
    );
}

// ─── Recursive computation patterns ───

#[test]
fn test_rec_binary_search() {
    assert_aot_success(
        r#"
@binary_search (target: int, lo: int, hi: int) -> int = {
    if lo > hi then -1
    else {
        let mid = lo + (hi - lo) / 2;
        if mid == target then mid
        else if mid < target then binary_search(target: target, lo: mid + 1, hi: hi)
        else binary_search(target: target, lo: lo, hi: mid - 1)
    }
};

@main () -> int = {
    // Search for 42 in range 0..100
    let idx = binary_search(target: 42, lo: 0, hi: 99);
    if idx == 42 then 0 else 1
}
"#,
        "rec_binary_search",
    );
}

#[test]
fn test_rec_ackermann() {
    assert_aot_success(
        r#"
@ack (m: int, n: int) -> int = {
    if m == 0 then n + 1
    else if n == 0 then ack(m: m - 1, n: 1)
    else ack(m: m - 1, n: ack(m: m, n: n - 1))
};

@main () -> int = {
    // ack(3, 3) = 61
    if ack(m: 3, n: 3) == 61 then 0 else 1
}
"#,
        "rec_ackermann",
    );
}

#[test]
fn test_rec_tower_of_hanoi_count() {
    assert_aot_success(
        r#"
@hanoi_moves (n: int) -> int = {
    if n <= 0 then 0
    else 1 + 2 * hanoi_moves(n: n - 1)
};

@main () -> int = {
    // 2^n - 1 moves: n=5 → 31
    if hanoi_moves(n: 5) == 31 then 0 else 1
}
"#,
        "rec_hanoi",
    );
}

// ─── Recursion with Option ───

#[test]
fn test_rec_find_first_above() {
    assert_aot_success(
        r#"
@find_above (start: int, limit: int, threshold: int) -> Option<int> = {
    if start >= limit then None
    else if start > threshold then Some(start)
    else find_above(start: start + 1, limit: limit, threshold: threshold)
};

@main () -> int = {
    let r = find_above(start: 0, limit: 100, threshold: 42);
    if r.is_some() && r.unwrap() == 43 then 0 else 1
}
"#,
        "rec_find_above",
    );
}
