// Same as bench_compute.rs but with #[inline(never)] on all functions

#[inline(never)]
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

#[inline(never)]
fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

#[inline(never)]
fn collatz_steps(n: i64) -> i64 {
    let mut steps: i64 = 0;
    let mut val = n;
    for _ in 0..10000000 {
        if val == 1 { break; }
        if val % 2 == 0 {
            val /= 2;
        } else {
            val = val * 3 + 1;
        }
        steps += 1;
    }
    steps
}

#[inline(never)]
fn sum_of_gcds(limit: i64) -> i64 {
    let mut total: i64 = 0;
    for i in 1..limit {
        for j in 1..limit {
            total += gcd(i, j);
        }
    }
    total
}

#[inline(never)]
fn is_prime(n: i64) -> bool {
    if n < 2 { return false; }
    if n < 4 { return true; }
    if n % 2 == 0 { return false; }
    let mut i: i64 = 3;
    while i * i <= n {
        if n % i == 0 { return false; }
        i += 2;
    }
    true
}

#[inline(never)]
fn count_primes(limit: i64) -> i64 {
    let mut count: i64 = 0;
    for n in 2..limit {
        if is_prime(n) { count += 1; }
    }
    count
}

fn main() -> std::process::ExitCode {
    let mut fib_sum: i64 = 0;
    for _ in 0..1000000 {
        fib_sum += fibonacci(40);
    }
    let gcds = sum_of_gcds(3000);
    let mut collatz_sum: i64 = 0;
    for n in 1..100000 {
        collatz_sum += collatz_steps(n);
    }
    let primes = count_primes(500000);

    if fib_sum > 0 && gcds > 0 && collatz_sum > 0 && primes > 9000 {
        std::process::ExitCode::from(0)
    } else {
        std::process::ExitCode::from(1)
    }
}
