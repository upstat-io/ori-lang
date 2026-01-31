# Move Duration and Size to Standard Library

**Status:** Draft
**Author:** Claude
**Created:** 2026-01-31
**Depends On:** operator-traits-proposal.md, associated-functions-language-feature.md

## Summary

Remove Duration and Size as compiler built-in types and implement them as regular Ori types in the standard library prelude, using pure language features.

## Motivation

Currently, Duration and Size are deeply embedded in the compiler:

- Special token types in the lexer (`DurationLiteral`, `SizeLiteral`)
- Built-in `Type::Duration` and `Type::Size` variants
- Hardcoded operator implementations in `ori_eval/src/operators.rs`
- Hardcoded method dispatch in `ori_eval/src/methods.rs`
- Special type checking in `ori_typeck/src/operators.rs`

This creates maintenance burden and prevents the language from being self-hosting. If Duration and Size can be implemented in pure Ori, it validates that the language's type system and abstraction mechanisms are sufficient for real-world use.

### Benefits

1. **Reduced compiler complexity**: Remove ~500 lines of special-case code
2. **Validation of language features**: Proves operator overloading and associated functions work
3. **User extensibility**: Users can create similar unit types (Temperature, Currency, etc.)
4. **Consistency**: All types follow the same rules

## Design

### Duration Implementation

```ori
// library/std/duration.ori

/// Duration represents a span of time in nanoseconds.
#derive(Eq, Comparable, Hashable, Clone, Debug, Default)
pub type Duration = { nanoseconds: int }

impl Add for Duration {
    @add (self, other: Duration) -> Duration =
        Duration { nanoseconds: self.nanoseconds + other.nanoseconds }
}

impl Sub for Duration {
    @sub (self, other: Duration) -> Duration =
        Duration { nanoseconds: self.nanoseconds - other.nanoseconds }
}

impl Mul<int> for Duration {
    @mul (self, n: int) -> Duration =
        Duration { nanoseconds: self.nanoseconds * n }
}

impl Div<int> for Duration {
    @div (self, n: int) -> Duration =
        Duration { nanoseconds: self.nanoseconds / n }
}

impl Neg for Duration {
    @neg (self) -> Duration =
        Duration { nanoseconds: -self.nanoseconds }
}

impl Duration {
    // Factory methods (associated functions)
    pub @from_nanoseconds (ns: int) -> Self = Duration { nanoseconds: ns }
    pub @from_microseconds (us: int) -> Self = Duration { nanoseconds: us * 1_000 }
    pub @from_milliseconds (ms: int) -> Self = Duration { nanoseconds: ms * 1_000_000 }
    pub @from_seconds (s: int) -> Self = Duration { nanoseconds: s * 1_000_000_000 }
    pub @from_minutes (m: int) -> Self = Duration { nanoseconds: m * 60_000_000_000 }
    pub @from_hours (h: int) -> Self = Duration { nanoseconds: h * 3_600_000_000_000 }

    // Extraction methods
    pub @nanoseconds (self) -> int = self.nanoseconds
    pub @microseconds (self) -> int = self.nanoseconds / 1_000
    pub @milliseconds (self) -> int = self.nanoseconds / 1_000_000
    pub @seconds (self) -> int = self.nanoseconds / 1_000_000_000
    pub @minutes (self) -> int = self.nanoseconds / 60_000_000_000
    pub @hours (self) -> int = self.nanoseconds / 3_600_000_000_000
}

impl Printable for Duration {
    @to_str (self) -> str = run(
        let ns = self.nanoseconds,
        if ns % 3_600_000_000_000 == 0 then `{ns / 3_600_000_000_000}h`
        else if ns % 60_000_000_000 == 0 then `{ns / 60_000_000_000}m`
        else if ns % 1_000_000_000 == 0 then `{ns / 1_000_000_000}s`
        else if ns % 1_000_000 == 0 then `{ns / 1_000_000}ms`
        else if ns % 1_000 == 0 then `{ns / 1_000}us`
        else `{ns}ns`,
    )
}
```

### Size Implementation

```ori
// library/std/size.ori

/// Size represents a byte count.
#derive(Eq, Comparable, Hashable, Clone, Debug, Default)
pub type Size = { bytes: int }

impl Add for Size {
    @add (self, other: Size) -> Size =
        Size { bytes: self.bytes + other.bytes }
}

impl Sub for Size {
    @sub (self, other: Size) -> Size = run(
        let result = self.bytes - other.bytes,
        if result < 0 then panic(msg: "Size cannot be negative"),
        Size { bytes: result },
    )
}

impl Mul<int> for Size {
    @mul (self, n: int) -> Size = run(
        let result = self.bytes * n,
        if result < 0 then panic(msg: "Size cannot be negative"),
        Size { bytes: result },
    )
}

impl Div<int> for Size {
    @div (self, n: int) -> Size = Size { bytes: self.bytes / n }
}

impl Size {
    // Factory methods (associated functions)
    pub @from_bytes (b: int) -> Self = run(
        if b < 0 then panic(msg: "Size cannot be negative"),
        Size { bytes: b },
    )
    pub @from_kilobytes (kb: int) -> Self = Self.from_bytes(b: kb * 1024)
    pub @from_megabytes (mb: int) -> Self = Self.from_bytes(b: mb * 1024 * 1024)
    pub @from_gigabytes (gb: int) -> Self = Self.from_bytes(b: gb * 1024 * 1024 * 1024)
    pub @from_terabytes (tb: int) -> Self = Self.from_bytes(b: tb * 1024 * 1024 * 1024 * 1024)

    // Extraction methods
    pub @bytes (self) -> int = self.bytes
    pub @kilobytes (self) -> int = self.bytes / 1024
    pub @megabytes (self) -> int = self.bytes / (1024 * 1024)
    pub @gigabytes (self) -> int = self.bytes / (1024 * 1024 * 1024)
    pub @terabytes (self) -> int = self.bytes / (1024 * 1024 * 1024 * 1024)
}
```

