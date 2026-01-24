# Mandatory Tests

This document covers Sigil's mandatory testing requirements: why testing is required, what's exempt, how the compiler enforces it, and why this matters for AI-authored code.

---

## Overview

In Sigil, **every function must have at least one test**. This is not a guideline or best practice---it's a compiler requirement. Code without tests fails to compile.

```sigil
@factorial (number: int) -> int = recurse(
    .condition: number <= 1,
    .base: 1,
    .step: number * self(number - 1)
)

// REQUIRED - compilation fails without this test
@test_factorial tests @factorial () -> void = run(
    assert_eq(
        .actual: factorial(.number: 0),
        .expected: 1,
    ),
    assert_eq(
        .actual: factorial(.number: 5),
        .expected: 120,
    ),
)
```

This is one of Sigil's most distinctive features and is central to its AI-first design philosophy.

---

## The Compiler Requirement

### Functions Without Tests

If you define a function without a corresponding test, the compiler emits an error:

```sigil
// math.si
@add (left: int, right: int) -> int = left + right
@multiply (left: int, right: int) -> int = left * right
```

```
error[E0500]: function @add has no tests
  --> math.si:2:1
   |
 2 | @add (left: int, right: int) -> int = left + right
   | ^^^^
   |
   = help: add a test with `@test_add tests @add () -> void = ...`

error[E0500]: function @multiply has no tests
  --> math.si:3:1
   |
 3 | @multiply (left: int, right: int) -> int = left * right
   | ^^^^^^^^^
```

The compilation halts until tests are provided.

### Minimum Test Requirement

Each function needs **at least one** test function that uses the `tests` keyword:

```sigil
@add (left: int, right: int) -> int = left + right

// Satisfies the requirement
@test_add tests @add () -> void = run(
    assert_eq(
        .actual: add(.left: 2, .right: 3),
        .expected: 5,
    ),
)
```

Multiple tests per function are encouraged but not required:

```sigil
@add (left: int, right: int) -> int = left + right

// All three tests count toward @add's coverage
@test_add_positive tests @add () -> void = run(
    assert_eq(
        .actual: add(.left: 2, .right: 3),
        .expected: 5,
    ),
    assert_eq(
        .actual: add(.left: 10, .right: 20),
        .expected: 30,
    ),
)

@test_add_negative tests @add () -> void = run(
    assert_eq(
        .actual: add(.left: -1, .right: 1),
        .expected: 0,
    ),
    assert_eq(
        .actual: add(.left: -5, .right: -3),
        .expected: -8,
    ),
)

@test_add_zero tests @add () -> void = run(
    assert_eq(
        .actual: add(.left: 0, .right: 0),
        .expected: 0,
    ),
    assert_eq(
        .actual: add(.left: 5, .right: 0),
        .expected: 5,
    ),
)
```

---

## Exemptions

### The @main Function

The `@main` function is the only function exempt from the test requirement:

```sigil
@main () -> void = run(
    let result = factorial(.number: 10),
    print(.message: str(.value: result)),
)

// No test required for @main
```

**Rationale:**

1. **Entry point** - `@main` is tested by running the program
2. **Integration** - It typically orchestrates other functions (which are tested)
3. **Side effects** - Entry points often perform I/O that's hard to unit test
4. **No return value** - `@main` returns `void`, making assertions difficult

### Config Variables

Config variables (the `$` prefix) don't require tests:

```sigil
$max_retries = 3
$timeout = 30s
$api_endpoint = "https://api.example.com"

// No tests required for config values
```

Config variables are constants. Testing that `$max_retries == 3` would be tautological.

### Type Definitions

Type definitions don't require tests:

```sigil
type User = {
    name: str,
    email: str,
    age: int
}

type Result<T, E> = Ok(T) | Err(E)

// Types don't need tests - functions that use them do
```

Types are structural. The functions that manipulate them carry the test burden.

### Test Functions Themselves

Test functions don't require tests of their own:

```sigil
@add (left: int, right: int) -> int = left + right

@test_add tests @add () -> void = run(
    assert_eq(
        .actual: add(.left: 2, .right: 3),
        .expected: 5,
    ),
)

// @test_add doesn't need its own test
```

This would create infinite regress.

---

## Why Mandatory Testing?

### The AI Validation Problem

AI-generated code has a fundamental challenge: **how do you know it's correct?**

Traditional approaches:

| Approach | Problem |
|----------|---------|
| Code review | Expensive, slow, error-prone |
| Trust the AI | Dangerous |
| Compile-time checks | Only catches type errors |
| Run and hope | Bugs discovered in production |

