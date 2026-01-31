# Associated Functions as a Language Feature

**Status:** Approved
**Approved:** 2026-01-31
**Author:** Claude
**Created:** 2026-01-31
**Supersedes:** associated-functions-proposal.md
**Depends On:** None

## Summary

Implement associated functions as a general language feature that works for any type with an `impl` block, rather than hardcoding support for specific types like Duration and Size.

## Motivation

The current implementation of associated functions is a hack:

```rust
fn is_type_name_for_associated_functions(name: &str) -> bool {
    matches!(name, "Duration" | "Size")
}
```

This hardcodes specific type names in the compiler. Associated functions should work for ANY user-defined or built-in type that has methods defined without `self` in an `impl` block.

### Use Cases

1. **User-defined constructors**:
   ```ori
   type Point = { x: int, y: int }

   impl Point {
       @origin () -> Self = Point { x: 0, y: 0 }
       @new (x: int, y: int) -> Self = Point { x, y }
   }

   let p = Point.origin()
   let q = Point.new(x: 10, y: 20)
   ```

2. **Builder pattern**:
   ```ori
   type Config = { host: str, port: int, timeout: Duration }

   impl Config {
       @default () -> Self = Config { host: "localhost", port: 8080, timeout: 30s }
       @with_host (self, host: str) -> Self = Config { ...self, host }
       @with_port (self, port: int) -> Self = Config { ...self, port }
       @with_timeout (self, timeout: Duration) -> Self = Config { ...self, timeout }
   }

   let cfg = Config.default().with_host(host: "example.com").with_port(port: 443)
   ```

3. **Factory methods on generic types**:
   ```ori
   impl<T> Option<T> {
       @some (value: T) -> Self = Some(value)
       @none () -> Self = None
   }

   let x = Option<int>.some(value: 42)
   ```

4. **Namespace organization**:
   ```ori
   type Math = {}

   impl Math {
       @sqrt (x: float) -> float = ...
       @abs (x: float) -> float = ...
   }

   let r = Math.sqrt(x: 2.0)
   ```

## Design

### Definition

An *associated function* is a function defined in an `impl` block without a `self` parameter:

```ori
impl MyType {
    // Instance method (has self)
    @get_value (self) -> int = self.value

    // Associated function (no self)
    @create (value: int) -> Self = MyType { value }
}
```

### Calling Syntax

Associated functions are called using `Type.method(args)`:

```ori
let instance = MyType.create(value: 42)
```

### Type Resolution

When the type checker encounters `Ident.method(...)`:

1. Look up `Ident` in the current scope
2. If it resolves to a **value** → treat as instance method call on that value
3. If it resolves to a **type name** → look up associated function in that type's impl blocks
4. If no associated function found → error

### Impl Block Processing

When processing `impl` blocks, distinguish between:
- Methods with `self` parameter → register as instance methods
- Methods without `self` parameter → register as associated functions

### Chaining

Associated functions that return `Self` enable method chaining:

```ori
impl Builder {
    @new () -> Self = Builder { ... }
    @with_name (self, name: str) -> Self = Builder { ...self, name }
    @with_value (self, value: int) -> Self = Builder { ...self, value }
    @build (self) -> Product = ...
}

let product = Builder.new()
    .with_name(name: "example")
    .with_value(value: 42)
    .build()
```

### Generic Types

For generic types, type arguments must be provided:

```ori
impl<T> Option<T> {
    @some (value: T) -> Self = Some(value)
}

let x = Option<int>.some(value: 42)
```

### Visibility

Associated functions follow the same visibility rules as instance methods:

```ori
impl Point {
    pub @new (x: int, y: int) -> Point = Point { x, y }  // Public
    @internal () -> Point = Point { x: 0, y: 0 }         // Private
}
```

### Error Cases

It is an error if:
- A type name is used in expression position except as the receiver of a method call
- An associated function is called on a type that has no such function defined
- A local binding shadows a type name when the intent is to call an associated function

```ori
let Duration = 10
let d = Duration.from_seconds(s: 5)  // Error: Duration is a value (int), not a type
                                      // Rename variable to access type's associated functions
```

## Migration

1. Remove `is_type_name_for_associated_functions()` checks in type checker and evaluator
2. Implement general associated function lookup via impl blocks
3. Register Duration/Size factory methods as impl-block associated functions

## Testing

Tests should cover:

1. **Basic associated functions** on user-defined types
2. **Method chaining** with builder pattern (`Type.new().configure().build()`)
3. **Generic types** with type parameters
4. **Self return type** usage
5. **Visibility** (pub vs private associated functions)
6. **Shadowing** (local variable shadows type name)
7. **Error cases** (calling non-existent associated function)

## Alternatives Considered

### Keep Hardcoded Type Names

Continue adding types to `is_type_name_for_associated_functions()` as needed.

**Rejected**: Doesn't scale, prevents user-defined associated functions.

### Separate Syntax for Associated Functions

Use `Type::method()` (Rust-style) instead of `Type.method()`.

**Rejected**: Inconsistent with Ori's uniform dot syntax for member access.

## References

- Current implementation: `compiler/ori_typeck/src/infer/call.rs`
- Spec: `docs/ori_lang/0.1-alpha/spec/08-declarations.md` § Associated Functions
