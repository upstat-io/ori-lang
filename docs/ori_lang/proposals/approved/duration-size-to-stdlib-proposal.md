# Move Duration and Size to Standard Library

**Status:** Approved
**Approved:** 2026-01-31
**Author:** Claude
**Created:** 2026-01-31
**Depends On:** operator-traits-proposal.md, associated-functions-language-feature.md
**Supersedes:** Portions of stdlib-philosophy-proposal.md (moves Duration/Size from Core to Stdlib)

## Summary

Remove Duration and Size as compiler built-in types and implement them as regular Ori types in the standard library prelude, using pure language features. Literal suffixes (`10s`, `5mb`) remain compiler-recognized but desugar to associated function calls.

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
3. **User extensibility**: Users can create similar unit types (Temperature, Currency, Angle, etc.)
4. **Consistency**: All types follow the same rules — no magic

## Design

### Literal Suffix Desugaring

Literal suffixes remain compiler-recognized but desugar to associated function calls:

```ori
10s      // desugars to: Duration.from_seconds(s: 10)
5mb      // desugars to: Size.from_megabytes(mb: 5)
100ns    // desugars to: Duration.from_nanoseconds(ns: 100)
1kb      // desugars to: Size.from_kilobytes(kb: 1)
```

The compiler:
1. Recognizes suffix patterns in lexer
2. Generates AST for the associated function call
3. Type checks and evaluates normally

This keeps the ergonomic syntax while moving implementation to the library.

### Duration Implementation

```ori
// library/std/duration.ori

/// Duration represents a span of time in nanoseconds.
#derive(Eq, Comparable, Hashable, Clone, Debug, Default, Sendable)
pub type Duration = { nanoseconds: int }

// Duration + Duration -> Duration
impl Add for Duration {
    @add (self, other: Duration) -> Duration =
        Duration { nanoseconds: self.nanoseconds + other.nanoseconds }
}

// Duration - Duration -> Duration
impl Sub for Duration {
    @subtract (self, other: Duration) -> Duration =
        Duration { nanoseconds: self.nanoseconds - other.nanoseconds }
}

// Duration * int -> Duration
impl Mul<int> for Duration {
    type Output = Duration
    @multiply (self, n: int) -> Duration =
        Duration { nanoseconds: self.nanoseconds * n }
}

// int * Duration -> Duration (commutative)
impl Mul<Duration> for int {
    type Output = Duration
    @multiply (self, d: Duration) -> Duration = d * self
}

// Duration / int -> Duration
impl Div<int> for Duration {
    type Output = Duration
    @divide (self, n: int) -> Duration =
        Duration { nanoseconds: self.nanoseconds / n }
}

// Duration / Duration -> int (ratio)
impl Div for Duration {
    type Output = int
    @divide (self, other: Duration) -> int =
        self.nanoseconds / other.nanoseconds
}

// Duration % Duration -> Duration (remainder)
impl Rem for Duration {
    @remainder (self, other: Duration) -> Duration =
        Duration { nanoseconds: self.nanoseconds % other.nanoseconds }
}

// -Duration -> Duration
impl Neg for Duration {
    @negate (self) -> Duration =
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

    // Extraction methods (truncate toward zero)
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

/// Size represents a byte count (non-negative).
#derive(Eq, Comparable, Hashable, Clone, Debug, Default, Sendable)
pub type Size = { bytes: int }

// Size + Size -> Size
impl Add for Size {
    @add (self, other: Size) -> Size =
        Size { bytes: self.bytes + other.bytes }
}

// Size - Size -> Size (panics if negative)
impl Sub for Size {
    @subtract (self, other: Size) -> Size = run(
        let result = self.bytes - other.bytes,
        if result < 0 then panic(msg: "Size cannot be negative"),
        Size { bytes: result },
    )
}

// Size * int -> Size (panics if negative)
impl Mul<int> for Size {
    type Output = Size
    @multiply (self, n: int) -> Size = run(
        let result = self.bytes * n,
        if result < 0 then panic(msg: "Size cannot be negative"),
        Size { bytes: result },
    )
}

// int * Size -> Size (commutative)
impl Mul<Size> for int {
    type Output = Size
    @multiply (self, s: Size) -> Size = s * self
}

// Size / int -> Size
impl Div<int> for Size {
    type Output = Size
    @divide (self, n: int) -> Size = Size { bytes: self.bytes / n }
}

// Size / Size -> int (ratio)
impl Div for Size {
    type Output = int
    @divide (self, other: Size) -> int = self.bytes / other.bytes
}

// Size % Size -> Size (remainder)
impl Rem for Size {
    @remainder (self, other: Size) -> Size =
        Size { bytes: self.bytes % other.bytes }
}

// Note: Neg is NOT implemented for Size — unary negation is a compile error

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

    // Extraction methods (truncate toward zero)
    pub @bytes (self) -> int = self.bytes
    pub @kilobytes (self) -> int = self.bytes / 1024
    pub @megabytes (self) -> int = self.bytes / (1024 * 1024)
    pub @gigabytes (self) -> int = self.bytes / (1024 * 1024 * 1024)
    pub @terabytes (self) -> int = self.bytes / (1024 * 1024 * 1024 * 1024)
}

impl Printable for Size {
    @to_str (self) -> str = run(
        let b = self.bytes,
        if b % 1_099_511_627_776 == 0 then `{b / 1_099_511_627_776}tb`
        else if b % 1_073_741_824 == 0 then `{b / 1_073_741_824}gb`
        else if b % 1_048_576 == 0 then `{b / 1_048_576}mb`
        else if b % 1024 == 0 then `{b / 1024}kb`
        else `{b}b`,
    )
}
```

## Language Features Required

All required features are now approved:

### 1. Operator Traits (APPROVED)

See `operator-traits-proposal.md`. Defines `Add`, `Sub`, `Mul`, `Div`, `Neg`, `Rem` traits that types implement to support operator syntax.

### 2. Associated Functions (APPROVED)

See `associated-functions-language-feature.md`. Enables `Type.method()` syntax for factory methods.

### 3. Derive for Operator Traits (NICE TO HAVE)

Allow `#derive(Add, Sub, Mul, Div)` for newtypes wrapping numeric types:

```ori
#derive(Add, Sub, Mul, Div)
type Celsius = { value: float }
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

### Remove Literal Suffixes

Require explicit `Duration.from_seconds(s: 10)` instead of `10s`.

**Rejected**: Too verbose, significantly reduces language ergonomics.

### User-Defined Literal Suffixes

Allow users to define custom suffixes via attribute.

**Deferred**: More complex, enables future extensibility. Defer to future proposal.

## References

- Current Duration implementation: `compiler/ori_eval/src/methods.rs`
- Current Size implementation: `compiler/ori_eval/src/methods.rs`
- Current operator handling: `compiler/ori_eval/src/operators.rs`
- Operator traits proposal: `proposals/approved/operator-traits-proposal.md`
- Associated functions proposal: `proposals/approved/associated-functions-language-feature.md`
- Rust's approach: `std::ops` module with operator traits
