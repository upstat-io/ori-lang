# Errors and Panics

Ori distinguishes between recoverable errors and unrecoverable panics.

## Recoverable Errors

Recoverable errors use `Result<T, E>` and `Option<T>` types. See [Types](06-types.md) for type definitions and methods.

### Error Propagation

The `?` operator propagates errors. See [Control Flow](19-control-flow.md) for details.

```ori
@load (path: str) -> Result<Data, Error> = run(
    let content = read_file(path)?,
    let data = parse(content)?,
    Ok(data),
)
```

## Panics

A _panic_ is an unrecoverable error that terminates normal execution.

### Implicit Panics

The following operations cause implicit panics:

| Operation | Condition | Panic message |
|-----------|-----------|---------------|
| List index | Index out of bounds | "index out of bounds: index N, length M" |
| String index | Index out of bounds | "index out of bounds: index N, length M" |
| `.unwrap()` | Called on `None` | "called unwrap on None" |
| `.unwrap()` | Called on `Err(e)` | "called unwrap on Err: {e}" |
| Division | Divisor is zero | "division by zero" |
| Integer arithmetic | Result overflows `int` range | "integer overflow in {operation}" |

### Explicit Panic

The `panic` function triggers a panic explicitly:

```ori
panic(message)
```

`panic` has return type `Never` and never returns normally:

```ori
let x: int = if valid then value else panic("invalid state")
```

### Panic Behavior

When a panic occurs:

1. Error message is recorded
2. Stack trace is captured
3. If inside `catch(...)`, control transfers to the catch
4. Otherwise, message and trace print to stderr, program exits with code 1

## Integer Overflow

Integer arithmetic panics on overflow:

```ori
let max: int = 9223372036854775807  // max signed 64-bit
let result = catch(expr: max + 1)   // Err("integer overflow")
```

Addition, subtraction, multiplication, and negation all panic on overflow. Programs requiring wrapping or saturating arithmetic should use methods from `std.math`.

## Catching Panics

The `catch` pattern captures panics and converts them to `Result<T, str>`:

```
catch_expr = "catch" "(" "expr" ":" expression ")" .
```

```ori
let result = catch(expr: dangerous_operation())
// result: Result<T, str>

match(result,
    Ok(value) -> use(value),
    Err(msg) -> handle_error(msg),
)
```

If the expression evaluates successfully, `catch` returns `Ok(value)`. If the expression panics, `catch` returns `Err(message)` where `message` is the panic message string.

### Nested Catch

`catch` expressions may be nested. A panic propagates to the innermost enclosing `catch`:

```ori
catch(expr: run(
    let x = catch(expr: may_panic())?,  // inner catch
    process(x),                          // may also panic
))
// outer catch handles panics from process()
```

### Limitations

`catch` cannot recover from:
- Process-level signals (SIGKILL, SIGSEGV)
- Out of memory conditions
- Stack overflow

These conditions terminate the program immediately.

## Panic Assertions

The prelude provides two functions for testing panic behavior:

### `assert_panics`

```ori
assert_panics(f: () -> void) -> void
```

`assert_panics` evaluates the thunk `f`. If `f` panics, the assertion succeeds. If `f` returns normally, `assert_panics` itself panics with the message `"assertion failed: expected panic but succeeded"`.

### `assert_panics_with`

```ori
assert_panics_with(f: () -> void, msg: str) -> void
```

`assert_panics_with` evaluates the thunk `f`. If `f` panics with a message equal to `msg`, the assertion succeeds. If `f` panics with a different message or returns normally, `assert_panics_with` panics.

## Error Conventions

The `Error` trait provides a standard interface for error types:

```ori
trait Error {
    @message (self) -> str
}
```

Custom error types should implement `Error`:

```ori
type ParseError = { line: int, message: str }

impl Error for ParseError {
    @message (self) -> str =
        "line " + str(self.line) + ": " + self.message
}
```

Functions returning `Result` conventionally use `E: Error`, but any type may be used as the error type.
