---
title: "Testing"
description: "Mandatory testing, assertions, mocking, and test organization."
order: 12
part: "Program Structure"
---

# Testing

Testing isn't optional in Ori — it's part of compilation. Every function (except `@main`) must have at least one test. This guide covers comprehensive testing strategies.

## The Philosophy

Ori enforces what discipline alone cannot. If code compiles:
- It has tests
- Those tests pass
- Changing code tells you what broke

This isn't about process or policy — it's about the compiler refusing to produce code it can't verify.

## Test Declaration

### Basic Targeted Tests

Tests are bound to functions with the `tests` keyword:

```ori
@greet (name: str) -> str = `Hello, {name}!`;

@test_greet tests @greet () -> void = {
    assert_eq(actual: greet(name: "Alice"), expected: "Hello, Alice!");
    assert_eq(actual: greet(name: ""), expected: "Hello, !")
}
```

The key part is `tests @greet` — it binds the test to a specific function.

### Multiple Targets

One test can verify several related functions:

```ori
@encode (data: str) -> str = ...;
@decode (data: str) -> str = ...;

@test_encode_decode_roundtrip tests @encode tests @decode () -> void = {
    let original = "Hello, World!";
    let encoded = encode(data: original);
    let decoded = decode(data: encoded);
    assert_eq(actual: decoded, expected: original)
}
```

This is useful for testing inverse operations or related functionality.

### Free-Floating Tests

Some tests don't target specific functions — integration tests, system tests, or tests spanning many functions. Use `_` as the target:

```ori
@test_full_workflow tests _ () -> void = {
    let user = create_user(name: "Alice");
    let order = create_order(user_id: user.id, items: ["widget"]);
    let receipt = process_payment(order_id: order.id);
    assert_eq(actual: receipt.status, expected: "completed")
}
```

Free-floating tests:
- Don't run during `ori check` (only targeted tests run)
- Run during `ori test`
- Are useful for integration and end-to-end tests

## Assertions

Ori provides comprehensive assertions for different scenarios.

### Basic Assertions

```ori
// General condition
assert(condition: result > 0);

// Equality
assert_eq(actual: add(a: 2, b: 2), expected: 4);

// Inequality
assert_ne(actual: generate_id(), unexpected: "");
```

### Option Assertions

```ori
// Check for Some
assert_some(option: find_user(id: 1));

// Check for None
assert_none(option: find_user(id: -1));
```

### Result Assertions

```ori
// Check for Ok
assert_ok(result: parse_int(text: "42"));

// Check for Err
assert_err(result: parse_int(text: "not a number"));
```

### Panic Assertions

```ori
// Assert that code panics
assert_panics(f: () -> divide(a: 1, b: 0));

// Assert panic with specific message
assert_panics_with(f: () -> panic(msg: "oops"), msg: "oops");
```

Note that panic assertions take a thunk (zero-argument function) — the expression `() -> divide(a: 1, b: 0)` delays execution until the assertion evaluates it.

## Test Attributes

### Skip Tests

Skip tests that can't run in certain environments:

```ori
#skip("database not available in CI")
@test_database_connection tests @connect () -> void = {...}
```

### Expected Failure

Mark tests that are known to fail:

```ori
#fail("known bug, fix in progress")
@test_edge_case tests @process () -> void = {...}
```

The test still runs, but failure is expected. When the bug is fixed, the test passes and you should remove the attribute.

### Compile-Time Failure

Verify that invalid code is rejected:

```ori
#compile_fail("type mismatch")
@test_type_safety tests _ () -> void = {
    let x: int = "string";
    ()
}
```

Compile-fail tests verify the compiler rejects invalid code. The string specifies what error message should appear.

## Testing with Capabilities

One of Ori's biggest testing advantages is capability injection. Functions that use capabilities can be tested with mocks.

### Basic Mocking

```ori
@fetch_user (id: int) -> Result<User, Error> uses Http =
    Http.get(url: `/api/users/{id}`);

@test_fetch_user tests @fetch_user () -> void =
    with Http = MockHttp {
        responses: {
            "/api/users/1": `{"id": 1, "name": "Alice"}`,
            "/api/users/999": Error { code: 404, message: "Not found" },
        },
    } in {
        // Test successful fetch
        let result = fetch_user(id: 1);
        assert_ok(result: result);

        // Test error case
        let error_result = fetch_user(id: 999);
        assert_err(result: error_result)
    }
```

