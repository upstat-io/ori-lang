---
title: "String SSO"
description: "Ori Compiler Design — Small String Optimization"
order: 1103
section: "Runtime"
sidebar_title: "String SSO"
sidebar_order: 3
sidebar_path: "/docs/compiler-design/11-runtime/string-sso"
---

# String SSO

The Ori runtime represents strings with a 24-byte tagged union that uses Small
String Optimization (SSO). Strings of 23 bytes or fewer are stored entirely
inline, avoiding all heap allocation and reference counting overhead.

## OriStr Layout

`OriStr` occupies exactly 24 bytes (3 machine words on 64-bit platforms). The
layout is a `#[repr(C)]` union of two variants, discriminated by the high bit
of byte 23:

```
OriStr (24 bytes total):

SSO mode (byte 23 high bit set -- 0x80):
  +--------------------------------------------+-------+
  |  inline bytes (up to 23 bytes)             | flags |
  |  [0..22]                                   | [23]  |
  +--------------------------------------------+-------+
  flags = SSO_FLAG (0x80) | length (low 7 bits, 0..=23)

Heap mode (byte 23 high bit clear):
  +----------+----------+----------+
  | len: i64 | cap: i64 | data: *  |
  | [0..7]   | [8..15]  | [16..23] |
  +----------+----------+----------+
  data points to RC-managed buffer (via ori_rc_alloc)
```

The Rust implementation uses a `union` of `OriStrHeap { len, cap, data }` and
`OriStrSSO { bytes: [u8; 23], flags: u8 }`.

## Discriminator

The discriminator is the high bit (bit 7) of byte 23:

- **Set (0x80)**: SSO mode. The low 7 bits of byte 23 store the string length
  (0 to 23). `flags = SSO_FLAG | len`.
- **Clear**: Heap mode. Byte 23 is the MSB of the `data` pointer, which on
  current 64-bit platforms always has bit 63 clear (user-space addresses use
  canonical addressing with at most 48 or 57 significant bits).

This single-bit discriminator enables an O(1) mode check:

```rust
fn is_sso(&self) -> bool {
    self.sso.flags & SSO_FLAG != 0  // SSO_FLAG = 0x80
}
```

The `EMPTY` constant is an SSO string with zero length: all bytes zero except
byte 23 which is `0x80`.

## SSO Mode Details

In SSO mode, the 24-byte struct is used directly as a byte buffer:

- Bytes 0 through `len - 1` contain the string data (valid UTF-8)
- Bytes `len` through 22 are unused (may contain garbage from prior values)
- Byte 23 contains `0x80 | len`

SSO strings have **no heap allocation, no RC header, and no refcount operations**.
Copying an SSO string is a 24-byte `memcpy`. Dropping is a no-op.

### SSO Threshold: 23 Bytes

This covers:
- All ASCII strings up to 23 characters
- Many common UTF-8 strings (most Western European text fits in 1-2 bytes per
  codepoint)
- Common identifier names, error codes, format strings, and short messages

The threshold fills the full 24-byte struct minus the 1-byte flags field.

## Heap Mode Details

In heap mode, the 24 bytes are interpreted as three 64-bit fields:

| Field  | Offset | Description                                     |
|--------|--------|-------------------------------------------------|
| `len`  | 0      | Number of valid bytes in the buffer              |
| `cap`  | 8      | Total capacity of the buffer (or slice encoding) |
| `data` | 16     | Pointer to RC-managed buffer via `ori_rc_alloc`  |

The `data` pointer points to the user data region of an RC allocation (past the
16-byte RC header). The buffer is managed by the standard RC protocol: `inc` on
copy, `dec` on drop, `is_unique` for COW.

Heap strings also support **seamless slices** using the same negative-capacity
encoding as lists: when `cap < 0`, `data` points into another string's buffer,
and the lower 63 bits of `cap` encode the byte offset from the original
allocation's data start.

## Factory Functions

### `from_bytes(bytes: &[u8]) -> OriStr`

The primary factory. Automatically selects SSO or heap mode based on length:
- `len <= 23`: copies bytes inline, sets SSO flags byte
- `len > 23`: allocates heap buffer via `ori_rc_alloc`, copies bytes

### `from_sso(bytes: &[u8]) -> OriStr`

Creates an SSO string directly. Debug-asserts that `bytes.len() <= 23`.

### `from_heap(bytes: &[u8]) -> OriStr`

Creates a heap string. Allocates via `ori_rc_alloc` with capacity equal to
length. Returns `EMPTY` for empty input.

### `with_capacity(cap: usize) -> OriStr`

Pre-allocates a heap buffer of `cap` bytes with length 0. Returns `EMPTY` for
zero capacity. Used for building strings incrementally (concat chains).

### `ori_str_from_raw(src: *const u8, len: i64) -> OriStr`

C-ABI entry point. Creates a string from a raw pointer and length. Used by
LLVM codegen for string literals and runtime string construction.

### `ori_str_from_int` / `ori_str_from_float` / `ori_str_from_bool`

Type conversion functions. `from_bool` always produces SSO ("true" is 4 bytes,
"false" is 5 bytes). `from_int` and `from_float` delegate to Rust's
`to_string()` and wrap the result via `from_owned`.

## Promotion: SSO to Heap

When an SSO string needs to grow beyond 23 bytes, it is promoted to heap mode
via `promote_to_heap(min_cap)`:

1. Computes capacity via `next_capacity(0, min_cap)` (at least 4, at least
   `min_cap`, doubling from 0)
