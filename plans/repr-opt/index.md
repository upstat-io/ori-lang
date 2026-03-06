---
reroute: true
name: "Repr Opt"
full_name: "Representation Optimization & ARC Intelligence"
status: queued
order: 2
---

# Representation Optimization & ARC Intelligence Index

> **Maintenance Notice:** Update this index when adding/modifying sections.
> **Implements:** `docs/ori_lang/proposals/approved/representation-optimization-proposal.md`

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Performance Validation

Use `/benchmark short` after modifying hot paths.

**When to benchmark:** §01 (pipeline integration), §04 (integer narrowing codegen), §06 (struct layout), §08 (escape analysis), §11 (SSO/SVO)
**Skip benchmarks for:** §02 (triviality — correctness only), §03 (range analysis — analysis time, not runtime), §12 (verification itself)

---

## Keyword Clusters by Section

### Section 01: Representation IR & Decision Framework
**File:** `section-01-repr-ir.md` | **Status:** Not Started

```
MachineRepr, ReprPlan, ReprDecision, DecisionSource, DecisionReason
IntWidth, FloatWidth, StructRepr, EnumRepr, TupleRepr, RcRepr
FatRepr, ClosureRepr, VariantRepr, FieldRepr, ReprAttribute, RcStrategy
ori_repr crate, representation plan, narrowing decision
TypeLayoutResolver, TypeInfo, storage_type, TypeInfoStore
canonical representation, machine representation, semantic contract
Char, Byte, Duration, Size, Ordering, Range, FatPointer, OpaquePtr
generic type, monomorphization, type variable, resolve_fully
Salsa integration, incremental, invalidation, JIT hot-reload
#repr("c"), #repr("packed"), #repr("transparent"), #repr("aligned", N)
migration, TypeInfoStore → ReprPlan, Phase A/B/C
Lean4 LCNF, Zig InternPool, Roc STLayoutInterner
```

---

### Section 02: Transitive Triviality & ARC Elision
**File:** `section-02-transitive-triviality.md` | **Status:** Not Started

```
Triviality, trivial, non-trivial, ARC elision, RC elision
ArcClassifier, ArcClass, Scalar, DefiniteRef, PossibleRef
is_trivial, classify_triviality, classify_recursive, transitive walk
triviality/mod.rs, triviality/tests.rs, directory module
Option<int>, (int, float), Result<int, Ordering>, struct Point
drop function elision, no RC, zero overhead, compute_drop_info
ori_arc::ArcClassifier, ori_types::triviality, TypeInfoStore::is_trivial
newtype, UserId, Named, resolve_fully, TypeKind::Newtype
CPtr, JsValue, c_int, FFI type, opaque pointer
generic type, monomorphization, Pair<T>, type variable
Salsa, caching, RefCell, FxHashMap, incremental
ori_eval, evaluator, not affected
merge_triviality, cycle detection, FxHashSet, visiting
```

---

### Section 03: Value Range Analysis Framework
**File:** `section-03-range-analysis.md` | **Status:** Not Started

```
ValueRange, interval, range analysis, VRP, value range propagation
abstract interpretation, lattice, join, meet, widen, narrow
transfer function, fixed point, fixpoint iteration
conditional refinement, branch condition, if x < N
CorrelatedValuePropagation, LazyValueInfo, tree-vrp
Roc NumericRange, LLVM VRP, GCC VRP
range_add, range_sub, range_mul, range_literal
```

---

### Section 04: Integer Narrowing Pipeline
**File:** `section-04-integer-narrowing.md` | **Status:** Not Started

```
integer narrowing, int → i32, int → i16, int → i8
select_int_width, IntWidth, width selection
ABI boundary, widening, sext, trunc, sign extension
overflow guard, checked arithmetic, overflow detection
NarrowingPolicy, aggressive, conservative, disabled
loop counter narrowing, struct field narrowing
Zig comptime_int, Roc NumericRange, LLVM InstCombine
```

---

### Section 05: Float Narrowing Pipeline
**File:** `section-05-float-narrowing.md` | **Status:** Not Started

```
float narrowing, f64 → f32, FloatWidth, precision
is_f32_exact, fpext, fptrunc, precision loss
FloatRange, F32Exact, integer-valued float
storage-only narrowing, computation narrowing
IEEE 754, double to float, f32 representable
```

---

### Section 06: Struct & Tuple Layout Optimization
**File:** `section-06-struct-layout.md` | **Status:** Not Started

