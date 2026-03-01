---
title: "Collections & COW"
description: "Ori Compiler Design — Copy-on-Write Collection Semantics"
order: 1102
section: "Runtime"
sidebar_title: "Collections & COW"
sidebar_order: 2
sidebar_path: "/docs/compiler-design/11-runtime/collections-cow"
---

# Collections & COW

Ori collections (lists, maps, sets) use Copy-on-Write (COW) semantics to combine
the safety of immutable value semantics with the performance of in-place mutation.
When a collection has a single owner (reference count of 1), mutations happen in
place. When the collection is shared (reference count greater than 1), mutations
first copy the underlying buffer.

## The COW Decision Flow

Every mutating collection operation follows the same decision sequence:

```
is_slice_cap(cap)?     -->  yes -->  SLOW PATH (always copy)
       |
       no
       |
ori_rc_is_unique(data)? -->  no  -->  SLOW PATH (copy + inc elements)
       |
       yes
       |
has capacity?  -->  no  -->  ori_rc_realloc (grow in place, O(1) amortized)
       |
       yes
       |
FAST PATH: mutate data buffer directly (O(1))
```

Slices always take the slow path because their `data` pointer is interior to
another allocation -- calling `ori_rc_is_unique(data)` on a slice would read
garbage from the wrong memory location.

## Consuming Semantics

All COW operations use **consuming semantics**: they take ownership of the
caller's reference to the data buffer and produce a new `{len, cap, data}`
triple via an `out_ptr` sret parameter. The caller must not access the original
buffer after the call.

This design enables the fast path to reuse the existing buffer without any RC
changes -- the sole reference simply transfers from input to output. On the slow
path, the old buffer's RC is decremented (via `dec_list_buffer` for lists, which
handles both regular buffers and seamless slices).

```
// Conceptual signature of a COW operation:
fn mutate_cow(
    data: *mut u8,       // consumed: caller's reference transferred
    len: i64,
    cap: i64,
    /* mutation parameters */
    inc_fn: Option<fn(*mut u8)>,  // element RC increment callback
    out_ptr: *mut u8,    // sret: {len, cap, data} written here
)
```

## Element RC Callbacks

When the slow path copies elements from one buffer to another, each element
that is itself RC-managed needs its reference count incremented. The
`inc_copied_elements` helper handles this:

```rust
fn inc_copied_elements(
    data: *mut u8,
    count: usize,
    elem_size: usize,
    inc_fn: Option<extern "C" fn(*mut u8)>,
)
```

If `inc_fn` is `None`, the elements are plain data (`int`, `float`, `bool`) and
no RC work is needed. If `inc_fn` is `Some`, it is called once per element to
increment the element's internal reference counts.

This callback-based design keeps the runtime type-agnostic. The LLVM backend
generates type-specialized `inc_fn` and `elem_dec_fn` trampolines for each
concrete element type.

## List COW Operations

### `ori_list_push_cow`

Appends an element to a list.

**Fast path** (unique, has capacity):
1. Copies `elem_size` bytes from `elem` to `data + len * elem_size`
2. Increments `len`
3. Returns same `data` pointer -- no RC changes

**Fast path** (unique, needs growth):
1. Calls `ori_rc_realloc` to grow the buffer (2x capacity via `next_capacity`)
2. Copies the element into the new space
3. Returns the (possibly new) `data` pointer

**Slow path** (shared or empty):
1. Allocates a new buffer with capacity `next_capacity(old_cap, old_len + 1)`
2. Copies all existing elements via `copy_nonoverlapping`
3. Copies the new element into position
4. Calls `inc_copied_elements` to increment RC on all copied elements
5. Calls `dec_list_buffer` on the old buffer (slice-aware RC dec)
6. Writes the new list to `out_ptr`

### `ori_list_pop_cow`

Removes the last element. The element must be extracted before calling pop
(via index access) -- this function only shortens the list.

**Fast path** (unique): Decrements `len` (O(1) -- element remains in buffer
but is logically inaccessible). Does not auto-shrink capacity.

**Slow path** (shared or slice): Allocates new buffer, copies `len - 1`
elements, increments their RC, decrements old buffer's RC.

### `ori_list_set_cow`

Replaces the element at a given index.

**Fast path** (unique): Overwrites `elem_size` bytes at `data + index * elem_size`
in place. The old element's RC is the codegen's responsibility.

**Slow path** (shared): Allocates new buffer, copies all elements, overwrites
the element at `index`, increments RC for all copied elements except the
overwritten one.

