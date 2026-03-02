---
title: "Reference Counting"
description: "Ori Compiler Design — RC Header Layout and Tracing"
order: 1101
section: "Runtime"
---

# Reference Counting

The runtime implements atomic reference counting for all heap-allocated values.
This system is the execution-time counterpart to the ARC analysis pass, which
statically determines where `inc` and `dec` operations must be inserted.

## V3 RC Header Layout

Every RC-managed allocation uses a 16-byte header placed immediately before the
user data:

```
Memory layout:

  ┌──────────────────┬──────────────────┬─────────────────────┐
  │  data_size: i64  │ strong_count: i64│  user data bytes... │
  │  (8 bytes)       │  (8 bytes)       │  (variable)         │
  └──────────────────┴──────────────────┴─────────────────────┘
  ^                                      ^
  │                                      │
  header_ptr                             data_ptr (returned to caller)
```

Key design decisions:

- **Data pointer points to user data, not the header.** This provides C FFI
  transparency: callers see a normal data pointer and never need to know about
  the header. The runtime recovers the header by subtracting 16 bytes from the
  data pointer.
- **`data_size` is stored in the header** rather than passed to every RC
  operation. This is essential for slices, where the runtime needs to know the
  allocation size for reallocation and deallocation without the caller tracking
  it separately.
- **`strong_count` is always 8-byte aligned** due to the preceding `data_size`
  field. This ensures atomic operations work correctly on all architectures
  without additional alignment padding.

The header is defined conceptually as:

```
struct RcHeader {
    data_size: i64,     // offset -16 from data_ptr
    strong_count: i64,  // offset -8 from data_ptr
}
```

## Core Functions

### `ori_rc_alloc(size: i64, align: i64) -> *mut u8`

Allocates a new RC-managed block with the given size and alignment. The returned
pointer points to the user data region (past the header).

1. Computes total allocation: `header_size + size` (where `header_size = 16`)
2. Allocates via `std::alloc::alloc` with the computed layout
3. Writes `data_size = size` at offset 0
4. Writes `strong_count = 1` at offset 8
5. Returns `alloc_ptr + 16`
6. Increments `RC_LIVE_COUNT` (atomic, for leak detection)

The initial reference count is always 1. The caller owns this reference.

### `ori_rc_inc(data_ptr: *mut u8)`

Atomically increments the strong count by 1.

```
(*header).strong_count.fetch_add(1, Ordering::Relaxed)
```

**Relaxed ordering** is sufficient for increments because:
- The increment only needs to be visible to the thread that later decrements.
- No data access depends on the increment being visible immediately.
- This matches the ordering used by Rust's `Arc::clone()` and Swift's
  `swift_retain`.

**Overflow protection**: If the count reaches `MAX_REFCOUNT` (defined as
`isize::MAX`), the process aborts immediately. Reference count overflow
indicates a logic error (likely a cycle or unbounded cloning) and cannot be
recovered from. This matches Rust's `Arc` behavior.

### `ori_rc_dec(data_ptr: *mut u8, drop_fn: Option<fn(*mut u8)>)`

Atomically decrements the strong count by 1. If the count reaches zero, the
allocation is freed.

```
let prev = (*header).strong_count.fetch_sub(1, Ordering::Release);
if prev == 1 {
    atomic::fence(Ordering::Acquire);
    // drop_fn callback (if any)
    // deallocate
}
```

The synchronization protocol follows the established Release/Acquire pattern:

1. **Release ordering on decrement**: Ensures all writes to the managed data
   by this thread are visible to whichever thread performs the final decrement.
2. **Acquire fence before drop**: Ensures the dropping thread sees all writes
   from all threads that previously decremented. This fence is only executed
   on the final decrement (count reaches zero), so it has no cost in the
   common case.

This is the same synchronization model used by:
- Rust's `Arc` (`std::sync::Arc`)
- Swift's `swift_release`
- C++ `shared_ptr` (libstdc++/libc++)

**Drop callback**: If `drop_fn` is `Some`, it is called with the data pointer
before deallocation. This handles recursive RC decrements for nested
heap-allocated values (e.g., a list of strings needs to decrement each string's
RC before freeing the list buffer).

**Underflow detection**: The runtime always checks for underflow (`prev == 0`
before decrement). This is a single branch per decrement that is always
not-taken in correct programs, so branch prediction eliminates the cost. On
underflow, the process aborts with a diagnostic message. This is always enabled
(not gated behind `ORI_RT_DEBUG`) because the cost is negligible and the
alternative (silent corruption) is unacceptable.

### `ori_rc_is_unique(data_ptr: *mut u8) -> bool`

Checks whether the allocation has exactly one reference:

