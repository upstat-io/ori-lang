# std

The core module providing fundamental utilities.

```sigil
use std { ... }
```

---

## Overview

The `std` module re-exports commonly used items from submodules for convenience. Most items are better imported from their specific modules.

---

## Re-exports

| Item | From | Description |
|------|------|-------------|
| `read_file` | `std.fs` | Read file contents |
| `write_file` | `std.fs` | Write file contents |
| `Date` | `std.time` | Date type |
| `Time` | `std.time` | Time type |
| `DateTime` | `std.time` | Combined date and time |

---

## Functions

### @assert

```sigil
@assert (condition: bool) -> void
@assert (condition: bool, message: str) -> void
```

Panics if condition is false. Used for invariant checking.

```sigil
assert(x > 0)
assert(x > 0, "x must be positive")
```

> **Note:** For tests, use `std.testing` assertions which provide better error messages.

---

### @assert_eq

```sigil
@assert_eq<T: Eq + Printable> (actual: T, expected: T) -> void
```

Panics if values are not equal, showing both values.

```sigil
assert_eq(
    .actual: result,
    .expected: expected,
)
// Panic: assertion failed: 41 != 42
```

---

### @todo

```sigil
@todo (message: str) -> Never
```

Marks unfinished code. Panics with the message.

```sigil
@process_payment (order: Order) -> Result<Receipt, Error> =
    todo("implement payment processing")
```

---

### @unreachable

```sigil
@unreachable () -> Never
```

Marks code that should never execute.

```sigil
match(status,
    Active -> handle_active(),
    Inactive -> handle_inactive(),
    _ -> unreachable(),  // enum is exhaustive
)
```

---

### @dbg

```sigil
@dbg<T: Printable> (value: T) -> T
```

Prints value with source location and returns it. For debugging.

```sigil
let result = dbg(calculate(x))
// [src/main.si:42] calculate(x) = 123
```

---

## See Also

- [Prelude](../prelude.md) — Auto-imported items
- [std.fs](../std.fs/) — Filesystem operations
- [std.time](../std.time/) — Date and time