### `ori_list_insert_cow` and `ori_list_remove_cow`

These follow the same COW pattern but additionally shift elements to maintain
contiguity. Insert uses `memmove` to shift elements right from the insertion
point; remove uses `memmove` to shift elements left to fill the gap.

### `ori_list_concat_cow` (Dual-Consuming)

Concatenation consumes **both** input lists. The runtime checks uniqueness of
each buffer independently to select the optimal strategy:

| list1   | list2   | Strategy                                        |
|---------|---------|-------------------------------------------------|
| unique  | unique  | Reuse list1 buffer, **move** list2 (no inc)     |
| unique  | shared  | Reuse list1 buffer, **copy** list2 (inc each)   |
| shared  | unique  | New buffer, copy list1 (inc), **move** list2    |
| shared  | shared  | New buffer, copy both (inc all)                 |

**Bonus**: list1 empty + list2 unique triggers a O(1) takeover of list2's buffer.

List2's consumed buffer is cleaned up via `dec_consumed_list2`, which checks
uniqueness at disposal time (not the initial snapshot) to handle self-concat
(`x + x`) correctly.

### `ori_list_reverse_cow`

**Fast path** (unique): Swaps pairs from both ends working inward using a
temporary element buffer. O(n), no allocation.

**Slow path** (shared): Allocates new buffer, copies elements in reverse order.

### `ori_list_sort_cow` / `ori_list_sort_stable_cow`

**Both paths**: Build a sorted index array via Rust's `sort_unstable_by` (or
`sort_by` for stable). Index-based sorting avoids moving elements during
comparison.

**Fast path** (unique): Applies the permutation in place using cycle-following
(`apply_permutation_in_place`). O(n log n) sort + O(n) permutation, no
allocation beyond the index array.

**Slow path** (shared): Copies elements to a new buffer in sorted order using
the index array. O(n log n) sort + O(n) copy.

## Map COW Operations

Maps use a split-buffer layout (`[keys|values]`), which adds complexity to COW
growth: when the buffer is reallocated with a larger capacity, the values section
must be **relocated** because the keys section has expanded.

### `ori_map_insert_cow`

**Key exists + unique**: Overwrites value in place at
`data + cap * key_size + idx * val_size`.

**Key exists + shared**: Copies all keys and values to a new buffer, overwrites
value at `idx`, increments RC for all copied keys and for all copied values
except the overwritten one.

**New key + unique + has capacity**: Appends key at `data + len * key_size` and
value at `data + cap * key_size + len * val_size`.

**New key + unique + needs growth**: Reallocs the buffer, then uses `memmove`
to relocate the values section from `old_cap * key_size` to
`new_cap * key_size`. Then appends the new entry.

**New key + shared/empty**: Allocates a new buffer, copies existing keys to
offset 0, copies existing values to `new_cap * key_size`, appends the new entry.
Increments RC for all copied keys and values.

### `ori_map_remove_cow`

**Unique + key found**: Uses `memmove` to shift keys left and values left
separately (overlapping regions). Returns with `len - 1`.

**Unique + last entry**: Frees the buffer, returns empty sentinel.

**Key not found**: Returns input unchanged regardless of sharing.

**Shared + key found**: Allocates new buffer with `new_len` entries, copies
all entries except the removed one. Increments RC for copied elements.

### `ori_map_update_cow`

Replaces the value for an existing key. If the key is not found, returns the
input unchanged (no insertion). Delegates to the same `cow_insert_existing`
logic as `ori_map_insert_cow` to avoid code duplication.

## Set COW Operations

Sets use the same contiguous layout as lists, so COW mechanics are simpler
than maps (no split-buffer relocation needed). Set COW operations accept an
`elem_eq` callback for type-agnostic element comparison.

### `ori_set_insert_cow`

Checks for membership first via `find_elem` with the `elem_eq` callback. If
the element already exists, returns the input unchanged (no-op). Otherwise
follows the same push-style COW pattern as lists.

### `ori_set_remove_cow`

Finds the element via `find_elem`. If not found, returns unchanged. If found:
- **Unique**: Shifts remaining elements left via `memmove`.
- **Unique + last element**: Frees buffer, returns empty sentinel.
- **Shared**: Copies all elements except the removed one to a new buffer.

### `ori_set_union_cow` (Consuming set1, Borrowing set2)

Union consumes set1's reference but only borrows set2. First counts how many
elements from set2 are new (not in set1). If zero, returns set1 unchanged.

