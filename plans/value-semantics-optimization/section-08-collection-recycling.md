---
section: "08"
title: "Collection Memory Recycling"
status: complete
goal: "Dead collection buffers are recycled for new allocations, reducing malloc/free pressure"
inspired_by:
  - "Lean 4 IR/ExpandResetReuse.lean — conditional memory reuse for constructors"
  - "Koka Backend/C/ParcReuse.hs — Available map tracks reusable slots by size"
  - "Koka Backend/C/ParcReuseSpec.hs — field-level specialization (skip unchanged fields)"
depends_on: ["07"]
sections:
  - id: "08.1"
    title: "Collection Literal Buffer Reuse"
    status: complete
  - id: "08.2"
    title: "Buffer Recycling Strategy Decision"
    status: complete
  - id: "08.3"
    title: "Drop Specialization for Unique Collections"
    status: complete
  - id: "08.4"
    title: "Cross-Operation Buffer Reuse"
    status: complete
  - id: "08.5"
    title: "Completion Checklist"
    status: complete
---

# Section 08: Collection Memory Recycling

**Status:** Complete
**Goal:** When a collection is about to be freed (RC reaches 0) and a same-type collection is about to be allocated, the dead collection's buffer is reused directly — no malloc/free roundtrip. Additionally, provably unique collection drops skip atomic RC operations entirely.

**Context:** The existing reset/reuse optimization in `ori_arc` works for constructors (structs, enums) but explicitly excludes collection buffers (`is_collection_ctor` gate). Collection reuse requires different mechanics: buffer manipulation, element cleanup, and capacity checks — unlike struct reuse which uses per-field `Set` mutations.

**Reference implementations:**
- **Lean 4** `ExpandResetReuse.lean`: `reset x` → `if isShared(x) { slow } else { reuse x's memory }`. The reuse token is threaded to the allocation site.
- **Koka** `ParcReuse.hs`: Tracks an `Available` map of freed buffers by size. New allocations check the map before calling `malloc`. `ParcReuseSpec.hs` specializes field writes (only write changed fields).

**Depends on:** Section 07 (static uniqueness analysis provides the foundation for knowing when values are consumed).

---

## 08.1 Collection Literal Buffer Reuse

**Status:** Complete

When `RcDec(old_list)` is followed by `Construct(ListLiteral, [new_elems])` of the same element type in the same block, the old buffer can be reused instead of freeing + allocating.

### New ARC IR instruction: `CollectionReuse`

**File:** `compiler/ori_arc/src/ir/instr.rs`

Unlike struct reuse (which uses `Reset`/`Reuse` → expansion to `IsShared` + `Set` mutations), collection reuse is a single self-contained instruction that delegates to a runtime helper:

```rust
CollectionReuse {
    old_var: ArcVarId,      // collection being recycled
    dst: ArcVarId,          // destination variable
    ty: Idx,                // collection type
    ctor: CtorKind,         // ListLiteral or SetLiteral
    args: Vec<ArcVarId>,    // new element values
}
```

**Rationale:** Collection buffer manipulation (element cleanup, capacity check, potential realloc) is too complex to inline as ARC IR — a runtime call is appropriate. The runtime function handles both the unique (reuse) and shared (alloc fresh) paths internally.

### Runtime function: `ori_list_reset_buffer`

**File:** `compiler/ori_rt/src/list/reset/mod.rs`

```rust
pub extern "C" fn ori_list_reset_buffer(
    old_data: *mut u8, old_len: i64, old_cap: i64,
    new_len: i64, elem_size: i64,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
    out_cap: *mut i64,
) -> *mut u8
```

1. If unique (`ori_rc_is_unique`): dec old elements, reuse/realloc buffer
2. If shared: dec old buffer normally, alloc fresh buffer
3. Seamless slices: fall back to dec + alloc (slice cap encoding is not a real capacity)
4. Write result capacity to `*out_cap`

