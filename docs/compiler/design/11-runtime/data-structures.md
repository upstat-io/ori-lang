---
title: "Data Structures"
description: "Ori Compiler Design — Runtime Data Structure Layouts"
order: 1104
section: "Runtime"
sidebar_title: "Data Structures"
sidebar_order: 4
sidebar_path: "/docs/compiler-design/11-runtime/data-structures"
---

# Data Structures

This document describes the memory layout of the core data types in the Ori
runtime. Each type is designed for C-ABI compatibility (`#[repr(C)]`),
efficient COW semantics, and minimal allocation overhead.

## RC Header (V3)

All heap-allocated, reference-counted objects share a 16-byte header placed
**before** the data pointer:

```
RC allocation (V3 layout):
  +------------------+------------------+---------------------+
  | data_size: i64   | strong_count: i64| data bytes ...      |
  +------------------+------------------+---------------------+
  ^                   ^                   ^
  base (ptr - 16)     ptr - 8             data_ptr (returned by ori_rc_alloc)
```

| Field          | Offset from data_ptr | Description                           |
|----------------|---------------------|---------------------------------------|
| `data_size`    | -16                 | User data size in bytes               |
| `strong_count` | -8                  | Reference count (atomic i64)          |

Key properties:

- **Data pointer returned**: `ori_rc_alloc` returns the data pointer (past the
  header), not the base pointer. Data pointers can be passed to C FFI without
  adjustment.
- **Single pointer on stack**: No separate header pointer needed. All RC
  operations (`inc`, `dec`, `count`, `is_unique`) access the count at
  `data_ptr - 8`.
- **`data_size` enables seamless slices**: When a slice is the last reference,
  it can compute the original data pointer and read the allocation size from
  the header for `ori_rc_free` without external bookkeeping.
- **Atomic operations**: Multi-threaded mode uses `AtomicI64` with `Relaxed`
  for `inc`/`is_unique` and `Release` + `Acquire` fence for `dec`. The
  `single-threaded` feature flag substitutes plain `i64` reads/writes.

The header size is `RC_HEADER_SIZE = 16`. Minimum alignment is 8 bytes
(enforced by `ori_rc_alloc`).

## OriList

`OriList` is a 24-byte `#[repr(C)]` struct representing a dynamic array:

```
OriList (24 bytes):
  +----------+----------+----------+
  | len: i64 | cap: i64 | data: *  |
  | [0..7]   | [8..15]  | [16..23] |
  +----------+----------+----------+
```

| Field  | Type      | Description                                       |
|--------|-----------|---------------------------------------------------|
| `len`  | `i64`     | Number of elements currently in the list           |
| `cap`  | `i64`     | Capacity in elements (negative = seamless slice)   |
| `data` | `*mut u8` | Pointer to RC-managed data buffer (or null)        |

Elements are stored contiguously at `data + index * elem_size`. The element
size is **not** stored in the struct -- it is always passed as a parameter to
runtime functions. This keeps the struct at 24 bytes and avoids redundancy.

### Data Buffer Layout

```
RC allocation:
  +-------------+-------------+--------+--------+-----+--------+---------+
  | data_size   | strong_count| elem0  | elem1  | ... | elemN  | (unused)|
  | (RC header) | (RC header) | (user data)                    |         |
  +-------------+-------------+--------+--------+-----+--------+---------+
                               ^
                               data pointer
```

### Empty List

```
OriList { len: 0, cap: 0, data: null }
```

No allocation occurs for empty lists. `ori_rc_inc(null)` and
`ori_rc_dec(null)` are no-ops. The first element insertion triggers an
allocation with `MIN_COLLECTION_CAPACITY = 4`.

### Seamless Slices

List slices reuse the `OriList` struct with a special encoding in `cap`:

```
cap >= 0:  regular list (cap is capacity in elements)
cap <  0:  seamless slice
           bit 63 = 1 (SLICE_FLAG = i64::MIN)
           bits 0-62 = byte offset from original allocation's data start
```