Sigil's approach: **force AI to prove its work**.

When an AI generates a function, it must also generate tests that pass. If the tests fail, the code doesn't compile. The tests become evidence of correctness.

### Executable Specification

Tests serve as **executable documentation**:

```sigil
@fibonacci (number: int) -> int = recurse(
    .condition: number <= 1,
    .base: number,
    .step: self(number - 1) + self(number - 2),
    .memo: true
)

@test_fib_base tests @fibonacci () -> void = run(
    assert_eq(
        .actual: fibonacci(.number: 0),
        .expected: 0,
    ),
    assert_eq(
        .actual: fibonacci(.number: 1),
        .expected: 1,
    ),
)

@test_fib_sequence tests @fibonacci () -> void = run(
    assert_eq(
        .actual: fibonacci(.number: 2),
        .expected: 1,
    ),
    assert_eq(
        .actual: fibonacci(.number: 3),
        .expected: 2,
    ),
    assert_eq(
        .actual: fibonacci(.number: 4),
        .expected: 3,
    ),
    assert_eq(
        .actual: fibonacci(.number: 5),
        .expected: 5,
    ),
    assert_eq(
        .actual: fibonacci(.number: 6),
        .expected: 8,
    ),
)
```

A human reviewer can look at the tests to understand:
- What `fibonacci(0)` should return
- What `fibonacci(1)` should return
- The expected sequence

No need to trace through the recursive implementation.

### Catching Edge Cases

Mandatory testing forces consideration of boundaries:

```sigil
@divide (dividend: int, divisor: int) -> Result<int, str> =
    if divisor == 0 then Err("division by zero")
    else Ok(dividend / divisor)

@test_divide tests @divide () -> void = run(
    assert_eq(
        .actual: divide(.dividend: 10, .divisor: 2),
        .expected: Ok(5),
    ),
    assert_eq(
        .actual: divide(.dividend: 0, .divisor: 5),
        .expected: Ok(0),
    ),
    assert_eq(
        .actual: divide(.dividend: 10, .divisor: 0),
        .expected: Err("division by zero"),
    ),
    assert_eq(
        .actual: divide(.dividend: -10, .divisor: 2),
        .expected: Ok(-5),
    ),
)
```

The test requirement prompts thinking about:
- Normal cases
- Zero inputs
- Error conditions
- Negative numbers

### No "I'll Add Tests Later"

One of the most common forms of technical debt is untested code:

```
"Ship it now, add tests later"
"Tests are blocking the release"
"It works, why test it?"
```

Sigil eliminates this category of debt. There is no "later"---code without tests doesn't compile.

### Self-Correcting AI Workflow

With mandatory tests, AI development follows a verifiable loop:

```
1. AI generates function + tests
2. Compile
3. If tests fail:
   - AI sees structured error output
   - AI fixes the function or tests
   - Return to step 2
4. If tests pass: code is verified
```

The AI can't skip step 1 (tests are required) and can't lie about step 4 (tests are run).

---

## Compiler Enforcement

### Test Discovery

The compiler discovers tests through the `tests` keyword:

```sigil
@target_function (...) -> ... = ...

// @test_name is linked to @target_function via `tests` keyword
@test_name tests @target_function () -> void = ...
```

The compiler maintains a mapping:

```
function -> [tests that declare it as target]
```

Any function with an empty test list (except `@main`) produces error E0500.

### Cross-File Tests

Tests can be in separate files:

```
src/
  math.si              # Contains @add
  _test/
    math.test.si       # Contains @test_add tests @add
```

The test file imports the module:

```sigil
// math.test.si
use math { add }

@test_add tests @add () -> void = run(
    assert_eq(
        .actual: add(.left: 2, .right: 3),
        .expected: 5,
    ),
)
```

The compiler still enforces coverage. It considers all `tests @add` declarations across the codebase.

### Multiple Targets

A test can target multiple functions:

```sigil
@parse (input: str) -> Ast = ...
@format (ast: Ast) -> str = ...

// Tests round-trip behavior
@test_round_trip tests @parse tests @format () -> void = run(
    let ast = parse(.input: "x + 1"),
    let output = format(.ast: ast),
    assert_eq(
        .actual: output,
        .expected: "x + 1",
    ),
)
```

Both `@parse` and `@format` are considered tested.

### Private Functions

Private functions (not exported with `pub`) still require tests. There are two ways to test them:

**Option 1: Inline tests (same file)**

Tests in the same file have access to all items, including private ones:

