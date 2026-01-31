# Proposal: Duration and Size Literal Types

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Approved:** 2026-01-30
**Affects:** Compiler, type system, literals

---

## Summary

This proposal formalizes the `Duration` and `Size` types, including literal syntax, arithmetic operations, conversions, and comparison semantics.

---

## Problem Statement

The spec lists `Duration` and `Size` as primitive types but leaves unclear:

1. **Internal representation**: How are values stored?
2. **Literal parsing**: What units are supported?
3. **Arithmetic**: What operations are valid?
4. **Conversions**: How to convert between units?
5. **Overflow**: What happens on overflow?

---

## Duration Type

### Definition

`Duration` represents a span of time with nanosecond precision.

### Internal Representation

```ori
// Internally stored as:
type Duration = { nanoseconds: int }  // 64-bit signed integer
```

Range: approximately ±292 years.

### Literal Syntax

| Suffix | Unit | Nanoseconds |
|--------|------|-------------|
| `ns` | nanoseconds | 1 |
| `us` | microseconds | 1,000 |
| `ms` | milliseconds | 1,000,000 |
| `s` | seconds | 1,000,000,000 |
| `m` | minutes | 60,000,000,000 |
| `h` | hours | 3,600,000,000,000 |

```ori
let t1 = 100ns   // 100 nanoseconds
let t2 = 50us    // 50 microseconds
let t3 = 100ms   // 100 milliseconds
let t4 = 30s     // 30 seconds
let t5 = 5m      // 5 minutes
let t6 = 2h      // 2 hours
```

### Numeric Prefix

Literals accept integer prefixes:

```ori
let timeout = 30s
let delay = 100ms
let interval = 5m
```

Floating-point prefixes are NOT supported:

```ori
let bad = 1.5s   // ERROR: use 1500ms instead
let bad = 0.5h   // ERROR: use 30m instead
```

### Arithmetic

| Operation | Types | Result |
|-----------|-------|--------|
| `d1 + d2` | Duration + Duration | Duration |
| `d1 - d2` | Duration - Duration | Duration |
| `d * n` | Duration * int | Duration |
| `n * d` | int * Duration | Duration |
| `d / n` | Duration / int | Duration |
| `d1 / d2` | Duration / Duration | int (ratio) |
| `d1 % d2` | Duration % Duration | Duration (remainder) |
| `-d` | -Duration | Duration |

```ori
let total = 5s + 100ms      // 5100ms
let diff = 10s - 3s         // 7s
let scaled = 100ms * 10     // 1000ms = 1s
let halved = 2h / 2         // 1h
let ratio = 1h / 30m        // 2
let remainder = 5s % 3s     // 2s
```

### Comparison

Durations are fully comparable:

```ori
100ms < 1s    // true
1h == 60m     // true
30s >= 30s    // true
```

### Conversion Methods

```ori
impl Duration {
    @nanoseconds (self) -> int
    @microseconds (self) -> int
    @milliseconds (self) -> int
    @seconds (self) -> int
    @minutes (self) -> int
    @hours (self) -> int

    @from_nanoseconds (ns: int) -> Duration
    @from_microseconds (us: int) -> Duration
    @from_milliseconds (ms: int) -> Duration
    @from_seconds (s: int) -> Duration
    @from_minutes (m: int) -> Duration
    @from_hours (h: int) -> Duration
}
```

```ori
let d = 90s
d.seconds()   // 90
d.minutes()   // 1 (truncated)
d.milliseconds()  // 90000

let d2 = Duration.from_seconds(s: 120)  // 2m
```

### Traits Implemented

- `Eq` — equality comparison
- `Comparable` — ordering
- `Hashable` — can be map key
- `Clone` — copyable
- `Debug` — debug representation
- `Printable` — human-readable format
- `Default` — `0ns`
- `Sendable` — safe across tasks

### Overflow Behavior

Duration arithmetic panics on overflow:

```ori
let max = 9223372036854775807ns
max + 1ns  // panic: Duration overflow
```

---

## Size Type

### Definition

`Size` represents a byte count.

### Internal Representation

```ori
// Internally stored as:
type Size = { bytes: int }  // 64-bit signed integer (non-negative)
```

Range: 0 to ~8 exabytes.

### Literal Syntax

| Suffix | Unit | Bytes |
|--------|------|-------|
| `b` | bytes | 1 |
| `kb` | kilobytes | 1,024 |
| `mb` | megabytes | 1,048,576 |
| `gb` | gigabytes | 1,073,741,824 |
| `tb` | terabytes | 1,099,511,627,776 |

```ori
let s1 = 100b    // 100 bytes
let s2 = 4kb     // 4 kilobytes = 4096 bytes
let s3 = 10mb    // 10 megabytes
let s4 = 2gb     // 2 gigabytes
let s5 = 1tb     // 1 terabyte
```

### Binary vs Decimal

Ori uses binary units (powers of 1024), not decimal (powers of 1000):

```ori
1kb == 1024b    // true (binary kilobyte)
1mb == 1024kb   // true
```