**Fast path** (set1 unique): Extends set1 in place, appending unique elements
from set2. Reallocs if capacity is insufficient.

**Slow path** (set1 shared): Allocates new buffer containing all of set1 plus
unique elements from set2. Uses write-cursor compaction.

### `ori_set_intersection_cow` / `ori_set_difference_cow`

Both use write-cursor compaction on the fast path (unique owner): iterate
through set1, keeping only elements that pass the membership test (present
in set2 for intersection, absent from set2 for difference). No shifting needed
-- the write cursor naturally compacts surviving elements.

**Slow path** (shared): Allocates a new buffer, copies only qualifying
elements. If the result is empty, frees the new buffer and returns the empty
sentinel.

## Seamless Slices

List slices use a negative capacity value to encode a byte offset back to the
original buffer:

```
cap >= 0:  regular collection (cap is capacity in elements)
cap <  0:  seamless slice
           SLICE_FLAG (i64::MIN) | byte_offset_from_original_data_start

Recovering the original data pointer:
  original_data = slice_data - byte_offset
  RC header at original_data - 16
```

This allows slices to share the underlying buffer with the original list
without a separate slice type. Key properties:

- `len` gives the slice length as usual
- `data` gives direct access to the slice elements (no offset calculation)
- Deallocation decrements the RC of the **original** buffer via
  `slice_original_data(data, cap)`
- COW on a slice creates a fresh buffer with only the slice's elements
- Slices of slices are supported: the byte offset accumulates to always point
  back to the original allocation

The `is_slice_cap` / `slice_byte_offset` / `make_slice_cap` / `slice_original_data`
helpers in `slice_encoding/mod.rs` provide the encoding primitives.

## Capacity Management

### `MIN_COLLECTION_CAPACITY = 4`

The minimum initial capacity for all collections. Avoids pathological
single-element reallocations (1 -> 2 -> 4) by jumping directly to a
reasonable initial size.

### Growth: `next_capacity(current, required)`

Returns `max(required, current * 2, MIN_COLLECTION_CAPACITY)`. Uses 2x
doubling (matching Rust's `Vec`, Swift's `Array`, Java's `ArrayList`):
- Amortized O(1) insertion
- At most 50% wasted capacity
- Simple and well-understood

### Empty Collections

Empty collections (`len == 0`) use a null data pointer and zero capacity. No
allocation occurs until the first element is added. `ori_rc_inc(null)` and
`ori_rc_dec(null)` are no-ops, so empty collections require zero allocation
and zero cleanup.

### No Auto-Shrink

Collections do not automatically shrink. Once a buffer is allocated at a given
capacity, it retains that capacity even if elements are removed. This avoids
the performance cliff of alternating growth and shrinkage around a capacity
boundary. Matches Rust's `Vec` behavior.

## sret Output Convention

All COW operations write results through an `out_ptr` parameter (the sret
pattern) rather than returning by value:

```c
// Caller (generated by LLVM codegen):
OriList result;
ori_list_push_cow(&result, data, len, cap, &elem, elem_size, elem_align, inc_fn);
// result now contains the updated {len, cap, data} triple
```

This is necessary because `OriList`, `OriMap`, and `OriSet` are 24 bytes,
which exceeds the 16-byte threshold for register return on x86-64 System V
ABI. With explicit `sret`, the codegen controls the destination, which is
important for correct integration with the rest of the compiled code.

## Performance Summary

| Operation        | Fast Path (unique) | Slow Path (shared)        |
|------------------|--------------------|---------------------------|
| Push             | O(1) amortized     | O(n) copy + alloc         |
| Pop              | O(1)               | O(n) copy + alloc         |
| Set (by index)   | O(1)               | O(n) copy + alloc         |
| Insert (at index)| O(n) shift         | O(n) copy + alloc         |
| Remove (at index)| O(n) shift         | O(n) copy + alloc         |
| Concat           | O(m) append        | O(n+m) copy + alloc       |
| Reverse          | O(n) swap          | O(n) copy + alloc         |
| Sort             | O(n log n)         | O(n log n) sort + O(n) copy|
| Map insert       | O(n) scan + O(1)   | O(n) scan + O(n) copy     |
| Map remove       | O(n) scan + shift  | O(n) scan + O(n) copy     |
| Set insert       | O(n) scan + O(1)   | O(n) scan + O(n) copy     |
| Set union        | O(n*m) membership  | O(n*m) membership + copy  |