```sigil
// math.si
// private (no pub)
@helper (value: int) -> int = value * 2

pub @double_helper (value: int) -> int = helper(.value: value)

// Test can access @helper because it's in the same file
@test_helper tests @helper () -> void = run(
    assert_eq(
        .actual: helper(.value: 5),
        .expected: 10,
    ),
)
```

**Option 2: `_test/` subdirectory (recommended)**

Test files in a `_test/` subdirectory have special access to their parent module's private items:

```
src/
  math.si              # Contains @helper (private)
  _test/
    math.test.si       # Has access to @helper via test import
```

```sigil
// math.test.si
// Can import private items for testing
use math { helper }

@test_helper tests @helper () -> void = run(
    assert_eq(
        .actual: helper(.value: 5),
        .expected: 10,
    ),
)
```

**Why `_test/` has special access:**

The `_test/` convention signals "these are tests for the parent module." The compiler treats `_test/*.test.si` files as part of the same compilation unit as the parent module, granting access to private items. This keeps implementation details hidden from external consumers while allowing thorough testing.

```
Regular import:    math → external_module    (only pub items)
Test import:       math → math/_test/        (all items)
```

**What test files can access:**

| Item Type | From External Module | From `_test/` Directory |
|-----------|---------------------|-------------------------|
| `pub` functions | Yes | Yes |
| Private functions | No | Yes |
| `pub` types | Yes | Yes |
| Private types | No | Yes |
| Config variables | Yes | Yes |

**Note:** This access only applies to `.test.si` files in the `_test/` subdirectory of the module being tested. Regular modules in `_test/` (not ending in `.test.si`) follow normal visibility rules.

---

## Testing Functions with Side Effects

Functions that perform I/O, network calls, or other side effects use the [capability system](../14-capabilities/index.md) to remain testable.

### The Problem

Without capabilities, testing effectful code is problematic:

```sigil
// How do you test this without hitting the real network?
@get_user (id: str) -> Result<User, Error> = try(
    let json = http_get(.url: "https://api.com/users/" + id)?,
    Ok(parse(.json: json)),
)
```

### The Solution

Declare effects with `uses` and provide mocks in tests:

```sigil
@get_user (id: str) -> Result<User, Error> uses Http = try(
    let json = Http.get(.path: "/users/" + id)?,
    Ok(parse(.json: json)),
)

@test_get_user tests @get_user () -> void =
    with Http = MockHttp { responses: {"/users/1": "{\"name\": \"Alice\"}"} } in
    run(
        let result = get_user(.id: "1"),
        assert_eq(
            .actual: result,
            .expected: Ok(User { name: "Alice" }),
        ),
    )
```

The `uses` clause declares dependencies, and `with`...`in` provides implementations. Tests provide mocks; production provides real implementations.

See [Capabilities](../14-capabilities/index.md) for complete documentation.

---

## Coverage Metrics

### Test Count

The compiler tracks how many tests cover each function:

```
src/math.si
  @add         3 tests (test_add_positive, test_add_negative, test_add_zero)
  @multiply    1 test  (test_multiply)
  @divide      2 tests (test_divide, test_divide_error)
```

### CLI Output

```bash
sigil check math.si
```

```
Checking math.si...

Function Coverage:
  @add          ✓  3 tests
  @multiply     ✓  1 test
  @divide       ✓  2 tests

All functions have tests.
```

With missing tests:

```
Checking math.si...

Function Coverage:
  @add          ✓  3 tests
  @multiply     ✗  0 tests (missing!)
  @divide       ✓  2 tests

error: 1 function missing tests
```

### JSON Output

For tooling integration:

```bash
sigil check math.si --format json
```

```json
{
  "file": "math.si",
  "functions": [
    {"name": "@add", "tests": 3, "covered": true},
    {"name": "@multiply", "tests": 0, "covered": false},
    {"name": "@divide", "tests": 2, "covered": true}
  ],
  "all_covered": false,
  "missing": ["@multiply"]
}
```

---

## AI Development Workflow

### Generation Phase

When an AI generates a function, it must also generate tests:

```
User: "Write a function to check if a number is prime"

AI generates:
```

