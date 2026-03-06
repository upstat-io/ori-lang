# Proposal: Character and Byte Classification Methods

**Status:** Approved
**Author:** Eric (with Claude)
**Created:** 2026-03-05
**Approved:** 2026-03-05

---

## Summary

Define standard classification methods on `char` and `byte` types for character category testing. These are essential for text processing, lexers, parsers, and input validation.

```ori
'A'.is_alphabetic()    // true
b'\t'.is_whitespace()  // true
'3'.is_digit()         // true
b'\xFF'.is_ascii()     // false
```

---

## Motivation

### The Problem

Ori has `char` and `byte` types but no spec-defined methods for character classification. Writing a lexer requires constant range checks:

```ori
// Current: manual range checks
if (ch >= b'a' && ch <= b'z') || (ch >= b'A' && ch <= b'Z') then ...
if ch >= b'0' && ch <= b'9' then ...
if ch == b' ' || ch == b'\t' || ch == b'\n' || ch == b'\r' then ...
```

This is error-prone (easy to miss a case), verbose, and lacks intent clarity.

### What We Want

```ori
if ch.is_alpha() then ...
if ch.is_digit() then ...
if ch.is_whitespace() then ...
```

Self-documenting, correct, and concise.

### Prior Art

Every systems language provides these:

| Language | Module/Type | Example Methods |
|----------|-------------|-----------------|
| Rust | `char`, `u8` | `is_alphabetic()`, `is_ascii_digit()`, `is_whitespace()` |
| Go | `unicode` | `IsLetter()`, `IsDigit()`, `IsSpace()` |
| Swift | `Character` | `isLetter`, `isNumber`, `isWhitespace` |
| Zig | `std.ascii` | `isAlphabetic()`, `isDigit()`, `isWhitespace()` |

### Archived Design

The Ori archived design doc (`03-type-system/01-primitive-types.md`) already lists `is_alphabetic()`, `is_numeric()`, `is_whitespace()`, `is_ascii()` on `char`. This proposal formalizes and extends that design.

---

## Design

### Char Methods (Unicode)

Methods on `char` operate on full Unicode:

| Method | Description | Unicode Category |
|--------|-------------|-----------------|
| `is_alphabetic()` | Letter | `L*` (Letter) |
| `is_digit()` | Decimal digit | `Nd` (Decimal Number) — includes digits from all scripts |
| `is_alphanumeric()` | Letter or digit | `L*` or `Nd` |
| `is_whitespace()` | Whitespace | `Zs`, `\t`, `\n`, `\r`, `\u{000B}`, `\u{000C}`, `\u{0085}`, `\u{2028}`, `\u{2029}` |
| `is_uppercase()` | Uppercase letter | `Lu` |
| `is_lowercase()` | Lowercase letter | `Ll` |
| `is_ascii()` | In ASCII range | U+0000..U+007F |
| `is_control()` | Control character | `Cc` |

All return `bool`. All are pure (no self-mutation).

### Char Methods (ASCII)

ASCII-scoped methods on `char` for when Unicode classification is not desired:

| Method | Description | Range |
|--------|-------------|-------|
| `is_ascii_alphabetic()` | ASCII letter | `a-z`, `A-Z` |
| `is_ascii_digit()` | ASCII digit | `0-9` |
| `is_ascii_alphanumeric()` | ASCII letter or digit | `a-z`, `A-Z`, `0-9` |
| `is_ascii_whitespace()` | ASCII whitespace | `' '`, `\t`, `\n`, `\r`, `\x0B`, `\x0C` |
| `is_ascii_uppercase()` | ASCII uppercase | `A-Z` |
| `is_ascii_lowercase()` | ASCII lowercase | `a-z` |
| `is_ascii_hex_digit()` | Hex digit | `0-9`, `a-f`, `A-F` |
| `is_ascii_punctuation()` | ASCII punctuation | `!-/`, `:-@`, `[-`\``, `{-~` |
| `is_ascii_control()` | ASCII control | 0x00..0x1F, 0x7F |

These return `false` for any `char` outside the ASCII range (U+0080 and above). This mirrors Rust's `char::is_ascii_*()` family.

**Rationale:** Without ASCII-scoped methods on `char`, checking if a character is an ASCII digit requires `ch.is_ascii() && (ch as byte).is_digit()` which is awkward. Lexers and parsers frequently work with `char` values but only care about ASCII.

### Byte Methods

Methods on `byte` operate on ASCII only (0-127). Bytes outside ASCII range return `false` for all classification methods except where noted:

| Method | Description | Range |
|--------|-------------|-------|
| `is_ascii()` | In ASCII range | 0x00..0x7F |
| `is_ascii_alpha()` | ASCII letter | `a-z`, `A-Z` |
| `is_ascii_digit()` | ASCII digit | `0-9` |
| `is_ascii_alphanumeric()` | ASCII letter or digit | `a-z`, `A-Z`, `0-9` |
| `is_ascii_whitespace()` | ASCII whitespace | `' '`, `\t`, `\n`, `\r`, `\x0B`, `\x0C` |
| `is_ascii_uppercase()` | ASCII uppercase | `A-Z` |
| `is_ascii_lowercase()` | ASCII lowercase | `a-z` |
| `is_ascii_punctuation()` | ASCII punctuation | `!-/`, `:-@`, `[-`\``, `{-~` |
| `is_ascii_control()` | ASCII control | 0x00..0x1F, 0x7F |
| `is_ascii_hex_digit()` | Hex digit | `0-9`, `a-f`, `A-F` |

