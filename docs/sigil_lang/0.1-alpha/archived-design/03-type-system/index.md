# Type System

This section covers Sigil's type system: primitives, compound types, user-defined types, generics, inference, and the compositional model.

---

## Documents

| Document | Description |
|----------|-------------|
| [Primitive Types](01-primitive-types.md) | int, float, bool, str, char, byte, void, Never |
| [Compound Types](02-compound-types.md) | List, Map, Set, Tuple, Range, Option, Result, Ordering |
| [User-Defined Types](03-user-defined-types.md) | Structs, newtypes, sum types |
| [Generics](04-generics.md) | Generic types and functions |
| [Type Inference](05-type-inference.md) | Inference rules and boundaries |
| [Compositional Model](06-compositional-model.md) | No subtyping, traits over inheritance |

---

## Overview

Sigil's type system is:

- **Nominal** - Types are distinct by name, not structure
- **Strong** - No implicit conversions
- **Static** - All types known at compile time
- **Inferred** - Types inferred within functions, explicit at boundaries

### Type Categories

| Category | Examples |
|----------|----------|
| Primitives | `int`, `float`, `bool`, `str`, `char`, `byte`, `void`, `Never` |
| Collections | `[T]` (list), `{K: V}` (map), `Set<T>` (set) |
| Ranges | `Range<T>` (`0..10`, `'a'..='z'`) |
| Tuples | `(T, U)`, `(T, U, V)` |
| Optional | `Option<T>` (Some/None) |
| Result | `Result<T, E>` (Ok/Err) |
| Ordering | `Ordering` (Less/Equal/Greater) |
| User-defined | `type Point = { x: int, y: int }` |
| Sum types | `type Status = Pending \| Running \| Done` |
| Functions | `(int, int) -> int` |

### Key Principles

1. **No null** - Use `Option<T>` for optional values
2. **No exceptions** - Use `Result<T, E>` for errors
3. **No subtyping** - Types match exactly or they don't
4. **Explicit conversions** - All type conversions are visible

---

## See Also

- [Main Index](../00-index.md)
- [Traits](../04-traits/index.md)
- [Error Handling](../05-error-handling/index.md)
