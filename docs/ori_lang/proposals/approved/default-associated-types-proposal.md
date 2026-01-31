# Default Associated Types

**Status:** Approved
**Approved:** 2026-01-31
**Author:** Claude
**Created:** 2026-01-31
**Depends On:** None
**Enables:** operator-traits-proposal.md (with default-type-parameters-proposal.md)
**Parallel With:** default-type-parameters-proposal.md

## Summary

Allow associated types in traits to have default values, enabling `type Output = Self` where implementors can omit the associated type if the default is acceptable.

## Motivation

Without default associated types, every impl must specify all associated types even when the default would suffice:

```ori
// Today: verbose
impl Add for Vector2 {
    type Output = Vector2  // Required even though it's always Self
    @add (self, rhs: Vector2) -> Vector2 = ...
}

// Goal: omit when default applies
impl Add for Vector2 {
    @add (self, rhs: Vector2) -> Self = ...  // Output defaults to Self
}
```

Most operator implementations return `Self`. Requiring explicit `type Output = Self` in every impl is boilerplate.

## Design

### Syntax

Default associated types use `= Type` after the type name:

```ori
trait Add<Rhs = Self> {
    type Output = Self  // Defaults to implementing type
    @add (self, rhs: Rhs) -> Self.Output
}

trait Iterator {
    type Item  // No default - must be specified
    @next (self) -> Option<Self.Item>
}

trait IntoIterator {
    type Item
    type Iter: Iterator = Self  // Default to Self if Self: Iterator
    @into_iter (self) -> Self.Iter
}
```

### Semantics

1. Default applies when impl omits the associated type
2. `Self` in default position refers to the implementing type
3. Defaults are evaluated at impl site
4. Defaults can reference type parameters and other associated types

```ori
trait Container {
    type Item
    type Iter = [Self.Item]  // Default references another associated type
}
```

### Self Resolution

`Self` in a default associated type refers to the implementing type:

```ori
trait Add<Rhs = Self> {
    type Output = Self
    @add (self, rhs: Rhs) -> Self.Output
}

impl Add for Point {
    // Output = Self = Point (default)
    @add (self, rhs: Point) -> Point = ...
}

impl Add<int> for Vector2 {
    type Output = Vector2  // Explicit override
    @add (self, rhs: int) -> Vector2 = ...
}
```

### Grammar Change

Current:
```ebnf
trait_item = associated_type | method_sig .
associated_type = "type" identifier [ ":" bounds ] .
```

Proposed:
```ebnf
trait_item = associated_type | method_sig .
associated_type = "type" identifier [ ":" bounds ] [ "=" type ] .
```

### Override Behavior

Impls can always override the default:

```ori
trait Add<Rhs = Self> {
    type Output = Self
    @add (self, rhs: Rhs) -> Self.Output
}

// Use default: Output = Self = BigInt
impl Add for BigInt {
    @add (self, rhs: BigInt) -> Self = ...
}

// Override: Output = bool (not Self)
impl Add for Set {
    type Output = bool  // Union returns whether any new elements added
    @add (self, rhs: Set) -> bool = ...
}
```

### Bounds on Defaults

Defaults must satisfy any bounds on the associated type:

```ori
trait Process {
    type Output: Clone = Self  // Default only valid if Self: Clone
    @process (self) -> Self.Output
}

impl Process for String {  // OK: String: Clone
    @process (self) -> Self = self.clone()
}

impl Process for Connection {  // ERROR if Connection: !Clone and no override
    // Must provide explicit Output type since Self doesn't satisfy Clone
    type Output = ConnectionHandle
    @process (self) -> ConnectionHandle = ...
}
```

## Semantics Details

### Bounds Checking with Defaults

When an impl uses a default associated type:

1. Substitute `Self` with the implementing type
2. Substitute any referenced associated types
3. Verify the resulting type satisfies all bounds on the associated type

If the default does not satisfy bounds after substitution, it is a compile error at the impl site. The impl must provide an explicit associated type.

### Self in Trait Definition Scope

`Self` in a default associated type refers to the implementing type. This is resolved at the impl site, not at trait definition time. This follows the same semantics as default type parameters.

## Implementation

### Type Checker Changes

1. Parse default type in `associated_type` grammar rule
2. When checking `impl Trait for Type`:
   - Collect provided associated types
   - For missing associated types with defaults:
     - Substitute `Self` with implementing type
     - Substitute other associated types if referenced
     - Verify default satisfies bounds
   - Proceed with normal impl checking

### Example Resolution

```ori
trait Add<Rhs = Self> {
    type Output = Self
    @add (self, rhs: Rhs) -> Self.Output
}

impl Add for Point {
    @add (self, rhs: Point) -> Point = ...
}
```

Resolution steps:
1. `impl Add for Point` - no associated types provided
2. Trait has `type Output = Self` default
3. Substitute `Self` → `Point`
4. Result: `Output = Point`

## Interaction with Default Type Parameters

Default associated types can reference default type parameters:

```ori
trait Convert<T = Self> {
    type Output = T  // References type parameter
    @convert (self) -> Self.Output
}

impl Convert for String {
    // T = Self = String (from default type param)
    // Output = T = String (from default associated type)
    @convert (self) -> String = self.clone()
}
```

## Alternatives Considered

### No Defaults

Require all associated types to be specified.

**Rejected:** Too verbose for operator traits where Output is almost always Self.

### Inference from Method Signatures

Infer associated types from method return types.

**Rejected:** Complex, potentially ambiguous, less explicit.

## References

- Rust: Default associated types (stabilized in 1.0 for some cases)
- Swift: Associated type defaults
- Current grammar: `docs/ori_lang/0.1-alpha/spec/grammar.ebnf`
