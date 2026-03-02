---
title: "Runtime Overview"
description: "Ori Compiler Design — AOT Runtime (ori_rt)"
order: 1100
section: "Runtime"
sidebar_title: "Runtime"
sidebar_order: 11
sidebar_path: "/docs/compiler-design/11-runtime"
---

# Runtime Overview

The `ori_rt` crate is the Ori runtime library for AOT-compiled programs. It provides
C-ABI functions that LLVM-generated code calls for memory management, reference
counting, collection operations, string handling, and I/O. The runtime has **zero
compiler dependencies** -- it links only against the Rust standard library and
system allocator.

## Architecture

The runtime sits at the bottom of the compilation pipeline. The LLVM backend
(`ori_llvm`) emits `call` instructions targeting `ori_rt`'s `#[no_mangle] extern "C"`
functions. These calls are resolved at link time when `libori_rt.a` (staticlib) is
linked into the final binary, or at JIT time when the rlib is loaded into the
`ori_llvm` execution engine.

```
Ori source  -->  ori_parse  -->  ori_types  -->  ori_llvm  -->  LLVM IR
                                                                  |
                                                            links against
                                                                  |
                                                              ori_rt (C ABI)
                                                                  |
                                                          native binary / JIT
```

The runtime never calls back into the compiler. Data flows one way: compiled code
calls runtime functions, the runtime operates on raw memory, and results are
returned through C ABI conventions (return values, sret output pointers, or
in-place mutation).

## Build Modes

The crate builds as both an `rlib` and a `staticlib`:

- **rlib** (`libori_rt.rlib`): Used by `ori_llvm` for JIT execution. Rust consumers
  call the runtime functions directly through normal Rust linking.
- **staticlib** (`libori_rt.a`): Linked into AOT-compiled binaries. The LLVM
  backend resolves external symbol references against this archive at link time.

Both are built by `cargo bl` (debug) or `cargo blr` (release). The crate has no
dependencies beyond the Rust standard library.

### Feature Flags

- **`single-threaded`**: Uses non-atomic refcount operations (`i64` reads/writes
  instead of `AtomicI64` with `fetch_add`/`fetch_sub`). Saves atomic operation
  overhead on programs that do not use task parallelism.

## Function Categories

### Memory Allocation

Low-level allocator wrappers that back all runtime allocations:

| Function | Purpose |
|----------|---------|
| `ori_alloc` | Allocate raw memory (minimum 8-byte alignment) |
| `ori_free` | Free memory allocated by `ori_alloc` |
| `ori_realloc` | Resize an allocation, preserving contents |

### Reference Counting (`rc/`)

The ARC lifecycle primitives. All RC-managed allocations use a 16-byte header
(`[data_size: i64 | strong_count: i64]`) placed before the data pointer. See
the [Reference Counting](./reference-counting.md) section for details.

| Function | Purpose |
|----------|---------|
| `ori_rc_alloc` | Allocate with RC header (initial count = 1) |
| `ori_rc_inc` | Increment refcount (Relaxed ordering) |
| `ori_rc_dec` | Decrement refcount; call drop function at zero |
| `ori_rc_free` | Unconditionally free an RC allocation |
| `ori_rc_is_unique` | Check RC == 1 (COW gate) |
| `ori_rc_is_unique_or_null` | Unique or null sentinel (COW helper) |
| `ori_rc_realloc` | Resize an RC allocation, preserving header |
| `ori_rc_data_size` | Read stored data size from header |
| `ori_rc_count` | Read current refcount (diagnostic) |
| `ori_rc_live_count` | Number of live RC allocations (leak detection) |
| `ori_buffer_rc_dec` | Decrement buffer RC with per-element cleanup |
| `ori_map_buffer_rc_dec` | Decrement map buffer RC with key/value cleanup |
| `ori_list_rc_inc` | Slice-aware RC increment for list buffers |
| `ori_memcpy_elements` | Bulk copy without RC (caller manages element RC) |
| `ori_memmove_elements` | Overlapping bulk copy (insert/remove shifting) |

### Collection Operations (`list/`, `map/`, `set/`)

Copy-on-write collection mutations. Every COW function follows consuming semantics:
it takes ownership of the caller's reference to the data buffer and produces a new
`{len, cap, data}` triple via an sret output pointer. See the
[Collections & COW](./collections-cow.md) section for details.

