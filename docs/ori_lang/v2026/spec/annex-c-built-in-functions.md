---
title: "Annex C — Built-in functions"
description: "Ori Language Specification — Annex C (normative)"
order: 102
section: "Annexes"
---

# Annex C (normative) — Built-in functions

This annex defines the functions, methods, and types provided by the standard prelude. A conforming implementation shall provide all functions defined in this annex.

Core functions provided by the language. All built-in functions require named arguments, except type conversions.

## Reserved Names

Built-in names cannot be used for function definitions. Reserved in call position only; may be used as variables.

```ori
let min = 5;           // OK: variable
min(left: a, right: b) // OK: calls built-in
@min (...) = ...      // error: reserved name
```

## Type Conversions (function_val)

Type conversions are the sole exception to named argument requirements. Positional syntax is allowed.

| Function | From | Behavior |
|----------|------|----------|
| `int(x)` | `float` | Truncates toward zero |
| | `str` | Parses decimal; panics on invalid |
| | `bool` | `true`→1, `false`→0 |
| | `byte` | Zero-extends |
| `float(x)` | `int` | Exact (may lose precision) |
| | `str` | Parses; panics on invalid |
| `str(x)` | `int`, `float`, `bool` | Decimal representation |
| `byte(x)` | `int` | Truncates to 8 bits |
| | `str` | First UTF-8 byte |

## Collection Functions

```
len<T: Len>(collection: T) -> int
is_empty<T: IsEmpty>(collection: T) -> bool
```

`len(collection:)` desugars to `collection.len()`. The result is always ≥ 0. Complexity is O(1) for all built-in types.

Built-in `Len` implementations:

| Type | Returns |
|------|---------|
| `[T]` | Number of elements |
| `{K: V}` | Number of entries |
| `str` | Number of UTF-8 bytes (not codepoints) |
| `Set<T>` | Number of elements |
| `Range<int>` | Number of values in range |
| Tuples | Number of elements (compile-time) |

NOTE  For string codepoint count, use `.chars().count()`. Infinite ranges do not implement `Len`.

`is_empty(collection:)` desugars to `collection.is_empty()`. Equivalent to `len(collection:) == 0` for types implementing both `Len` and `IsEmpty`.

## Option Functions

```
is_some(opt: Option<T>) -> bool
is_none(opt: Option<T>) -> bool
```

### Option Methods

```
Option<T>.map(transform: T -> U) -> Option<U>
Option<T>.unwrap_or(default: T) -> T
Option<T>.ok_or(err: E) -> Result<T, E>
Option<T>.and_then(then: T -> Option<U>) -> Option<U>
Option<T>.filter(predicate: T -> bool) -> Option<T>
```

## Result Functions

```
is_ok(result: Result<T, E>) -> bool
is_err(result: Result<T, E>) -> bool
```

### Result Methods

```
Result<T, E>.map(transform: T -> U) -> Result<U, E>
Result<T, E>.map_err(transform: E -> F) -> Result<T, F>
Result<T, E>.unwrap_or(default: T) -> T
Result<T, E>.ok() -> Option<T>
Result<T, E>.err() -> Option<E>
Result<T, E>.and_then(then: T -> Result<U, E>) -> Result<U, E>
```

## Assertions

All assertion functions are available without import (`use std.testing { ... }` is required for test files that are not attached tests).

```
assert(condition: bool) -> void
```

Panics with "assertion failed" when `condition` is `false`. No capability required.

```
assert_eq<T: Eq + Debug>(actual: T, expected: T) -> void
assert_ne<T: Eq + Debug>(actual: T, unexpected: T) -> void
```

`assert_eq` panics when `actual != expected`. `assert_ne` panics when `actual == unexpected`. Panic messages include the `Debug` representation of both values.

NOTE  `assert_eq(actual: float.nan, expected: float.nan)` panics because NaN is not equal to itself per IEEE 754.

