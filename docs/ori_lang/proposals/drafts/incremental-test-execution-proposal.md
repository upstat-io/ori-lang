# Proposal: Incremental Test Execution and Explicit Free-Floating Tests

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-23
**Approved:** 2026-01-25
**Affects:** Language design, compiler, test runner

---

## Summary

Two related changes to Ori's testing system:

1. **Explicit free-floating tests**: Replace naming convention with `tests _` syntax
2. **Incremental test execution**: Targeted tests auto-run during compilation when their targets change

```ori
// Targeted test - runs during compilation when @add changes
@test_add tests @add () -> void = run(
    assert_eq(actual: add(a: 1, b: 2), expected: 3)
)

// Free-floating test - only runs via `ori test`
@integration_suite tests _ () -> void = run(
    let result = full_pipeline("input"),
    assert_ok(result: result)
)
```

---

## Motivation

### Problem 1: Ambiguous Free-Floating Tests

Current spec: free-floating tests are identified by naming convention (`test_` prefix without `tests @target`).

```ori
// Is this a test or a helper?
@test_helper () -> void = setup_data()

// This is a free-floating test (by naming convention)
@test_integration () -> void = run(...)

// This is a function (no test_ prefix)
@integration_check () -> void = run(...)
```

Problems:
- Naming convention is implicit, easy to mistake
- Helper functions in test files might accidentally start with `test_`
- No syntactic marker that something is a test

### Problem 2: Forgetting to Run Tests

Developers (and AI assistants) often:
1. Make changes to code
2. Forget to run tests
3. Or run the wrong tests
4. Or run all tests when only a few are affected

The compiler already knows the dependency graph. The `tests @target` syntax explicitly declares which tests cover which functions. This information exists but isn't used for execution.

### The Insight

The `tests` keyword creates an edge in the dependency graph:

```
@test_tokenize tests @tokenize
       │
       ▼
    @tokenize (changed)
       │
       ▼
    @parse_token (dependency)
```

When `@tokenize` changes, the compiler knows `@test_tokenize` is affected. Why not run it automatically?

---

## Design

### Part 1: Explicit Free-Floating Tests with `tests _`

All tests must use the `tests` keyword. Free-floating tests use `_` as the target:

```ori
// Targeted test
@test_add tests @add () -> void = run(...)

// Multiple targets
@test_roundtrip tests @parse tests @format () -> void = run(...)

// Free-floating test (explicit)
@test_integration tests _ () -> void = run(...)
```

#### Grammar

```ebnf
test    = "@" identifier "tests" targets params "->" "void" "=" expression .
targets = "_" | target { "tests" target } .
target  = "@" identifier .
```

#### Semantics

- `tests @fn` — targeted test, covers `@fn` for test requirement
- `tests _` — free-floating test, covers no function

The `_` token is consistent with its use elsewhere:
- Pattern matching: `_ -> default` (match anything)
- Lambdas: `(_, b) -> b` (ignore parameter)
- Tests: `tests _` (targets nothing specific)

#### Migration

The `test_` naming convention is no longer special. Existing code:

```ori
// Old: free-floating by naming convention
@test_integration () -> void = run(...)

// New: explicit free-floating
@test_integration tests _ () -> void = run(...)
```

### Part 2: Incremental Test Execution

During compilation, targeted tests whose targets (or transitive dependencies) have changed are automatically executed.

#### Compilation Flow

```
$ ori check src/parser.ori

Compiling...
  ✓ @parse_token (changed)
  ✓ @tokenize (depends on @parse_token)

Running affected tests...
  ✓ @test_parse_token (2 assertions)
  ✓ @test_tokenize (3 assertions)
  ✗ @test_precedence (expected 23, got 35)
    src/parser.ori:47

Build succeeded with 1 test failure.
```

#### Which Tests Run?

| Test Type | When It Runs |
|-----------|--------------|
| Targeted (`tests @fn`) | During compilation if `@fn` or its dependencies changed |
| Free-floating (`tests _`) | Only via explicit `ori test` |