```
struct layout, field reordering, padding minimization
StructRepr, FieldRepr, alignment, offset, padding
sort by alignment, descending alignment, descending size
#repr("c"), #repr("packed"), #repr("transparent"), #repr("aligned", N), ABI stable, FFI interop
ReprAttribute, Default, C, Packed, Transparent, Aligned
tuple layout, anonymous struct, optimize_tuple_layout
Rust repr(Rust), Zig struct layout, LLVM DataLayout
```

---

### Section 07: Enum Representation Optimization
**File:** `section-07-enum-repr.md` | **Status:** Not Started

```
niche, niche filling, niche optimization, tagged pointer
EnumRepr, EnumTag, Niche, discriminant narrowing
Option<bool> 1 byte, Option<&T> null niche
invalid bit pattern, spare bits, niche_value
tagged pointer, low bits, alignment bits
payload compression, variant layout, shared prefix
Rust niche, Swift GenEnum, Zig optional
```

---

### Section 08: Escape Analysis & Stack Promotion
**File:** `section-08-escape-analysis.md` | **Status:** Not Started

```
escape analysis, stack promotion, alloca, heap to stack
EscapeState, NoEscape, ArgEscape, GlobalEscape
connection graph, CgNode, CgEdge, PointsTo, Deferred
intraprocedural, interprocedural, function summary
FunctionEscapeSummary, param_escapes, return_aliases
stack allocation, no RC header, eliminate ori_rc_alloc
bump allocation, bump allocator, arena, AllocStrategy
ori_bump_alloc, ori_bump_free, dynamic-size, non-escaping
Go escape analysis, Swift StackPromotion, Java scalar replacement
Lean4 Borrow.lean, ori_arc borrow inference
```

---

### Section 09: ARC Header Compression
**File:** `section-09-arc-header.md` | **Status:** Not Started

```
ARC header, RC header, refcount width, header compression
SharingBound, Unique, Bounded, Unbounded
i8 refcount, i16 refcount, i32 refcount, i64 refcount
ori_rc_alloc_i8, ori_rc_inc_i8, ori_rc_dec_i8
immortal, overflow, promote to immortal
sharing analysis, max references, refcount bound
Swift RefCount.h, Lean4 object.h, CPython
```

---

### Section 10: Thread-Local Non-Atomic ARC
**File:** `section-10-thread-local-arc.md` | **Status:** Not Started

```
thread local, non-atomic, atomic, plain load, plain store
ThreadLocality, ThreadLocal, ThreadShared
ori_rc_inc_nonatomic, ori_rc_dec_nonatomic
thread escape, spawn, channel, send
Rc vs Arc, non-atomic refcount, cache line
migration fence, static migration, dynamic migration
Rust Rc/Arc, Swift isUniquelyReferenced, CPython GIL
```

---

### Section 11: Collection Specialization
**File:** `section-11-collection-spec.md` | **Status:** Not Started

```
SSO, small string optimization, inline string, 22 byte
SVO, small vector optimization, inline vector, SmallVec
packed bool, bit packing, 1 bit per bool, PackedBoolArray
narrow element, backing store, [i8], [i16], [f32]
ori_str_new, ori_str_is_inline, OriStr
collection narrowing, element narrowing, map key narrowing
C++ basic_string, Rust SmallVec, Swift Array COW
```

---

### Section 12: Verification & Benchmarks
**File:** `section-12-verification.md` | **Status:** Not Started

```
verification, dual execution, semantic equivalence
Valgrind, AddressSanitizer, memory safety, use-after-free
benchmark, perf-baseline, perf-compare, speedup
test matrix, regression, correctness, stress test
code journey, dual-exec-verify, --no-repr-opt
compile time, binary size, runtime, peak RSS, RC count
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Representation IR & Decision Framework | `section-01-repr-ir.md` |
| 02 | Transitive Triviality & ARC Elision | `section-02-transitive-triviality.md` |
| 03 | Value Range Analysis Framework | `section-03-range-analysis.md` |
| 04 | Integer Narrowing Pipeline | `section-04-integer-narrowing.md` |
| 05 | Float Narrowing Pipeline | `section-05-float-narrowing.md` |
| 06 | Struct & Tuple Layout Optimization | `section-06-struct-layout.md` |
| 07 | Enum Representation Optimization | `section-07-enum-repr.md` |
| 08 | Escape Analysis & Stack Promotion | `section-08-escape-analysis.md` |
| 09 | ARC Header Compression | `section-09-arc-header.md` |
| 10 | Thread-Local Non-Atomic ARC | `section-10-thread-local-arc.md` |
| 11 | Collection Specialization | `section-11-collection-spec.md` |
| 12 | Verification & Benchmarks | `section-12-verification.md` |
