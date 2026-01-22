# Test Syntax

This document covers Sigil's test syntax: the `tests` keyword, assertion functions, organizing tests across files, and test file conventions.

---

## The `tests` Keyword

Tests are functions that declare which function they test:

```sigil
@test_name tests @target_function () -> void = expression
```

### Syntax Breakdown

| Component | Description |
|-----------|-------------|
| `@test_name` | Name of the test function (must start with `@`) |
| `tests` | Keyword linking test to target |
| `@target_function` | The function being tested |
| `() -> void` | Test signature (always takes no arguments, returns void) |
| `= expression` | Test body (typically a `run` pattern) |

### Basic Example

```sigil
@add (a: int, b: int) -> int = a + b

@test_add tests @add () -> void = run(
    assert_eq(
        .actual: add(.a: 2, .b: 3),
        .expected: 5,
    ),
    assert_eq(
        .actual: add(.a: -1, .b: 1),
        .expected: 0,
    ),
    assert_eq(
        .actual: add(.a: 0, .b: 0),
        .expected: 0,
    ),
)
```

The `tests` keyword creates a formal relationship between `@test_add` and `@add`. The compiler uses this to:

1. Track test coverage for `@add`
2. Run `@test_add` when testing `@add`
3. Display test status in tooling

---

## Test Function Naming

### Convention

Test function names should follow the pattern:

```
@test_<target>_<description>
```

Examples:

```sigil
@test_add                    // Basic test
@test_add_positive          // Tests positive numbers
@test_add_negative          // Tests negative numbers
@test_add_overflow          // Tests overflow behavior
@test_factorial_base_cases  // Tests base cases
@test_fibonacci_sequence    // Tests sequence values
```

### Multiple Tests Per Function

A function can have multiple tests, each testing different aspects:

```sigil
@factorial (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: 1,
    .step: n * self(n - 1)
)

@test_factorial_base tests @factorial () -> void = run(
    assert_eq(
        .actual: factorial(.n: 0),
        .expected: 1,
    ),
    assert_eq(
        .actual: factorial(.n: 1),
        .expected: 1,
    ),
)

@test_factorial_small tests @factorial () -> void = run(
    assert_eq(
        .actual: factorial(.n: 2),
        .expected: 2,
    ),
    assert_eq(
        .actual: factorial(.n: 3),
        .expected: 6,
    ),
    assert_eq(
        .actual: factorial(.n: 4),
        .expected: 24,
    ),
    assert_eq(
        .actual: factorial(.n: 5),
        .expected: 120,
    ),
)

@test_factorial_large tests @factorial () -> void = run(
    assert_eq(
        .actual: factorial(.n: 10),
        .expected: 3628800,
    ),
    assert_eq(
        .actual: factorial(.n: 12),
        .expected: 479001600,
    ),
)
```

All three tests contribute to `@factorial`'s test coverage.

### Testing Multiple Functions

A single test can declare multiple targets:

```sigil
@parse (input: str) -> Ast = ...
@format (ast: Ast) -> str = ...

// Tests both @parse and @format
@test_round_trip tests @parse tests @format () -> void = run(
    let original = "x + 1",
    let ast = parse(.input: original),
    let result = format(.ast: ast),
    assert_eq(
        .actual: result,
        .expected: original,
    ),
)
```

Both `@parse` and `@format` are marked as having test coverage.

---

## Skipping Tests

Sometimes a test cannot run - perhaps it depends on a feature not yet implemented, or it's flaky and needs investigation. Use the `#[skip]` attribute:

```sigil
#[skip("tuple destructuring not yet supported")]
@test_destructure tests @parse () -> void = run(
    let (a, b) = parse("1, 2"),
    assert_eq(.actual: a, .expected: 1),
)
```

### Syntax

```sigil
#[skip("reason")]
@test_name tests @target () -> void = ...
```

The reason string is required - it documents why the test is skipped.

### Behavior

A skipped test:

1. **Is still parsed and type-checked** - syntax errors are caught
2. **Is not executed** - the test body doesn't run
3. **Counts as coverage** - target functions are considered tested
4. **Appears in output** - visible but marked as skipped

### Output

