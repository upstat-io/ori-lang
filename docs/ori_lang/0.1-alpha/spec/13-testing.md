---
title: "Testing"
description: "Ori Language Specification — Testing"
order: 13
section: "Verification"
---

# Testing

Ori enforces mandatory verification: every function must have at least one test. Tests are first-class constructs bound to their targets via the `tests` keyword. The compiler executes affected tests automatically during compilation.

> **Grammar:** See [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) § DECLARATIONS (test, attribute)
>
> **Implementation Model:** See [Test Execution Model Proposal](../../../proposals/approved/test-execution-model-proposal.md) for data structures, algorithms, and cache formats.

## Test Declaration

A _test_ is a function that verifies the behavior of one or more target functions. All tests must use the `tests` keyword.

> **Grammar:** See [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) § DECLARATIONS (test)

### Attached Tests

An _attached test_ declares one or more functions it tests:

```ori
@test_add tests @add () -> void = {
    assert_eq(actual: add(a: 2, b: 3), expected: 5)
}
```

Multiple targets are specified by repeating the `tests` keyword:

```ori
@test_roundtrip tests @parse tests @format () -> void = {
    let ast = parse(input: "x + 1")
    let output = format(ast: ast)
    assert_eq(actual: output, expected: "x + 1")
}
```

An attached test satisfies the test coverage requirement for all of its targets.

### Floating Tests

A _floating test_ uses `_` as its target, indicating it tests no specific function:

```ori
@test_integration tests _ () -> void = {
    let result = full_pipeline(input: "program")
    assert_ok(result: result)
}
```

Floating tests:
- Do not satisfy coverage requirements for any function
- Do not run during normal compilation
- Run only via explicit `ori test` command

The `_` token is consistent with its use elsewhere in the language: pattern matching wildcards, ignored lambda parameters.

### Test Signature

All tests must:
- Take no parameters: `()`
- Return `void`: `-> void`
- Have a body expression

```ori
// Valid
@test_example tests @example () -> void = {...}

// Invalid - tests cannot have parameters
@test_bad tests @bad (x: int) -> void = ...  // error

// Invalid - tests must return void
@test_bad tests @bad () -> int = ...  // error
```

## Test Coverage Requirement

Every function must have at least one attached test. It is a compile-time error if a function has no tests.

```
error[E0500]: function @multiply has no tests
  --> src/math.ori:15:1
   |
15 | @multiply (a: int, b: int) -> int = a * b
   | ^^^^^^^^^ untested function
   |
   = help: add a test with `@test_multiply tests @multiply () -> void = ...`
```

### Exemptions

The following declarations are exempt from the test coverage requirement:

- `@main` — program entry point
- Test functions — tests do not require tests
- Immutable bindings (`let $name = ...`) — constants
- Type definitions (`type Name = ...`)
- Trait definitions (`trait Name { ... }`)
- Trait implementations (`impl Trait for Type { ... }`)
- Default implementations (`def impl Trait { ... }`)

## Test Execution Model

Tests execute as part of the compilation process. The compiler integrates test execution after successful type checking of affected code.

### Compilation Phases

```
Source Files
    │
    ▼
┌─────────────────┐
│     Parse       │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   Type Check    │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Test Discovery │  ◄── Identify tests for affected functions
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Test Execution  │  ◄── Run affected attached tests
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│    Code Gen     │  (if requested)
└─────────────────┘
```

Tests run after type checking succeeds for their targets. A test cannot execute if its target function fails to type check.

### Execution Guarantees

- Tests execute in isolation with no shared mutable state
- Tests may execute in parallel when they have no ordering dependencies
- Each test receives a fresh environment
- Test execution order is unspecified

## Dependency-Aware Execution

The compiler maintains a _dependency graph_ tracking which functions call which other functions. Test execution uses this graph to determine which tests to run when code changes.

### Forward and Reverse Dependencies

For any function `f`:
- _Forward dependencies_: functions that `f` calls
- _Reverse dependencies_ (callers): functions that call `f`

```
@helper ← @process ← @handle_request ← @main
   │          │            │
   │          │            └── reverse dependency of @process
   │          └── reverse dependency of @helper
   └── forward dependency of @process
```

### Reverse Transitive Closure

