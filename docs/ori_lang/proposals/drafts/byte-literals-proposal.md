# Proposal: Byte Literals

**Status:** Draft
**Author:** Eric (with Claude)
**Created:** 2026-03-05

---

## Summary

Add byte literal syntax `b'x'` for creating `byte` values directly, mirroring the existing `char` literal syntax `'x'`.

```ori
let space: byte = b' ';
let newline: byte = b'\n';
let null: byte = b'\0';
```

---

## Motivation

### The Problem

Ori has a `byte` type (unsigned 0–255) but no literal syntax for it. To create a byte value, you must use numeric literals or conversions:

```ori
let space: byte = 32;          // magic number — what character is 32?
let space: byte = ' ' as byte; // verbose conversion
```

This is unacceptable for byte-processing code like lexers, parsers, and binary protocol handlers where byte patterns appear constantly:

```ori
// Current: magic numbers or verbose conversions
match self.buf[self.pos] {
    32 -> ...,                    // what is 32?
    9 -> ...,                     // what is 9?
    10 -> ...,                    // what is 10?
    _ -> ...,
}

// Or:
match self.buf[self.pos] {
    (' ' as byte) -> ...,        // verbose
    ('\t' as byte) -> ...,       // verbose
    ('\n' as byte) -> ...,       // verbose
    _ -> ...,
}
```

### What We Want

```ori
match self.buf[self.pos] {
    b' ' | b'\t' -> self.pos += 1,
    b'\n' -> self.handle_newline(),
    b'a'..b'z' | b'A'..b'Z' -> self.read_identifier(),
    b'0'..b'9' -> self.read_number(),
    _ -> self.error("unexpected byte"),
}
```

Clear, readable, and self-documenting.

### Prior Art

| Language | Byte Literal | Type |
|----------|-------------|------|
| Rust | `b'x'` | `u8` |
| Go | No (uses `byte('x')`) | `byte` (alias for `uint8`) |
| Zig | `'x'` (chars are `u8`) | `u8` |
| Python | `b"string"[0]` | `int` |
| C | `'x'` (chars are ints) | `int` |

Rust's `b'x'` is the clearest prior art and the syntax most developers expect.

---

## Design

### Syntax

```ebnf
byte_lit = "b'" ( byte_char | byte_escape ) "'" .
byte_char = ascii_char .  /* U+0020 to U+007E, excluding ' and \ */
byte_escape = "\\" | "\'" | "\n" | "\t" | "\r" | "\0" | "\x" hex hex .
hex = "0".."9" | "a".."f" | "A".."F" .
```

A byte literal is prefixed with `b` and contains a single ASCII character or escape sequence. The type is `byte`.

### ASCII Only

Byte literals accept only ASCII characters (0x00–0x7F). Non-ASCII characters are an error:

```ori
b'a'          // OK: 0x61
b'\n'         // OK: 0x0A
b'\x1B'       // OK: escape character (0x1B)
b'\xFF'       // OK: 255 (max byte value)
b'\u{E9}'     // error: byte literal cannot contain unicode escape
```

**Rationale:** A `byte` is an unsigned 8-bit integer (0–255). Unicode code points can exceed 255, so `\u{...}` escapes are not meaningful for bytes. Use `\x` for arbitrary byte values.

### Hex Escape

`\xHH` specifies a byte value from 0x00 to 0xFF using exactly two hex digits:

```ori
b'\x00'    // null byte
b'\x1B'    // ESC
b'\x7F'    // DEL
b'\xFF'    // 255
b'\x0'     // error: \x requires exactly 2 hex digits
```

### Escape Sequences

| Escape | Value | Name |
|--------|-------|------|
| `\\` | 0x5C | Backslash |
| `\'` | 0x27 | Single quote |
| `\n` | 0x0A | Newline |
| `\t` | 0x09 | Tab |
| `\r` | 0x0D | Carriage return |
| `\0` | 0x00 | Null |
| `\xHH` | 0x00–0xFF | Hex byte value |

This is a subset of the char escape sequences (no `\u{...}`), plus `\xHH`.

### Type

The type of a byte literal is `byte`. No inference ambiguity — `b'x'` is always `byte`.

### Relationship to Char Literals

| Literal | Type | Range |
|---------|------|-------|
| `'x'` | `char` | Unicode scalar values (U+0000–U+10FFFF) |
| `b'x'` | `byte` | 0–255 |

`char` and `byte` remain distinct types. Conversion requires explicit `as`:

```ori
let c: char = 'A';
let b: byte = c as byte;     // OK if c is ASCII, panics if > 127
let b: byte = b'A';          // direct — no conversion needed
```

---

## Migration / Compatibility

- **No breaking changes.** `b'x'` is currently a syntax error (identifier `b` followed by char literal), so no existing code is affected.
- **Lexer change:** The lexer must recognize `b'` as the start of a byte literal, not as identifier `b` followed by `'`.

---

## Open Questions

1. **Byte string literals?** Should `b"hello"` produce `[byte]`? This is a natural extension but could be deferred.
2. **`\x` in char literals?** Should `\xHH` also be valid in char literals (for ASCII range)? Currently only `\u{...}` is supported for char.

---

## References

- [Spec 7.7.5 — Character Literals](../../v2026/spec/07-lexical-elements.md)
- [Spec 8.1 — Primitive Types](../../v2026/spec/08-types.md)
- [Rust Byte Literals](https://doc.rust-lang.org/reference/tokens.html#byte-literals)

---

## Changelog

- 2026-03-05: Initial draft
