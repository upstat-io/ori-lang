# Mandatory Tests

This document covers Sigil's mandatory testing requirements: why testing is required, what's exempt, how the compiler enforces it, and why this matters for AI-authored code.

---

## Overview

In Sigil, **every function must have at least one test**. This is not a guideline or best practice---it's a compiler requirement. Code without tests fails to compile.

```sigil
@factorial (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: 1,
    .step: n * self(n - 1)
)

// REQUIRED - compilation fails without this test
@test_factorial tests @factorial () -> void = run(
    assert_eq(factorial(0), 1),
    assert_eq(factorial(5), 120)
)
```

This is one of Sigil's most distinctive features and is central to its AI-first design philosophy.

---

## The Compiler Requirement

### Functions Without Tests

If you define a function without a corresponding test, the compiler emits an error:

```sigil
// math.si
@add (a: int, b: int) -> int = a + b
@multiply (a: int, b: int) -> int = a * b
```

```
error[E0500]: function @add has no tests
  --> math.si:2:1
   |
 2 | @add (a: int, b: int) -> int = a + b
   | ^^^^
   |
   = help: add a test with `@test_add tests @add () -> void = ...`

error[E0500]: function @multiply has no tests
  --> math.si:3:1
   |
 3 | @multiply (a: int, b: int) -> int = a * b
   | ^^^^^^^^^
```

The compilation halts until tests are provided.

### Minimum Test Requirement

Each function needs **at least one** test function that uses the `tests` keyword:

```sigil
@add (a: int, b: int) -> int = a + b

// Satisfies the requirement
@test_add tests @add () -> void = run(
    assert_eq(add(2, 3), 5)
)
```

Multiple tests per function are encouraged but not required:

```sigil
@add (a: int, b: int) -> int = a + b

// All three tests count toward @add's coverage
@test_add_positive tests @add () -> void = run(
    assert_eq(add(2, 3), 5),
    assert_eq(add(10, 20), 30)
)

@test_add_negative tests @add () -> void = run(
    assert_eq(add(-1, 1), 0),
    assert_eq(add(-5, -3), -8)
)

@test_add_zero tests @add () -> void = run(
    assert_eq(add(0, 0), 0),
    assert_eq(add(5, 0), 5)
)
```

---

## Exemptions

### The @main Function

The `@main` function is the only function exempt from the test requirement:

```sigil
@main () -> void = run(
    result = factorial(10),
    print(str(result))
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
@add (a: int, b: int) -> int = a + b

@test_add tests @add () -> void = run(
    assert_eq(add(2, 3), 5)
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
@fibonacci (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: n,
    .step: self(n - 1) + self(n - 2),
    .memo: true
)

@test_fib_base tests @fibonacci () -> void = run(
    assert_eq(fibonacci(0), 0),
    assert_eq(fibonacci(1), 1)
)

@test_fib_sequence tests @fibonacci () -> void = run(
    assert_eq(fibonacci(2), 1),
    assert_eq(fibonacci(3), 2),
    assert_eq(fibonacci(4), 3),
    assert_eq(fibonacci(5), 5),
    assert_eq(fibonacci(6), 8)
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
@divide (a: int, b: int) -> Result<int, str> =
    if b == 0 then Err("division by zero")
    else Ok(a / b)

@test_divide tests @divide () -> void = run(
    assert_eq(divide(10, 2), Ok(5)),
    assert_eq(divide(0, 5), Ok(0)),
    assert_eq(divide(10, 0), Err("division by zero")),
    assert_eq(divide(-10, 2), Ok(-5))
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
    assert_eq(add(2, 3), 5)
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
    ast = parse("x + 1"),
    output = format(ast),
    assert_eq(output, "x + 1")
)
```

Both `@parse` and `@format` are considered tested.

### Private Functions

Private functions (not exported with `pub`) still require tests:

```sigil
// Internal helper, not exported
@helper (x: int) -> int = x * 2

// Still needs a test
@test_helper tests @helper () -> void = run(
    assert_eq(helper(5), 10)
)
```

The test file can access non-public items within the same module.

---

## Testing Functions with Side Effects

Functions that perform I/O, network calls, or other side effects use the [capability system](../14-capabilities/index.md) to remain testable.

### The Problem

Without capabilities, testing effectful code is problematic:

```sigil
// How do you test this without hitting the real network?
@get_user (id: str) -> Result<User, Error> = try(
    json = http_get("https://api.com/users/" + id),
    Ok(parse(json))
)
```

### The Solution

Declare effects with `uses` and provide mocks in tests:

```sigil
@get_user (id: str) -> Result<User, Error> uses Http = try(
    json = Http.get("/users/" + id),
    Ok(parse(json))
)

@test_get_user tests @get_user () -> void =
    with Http = MockHttp { responses: {"/users/1": "{\"name\": \"Alice\"}"} } in
    run(
        result = get_user("1"),
        assert_eq(result, Ok(User { name: "Alice" }))
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
@is_prime (n: int) -> bool =
    if n <= 1 then false
    else if n <= 3 then true
    else if n % 2 == 0 || n % 3 == 0 then false
    else check_factors(n, 5)

@check_factors (n: int, i: int) -> bool = recurse(
    .cond: i * i > n,
    .base: true,
    .step: if n % i == 0 || n % (i + 2) == 0 then false
           else self(n, i + 6)
)

@test_is_prime tests @is_prime () -> void = run(
    // Edge cases
    assert(!is_prime(0)),
    assert(!is_prime(1)),

    // Small primes
    assert(is_prime(2)),
    assert(is_prime(3)),
    assert(is_prime(5)),
    assert(is_prime(7)),

    // Non-primes
    assert(!is_prime(4)),
    assert(!is_prime(6)),
    assert(!is_prime(9)),

    // Larger cases
    assert(is_prime(97)),
    assert(!is_prime(100))
)

@test_check_factors tests @check_factors () -> void = run(
    assert(check_factors(7, 5)),
    assert(!check_factors(25, 5))
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
@identity<T> (x: T) -> T = x

@test_identity tests @identity () -> void = run(
    assert_eq(identity(5), 5),
    assert_eq(identity("hello"), "hello")
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