When a function changes, the compiler computes its _reverse transitive closure_: the set of all functions that directly or transitively depend on it.

Given this dependency graph:

```
@parse ← @compile ← @run_program
           ↑
       @optimize
```

If `@parse` changes:
- Direct reverse dependencies: `@compile`
- Transitive reverse dependencies: `@run_program`
- Reverse transitive closure: `{@parse, @compile, @run_program}`

### Affected Test Determination

A test is _affected_ by a change if any function in the reverse transitive closure is one of its targets.

```ori
@test_parse tests @parse () -> void = ...
@test_compile tests @compile () -> void = ...
@test_optimize tests @optimize () -> void = ...
@test_run tests @run_program () -> void = ...
```

If `@parse` changes:
- `@test_parse` runs (direct target)
- `@test_compile` runs (`@compile` calls `@parse`)
- `@test_run` runs (`@run_program` calls `@compile` which calls `@parse`)
- `@test_optimize` does not run (`@optimize` does not depend on `@parse`)

### Algorithm

```
function affected_tests(changed_functions):
    affected = {}

    for func in changed_functions:
        affected.add(func)
        affected.union(reverse_transitive_closure(func))

    return tests where any target in affected

function reverse_transitive_closure(func):
    result = {func}
    queue = [func]

    while queue is not empty:
        current = queue.pop()
        for caller in direct_callers(current):
            if caller not in result:
                result.add(caller)
                queue.append(caller)

    return result
```

## Incremental Compilation

During incremental compilation, the compiler tracks which functions have changed and executes only the tests affected by those changes.

### Change Detection

A function is considered _changed_ if:
- Its source code has been modified (detected via content hash)
- Any of its forward dependencies has changed (transitive)

The compiler maintains a cache of function content hashes:

```
.ori/cache/
├── hashes.bin      # Function content hashes
├── deps.bin        # Dependency graph
└── test-results/   # Cached test results
```

### Incremental Execution Flow

1. **Detect changes**: Compare current function hashes to cached hashes
2. **Compute affected set**: Build reverse transitive closure of changed functions
3. **Filter tests**: Select attached tests where any target is in affected set
4. **Check cache**: Skip tests whose inputs (target hashes) match cached results
5. **Execute**: Run tests not satisfied by cache
6. **Update cache**: Store new results keyed by input hashes

### Full Compilation

During full compilation (no cache or cache invalidated):
1. All attached tests execute
2. Results are cached for subsequent incremental builds
3. Floating tests do not execute (require explicit `ori test`)

## Test Results

### Non-Blocking Execution

By default, test failures are reported but do not block compilation:

```
$ ori check src/math.ori

Compiling...
  ✓ @add (changed)

Running affected tests...
  ✗ @test_add
    assertion failed: expected 5, got 6
    at src/math.ori:12:5

Build succeeded with 1 test failure.
```

The compilation completes, allowing developers to iterate on failing tests.

### Strict Mode

In strict mode (`--strict`), any test failure causes the build to fail:

```
$ ori check --strict src/math.ori

Compiling...
Running affected tests...
  ✗ @test_add

Build FAILED: 1 test failure.
```

Strict mode is intended for CI environments and pre-commit hooks.

### Result States

A test execution produces one of the following results:

| Result | Meaning |
|--------|---------|
| Pass | All assertions succeeded |
| Fail | An assertion failed or the test panicked |
| Skip | Test has `#skip` attribute |
| Error | Test could not execute (e.g., target failed to compile) |

## Performance Considerations

Attached tests run during compilation and should be fast. The compiler emits a warning if an attached test exceeds the slow test threshold.

### Slow Test Warning

```
warning: attached test @test_parse took 250ms
  --> src/parser.ori:45:1
   |
45 | @test_parse tests @parse () -> void = ...
   | ^^^^^^^^^^^ slow attached test
   |
   = note: attached tests run during compilation
   = help: consider making this a floating test: `tests _`
   = note: threshold is 100ms (configurable in ori.toml)
```

### Threshold Configuration

The slow test threshold is configurable via `ori.toml`:

```toml
[testing]
slow_test_threshold = "100ms"
```

Supported duration units: `ms`, `s`, `m`. Default is `100ms`.

### Guidelines

