# Proposal: Byte-Level String Access

**Status:** Approved
**Author:** Eric (with Claude)
**Created:** 2026-03-05
**Approved:** 2026-03-05

---

## Summary

Add methods for accessing the raw UTF-8 bytes of a `str` value, and provide efficient byte-buffer types for byte-oriented processing like lexers and parsers.

```ori
let bytes: [byte] = "hello".to_bytes();
let view: [byte] = "hello".as_bytes();   // zero-copy view (seamless slice)

let ch: byte = bytes[0];                  // b'h' — O(1) access
```

---

## Motivation

### The Problem

Ori's `str` type is UTF-8 encoded, and `str[i]` returns a single-codepoint `str` (not a `byte`). This is correct for user-facing string manipulation, but wrong for byte-oriented processing.

Lexers, parsers, protocol handlers, and binary format readers need:
- O(1) byte-level indexing
- Byte pattern matching
- Byte-by-byte iteration
- No UTF-8 decoding overhead

```ori
// Current: str[i] returns str, not byte
let source = "let x = 42";
let first = source[0];        // "l" (type: str, not byte)
// No way to get the raw byte value
```

### What We Want

```ori
let source = "let x = 42";
let buf = source.as_bytes();   // [byte] view — zero copy
let first = buf[0];            // b'l' (type: byte, O(1))

// Lexer can now work at byte level:
match buf[pos] {
    b'a'..b'z' | b'A'..b'Z' -> read_ident(),
    b'0'..b'9' -> read_number(),
    b' ' | b'\t' | b'\n' -> skip_whitespace(),
    _ -> error(),
}
```

### Prior Art

| Language | Method | Returns | Zero-Copy |
|----------|--------|---------|-----------|
| Rust | `s.as_bytes()` | `&[u8]` | Yes (borrow) |
| Go | `[]byte(s)` | `[]byte` | No (copy) |
| Swift | `s.utf8` | `String.UTF8View` | Yes (view) |
| Zig | `s.ptr[0..s.len]` | `[]const u8` | Yes (slice) |

---

## Design

### Methods on `str`

| Method | Returns | Copy? | Description |
|--------|---------|-------|-------------|
| `as_bytes()` | `[byte]` | No — seamless slice | Zero-copy view of UTF-8 bytes |
| `to_bytes()` | `[byte]` | Yes | Independent copy of UTF-8 bytes |
| `byte_len()` | `int` | N/A | Number of UTF-8 bytes (O(1)) |

```ori
let s = "hello";
s.byte_len()     // 5
s.as_bytes()     // [104, 101, 108, 108, 111] — zero-copy view
s.to_bytes()     // [104, 101, 108, 108, 111] — independent copy
```

### `as_bytes()` — Zero-Copy Semantics

`as_bytes()` returns a `[byte]` that shares the underlying allocation with the source `str` via seamless slicing (spec 21.4). No data is copied. The returned list is read-only in the sense that COW semantics apply — modifying the `[byte]` triggers a copy, leaving the original `str` unaffected.

```ori
let s = "hello";
let bytes = s.as_bytes();     // shares allocation (seamless slice)
let b = bytes[0];             // b'h' — O(1), no copy
bytes[0] = b'H';              // COW: bytes gets its own copy, s unaffected
```

#### Flattening for Substrings

If the source `str` is itself a seamless slice (e.g., from `.substring()` or `.trim()`), `as_bytes()` produces a single-level `[byte]` view of the same byte range. No nested slices are created — the implementation takes the substring's pointer and length and creates a byte slice directly from those.

```ori
let s = "hello world";
let sub = s.substring(start: 0, end: 5);  // "hello" — seamless slice of s
let bytes = sub.as_bytes();                // [byte] view of bytes 0..5 — single level
```

### `to_bytes()` — Owned Copy

`to_bytes()` returns an independent `[byte]` copy. Use when you need to mutate the bytes without affecting the source string.

### `byte_len()` vs `len()`

| Method | Returns |
|--------|---------|
| `s.len()` | Number of Unicode code points (grapheme clusters or codepoints — TBD) |
| `s.byte_len()` | Number of UTF-8 bytes |

For ASCII strings, these are equal. For multibyte characters, they differ:

```ori
let s = "cafe\u{0301}";      // "cafe" — 6 bytes, 5 codepoints
s.byte_len()                  // 6
s.len()                       // depends on len() definition
```

### Constructing `str` from `[byte]`

```ori
@from_utf8 (bytes: [byte]) -> Result<str, Error>
@from_utf8_unchecked (bytes: [byte]) -> str   // unsafe — caller guarantees valid UTF-8
```

`from_utf8` validates UTF-8 encoding and returns an error on invalid sequences.

`from_utf8_unchecked` skips validation and requires `unsafe`. If called with invalid UTF-8, the behavior is **unspecified but memory-safe** — the program may panic, produce garbled string output, or behave unexpectedly, but it shall never cause memory corruption, buffer overflows, or use-after-free. This is consistent with Ori's safety philosophy: `unsafe` relaxes type-level guarantees but does not permit memory unsafety.

These are associated functions on `str`:

```ori
let bytes: [byte] = [104, 101, 108, 108, 111];
let s = str.from_utf8(bytes:);               // Ok("hello")

unsafe {
    let s = str.from_utf8_unchecked(bytes:);  // "hello"
}
```

### Iteration

```ori
// Iterate over bytes
for b in "hello".as_bytes().iter() do { ... }

// Iterate over chars (existing)
for c in "hello".chars() do { ... }
```

---

## Interaction with Seamless Slicing

`as_bytes()` leverages the existing seamless slicing mechanism (spec 21.4):

- The `[byte]` view shares the `str`'s heap allocation
- The `SLICE_FLAG` in the capacity field marks it as a view
- `ori_buffer_rc_dec` handles cleanup for both regular and slice-backed lists
- COW on the `[byte]` view triggers materialization (copy) — the original `str` is never affected

This is the same mechanism used by `list.take()`, `list.skip()`, `str.substring()`, and `str.trim()`.

---

## Migration / Compatibility

- **No breaking changes.** New methods on existing types.
- **`as_bytes()` is the preferred method** for read-only byte access. `to_bytes()` for when mutation is needed.

---

## Depends On

- `byte-literals-proposal.md` [approved] — uses byte literals in examples
- Seamless slicing (spec 21.4) [implemented] — runtime mechanism for zero-copy views

---

## References

- [Spec 8.1 — Primitive Types](../../v2026/spec/08-types.md)
- [Spec 21.4 — Reference Counting Optimizations](../../v2026/spec/21-memory-model.md)
- [Rust `str::as_bytes()`](https://doc.rust-lang.org/std/primitive.str.html#method.as_bytes)

---

## Changelog

- 2026-03-05: Initial draft
- 2026-03-05: Approved — specified flatten behavior for substring slices; clarified `from_utf8_unchecked` as unspecified-but-memory-safe (no true UB); resolved `byte_len` naming; resolved all open questions (byte string literals deferred)