### Detection changes

**File:** `compiler/ori_arc/src/reset_reuse/mod.rs`

- `ListLiteral` and `SetLiteral` matches emit `CollectionReuse` directly (not `Reset`/`Reuse`)
- `MapLiteral` excluded (dual-region layout — defer)
- Struct/enum matches continue through the existing `Reset`/`Reuse` → expansion pipeline
- Extracted `apply_struct_reuse()` and `apply_collection_reuse()` helpers

### Pipeline integration

**Files updated for `CollectionReuse` exhaustive matches:**
- `compiler/ori_arc/src/uniqueness/intra/mod.rs` — marks `dst` as `Unique`
- `compiler/ori_arc/src/borrow/mod.rs` — marks `old_var` and `args` as requiring ownership
- `compiler/ori_arc/src/borrow/derived.rs` — marks `dst` as `Fresh`
- `compiler/oric/src/arc_dump/instr.rs` — formatting

### LLVM emission

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/construction.rs`

1. Load old `{len, cap, data}` from `old_var`
2. Get element dec function via `get_or_generate_elem_dec_fn`
3. Alloca `out_cap` on stack
4. Call `ori_list_reset_buffer` → new data pointer
5. GEP+store each new element
6. Load `out_cap`, build result `{new_len, out_cap, new_data_ptr}`
7. Bind to `dst`

### Tests

- Runtime: `ori_list_reset_buffer` reuses unique buffers, allocs for shared/null
- Pipeline: all 10,885 tests pass
- Valgrind: zero memory errors across all test programs

---

## 08.2 Buffer Recycling Strategy Decision

**Status:** Complete (decision documented)

### Decision: Static Detection Only (No Runtime Buffer Pool)

We use compile-time pattern matching only. No runtime buffer pool.

**Rationale:**
- Static detection covers the most common patterns: literal-to-literal replacement within the same function
- A runtime pool adds: per-drop overhead (pool lookup/insertion), memory retention (buffers held speculatively), thread-safety complexity (per-thread pools or locking), and non-deterministic memory behavior
- The ARC pipeline already has the infrastructure for static pattern detection (reset/reuse framework)

**Revisit conditions:**
- Profiling shows >10% time in `malloc`/`free` for collection workloads that static detection cannot cover
- Cross-function reuse patterns become common (e.g., producer/consumer across call boundaries)

---

## 08.3 Drop Specialization for Unique Collections

**Status:** Complete

When a collection is provably unique (RC == 1) at its `RcDec` point, skip the atomic RC decrement and directly call element cleanup + buffer free. Saves one `atomic::fetch_sub` + `Ordering::Acquire` fence per unique collection drop.

### Drop hints analysis

**File:** `compiler/ori_arc/src/uniqueness/drop_hints/mod.rs`

A lightweight post-pipeline pass that identifies `RcDec` instructions where the variable is provably unique. Conservative heuristic (O(n) per function):

A variable `v` at `RcDec { var: v }` is **unique-droppable** if:
1. `v` is defined by `Construct` (fresh allocation, RC starts at 1) in the same function
2. No `RcInc { var: v }` exists anywhere in the function (never shared)
3. `v`'s type is a collection (list/set/map)

Result stored as `drop_hints: DropHints` on `ArcFunction` — keyed by `(block_idx, instr_idx)`, following the `CowAnnotations` pattern.

### Runtime functions

**File:** `compiler/ori_rt/src/rc/collections.rs`

```rust
// List/Set: skip atomic RC dec, directly clean elements + free buffer
pub extern "C" fn ori_buffer_drop_unique(
    data: *mut u8, len: i64, cap: i64, elem_size: i64,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
)

// Map: skip atomic RC dec, clean keys + values + free buffer
pub extern "C" fn ori_map_buffer_drop_unique(
    data: *mut u8, cap: i64, len: i64, key_size: i64, val_size: i64,
    key_dec_fn: Option<extern "C" fn(*mut u8)>,
    val_dec_fn: Option<extern "C" fn(*mut u8)>,
)
```

### LLVM emission

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/rc_ops.rs`

