# Testing

This section defines the mandatory testing system.

## Test Requirement

Every function must have at least one associated test. It is a compile-time error if a function has no tests.

### Exemptions

The following are exempt from the test requirement:

1. The `@main` function
2. Test functions themselves (functions declared with `tests`)
3. Config variables
4. Type definitions
5. Trait definitions

## Test Declaration

### Syntax

```
test          = "@" identifier "tests" target { "tests" target } params "->" "void" "=" expression .
target        = "@" identifier .
params        = "(" ")" .
```

### Semantics

A test declaration:

1. Introduces a test function with the given name
2. Associates the test with one or more target functions via the `tests` keyword
3. Must have no parameters
4. Must return `void`

```sigil
@test_add tests @add () -> void = run(
    assert_eq(add(2, 3), 5),
    assert_eq(add(-1, 1), 0),
)
```

### Multiple Targets

A single test may cover multiple functions:

```sigil
@test_roundtrip tests @parse tests @format () -> void = run(
    let ast = parse("x + 1"),
    let output = format(ast),
    assert_eq(output, "x + 1"),
)
```

Both `@parse` and `@format` are considered tested.

## Test Functions

### Naming Convention

Test function names should begin with `test_`:

```sigil
@test_add tests @add () -> void = ...
@test_multiply tests @multiply () -> void = ...
```

### Test Body

A test body typically uses `run` to sequence assertions:

```sigil
@test_factorial tests @factorial () -> void = run(
    assert_eq(factorial(0), 1),
    assert_eq(factorial(1), 1),
    assert_eq(factorial(5), 120),
)
```

## Assertions

### assert

Assert that a condition is true:

```sigil
assert(x > 0)
assert(result.is_ok())
```

If the condition is false, the test fails.

### assert_eq

Assert that two values are equal:

```sigil
assert_eq(actual, expected)
assert_eq(add(2, 3), 5)
```

If the values are not equal, the test fails with a diagnostic showing both values.

### assert_ne

Assert that two values are not equal:

```sigil
assert_ne(actual, unexpected)
```

### assert_some / assert_none

Assert that an `Option` is `Some` or `None`:

```sigil
assert_some(option)
assert_none(option)
```

### assert_ok / assert_err

Assert that a `Result` is `Ok` or `Err`:

```sigil
assert_ok(result)
assert_err(result)
```

### assert_panics / assert_panics_with

Assert that evaluating an expression panics:

```sigil
assert_panics(expr)
assert_panics_with(expr, "message")
```

## Test Organization

### Inline Tests

Tests may be in the same file as the code they test:

```sigil
// math.si

@add (a: int, b: int) -> int = a + b

@test_add tests @add () -> void = run(
    assert_eq(add(2, 3), 5),
)
```

### Test Files

Tests may be in separate files in `_test/` directories:

```
src/
  math.si
  _test/
    math.test.si
```

```sigil
// _test/math.test.si

use '../math' { add }

@test_add tests @add () -> void = run(
    assert_eq(add(2, 3), 5),
)
```

Test files use relative imports to reference their parent module. The `'../math'` path explicitly shows the file location.

### Testing Private Items

Private items can be imported using the `::` prefix:

```sigil
// _test/math.test.si

use '../math' { add, ::helper }  // helper is private, :: makes access explicit

@test_helper tests @helper () -> void = run(
    assert_eq(helper(5), 10),
)
```

The `::` prefix works from any file, not just test files. See [Modules § Private Imports](12-modules.md#private-imports).

## Test Execution

### Running Tests

Tests are executed by the compiler as part of the build process, or explicitly via CLI:

```
sigil test           # run all tests
sigil test file.si   # run tests for specific file
```

### Test Isolation

Each test function runs in isolation. Tests may not depend on execution order or shared mutable state.

### Parallel Execution

Tests may be executed in parallel. Tests must not rely on sequential execution.

## Test Coverage

### Compiler Enforcement

The compiler tracks which functions have associated tests:

```
error[E0500]: function @multiply has no tests
  --> math.si:5:1
   |
 5 | @multiply (a: int, b: int) -> int = a * b
   | ^^^^^^^^^
   |
   = help: add a test with `@test_multiply tests @multiply () -> void = ...`
```

### Coverage Report

```
sigil check math.si
```

```
Function Coverage:
  @add          ✓  3 tests
  @multiply     ✗  0 tests (missing!)
  @divide       ✓  2 tests

error: 1 function missing tests
```

## Testing Effectful Code

Functions with capabilities are tested by providing mock implementations:

```sigil
@get_user (id: str) -> Result<User, Error> uses Http = try(
    let json = Http.get("/users/" + id)?,
    Ok(parse(json)),
)

@test_get_user tests @get_user () -> void =
    with Http = MockHttp { responses: {"/users/1": "{\"name\": \"Alice\"}"} } in
    run(
        let result = get_user("1"),
        assert_eq(result, Ok(User { name: "Alice" })),
    )
```

The `with ... in` construct provides a mock implementation for testing.

See [Capabilities](14-capabilities.md) for details.

## Compile-Fail Tests

Tests may verify that certain code fails to compile:

```sigil
// #compile-fail
// #error: type mismatch
let x: int = "hello"
```

The `#compile-fail` directive indicates the code should not compile. The `#error:` directive specifies the expected error.

## Test Output

### Success

```
Running tests...
  ✓ @test_add (3 assertions)
  ✓ @test_multiply (2 assertions)
All tests passed.
```

### Failure

```
Running tests...
  ✗ @test_add
    assertion failed: assert_eq
      actual:   6
      expected: 5
    at math.test.si:5

  ✓ @test_multiply (2 assertions)

1 test failed.
```

### JSON Output

For tooling integration:

```json
{
  "tests": [
    {
      "name": "@test_add",
      "status": "failed",
      "assertions": 3,
      "failure": {
        "type": "assert_eq",
        "actual": "6",
        "expected": "5",
        "location": {
          "file": "math.test.si",
          "line": 5
        }
      }
    }
  ]
}
```
