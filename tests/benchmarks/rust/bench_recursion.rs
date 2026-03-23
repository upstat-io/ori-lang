// Benchmark: Recursion-heavy
// Exercises: deep recursion, tree-like patterns, recursive data processing
// Measures: call overhead, tail-call optimization, stack frame management

fn ackermann(m: i64, n: i64) -> i64 {
    if m == 0 { n + 1 }
    else if n == 0 { ackermann(m - 1, 1) }
    else { ackermann(m - 1, ackermann(m, n - 1)) }
}

fn tak(x: i64, y: i64, z: i64) -> i64 {
    if y >= x { z }
    else {
        tak(
            tak(x - 1, y, z),
            tak(y - 1, z, x),
            tak(z - 1, x, y),
        )
    }
}

fn fib_recursive(n: i64) -> i64 {
    if n < 2 { n }
    else { fib_recursive(n - 1) + fib_recursive(n - 2) }
}

fn sum_recursive(n: i64) -> i64 {
    if n == 0 { 0 }
    else { n + sum_recursive(n - 1) }
}

fn power(base: i64, exp: i64) -> i64 {
    if exp == 0 { 1 }
    else if exp % 2 == 0 {
        let half = power(base, exp / 2);
        half * half
    } else { base * power(base, exp - 1) }
}

fn main() -> std::process::ExitCode {
    // Ackermann — exponential recursion (3,7 is tractable)
    let ack = ackermann(3, 7);

    // Takeuchi — triply recursive
    let tak_result = tak(24, 16, 8);

    // Naive fibonacci — exponential tree recursion
    let fib = fib_recursive(38);

    // Deep linear recursion (many times)
    let mut sum_total: i64 = 0;
    for _ in 0..100000 {
        sum_total += sum_recursive(100);
    }

    // Fast exponentiation
    let mut pow_total: i64 = 0;
    for _ in 0..1000000 {
        pow_total += power(2, 30);
    }

    if ack == 1021 && tak_result >= 0 && fib == 39088169 && sum_total > 0 && pow_total > 0 {
        std::process::ExitCode::from(0)
    } else {
        std::process::ExitCode::from(1)
    }
}
