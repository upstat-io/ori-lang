# Associated Functions Proposal

**Status:** Approved
**Author:** Claude
**Created:** 2026-01-31
**Approved:** 2026-01-31

## Summary

Add support for associated functions (static methods) on types, enabling syntax like `Duration.from_seconds(s: 10)` and `Size.from_bytes(b: 1024)`.

## Motivation

The spec defines factory methods for Duration and Size types:

```ori
impl Duration {
    @from_nanoseconds (ns: int) -> Duration
    @from_seconds (s: int) -> Duration
    // ...
}
```

These methods don't have `self` as a parameter - they're associated functions, not instance methods. Currently, method calls require a receiver value (`value.method()`), but associated functions are called on the type itself (`Type.method()`).

### Use Cases

1. **Factory methods**: Create values with explicit semantics
   - `Duration.from_seconds(s: 10)` instead of `10s`
   - `Size.from_megabytes(mb: 100)` instead of `100mb`

2. **Constructor alternatives**: Named constructors for complex types
   - `Point.origin()` instead of `Point { x: 0, y: 0 }`
   - `Result.ok(value:)` and `Result.err(error:)`

3. **Namespace organization**: Group related functions with their type
   - `Math.sqrt(x:)` instead of `sqrt(x:)`
   - `Random.int(min:, max:)` instead of standalone functions

## Design

### Syntax

Associated functions are defined in `impl` blocks without a `self` parameter:

```ori
impl Duration {
    // Instance method (has self)
    @seconds (self) -> int = ...

    // Associated function (no self)
    @from_seconds (s: int) -> Duration = ...
}
```

### Calling Syntax

Associated functions are called using `Type.method(args)`:

```ori
let d = Duration.from_seconds(s: 10)
let s = Size.from_megabytes(mb: 100)
```

### Type Resolution

When parsing `Ident.method(...)`:
1. If `Ident` resolves to a value → instance method call
2. If `Ident` resolves to a type name → associated function call
3. Otherwise → error

### Self in Associated Functions

Associated functions may use `Self` as a return type, referring to the implementing type:

```ori
impl Point {
    @origin () -> Self = Point { x: 0, y: 0 }
    @new (x: int, y: int) -> Self = Point { x, y }
}
```

`Self` refers to the type being implemented, including any type parameters. It is only valid in the return type position, not as a parameter type (for associated functions).

### Generic Types

For generic types, full type arguments are required:

```ori
let x: Option<int> = Option<int>.some(value: 42)
let r: Result<str, Error> = Result<str, Error>.ok(value: "success")
```

Type inference does not apply to the type name prefix in associated function calls.

### Visibility

Associated functions follow the same visibility rules as instance methods:

```ori
impl Point {
    pub @new (x: int, y: int) -> Point = Point { x, y }  // Public
    @internal () -> Point = Point { x: 0, y: 0 }         // Private to module
}
```

In a `pub impl` block, all associated functions are public.

### Built-in Types

For primitive types (int, float, Duration, Size, etc.), associated functions are registered as built-in methods that don't require `self`.

## Trait Associated Functions

Traits may define associated functions that implementors must provide:

```ori
trait Default {
    @default () -> Self
}

impl Default for Point {
    @default () -> Self = Point { x: 0, y: 0 }
}
```

Calling associated functions via trait objects is not allowed (requires concrete type):

```ori
@make_default (d: Default) -> Default = d.default()  // ERROR: cannot call associated function on trait object
```

This is consistent with object safety rules — associated functions returning `Self` prevent trait object usage.

## Design Decisions

### Extensions Cannot Have Associated Functions

Extensions cannot define associated functions (methods without `self`). Use inherent `impl` blocks for associated functions.

```ori
// ERROR: extensions cannot have associated functions
extend Point {
    @origin () -> Point = Point { x: 0, y: 0 }  // No self parameter
}

// OK: use impl block
impl Point {
    @origin () -> Point = Point { x: 0, y: 0 }
}
```

### Dot Syntax, Not Double Colon

Associated functions use `.` syntax for consistency with method calls:

```ori
Duration.from_seconds(s: 10)   // Ori style
// Not: Duration::from_seconds(s: 10)  // Rust style
```

This maintains uniform syntax for all member access.

## Implementation

### Phase 1: Type Names as Values

Make type names usable in expression position when followed by `.`:
- `Duration.from_seconds(...)` - type followed by method access
- Parser recognizes `TypeName.method(...)` pattern

### Phase 2: Associated Function Registry

Extend the method registry to support methods without `self`:
- `BuiltinMethodRegistry` gains `check_associated(type, method, args)`
- User `impl` blocks can define methods without `self`

### Phase 3: Built-in Factory Methods

Implement Duration/Size factory methods:
- `Duration.from_nanoseconds(ns:)`, `.from_microseconds(us:)`, etc.
- `Size.from_bytes(b:)`, `.from_kilobytes(kb:)`, etc.

## Alternatives Considered

### Module-Level Functions

Instead of `Duration.from_seconds(s: 10)`, use `duration_from_seconds(s: 10)`:

```ori
let d = duration_from_seconds(s: 10)
```

**Rejected**: Less discoverable, pollutes namespace, doesn't match spec.

### Constructor Syntax

Use the type as a function: `Duration(seconds: 10)`:

```ori
let d = Duration(seconds: 10)
```

**Rejected**: Conflicts with newtype construction syntax, less explicit about which unit.

## References

- Spec: `docs/ori_lang/0.1-alpha/spec/06-types.md` § Duration, Size
- Roadmap: `plans/roadmap/phase-01-type-system.md` § 1.1A Duration/Size factory methods
