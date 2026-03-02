---
title: "Program Execution"
description: "Clause 23: Ori Language Specification — Program Execution"
order: 23
section: "Language"
---

# 23 Program execution

A _program_ is a complete, executable Ori application.

> **Grammar:** See [grammar.ebnf](grammar.md) § PROGRAM ENTRY (main_function)

## 23.1 Entry point

Every executable program shall have exactly one `@main` function.

Valid signatures:

```ori
@main () -> void = ...;
@main () -> int = ...;
@main (args: [str]) -> void = ...;
@main (args: [str]) -> int = ...;
```

The `args` parameter, if present, contains command-line arguments passed to the program. It does not include the program name.

```ori
// invoked as: ori run program.ori hello world
@main (args: [str]) -> void = {
    // args = ["hello", "world"]
    print(args[0]),  // prints "hello"
}
```

## 23.2 Initialization

Program initialization proceeds in the following order:

1. All modules are initialized eagerly
2. Config variables (`$name`) are evaluated in dependency order
3. `@main` is invoked

### 23.2.1 Config variables

Config variables are compile-time constants evaluated before program execution.

```ori
$timeout = 30s;
$max_retries = 3;
$double_timeout = $timeout * 2;  // references other config
```

Config variables may reference other config variables. The compiler determines evaluation order from dependencies. Circular dependencies are an error.

### 23.2.2 Module initialization

All imported modules are initialized before `@main` runs.

Modules form a directed acyclic graph via import declarations. The initialization order is determined by a deterministic topological sort of this graph: a module is initialized only after all modules it imports have been initialized. Ties in the sort order are broken by import declaration order.

Circular module dependencies are a compile-time error.

Module-level constant initializers shall be pure expressions (see [12.2](12-constants.md#122-module-level-constants)). This ensures that initialization order does not affect observable behavior.

## 23.3 Termination

### 23.3.1 Normal termination

A program terminates normally when `@main` returns.

| Return type | Exit code |
|-------------|-----------|
| `void` | 0 |
| `int` | Returned value |

Exit codes outside the range 0–255 are truncated to the low 8 bits.

On normal termination, `Drop` implementations run for all live values in reverse declaration order (LIFO). All `Drop` implementations shall complete before the process exits.

### 23.3.2 Panic termination

A program terminates abnormally when an unhandled panic occurs.

On panic, the runtime executes the following sequence:

1. Construct a `PanicInfo` value with message, location, and stack trace
2. If an `@panic` handler is defined, call it (see 23.4)
3. Print the error message to stderr
4. Print the stack trace to stderr
5. Exit with code 1

`Drop` implementations do _not_ run during panic. Ori uses an abort model: panics terminate immediately without unwinding the stack.

NOTE  Panics represent bugs in the program. Use `Result` for recoverable errors. See [Clause 17](17-errors-and-panics.md) for the error handling model.

### 23.3.3 Standard streams

- `print(msg:)` writes to stdout, followed by a newline
- Panic messages and stack traces write to stderr
- `print(msg:)` inside an `@panic` handler writes to stderr
- There is no built-in for reading stdin; use the `std.io` module

## 23.4 Panic handler

A program may define an optional `@panic` handler:

```ori
@panic (info: PanicInfo) -> void = {
    print(msg: `FATAL: {info.message}`);
}
```

The `@panic` handler, if present, is called before the default panic behavior. At most one `@panic` handler shall exist per program.

A panic inside the `@panic` handler (a _double panic_) causes immediate termination with no further processing. A panic during a `Drop` implementation also causes immediate termination.

The `PanicInfo` type is defined in the prelude:

```ori
type PanicInfo = {
    message: str,
    location: TraceEntry,
    stack_trace: [TraceEntry],
    thread_id: Option<int>,
}
```

## 23.5 Runtime panic catalogue

The following table lists all conditions that cause a runtime panic. Each condition is normative: an implementation shall panic under exactly these circumstances.

| Condition | Clause | Message pattern |
|-----------|--------|----------------|
| Integer overflow (add, sub, mul, neg) | [14.3](14-expressions.md) | "integer overflow" |
| Integer division by zero | [14.3](14-expressions.md) | "division by zero" |
| Integer modulo by zero | [14.3](14-expressions.md) | "modulo by zero" |
| `int.min / -1` or `int.min % -1` | [14.3](14-expressions.md) | "integer overflow" |
| Shift count negative | [14.3](14-expressions.md) | "negative shift count" |
| Shift count ≥ bit width | [14.3](14-expressions.md) | "shift count exceeds bit width" |
| `1 << 63` (shift overflow) | [14.3](14-expressions.md) | "shift overflow" |
| List index out of bounds | [14.1.2](14-expressions.md) | "index out of bounds: N, length M" |
| String index out of bounds | [14.1.2](14-expressions.md) | "index out of bounds" |
| `as` conversion out of range | [14.1.5](14-expressions.md) | "conversion out of range" |
| `unwrap()` on `None` | [9](09-properties-of-types.md) | "unwrap called on None" |
| `unwrap()` on `Err` | [9](09-properties-of-types.md) | "unwrap called on Err" |
| `panic(msg:)` | [Annex C](annex-c-built-in-functions.md) | User-provided message |
| `todo()` | [Annex C](annex-c-built-in-functions.md) | "not yet implemented" |
| `unreachable()` | [Annex C](annex-c-built-in-functions.md) | "entered unreachable code" |
| `assert` failure | [Annex C](annex-c-built-in-functions.md) | "assertion failed" |
| `assert_eq` / `assert_ne` failure | [Annex C](annex-c-built-in-functions.md) | "assertion failed: ..." |
| Range step is zero | [14.6](14-expressions.md) | "step cannot be zero" |
| Fixed-capacity list push when full | [8](08-types.md) | "list is full" |
| Stack overflow | 23 | "stack overflow" |
| Pre-condition violation | [14](14-expressions.md) | "pre-condition failed: ..." |
| Post-condition violation | [14](14-expressions.md) | "post-condition failed: ..." |

All panics produce a `PanicInfo` value with message, source location, and stack trace.

Panics are not recoverable. There is no `catch` mechanism for panics. Use `Result` for recoverable errors and `assert_panics` in tests to verify panic behavior.

## 23.6 Stack depth

The maximum call stack depth is implementation-defined. An implementation shall support at least 1 000 stack frames. Exceeding the stack depth limit causes a "stack overflow" panic.