The dependency graph determines "affected":

```
Change @helper
    ↓ (used by)
@process uses @helper
    ↓ (tested by)
@test_process tests @process  ← runs

@test_e2e tests _  ← does NOT run (free-floating)
```

#### Non-Blocking by Default

Test failures are reported but don't block compilation:

```
$ ori check src/math.ori

Compiling...
  ✓ @add (changed)

Running affected tests...
  ✗ @test_add (expected 5, got 6)

Build succeeded with 1 test failure.
```

For strict mode (CI, pre-commit):

```
$ ori check --strict src/math.ori

Build failed: 1 test failure.
```

#### Caching

The compiler tracks:
1. Hash of each function's normalized AST
2. Test results from previous runs
3. Dependency edges

On incremental compile:
1. Compute changed functions (hash mismatch)
2. Walk dependency graph to find affected tests
3. Run only affected targeted tests
4. Cache results keyed by input hashes

### Part 3: Performance Expectations

Targeted tests run during compilation, so they should be fast.

```ori
// Good: targeted test is fast and focused
@test_parse_int tests @parse_int () -> void = run(
    assert_eq(actual: parse_int("42"), expected: Some(42)),
    assert_eq(actual: parse_int("abc"), expected: None),
)

// Good: slow test is free-floating
@test_full_compile_cycle tests _ () -> void = run(
    let source = read_file("large_program.ori"),
    let result = compile_and_run(source),
    assert_ok(result: result),
)
```

#### Compiler Warning

If a targeted test exceeds a threshold (configurable, default 100ms):

```
warning: targeted test @test_parse took 250ms
  --> src/parser.ori:45
  |
  | Targeted tests run during compilation.
  | Consider making this a free-floating test: tests _
  |
  = hint: targeted tests should complete in <100ms
```

---

## Examples

### Basic Usage

```ori
@add (a: int, b: int) -> int = a + b

@multiply (a: int, b: int) -> int = a * b

// Targeted: runs when @add changes
@test_add tests @add () -> void = run(
    assert_eq(actual: add(a: 2, b: 3), expected: 5),
)

// Targeted: runs when @multiply changes
@test_multiply tests @multiply () -> void = run(
    assert_eq(actual: multiply(a: 2, b: 3), expected: 6),
)

// Free-floating: only runs via `ori test`
@test_math_integration tests _ () -> void = run(
    let result = add(a: multiply(a: 2, b: 3), b: 1),
    assert_eq(actual: result, expected: 7),
)
```

### Transitive Dependencies

```ori
@helper (x: int) -> int = x * 2

@process (x: int) -> int = helper(x) + 1

// Runs when @helper OR @process changes
@test_process tests @process () -> void = run(
    assert_eq(actual: process(5), expected: 11),
)
```

### Multiple Targets

```ori
@parse (s: str) -> Ast = ...
@format (a: Ast) -> str = ...

// Runs when @parse OR @format changes
@test_roundtrip tests @parse tests @format () -> void = run(
    let ast = parse("x + 1"),
    let output = format(ast),
    assert_eq(actual: output, expected: "x + 1"),
)
```

### Test Organization

```ori
// src/tokenizer.ori

@tokenize (input: str) -> [Token] = ...

// Fast unit test - runs during compilation
@test_tokenize_basic tests @tokenize () -> void = run(
    assert_eq(
        actual: tokenize("1 + 2"),
        expected: [Int(1), Plus, Int(2)],
    ),
)

// Slow integration test - runs only via `ori test`
@test_tokenize_large_file tests _ () -> void = run(
    let input = read_file("fixtures/large.ori"),
    let tokens = tokenize(input),
    assert(.cond: len(collection: tokens) > 10000),
)
```

---

## CLI Changes

### `ori check` (default)

Compiles and runs affected targeted tests:

```
$ ori check src/

Compiling 3 files...
Running 5 affected tests...
  ✓ @test_parse (2 assertions)
  ✓ @test_tokenize (3 assertions)
  ...

Build succeeded. 5 tests passed.
```