## Missing Language Features

To implement Duration and Size in pure Ori, the following features are required:

### 1. Operator Traits (REQUIRED)

Traits that types can implement to support operators:

```ori
trait Add<Rhs = Self> {
    type Output = Self
    @add (self, rhs: Rhs) -> Self.Output
}

trait Sub<Rhs = Self> {
    type Output = Self
    @sub (self, rhs: Rhs) -> Self.Output
}

trait Mul<Rhs> {
    type Output
    @mul (self, rhs: Rhs) -> Self.Output
}

trait Div<Rhs> {
    type Output
    @div (self, rhs: Rhs) -> Self.Output
}

trait Neg {
    type Output = Self
    @neg (self) -> Self.Output
}

trait Rem<Rhs = Self> {
    type Output = Self
    @rem (self, rhs: Rhs) -> Self.Output
}
```

The compiler must recognize these traits and dispatch operators to their methods:
- `a + b` → `a.add(rhs: b)`
- `a - b` → `a.sub(rhs: b)`
- `a * b` → `a.mul(rhs: b)`
- `a / b` → `a.div(rhs: b)`
- `-a` → `a.neg()`
- `a % b` → `a.rem(rhs: b)`

**Status**: NOT IMPLEMENTED - operators are hardcoded for built-in types only

### 2. Literal Suffixes (REQUIRED for ergonomics)

The syntax `10s`, `5mb` requires compiler support. Options:

#### Option A: Keep Literal Suffixes in Compiler (Recommended)

Literal suffixes remain compiler-recognized but desugar to associated function calls:

```ori
10s      // desugars to: Duration.from_seconds(s: 10)
5mb      // desugars to: Size.from_megabytes(mb: 5)
100ns    // desugars to: Duration.from_nanoseconds(ns: 100)
```

The compiler:
1. Recognizes suffix patterns in lexer
2. Generates AST for the associated function call
3. Type checks and evaluates normally

This keeps the ergonomic syntax while moving implementation to the library.

#### Option B: User-Defined Literal Suffixes

Allow users to define custom suffixes:

```ori
#suffix("s")
@seconds_suffix (n: int) -> Duration = Duration.from_seconds(s: n)
```

**More complex, defer to future proposal.**

#### Option C: Remove Literal Suffixes

Require explicit factory calls:

```ori
let d = Duration.from_seconds(s: 10)  // instead of 10s
let s = Size.from_megabytes(mb: 5)    // instead of 5mb
```

**Rejected**: Too verbose, reduces language ergonomics significantly.

### 3. Default Trait Implementations with Self (NICE TO HAVE)

For traits like `Default`, the implementation could use `Self`:

```ori
impl Default for Duration {
    @default () -> Self = Duration { nanoseconds: 0 }
}
```

**Status**: Partially implemented, needs verification with associated functions.

### 4. Derive for Operator Traits (NICE TO HAVE)

Allow `#derive(Add, Sub, Mul, Div)` for newtypes that wrap numeric types:

```ori
#derive(Add, Sub, Mul, Div)
type Duration = { nanoseconds: int }
```

**Defer to future proposal.**

## Migration Plan

### Phase 1: Implement Operator Traits

1. Define `Add`, `Sub`, `Mul`, `Div`, `Neg`, `Rem` traits in prelude
2. Implement trait dispatch for operators in type checker
3. Implement trait dispatch for operators in evaluator
4. Add default implementations for built-in numeric types

### Phase 2: Literal Suffix Desugaring

1. Modify lexer to produce generic "suffixed literal" tokens
2. Modify parser to desugar to associated function calls
3. Remove special Duration/Size literal handling

### Phase 3: Move Duration/Size to Library

1. Create `library/std/duration.ori` and `library/std/size.ori`
2. Implement all methods using operator traits
3. Add to prelude exports
4. Remove compiler built-in Duration/Size types
5. Remove hardcoded operator implementations
6. Remove hardcoded method dispatch

### Phase 4: Cleanup

1. Remove `Type::Duration` and `Type::Size` variants
2. Remove Duration/Size from `Value` enum (use regular struct values)
3. Remove special-case code throughout compiler

## Testing

1. All existing Duration/Size tests must continue to pass
2. New tests for operator trait implementations
3. New tests for user-defined types with operator traits
4. Performance comparison (should be equivalent)

## Risks

1. **Performance**: Trait dispatch may be slower than hardcoded operators
   - Mitigation: Inline trait method calls during compilation

2. **Error messages**: Generic trait errors less clear than built-in type errors
   - Mitigation: Special-case error messages for common operator trait failures

3. **Bootstrapping**: Need operator traits before Duration/Size can be implemented
   - Mitigation: Implement operator traits first, keep built-ins until ready

## Alternatives Considered

### Keep Duration/Size as Built-ins

Continue with hardcoded implementation.

**Rejected**: Prevents language self-hosting, maintains technical debt.

### Implement Only Some Methods in Library

Keep operators hardcoded but move factory methods to library.

**Rejected**: Half-measure that doesn't address core issue.

## References

- Current Duration implementation: `compiler/ori_eval/src/methods.rs`
- Current Size implementation: `compiler/ori_eval/src/methods.rs`
- Current operator handling: `compiler/ori_eval/src/operators.rs`
- Rust's approach: `std::ops` module with operator traits