The slice's `data` pointer points directly to the first slice element within
the original buffer. To find the original allocation for RC operations:

```
original_data = slice_data - byte_offset
RC header at original_data - 16
```

Properties:
- `len` gives the slice length as usual
- `data` gives direct access to the slice elements
- RC dec goes through the original buffer (via `slice_original_data`)
- COW on a slice always takes the slow path (allocates independent buffer)
- Slices of slices accumulate the byte offset

### Allocation Functions

| Function              | Purpose                                          |
|-----------------------|--------------------------------------------------|
| `ori_list_alloc_data` | Allocate RC-managed buffer for `cap * elem_size` bytes |
| `ori_list_box_new`    | Wrap `{len, cap, data}` in an RC-managed OriList |
| `ori_list_new`        | Allocate OriList + data buffer (AOT mode)        |
| `ori_list_free`       | Free heap-allocated OriList (from `ori_list_new`) |
| `ori_list_free_data`  | Free data buffer only (stack-struct lists)        |

Also used for sets, which share the same memory layout (`OriList` struct with
contiguous element storage).

## OriMap

`OriMap` is a 24-byte `#[repr(C)]` struct representing an associative array:

```
OriMap (24 bytes):
  +----------+----------+----------+
  | len: i64 | cap: i64 | data: *  |
  | [0..7]   | [8..15]  | [16..23] |
  +----------+----------+----------+
```

### Split-Buffer Layout

Maps store keys and values in a **single contiguous RC-managed buffer** with
keys packed at the front and values packed after the key region:

```
RC-managed buffer:
  +------+------+-----+------+---------+------+------+-----+------+---------+
  | key0 | key1 | ... | keyN | (unused)| val0 | val1 | ... | valN | (unused)|
  +------+------+-----+------+---------+------+------+-----+------+---------+
  ^                                     ^
  data + 0                              data + cap * key_size
  (keys region)                         (values region)
```

Key storage: `data + index * key_size`
Value storage: `data + cap * key_size + index * val_size`
Total buffer size: `cap * key_size + cap * val_size`

Advantages:
- **Single RC header**: One `ori_rc_alloc` covers the entire map. One
  `ori_rc_is_unique` check determines the COW path.
- **Cache locality**: Keys are packed together, which helps the linear scan
  in `find_key` stay in cache.
- **Simple COW**: A single buffer copy duplicates both keys and values.

### Type-Agnostic Key Lookup

Maps do not use hash tables. Key lookup uses linear scan with a caller-provided
equality callback:

```rust
fn find_key(
    data: *const u8,
    len: usize,
    key_size: usize,
    needle: *const u8,
    key_eq: extern "C" fn(*const u8, *const u8) -> bool,
) -> Option<usize>
```

The `key_eq` callback is generated by the LLVM codegen for each concrete key
type. For `int` keys, this compiles to a direct 8-byte comparison. For `str`
keys, this calls `ori_str_eq`. For user-defined types, it invokes the derived
`Eq` implementation.

Linear scan is efficient for small maps (the common case in Ori programs).

### Growth Complication

When a map buffer is reallocated with a larger capacity, the values section
must be **relocated** because `cap * key_size` increases. This is handled in
`cow_insert_new` using `memmove` (overlapping regions) after realloc:

```
Before realloc (cap=4):  [k0 k1 _ _ | v0 v1 _ _]
After  realloc (cap=8):  [k0 k1 _ _ _ _ _ _ | v0 v1 (from old offset)]
                                               ^-- must memmove to new offset
Relocated (cap=8):       [k0 k1 _ _ _ _ _ _ | v0 v1 _ _ _ _ _ _]
```

### Empty Map

```
OriMap { len: 0, cap: 0, data: null }
```

## OriSet

Sets are built on the same infrastructure as lists, using a contiguous
`OriList`-style layout with a single element type. Set operations accept either
`raw_bytes_eq` (memcmp-based for fixed-representation types) or an `elem_eq`
callback (for COW operations).

