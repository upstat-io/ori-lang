# Proposal: Default Implementation Resolution

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-29
**Affects:** Compiler, module system, trait system

---

## Summary

This proposal specifies the resolution rules for `def impl` (default implementations), including how conflicts between multiple defaults are resolved, interaction with `with...in` bindings, and initialization order for config variables in default implementations.

---

## Problem Statement

The spec introduces `def impl` for default trait implementations but leaves unclear:

1. **Conflicting imports**: What if two imported modules provide `def impl` for the same trait?
2. **Override resolution**: How do `with...in` bindings interact with `def impl`?
3. **Scope**: Is `def impl` visible when the trait is re-exported?
4. **Initialization**: What order do config variables in `def impl` bodies initialize?

---

## Default Implementation Basics

### Definition

A `def impl` provides a default implementation bound to a trait import:

```ori
pub def impl Logger {
    @debug (message: str) -> void = print(msg: `[DEBUG] {message}`)
    @info (message: str) -> void = print(msg: `[INFO] {message}`)
    @error (message: str) -> void = print(msg: `[ERROR] {message}`)
}
```

### Automatic Binding

When a module imports a trait with an associated `def impl`, the default is automatically bound:

```ori
// In app.ori
use std.logging { Logger }  // def impl automatically bound

@main () -> void =
    Logger.info(message: "Hello")  // Uses def impl
```

---

## Conflict Resolution

### Rule: One Default Per Trait Per Scope

A scope can have at most ONE `def impl` for each trait. Conflicts are resolved at import time.

### Same-Module Conflict

```ori
// ERROR: two def impl for same trait in same module
def impl Logger { ... }
def impl Logger { ... }  // Error: duplicate default implementation for Logger
```

### Import Conflict

When importing from modules with conflicting defaults:

```ori
// module_a.ori
pub def impl Logger { @info (msg: str) = print(msg: "A: " + msg) }

// module_b.ori
pub def impl Logger { @info (msg: str) = print(msg: "B: " + msg) }

// app.ori
use "module_a" { Logger }   // Brings in module_a's def impl
use "module_b" { Logger }   // ERROR: conflicting default for Logger
```

**Resolution**: The second import is an error. To use both modules, explicitly choose one:

```ori
use "module_a" { Logger }
use "module_b" as b { }  // Import module, not its def impl

// Use module_a's default, explicitly call module_b when needed
Logger.info(message: "Using A's default")
with Logger = b.Logger in Logger.info(message: "Using B's impl")
```

### Explicit Import Syntax

To import a trait WITHOUT its default:

```ori
use "module_a" { Logger without def }  // Import trait, skip def impl

// Must provide implementation explicitly
with Logger = MyLogger in Logger.info(message: "Custom")
```

---

## with...in Interaction

### Override Precedence

`with...in` always overrides `def impl`:

```ori
def impl Logger { @info (msg: str) = print(msg: "[DEF] " + msg) }

@example () -> void = run(
    Logger.info(message: "A"),  // Uses def impl: "[DEF] A"

    with Logger = CustomLogger in run(
        Logger.info(message: "B"),  // Uses CustomLogger
    ),

    Logger.info(message: "C"),  // Back to def impl: "[DEF] C"
)
```

### Nested with...in

Inner `with` shadows outer:

```ori
with Logger = LoggerA in run(
    Logger.info(message: "A"),  // LoggerA

    with Logger = LoggerB in run(
        Logger.info(message: "B"),  // LoggerB (shadows A)
    ),

    Logger.info(message: "C"),  // LoggerA again
)
```

### with...in Shadows def impl

```ori
def impl Logger { ... }

with Logger = TestLogger in run(
    // def impl is completely shadowed here
    Logger.info(message: "Test"),  // Uses TestLogger
)
```

---

## Scope and Visibility

### Module-Local Default

A `def impl` without `pub` is module-local:

```ori
// internal.ori
def impl Logger { ... }  // Only visible in this module

pub @log_something () -> void =
    Logger.info(message: "Internal logging")
```

### Public Default

A `pub def impl` is exported with the trait:

```ori
// logging.ori
pub trait Logger { ... }
pub def impl Logger { ... }

// Importing Logger also imports its def impl
```

### Re-export Behavior

Re-exporting a trait includes its default if both are public:

```ori
// re_export.ori
pub use std.logging { Logger }  // Re-exports trait AND def impl
```

To re-export trait without default:

```ori
pub use std.logging { Logger without def }
```

---

## Config Variables in def impl

### Definition

`def impl` can use module-level `$` bindings for configuration:

```ori
let $LOG_LEVEL = LogLevel.Info

pub def impl Logger {
    @info (msg: str) -> void =
        if $LOG_LEVEL <= LogLevel.Info then print(msg: msg)
}
```

### Initialization Order

Config variables are initialized in dependency order before any code runs:

1. Module-level `$` bindings are topologically sorted by dependencies
2. Each binding is evaluated once, in order
3. `def impl` methods can reference these after initialization

