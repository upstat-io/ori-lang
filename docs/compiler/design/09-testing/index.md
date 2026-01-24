# Testing System Overview

The Sigil test system provides test discovery, parallel execution, and coverage tracking. Testing is mandatory in Sigil - every function requires tests.

## Location

```
compiler/sigilc/src/test/
├── mod.rs          # Module exports
├── runner.rs       # Test execution (~494 lines)
└── discovery.rs    # Test finding (~310 lines)
```

## Design Goals

1. **Mandatory testing** - Functions without tests fail compilation
2. **Parallel execution** - Tests run concurrently
3. **Targeted tests** - Tests declare what they test
4. **Fast feedback** - Quick test discovery and execution

## Test Types

### Targeted Tests

Test a specific function:

```sigil
@add (a: int, b: int) -> int = a + b

@test_add tests @add () -> void = run(
    assert_eq(actual: add(2, 3), expected: 5),
)
```

### Free-Floating Tests

Test multiple things or integration:

```sigil
@test_integration () -> void = run(
    let result = process_data(input),
    assert(cond: result.is_valid),
)
```

### Multi-Target Tests

Test multiple functions:

```sigil
@test_math tests @add tests @subtract () -> void = run(
    assert_eq(actual: add(1, 2), expected: 3),
    assert_eq(actual: subtract(5, 3), expected: 2),
)
```

## Test Attributes

### #[skip("reason")]

Skip a test:

```sigil
#[skip("not implemented yet")]
@test_future_feature () -> void = ...
```

### #[compile_fail("error")]

Expect compilation to fail:

```sigil
#[compile_fail("type mismatch")]
@test_type_error () -> void = run(
    let x: int = "not an int",
)
```

### #[fail("message")]

Expect test to fail at runtime:

```sigil
#[fail("assertion failed")]
@test_expected_failure () -> void = run(
    assert(cond: false),
)
```

## Test Output

```
Running 42 tests...

test @test_add ... ok (2ms)
test @test_subtract ... ok (1ms)
test @test_multiply ... FAILED (5ms)

  assertion failed: expected 6, got 5
    at src/mathsi:15:5

test @test_divide ... ok (1ms)
test @test_skip ... skipped (not implemented yet)

Results: 40 passed, 1 failed, 1 skipped
Coverage: 95% of functions tested
```

## Related Documents

- [Test Discovery](test-discovery.md) - Finding tests
- [Test Runner](test-runner.md) - Executing tests
