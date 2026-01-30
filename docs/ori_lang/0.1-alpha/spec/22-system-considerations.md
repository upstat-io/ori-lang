---
title: "System Considerations"
description: "Ori Language Specification — System Considerations"
order: 22
section: "Tooling"
---

# System Considerations

This section specifies implementation-level requirements and platform considerations.

## Numeric Types

### Integers

The `int` type is a 64-bit signed integer.

| Property | Value |
|----------|-------|
| Size | 64 bits |
| Minimum | -9,223,372,036,854,775,808 (-2^63) |
| Maximum | 9,223,372,036,854,775,807 (2^63 - 1) |
| Overflow | Panics (see [Error Codes](https://ori-lang.com/docs/compiler-design/appendices/c-error-codes)) |

There is no separate unsigned integer type. Bitwise operations treat the value as unsigned bits.

### Floats

The `float` type is a 64-bit IEEE 754 double-precision floating-point number.

| Property | Value |
|----------|-------|
| Size | 64 bits |
| Precision | ~15-17 significant decimal digits |
| Range | ±1.7976931348623157 × 10^308 |

Special values `inf`, `-inf`, and `nan` are supported.

## Strings

### Encoding

All strings are UTF-8 encoded. There is no separate ASCII or byte-string type.

```ori
let greeting = "Hello, 世界"  // UTF-8
let emoji = "🎉"              // UTF-8
```

### Indexing

String indexing returns a single Unicode codepoint as a `str`:

```ori
let s = "héllo"
s[0]  // "h"
s[1]  // "é" (single codepoint)
```

The index refers to codepoint position, not byte position. Out-of-bounds indexing panics.

### Grapheme Clusters

Some visual characters consist of multiple codepoints:

```ori
let astronaut = "🧑‍🚀"  // 3 codepoints: person + ZWJ + rocket
len(astronaut)        // 3
astronaut[0]          // "🧑"
```

For grapheme-aware operations, use standard library functions.

### Length

`len(str)` returns the number of codepoints, not bytes.

```ori
len("hello")  // 5
len("世界")    // 2
len("🧑‍🚀")    // 3 (codepoints, not graphemes)
```

## Collections

### Limits

Collections have no fixed size limits. Maximum size is bounded by available memory.

| Collection | Limit |
|------------|-------|
| List | Memory |
| Map | Memory |
| String | Memory |

### Capacity

Implementations may pre-allocate capacity for performance. This is not observable behavior.

## Recursion

### Tail Call Optimization

Tail calls are guaranteed to be optimized. A tail call does not consume stack space:

```ori
@countdown (n: int) -> void =
    if n <= 0 then void else countdown(n: n - 1)  // tail call

countdown(n: 1000000)  // does not overflow stack
```

A call is in tail position if it is the last operation before the function returns.

### Non-Tail Recursion

Non-tail recursive calls consume stack space. Deep recursion may cause stack overflow:

```ori
@sum_to (n: int) -> int =
    if n <= 0 then 0 else n + sum_to(n: n - 1)  // not tail call

sum_to(n: 1000000)  // may overflow stack
```

For deep recursion, use the `recurse` pattern with `memo: true` or convert to tail recursion.

## Platform Support

### Target Platforms

Conforming implementations should support:

- Linux (x86-64, ARM64)
- macOS (x86-64, ARM64)
- Windows (x86-64)
- WebAssembly (WASM)

### Endianness

Byte order is implementation-defined. Programs should not depend on endianness unless using platform-specific byte manipulation.

### Path Separators

File paths use the platform-native separator. The standard library provides cross-platform path operations.

## Implementation Limits

Implementations may impose limits on:

| Aspect | Minimum Required |
|--------|------------------|
| Identifier length | 1024 characters |
| Nesting depth | 256 levels |
| Function parameters | 255 |
| Generic parameters | 64 |

Exceeding these limits is a compile-time error.