```
OriSet (24 bytes, same as OriList):
  +----------+----------+----------+
  | len: i64 | cap: i64 | data: *  |
  +----------+----------+----------+
  data buffer: [elem0 | elem1 | ... | elemN | (unused)]
```

## OriStr

`OriStr` is a 24-byte `#[repr(C)]` union with two variants. See the
[String SSO](./string-sso.md) section for the full layout and semantics.

```
OriStr (24 bytes, union):
  SSO:  [23 inline bytes | flags byte (0x80 | len)]
  Heap: [len: i64 | cap: i64 | data: *mut u8]
```

## OriOption

`OriOption<T>` represents Ori's `Option<T>` type:

```
OriOption<T>:
  +----------+------------------+
  | tag: i8  | value: T         |
  | (1 byte) | (sizeof T bytes) |
  +----------+------------------+
```

| Tag | Variant | Description                        |
|-----|---------|------------------------------------|
| `0` | `None`  | No value; `value` field is unused  |
| `1` | `Some`  | Value present in `value` field     |

The tag is a single byte (`i8`). Total size is `1 + sizeof(T)` plus alignment
padding required by `T`.

## OriResult

`OriResult<T>` represents Ori's `Result<T, E>` type:

```
OriResult<T, E>:
  +----------+-------------------------------+
  | tag: i8  | value: max(sizeof T, sizeof E)|
  | (1 byte) | (overlapping storage)         |
  +----------+-------------------------------+
```

| Tag | Variant | Description                            |
|-----|---------|----------------------------------------|
| `0` | `Ok`    | Success value of type `T` in `value`   |
| `1` | `Err`   | Error value of type `E` in `value`     |

The value field is sized to the larger of `T` and `E`. The storage is shared
(union-like). The compiler generates the correct access code based on the tag.

## OriPanic

The panic payload for stack unwinding in AOT mode:

```rust
pub struct OriPanic {
    pub message: String,
}
```

Wrapped in `std::panic::panic_any` so the Itanium EH ABI unwinds through
LLVM-generated `invoke`/`landingpad` pairs, giving cleanup handlers a chance
to release RC'd resources. The entry point wrapper (`ori_run_main`) catches
this with `catch_unwind`.

## Memory Diagrams

### List with 3 elements (`[int]`, elem_size = 8)

```
Stack (OriList, 24 bytes):
  +---------+---------+------------------+
  | len = 3 | cap = 4 | data ---------->-+---+
  +---------+---------+------------------+   |
                                             |
Heap (RC allocation):                        |
  +-------------+-------------+-----------+--v--------+----------+----------+
  | size = 32   | count = 1   | elem0 = 10| elem1 = 20| elem2 = 30| (unused) |
  | (RC header) | (RC header) | 8 bytes   | 8 bytes   | 8 bytes   | 8 bytes  |
  +-------------+-------------+-----------+-----------+----------+----------+
```

### Map with 2 entries (`{int: int}`, key_size = 8, val_size = 8)

```
Stack (OriMap, 24 bytes):
  +---------+---------+------------------+
  | len = 2 | cap = 4 | data ---------->-+---+
  +---------+---------+------------------+   |
                                             |
Heap (RC allocation, single buffer):         |
  +-------------+-------------+---+------+--v---+----+---------+---------+
  | size = 64   | count = 1   |key0|key1 | _ | _ | val0| val1 | _ | _   |
  | (RC header) | (RC header) | 8B | 8B  |8B |8B | 8B  | 8B   |8B |8B  |
  +-------------+-------------+----+-----+---+---+-----+------+---+----+
                               ^                   ^
                               data + 0            data + 4 * 8 = data + 32
                               (keys region)       (values region)
```

### Seamless Slice (slice of elements 1..3 from above list)