### Arithmetic

| Operation | Types | Result |
|-----------|-------|--------|
| `s1 + s2` | Size + Size | Size |
| `s1 - s2` | Size - Size | Size (panics if negative) |
| `s * n` | Size * int | Size |
| `n * s` | int * Size | Size |
| `s / n` | Size / int | Size |
| `s1 / s2` | Size / Size | int (ratio) |
| `s1 % s2` | Size % Size | Size (remainder) |

```ori
let total = 1mb + 512kb   // 1.5mb = 1572864b
let ratio = 1gb / 1mb     // 1024
let aligned = 1025b % 1kb // 1b
```

### Non-Negative Constraint

Size cannot be negative. Unary negation (`-`) is not permitted on Size literals or expressions:

```ori
let bad = -1kb   // ERROR: unary negation not allowed on Size
let bad = 1kb - 2kb  // panic: Size cannot be negative
```

### Comparison

Sizes are fully comparable:

```ori
1mb < 1gb     // true
1024kb == 1mb // true
```

### Conversion Methods

```ori
impl Size {
    @bytes (self) -> int
    @kilobytes (self) -> int
    @megabytes (self) -> int
    @gigabytes (self) -> int
    @terabytes (self) -> int

    @from_bytes (b: int) -> Size
    @from_kilobytes (kb: int) -> Size
    @from_megabytes (mb: int) -> Size
    @from_gigabytes (gb: int) -> Size
    @from_terabytes (tb: int) -> Size
}
```

```ori
let s = 1536kb
s.kilobytes()   // 1536
s.megabytes()   // 1 (truncated)
s.bytes()       // 1572864
```

### Traits Implemented

- `Eq` — equality comparison
- `Comparable` — ordering
- `Hashable` — can be map key
- `Clone` — copyable
- `Debug` — debug representation
- `Printable` — human-readable format
- `Default` — `0b`
- `Sendable` — safe across tasks

---

## Printable Format

### Duration

```ori
100ns.to_str()    // "100ns"
1500ms.to_str()   // "1500ms" or "1.5s"
90s.to_str()      // "1m 30s"
3661s.to_str()    // "1h 1m 1s"
```

Implementation may use most appropriate unit(s).

### Size

Uses casual notation (KB/MB) rather than IEC notation (KiB/MiB):

```ori
100b.to_str()     // "100 bytes"
1024b.to_str()    // "1 KB"
1536kb.to_str()   // "1.5 MB"
```

---

## Debug Format

### Duration

```ori
100ms.debug()  // "Duration { nanoseconds: 100000000 }"
```

### Size

```ori
1kb.debug()  // "Size { bytes: 1024 }"
```

---

## Use Cases

### Timeouts

```ori
timeout(op: fetch(url), after: 30s)
```

### Caching

```ori
cache(key: url, op: fetch(url), ttl: 5m)
```

### Rate Limiting

```ori
let min_interval = 100ms
```

### Buffer Sizes

```ori
let buffer_size = 64kb
let max_file_size = 10mb
```

### Memory Limits

```ori
let heap_limit = 2gb
```

---

## Error Messages

### Invalid Suffix

```
error[E0910]: invalid duration suffix
  --> src/main.ori:5:10
   |
 5 | let t = 100x
   |            ^ unknown suffix 'x'
   |
   = note: valid duration suffixes: ns, us, ms, s, m, h
   = note: valid size suffixes: b, kb, mb, gb, tb
```

### Float Prefix

```
error[E0911]: floating-point duration literal not supported
  --> src/main.ori:5:10
   |
 5 | let t = 1.5s
   |         ^^^^ use integer with smaller unit
   |
   = help: use `1500ms` instead of `1.5s`
```

### Negative Size

```
error[E0912]: Size cannot be negative
  --> src/main.ori:5:10
   |
 5 | let s = -1kb
   |         ^^^^ unary negation not allowed on Size
   |
   = note: Size represents byte counts (non-negative)
```

---

## Spec Changes Required

### Update `06-types.md`

Expand Duration and Size sections with:
1. Internal representation
2. Complete literal syntax
3. Arithmetic operations (including modulo)
4. Conversion methods
5. Trait implementations

### Update `03-lexical-elements.md`

Add duration and size literal tokens to lexical grammar.

### Update `grammar.ebnf`

Update duration and size unit productions:
- `duration_unit = "ns" | "us" | "ms" | "s" | "m" | "h" .`
- `size_unit = "b" | "kb" | "mb" | "gb" | "tb" .`

---

## Summary

| Aspect | Duration | Size |
|--------|----------|------|
| Represents | Time span | Byte count |
| Storage | 64-bit nanoseconds | 64-bit bytes |
| Suffixes | ns, us, ms, s, m, h | b, kb, mb, gb, tb |
| Negative | Allowed | Compile error (unary -) or panic (subtraction) |
| Overflow | Panic | Panic |
| Default | `0ns` | `0b` |
| Units | Metric time | Binary (1024-based) |
| Modulo | Supported | Supported |
