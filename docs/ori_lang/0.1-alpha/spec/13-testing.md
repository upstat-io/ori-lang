---
title: "Testing"
description: "Ori Language Specification — Testing"
order: 13
---

# Testing

Every function must have at least one test. Compile-time error otherwise.

> **Grammar:** See [grammar.ebnf](https://ori-lang.com/docs/compiler-design/04-parser#grammar) § DECLARATIONS (test, attribute)

## Test Declaration

All tests must use the `tests` keyword.

> **Grammar:** See [grammar.ebnf](https://ori-lang.com/docs/compiler-design/04-parser#grammar) § DECLARATIONS (test)

### Targeted Test

```ori
@test_add tests @add () -> void = run(
    assert_eq(actual: add(a: 2, b: 3), expected: 5),
)
```

### Multiple Targets

```ori
@test_roundtrip tests @parse tests @format () -> void = ...
```

### Free-Floating Test

```ori
@test_integration tests _ () -> void = ...
```

The `_` wildcard indicates the test targets no specific function. Free-floating tests do not satisfy coverage requirements; they are used for integration tests.

## Exemptions

- `@main`
- Test functions
- Config variables
- Type definitions
- Trait definitions

## Attributes

### skip

```ori
#skip("not yet implemented")
@test_feature tests @feature () -> void = ...
```

Parsed, type-checked, not executed. Satisfies coverage.

### compile_fail

```ori
#compile_fail("type mismatch")
@test_type_error tests @main () -> void = run(
    let x: int = "hello",
    (),
)
```

Passes if compilation fails with error containing the substring.

### fail

```ori
#fail("division by zero")
@test_div_zero tests @divide () -> void = run(
    divide(a: 10, b: 0),
    (),
)
```

Passes if execution fails with message containing the substring.

## Assertions

```
assert(condition: bool) -> void
assert_eq(actual: T, expected: T) -> void
assert_ne(actual: T, unexpected: T) -> void
assert_some(opt: Option<T>) -> void
assert_none(opt: Option<T>) -> void
assert_ok(result: Result<T, E>) -> void
assert_err(result: Result<T, E>) -> void
assert_panics(expr: T) -> void
assert_panics_with(expr: T, message: str) -> void
```

## Organization

### Inline

```ori
@add (a: int, b: int) -> int = a + b

@test_add tests @add () -> void = ...
```

### Separate Files

```
src/
  math.ori
  _test/
    math.test.ori
```

```ori
// _test/math.test.ori
use "../math" { add, ::private_helper }

@test_add tests @add () -> void = ...
```

## Testing Capabilities

```ori
@test_fetch tests @fetch () -> void =
    with Http = MockHttp { responses: {"url": "data"} } in
    run(
        assert_ok(result: fetch(url: "url")),
    )
```

## Execution

```
ori test           # all tests
ori test file.ori   # specific file
```

Tests run in isolation, possibly in parallel. No shared mutable state.

## Incremental Execution

During compilation, targeted tests whose targets (or transitive dependencies) have changed are automatically executed.

| Test Type | When It Runs |
|-----------|--------------|
| Targeted (`tests @fn`) | During `ori check` if `@fn` or its dependencies changed |
| Free-floating (`tests _`) | Only via explicit `ori test` |

### CLI Flags

| Command | Behavior |
|---------|----------|
| `ori check` | Compile + run affected targeted tests |
| `ori check --no-test` | Compile only, skip tests |
| `ori check --strict` | Fail build on test failure |
| `ori test` | Run all tests (targeted + free-floating) |
| `ori test --only-targeted` | Run only targeted tests |

Test failures are reported but do not block compilation by default. Use `--strict` for CI environments.

## Coverage

```
error[E0500]: function @multiply has no tests
```

```
ori check math.ori
```
