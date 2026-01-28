---
title: "Program Execution"
description: "Ori Language Specification — Program Execution"
order: 18
---

# Program Execution

A _program_ is a complete, executable Ori application.

> **Grammar:** See [grammar.ebnf](grammar.ebnf) § PROGRAM ENTRY (main_function)

## Entry Point

Every executable program must have exactly one `@main` function.

Valid signatures:

```ori
@main () -> void = ...
@main () -> int = ...
@main (args: [str]) -> void = ...
@main (args: [str]) -> int = ...
```

The `args` parameter, if present, contains command-line arguments passed to the program. It does not include the program name.

```ori
// invoked as: ori run program.ori hello world
@main (args: [str]) -> void = run(
    // args = ["hello", "world"]
    print(args[0]),  // prints "hello"
)
```

## Initialization

Program initialization proceeds in the following order:

1. All modules are initialized eagerly
2. Config variables (`$name`) are evaluated in dependency order
3. `@main` is invoked

### Config Variables

Config variables are compile-time constants evaluated before program execution.

```ori
$timeout = 30s
$max_retries = 3
$double_timeout = $timeout * 2  // references other config
```

Config variables may reference other config variables. The compiler determines evaluation order from dependencies. Circular dependencies are an error.

### Module Initialization

All imported modules are initialized before `@main` runs. Initialization order is determined by the module dependency graph. A module is initialized after all modules it imports.

## Termination

### Normal Termination

A program terminates normally when `@main` returns.

| Return type | Exit code |
|-------------|-----------|
| `void` | 0 |
| `int` | Returned value |

```ori
@main () -> int = run(
    let success = do_work(),
    if success then 0 else 1,
)
```

### Panic Termination

A program terminates abnormally when an unhandled panic occurs.

On panic:
1. Error message is printed to stderr
2. Stack trace is printed to stderr
3. Program exits with code 1

```ori
@main () -> void = run(
    let list = [1, 2, 3],
    list[10],  // panic: index out of bounds
)
```

See [Errors and Panics](20-errors-and-panics.md) for panic semantics.