```
assert_some<T>(option: Option<T>) -> T
assert_none<T>(option: Option<T>) -> void
assert_ok<T, E>(result: Result<T, E>) -> T
assert_err<T, E>(result: Result<T, E>) -> E
```

`assert_some` and `assert_ok` return the inner value on success. `assert_err` returns the error value. All panic when the assertion fails.

```
assert_panics(expr: () -> void) -> void
assert_panics_with(expr: () -> void, message: str) -> void
```

`assert_panics` evaluates the closure `expr` and panics if `expr` does _not_ panic. `assert_panics_with` additionally verifies that the panic message contains `message` as a substring.

## Comparison

```
compare<T: Comparable>(left: T, right: T) -> Ordering
```

Desugars to `left.compare(other: right)`. Returns `Less`, `Equal`, or `Greater`.

```
min<T: Comparable>(left: T, right: T) -> T
max<T: Comparable>(left: T, right: T) -> T
```

Returns the lesser (or greater) of two values. When values are equal, returns `left` (the first argument). For `float`, NaN is considered greater than all other values (per `Comparable` trait contract).

```
hash_combine(seed: int, value: int) -> int
```

Combines two hash values using the Boost hash_combine algorithm. Pure function with no side effects.

## I/O

```
print(msg: str) -> void
```

Writes `msg` to stdout followed by a newline. Requires the `Print` capability (available by default). Inside an `@panic` handler, writes to stderr instead.

## Control

```
panic(msg: str) -> Never
```

