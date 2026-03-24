// Benchmark: Small (~30 lines)
// Exercises: recursion, arithmetic, loops, basic control flow
// Expected: compute-heavy with minimal allocation

fn fibonacci(n: i64) -> i64 {
    let mut a: i64 = 0;
    let mut b: i64 = 1;
    for _ in 0..n {
        let next = a + b;
        a = b;
        b = next;
    }
    a
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn collatz_steps(n: i64) -> i64 {
    let mut steps: i64 = 0;
    let mut val = n;
    for _ in 0..10000 {
        if val == 1 { break; }
        if val % 2 == 0 {
            val = val / 2;
        } else {
            val = val * 3 + 1;
        }
        steps += 1;
    }
    steps
}

fn main() -> std::process::ExitCode {
    let fib_result = fibonacci(30);
    let gcd_result = gcd(48, 18);
    let collatz = collatz_steps(27);

    if fib_result == 832040 && gcd_result == 6 && collatz == 111 {
        std::process::ExitCode::from(0)
    } else {
        std::process::ExitCode::from(1)
    }
}
