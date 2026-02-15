use super::*;

#[test]
fn test_shallow_recursion() {
    fn factorial(n: u64) -> u64 {
        ensure_sufficient_stack(|| if n <= 1 { 1 } else { n * factorial(n - 1) })
    }

    assert_eq!(factorial(10), 3_628_800);
}

#[test]
fn test_deep_recursion() {
    // This would overflow without stack growth
    fn deep_recurse(n: u64) -> u64 {
        ensure_sufficient_stack(|| if n == 0 { 0 } else { deep_recurse(n - 1) + 1 })
    }

    // 100k recursions - would overflow a typical 8MB stack
    assert_eq!(deep_recurse(100_000), 100_000);
}

#[test]
fn test_returns_closure_result() {
    let result = ensure_sufficient_stack(|| 42);
    assert_eq!(result, 42);
}

#[test]
fn test_works_with_result_type() {
    let result: Result<i32, &str> = ensure_sufficient_stack(|| Ok(123));
    assert_eq!(result, Ok(123));
}
