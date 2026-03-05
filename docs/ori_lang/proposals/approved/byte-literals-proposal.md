# Proposal: Byte Literals

**Status:** Approved
**Author:** Eric (with Claude)
**Created:** 2026-03-05
**Approved:** 2026-03-05

---

## Summary

Add byte literal syntax `b'x'` for creating `byte` values directly, mirroring the existing `char` literal syntax `'x'`. Additionally, add `\xHH` hex escapes to both byte and char literals.

```ori
let space: byte = b' ';
let newline: byte = b'\n';
let null: byte = b'\0';
let esc: byte = b'\x1B';
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
byte_lit    = "b'" ( byte_char | byte_escape ) "'" .
byte_char   = ascii_char - "'" - "\" .  /* U+0020 to U+007E, excluding ' and \ */
byte_escape = "\\" | "\'" | "\n" | "\t" | "\r" | "\0" | hex_escape .
hex_escape  = "\x" hex_digit hex_digit .
hex_digit   = "0".."9" | "a".."f" | "A".."F" .
```

A byte literal is prefixed with `b` and contains a single ASCII character or escape sequence. The type is `byte`.

### Character Content

Unescaped characters in byte literals shall be printable ASCII (U+0020–U+007E), excluding the delimiter `'` and the escape introducer `\`. Non-ASCII characters are an error:

```ori
b'a'          // OK: 0x61
b'\n'         // OK: 0x0A
b'\x1B'       // OK: escape character (0x1B)
b'\xFF'       // OK: 255 (max byte value via hex escape)
b'\u{E9}'     // error: byte literal cannot contain unicode escape
```

**Rationale:** A `byte` is an unsigned 8-bit integer (0–255). Unicode code points can exceed 255, so `\u{...}` escapes are not meaningful for bytes. Use `\xHH` for arbitrary byte values.

### Hex Escape (`\xHH`)

`\xHH` specifies a byte value from 0x00 to 0xFF using exactly two hex digits:

```ori
b'\x00'    // null byte
b'\x1B'    // ESC
b'\x7F'    // DEL
b'\xFF'    // 255
b'\x0'     // error: \x requires exactly 2 hex digits
```

#### `\xHH` in char literals

This proposal also adds `\xHH` to char literals, restricted to the ASCII range (`\x00`–`\x7F`). Values `\x80`–`\xFF` in char literals are a compile-time error, since characters above U+007F have multi-byte UTF-8 encodings that `\xHH` cannot represent unambiguously.

```ori
'\x41'     // OK: 'A' (U+0041)
'\x0A'     // OK: '\n' (U+000A)
'\x7F'     // OK: DEL (U+007F)
'\x80'     // error: \x value exceeds ASCII range in char literal (use \u{80})
'\xFF'     // error: \x value exceeds ASCII range in char literal (use \u{FF})
```

This keeps the escape grammar consistent across literal contexts while preserving type safety.

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

This is a subset of the char escape sequences (no `\u{...}`), plus `\xHH` (full range).

The escape `\"` is not valid in byte literals. Double quotes do not need escaping since they are not the delimiter. This matches char literal behavior.

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

## Implementation

### Affected Compiler Layers

The two-layer lexer design means byte literal support touches both layers independently:

1. **Raw scanner** (`ori_lexer_core`): Modify identifier dispatch to check for `b'` lookahead. Add `RawTag::ByteLiteral` and `RawTag::UnterminatedByteLiteral` variants. Reuse `skip_escape_body()` with extension for `\x` followed by exactly 2 hex digits.

2. **Cooker** (`ori_lexer`): Add `cook_byte_literal()` method in `escape_cooking.rs`. New `unescape_byte_v2()` function that rejects `\u{...}`, adds `\xHH`, and produces `u8`.

3. **Token kind** (`ori_ir`): Add `TokenKind::Byte(u8)` variant.

4. **Parser** (`ori_parse`): Parse `TokenKind::Byte(u8)` as a literal expression.

5. **Type checker** (`ori_types`): Infer `byte` type for byte literal expressions.

6. **Evaluator** (`ori_eval`): Evaluate byte literals to `Value::Byte(u8)`.

7. **LLVM codegen** (`ori_llvm`): Emit byte literal as `i8` constant.

### Char literal `\xHH` support

Extend `unescape_char_v2()` in `cook_escape/mod.rs` to handle `\x` escapes. Parse two hex digits, validate the value is ≤ 0x7F, and convert to `char`. Values > 0x7F produce a compile-time error.

Extend `skip_escape_body()` in the raw scanner to recognize `\x` and consume exactly 2 hex digits (boundary detection only — validation in cooker).

---

## Migration / Compatibility

- **No breaking changes.** `b'x'` is currently a syntax error (identifier `b` followed by char literal), so no existing code is affected.
- **Lexer change:** The lexer must recognize `b'` as the start of a byte literal, not as identifier `b` followed by `'`.
- **Existing numeric byte creation remains valid.** `let space: byte = 32;` continues to work — `b' '` is an additional way to create a byte, not a replacement.

---

## Dependencies and Related Proposals

### Prerequisites for

- **Range patterns on char and byte** (`range-patterns-char-byte-proposal.md`) — uses `b'a'..b'z'` syntax
- **Character and byte classification methods** (`char-byte-classification-proposal.md`) — uses byte literals in examples and implementations
- **Byte-level string access** (`byte-string-access-proposal.md`) — uses byte literals in examples

### Open Questions

1. **Byte string literals?** Should `b"hello"` produce `[byte]`? This is a natural extension but is deferred. See `byte-string-access-proposal.md` for the `as_bytes()`/`to_bytes()` approach.

---

## References

- [Spec 7.7.5 — Character Literals](../../v2026/spec/07-lexical-elements.md)
- [Spec 8.1 — Primitive Types](../../v2026/spec/08-types.md)
- [Spec 8.8 — Type Conversions](../../v2026/spec/08-types.md)
- [Rust Byte Literals](https://doc.rust-lang.org/reference/tokens.html#byte-literals)

---

## Changelog

- 2026-03-05: Initial draft
- 2026-03-05: Approved — resolved `\xHH` in char literals (yes, \x00-\x7F), excluded `\"`, fixed grammar precision, added implementation scope, added cross-references