- Attached tests should complete in under 100ms
- Use capability mocking to avoid I/O in attached tests
- Use floating tests (`tests _`) for integration tests requiring real I/O
- Use floating tests for tests with complex setup or large data sets

## Test Attributes

### skip

A skipped test is parsed and type-checked but not executed:

```ori
#skip("waiting for feature X")
@test_feature tests @feature () -> void = {...}
```

Skipped tests satisfy the coverage requirement for their targets.

### compile_fail

A compile-fail test passes if compilation fails with an error containing the specified substring:

```ori
#compile_fail("type mismatch")
@test_type_error tests @main () -> void = {
    let x: int = "hello"
    ()
}
```

The test fails if:
- Compilation succeeds
- Compilation fails but error message does not contain the substring

### fail

A fail test passes if execution panics with a message containing the specified substring:

```ori
#fail("division by zero")
@test_div_zero tests @divide () -> void = {
    divide(a: 10, b: 0)
    ()
}
```

The test fails if:
- Execution completes without panicking
- Execution panics but message does not contain the substring

## Assertions

The following assertion functions are available in the prelude:

```
assert(condition: bool) -> void
assert_eq(actual: T, expected: T) -> void
assert_ne(actual: T, unexpected: T) -> void
assert_some(opt: Option<T>) -> void
assert_none(opt: Option<T>) -> void
assert_ok(result: Result<T, E>) -> void
assert_err(result: Result<T, E>) -> void
assert_panics(f: () -> void) -> void
assert_panics_with(f: () -> void, msg: str) -> void
```

All assertions panic on failure with a descriptive message including the source location.

## Test Organization

All tests must be placed in a `_test/` subdirectory with `.test.ori` suffix. It is a compile-time error to define a test function outside of a `_test/` directory.

```
src/
├── math.ori
└── _test/
    └── math.test.ori
```

```
error[E0501]: test defined outside _test/ directory
  --> src/math.ori:5:1
   |
 5 | @test_add tests @add () -> void = ...
   | ^^^^^^^^^ tests must be in a _test/ directory
   |
   = help: move this test to src/_test/math.test.ori
```

This convention cleanly separates test code from production code. Test files are excluded from compiled output by directory path alone — no conditional compilation flags or build-time stripping required.

### Test File Naming

Test files use the `.test.ori` suffix. By convention, each source file `foo.ori` has a corresponding `_test/foo.test.ori`, though a single test file may test functions from multiple source files.

### Example

```ori
// src/_test/math.test.ori
use "../math" { add, ::internal_helper }

@test_add tests @add () -> void = {
    assert_eq(actual: add(a: 2, b: 3), expected: 5)
}

@test_helper tests @internal_helper () -> void = {
    assert_eq(actual: internal_helper(x: 5), expected: 10)
}
```

Private items may be imported using the `::` prefix (see [Modules § Private Access](12-modules.md#private-access)).

## Testing Capabilities

Functions with capabilities are tested by providing mock implementations via `with...in`:

```ori
@fetch_user (id: int) -> Result<User, Error> uses Http = {
    let response = Http.get(url: `/users/{id}`)?
    Ok(parse_user(data: response))
}

@test_fetch_user tests @fetch_user () -> void =
    with Http = MockHttp { responses: {"/users/1": `{"name": "Alice"}`} } in
    {
        let result = fetch_user(id: 1)
        assert_ok(result: result)
        let user = result.unwrap()
        assert_eq(actual: user.name, expected: "Alice")
    }
```

This enables fast, deterministic tests without actual I/O.

## Command-Line Interface

### ori check

Compiles source files and runs affected attached tests:

```
ori check [OPTIONS] <PATH>

Options:
    --no-test     Compile only, skip test execution
    --strict      Fail build on any test failure
    --verbose     Show all test results, not just failures
```

### ori test

Runs all tests (attached and floating):

```
ori test [OPTIONS] [PATH]

Options:
    --only-attached    Run only attached tests (skip floating)
    --filter <PATTERN> Run only tests matching pattern
    --verbose          Show all test results
```

### Execution Summary

| Command | Targeted Tests | Free-Floating Tests |
|---------|----------------|---------------------|
| `ori check` | Affected only | Never |
| `ori check --no-test` | Never | Never |
| `ori test` | All | All |
| `ori test --only-attached` | All | Never |