```
Running tests...
  ✓ @test_add (3 assertions)
  ⊘ @test_destructure (skipped: tuple destructuring not yet supported)
  ✓ @test_multiply (2 assertions)

2 passed, 0 failed, 1 skipped
```

### When to Use Skip

- **Blocked features**: Test depends on unimplemented functionality
- **Known issues**: Test exposed a bug being fixed
- **Platform-specific**: Test only works on certain platforms
- **Temporary**: Test needs refactoring

### When NOT to Use Skip

- **Permanently broken**: Delete the test or fix the code
- **Slow tests**: Use a different mechanism for slow test filtering
- **Conditional logic**: Use `if` in the test body instead

---

## Assertion Functions

Sigil provides built-in assertion functions for tests.

### assert

Basic boolean assertion:

```sigil
assert(.cond: condition)
```

Fails if `condition` is `false`.

```sigil
@test_is_positive tests @is_positive () -> void = run(
    assert(.cond: is_positive(.n: 5)),
    assert(.cond: is_positive(.n: 1)),
    assert(.cond: !is_positive(.n: 0)),
    assert(.cond: !is_positive(.n: -1)),
)
```

### assert_eq

Equality assertion with better error messages:

```sigil
assert_eq(
    .actual: actual,
    .expected: expected,
)
```

Fails if `actual != expected`, showing both values.

```sigil
@test_add tests @add () -> void = run(
    assert_eq(
        .actual: add(.a: 2, .b: 3),
        .expected: 5,
    ),
    assert_eq(
        .actual: add(.a: -1, .b: 1),
        .expected: 0,
    ),
)
```

On failure:

```
assertion failed: add(2, 3) == 5
  actual:   4
  expected: 5
```

### assert_ne

Inequality assertion:

```sigil
assert_ne(
    .actual: actual,
    .unexpected: unexpected,
)
```

Fails if `actual == unexpected`.

```sigil
@test_random tests @random () -> void = run(
    let a = random(.max: 100),
    let b = random(.max: 100),
    // Highly likely to be different
    assert_ne(
        .actual: a,
        .unexpected: b,
    ),
)
```

### assert_some

Option assertion:

```sigil
assert_some(.option: option)
```

Fails if `option` is `None`.

```sigil
@test_find tests @find () -> void = run(
    let result = find(
        .items: [1, 2, 3],
        .target: 2,
    ),
    assert_some(.option: result),
    assert_eq(
        .actual: result,
        .expected: Some(1),  // index 1
    ),
)
```

### assert_none

Inverse of `assert_some`:

```sigil
assert_none(.option: option)
```

Fails if `option` is `Some(...)`.

```sigil
@test_find_missing tests @find () -> void = run(
    let result = find(
        .items: [1, 2, 3],
        .target: 99,
    ),
    assert_none(.option: result),
)
```

### assert_ok

Result assertion:

```sigil
assert_ok(.result: result)
```

Fails if `result` is `Err(...)`.

```sigil
@test_parse tests @parse () -> void = run(
    let result = parse(.input: "42"),
    assert_ok(.result: result),
    assert_eq(
        .actual: result,
        .expected: Ok(42),
    ),
)
```

### assert_err

Inverse of `assert_ok`:

```sigil
assert_err(.result: result)
```

Fails if `result` is `Ok(...)`.

```sigil
@test_parse_invalid tests @parse () -> void = run(
    let result = parse(.input: "not a number"),
    assert_err(.result: result),
)
```

### assert_panics

Assert that an expression panics:

```sigil
assert_panics(.expr: expression)
```

Fails if `expression` completes without panicking.

```sigil
@get (items: [int], index: int) -> int =
    if index < 0 || index >= len(.of: items) then panic(.msg: "index out of bounds")
    else items[index]

@test_get_panics tests @get () -> void = run(
    assert_panics(.expr: get(.items: [], .index: 0)),
    assert_panics(.expr: get(.items: [1, 2, 3], .index: -1)),
    assert_panics(.expr: get(.items: [1, 2, 3], .index: 5)),
)
```

### assert_panics_with

Assert panic with specific message:

```sigil
assert_panics_with(
    .expr: expression,
    .msg: "expected message",
)
```