```
Stack (OriList, 24 bytes):
  +---------+---------------------+------------------+
  | len = 2 | cap = SLICE_FLAG|8  | data ---------->-+---+
  +---------+---------------------+------------------+   |
                                                         |
Heap (original allocation, shared):                      |
  +-------------+-------------+-----------+-----------+--v--------+----------+
  | size = 32   | count = 2   | elem0 = 10| elem1 = 20| elem2 = 30| (unused) |
  | (RC header) | (RC header) | 8 bytes   | 8 bytes   | 8 bytes   | 8 bytes  |
  +-------------+-------------+-----------+-----------+-----------+----------+
                                           ^
                                           slice data (offset 8 bytes from original)
```

## Iterator Runtime (`IterState`)

Iterators are represented as opaque `Box<IterState>` handles, cast to `*mut u8`
for the C ABI. LLVM code never sees the internal enum -- all interaction goes
through pointer-sized handles.

### `IterState` Variants

| Variant      | Source/Adapter | Key Fields                                |
|--------------|---------------|-------------------------------------------|
| `List`       | Source         | `data, len, pos, cap, elem_size, elem_dec_fn` |
| `Range`      | Source         | `current, end, step, inclusive`            |
| `Str`        | Source         | `data, len, byte_offset, owns_data`       |
| `Map`        | Source         | `data, cap, len, pos, key_size, val_size`  |
| `Mapped`     | Adapter        | `source, transform_fn, transform_env, in_size` |
| `Filtered`   | Adapter        | `source, predicate_fn, predicate_env, elem_size` |
| `TakeN`      | Adapter        | `source, remaining`                       |
| `SkipN`      | Adapter        | `source, remaining`                       |
| `Enumerated` | Adapter        | `source, index`                           |
| `Zipped`     | Adapter        | `left, right, left_elem_size`             |
| `Chained`    | Adapter        | `first, second, first_done`               |

### Trampoline Functions

Adapters that accept closures use C-ABI trampoline function pointers:

| Type            | Signature                                   | Used By          |
|-----------------|---------------------------------------------|------------------|
| `TransformFn`   | `(env, in_ptr, out_ptr) -> void`            | `Mapped`         |
| `PredicateFn`   | `(env, elem_ptr) -> bool`                   | `Filtered`, consumers |
| `ForEachFn`     | `(env, elem_ptr) -> void`                   | `for_each`       |
| `FoldFn`        | `(env, acc_ptr, elem_ptr, out_ptr) -> void` | `fold`           |

The `env` parameter is the closure environment pointer (may be null for
stateless operations). LLVM codegen generates type-specialized trampolines.

### Iterator `next()`

`ori_iter_next(iter, out_ptr, elem_size)` dispatches through the `IterState`
enum. Adapters use a stack scratch buffer of `MAX_ELEM_SIZE = 256` bytes for
intermediate values.

### RC Ownership

Source-level iterators (`List`, `Str`, `Map`) take ownership of one RC
reference to their data. The `Drop` impl releases this reference:

- `List`: calls `ori_buffer_rc_dec(data, len, cap, elem_size, elem_dec_fn)`
- `Str`: calls `ori_buffer_rc_dec(data, 0, len, 1, None)` when `owns_data`
- `Map`: calls `ori_map_buffer_rc_dec(...)` when `owns_data`

Adapter variants (`Mapped`, `Filtered`, etc.) contain a `Box<IterState>` that
Rust automatically drops after the parent's `drop()`, cascading cleanup to
the source iterator.

## Capacity Management

### `MIN_COLLECTION_CAPACITY = 4`

All collections start with capacity 4 upon first insertion. Avoids pathological
1 -> 2 -> 4 reallocations.

### `next_capacity(current, required) -> usize`

Returns `max(required, current * 2, MIN_COLLECTION_CAPACITY)`. Uses 2x
doubling for amortized O(1) insertion at the cost of up to 50% wasted capacity.
Matches Rust's `Vec`, Swift's `Array`, Java's `ArrayList`.

### No Auto-Shrink

Collections do not automatically shrink. Once allocated, capacity is retained
even after element removal. This avoids oscillation at capacity boundaries.