**List COW** (`list/cow.rs`, `list/cow_structural.rs`, `list/cow_sort.rs`):

| Function | Purpose |
|----------|---------|
| `ori_list_push_cow` | Append element (in-place if unique) |
| `ori_list_pop_cow` | Remove last element |
| `ori_list_set_cow` | Replace element at index |
| `ori_list_insert_cow` | Insert at index, shifting right |
| `ori_list_remove_cow` | Remove at index, shifting left |
| `ori_list_concat_cow` | Concatenate two lists (dual-consuming) |
| `ori_list_reverse_cow` | Reverse element order |
| `ori_list_sort_cow` | Unstable sort with comparator |
| `ori_list_sort_stable_cow` | Stable sort (TimSort) |

**List Slices** (`list/slice.rs`):

| Function | Purpose |
|----------|---------|
| `ori_list_slice` | Zero-copy view into existing buffer |
| `ori_list_slice_take` | First N elements as slice |
| `ori_list_slice_drop` | Skip N elements, rest as slice |
| `ori_list_materialize_slice` | Copy slice into standalone owned list |

**Map COW** (`map/cow.rs`):

| Function | Purpose |
|----------|---------|
| `ori_map_insert_cow` | Insert or update key-value pair |
| `ori_map_remove_cow` | Remove entry by key |
| `ori_map_update_cow` | Replace value for existing key |

**Set COW** (`set/cow.rs`):

| Function | Purpose |
|----------|---------|
| `ori_set_insert_cow` | Insert element (no-op if present) |
| `ori_set_remove_cow` | Remove element |
| `ori_set_union_cow` | Set union (consuming set1, borrowing set2) |
| `ori_set_intersection_cow` | Set intersection |
| `ori_set_difference_cow` | Set difference |

### String Operations (`string/`)

SSO-aware string handling. Strings <= 23 bytes are stored inline (no allocation).
See the [String SSO](./string-sso.md) section for details.

| Function | Purpose |
|----------|---------|
| `ori_str_concat` | COW-aware concatenation (SSO, in-place, or copy) |
| `ori_str_eq` / `ori_str_ne` | Equality / inequality |
| `ori_str_compare` | Lexicographic comparison (returns Ordering tag) |
| `ori_str_hash` | FNV-1a hash |
| `ori_str_len` / `ori_str_data` | SSO-safe length and data pointer |
| `ori_str_split` | Split by separator (seamless slices for long pieces) |
| `ori_str_substring` | Substring as seamless slice (heap) or copy (SSO) |
| `ori_str_contains` / `starts_with` / `ends_with` | String predicates |
| `ori_str_trim` | Whitespace trimming (seamless slice when possible) |
| `ori_str_to_uppercase` / `to_lowercase` | Case conversion (COW in-place for ASCII) |
| `ori_str_replace` | Pattern replacement (COW in-place for same-length) |
| `ori_str_repeat` | Repeat N times |
| `ori_str_push_char` | Append single character (COW protocol) |
| `ori_str_next_char` | UTF-8 codepoint decoding at byte offset |
| `ori_str_from_int` / `from_float` / `from_bool` / `from_raw` | Type conversions |

### Format Operations (`format/`)

Template string interpolation for `{value:spec}` expressions:

| Function | Purpose |
|----------|---------|
| `ori_format_int` | Format integer with spec (`b`, `o`, `x`, `X`, width, etc.) |
| `ori_format_float` | Format float (`e`, `E`, `f`, `%`, precision, etc.) |
| `ori_format_str` | Format string (width, alignment, precision truncation) |
| `ori_format_bool` | Format boolean |
| `ori_format_char` | Format character (Unicode codepoint) |

### Iterator Runtime (`iterator/`)

Opaque iterator handles manipulated through C-ABI functions. LLVM code never sees
the internal `IterState` enum -- all interaction is through pointer-sized handles.

**Constructors**: `ori_iter_from_list`, `ori_iter_from_range`, `ori_iter_from_str`,
`ori_iter_from_map`

**Adapters**: `ori_iter_map`, `ori_iter_filter`, `ori_iter_take`, `ori_iter_skip`,
`ori_iter_enumerate`, `ori_iter_zip`, `ori_iter_chain`

