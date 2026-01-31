# Proposal: Additional Built-in Functions and Types

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Approved:** 2026-01-30
**Affects:** Compiler, built-ins, type system

---

## Summary

This proposal formalizes three built-in functions and one type: `repeat`, `compile_error`, `PanicInfo`, and clarifies the null coalescing operator (`??`).

---

# repeat Built-in

## Definition

```ori
@repeat<T: Clone> (value: T) -> impl Iterator where Item == T
```

Creates an infinite iterator that yields clones of `value`.

## Usage

```ori
repeat(value: 0).take(count: 5).collect()  // [0, 0, 0, 0, 0]
repeat(value: "x").take(count: 3).collect()  // ["x", "x", "x"]
```

## Infinite Iterator

`repeat` produces an infinite iterator:

```ori
repeat(value: 1).collect()  // Infinite loop, eventual OOM
```

Always bound with `.take()` before `.collect()`:

```ori
repeat(value: default).take(count: n).collect()  // Safe
```

## Clone Requirement

The value must implement `Clone`:

```ori
repeat(value: 42)           // OK: int is Clone
repeat(value: file_handle)  // ERROR if FileHandle: !Clone
```

## Common Patterns

### Initialize Array

```ori
let zeros: [int] = repeat(value: 0).take(count: 100).collect()
```

### Pad List

```ori
let padded = list + repeat(value: padding).take(count: target_len - len(collection: list)).collect()
```

### Zip with Constant

```ori
items.iter().zip(other: repeat(value: multiplier)).map(transform: (x, m) -> x * m)
```

---

# compile_error Built-in

## Definition

```ori
@compile_error (msg: str) -> Never
```

Causes a compile-time error with the given message.

## Constraints

`compile_error` is valid only in contexts that are statically evaluable at compile time:

1. **Conditional compilation branches**: Inside `#target(...)` or `#cfg(...)` blocks
2. **Const-if branches**: Inside `if $constant then ... else ...` where the condition involves compile-time constants

It is a compile-time error to use `compile_error` in runtime-reachable code:

```ori
// ERROR: compile_error in unconditional code
@bad () -> void = compile_error(msg: "always fails")

// OK: compile_error in dead branch
@platform_check () -> void =
    if $target_os == "windows" then
        compile_error(msg: "Windows not supported")
    else
        real_impl()
```

## Usage

Used with conditional compilation to enforce constraints:

```ori
#!target(os: "linux")

#target(os: "windows")
@platform_specific () -> void = compile_error(msg: "Windows not supported")

#target(os: "linux")
@platform_specific () -> void = linux_impl()
```

## Compile-Time Only

`compile_error` is evaluated at compile time:

```ori
@check_platform () -> void =
    if $target_os == "windows" then
        compile_error(msg: "Windows not supported")
    else
        ()
```

The error triggers during compilation, not at runtime.

## Feature Gating

```ori
@optional_feature () -> void =
    #cfg(feature: "advanced")
    advanced_impl()

    #cfg(not_feature: "advanced")
    compile_error(msg: "Enable 'advanced' feature to use this function")
```

## FFI Availability

```ori
extern "c" from "libfoo" {
    #target(os: "linux")
    @_foo () -> int as "foo"

    #target(os: "windows")
    @_foo () -> int = compile_error(msg: "libfoo not available on Windows")
}
```

---

# PanicInfo Type

## Definition

```ori
type PanicInfo = {
    message: str,
    file: str,
    line: int,
    column: int,
}
```

Contains information about a panic.

## Usage

### In catch Pattern

```ori
let result = catch(expr: risky_operation())
match(result,
    Ok(v) -> v,
    Err(msg) -> handle_panic(msg),  // msg is panic message string
)
```

Note: `catch` returns `Result<T, str>`, not `Result<T, PanicInfo>`.

### In Panic Hooks (Future)

```ori
// Future: custom panic handlers
@set_panic_hook (handler: (PanicInfo) -> void) -> void = ...

set_panic_hook(handler: info ->
    log(msg: `panic at {info.file}:{info.line}: {info.message}`)
)
```

## Standard Implementation

```ori
impl Printable for PanicInfo {
    @to_str (self) -> str =
        `panic at {self.file}:{self.line}:{self.column}: {self.message}`
}

impl Debug for PanicInfo {
    @debug (self) -> str =
        `PanicInfo \{ message: {self.message.debug()}, file: {self.file.debug()}, line: {self.line}, column: {self.column} \}`
}
```

---

# Null Coalescing Operator (`??`)

## Semantics

The `??` operator provides a default for `None` or `Err`:

### With Option

```ori
let value = opt ?? default
// Equivalent to:
match(opt,
    Some(v) -> v,
    None -> default,
)
```

### With Result

```ori
let value = result ?? default
// Equivalent to:
match(result,
    Ok(v) -> v,
    Err(_) -> default,
)
```

## Short-Circuit Evaluation

The right side is only evaluated if needed:

```ori
Some(42) ?? expensive()   // expensive() NOT called, returns 42
None ?? expensive()       // expensive() called
```

## Chaining

```ori
first ?? second ?? third ?? default
// Try first, then second, then third, finally default
```

## Type Constraints

```ori
opt: Option<T> ?? default: T  -> T
result: Result<T, E> ?? default: T  -> T
```

The default must match the inner type.

## Precedence

`??` has the lowest precedence (level 14, after `||`):

```ori
a + b ?? c  // Parsed as: (a + b) ?? c

// Precedence examples:
x && y ?? z     // (x && y) ?? z
a || b ?? c     // (a || b) ?? z
list[0] ?? d    // (list[0]) ?? d
```

Use parentheses for the less common case:

```ori
a + (b ?? c)    // Add a to (b-or-default)
```

---

## Error Messages

### compile_error

```
error: Windows not supported
  --> src/platform.ori:5:5
   |
 5 |     compile_error(msg: "Windows not supported")
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ explicit compile error
```

---

## Spec Changes Required

### Update `11-built-in-functions.md`

Add sections for:
1. `repeat` function
2. `compile_error` function

### Update `06-types.md`

Add `PanicInfo` type definition.

### Update `09-expressions.md`

Confirm null coalescing operator section with precedence clarification.

---

## Summary

| Item | Type | Purpose |
|------|------|---------|
| `repeat` | Function | Infinite iterator of cloned values |
| `compile_error` | Function | Compile-time error |
| `PanicInfo` | Type | Panic location and message |
| `??` | Operator | Default for None/Err (already in spec) |