### `ori check --no-test`

Compile only, skip test execution:

```
$ ori check --no-test src/

Compiling 3 files...
Build succeeded.
```

### `ori check --strict`

Fail build on test failure (for CI):

```
$ ori check --strict src/

Compiling...
Running affected tests...
  ✗ @test_parse (assertion failed)

Build FAILED: 1 test failure.
```

### `ori test`

Run all tests (targeted and free-floating):

```
$ ori test

Running all tests...
  ✓ @test_parse (2 assertions)
  ✓ @test_tokenize (3 assertions)
  ✓ @test_integration (5 assertions)  // free-floating runs here
  ...

42 passed, 0 failed.
```

### `ori test --only-targeted`

Run only targeted tests (useful for quick check):

```
$ ori test --only-targeted

Running targeted tests...
  ✓ @test_parse (2 assertions)
  ...

38 passed, 0 failed. (4 free-floating tests skipped)
```

---

## Benefits

### For Developers

- **No forgetting tests** — they run automatically on compile
- **Fast feedback** — only affected tests run, not the entire suite
- **Clear distinction** — `tests @fn` vs `tests _` is unambiguous

### For AI Assistants

- **Built-in correctness** — can't "forget" to run tests
- **Immediate feedback** — see test failures alongside compile errors
- **No guessing** — don't need to figure out which tests to run

### For CI

- **Faster builds** — incremental test caching
- **Strict mode** — `--strict` fails on any test failure
- **Same behavior** — CI runs same tests as local development

---

## Implementation Notes

### Compiler Changes

1. **Parser**: Require `tests` keyword for all tests, accept `_` as target
2. **AST**: `TestDef.targets` becomes `enum { Targeted(Vec<Name>), FreeFloating }`
3. **Dependency graph**: Index tests by target function
4. **Incremental**: Track function hashes, compute affected tests
5. **Execution**: Run affected tests after type checking

### Test Runner Changes

1. Accept filter for targeted-only vs all tests
2. Report timing per test for threshold warnings
3. Cache test results keyed by dependency hashes

### Migration Path

1. Emit warning for `test_` functions without `tests` keyword
2. Provide automated fix: add `tests _` to free-floating tests
3. After transition period, require `tests` keyword

---

## Alternatives Considered

### 1. Keep Naming Convention

Status quo: `test_` prefix indicates free-floating test.

Rejected: Implicit, easy to confuse with helper functions.

### 2. Use `tests void` Instead of `tests _`

```ori
@test_integration tests void () -> void = ...
```

Rejected: `void` is a type, overloading it is confusing. `_` is the established "don't care" token.

### 3. Separate `@test` Declaration

```ori
@test test_integration () -> void = ...
```

Rejected: Inconsistent with targeted test syntax, requires new keyword position.

### 4. Attribute for Free-Floating

```ori
#[free]
@test_integration tests () -> void = ...
```

Rejected: More verbose, attributes are for modifiers not core semantics.

### 5. Optional Test Execution

Make incremental test execution opt-in.

Rejected: The value is in being automatic. Opt-in means people forget.

---

## Summary

This proposal makes Ori's testing system:

1. **Explicit** — `tests _` clearly marks free-floating tests
2. **Automatic** — affected tests run during compilation
3. **Fast** — only changed code's tests run
4. **Non-intrusive** — failures shown but don't block by default

The `tests` keyword becomes the universal marker for "this is a test", with the target indicating scope:

| Syntax | Meaning | Runs During |
|--------|---------|-------------|
| `tests @fn` | Tests specific function | Compilation (when affected) |
| `tests _` | Tests nothing specific | `ori test` only |

```ori
// Change this function...
@parse (input: str) -> Ast = ...

// ...and this test runs automatically
@test_parse tests @parse () -> void = run(...)

// ...but this one waits for explicit `ori test`
@test_e2e tests _ () -> void = run(...)
```
