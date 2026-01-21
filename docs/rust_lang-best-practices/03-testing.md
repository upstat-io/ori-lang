# Testing

Guidelines for testing Rust code based on official Rust documentation.

## Quick Reference

- [ ] Unit tests in `#[cfg(test)]` modules within source files
- [ ] Integration tests in `tests/` directory
- [ ] Doc tests for public API examples
- [ ] Use `#[test]` attribute for test functions
- [ ] Use `assert!`, `assert_eq!`, `assert_ne!` for assertions
- [ ] Use `#[should_panic]` for expected panics
- [ ] Run with `cargo test`

## Test Organization

### Unit Tests

Unit tests live in the same file as the code they test, inside a `#[cfg(test)]` module:

```rust
// src/lib.rs
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn test_add_negative() {
        assert_eq!(add(-1, 1), 0);
    }
}
```

The `#[cfg(test)]` attribute ensures the test module is only compiled when running tests.

### Integration Tests

Integration tests go in a separate `tests/` directory at the crate root:

```
my_crate/
├── src/
│   └── lib.rs
├── tests/
│   ├── integration_test.rs
│   └── common/
│       └── mod.rs    # Shared test utilities
└── Cargo.toml
```

```rust
// tests/integration_test.rs
use my_crate::add;

#[test]
fn test_add_integration() {
    assert_eq!(add(2, 2), 4);
}
```

Integration tests:
- Each file in `tests/` is compiled as a separate crate
- Can only test public API
- Don't need `#[cfg(test)]` attribute

### Shared Test Utilities

To share code between integration tests, use a `common/mod.rs` submodule:

```rust
// tests/common/mod.rs
pub fn setup() -> TestData {
    // shared setup code
}
```

```rust
// tests/integration_test.rs
mod common;

#[test]
fn test_with_setup() {
    let data = common::setup();
    // use data...
}
```

### Doc Tests

Documentation examples are tested automatically:

```rust
/// Adds two numbers together.
///
/// # Examples
///
/// ```
/// let result = my_crate::add(2, 3);
/// assert_eq!(result, 5);
/// ```
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

Doc tests ensure examples in documentation stay in sync with the code.

## Assertions

### Basic Assertions

```rust
#[test]
fn test_assertions() {
    // Boolean assertion
    assert!(true);
    assert!(1 + 1 == 2);
    assert!(condition, "Custom message: {}", value);

    // Equality
    assert_eq!(actual, expected);
    assert_eq!(actual, expected, "Values differ");

    // Inequality
    assert_ne!(a, b);
    assert_ne!(a, b, "Values should differ");
}
```

### Custom Failure Messages

All assertion macros accept optional format arguments:

```rust
#[test]
fn test_with_message() {
    let x = 5;
    let y = 10;
    assert!(x < y, "Expected {} < {}", x, y);
    assert_eq!(x + y, 15, "Sum of {} and {} should be 15", x, y);
}
```

## Testing Patterns

### Expected Panics

Use `#[should_panic]` for functions that should panic:

```rust
#[test]
#[should_panic]
fn test_panics() {
    panic!("This test should panic");
}

#[test]
#[should_panic(expected = "index out of bounds")]
fn test_specific_panic_message() {
    let v = vec![1, 2, 3];
    let _ = v[10];  // Panics with "index out of bounds"
}
```

### Testing Results

Tests can return `Result<(), E>` to use the `?` operator:

```rust
#[test]
fn test_result() -> Result<(), String> {
    let result = operation_that_might_fail()?;
    assert_eq!(result, expected_value);
    Ok(())
}
```

This is cleaner than unwrapping everywhere.

### Testing Error Conditions

```rust
#[test]
fn test_error_returned() {
    let result = parse("invalid input");
    assert!(result.is_err());
}

#[test]
fn test_specific_error() {
    let result = parse("invalid");
    match result {
        Err(ParseError::InvalidSyntax) => (), // expected
        _ => panic!("Expected InvalidSyntax error"),
    }
}
```

### Ignoring Tests

Use `#[ignore]` for tests that shouldn't run by default:

```rust
#[test]
#[ignore]
fn expensive_test() {
    // This test takes a long time
}
```

Run ignored tests with `cargo test -- --ignored`.

## Running Tests

```bash
# Run all tests
cargo test

# Run specific test by name
cargo test test_name

# Run tests containing a string
cargo test add

# Run tests in specific module
cargo test tests::

# Show output from passing tests
cargo test -- --nocapture

# Run ignored tests
cargo test -- --ignored

# Run all tests including ignored
cargo test -- --include-ignored

# Run tests sequentially (not in parallel)
cargo test -- --test-threads=1

# Run only doc tests
cargo test --doc

# Run only a specific integration test file
cargo test --test integration_test
```

## Test Output

By default, Rust captures stdout from passing tests. Use `--nocapture` to see output:

```rust
#[test]
fn test_with_output() {
    println!("This only shows with --nocapture");
    assert!(true);
}
```

```bash
cargo test -- --nocapture
```

## Guidelines

### Do

- Put unit tests in `#[cfg(test)]` modules in the same file as the code
- Put integration tests in `tests/` directory
- Test edge cases (empty input, boundary values, error conditions)
- Use descriptive test names that explain what is being tested
- Test both success and failure paths
- Keep tests focused on one behavior each

### Don't

- Don't test private implementation details (test through public API)
- Don't make tests depend on each other or on execution order
- Don't ignore flaky tests - fix them
- Don't test trivial code (simple getters with no logic)

## Resources

- [Writing Automated Tests - The Rust Book](https://doc.rust-lang.org/book/ch11-01-writing-tests.html)
- [Test Organization - The Rust Book](https://doc.rust-lang.org/book/ch11-03-test-organization.html)
- [Controlling How Tests Are Run - The Rust Book](https://doc.rust-lang.org/book/ch11-02-running-tests.html)
