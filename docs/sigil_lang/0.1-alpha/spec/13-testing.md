# Testing

Every function must have at least one test. Compile-time error otherwise.

## Test Declaration

```
test   = [ attribute ] "@" identifier [ "tests" target { "tests" target } ] "()" "->" "void" "=" expression .
target = "@" identifier .
```

### Targeted Test

```sigil
@test_add tests @add () -> void = run(
    assert_eq(actual: add(a: 2, b: 3), expected: 5),
)
```

### Multiple Targets

```sigil
@test_roundtrip tests @parse tests @format () -> void = ...
```

### Free-Floating Test

```sigil
@test_integration () -> void = ...
```

Does not satisfy coverage; used for integration tests.

## Exemptions

- `@main`
- Test functions
- Config variables
- Type definitions
- Trait definitions

## Attributes

```
attribute = "#" ( "skip" | "compile_fail" | "fail" ) "(" string_literal ")" .
```

### skip

```sigil
#skip("not yet implemented")
@test_feature tests @feature () -> void = ...
```

Parsed, type-checked, not executed. Satisfies coverage.

### compile_fail

```sigil
#compile_fail("type mismatch")
@test_type_error tests @main () -> void = run(
    let x: int = "hello",
    (),
)
```

Passes if compilation fails with error containing the substring.

### fail

```sigil
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

```sigil
@add (a: int, b: int) -> int = a + b

@test_add tests @add () -> void = ...
```

### Separate Files

```
src/
  math.si
  _test/
    math.test.si
```

```sigil
// _test/math.test.si
use '../math' { add, ::private_helper }

@test_add tests @add () -> void = ...
```

## Testing Capabilities

```sigil
@test_fetch tests @fetch () -> void =
    with Http = MockHttp { responses: {"url": "data"} } in
    run(
        assert_ok(result: fetch(url: "url")),
    )
```

## Execution

```
sigil test           # all tests
sigil test file.si   # specific file
```

Tests run in isolation, possibly in parallel. No shared mutable state.

## Coverage

```
error[E0500]: function @multiply has no tests
```

```
sigil check math.si
```