No network calls, no test databases, no flaky tests. The mock provides exactly the responses you specify.

### Testing Error Cases

Mock capabilities can simulate failures:

```ori
@test_fetch_profile_network_error tests @fetch_user_profile () -> void =
    with Http = MockHttp {
        responses: {},
        default_error: NetworkError { message: "Connection refused" },
    } in {
        let result = fetch_user_profile(id: 1);
        assert_err(result: result)
    }
```

### Testing Time-Dependent Code

```ori
@is_business_hours () -> bool uses Clock = {
    let now = Clock.now();
    now.hour >= 9 && now.hour < 17 && now.day_of_week != Saturday && now.day_of_week != Sunday
}

@test_business_hours tests @is_business_hours () -> void = {
    // Test during business hours — stateful handler with fixed time
    with Clock = handler(state: Instant.parse(s: "2024-01-15T10:30:00")) {
        now: (s) -> (s, s)
    } in assert(condition: is_business_hours());

    // Test after hours
    with Clock = handler(state: Instant.parse(s: "2024-01-15T20:00:00")) {
        now: (s) -> (s, s)
    } in assert(condition: !is_business_hours());

    // Test weekend
    with Clock = handler(state: Instant.parse(s: "2024-01-13T10:30:00")) {
        now: (s) -> (s, s)
    } in assert(condition: !is_business_hours())
}
```

### Testing Random Code

```ori
@generate_code () -> str uses Random = {
    let chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    for _ in 0..6 yield chars[Random.rand_int(min: 0, max: len(collection: chars) - 1)]
}

@test_generate_code tests @generate_code () -> void =
    with Random = MockRandom { sequence: [0, 1, 2, 3, 4, 5] } in {
        let code = generate_code();
        assert_eq(actual: code, expected: "ABCDEF")
    }
```

### Testing Pure Functions

Pure functions (no capabilities) are easiest to test:

```ori
@calculate_tax (amount: float, rate: float) -> float =
    amount * rate;

@test_calculate_tax tests @calculate_tax () -> void = {
    assert_eq(actual: calculate_tax(amount: 100.0, rate: 0.1), expected: 10.0);
    assert_eq(actual: calculate_tax(amount: 0.0, rate: 0.1), expected: 0.0);
    assert_eq(actual: calculate_tax(amount: 100.0, rate: 0.0), expected: 0.0)
}
```

No setup, no mocks, no cleanup. This is why pure functions are preferred when possible.

## Test Organization

### Inline Tests

For small modules, put tests right after the functions they test:

```ori
@add (a: int, b: int) -> int = a + b;

@test_add tests @add () -> void = {
    assert_eq(actual: add(a: 2, b: 3), expected: 5)
}

@subtract (a: int, b: int) -> int = a - b;

@test_subtract tests @subtract () -> void = {
    assert_eq(actual: subtract(a: 5, b: 3), expected: 2)
}
```

Benefits:
- Tests are right next to the code
- Easy to see what's tested
- Changes to function and test happen together

### Separate Test Files

For larger modules, use a `_test/` subdirectory:

```
src/
├── math.ori
├── user.ori
├── order.ori
└── _test/
    ├── math.test.ori
    ├── user.test.ori
    └── order.test.ori
```

**math.test.ori:**

```ori
use "../math" { add, subtract, multiply, divide };

@test_add tests @add () -> void = {
    assert_eq(actual: add(a: 2, b: 3), expected: 5);
    assert_eq(actual: add(a: -1, b: 1), expected: 0);
    assert_eq(actual: add(a: 0, b: 0), expected: 0)
}

@test_subtract tests @subtract () -> void = {
    assert_eq(actual: subtract(a: 5, b: 3), expected: 2);
    assert_eq(actual: subtract(a: 3, b: 5), expected: -2)
}
```

Benefits:
- Keeps main module focused on implementation
- Test files can access private items for thorough testing
- Easier to find all tests for a module

### Testing Private Functions

Test files in `_test/` can access private functions using the `::` prefix:

```ori
// In _test/math.test.ori
use "../math" { ::internal_helper, pub_function };

@test_internal tests ::internal_helper () -> void = {
    // Test the private helper
    assert_eq(actual: internal_helper(x: 5), expected: 10)
}
```

This lets you test implementation details without exposing them publicly.

## Dependency-Aware Testing

One of Ori's most powerful features is dependency-aware testing. When you change a function, the compiler knows which tests need to run.

### How It Works