When emitting `RcDec` for a collection with `HeapPointer` strategy, check `drop_hints`. If unique, call `ori_buffer_drop_unique` / `ori_map_buffer_drop_unique` instead of the standard `ori_buffer_rc_dec` / `ori_map_buffer_rc_dec`.

### Tests

- ARC: hint computation (fresh local → hint, parameter → no hint, inc'd → no hint)
- Runtime: `ori_buffer_drop_unique` correctness
- AOT: unique drop of list-of-strings
- Valgrind: clean on unique drop paths

---

## 08.4 Cross-Operation Buffer Reuse

**Status:** Complete (analysis documented)

### Already Handled by COW (Sections 02-07)

These operations already reuse buffers in-place when the collection is unique, via the COW infrastructure:

| Operation | Runtime Function | Behavior When Unique |
|-----------|-----------------|---------------------|
| `sort` | `ori_list_sort_cow` | Mutates in-place |
| `reverse` | `ori_list_reverse_cow` | Mutates in-place |
| `push` | `ori_list_push_cow` | Appends in-place (may grow) |
| `insert` | `ori_list_insert_cow` | Shifts + inserts in-place |
| `remove` | `ori_list_remove_cow` | Shifts in-place |
| `concat` | `ori_list_concat_cow` | Extends in-place |
| `map[k] = v` | `ori_map_insert_cow` | Inserts in-place |
| `set.add(v)` | `ori_set_insert_cow` | Inserts in-place |

With Section 07's static uniqueness analysis, the runtime uniqueness check is eliminated for provably unique values (`CowMode::StaticUnique`).

### Deferred: Iterator Chain Fusion

These patterns require **iterator fusion** — a fundamentally different optimization:

| Pattern | Why Deferred |
|---------|-------------|
| `list.map(f)` | Allocation happens inside `collect()` as an `Apply` call, not a `Construct(ListLiteral)`. Buffer reuse requires detecting that the `Apply` is a collect that could reuse the iterator source's buffer. |
| `list.filter(f)` | Same as `map` — the allocation is in `collect()`. |
| `list.map(f).filter(g).collect()` | Requires fusing the entire iterator chain into a single pass that writes into the source buffer. |

**Prerequisites for iterator fusion:**
- Inline `collect()` at the ARC IR level to expose the allocation
- Track buffer provenance through iterator adapters
- Prove element type compatibility across transformations

This is a separate optimization pass, not an extension of reset/reuse.

---

## 08.5 Completion Checklist

- [x] Drop specialization: unique collection drops skip atomic RC dec
- [x] Drop hints computed correctly (fresh-local only, no false positives)
- [x] Collection literal buffer reuse via `CollectionReuse` instruction
- [x] `ori_list_reset_buffer` handles unique (reuse) and shared (alloc) paths
- [x] LLVM emission for `CollectionReuse` produces correct code
- [x] Valgrind clean on all recycling test programs (7/7 pass)
- [x] `./test-all.sh` — all 10,885 tests pass
- [x] `./clippy-all.sh` — no warnings
- [x] `diagnostics/dual-exec-verify.sh` — pre-existing mismatches only (LLVM warning banner)
- [x] Runtime tests: `ori_list_reset_buffer`, `ori_buffer_drop_unique`, `ori_map_buffer_drop_unique`
- [x] `./scripts/perf-baseline.sh` — baseline recorded (2026-03-01): bench_hello 154ms/15K, bench_small 254ms/4613K AOT 27x, bench_medium 366ms/6587K AOT 72x

**Exit Criteria (met):** Drop of a provably unique list emits `ori_buffer_drop_unique` (no atomic RC dec). Collection literal replacement in same function reuses buffer when types match. Valgrind reports zero errors on all recycling test programs.
