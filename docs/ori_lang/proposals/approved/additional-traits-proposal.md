# Proposal: Additional Core Traits

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Approved:** 2026-01-30
**Affects:** Compiler, type system, traits

---

## Summary

This proposal formalizes three core traits: `Printable`, `Default`, and `Traceable`.

The `Iterable`, `Iterator`, `DoubleEndedIterator`, and `Collect` traits are already defined in the [Types specification](../../0.1-alpha/spec/06-types.md#iterator-traits) and are not modified by this proposal.

---

# Printable Trait

## Definition

```ori
trait Printable {
    @to_str (self) -> str
}
```

`Printable` provides human-readable string conversion.

## Usage

Required for string interpolation without format specs:

```ori
let x = 42
`value: {x}`  // Calls x.to_str()
```

## Standard Implementations

| Type | Output |
|------|--------|
| `int` | `"42"` |
| `float` | `"3.14"` |
| `bool` | `"true"` or `"false"` |
| `str` | Identity |
| `char` | Single character string |
| `byte` | Numeric string |
| `[T]` where `T: Printable` | `"[1, 2, 3]"` |
| `Option<T>` where `T: Printable` | `"Some(42)"` or `"None"` |
| `Result<T, E>` where both Printable | `"Ok(42)"` or `"Err(msg)"` |

## Derivation

```ori
#derive(Printable)
type Point = { x: int, y: int }

Point { x: 1, y: 2 }.to_str()  // "Point(1, 2)"
```

Derived implementation creates human-readable format with type name and field values in order.

---

# Default Trait

## Definition

```ori
trait Default {
    @default () -> Self
}
```

`Default` provides zero/empty values.

## Usage

```ori
let config: Config = Config.default()
let map: {str: int} = {str: int}.default()  // {}
```

## Standard Implementations

| Type | Default Value |
|------|---------------|
| `int` | `0` |
| `float` | `0.0` |
| `bool` | `false` |
| `str` | `""` |
| `byte` | `0` |
| `char` | `'\0'` |
| `void` | `()` |
| `[T]` | `[]` |
| `{K: V}` | `{}` |
| `Set<T>` | `Set.new()` |
| `Option<T>` | `None` |
| `Duration` | `0ns` |
| `Size` | `0b` |

## Derivation

```ori
#derive(Default)
type Config = {
    host: str,    // ""
    port: int,    // 0
    debug: bool,  // false
}
```

All fields must implement `Default`.

## Cannot Derive

Sum types cannot derive `Default` (ambiguous variant):

```ori
#derive(Default)  // ERROR
type Status = Pending | Running | Done
```

---

# Traceable Trait

## Definition

```ori
trait Traceable {
    @with_trace (self, entry: TraceEntry) -> Self
    @trace (self) -> str
    @trace_entries (self) -> [TraceEntry]
    @has_trace (self) -> bool
}
```

`Traceable` enables error trace propagation.

## TraceEntry Type

```ori
type TraceEntry = {
    function: str,   // Function name with @ prefix
    file: str,       // Source file path
    line: int,       // Line number
    column: int,     // Column number
}
```

## Usage

### Automatic Trace Addition

The `?` operator automatically adds trace entries:

```ori
@outer () -> Result<int, Error> = {
    let x = inner()?,  // Adds trace entry for this location
    Ok(x * 2)
}
```

### Manual Trace Access

```ori
match result {
    Ok(v) -> v
    Err(e) -> {
        for entry in e.trace_entries() do
            log(msg: `  at {entry.function} ({entry.file}:{entry.line})`)
        panic(msg: e.message)
    }
}
```

## Standard Implementations

| Type | Implements |
|------|------------|
| `Error` | Yes |
| `Result<T, E>` where `E: Traceable` | Yes (delegates to E) |

## Error Type

```ori
type Error = {
    message: str,
    source: Option<Error>,
    // trace stored internally
}

impl Traceable for Error {
    @with_trace (self, entry: TraceEntry) -> Error = ...
    @trace (self) -> str = ...  // Formatted trace string
    @trace_entries (self) -> [TraceEntry] = ...
    @has_trace (self) -> bool = ...
}
```

---

## Relationships

```
             Eq
            /  \
     Hashable  Comparable

     Iterator
         |
DoubleEndedIterator

     Printable
         |
    Formattable (blanket impl)

     Traceable (for error types)
```

---

## Error Messages

### Missing Printable

```
error[E1040]: `MyType` does not implement `Printable`
  --> src/main.ori:5:15
   |
 5 | let s = `value: {my_value}`
   |                  ^^^^^^^^ cannot convert to string
   |
   = help: implement `Printable` or derive it
```

### Cannot Derive Default

```
error[E1042]: cannot derive `Default` for sum type
  --> src/main.ori:2:1
   |
 2 | type Status = Pending | Running | Done
   | ^^^^^^ sum types have no unambiguous default variant
   |
   = help: implement `Default` manually if needed
```

---

## Spec Changes Required

### Update `07-properties-of-types.md`

Add sections for `Printable`, `Default`, and `Traceable` with:
1. Definition
2. Standard implementations
3. Derivation rules
4. Usage examples

---

## Summary

| Trait | Purpose | Derivable |
|-------|---------|-----------|
| `Printable` | String conversion | Yes |
| `Default` | Zero/empty values | Yes (structs only) |
| `Traceable` | Error traces | No |