```ori
// helpers.ori
pub @double (x: int) -> int = x * 2;

@test_double tests @double () -> void = {
    assert_eq(actual: double(x: 5), expected: 10)
}

// math.ori
use "./helpers" { double };

pub @quadruple (x: int) -> int = double(x: double(x: x));

@test_quadruple tests @quadruple () -> void = {
    assert_eq(actual: quadruple(x: 3), expected: 12)
}
```

If you change `double`, both `test_double` AND `test_quadruple` run. Why? Because `quadruple` depends on `double` — a bug in `double` could break `quadruple`.

### Checking Impact Before Changes

Use the Ori CLI to see what a change would affect:

```bash
ori impact helpers.ori::double
```

Output:

```
Functions that would be affected by changes to @double:
  helpers.ori::double
  math.ori::quadruple

Tests that would run:
  helpers.ori::test_double
  math.ori::test_quadruple
```

This helps you understand the blast radius before making changes.

### Tracing Test Failures

When a test fails, Ori can trace it back to the source:

```bash
ori why test_quadruple
```

Output:

```
test_quadruple failed because:
  → quadruple calls double
  → double was changed in commit abc123
  → double now returns x * 3 instead of x * 2
```

This makes debugging faster — you know exactly what changed and what broke.

## Running Tests

### During Development

```bash
# Compile and run affected tests
ori check main.ori

# Compile only (skip tests for quick syntax check)
ori check --no-test main.ori

# Run all tests
ori test

# Run only targeted tests (skip free-floating)
ori test --only-targeted
```

### In CI/CD

```bash
# Fail the build if any test fails
ori check --strict main.ori
```

The `--strict` flag makes test failures fail the build, perfect for CI pipelines.

### Incremental Testing

When you change a function, Ori runs only the tests that could be affected:

1. Tests directly targeting the changed function
2. Tests targeting functions that call the changed function (transitively)

This keeps the feedback loop fast. Change one function? Only its tests run. Not the whole test suite.

## Writing Good Tests

### Test Normal Cases

Test typical inputs:

```ori
@test_greet_normal tests @greet () -> void = {
    assert_eq(actual: greet(name: "Alice"), expected: "Hello, Alice!");
    assert_eq(actual: greet(name: "Bob"), expected: "Hello, Bob!")
}
```

### Test Edge Cases

Test boundary conditions:

```ori
@test_greet_edge_cases tests @greet () -> void = {
    assert_eq(actual: greet(name: ""), expected: "Hello, !");
    assert_eq(actual: greet(name: " "), expected: "Hello,  !")
}
```

### Test Error Cases

Test invalid inputs and failure modes:

```ori
@test_divide_errors tests @divide () -> void = {
    assert_panics(f: () -> divide(a: 1, b: 0));
    assert_eq(actual: divide(a: 0, b: 5), expected: 0)
}
```

### Use Descriptive Names

```ori
// Good: describes what's being tested
@test_add_returns_sum_of_positive_numbers tests @add () -> void = ...;
@test_add_handles_negative_numbers tests @add () -> void = ...;
@test_divide_by_zero_panics tests @divide () -> void = ...;

// Less helpful: generic names
@test_add tests @add () -> void = ...;
```

When a test fails, the name should tell you what broke.

### Test Edge Cases Thoroughly

```ori
@test_parse_int tests @parse_int () -> void = {
    // Normal cases
    assert_eq(actual: parse_int(text: "42"), expected: Ok(42));
    assert_eq(actual: parse_int(text: "-17"), expected: Ok(-17));

    // Edge cases
    assert_eq(actual: parse_int(text: "0"), expected: Ok(0));
    assert_eq(actual: parse_int(text: "-0"), expected: Ok(0));

    // Boundaries
    assert_ok(result: parse_int(text: "9223372036854775807"));
    assert_err(result: parse_int(text: "9223372036854775808"));

    // Error cases
    assert_err(result: parse_int(text: ""));
    assert_err(result: parse_int(text: "abc"));
    assert_err(result: parse_int(text: "12abc"));
    assert_err(result: parse_int(text: "  42"))
}
```

## Complete Example