### Short Aliases on Byte

For conciseness in byte-heavy code (lexers), short aliases are provided:

| Short | Full | Reason |
|-------|------|--------|
| `is_alpha()` | `is_ascii_alpha()` | Bytes are inherently ASCII-scoped |
| `is_digit()` | `is_ascii_digit()` | Unambiguous for bytes |
| `is_alnum()` | `is_ascii_alphanumeric()` | Common abbreviation |
| `is_whitespace()` | `is_ascii_whitespace()` | Most common use |
| `is_upper()` | `is_ascii_uppercase()` | Concise |
| `is_lower()` | `is_ascii_lowercase()` | Concise |
| `is_hex_digit()` | `is_ascii_hex_digit()` | Common in parsers |

**Rationale:** `byte` is an 8-bit unsigned integer. It has no Unicode semantics. The `is_ascii_*` prefix is redundant for bytes — all byte classification is inherently ASCII. The short aliases remove this noise.

**No short aliases on `char`.** `char` methods use full names (`is_alphabetic()`, not `is_alpha()`) to reinforce that they operate on full Unicode. The naming distinction between `char.is_alphabetic()` (Unicode) and `byte.is_alpha()` (ASCII) signals the semantic difference.

### Conversion Methods

| Method | On | Returns | Description |
|--------|----|---------|-------------|
| `to_ascii_uppercase()` | `char`, `byte` | `char`, `byte` | Uppercase if ASCII letter, else self |
| `to_ascii_lowercase()` | `char`, `byte` | `char`, `byte` | Lowercase if ASCII letter, else self |
| `to_digit(radix: int)` | `char`, `byte` | `Option<int>` | Convert digit char to numeric value |

```ori
b'a'.to_ascii_uppercase()    // b'A'
'3'.to_digit(radix: 10)      // Some(3)
'f'.to_digit(radix: 16)      // Some(15)
'z'.to_digit(radix: 10)      // None
```

#### `to_digit` Radix Rules

The `radix` parameter shall be in the range 2..=36 (inclusive). Values outside this range panic:

```ori
'a'.to_digit(radix: 16)      // Some(10)
'a'.to_digit(radix: 10)      // None ('a' is not a base-10 digit)
'a'.to_digit(radix: 0)       // panic: radix must be in range 2..=36
'a'.to_digit(radix: 37)      // panic: radix must be in range 2..=36
```

For radices > 10, letters `a-z` / `A-Z` represent digit values 10-35 (case-insensitive).

#### Full Unicode Case Conversion

Full Unicode case conversion (`to_uppercase()`, `to_lowercase()`) is deferred. These are complex (locale-sensitive, one-to-many mappings like `ß` -> `SS`) and will be addressed in a future proposal if needed. The ASCII conversion methods cover the common case.

---

## Implementation

### Char Methods

Unicode-aware methods use lookup tables generated from the Unicode Character Database (UCD). The compiler or stdlib includes compressed tables for:
- `L*` (Letter) categories
- `Nd` (Decimal Number) category
- `Zs` (Space Separator) category
- `Lu`, `Ll` (Upper/Lowercase Letter)

ASCII-scoped methods on `char` compile to simple range checks (same as byte methods, with an additional `ch <= '\x7F'` guard).

### Byte Methods

All byte methods compile to simple range checks — no tables needed:

```ori
// is_ascii_alpha() — conceptual implementation
@is_ascii_alpha (self) -> bool =
    (self >= b'a' && self <= b'z') || (self >= b'A' && self <= b'Z');
```

These are trivially inlineable.

---

## Where These Methods Live

These are inherent methods on primitive types, defined in the prelude:

```ori
impl char {
    @is_alphabetic (self) -> bool = ...;
    @is_digit (self) -> bool = ...;
    @is_ascii_alphabetic (self) -> bool = ...;
    @is_ascii_digit (self) -> bool = ...;
    // ...
}

impl byte {
    @is_ascii (self) -> bool = ...;
    @is_ascii_alpha (self) -> bool = ...;
    @is_alpha (self) -> bool = ...;       // alias for is_ascii_alpha
    // ...
}
```

---

## Migration / Compatibility

- **No breaking changes.** These are new methods on existing types.
- **Prelude inclusion:** All methods are available without import.

---

## Depends On

- `byte-literals-proposal.md` [approved] — uses byte literals in examples and implementations

---

## References

- [Spec 8.1 — Primitive Types](../../v2026/spec/08-types.md)
- [Archived Design — Primitive Types](../../v2026/archived-design/03-type-system/01-primitive-types.md)
- [Rust char methods](https://doc.rust-lang.org/std/primitive.char.html)
- [Unicode General Categories](https://www.unicode.org/reports/tr44/#General_Category_Values)

---

## Changelog

- 2026-03-05: Initial draft
- 2026-03-05: Approved — added ASCII-scoped methods on `char`; resolved `to_digit` radix rules (panic on invalid, 2..=36); no short aliases on `char`; deferred full Unicode case conversion; resolved all open questions