```sigil
@is_prime (number: int) -> bool =
    if number <= 1 then false
    else if number <= 3 then true
    else if number % 2 == 0 || number % 3 == 0 then false
    else check_factors(.number: number, .factor: 5)

@check_factors (number: int, factor: int) -> bool = recurse(
    .condition: factor * factor > number,
    .base: true,
    .step: if number % factor == 0 || number % (factor + 2) == 0 then false
           else self(.number: number, .factor: factor + 6),
)

@test_is_prime tests @is_prime () -> void = run(
    // Edge cases
    assert(.condition: !is_prime(.number: 0)),
    assert(.condition: !is_prime(.number: 1)),

    // Small primes
    assert(.condition: is_prime(.number: 2)),
    assert(.condition: is_prime(.number: 3)),
    assert(.condition: is_prime(.number: 5)),
    assert(.condition: is_prime(.number: 7)),

    // Non-primes
    assert(.condition: !is_prime(.number: 4)),
    assert(.condition: !is_prime(.number: 6)),
    assert(.condition: !is_prime(.number: 9)),

    // Larger cases
    assert(.condition: is_prime(.number: 97)),
    assert(.condition: !is_prime(.number: 100)),
)

@test_check_factors tests @check_factors () -> void = run(
    assert(.condition: check_factors(.number: 7, .factor: 5)),
    assert(.condition: !check_factors(.number: 25, .factor: 5)),
)
```

The AI can't submit `@is_prime` without `@test_is_prime`.

### Verification Phase

The compiler verifies the AI's work:

```bash
sigil build prime.si
```

If tests pass:
```
Compiling prime.si...
Running tests...
  ✓ @test_is_prime (11 assertions)
  ✓ @test_check_factors (2 assertions)
Build successful.
```

If tests fail:
```
Running tests...
  ✗ @test_is_prime
    assertion failed: is_prime(1) expected false, got true
    at prime.si:20
```

The AI receives structured feedback and can self-correct.

### Iteration Phase

AI fixes based on error output:

```json
{
  "test": "@test_is_prime",
  "status": "failed",
  "assertion": {
    "expression": "!is_prime(1)",
    "expected": true,
    "actual": false
  },
  "location": {
    "file": "prime.si",
    "line": 20
  }
}
```

The AI can reason:
- `is_prime(1)` returned `true` but should return `false`
- The base case handling is wrong
- Fix: adjust the condition for `n <= 1`

---

## Comparison with Other Languages

### Languages Without Test Requirements

| Language | Testing | Consequence |
|----------|---------|-------------|
| Python | Optional | ~30% of packages have tests |
| JavaScript | Optional | Untested code ships frequently |
| Java | Optional | "Test later" becomes "test never" |
| Go | Encouraged | Better, but still optional |

### Languages with Testing Support

| Feature | Rust | Sigil |
|---------|------|-------|
| Built-in test framework | Yes | Yes |
| `#[test]` attribute | Yes | `tests @func` |
| Required for compilation | No | Yes |
| Doc tests | Yes | Future |
| Coverage tools | External | Built-in |

Rust has excellent testing culture but doesn't require tests. Sigil goes further: **no tests = no compilation**.

---

## Design Rationale Summary

| Decision | Rationale |
|----------|-----------|
| Tests required | AI needs verification mechanism |
| Compile-time enforcement | Can't skip or defer |
| `@main` exempt | Entry point tested by running |
| Config exempt | Constants are tautological to test |
| Multiple tests allowed | Encourage thorough coverage |
| Cross-file tests | Support test organization |
| Structured output | AI can parse and self-correct |

---

## Common Questions

### "What about trivial functions?"

Even trivial functions need tests:

```sigil
@identity<T> (value: T) -> T = value

@test_identity tests @identity () -> void = run(
    assert_eq(
        .actual: identity(.value: 5),
        .expected: 5,
    ),
    assert_eq(
        .actual: identity(.value: "hello"),
        .expected: "hello",
    ),
)
```

This may seem excessive, but:
- Test documents the expected behavior
- Catches if someone "improves" the function incorrectly
- Trivial to write

### "What about generated code?"

Code generators must also generate tests. If a macro or template produces functions, it must produce corresponding tests.

### "Can I disable the requirement?"

No. There is no `--no-tests` flag. The requirement is fundamental to Sigil's design, not a lint rule that can be silenced.

### "What about third-party libraries?"

Libraries published to the package registry must have tests for all exported functions. This is verified during publishing.

---

## See Also

- [Test Syntax](02-test-syntax.md) - `tests` keyword, assertions
- [Compile-Fail Tests](03-compile-fail-tests.md) - Testing expected errors
- [Capabilities](../14-capabilities/index.md) - Testing effectful code
- [AI-First Design](../01-philosophy/01-ai-first-design.md) - Why mandatory testing matters
- [Structured Errors](../12-tooling/02-structured-errors.md) - JSON error output for AI