```ori
type User = { id: int, name: str, email: str }
type UserError = InvalidEmail(email: str) | NotFound(id: int);

impl Printable for UserError {
    @to_str (self) -> str = match self {
        InvalidEmail(email) -> `Invalid email: {email}`
        NotFound(id) -> `User {id} not found`
    };
}

// Validation function
@validate_email (email: str) -> bool =
    email.contains(substring: "@") && email.contains(substring: ".");

@test_validate_email tests @validate_email () -> void = {
    // Valid emails
    assert(condition: validate_email(email: "user@example.com"));
    assert(condition: validate_email(email: "a@b.c"));

    // Invalid emails
    assert(condition: !validate_email(email: "invalid"));
    assert(condition: !validate_email(email: "@example.com"));
    assert(condition: !validate_email(email: "user@"));
    assert(condition: !validate_email(email: ""))
}

// User creation with validation
@create_user (name: str, email: str) -> Result<User, UserError> = {
    if !validate_email(email: email) then
        return Err(InvalidEmail(email: email));
    Ok(User { id: generate_id(), name, email })
}

// Simulated ID generator
@generate_id () -> int uses Random = Random.rand_int(min: 1, max: 1000000);

@test_generate_id tests @generate_id () -> void =
    with Random = MockRandom { sequence: [42, 100, 999] } in {
        assert_eq(actual: generate_id(), expected: 42);
        assert_eq(actual: generate_id(), expected: 100);
        assert_eq(actual: generate_id(), expected: 999)
    }

@test_create_user_success tests @create_user () -> void =
    with Random = MockRandom { sequence: [123] } in {
        let result = create_user(name: "Alice", email: "alice@example.com");
        assert_ok(result: result);
        match result {
            Ok(user) -> {
                assert_eq(actual: user.id, expected: 123);
                assert_eq(actual: user.name, expected: "Alice");
                assert_eq(actual: user.email, expected: "alice@example.com")
            }
            Err(_) -> panic(msg: "Expected Ok")
        }
    }

@test_create_user_invalid_email tests @create_user () -> void =
    with Random = MockRandom { sequence: [] } in {
        let result = create_user(name: "Bob", email: "invalid");
        assert_err(result: result);
        match result {
            Err(InvalidEmail(email)) -> assert_eq(actual: email, expected: "invalid")
            _ -> panic(msg: "Expected InvalidEmail")
        }
    }

// User lookup with database
@find_user (id: int) -> Result<User, UserError> uses Database =
    Database.get(table: "users", id: id)
        .ok_or(error: NotFound(id: id));

@test_find_user tests @find_user () -> void =
    with Database = MockDatabase {
        tables: {
            "users": {
                1: User { id: 1, name: "Alice", email: "alice@example.com" },
            },
        },
    } in {
        // Found
        let result = find_user(id: 1);
        assert_ok(result: result);

        // Not found
        let missing = find_user(id: 999);
        assert_err(result: missing)
    }

// Integration test
@test_user_workflow tests _ () -> void =
    with Random = MockRandom { sequence: [456] },
    Database = MockDatabase { tables: {} } in {
        // Create a user
        let result = create_user(name: "Charlie", email: "charlie@test.com");
        assert_ok(result: result)

        // Note: This is a simplified example - real integration tests
        // would save to database and then find
    }
```

## Quick Reference

### Test Syntax

```ori
// Targeted test
@test_name tests @target () -> void = {...}

// Multiple targets
@test_name tests @fn1 tests @fn2 () -> void = {...}

// Free-floating test
@test_name tests _ () -> void = {...}

// With capability mock
@test_name tests @target () -> void =
    with Capability = Mock { ... } in {...}
```

### Assertions

```ori
assert(condition: bool);
assert_eq(actual: T, expected: T);
assert_ne(actual: T, unexpected: T);
assert_some(option: Option<T>);
assert_none(option: Option<T>);
assert_ok(result: Result<T, E>);
assert_err(result: Result<T, E>);
assert_panics(f: () -> T);
assert_panics_with(f: () -> T, msg: str);
```

### Attributes

```ori
#skip("reason")
@test_name tests @fn () -> void = ...;

#fail("expected error")
@test_name tests @fn () -> void = ...;

#compile_fail("error substring")
@test_name tests _ () -> void = ...;
```

### CLI Commands

```bash
ori check file.ori          # Compile + run affected tests
ori check --no-test         # Compile only
ori check --strict          # Fail build on test failure (CI)
ori test                    # Run all tests
ori test --only-targeted    # Only targeted tests
ori impact fn_name          # Show change impact
ori why test_name           # Trace test failure
```

## What's Next

Now that you understand testing:

- **[Capabilities](/guide/13-capabilities)** — Explicit effects and testing
- **[Concurrency](/guide/14-concurrency)** — Parallel execution patterns