```sigil
@test_get_panic_message tests @get () -> void = run(
    assert_panics_with(
        .expr: get(.items: [], .index: 0),
        .msg: "index out of bounds",
    ),
)
```

---

## Test Body Structure

### Using the run Pattern

Most tests use the `run` pattern to sequence assertions:

```sigil
@test_example tests @target () -> void = run(
    // Setup
    let input = prepare_data(),

    // Execute
    let result = target(.input: input),

    // Assert
    assert(.cond: result.success),
    assert_eq(
        .actual: result.value,
        .expected: expected,
    ),
)
```

### Simple Tests

For single assertions, the `run` pattern is optional:

```sigil
@double (n: int) -> int = n * 2

// Single assertion - no run needed
@test_double tests @double () -> void = assert_eq(
    .actual: double(.n: 5),
    .expected: 10,
)
```

### Multiple Assertions

Group related assertions with `run`:

```sigil
@test_user_creation tests @create_user () -> void = run(
    let user = create_user(
        .name: "Alice",
        .email: "alice@example.com",
    ),

    // Verify all fields
    assert_eq(
        .actual: user.name,
        .expected: "Alice",
    ),
    assert_eq(
        .actual: user.email,
        .expected: "alice@example.com",
    ),
    assert(.cond: user.id > 0),
    assert_none(.option: user.avatar),
)
```

### Testing with Setup

Use `run` for setup and teardown:

```sigil
@test_database tests @query () -> void = run(
    // Setup
    let db = create_test_db(),
    insert(.db: db, .record: {id: 1, name: "Alice"}),
    insert(.db: db, .record: {id: 2, name: "Bob"}),

    // Test
    let result = query(.db: db, .sql: "SELECT * WHERE id = 1"),
    assert_eq(
        .actual: len(.of: result),
        .expected: 1,
    ),
    assert_eq(
        .actual: result[0].name,
        .expected: "Alice",
    ),

    // Cleanup happens automatically (db dropped at end of scope)
)
```

---

## Test File Convention

### Directory Structure

Test files live in a `_test/` subdirectory:

```
src/
  math.si                  # Source file
  utils.si                 # Source file
  _test/
    math.test.si           # Tests for math.si
    utils.test.si          # Tests for utils.si
```

### Naming Convention

Test files use the `.test.si` extension:

```
<module>.test.si
```

Examples:
- `math.test.si` - Tests for `math.si`
- `http_client.test.si` - Tests for `http_client.si`
- `parser.test.si` - Tests for `parser.si`

### File Structure

A test file imports the module it tests:

```sigil
// _test/math.test.si

use math { add, multiply, divide }

@test_add tests @add () -> void = run(
    assert_eq(
        .actual: add(.a: 2, .b: 3),
        .expected: 5,
    ),
    assert_eq(
        .actual: add(.a: -1, .b: 1),
        .expected: 0,
    ),
)

@test_multiply tests @multiply () -> void = run(
    assert_eq(
        .actual: multiply(.a: 3, .b: 4),
        .expected: 12,
    ),
    assert_eq(
        .actual: multiply(.a: -2, .b: 3),
        .expected: -6,
    ),
)

@test_divide tests @divide () -> void = run(
    assert_eq(
        .actual: divide(.a: 10, .b: 2),
        .expected: Ok(5),
    ),
    assert_eq(
        .actual: divide(.a: 10, .b: 0),
        .expected: Err("division by zero"),
    ),
)
```

### Inline vs Separate Tests

Tests can be in the same file as the function:

```sigil
// math.si

@add (a: int, b: int) -> int = a + b

@test_add tests @add () -> void = run(
    assert_eq(
        .actual: add(.a: 2, .b: 3),
        .expected: 5,
    ),
)
```

Or in a separate test file:

```sigil
// math.si
@add (a: int, b: int) -> int = a + b
```

```sigil
// _test/math.test.si
use math { add }

@test_add tests @add () -> void = run(
    assert_eq(
        .actual: add(.a: 2, .b: 3),
        .expected: 5,
    ),
)
```

Both approaches satisfy the test requirement.

### Recommendation

- **Inline tests** - Good for small modules, keeps code and tests together
- **Separate files** - Better for larger modules, cleaner source files