```
(*header).strong_count.load(Ordering::Relaxed) == 1
```

This is the fast-path check for Copy-on-Write operations. If the value is
unique, mutations can proceed in place without copying. Relaxed ordering is
sufficient because:
- If we see `count == 1`, no other thread holds a reference, so there is no
  concurrent access to synchronize against.
- If we see `count > 1` (false negative is impossible due to the single-owner
  semantics of Ori's value types), we take the copy path, which is always safe.

### `ori_rc_realloc(data_ptr: *mut u8, old_size: i64, new_size: i64, align: i64) -> *mut u8`

Reallocates an RC-managed buffer to a new size. This function **requires unique
ownership** (the caller must have verified `ori_rc_is_unique` beforehand).

1. Computes old and new layouts including header
2. Calls `std::alloc::realloc`
3. Updates `data_size` in the header to `new_size`
4. Returns the new data pointer

If reallocation fails, the process aborts. There is no fallible reallocation
API.

### `ori_rc_free(data_ptr: *mut u8, size: i64, align: i64)`

Directly deallocates an RC-managed buffer without checking the reference count.
This is used in specific cases where the caller has already verified the count
is zero (e.g., inside `ori_rc_dec` after the final decrement).

1. Computes layout including header
2. Calls `std::alloc::dealloc`
3. Decrements `RC_LIVE_COUNT`

### `ori_rc_data_size(data_ptr: *mut u8) -> i64`

Reads the `data_size` field from the header. This is used by slice operations
that need to know the underlying buffer size without the caller passing it
explicitly.

```
*(data_ptr.sub(16) as *const i64)
```

## Synchronization Model

The atomic ordering choices are summarized:

| Operation       | Ordering            | Rationale                          |
|-----------------|---------------------|------------------------------------|
| `inc`           | Relaxed             | No data dependency                 |
| `dec`           | Release             | Publish writes before potential drop |
| fence before drop | Acquire           | See all prior writes               |
| `is_unique`     | Relaxed             | Single-owner semantics             |

This is the minimum correct ordering for a reference-counted pointer. Using
stronger orderings (e.g., `SeqCst`) would add unnecessary synchronization
barriers on architectures like x86 (where Release/Acquire are free) and ARM
(where they map to `dmb` instructions).

## MAX_REFCOUNT

```rust
const MAX_REFCOUNT: i64 = isize::MAX as i64;
```

If any increment would push the count past `MAX_REFCOUNT`, the process aborts.
This prevents the count from wrapping around to a small positive value, which
would lead to premature deallocation and use-after-free.

In practice, reaching `MAX_REFCOUNT` requires approximately 4.6 quintillion
increments on a 64-bit platform. This indicates a programming error (likely a
reference cycle), not legitimate usage.

## RC_LIVE_COUNT

The runtime maintains a global atomic counter of live RC allocations:

```rust
static RC_LIVE_COUNT: AtomicI64 = AtomicI64::new(0);
```

- Incremented in `ori_rc_alloc`
- Decremented in `ori_rc_free`

When `ORI_CHECK_LEAKS=1` is set, the runtime registers an `atexit` handler
that checks this counter. A non-zero value at exit indicates leaked allocations.

This is a coarse-grained diagnostic. It reports the total number of leaked
allocations but not their addresses or types. For per-allocation tracking,
use `ORI_TRACE_RC=1`.

## Tracing

When `ORI_TRACE_RC=1` is set, every RC operation logs a diagnostic line to
stderr:

```
[RC] alloc   0x7f8a1c000b70  size=48   count=1
[RC] inc     0x7f8a1c000b70  count=1->2
[RC] dec     0x7f8a1c000b70  count=2->1
[RC] dec     0x7f8a1c000b70  count=1->0  (dropping)
[RC] free    0x7f8a1c000b70  size=48
```

The tracing check uses `std::sync::Once` to read the environment variable once
and cache the result. The cost when tracing is disabled is a single
always-not-taken branch per RC operation.

## Interaction with ARC Analysis

The ARC analysis pass (see [ARC Analysis](../07-arc/index.md)) determines
statically where `ori_rc_inc` and `ori_rc_dec` calls must be inserted. The
runtime provides the actual implementations of these operations. The contract
between them is:

1. Every value produced by `ori_rc_alloc` starts with count 1.
2. Every additional use requires an `ori_rc_inc`.
3. Every end-of-use requires an `ori_rc_dec`.
4. The ARC pass guarantees that the net count reaches zero exactly when the
   value is no longer reachable.

The runtime does not perform any reachability analysis or cycle detection. Cycles
are prevented by Ori's value semantics (no shared mutable references).