2. Allocates via `ori_rc_alloc(capacity, 1)`
3. Copies the inline bytes to the new buffer
4. Rewrites the struct fields to heap mode: `{len, cap, data}`

There is **no demotion** (heap back to SSO). The common case after promotion
is continued growth, and checking the length on every mutation would add
overhead to the fast path.

## Capacity Management: `ensure_capacity`

Ensures a heap string has at least `required` bytes of capacity. This is only
called on uniquely-owned heap strings (precondition):

1. If `cap >= required`: no-op
2. If `cap < required`: realloc via `ori_rc_realloc` with
   `next_capacity(old_cap, required)` for amortized growth

The C-ABI entry point `ori_str_ensure_capacity` is a no-op for SSO strings
(promotion is handled by the caller).

## SSO-Aware Operations

### Concatenation (`ori_str_concat`)

Four cases, from fastest to slowest:

1. **Both SSO, result <= 23 bytes**: Copy `a` bytes then `b` bytes into an
   inline buffer. Write SSO result. **Zero allocation.**
2. **`a` is heap, unique, has capacity**: Append `b` bytes in place at
   `data + a_len`. Increment RC on `a`'s data (caller will dec the old ref).
   O(m) where m = len(b).
3. **`a` is heap, unique, needs growth**: Cannot use realloc (would invalidate
   caller's old pointer -- see source comment). Falls through to case 4.
4. **Allocate new**: Uses `next_capacity(a_cap, combined)` for amortized
   doubling. Copies both strings into the new buffer.

### Push Char (`ori_str_push_char`)

Same four-case COW protocol as concat. Encodes the char to UTF-8 (1-4 bytes),
then follows the same SSO/heap/unique/shared decision tree. Case 3 (unique,
no capacity) uses `ori_rc_realloc` directly (safe because `push_char` owns
the string by value, unlike concat which borrows).

### Substring (`ori_str_substring`)

- **SSO source**: Copies the byte range into a new SSO or heap string.
- **Heap source, result <= 23 bytes**: Copies bytes into SSO (cheaper than
  RC management for small results).
- **Heap source, result > 23 bytes**: Creates a **seamless slice** sharing
  the original buffer's RC. Increments RC on the original allocation. Supports
  slice-of-slice by accumulating byte offsets.

### Split (`ori_str_split`)

Returns a list of `OriStr` values. Uses a hybrid strategy:

- If the source string is heap (`str_len > SSO_MAX_LEN`), pieces longer than
  23 bytes are returned as seamless slices (zero-copy, sharing the original
  buffer's RC via `ori_rc_inc`).
- Pieces of 23 bytes or fewer use SSO (no heap allocation, no RC).
- If the source is SSO, all pieces fit in SSO anyway.

### Trim (`ori_str_trim`)

Finds the whitespace boundaries, then delegates to `ori_str_substring`, which
produces a seamless slice for heap strings or an SSO copy for inline strings.

### Case Conversion (`ori_str_to_uppercase` / `ori_str_to_lowercase`)

Three-tier COW optimization:

1. **Non-ASCII content**: Falls through to Rust's `to_uppercase()` /
   `to_lowercase()` (may change byte length, e.g., "ss" -> "SS").
2. **ASCII + SSO**: Transforms bytes in place on a copy of the SSO struct.
3. **ASCII + heap + unique**: Transforms bytes in place in the buffer
   (ASCII case change preserves byte length). Returns the same struct.
4. **ASCII + heap + shared**: Allocates new buffer with transformed bytes.

### Replace (`ori_str_replace`)

COW optimization for same-length replacement on unique heap strings: scans the
buffer and overwrites matches in place. General case delegates to Rust's
`replace()` and wraps the result.

### Repeat (`ori_str_repeat`)

Always allocates a new buffer with exact capacity. If the result fits in SSO,
fills the inline bytes directly. Otherwise allocates via `ori_rc_alloc` with
`n * len` bytes.

## Length and Data Access

`ori_str_len` and `ori_str_data` are SSO-safe C-ABI entry points:

- **`ori_str_len`**: Returns `flags & 0x7F` for SSO, `heap.len` for heap.
- **`ori_str_data`**: Returns a pointer to the inline bytes (the struct itself)
  for SSO, or `heap.data` for heap strings.

**Lifetime warning**: For SSO strings, the data pointer points into the
`OriStr` struct itself. If the struct is on the stack, the pointer is only
valid while that stack frame is live. Codegen must not store SSO data pointers
in long-lived structures.

## Performance Characteristics

| Operation              | SSO                | Heap                        |
|------------------------|--------------------|-----------------------------|
| Create (small)         | memcpy, no alloc   | N/A                         |
| Create (large)         | N/A                | alloc + memcpy              |
| Length                  | mask byte 23       | load field                  |
| Data access            | pointer to self    | pointer to buffer           |
| Copy                   | 24-byte memcpy     | 24-byte memcpy + RC inc     |
| Drop                   | no-op              | RC dec (possibly free)      |
| Concat (result small)  | memcpy, no alloc   | N/A                         |
| Concat (result large)  | N/A                | alloc or in-place + memcpy  |
| Substring (small)      | memcpy (SSO copy)  | memcpy (SSO copy)           |
| Substring (large)      | N/A                | seamless slice (RC inc only)|
| Case conversion (ASCII)| in-place on copy   | in-place if unique          |

The key insight is that SSO strings have the same copy cost as a primitive value
(24-byte memcpy) and zero drop cost. This makes small strings as cheap as
integers in terms of memory management overhead.
