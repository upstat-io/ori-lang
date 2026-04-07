---
reroute: true
name: "Repr Opt"
full_name: "Representation Optimization & ARC Intelligence"
status: active
reviewed: false
order: 1
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

**When to benchmark:** §01 (pipeline integration), §04 (integer narrowing codegen), §06 (struct layout), §08 (escape analysis), §11 (SSO audit / SVO / packed collections)
**Skip benchmarks for:** §02 (triviality — correctness only), §03 (range analysis — analysis time, not runtime), §12 (verification itself)

---

## Keyword Clusters by Section

### Section 01: Representation IR & Decision Framework
**File:** `section-01-repr-ir.md` | **Status:** Complete

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
--no-repr-opt flag, ORI_NO_REPR_OPT, NarrowingPolicy, Aggressive Conservative Disabled
ori_repr workspace registration, Cargo.toml members
set_var_ranges, function_var_ranges, var_range, ArcVarId ValueRange
float_width, int_width, is_trivial, escapes, rc_strategy, RcStrategy
set_escape_info, set_rc_strategy, EscapeInfo placeholder, ValueRange placeholder
FieldRepr name field, debug symbols, C-ABI reorder verification
compute_repr_plan arc_functions pool policy, pass stubs, stub functions
range/mod.rs, escape/mod.rs, placeholder module, immediate compilation
ori_repr tracing, ORI_LOG=ori_repr, tracing_setup
```

---

### Section 02: Transitive Triviality & ARC Elision
**File:** `section-02-transitive-triviality.md` | **Status:** Complete

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
analyze_triviality, validation pass, is_trivial_repr, canonical consistency
§01.8 Phase B, TypeInfoStore delegation, classify_trivial removal
§08 feed-forward, §09 feed-forward, RcStrategy::None
```

---

### Section 03: Value Range Analysis Framework
**File:** `section-03-range-analysis.md` | **Status:** Complete

```
ValueRange, interval, range analysis, VRP, value range propagation
abstract interpretation, lattice, join, meet, widen, narrow
transfer function, fixed point, fixpoint iteration, range_fixpoint
TransferContext, var_types, field_summaries
conditional refinement, branch condition, if x < N
BranchRefinement, refine_from_branch, conditional.rs
block parameters, phi handling, predecessor map, compute_predecessors
ArcTerminator, Invoke dst, Switch u64 cases, Branch refinement
RangeAnalysisConfig, max_iterations, max_blocks, WIDEN_THRESHOLD
compute_rpo, compute_postorder, successor_block_ids, RPO
FieldSummaryTable, field_summary, observe_construct, field_range
field_range_summaries, join_field_range, flush_to_repr_plan
RangeFixpointResult, return_range, var_ranges
FunctionRangeInfo, ParamRange, call-site range, signatures.rs
SCC, CallGraph, compute_sccs, interprocedural fixpoint
ReprPlan handoff, function_var_ranges, var_range query
CorrelatedValuePropagation, LazyValueInfo, tree-vrp
Roc NumericRange, LLVM VRP, GCC VRP
range_add, range_sub, range_mul, range_literal, transfer_primop
```

---

### Section 04: Integer Narrowing Pipeline
**File:** `section-04-integer-narrowing.md` | **Status:** Complete

```
integer narrowing, int → i32, int → i16, int → i8
IntWidth, width selection, min_width, ValueRange::min_width
ABI boundary, widening, sext, trunc, sign extension, AbiBoundary
overflow guard, can_overflow, BinaryOp, checked arithmetic, overflow detection
NarrowingPolicy, aggressive, conservative, disabled
--no-repr-opt, ORI_NO_REPR_OPT
loop counter narrowing, struct field narrowing, collection element narrowing
Phase A, Phase B, Phase C
apply_integer_narrowing, FieldSummaryTable, element_store_size
TypeLayoutResolver, try_repr_to_llvm_type, ArcIrEmitter
§04/§06 interface contract, FieldRepr.repr, FieldRepr.offset placeholder
narrowing/int.rs, narrowing/abi.rs, narrowing/overflow.rs
Zig comptime_int, Roc NumericRange, LLVM InstCombine
```

---

### Section 05: Float Narrowing Pipeline
**File:** `section-05-float-narrowing.md` | **Status:** Complete

```
float narrowing, f64 → f32, FloatWidth, precision
is_f32_exact, fpext, fptrunc, precision loss
FloatRange, F32Exact, Bottom, Top, observe, observe_arithmetic
FloatFieldSummaryTable, collect_float_field_summaries
storage-only narrowing, Phase A, literal-only
narrow_float_fields, narrowing/float.rs
IEEE 754, double to float, f32 representable, roundtrip
NaN, infinity, negative zero, subnormal, out-of-range
MachineRepr::Float { width: F32 }, try_repr_to_llvm_type
ArcIrEmitter, Construct fptrunc, Project fpext
trunc_for_narrowed_struct, sext_narrowed_field, float_trunc, float_ext
try_lower_narrowed_aggregate, all-scalar-primitives guard
DecisionSource::FloatNarrowing, has_fixed_layout_attr
combined int+float narrowing, §04 codegen extension
§05→§07 handoff, f32 niche skip, NaN-based niche
NarrowingPolicy, Disabled, Conservative, Aggressive
apply_float_narrowing, arc_functions parameter
find_literal_float_value, narrowing_codegen.rs, float/tests.rs
is_narrowing_safe_for_codegen rename, float_field_range_summary_string
```

---

### Section 06: Struct & Tuple Layout Optimization
**File:** `section-06-struct-layout.md` | **Status:** Complete

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
**File:** `section-07-enum-repr.md` | **Status:** In Progress (§07.0 complete, §07.1 complete, §07.2 in progress)

```
niche, niche filling, niche optimization, tagged pointer
EnumRepr, EnumTag, Niche, discriminant narrowing
Option<bool> 1 byte, Option<&T> null niche, Option<str> null ptr niche
Option<[int]> no niche (empty list = null ptr), FatRepr::Str niche only
invalid bit pattern, spare bits, niche_value
tagged pointer, low bits, alignment bits
payload compression, variant layout, all-unit tag narrowing
codegen consumer inventory, TagAccess abstraction, tag_access.rs
resolve_enum, emit_construct, SetTag, Switch, drop_enum, rc_helpers
abi/mod.rs stale comments, abi_size_inner, consumer #16
Rust niche, Swift GenEnum, Zig optional
f32 niche, float niche, NaN bit pattern, f32-typed field niche
depends §04 integer narrowing, depends §05 float narrowing
current tag always i64 (not i8), discriminant narrowing i64→i8
discriminant narrowing BEFORE niche filling, TagAccess gate
pattern match niche, niche-aware RC, niche-aware drop
find_enum_niches, nested niche, Option<Option<bool>>
evaluator unaffected, Value::Variant, dual-execution parity
niche.rs, tagged_ptr.rs, layout submodules
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
SSO, small string optimization, inline string, 23 byte
SVO, small vector optimization, inline vector, SmallVec
packed bool, bit packing, 1 bit per bool, PackedBoolArray
narrow element, backing store, [i8], [i16], [f32]
ori_str_len, ori_str_data, OriStr, OriStr::is_sso, SSO_FLAG, SLICE_FLAG
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