Unconditionally panics with the given message. Never returns. Triggers the `@panic` handler if defined. See [23.4](23-program-execution.md#234-panic-handler).

## Panic Handler

An optional top-level function that executes before program termination when a panic occurs.

```ori
@panic (info: PanicInfo) -> void = {
    print(msg: `Fatal error: {info.message}`);
    print(msg: `Location: {info.location.file}:{info.location.line}`);
}
```

### Rules

- At most one `@panic` function per program
- Shall have signature `(PanicInfo) -> void`
- Executes synchronously before program exit
- If `@panic` itself panics, immediate termination (no recursion)

### PanicInfo Type

```ori
type PanicInfo = {
    message: str,
    location: TraceEntry,
    stack_trace: [TraceEntry],
    thread_id: Option<int>,
}
```

The `location` field uses `TraceEntry` (which has `function`, `file`, `line`, `column`). The `stack_trace` is ordered from most recent to oldest. The `thread_id` is `Some(id)` in concurrent contexts, `None` for single-threaded execution.

### Implicit Stderr

Inside `@panic`, the `print()` function writes to stderr instead of stdout.

### Concurrent Panics

When multiple tasks panic simultaneously:

1. The first panic to reach the handler wins
2. Subsequent panics are recorded but do not re-run the handler
3. After the handler completes, the program exits with non-zero code

### Default Handler

If no `@panic` handler is defined:

```ori
@panic (info: PanicInfo) -> void = {
    print(msg: `panic: {info.message}`);
    print(msg: `  at {info.location.file}:{info.location.line}`);
    for frame in info.stack_trace do
        print(msg: `    {frame.function}`);
}
```

### Capabilities

Any capability may be declared. Capabilities that perform I/O (Http, FileSystem) may hang or fail, risking the handler never completing. Simple handlers (stderr logging) are safest.

## Developer Functions

### todo

```
todo() -> Never
todo(reason: str) -> Never
```

Marks unfinished code. Panics with "not yet implemented" and file location.

```ori
@parse_json (input: str) -> Result<Json, Error> = todo();
// Panics: "not yet implemented at src/parser.ori:15"

@handle_event (event: Event) -> void = match event {
    Click(pos) -> handle_click(pos: pos),
    Scroll(delta) -> todo(reason: "scroll handling"),
    // Panics: "not yet implemented: scroll handling at src/ui.ori:42"
    KeyPress(key) -> handle_key(key: key),
}
```

### unreachable

```
unreachable() -> Never
unreachable(reason: str) -> Never
```

Marks code that should never execute. Panics with "unreachable code reached" and file location.

```ori
@day_type (day: int) -> str = match day {
    1 | 2 | 3 | 4 | 5 -> "weekday",
    6 | 7 -> "weekend",
    _ -> unreachable(reason: "day must be 1-7"),
}
```

### dbg

```
dbg<T: Debug>(value: T) -> T
dbg<T: Debug>(value: T, label: str) -> T
```

Prints value with location to stderr, returns value unchanged. Requires `Debug` trait. Uses `Print` capability.

```ori
let x = dbg(value: calculate());
// Prints: [src/math.ori:10] = 42

let y = dbg(value: get_y(), label: "y coordinate");
// Prints: [src/point.ori:6] y coordinate = 200
```

Output format:
- Without label: `[file:line] = value`
- With label: `[file:line] label = value`

## Concurrency

```
is_cancelled() -> bool
```

Available in async contexts. Returns `true` if the current task has been marked for cancellation. See [Patterns § nursery](15-patterns.md#1544-nursery) for cancellation semantics.

## Resource Management

### drop_early

```
drop_early<T>(value: T) -> void
```

Forces a value to be dropped before the end of its scope. Takes ownership of the value, causing its destructor to run immediately (if the type implements `Drop`) and its memory to be reclaimed.

```ori
{
    let file = open_file(path: "data.txt");
    let content = read_all(file);
    drop_early(value: file);  // Close immediately, don't wait for scope end
    process(content);         // Continue with content, file already closed
}
```

Works for any type, not just types implementing `Drop`:
- For types with `Drop`: the `drop` method is called, then memory is reclaimed
- For types without `Drop`: memory is reclaimed immediately

Use `drop_early` to release resources (file handles, connections, locks) as soon as they are no longer needed, rather than waiting for scope exit.

## Iteration

### repeat

```
repeat<T: Clone>(value: T) -> impl Iterator
```

Creates an infinite iterator that yields clones of `value`. The type `T` shall implement `Clone`.

```ori
repeat(value: 0).take(count: 5).collect()  // [0, 0, 0, 0, 0]
repeat(value: "x").take(count: 3).collect()  // ["x", "x", "x"]
```

Infinite iterators shall be bounded before terminal operations:

```ori
repeat(value: 1).collect()                      // Infinite loop, eventual OOM
repeat(value: default).take(count: n).collect() // Safe
```

Common patterns:

```ori
// Initialize list
let zeros: [int] = repeat(value: 0).take(count: 100).collect();

// Zip with constant
items.iter().zip(other: repeat(value: multiplier)).map(transform: (x, m) -> x * m)
```

## Compile-Time

### compile_error

```
compile_error(msg: str) -> Never
```

Causes a compile-time error with the given message. Valid only in contexts that are statically evaluable at compile time:

1. **Conditional compilation branches**: Inside `#target(...)` or `#cfg(...)` blocks
2. **Const-if branches**: Inside `if $constant then ... else ...` where the condition involves compile-time constants

```ori
// OK: compile_error in conditional block
#target(os: "windows")
@platform_specific () -> void = compile_error(msg: "Windows not supported");

// OK: compile_error in dead branch
@check () -> void =
    if $target_os == "windows" then
        compile_error(msg: "Windows not supported")
    else
        ();

// ERROR: compile_error in unconditional code
@bad () -> void = compile_error(msg: "always fails");
```

See [Conditional Compilation](25-conditional-compilation.md) for conditional compilation semantics.

### embed

```
embed(path_expr) -> str | [byte]
```

Embeds the contents of a file at compile time. The path shall be a const-evaluable `str` expression, resolved relative to the source file containing the `embed` expression. The return type is determined by the expected type in context:

- If `str` is expected, the file shall be valid UTF-8. A compile-time error is produced if the file contains invalid UTF-8 sequences.
- If `[byte]` is expected, the file is read as raw bytes with no encoding validation.

If the expected type cannot be inferred, it is an error. The compiler shall require an explicit type annotation.

```ori
// UTF-8 text embedding
let $SCHEMA: str = embed("schema.sql");

// Binary embedding
let $ICON: [byte] = embed("assets/icon.png");

// Const expression paths (not limited to literals)
let $DATA_DIR = "data";
let $CONFIG: str = embed(`{$DATA_DIR}/config.toml`);
```

**Path restrictions:**
- Absolute paths are an error.
- Paths that resolve outside the project root (via `..`) are an error.
- Path separators shall be `/` (normalized by the compiler across platforms).

**Size limit:** The default maximum embedded file size is 10 MB. This limit may be overridden per-expression with `#embed_limit(size:)` or project-wide in `ori.toml`.

**Dependency tracking:** The compiler shall track embedded files as build dependencies. Modifications to an embedded file shall trigger recompilation of the module containing the `embed` expression.

### has_embed

```
has_embed(path_expr) -> bool
```

Compile-time boolean expression. Evaluates to `true` if the file at `path_expr` exists and is readable, `false` otherwise. The path is resolved with the same rules as `embed`.

```ori
let $HELP: str = if has_embed("HELP.md") then embed("HELP.md") else "No help available";
```

**Dependency tracking:** The compiler shall track files referenced by `has_embed`. A change in file existence shall trigger recompilation.

## Prelude

Available without import:
- `int`, `float`, `str`, `byte`
- `len`, `is_empty`
- `is_some`, `is_none`, `Some`, `None`
- `is_ok`, `is_err`, `Ok`, `Err`
- All assertions
- `print`, `panic`, `compare`, `min`, `max`
- `todo`, `unreachable`, `dbg`
- `repeat`, `drop_early`
- `compile_error`
- `embed`, `has_embed`
- `is_cancelled` (async contexts only)
- `CancellationError`, `CancellationReason` types
- `PanicInfo` type (with `message`, `location`, `stack_trace`, `thread_id`)

## Collection Methods

Data transformation via method calls on collections.

### List Methods

```
[T].map(transform: T -> U) -> [U]
[T].filter(predicate: T -> bool) -> [T]
[T].fold(initial: U, op: (U, T) -> U) -> U
[T].find(where: T -> bool) -> Option<T>
[T].any(predicate: T -> bool) -> bool
[T].all(predicate: T -> bool) -> bool
[T].first() -> Option<T>
[T].last() -> Option<T>
[T].take(n: int) -> [T]
[T].skip(n: int) -> [T]
[T].reverse() -> [T]
[T].sort() -> [T] where T: Comparable
[T].contains(value: T) -> bool where T: Eq
```

### Range Methods

```
Range<T>.map(transform: T -> U) -> [U]
Range<T>.filter(predicate: T -> bool) -> [T]
Range<T>.fold(initial: U, op: (U, T) -> U) -> U
Range<T>.collect() -> [T]
Range<T>.contains(value: T) -> bool
```

### String Methods

```
str.split(sep: str) -> [str]
str.trim() -> str
str.upper() -> str
str.lower() -> str
str.starts_with(prefix: str) -> bool
str.ends_with(suffix: str) -> bool
str.contains(substr: str) -> bool
```

## Standard Library

| Module | Contents |
|--------|----------|
| `std.math` | `sqrt`, `abs`, `pow`, `sin`, `cos`, `tan` |
| `std.string` | `split`, `join`, `trim`, `upper`, `lower` |
| `std.list` | `sort`, `reverse`, `unique` |
| `std.io` | File I/O |
| `std.net` | Network |
| `std.resilience` | `retry`, `exponential`, `linear` |
| `std.validate` | `validate` |

### std.resilience

```ori
use std.resilience { retry, exponential, linear };

retry(op: fetch(url), attempts: 3, backoff: exponential(base: 100ms));
retry(op: fetch(url), attempts: 5, backoff: linear(delay: 100ms))
```

### std.validate

```ori
use std.validate { validate };

validate(
    rules: [
        age >= 0 | "age must be non-negative",
        name != "" | "name required",
    ],
    value: User { name, age },
)
```

Returns `Result<T, [str]>`.