```ori
let $CONFIG_PATH = "./config.json"
let $CONFIG = load_config(path: $CONFIG_PATH)  // Depends on $CONFIG_PATH
let $LOG_LEVEL = $CONFIG.log_level             // Depends on $CONFIG

def impl Logger {
    @info (msg: str) -> void =
        if $LOG_LEVEL <= LogLevel.Info then print(msg: msg)
}
```

Initialization order: `$CONFIG_PATH` → `$CONFIG` → `$LOG_LEVEL`

### Circular Dependency

Circular dependencies are a compile error:

```ori
let $A = $B + 1  // Error: circular dependency
let $B = $A + 1
```

---

## Multiple Traits

### Independent Defaults

A module can have `def impl` for multiple traits:

```ori
def impl Logger { ... }
def impl Cache { ... }
def impl Http { ... }
```

Each is independent — conflicts are per-trait.

### Partial Override

`with...in` can override some capabilities while keeping others:

```ori
def impl Logger { ... }
def impl Cache { ... }

with Logger = TestLogger in run(
    // TestLogger used, but Cache still uses def impl
    Logger.info(message: "Test")
    Cache.get(key: "foo")  // Uses def impl
)
```

---

## No self Parameter

### Stateless Implementations

`def impl` methods cannot have `self` — they're stateless:

```ori
// OK: no self
pub def impl Logger {
    @info (message: str) -> void = print(msg: message)
}

// ERROR: def impl methods cannot have self
pub def impl Logger {
    @info (self, message: str) -> void = ...  // Error
}
```

### State via Config

Use module-level bindings for "state":

```ori
let $connection_pool = create_pool()

pub def impl Database {
    @query (sql: str) -> Result<Rows, Error> =
        $connection_pool.execute(sql)
}
```

---

## Error Messages

### Conflicting Imports

```
error[E1000]: conflicting default implementations for trait `Logger`
  --> src/app.ori:2:1
   |
1  | use "module_a" { Logger }
   | ------------------------- first default from here
2  | use "module_b" { Logger }
   | ^^^^^^^^^^^^^^^^^^^^^^^^^ conflicting default from here
   |
   = help: use `Logger without def` to import trait without default
   = help: or use different aliases: `use "module_b" as b { }`
```

### Duplicate def impl

```
error[E1001]: duplicate default implementation for trait `Logger`
  --> src/logging.ori:10:1
   |
5  | def impl Logger { ... }
   | ----------------------- first definition here
...
10 | def impl Logger { ... }
   | ^^^^^^^^^^^^^^^^^^^^^^^ duplicate definition
```

### Self in def impl

```
error[E1002]: `def impl` methods cannot have `self` parameter
  --> src/logging.ori:3:5
   |
3  |     @info (self, message: str) -> void = ...
   |            ^^^^ `self` not allowed in default implementation
   |
   = note: default implementations are stateless
   = help: use module-level bindings for configuration
```

---

## Examples

### Complete Logging Setup

```ori
// logging/mod.ori
pub trait Logger {
    @debug (message: str) -> void
    @info (message: str) -> void
    @warn (message: str) -> void
    @error (message: str) -> void
}

let $LOG_LEVEL: LogLevel = parse_env_log_level()

pub def impl Logger {
    @debug (message: str) -> void =
        if $LOG_LEVEL <= LogLevel.Debug then
            print(msg: `[DEBUG] {now()}: {message}`)

    @info (message: str) -> void =
        if $LOG_LEVEL <= LogLevel.Info then
            print(msg: `[INFO] {now()}: {message}`)

    @warn (message: str) -> void =
        if $LOG_LEVEL <= LogLevel.Warn then
            print(msg: `[WARN] {now()}: {message}`)

    @error (message: str) -> void =
        print(msg: `[ERROR] {now()}: {message}`)  // Always log errors
}
```

### Testing with Override

```ori
use std.logging { Logger }

@my_function () -> Result<int, Error> = run(
    Logger.info(message: "Starting"),
    // ... do work ...
    Ok(42),
)

@test_my_function tests @my_function () -> void = run(
    let logs = [],
    let capture = MockLogger { logs: logs },

    with Logger = capture in
        my_function(),

    assert_eq(actual: logs.len(), expected: 1),
    assert(condition: logs[0].contains(substr: "Starting")),
)
```

---

## Spec Changes Required

### Update `08-declarations.md`

Add:
1. `def impl` conflict resolution rules
2. Import syntax (`without def`)
3. Stateless requirement (no `self`)

### Update `12-modules.md`

Add:
1. Re-export behavior with `def impl`
2. Visibility rules (`pub def impl`)

### Update `14-capabilities.md`

Add:
1. `with...in` override precedence
2. Interaction between capability binding and `def impl`

---

## Summary

| Aspect | Rule |
|--------|------|
| Conflict | One `def impl` per trait per scope |
| Import conflict | Compile error; use `without def` |
| `with...in` | Always overrides `def impl` |
| Nested `with` | Inner shadows outer |
| Visibility | `pub def impl` exported with trait |
| Re-export | Includes `def impl` if both public |
| Config variables | Topologically sorted initialization |
| Circular deps | Compile error |
| `self` parameter | Not allowed (stateless) |