**Consumers**: `ori_iter_collect`, `ori_iter_count`, `ori_iter_any`, `ori_iter_all`,
`ori_iter_find`, `ori_iter_for_each`, `ori_iter_fold`

**Lifecycle**: `ori_iter_next` (advance), `ori_iter_drop` (free handle)

### I/O and Panic (`io.rs`)

| Function | Purpose |
|----------|---------|
| `ori_print` / `ori_print_int` / `ori_print_float` / `ori_print_bool` | Stdout output |
| `ori_panic` / `ori_panic_cstr` | Panic dispatch (user handler -> JIT longjmp -> unwind) |
| `ori_assert` / `ori_assert_eq_*` | Runtime assertions |
| `ori_catch_cleanup` / `ori_catch_recover` | Catch/recover for `catch(expr:)` |
| `ori_register_panic_handler` | Register user `@panic` trampoline |
| `ori_run_main` | Entry point wrapper with `catch_unwind` |
| `ori_args_from_argv` | Convert C argc/argv to Ori `[str]` list |

### Comparison Utilities

| Function | Purpose |
|----------|---------|
| `ori_compare_int` | Integer comparison (returns -1/0/1) |
| `ori_min_int` / `ori_max_int` | Integer min/max |

## C ABI Design Decisions

All runtime functions use `#[no_mangle] extern "C"` for FFI compatibility:

- **Data pointers, not struct pointers**: RC allocations return data pointers
  (past the 16-byte header), not header pointers. This lets LLVM code pass
  data pointers directly to C FFI without adjustment.

- **sret output pattern**: Functions returning structs larger than 16 bytes
  (lists, maps, sets as `{len, cap, data}`) write results through an
  `out_ptr` parameter rather than returning by value. This avoids ABI
  mismatches across platforms for aggregate return types.

- **Null sentinels for empty collections**: Empty lists, maps, and sets use
  null data pointers. `ori_rc_inc(null)` and `ori_rc_dec(null)` are no-ops,
  so empty collections require zero allocation and zero cleanup.

- **Function pointer callbacks**: COW operations accept `inc_fn` (element RC
  increment), `elem_dec_fn` (element RC decrement), `key_eq` (key equality),
  and comparator callbacks as C function pointers. The LLVM backend generates
  type-specialized trampolines for each concrete type.

- **Consuming semantics**: COW mutation functions take ownership of the
  caller's reference to the data buffer. The caller must not access the
  original buffer after the call. This enables the fast path (unique owner)
  to mutate in place without any copies.

## Link to LLVM Backend

The LLVM backend's `arc_emitter` module generates calls to runtime functions.
For each ARC operation (increment, decrement, COW mutation), the emitter:

1. Looks up the runtime function by name (e.g., `"ori_list_push_cow"`)
2. Declares it as an external symbol with the correct LLVM function type
3. Emits a `call` instruction with the appropriate arguments
4. For COW functions, passes the `inc_fn`/`dec_fn` trampolines that the
   codegen generates for the specific element type

The emitter generates type-specialized drop functions for each RC type
(structs, collections with RC children). These drop functions are passed to
`ori_rc_dec` as the `drop_fn` parameter, which calls them when the refcount
reaches zero.

## Debugging and Diagnostics

The runtime provides three environment-variable-controlled diagnostic modes:

- **`ORI_TRACE_RC=1`** (or `verbose`): Logs every `alloc/inc/dec/free` event to
  stderr. Verbose mode adds backtraces. Zero overhead when disabled (cached in
  `OnceLock`, single atomic load after first check).

- **`ORI_RT_DEBUG=1`**: Enables runtime assertions that validate RC headers on
  every operation. Catches use-after-free, double-free, and corruption. Debug
  builds also track freed pointers in a `HashSet` for double-free detection.

- **`ORI_CHECK_LEAKS=1`**: Counts live RC allocations. At program exit, reports
  the count of unfreed allocations. Debug builds additionally track allocation
  sites (pointer address, size, alignment) for attribution in the leak report.

These modes compose: `ORI_TRACE_RC=1 ORI_CHECK_LEAKS=1 ORI_RT_DEBUG=1 ./binary`
enables all three. Exit code 2 indicates a detected leak.
