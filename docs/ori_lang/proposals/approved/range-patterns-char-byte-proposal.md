# Proposal: Range Patterns on char and byte

**Status:** Approved
**Author:** Eric (with Claude)
**Created:** 2026-03-05
**Approved:** 2026-03-05

---

## Summary

Extend range patterns in `match` to work on `char` and `byte` types, enabling concise character classification in pattern matching.

```ori
match ch {
    'a'..'z' | 'A'..'Z' -> TokenKind.Ident,
    '0'..'9' -> TokenKind.Number,
    ' ' | '\t' | '\n' -> TokenKind.Whitespace,
    _ -> TokenKind.Symbol,
}
```

---

## Motivation

### The Problem

The spec mentions range patterns in the pattern list (spec 15) and demonstrates `1..10` for integer ranges. But it does not define whether range patterns work on `char` or `byte`. This is a critical gap for any text-processing code.

Without char/byte range patterns, classification requires verbose alternatives:

```ori
// Current: explicit enumeration or guards
match ch {
    ch if ch >= 'a' && ch <= 'z' -> ...,
    ch if ch >= 'A' && ch <= 'Z' -> ...,
    ch if ch >= '0' && ch <= '9' -> ...,
    _ -> ...,
}

// Or classification methods (if they existed):
match true {
    _ if ch.is_alpha() -> ...,
    _ if ch.is_digit() -> ...,
    _ -> ...,
}
```

Both defeat the purpose of pattern matching — the pattern should express the classification directly.

### What We Want

```ori
match byte_val {
    b'a'..b'z' | b'A'..b'Z' | b'_' -> self.read_ident(),
    b'0'..b'9' -> self.read_number(),
    b'"' -> self.read_string(),
    b' ' | b'\t' | b'\n' | b'\r' -> self.skip_whitespace(),
    _ -> self.error("unexpected"),
}
```

This is the natural pattern-matching style for lexers.

### Prior Art

| Language | Char Range Pattern | Syntax |
|----------|-------------------|--------|
| Rust | Yes | `'a'..='z'` |
| Swift | Yes | `"a"..."z"` (in switch) |
| Zig | No (uses `if` chains) | -- |
| Go | No (no pattern matching) | -- |
| Gleam | No (no range patterns) | -- |
| Haskell | Yes | `['a'..'z']` (list syntax, not pattern) |

---

## Design

### Range Pattern Syntax

Range patterns use the same syntax as range expressions:

```
range_pattern = const_pattern ".." const_pattern .       /* exclusive end */
range_pattern = const_pattern "..=" const_pattern .      /* inclusive end */
```

For char and byte:

```ori
match ch {
    'a'..'z' -> ...,     // exclusive: 'a' through 'y'
    'a'..='z' -> ...,    // inclusive: 'a' through 'z'
}
```

### Const Pattern Endpoints

Range pattern endpoints shall be compile-time constants: either literals or `$`-prefixed constant identifiers:

```ori
let $MAX_ASCII = '\x7F';
let $MIN_DIGIT = '0';

match ch {
    $MIN_DIGIT..='9' -> "digit",
    '\x00'..$MAX_ASCII -> "other ascii",
    _ -> "non-ascii",
}
```

### Supported Types

Range patterns are valid for types that implement `Comparable` with a discrete domain:

| Type | Example | Ordering |
|------|---------|----------|
| `int` | `0..100` | Numeric |
| `char` | `'a'..'z'` | Unicode code point |
| `byte` | `b'0'..b'9'` | Numeric (0-255) |

`float` is excluded from range patterns due to NaN semantics and precision issues. Use guards for float ranges: `x if x >= 0.0 && x < 1.0 -> ...`.

### Semantics

A range pattern `lo..hi` matches value `v` when `v >= lo && v < hi`.
A range pattern `lo..=hi` matches value `v` when `v >= lo && v <= hi`.

Both endpoints must be compile-time constants (literals or `$` constants).

### Exhaustiveness

Range patterns participate in exhaustiveness checking:

```ori
match b: byte {
    0..128 -> "ascii",
    128..=255 -> "non-ascii",
}
// Exhaustive: covers 0..=255
```

```ori
match ch: char {
    'a'..='z' -> "lower",
    _ -> "other",           // required: char range is not exhaustive
}
```

### Combining with Or-Patterns

Range patterns compose with `|`:

```ori
match ch {
    'a'..='z' | 'A'..='Z' | '_' -> "ident_start",
    '0'..='9' -> "digit",
    _ -> "other",
}
```

### Combining with At-Patterns

Range patterns compose with `@`:

```ori
match ch {
    c @ 'a'..='z' -> handle_lower(c),
    c @ '0'..='9' -> handle_digit(c),
    _ -> handle_other(),
}
```

---

## Grammar Changes

```ebnf
// Update pattern production to include range patterns:
pattern = literal_pattern
        | identifier_pattern
        | wildcard_pattern
        | variant_pattern
        | struct_pattern
        | list_pattern
        | or_pattern
        | at_pattern
        | range_pattern .

range_pattern = const_pattern ".." const_pattern
              | const_pattern "..=" const_pattern .

const_pattern = literal | "$" identifier .
```

The `const_pattern` production allows both literals and compile-time constant identifiers as range endpoints.

---

## Type Checking

### Both endpoints must have the same type

```ori
match x {
    'a'..10 -> ...,   // error: cannot mix char and int in range pattern
}
```

### Endpoints must be comparable

The type must implement `Comparable`. All supported types (`int`, `char`, `byte`) do.

### Empty ranges are warnings

```ori
match x {
    10..5 -> ...,     // warning: empty range pattern (lo > hi)
    'z'..'a' -> ...,  // warning: empty range pattern
}
```

### Overlap detection

The compiler shall warn about overlapping range patterns:

```ori
match x {
    0..10 -> ...,
    5..15 -> ...,     // warning: overlapping range patterns (5..10)
    _ -> ...,
}
```

---

## Relationship to Existing Roadmap

Range patterns for `int` are already tracked in `plans/roadmap/section-09-match.md`. This proposal extends that work to `char` and `byte` types, and adds the `const_pattern` endpoint grammar. The existing `'a'..='z'` char range pattern support (parsing and evaluation, done 2026-02-14) is formalized by this proposal.

---

## Migration / Compatibility

- **No breaking changes.** Range patterns are a new pattern form.
- **Extends existing range syntax.** Uses `..` and `..=` which already exist as expression operators.
- **Backward compatible:** Existing `int` range patterns (if implemented) are unchanged.

---

## Depends On

- `byte-literals-proposal.md` [approved] — uses `b'x'` syntax in byte range patterns
- `pattern-matching-exhaustiveness-proposal.md` [approved] — exhaustiveness algorithm extended

---

## References

- [Spec 15 — Patterns](../../v2026/spec/15-patterns.md)
- [Spec 14 — Expressions (Range)](../../v2026/spec/14-expressions.md)
- [Rust Range Patterns](https://doc.rust-lang.org/reference/patterns.html#range-patterns)
- [Swift Pattern Matching](https://docs.swift.org/swift-book/documentation/the-swift-programming-language/patterns/)

---

## Changelog

- 2026-03-05: Initial draft
- 2026-03-05: Approved — removed float from supported types; added const_pattern endpoints ($identifier); referenced existing roadmap task; resolved all open questions (half-open ranges deferred, char ordering is code point)
