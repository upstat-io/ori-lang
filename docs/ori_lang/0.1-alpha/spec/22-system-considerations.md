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

The `int` type is a signed integer with the following semantic range:

| Property | Value |
|----------|-------|
| Canonical size | 64 bits |
| Minimum | -9,223,372,036,854,775,808 (-2⁶³) |
| Maximum | 9,223,372,036,854,775,807 (2⁶³ - 1) |
| Overflow | Panics (see [Error Codes](https://ori-lang.com/docs/compiler-design/appendices/c-error-codes)) |

The canonical size defines the semantic range. The compiler may use a narrower machine representation (see [§ Representation Optimization](#representation-optimization)).

There is no separate unsigned integer type. Bitwise operations treat the value as unsigned bits.

### Floats

The `float` type is an IEEE 754 double-precision floating-point number:

| Property | Value |
|----------|-------|
| Canonical size | 64 bits |
| Precision | ~15-17 significant decimal digits |
| Range | ±1.7976931348623157 × 10³⁰⁸ |

The canonical size defines the semantic precision. The compiler may use a narrower machine representation when it can prove no precision loss (see [§ Representation Optimization](#representation-optimization)).

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

`len(str)` returns the number of bytes, not codepoints. Use `.chars().count()` for codepoint count.

```ori
len("hello")  // 5 (5 bytes)
len("世界")    // 6 (each character is 3 UTF-8 bytes)
len("🧑‍🚀")    // 11 (multi-byte emoji ZWJ sequence: 4+3+4)
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

## Representation Optimization

The compiler may optimize the machine representation of any type, provided the optimization preserves _semantic equivalence_. An optimization is semantically equivalent if no conforming program can distinguish the optimized representation from the canonical one through any language-level operation.

### Canonical Representations

| Type | Canonical | Semantic Range |
|------|-----------|----------------|
| `int` | 64-bit signed two's complement | [-2⁶³, 2⁶³ - 1] |
| `float` | 64-bit IEEE 754 binary64 | ±1.8 × 10³⁰⁸, ~15-17 digits |
| `bool` | 1-bit | `true` or `false` |
| `byte` | 8-bit unsigned | [0, 255] |
| `char` | 32-bit Unicode scalar | U+0000–U+10FFFF excluding surrogates |
| `Ordering` | Tri-state | `Less`, `Equal`, `Greater` |

### Permitted Optimizations

Permitted optimizations include but are not limited to:

- Narrowing primitive machine types (`bool` → `i1`, `byte` → `i8`, `char` → `i32`, `Ordering` → `i8`)
- Enum discriminant narrowing (`i8` for ≤256 variants)
- All-unit enum payload elimination
- Sum type shared payload slots (`Result<T, E>` uses `max(sizeof(T), sizeof(E))`)
- ARC operation elision for transitively trivial types
- Newtype representation erasure
- Struct field reordering for alignment
- Integer narrowing based on value range analysis
- Float narrowing when precision loss is provably zero

### Guarantees

1. The semantic range of every type is always preserved
2. Overflow behavior is determined by the semantic type, not the machine representation
3. Values stored and retrieved through any language operation are identical
4. `debug()` and `print()` display semantic values
5. `x == y` and `hash(x) == hash(y)` relationships are representation-independent
6. Value/reference type classification is determined by canonical size, not machine size

### Non-Guarantees

1. The exact machine representation of any type is unspecified
2. Memory layout may differ between compiler versions and target platforms
3. Struct field order in memory may differ from declaration order

> **Note:** For the full specification including optimization tiers, cross-cutting invariants, and interaction with `#repr` attributes, see [Representation Optimization Proposal](../../proposals/approved/representation-optimization-proposal.md).
