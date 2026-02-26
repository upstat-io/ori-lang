# Value Semantics Optimization Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Runtime COW Foundation
**File:** `section-01-runtime-cow-foundation.md` | **Status:** Not Started

```
copy-on-write, COW, uniqueness check, is_unique, ori_rc_is_unique
refcount, reference count, RC==1, sole owner, unique owner
capacity, growth, doubling, 2x, amortized, realloc
empty sentinel, singleton, __empty_list, __empty_str
ori_rt, runtime primitives, ori_rc_alloc, ori_rc_inc, ori_rc_dec
runtime_decl, declare_runtime_functions, LLVM declarations
```

---

### Section 02: List COW Operations
**File:** `section-02-list-cow.md` | **Status:** Not Started

```
list, push, pop, append, prepend, insert, remove, set
ori_list_push, ori_list_pop, ori_list_set, ori_list_insert
ori_list_remove, ori_list_concat, ori_list_reverse, ori_list_sort
in-place mutation, mutate-in-place, fast path, slow path
OriList, len, cap, data, list capacity, list growth
vec, vector, dynamic array, contiguous array
arc_emitter, builtins/collections.rs, emit_list_push
```

---

### Section 03: String Optimization
**File:** `section-03-string-optimization.md` | **Status:** Not Started

```
string, str, SSO, small string optimization, inline string
OriStr, string layout, string capacity, string growth
ori_str_concat, ori_str_push_char, string builder
short string, 23-byte, inline storage, tagged pointer
string concatenation, string append, string COW
fat pointer, len, data, cow, Cow<str>
```

---

### Section 04: Map & Set COW Operations
**File:** `section-04-map-set-cow.md` | **Status:** Not Started

```
map, set, dictionary, hash map, hash set, hash table
ori_map_insert, ori_map_remove, ori_map_get
ori_set_insert, ori_set_remove, ori_set_union
ori_set_intersection, ori_set_difference
OriMap, OriSet, parallel arrays, keys, values
in-place insert, in-place remove, COW map, COW set
BTreeMap, ordered map, linear scan
```

---

### Section 05: Seamless Slices
**File:** `section-05-seamless-slices.md` | **Status:** Not Started

```
slice, seamless slice, zero-copy, view, borrow
list slice, string slice, substring, sublist
SEAMLESS_SLICE_BIT, flag bit, length encoding
shared backing, slice-aware RC, slice COW
ori_list_slice, ori_str_substring, ori_str_to_bytes
borrowing slice, consuming slice, slice mutation
offset, window, range, subrange
```

---

### Section 06: Interpreter COW Parity
**File:** `section-06-interpreter-parity.md` | **Status:** Not Started

```
interpreter, evaluator, ori_eval, Arc::make_mut
Heap<T>, make_mut, refcount check, Arc strong_count
behavioral equivalence, dual execution, JIT vs AOT
dispatch_list_method, dispatch_map_method, dispatch_set_method
Value::List, Value::Map, Value::Set, Value::Str
clone-on-modify, copy-on-write interpreter
```

---

### Section 07: Static Uniqueness Analysis
**File:** `section-07-static-uniqueness.md` | **Status:** Not Started

```
static analysis, uniqueness, ownership, linear, affine
uniqueness lattice, Unique, MaybeShared, Shared
intraprocedural, interprocedural, whole-program
borrow inference, ownership propagation, fixpoint
COW check elimination, branch elimination, dead branch
ori_arc, borrow/mod.rs, uniqueness/mod.rs
Lean 4, Koka PARC, Roc morphic, Swift SIL
```

---

### Section 08: Collection Memory Recycling
**File:** `section-08-collection-recycling.md` | **Status:** Not Started

```
reset/reuse, memory recycling, buffer reuse, allocation reuse
drop specialization, unique drop, shared drop, bulk free
collection drop, element cleanup, recursive dec
same-size reuse, cross-operation reuse, buffer pool
ori_arc, reset_reuse/mod.rs, drop_gen.rs
Lean 4 ExpandResetReuse, Koka ParcReuse, Roc reuse token
```

---

### Section 09: Verification & Benchmarks
**File:** `section-09-verification.md` | **Status:** Not Started

```
benchmark, performance, throughput, latency, memory
valgrind, memory safety, use-after-free, double-free, leak
dual execution, behavioral equivalence, JIT vs AOT
test matrix, correctness, regression, stress test
ORI_CHECK_LEAKS, valgrind-aot.sh, dual-exec-verify.sh
perf-baseline.sh, micro-benchmark, macro-benchmark
push benchmark, concat benchmark, slice benchmark
code journey, pipeline integration, end-to-end, differential testing
phase boundary, progressive complexity, eval-vs-LLVM divergence
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Runtime COW Foundation | `section-01-runtime-cow-foundation.md` |
| 02 | List COW Operations | `section-02-list-cow.md` |
| 03 | String Optimization | `section-03-string-optimization.md` |
| 04 | Map & Set COW Operations | `section-04-map-set-cow.md` |
| 05 | Seamless Slices | `section-05-seamless-slices.md` |
| 06 | Interpreter COW Parity | `section-06-interpreter-parity.md` |
| 07 | Static Uniqueness Analysis | `section-07-static-uniqueness.md` |
| 08 | Collection Memory Recycling | `section-08-collection-recycling.md` |
| 09 | Verification & Benchmarks | `section-09-verification.md` |