The choice is stylistic. Sigil supports both.

---

## Running Tests

### Run All Tests

```bash
sigil test
```

Runs all tests in the project:

```
Running tests...
  ✓ @test_add (3 assertions)
  ✓ @test_multiply (2 assertions)
  ✓ @test_divide (2 assertions)
  ✓ @test_factorial_base (2 assertions)
  ✓ @test_factorial_small (4 assertions)

5/5 tests passed
```

### Run Specific File

```bash
sigil test src/_test/math.test.si
```

### Run Tests for Function

```bash
sigil test --function @factorial
```

Runs only tests that declare `tests @factorial`:

```
Running tests for @factorial...
  ✓ @test_factorial_base (2 assertions)
  ✓ @test_factorial_small (4 assertions)
  ✓ @test_factorial_large (2 assertions)

3/3 tests passed
```

### Test Output Formats

#### Human-Readable (Default)

```
Running tests...
  ✓ @test_add (3 assertions)
  ✗ @test_multiply
    assertion failed: multiply(3, 4) == 12
      actual:   11
      expected: 12
    at src/_test/math.test.si:8

1/2 tests passed
```

#### JSON Format

```bash
sigil test --format json
```

```json
{
  "tests": [
    {
      "name": "@test_add",
      "target": "@add",
      "status": "passed",
      "assertions": 3
    },
    {
      "name": "@test_multiply",
      "target": "@multiply",
      "status": "failed",
      "error": {
        "type": "assertion_failed",
        "expression": "multiply(3, 4) == 12",
        "actual": 11,
        "expected": 12,
        "location": {
          "file": "src/_test/math.test.si",
          "line": 8
        }
      }
    }
  ],
  "summary": {
    "total": 2,
    "passed": 1,
    "failed": 1
  }
}
```

JSON output enables AI to parse and reason about failures.

---

## Test Isolation

### Independent Tests

Each test runs in isolation:

```sigil
@test_a tests @target () -> void = run(
    let x = 1,
    assert_eq(
        .actual: x,
        .expected: 1,
    ),
)

@test_b tests @target () -> void = run(
    // x is not visible here - each test is independent
    let y = 2,
    assert_eq(
        .actual: y,
        .expected: 2,
    ),
)
```

### No Shared State

Tests cannot depend on execution order:

```sigil
// BAD - relies on global state
$counter = 0

@test_first tests @increment () -> void = run(
    increment(),
    assert_eq(
        .actual: $counter,
        .expected: 1,  // Might fail if test_second runs first!
    ),
)

@test_second tests @increment () -> void = run(
    increment(),
    assert_eq(
        .actual: $counter,
        .expected: 2,  // Order-dependent!
    ),
)
```

Sigil's immutable-by-default design prevents most shared state issues.

### Parallel Execution

Tests run in parallel by default:

```bash
sigil test
```

```
Running tests (parallel)...
  ✓ @test_add
  ✓ @test_factorial
  ✓ @test_fibonacci
  ✓ @test_sort

4/4 tests passed in 0.12s
```

Use `--sequential` for debugging:

```bash
sigil test --sequential
```

---

## Test Helpers

### Shared Setup

Define helper functions for common setup:

```sigil
// _test/test_helpers.si

pub @create_test_user (name: str) -> User = User {
    id: 0,
    name: name,
    email: name + "@test.com",
    created_at: now()
}

pub @create_test_data () -> [int] = [1, 2, 3, 4, 5]
```

```sigil
// _test/user.test.si

use test_helpers { create_test_user }
use user { validate_user }

@test_validate_user tests @validate_user () -> void = run(
    let user = create_test_user(.name: "Alice"),
    let result = validate_user(.user: user),
    assert_ok(.result: result),
)
```

### Test Fixtures

For complex setup, use factory functions:

```sigil
// _test/fixtures.si

pub @fixture_empty_db () -> Database = Database.new(":memory:")

pub @fixture_populated_db () -> Database = run(
    let db = Database.new(":memory:"),
    db.insert(User { id: 1, name: "Alice" }),
    db.insert(User { id: 2, name: "Bob" }),
    db,
)
```

---

## Assertion Best Practices

