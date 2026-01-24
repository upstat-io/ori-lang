# Errors and Panics

Sigil distinguishes between recoverable errors and unrecoverable panics.

## Recoverable Errors

Recoverable errors use `Result<T, E>` and `Option<T>` types. See [Types](06-types.md) for type definitions and methods.

### Error Propagation

The `?` operator propagates errors. See [Control Flow](19-control-flow.md) for details.

```sigil
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

### Explicit Panic

The `panic` function triggers a panic explicitly:

```sigil
panic(message)
```

`panic` has return type `Never` and never returns normally:

```sigil
let x: int = if valid then value else panic("invalid state")
```

### Panic Behavior

When a panic occurs:

1. Error message is recorded
2. Stack trace is captured
3. If inside `catch(...)`, control transfers to the catch
4. Otherwise, message and trace print to stderr, program exits with code 1

## Integer Overflow

Integer arithmetic wraps on overflow:

```sigil
let max: int = 9223372036854775807  // max signed 64-bit
let wrapped = max + 1               // wraps to -9223372036854775808
```

Overflow does not panic. Programs requiring overflow detection should use checked methods from the standard library.

## Catching Panics

The `catch` pattern captures panics and converts them to `Result`:

```
catch_expr = "catch" "(" expression ")" .
```

```sigil
let result = catch(dangerous_operation())
// result: Result<T, PanicInfo>

match(result,
    Ok(value) -> use(value),
    Err(info) -> handle_panic(info),
)
```

### PanicInfo

`PanicInfo` contains information about the caught panic:

```sigil
type PanicInfo = {
    message: str,    // panic message
    location: str,   // source location "file:line"
}
```

### Nested Catch

`catch` expressions may be nested. A panic propagates to the innermost enclosing `catch`:

```sigil
catch(
    let x = catch(may_panic())?,  // inner catch
    process(x),                    // may also panic
)
// outer catch handles panics from process()
```

### Limitations

`catch` cannot recover from:
- Process-level signals (SIGKILL, SIGSEGV)
- Out of memory conditions
- Stack overflow

These conditions terminate the program immediately.

## Error Conventions

The `Error` trait provides a standard interface for error types:

```sigil
trait Error {
    @message (self) -> str
}
```

Custom error types should implement `Error`:

```sigil
type ParseError = { line: int, message: str }

impl Error for ParseError {
    @message (self) -> str =
        "line " + str(self.line) + ": " + self.message
}
```

Functions returning `Result` conventionally use `E: Error`, but any type may be used as the error type.
