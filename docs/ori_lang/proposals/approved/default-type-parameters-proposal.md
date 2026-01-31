# Default Type Parameters on Traits

**Status:** Approved
**Approved:** 2026-01-31
**Author:** Claude
**Created:** 2026-01-31
**Depends On:** None
**Enables:** operator-traits-proposal.md (with default-associated-types-proposal.md)
**Parallel With:** default-associated-types-proposal.md

## Summary

Allow type parameters on traits to have default values, enabling `trait Add<Rhs = Self>` where `Rhs` defaults to `Self` if not specified.

## Motivation

Without default type parameters, every impl must specify all type arguments:

```ori
// Today: verbose
impl Add<Vector2> for Vector2 { ... }
impl Eq<Vector2> for Vector2 { ... }

// Goal: concise when types match
impl Add for Vector2 { ... }  // Rhs defaults to Self = Vector2
impl Eq for Vector2 { ... }   // Rhs defaults to Self = Vector2
```

This is especially important for operator traits where the common case is operating on the same type.

## Design

### Syntax

Default type parameters use `= Type` after the parameter name:

```ori
trait Add<Rhs = Self> {
    type Output
    @add (self, rhs: Rhs) -> Self.Output
}

trait Convert<Target = Self> {
    @convert (self) -> Target
}
```

### Semantics

1. Default applies when impl omits the type argument
2. `Self` in default position refers to the implementing type
3. Defaults are evaluated at impl site, not trait definition site
4. Parameters with defaults must appear after all parameters without defaults

```ori
trait Example<T = int, U = T> {
    @method (self, t: T, u: U) -> void
}

// These are equivalent:
impl Example for Foo { ... }
impl Example<int, int> for Foo { ... }

// Partial specification:
impl Example<str> for Bar { ... }  // U defaults to T = str
impl Example<str, str> for Bar { ... }  // equivalent
```

Ordering constraint example:

```ori
// Valid: default after non-default
trait Transform<Input, Output = Input> { ... }

// Invalid: non-default after default
trait Invalid<T = int, U> { ... }  // Error: non-default parameter after default
```

### Self Resolution

`Self` may be used as a default value in trait type parameters. It refers to the implementing type at the impl site.

```ori
trait Add<Rhs = Self> { ... }

impl Add for Point { ... }
// Rhs = Self = Point

impl Add for Vector2 { ... }
// Rhs = Self = Vector2
```

### Grammar Change

Current:
```ebnf
type_param = identifier [ ":" bounds ] .
```

Proposed:
```ebnf
type_param = identifier [ ":" bounds ] [ "=" type ] .
```

### Multiple Defaults

Later parameters can reference earlier ones:

```ori
trait Transform<Input = Self, Output = Input> {
    @transform (self, input: Input) -> Output
}

impl Transform for Parser { ... }
// Input = Self = Parser, Output = Input = Parser

impl Transform<str> for Parser { ... }
// Input = str, Output = Input = str

impl Transform<str, Ast> for Parser { ... }
// Input = str, Output = Ast
```

## Implementation

### Type Checker Changes

1. Parse default type in `type_param` grammar rule
2. When checking `impl Trait for Type`:
   - Count provided type arguments
   - Fill missing arguments with defaults
   - Substitute `Self` with implementing type
   - Proceed with normal impl checking

### Example Resolution

```ori
trait Add<Rhs = Self> {
    @add (self, rhs: Rhs) -> Self
}

impl Add for Point {
    @add (self, rhs: Point) -> Self = ...
}
```

Resolution steps:
1. `impl Add for Point` - no type args provided
2. Trait has 1 type param with default `Self`
3. Substitute `Self` → `Point`
4. Result: `impl Add<Point> for Point`

## Alternatives Considered

### No Defaults

Require all type parameters to be specified.

**Rejected:** Too verbose for common cases like `impl Add for MyType`.

### Inference Instead of Defaults

Infer missing type parameters from usage.

**Rejected:** Less predictable, harder to understand, may conflict with other inference.

## References

- Rust: `trait Add<Rhs = Self>` in `std::ops`
- Scala: Default type parameters
- Grammar: See `grammar.ebnf` § Generics