### Prefer assert_eq Over assert

```sigil
// Less informative on failure
assert(.cond: add(.a: 2, .b: 3) == 5)

// More informative - shows actual and expected
assert_eq(
    .actual: add(.a: 2, .b: 3),
    .expected: 5,
)
```

### One Assertion Per Concept

Group related assertions, but test one concept:

```sigil
// Good - testing user creation
@test_user_creation tests @create_user () -> void = run(
    let user = create_user(.name: "Alice"),
    assert_eq(
        .actual: user.name,
        .expected: "Alice",
    ),
    assert(.cond: user.id > 0),
    assert_none(.option: user.deleted_at),
)

// Bad - testing unrelated things
@test_everything tests @create_user tests @delete_user () -> void = run(
    let user = create_user(.name: "Alice"),
    delete_user(.user: user),
    // Testing too many things in one test
)
```

### Test Edge Cases

```sigil
@test_divide_edge_cases tests @divide () -> void = run(
    // Division by zero
    assert_eq(
        .actual: divide(.a: 10, .b: 0),
        .expected: Err("division by zero"),
    ),

    // Zero dividend
    assert_eq(
        .actual: divide(.a: 0, .b: 5),
        .expected: Ok(0),
    ),

    // Negative numbers
    assert_eq(
        .actual: divide(.a: -10, .b: 2),
        .expected: Ok(-5),
    ),
    assert_eq(
        .actual: divide(.a: 10, .b: -2),
        .expected: Ok(-5),
    ),
    assert_eq(
        .actual: divide(.a: -10, .b: -2),
        .expected: Ok(5),
    ),
)
```

### Descriptive Test Names

```sigil
// Vague
@test_1 tests @process () -> void = ...
@test_2 tests @process () -> void = ...

// Descriptive
@test_process_valid_input tests @process () -> void = ...
@test_process_empty_input tests @process () -> void = ...
@test_process_malformed_input tests @process () -> void = ...
```

---

## Error Messages

### Assertion Failure Format

When a test fails, the error includes:

```
assertion failed: <expression>
  actual:   <actual value>
  expected: <expected value>
  at <file>:<line>
```

Example:

```
assertion failed: factorial(5) == 120
  actual:   24
  expected: 120
  at src/_test/math.test.si:15
```

### Structured Error Output

For AI consumption:

```json
{
  "test": "@test_factorial",
  "status": "failed",
  "assertion": {
    "type": "assert_eq",
    "expression": "factorial(5) == 120",
    "actual": {
      "type": "int",
      "value": 24
    },
    "expected": {
      "type": "int",
      "value": 120
    }
  },
  "location": {
    "file": "src/_test/math.test.si",
    "line": 15,
    "address": "@test_factorial.assertions[0]"
  },
  "suggestions": [
    {
      "message": "factorial(5) should be 5*4*3*2*1 = 120, but got 24 (which is 4!). Check recursion step.",
      "confidence": "medium"
    }
  ]
}
```

---

## Summary

| Feature | Syntax |
|---------|--------|
| Test declaration | `@test_name tests @target () -> void = ...` |
| Multiple targets | `tests @a tests @b` |
| Skip test | `#[skip("reason")] @test_name tests @target ...` |
| Boolean assert | `assert(.cond: condition)` |
| Equality assert | `assert_eq(.actual: a, .expected: b)` |
| Inequality assert | `assert_ne(.actual: a, .unexpected: b)` |
| Option asserts | `assert_some(.option: opt)`, `assert_none(.option: opt)` |
| Result asserts | `assert_ok(.result: res)`, `assert_err(.result: res)` |
| Panic assert | `assert_panics(.expr: expr)` |
| Test files | `_test/<module>.test.si` |
| Run all tests | `sigil test` |
| Run for function | `sigil test --function @name` |
| JSON output | `sigil test --format json` |

---

## See Also

- [Mandatory Tests](01-mandatory-tests.md) - Why tests are required
- [Compile-Fail Tests](03-compile-fail-tests.md) - Testing expected errors
- [Patterns Overview](../02-syntax/03-patterns-overview.md) - The `run` pattern
- [Error Handling](../05-error-handling/index.md) - Result and Option types
