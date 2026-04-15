---
paths:
  - "compiler/ori_llvm/**"
  - "compiler/ori_arc/**"
  - "compiler/ori_rt/**"
  - "compiler/ori_types/**"
  - "compiler/ori_repr/**"
  - "docs/ori_lang/v2026/spec/26-ffi.md"
  - "docs/ori_lang/v2026/spec/annex-e-system-considerations.md"
  - "tests/spec/repr/**"
---

# repr.md — Representation Layer SSOT

**Purpose.** repr.md is the spec-driven SSOT for the compiler's **representation layer** — the sub-phase inside LLVM codegen that converts the type checker's logical types into the physical layout data consumed by LLVM emission. It owns: the `ReprPlan` artifact (per-type layout summary), the integer-narrowing scope policy, the niche-encoding policy, the discriminant-width rule, the `#repr(...)` attribute layout semantics, and the RC-header offset contract that codegen uses to locate `strong_count`.

**Scope.** repr.md is a peer of `parse.md`, `typeck.md`, `types.md`, `aims-rules.md`, `codegen-rules.md`, `arc.md`, and `impl-hygiene.md`, navigated by `canon.md`. Every physical LLVM mapping, ABI classification, RC-header field schema, and emission mechanic is owned by `codegen-rules.md` (with `runtime.md` for `ori_rt` internals); repr.md cites those rules, it does not restate them. The layout POLICY (field reordering, narrowing soundness, niche availability, `#repr` semantics) lives here; the layout MAPPING (Idx → LLVM type, field offsets in emitted IR, RT-2 header schema) lives in `codegen-rules.md`. Spec authority is `docs/ori_lang/v2026/spec/`.

**Spec.** `docs/ori_lang/v2026/spec/` is authoritative for surface semantics. Relevant clauses: Clause 8.1 Primitive Types (with 8.1.1 Never Semantics, 8.1.2 Duration, 8.1.3 Size), Clause 8.2 Compound Types (List, Fixed-Capacity List, Map, Set, Tuple, Function, Range), Clause 8.4 Built-in Types (Ordering), Clause 8.6 User-Defined Types (Struct, Sum Type, Newtype, Derive), Clause 8.8 Trait Objects, Clause 21 Memory Model, Clause 26.4.9 `#repr` attribute (`docs/ori_lang/v2026/spec/26-ffi.md`), and Annex E §Representation Optimization, §ARC Runtime, §Built-in Type Representations (`docs/ori_lang/v2026/spec/annex-e-system-considerations.md`). The `#repr(...)` attribute surface syntax is summarized in `.claude/rules/ori-syntax.md §FFI` and specified in Clause 26.4.9.

**Shipped vs stubbed passes.** The `ori_repr` pipeline has two active passes (struct layout/field reordering, integer narrowing) and five **stubbed** passes with empty function bodies: `compute_enum_reprs` (enum niche optimization), `analyze_escape` (escape analysis for stack promotion), `compress_arc_headers` (RC header width narrowing), `apply_thread_local_arc` (Rc vs Arc selection), `specialize_collections` (SSO, SVO, packed bool, element narrowing). Rules in this document that reference stubbed passes describe the target-system behavior — the code does not yet implement them. The active passes are authoritative current behavior.

---

## Notation

- **SHALL** = mandatory requirement (violation is an implementation bug).
- **SHALL NOT** = mandatory prohibition.
- **SHOULD** = recommended practice; deviation requires justification.
- **MAY** = permitted behavior.
- **canonical width** = the LLVM primitive width produced by `CG:TR-1` for a given Ori type, without narrowing applied (e.g., `int` → `i64`).
- **narrowed width** = an LLVM primitive width smaller than canonical, produced by representation-optimization analysis when value-range analysis proves it sound.
- **storage site** = a program point that reads or writes a physical byte in a collection backing buffer, a struct field, or a local variable; these are the ONLY sites where narrowing is observable.
- **niche** = a bit pattern of a type `T` that is provably never produced by any valid value of `T`; niche availability enables tagless sum-type encoding.
- **ReprPlan** = the per-type layout summary produced by the representation layer and consumed by codegen (§2).
- Rules are numbered `CATEGORY-N`. Categories used in this file: `RP` (ReprPlan + layout fundamentals), `RN` (narrowing scope policy), `NI` (niche encoding policy), `RH` (RC header reads from the codegen header schema), `RV` (cross-phase invariants + verification).
- Cross-references: `codegen-rules.md` rules prefixed with `CG:` (e.g., `CG:TR-1`, `CG:AB-1`, `CG:NR-5`, `CG:RT-2`); `aims-rules.md` rules with `AIMS:` (e.g., `AIMS:L-9`, `aims-rules.md §1.6`); `typeck.md` with `CHK:`; `types.md` with `TYPES:`; `parse.md` with `PARSE:`; `impl-hygiene.md` with `HYG:`; `canon.md` with `CANON:` (e.g., `CANON:§4.6`); spec clauses with `Spec:` (e.g., `Spec: Clause 8.8`, `Spec: Annex E §Representation Optimization`).

---

## §1 Pipeline Position

Representation is NOT a distinct phase in `CANON:§1`. It is the sub-layer inside phase 8 (LLVM codegen, owner crate `ori_llvm`) that executes immediately after ARC realization (phase 7) and immediately before LLVM IR emission. The crossing point is the single source of truth on which side of the boundary types are logical versus physical:

| Phase | Type model | Characteristic operations |
|-------|-----------|--------------------------|
| 2 Parse | Syntactic only — no type info | Tokens → AST (`parse.md`) |
| 3 Type check | Logical/semantic (`Idx`, `Tag`) | HM inference, trait dispatch, capability checking (`typeck.md`, `types.md`) |
| 4 Canonicalize | Logical `Idx` on `CanExpr` | Desugar elimination, constant folding, pattern-tree compilation (`ori_canon`, `CANON:§4.3`) |
| 5 AIMS analysis | Logical `Idx` + lattice tuple on `CanExpr` + ARC IR | Access × Consumption × Cardinality × Uniqueness × Locality × Shape × Effect (`aims-rules.md §§1.1–1.7`) |
| 6 ARC realization | Logical `Idx` + realized RC/COW/reuse/drop | IR with `RcInc`/`RcDec`/`IsShared`/`Reuse`/`Drop` instructions (`aims-rules.md §8`) |
| **7a Repr** | Logical `Idx` → physical layout summary | **This file.** `ReprPlan` computation, narrowing scope, niche selection, field reordering, `#repr` application |
| 7b LLVM emission | Physical LLVM type + IR | `codegen-rules.md §1 TR-*` (type mapping) + §§2–8 (ABI, narrowing, trampolines, RC, runtime, attributes, verification) |
| 8 Optimize & emit | Physical LLVM IR | `aot.md` |

Phase purity (`CANON:§5`, `compiler.md §Phase-Specific Purity`) applies: repr consumes realized ARC IR (post phase 7), the type pool (`TYPES:§1`), and the converged AIMS state (`CANON:§4.4`); it produces `ReprPlan` entries keyed by `Idx` and a narrowing map keyed by SSA variable. Repr SHALL NOT re-type-check, re-canonicalize, re-run AIMS, or emit LLVM instructions itself — it prepares layout data that `codegen-rules.md` rules consume.

---

## §2 The `ReprPlan` Artifact

### RP-1 — Definition

`ReprPlan` is the per-type layout summary produced by the representation layer for every fully-resolved type index `Idx` (`CG:TR-2`). It is a pure data structure — no LLVM handles, no process-bound state — so it can participate in Salsa caching without losing determinism (`RP-3`, `RP-4`).

```
ReprPlan {
    abi_size:      u64,                  // total size in bytes after padding, PAYLOAD ONLY (no RC header)
    abi_alignment: u64,                  // required alignment in bytes, power of two
    repr_kind:     ReprKind,             // Default | C | Packed | Transparent | Aligned(u64)
    layout:        LayoutTree,           // see below — stable, hashable layout descriptor
    niche:         Option<NicheInfo>,    // niche availability for sum-type encoding (§7)
    origin:        Idx,                  // the type pool index this plan describes
}

enum LayoutTree {
    Primitive { width_bits: u32, signedness: Signedness, kind: PrimKind },
    Aggregate { fields: Vec<FieldEntry>, tag: Option<TagEntry> },
    FatPointer { elem_kind: Idx },       // cites CG:TR-4 for the { len, cap, data } shape
    Closure,                             // cites CG:TR-5 for the { fn_ptr, env_ptr } shape
    Opaque { runtime_handle: HandleKind }, // Iterator, Channel, etc. — runtime-managed
}

FieldEntry {
    original_index: u32,                 // index in source declaration order (type pool)
    memory_offset:  u64,                 // byte offset from struct base
    field_type:     Idx,                 // resolved type pool index; LLVM lowering is CG:TR-1's job
    storage_width:  Option<u32>,         // narrowed bit width, or None = canonical (§6)
}

TagEntry {
    width_bits: u32,                     // 8/16/32 per RP-22
    offset:     u64,                     // byte offset from struct base; 0 for standard tagged layout
}

NicheInfo {
    host_path:      Vec<u32>,            // projection path (field indices) to the field carrying the niche
    niche_pattern:  NichePattern,        // enumeration of niche kinds (null, surrogate range, unused tag, …)
    valid_range:    ValidRange,          // the semantic range of the host field
}
```

`ReprPlan` carries `Idx` references, not LLVM handles. LLVM type construction from an `Idx` is `CG:TR-1`'s responsibility and happens at emission time, downstream of this artifact.

### RP-2 — Uniqueness

For a given `Idx` in the frozen type pool (`TYPES:TY-6`), there SHALL be at most one `ReprPlan` per compilation session. `ReprPlan` is content-addressed by the fully-resolved type: structurally equal types MUST yield equal plans. The plan is a function of the type pool, the global `#repr(...)` attribute set, and the target triple — it SHALL NOT depend on call-site context, optimization level, or emission order.

### RP-3 — Determinism

Given the same type pool, the same `#repr` annotations, and the same target triple, `ReprPlan` computation SHALL produce byte-identical output across runs. Field ordering (`RP-20`), padding insertion (`RP-20`), discriminant assignment (`RP-22`), and niche selection (§7) are deterministic functions of their inputs; they SHALL NOT depend on `HashMap` iteration order, thread interleaving, or other sources of non-determinism (`CHK:SL-1`, `HYG:` §Pass Composition — Pass determinism).

### RP-4 — Imperative computation (NOT Salsa)

`ReprPlan` is **not** a Salsa tracked struct. It is computed imperatively by `compute_repr_plan()` as a forward pass that mutates state across multiple analysis phases (triviality → range → narrowing → layout). It is computed once per compilation and passed as `&ReprPlan` to codegen. It SHALL derive `Clone, Eq, PartialEq, Hash, Debug`. It SHALL NOT contain `Arc<Mutex<T>>`, `Rc`, raw pointers, function pointers, `dyn Trait`, or any LLVM handle type. All references to other compiler artifacts are via stable identifiers (`Idx`, `Name`).

### RP-5 — Consumers

`ReprPlan` has the following authoritative consumers. Each consumer SHALL read only from the plan (or the narrowing map of §6); it SHALL NOT recompute layout from the type pool directly.

| Consumer rule | What it reads | Use |
|---------------|---------------|-----|
| `CG:TR-1` canonical type mapping | `layout` to construct the LLVM type | Struct type construction, element type resolution |
| `CG:TR-3` aggregate field ordering | `FieldEntry.original_index` ↔ `memory_offset` | `struct_gep` for field access |
| `CG:AB-1` direct/indirect threshold | `abi_size` | 16-byte ABI boundary classification |
| `CG:AB-3` return classification | `abi_size` | `Direct` (≤16) vs `Sret` (>16) |
| `CG:NR-2`, `CG:NR-6` struct-field narrowing | `FieldEntry.storage_width` | `trunc_for_narrowed_struct` / `sext_narrowed_field` |
| `CG:RT-2` RC header reads | n/a — plan describes payload (`RH-4`) | Codegen uses `CG:RT-2` for header geometry |

### RP-6 — Declaration-order ↔ memory-order

`LayoutTree::Aggregate.fields` is in **memory order** chosen by `RP-20`, not source-declaration order. `FieldEntry.original_index` is the authoritative map from source declaration (type-pool field order) to memory slot. Any codegen consumer that resolves a field by source name MUST look up `original_index`; hard-coded positional offsets outside `ReprPlan` are a `LEAK:scattered-knowledge` violation (`HYG:` §SSOT).

---

## §3 Primitive Type Representations

### RP-10 — Primitive layouts cite `CG:TR-1`

`CG:TR-1` is the single source of truth for the Ori-to-LLVM primitive mapping. `LayoutTree::Primitive` entries in a `ReprPlan` SHALL agree with `CG:TR-1` exactly. The table below is REFERENTIAL, reproduced for convenience; `CG:TR-1` controls if there is ever any disagreement.

| Ori type | LLVM canonical type | Size (bytes) | Spec |
|----------|--------------------:|-------------:|------|
| `int` | `i64` | 8 | Annex E §Representation Optimization §Canonical Representations |
| `float` | `f64` | 8 | Annex E §Representation Optimization §Canonical Representations |
| `bool` | `i1` | 1 | Annex E §Representation Optimization §Canonical Representations |
| `byte` | `i8` | 1 | Annex E §Representation Optimization §Canonical Representations |
| `char` | `i32` | 4 | Annex E §Representation Optimization §Canonical Representations |
| `void` / `()` | `i64` (unit sentinel) | 8 | Clause 8.1 Built-in Types table; `CG:TR-1` |
| `Never` | `i64` (same storage as unit) | 8 | Clause 8.1.1 Never Semantics; `CG:TR-1` |
| `Ordering` | `i8` (`Less=0`, `Equal=1`, `Greater=2`) | 1 | Clause 8.4.1 Ordering; Annex E §Representation Optimization |
| `Duration` | `i64` (nanoseconds) | 8 | Clause 8.1.2 Duration; `CG:TR-1` |
| `Size` | `i64` (bytes) | 8 | Clause 8.1.3 Size; `CG:TR-1` |

Notes (load-bearing):

- **`void` and `Never` are both `i64`.** They are storage-identical sentinels; the difference is semantic (void is the unit value; Never is an uninhabited bottom type). LLVM `void` appears only in function-signature return positions handled by `CG:AB-3`'s `Void` classification — values of type `void` or `Never` that are materialized in IR use the 8-byte sentinel.
- **`Ordering` IS a niche source.** Its value set is `{0, 1, 2}` stored as `i8`; bit patterns `3..=255` provide 253 niches. The niche analysis in `ori_repr/src/layout/niche.rs` implements this. `Option<Ordering>` can be niche-encoded (tag value 3 = None).
- **`bool` storage.** The LLVM value type is `i1` (used for arithmetic and branching). Storage of `bool` inside aggregates follows `CG:TR-1`; repr.md does not redefine aggregate-storage zero-extension rules.

### RP-11 — Opaque pointer model

Ori compiles against LLVM's opaque pointer model. All pointer types lower to `ptr` (`CG:TR-1`). Type information for `GEP` is carried by the LLVM instruction, not the pointer type. `LayoutTree` records `field_type` as an `Idx`; pointee LLVM types are reconstructed by `CG:TR-1` at emission.

---

## §4 Composite Type Representations

### RP-20 — Struct layout (default)

A struct `type S = { f0: T0, f1: T1, ..., fn: Tn }` without a `#repr(...)` attribute has a default `ReprPlan` whose `LayoutTree::Aggregate.fields` is in **memory order chosen to minimize padding**:

1. Compute `(size, alignment)` for each field from its own `ReprPlan` (`RP-1`).
2. Sort fields by decreasing alignment, breaking ties by decreasing size, breaking further ties by ascending `original_index`. The tie-break order is deterministic (`RP-3`).
3. Lay out fields in sorted order, inserting the minimum padding needed to satisfy each field's alignment.
4. Round the final `abi_size` up to the struct's own `abi_alignment` (the maximum field alignment; `1` for zero-field structs).

The default layout is not ABI-compatible with C. Consumers that need C layout MUST annotate with `#repr("c")` per §5.

### RP-21 — Sum type layout (tagged)

A sum type `type T = A | B(x: int) | C(y: float, z: bool)` without niche encoding (§7) has a tagged `LayoutTree::Aggregate` with a `TagEntry` at field-slot 0:

- The `TagEntry.width_bits` follows `RP-22`.
- The payload region is sized and aligned to the maximum of all variant payload sizes/alignments.
- Variants with no payload fields contribute zero-sized payloads.
- All-unit enums (every variant payload-less) elide the payload entirely; the type reduces to a bare tag (`Spec: Annex E §Representation Optimization` — "All-unit enum payload elimination").

### RP-22 — Discriminant width

For a sum type with N variants, the `TagEntry.width_bits` SHALL be:

- `N ≤ 2`: `1` (stored as `i8` per `CG:TR-1` aggregate-storage rules) — unless niche encoding elides the tag entirely (§7).
- `3 ≤ N ≤ 256`: `8`.
- `257 ≤ N ≤ 65_536`: `16`.
- `N > 65_536`: `32`.

Discriminant values are assigned in declaration order starting from `0`. The mapping `variant_name → tag_value` is stable for a given type definition and is part of the `ReprPlan`. Cross-crate stability of discriminant values is a function of declaration-order stability in source — no hash-based or reordering strategy is permitted.

### RP-23 — Tuple layout

A tuple `(T0, T1, ..., Tn)` has the same `LayoutTree` shape as a struct with fields named `0, 1, ..., n`. Field reordering per `RP-20` applies unless `#repr("c")` is used. Field access via `.0`, `.1`, etc. SHALL resolve through `original_index` — source-position offsets SHALL NOT be hard-coded outside the plan (`RP-6`).

### RP-24 — Newtype layout

A newtype `type N = Existing` (`Spec: Clause 8.6.3`) has a `ReprPlan` structurally identical to that of `Existing` — same `abi_size`, `abi_alignment`, `layout`, `niche`. Newtypes SHALL carry `repr_kind = ReprKind::Transparent` implicitly; `#repr(...)` attributes SHALL NOT be applied directly to newtype declarations (`Spec: Clause 26.4.9`, `ori-syntax.md §FFI`). Construction (`N(v)`) and projection (`.inner`) emit no layout transformation; the `.inner` projection is at offset 0 with the inner type's full width. Newtype representation erasure is a permitted optimization per `Spec: Annex E §Representation Optimization §Permitted Optimizations`.

### RP-25 — Trait object layout

Trait object representation is chosen by the compiler (`Spec: Clause 8.8`: "The compiler determines the dispatch mechanism. Users specify what, not how"). The `ReprPlan` for a trait object `Trait` records a pointer-sized opaque handle whose concrete layout (e.g., thin pointer plus separately-dispatched vtable, or a paired fat-pointer struct) is determined by `codegen-rules.md` at emission and the runtime's dispatch contract. repr.md SHALL NOT hard-code a fixed `{ data_ptr, vtable }` shape — that decision lives downstream. Whatever shape codegen chooses, the `abi_size` is the value the layout produces, and ABI passing follows `CG:AB-1` / `CG:AB-3` from that size.

### RP-26 — Existential (`impl Trait`) layout

`impl Trait` in return position is statically dispatched (monomorphized). It has no vtable and no fat-pointer layout; its `ReprPlan` is the concrete monomorphic type's `ReprPlan` (`ori-syntax.md §Existential Types`). The existential is a compile-time abstraction and carries no additional runtime representation.

---

## §5 The `#repr(...)` Attribute

### RP-30 — Attribute surface

The `#repr(...)` attribute is a language-surface feature owned by `Spec: Clause 26.4.9` and summarized in `ori-syntax.md §FFI`. It applies ONLY to struct type declarations (`type S = { ... }`). Newtypes are implicitly transparent and SHALL NOT carry `#repr(...)` directly (`RP-24`). Sum types and tuples SHALL NOT carry `#repr(...)`.

Four forms are valid:

| Attribute | Resulting `repr_kind` | Layout semantics |
|-----------|----------------------|------------------|
| `#repr("c")` | `ReprKind::C` | Field order = declaration order (NO `RP-20` reorder); padding per platform C ABI; alignment = max field alignment |
| `#repr("packed")` | `ReprKind::Packed` | Field order = declaration order; alignment = 1; zero padding |
| `#repr("transparent")` | `ReprKind::Transparent` | Struct has exactly one field; inner type's `ReprPlan` is used verbatim |
| `#repr("aligned", N)` | `ReprKind::Aligned(N)` | Default layout plus forced minimum alignment `N`; `N` SHALL be a power of two. If `N < abi_alignment`, the actual alignment remains `abi_alignment` (the attribute is a floor, not an override) |

### RP-31 — Attribute validation

Attribute validation is owned by the type checker (`CHK:` §Diagnostics catalog, error `E2041` "Invalid `#repr` attribute"; cross-check with `CHK:` §11 DI-* in the current typeck revision). Per `Spec: Clause 26.4.9`, the following are compile-time errors:

- Unknown attribute value (e.g., `#repr("custom")`) — `E2041`.
- `#repr(...)` on a non-struct declaration (sum type, tuple, or direct newtype) — `E2041`.
- `#repr("transparent")` on a struct without exactly one field — `E2041`.
- `#repr("aligned", N)` where `N` is not a power of two or `N < 1` — `E2041`. Note: `N < abi_alignment` is valid (the attribute is a minimum-alignment floor; smaller values are accepted but have no effect).
- `#repr("packed")` combined with a field whose type requires natural alignment (e.g., a fat-pointer field, a type carrying an RC header reference) — `E2041`.

### RP-32 — Multi-attribute combinations

Per `Spec: Clause 26.4.9`, `#repr("c")` MAY combine with `#repr("aligned", N)` on the same declaration; the result is C layout with a minimum-alignment floor of `N`. All other multi-attribute combinations are invalid (`E2041`). `#repr("packed")` SHALL NOT combine with `#repr("aligned", N)`. `#repr("transparent")` SHALL NOT combine with any other `#repr(...)`.

### RP-33 — Interaction with narrowing

`#repr("c")` and `#repr("packed")` DISABLE integer narrowing on all fields of the annotated type — every `FieldEntry.storage_width` SHALL be `None`; canonical widths from `RP-10` apply throughout (§6 RN-3). `#repr("transparent")` and `#repr("aligned", N)` do NOT disable narrowing; the inner (or default) layout applies as normal.

### RP-34 — Interaction with niche encoding

Niche encoding (§7) is DISABLED on fields of `#repr("c")` and `#repr("packed")` types. `#repr("transparent")` inherits niche availability from its inner type (`RP-24`). `#repr("aligned", N)` permits niche encoding.

---

## §6 Integer Narrowing Scope Policy

### RN-1 — Storage-boundary principle

Integer narrowing is a representation optimization that may replace a canonical `int` (`i64` per `RP-10`) with a smaller integer width when value-range analysis proves the smaller width is sound. Narrowing decisions SHALL be consumed only at **storage sites** (collection backing buffers, struct fields, and local-variable definition points where codegen inserts a trunc+sext pair per `CG:NR-5`). All other code — iterator pipelines, trampolines, scratch buffers, runtime function arguments, ABI boundaries — SHALL operate on canonical widths.

This is the repr-layer statement of the same boundary enforced in emission by `CG:NR-1` through `CG:NR-6`. repr.md owns the policy and the sites; `codegen-rules.md` owns the emission mechanics.

### RN-2 — Narrowing sites

Narrowing is permitted at exactly the following sites; repr.md records the narrowing decision for each and codegen consumes it per the cited `CG:NR-*` rules:

1. **Struct and tuple fields**. When a field's type is `int` and range analysis proves a narrower width suffices, the field's `FieldEntry.storage_width` SHALL be set to `{8, 16, 32}`. Construction narrows canonical values; extraction widens them back. Emission rule: `CG:NR-6`.
2. **Collection backing buffers**. When a collection's element type is `int` and range analysis proves a narrower width suffices, the backing buffer stride is narrowed. The outer fat pointer's `len` and `cap` remain `i64`; only the element stride changes. Emission rules: `CG:NR-2` (storage boundary scope), `CG:NR-3` (iterator pipeline stays canonical), `CG:NR-4` (sext widening trampoline).
3. **Local variables of inferred type `int`**. The representation layer records a narrowing decision in the per-SSA-variable narrowing map. At emission, `CG:NR-5` inserts a `trunc` + `sext` pair at the variable's definition site; the value immediately widens back to canonical. The alloca SHALL remain canonical — narrowing at local sites constrains LLVM's known value range for downstream optimization passes, not the physical slot width.

### RN-3 — Narrowing exclusions

Narrowing SHALL NOT apply at:

- Function parameters and return values (ABI boundary; `CG:AB-1`, `CG:AB-3`).
- Arguments to runtime functions in `ori_rt` (`CG:RT-1` signature agreement).
- Arguments to FFI extern functions (`extern "c"`, `extern "js"`; `Spec: Clause 26`).
- Iterator `Item` associated types; the pipeline stays canonical per `CG:IT-*`.
- Trampoline element types (`CG:TM-2`).
- Fields of `#repr("c")` or `#repr("packed")` types (`RP-33`).
- Type-erased values crossing trait-object dispatch (`RP-25`).
- Values crossing `with...in` capability frame boundaries.
- The `tag` field of a sum type (discriminant width is `RP-22`, not narrowing).

### RN-4 — Sign extension on widen, truncation on narrow

The canonical representation of `int` is signed two's complement (`Spec: Annex E §Numeric Types §Integers`). When a narrowed storage site is loaded, codegen SHALL sign-extend (`sext`) to canonical before any further use. When a narrowed storage site is stored, codegen SHALL truncate (`trunc`) from canonical. Soundness follows from the range-analysis invariant: if the canonical value is within the narrowed range at the store, `sext(trunc(v))` reproduces `v` exactly. Emission: `CG:NR-4`, `CG:NR-6`.

### RN-5 — Disable flag

The compiler SHALL accept `ORI_NO_REPR_OPT=1` and `--no-repr-opt` to disable representation optimization (integer narrowing and enum packing, per `CLAUDE.md §Commands`). Under this flag every `FieldEntry.storage_width` SHALL be `None` and `ReprPlan.niche` SHALL be `None`. Field reordering is NOT governed by this flag — `RP-20` reordering applies regardless, unless the declaration carries `#repr("c")` or `#repr("packed")` which disable reordering on a per-type basis. Observable behavior under the disable flag MUST be identical to the optimized form (`CANON:§7.1` AIMS Invariant 2: active rewrites are sound).

### RN-6 — Range-analysis authority

The value-range analysis that drives `RN-2` is owned by the representation layer. It reads the typed IR (`CANON:§4.2`) and the realized ARC IR (`CANON:§4.5`) and produces a per-SSA-variable range summary. The analysis SHALL be conservative: if it cannot prove a narrower width, the canonical width is used. False negatives (missed narrowing) are permitted; false positives (claiming a narrower width that does not hold) are correctness bugs.

---

## §7 Niche Encoding Policy

### NI-1 — Niche definition

A **niche** of a type `T` is a bit pattern of `T`'s `abi_size` bytes that is provably never produced by any valid value of `T`. Niche availability enables tagless encoding of sum types: a payload-less variant can be represented by a niche of another variant's payload field, eliding the discriminant (`RP-21`).

### NI-2 — Niche sources

The following types have canonical niches selectable by the representation layer:

| Type | Niche pattern | Rationale |
|------|---------------|-----------|
| Non-null pointer (any `ptr` known never to be null: RC'd value, trait-object data slot, closure `env_ptr` for closures known to capture) | `null` (all-zero) | RC-owned pointers from `ori_rt` allocation are never null. |
| `char` | Values in the surrogate range `U+D800..=U+DFFF` and any value `> U+10FFFF` | `Spec: Clause 8.1` — `char` is a valid Unicode scalar value. |
| `bool` | Any value `!= 0 && != 1` | `CG:TR-1` — `bool` is `i1`; storage is `i8` with valid bit patterns `0` and `1`. |
| Sum type with unused discriminant values | Any tag value outside the assigned `{0..N-1}` range | `RP-22` fixes the assigned range; surplus is a niche. |
| Struct / tuple | The niche of any constituent field that has one | Propagation rule (`NI-6`). |

Types without niches for the purposes of this policy: `int`, `float`, `byte`, `Duration`, `Size`. Types WITH niches: `bool` (254), `Ordering` (253), `char` (Unicode gap), `RcPointer` (null=0), `str` fat pointer (null data).

### NI-3 — Niche encoding of `Option<T>`

For `Option<T>` where `T`'s `ReprPlan.niche` is `Some(n)`:

- `Some(v)` is encoded as `v`'s bit pattern (`abi_size_of_T` bytes).
- `None` is encoded as `n.niche_pattern`.
- The outer `Option<T>` has `abi_size = abi_size_of_T`, `abi_alignment = abi_alignment_of_T`, no explicit tag field, and its own niche availability drops to `None` (the niche has been consumed).

For `Option<T>` where `T` has no niche, the outer `ReprPlan` is tagged per `RP-21`: `{ tag: i8, payload: T }` with `None = tag(0)`, `Some = tag(1)` + payload valid.

### NI-4 — Niche encoding of `Result<T, E>`

`Result<T, E>` uses niche encoding when exactly one of `T` or `E` has a niche AND the other's payload size fits within the niche host. The general decision:

- If `T` has a niche and `abi_size_of_E <= abi_size_of_T`, the tag is elided: `Ok` is any non-niche bit pattern of `T`; `Err` uses the niche pattern and packs `E`'s payload into the remaining bytes.
- Otherwise `Result<T, E>` is tagged per `RP-21`: `{ tag: i8, payload: union(T, E) }`, with `Ok = tag(0)` and `Err = tag(1)`.

Niche encoding of `Result` is conservative — tagged encoding is used whenever niche encoding is not provably sound and space-saving.

### NI-5 — Multi-variant niche encoding

For a general sum type `T = A | B(x: U) | C(y: V)`, niche encoding elides the tag only when a single host field has enough niche bit patterns to represent every non-host variant. Otherwise tagged encoding per `RP-21` is used. The decision is made at `ReprPlan` computation time and is deterministic (`RP-3`).

### NI-6 — Niche propagation

A struct field whose type has a niche contributes that niche to the outer struct's `ReprPlan.niche`:

- `host_path` prepends the field's projection to the inner host's path (creating a full field chain).
- `niche_pattern` copies from the inner niche.
- `valid_range` copies from the inner niche.

This enables recursive niche encoding (e.g., `Option<Option<ptr>>` where the inner `Option<ptr>` consumes the `null` niche, and the outer `Option` either drops niche availability or selects a different non-`null` / non-`Some(None)` bit pattern). Which niche the outer layer selects is a deterministic decision of the representation layer.

### NI-7 — Interaction with `#repr`

`#repr("c")` and `#repr("packed")` DISABLE niche encoding on the annotated type (`RP-34`). Sum types with such fields fall back to tagged encoding per `RP-21`. `#repr("transparent")` inherits niche availability from the inner type (`RP-24`). `#repr("aligned", N)` permits niche encoding.

---

## §8 RC Header — Repr-Layer View

### RH-1 — Header schema is owned by `CG:RT-2`

The RC header schema (fields, widths, offsets, field meanings) is the single source of truth in `codegen-rules.md §7 RT-2` ("RC Header Layout"). repr.md SHALL NOT redefine the header geometry. When `CG:RT-2` evolves — adding, renaming, resizing, or relocating fields — repr.md consumers automatically inherit the change through cross-reference.

As of the current `CG:RT-2` revision, the header is a 32-byte structure preceding heap-allocated payload data, with fields `data_size`, `elem_dec_fn`, `elem_count`, and `strong_count`. `data_ptr` exposed to Ori code points at the first payload byte; the header precedes it.

### RH-2 — Named constants, not magic numbers

Every codegen site that inspects the RC header SHALL compute field addresses from the named constants defined alongside `CG:RT-2` (e.g., `RC_HEADER_SIZE_BYTES`, offsets per field). Hard-coded numeric offsets duplicated across codegen or runtime sites are a `LEAK:scattered-knowledge` violation (`HYG:` §SSOT). The `IsShared` emission (`CG:RE-4`) uses these constants to GEP to `strong_count`.

### RH-3 — Heap vs non-heap classification

The representation layer marks each `ReprPlan` as heap-allocated (has an RC header) or inline/stack/immortal (no header). Inputs to this classification come from:

- **Scalars** (`CG:TR-1` primitives enumerated in `RP-10`): no header. The AIMS lattice does not track scalars as lattice elements — they are sentinels excluded from the state map (`AIMS:L-9`), and the shape dimension does not carry a scalar variant (`aims-rules.md §1.6`).
- **`Value`-marker types** (`Spec: Clause 8.4`, `ori-syntax.md §Prelude`): `type T: Value = { ... }` is inline-stored, bitwise-copy, and never carries an RC header.
- **Stack-promoted values**: values whose AIMS `Locality` dimension is `BlockLocal` or `FunctionLocal` with no escape (`aims-rules.md §1.5`, pending the stack-promotion extension referenced by `CANON:§7.1` AIMS Invariant 5 and `CLAUDE.md §AIMS` through-line).
- **Immortal values**: statically-allocated constants detected by the immortal pre-pass feeding the lattice-driven analysis (`CLAUDE.md §AIMS`).

Codegen SHALL NOT emit RC operations on values in any of these categories (`CG:RE-2`).

### RH-4 — Size accounting describes payload only

`ReprPlan.abi_size` and `ReprPlan.abi_alignment` describe the **payload** — they do NOT include the RC header. Allocation sites that require header space consume `CG:RT-2`'s `RC_HEADER_SIZE_BYTES` constant; repr.md does not restate it. The allocator's alignment contract (ensuring `data_ptr` satisfies `abi_alignment` once the header is accounted for) is a runtime invariant owned by `ori_rt` (`runtime.md`).

---

## §9 Collection and Closure Representations

### RP-40 — Fat-pointer shape cites `CG:TR-4`

The fat-pointer layout for `str`, `[T]`, `{K: V}`, and `Set<T>` (`{ len: i64, cap: i64, data: ptr }`, 24 bytes, 8-byte alignment, field indices `FAT_PTR_FIELD_LEN=0`, `FAT_PTR_FIELD_CAP=1`, `FAT_PTR_FIELD_DATA=2`) is owned by `CG:TR-4` and corroborated by `CG:RT-6`. repr.md SHALL NOT redefine the shape; its `LayoutTree::FatPointer` variant is a stable descriptor carrying `elem_kind`, with `CG:TR-1` reconstructing the physical LLVM type at emission. The backing buffer pointed to by `data` carries an RC header per `CG:RT-2`.

The `str` entry in `Spec: Annex E §Built-in Type Representations` documents a 2-tuple `{ len, data }`; the shipped representation documented by `CG:TR-4` / `CG:RT-6` is the 3-tuple `{ len, cap, data }`. When the two disagree, `codegen-rules.md` is the SSOT for the current implementation — repr.md consumers (and downstream code) rely on `CG:TR-4` being authoritative for the physical shape.

### RP-41 — `str` specifics

`str` uses the generic `CG:TR-4` fat-pointer shape with an element interpretation of UTF-8-encoded bytes. `len` is the byte length (`Spec: Annex E §Strings §Length`). Codepoint indexing (`str[i]` per `ori-syntax.md`) is a prelude method, not a direct memory access, and does not add repr-layer-specific structure.

### RP-42 — `[T]` element stride

For `[T]`, the backing buffer's element stride is `ReprPlan_of_T.abi_size` padded to `ReprPlan_of_T.abi_alignment`. Narrowing (`RN-2` site 2) may reduce the stride when `T = int` and range analysis succeeds; the fat pointer's `len` and `cap` remain canonical `i64` and are measured in narrowed-element units per `CG:NR-2`.

### RP-43 — `{K: V}` and `Set<T>`

Maps and sets share the outer fat-pointer layout with lists and strings (`CG:TR-4`). Bucket/probe layout inside the backing buffer is owned by `ori_rt` (`runtime.md`); `len` is entry count, `cap` is bucket count.

### RP-44 — Fixed-capacity list `[T, max N]`

A fixed-capacity list (`Spec: Clause 8.2.2`, `ori-syntax.md §Fixed-Capacity Lists`) is inline-allocated. Its `ReprPlan` is a struct-shaped `LayoutTree::Aggregate` with:

- A 64-bit `len` field (runtime `0..=N`).
- An inline buffer of `N` contiguous `T` slots with stride = `ReprPlan_of_T.abi_size` padded to `ReprPlan_of_T.abi_alignment`.

`cap` is the compile-time constant `N` and is NOT a runtime field. The subtype relation `[T, max N] <: [T]` materializes a `CG:TR-4` fat pointer at the conversion site; whether the fat pointer's `data` points into the inline buffer or a heap copy depends on AIMS locality (`aims-rules.md §1.5`).

### RP-45 — Closure shape cites `CG:TR-5`

The closure representation `{ fn_ptr: ptr, env_ptr: ptr }` (16 bytes, field indices `CLOSURE_FIELD_FN=0`, `CLOSURE_FIELD_ENV=1`) is owned by `CG:TR-5`. repr.md SHALL NOT redefine the shape; `LayoutTree::Closure` is a stable descriptor. The environment at `env_ptr` is a separately-allocated value with its own `ReprPlan` describing captures; the environment carries an RC header per `CG:RT-2` when present.

---

## §10 ABI Boundary Interface

### RP-50 — ABI consumes `ReprPlan.abi_size`

The direct/indirect ABI classification at `CG:AB-1` consumes `ReprPlan.abi_size`: types with `abi_size <= 16` are `Direct` for both parameters and returns (`CG:AB-2`, `CG:AB-3`); types with `abi_size > 16` are `Indirect { alignment }` for parameters and `Sret { alignment }` for returns. repr.md owns `abi_size` (as the payload size, `RH-4`); `codegen-rules.md §2 AB-*` owns the passing-mode translation, calling conventions, FastISel restrictions, and the ARM64 `sret` attribute.

### RP-51 — Repr does not decide calling conventions

The representation layer SHALL NOT decide calling conventions, parameter passing modes, register allocation, or the `sret` attribute. Those decisions live in `codegen-rules.md §2 AB-*` and §§4 TM-*, §5 RE-*, §8 AT-*. repr.md provides the inputs (`abi_size`, `abi_alignment`, `layout`, narrowing map); it does not consume them.

### RP-52 — Narrowing does not cross the ABI

Per `RN-3`, narrowing is suppressed at ABI boundaries. A narrowed local variable SHALL be widened to canonical before being passed as an argument, returned from a function, or stored in an `sret` buffer. Enforcement is at emission (`CG:NR-4`, `CG:TM-2`) and at codegen audit (`ORI_AUDIT_CODEGEN=1`).

---

## §11 Phase Contracts and Cross-Phase Invariants

### RV-1 — Input contract (repr-layer entry)

On entry to the representation layer (pre-phase-8 sub-layer):

- The type pool is frozen for this compilation session (`TYPES:TY-6`). No new type indices are created.
- Every `Idx` reachable from the ARC IR is fully resolvable via `pool.resolve_fully(idx)` (no `Tag::Var`, no `Tag::Infer`, no `Tag::Projection`, no `Tag::SelfType`, no unresolved `Tag::Named`; `CANON:§4.2`).
- Every ARC IR instruction has types annotated as fully-resolved `Idx` values (`CANON:§4.5`).
- The AIMS lattice state is converged (`CANON:§4.4`).
- `#repr(...)` attributes have been validated by the type checker; no `E2041` errors remain (`RP-31`).

These conditions SHALL be `debug_assert!`'d at layer entry. A violation raises an internal compiler error; release builds MUST NOT silently miscompile.

### RV-2 — Output contract (repr-layer exit)

On exit from the representation layer:

- Every `Idx` referenced by downstream codegen has exactly one `ReprPlan` in the plan map (`RP-2`).
- Every `ReprPlan` satisfies: `abi_alignment` is a power of two, `abi_alignment >= 1`, `abi_size` is a multiple of `abi_alignment`, every aggregate `FieldEntry.memory_offset + size(FieldEntry.field_type) <= abi_size`, fields do not overlap (except inside niche-encoded sum payloads, where `NicheInfo` documents the intentional overlap).
- The narrowing map assigns a `storage_width` to every eligible SSA variable and every eligible `FieldEntry` per `RN-2`; sites not eligible have `storage_width = None`.
- For every sum type, either `niche = Some(...)` with a consistent `NicheInfo` or a `TagEntry` is present per `RP-21`.

### RV-3 — Agreement with `CANON:§4.5` (ARC realization output)

`CANON:§4.5` asserts that RC is balanced per block, drops are placed correctly, and COW diamonds are contracted. The repr layer SHALL NOT invalidate any of these properties — it is a pure layout assignment and does NOT add or remove instructions from the ARC IR. If a repr decision would require rerunning AIMS (for example, changing a value's locality classification), the decision is wrong (`CLAUDE.md §AIMS` Invariant 3 — "no pass may rely on stale summaries"; `CANON:§7.1` Invariant 3).

### RV-4 — Downstream consumers (in `codegen-rules.md`)

The following `codegen-rules.md` rules consume repr-layer output. Each SHALL read from `ReprPlan` (or the narrowing map); each SHALL NOT recompute layout from the type pool:

- `CG:TR-1` canonical type mapping
- `CG:TR-2` full resolution
- `CG:TR-3` aggregate field ordering
- `CG:TR-4` fat-pointer shape (for `elem_kind` propagation)
- `CG:TR-5` closure shape
- `CG:NR-1..NR-6` narrowing emission
- `CG:TM-2` canonical types in trampolines
- `CG:IT-*` iterator canonical types
- `CG:AB-1..AB-7` ABI classification and emission
- `CG:RE-2` scalar exemption
- `CG:RE-4` IsShared RC check
- `CG:RT-2` RC header reads (repr produces payload sizes; codegen consumes RC geometry directly)

A codegen site that recomputes layout independently of `ReprPlan` is a `LEAK:scattered-knowledge` violation (`HYG:` §SSOT) and MUST be refactored to consume the plan.

### RV-5 — Interface to `aims-rules.md`

The representation layer reads two inputs from AIMS:

1. **Shape dimension** (`aims-rules.md §1.6`) — to identify `ReusableCtor` values (structs/enum variants eligible for reuse) and `CollectionBuffer` values (always heap-allocated). Shape is a per-variable lattice dimension; repr reads its converged value.
2. **Scalar sentinel** (`AIMS:L-9`) — to identify values excluded from the lattice state map because they are scalars, and thus carry no RC header per `RH-3`. This is a sentinel check, not a lattice join.
3. **Locality dimension** (`aims-rules.md §1.5`) — to identify values whose escape class enables stack-promotion (carrying no RC header per `RH-3`), when the stack-promotion extension referenced by `CLAUDE.md §AIMS` through-line lands. Until then, `BlockLocal`/`FunctionLocal` is advisory for repr layout decisions.

repr SHALL NOT read, modify, or invalidate any other AIMS lattice dimension.

### RV-6 — No phase bleeding

repr.md SHALL NOT:

- Perform type checking or re-typing (`typeck.md`).
- Re-run canonicalization (`ori_canon`, `CANON:§4.3`).
- Invoke the parser or the lexer (`parse.md`).
- Re-run AIMS analysis (`aims-rules.md`).
- Emit LLVM IR instructions (`codegen-rules.md`).
- Allocate memory at runtime (`runtime.md`).

These are phase-purity invariants (`CANON:§5`, `compiler.md §Phase-Specific Purity`). Violation is a bug.

---

## §12 Verification Surface

### RV-10 — `debug_assert!` surface

Every load-bearing property in §11 SHALL be either a `debug_assert!` at a layer boundary or a test. Implicit invariants are invisible regressions (`CLAUDE.md §Stabilization Discipline`).

### RV-11 — Codegen audit (`ORI_AUDIT_CODEGEN=1`)

The codegen audit mode (`CLAUDE.md §Commands`) validates repr-layer output at LLVM emission time. It checks:

- Every struct `GEP` uses an offset derived from `ReprPlan`, not a hard-coded constant (the named RC-header offset constant from `CG:RT-2` is the only permitted exception; `RH-2`).
- Every narrowed load is followed by a `sext` to canonical before escape (`RN-4`, `CG:NR-4`).
- Every narrowed store is preceded by a `trunc` from canonical (`RN-4`, `CG:NR-6`).
- RC inspection sites resolve the header address via the `CG:RT-2` named constants (`RH-2`).
- No narrowed value crosses an ABI boundary (`RN-3`, `RP-52`).

### RV-12 — `ORI_NO_REPR_OPT` dual-execution parity

Under `ORI_NO_REPR_OPT=1` every `FieldEntry.storage_width` is `None` and every `ReprPlan.niche` is `None`. Observable behavior MUST be identical to the optimized form. The property is tested via dual-execution parity in the test harness; divergence is a correctness bug (`CLAUDE.md §Fix Completeness`).

### RV-13 — Alive2 translation validation

The Alive2 corpus (`tests/alive2/`, `diagnostics/alive2-verify.sh`) covers functions exercising narrowing, niche encoding, and field reordering. A repr-layer change that breaks Alive2 translation validation is a correctness bug.

### RV-14 — Determinism test

A regression test SHALL compile the same source twice and diff the emitted `ReprPlan` maps (via a dedicated dump channel; the test harness surfaces `ReprPlan` output separately from `ORI_DUMP_AFTER_ARC=1`, which dumps ARC IR). The outputs MUST be byte-identical (`RP-3`). The specific dump channel is an implementation concern owned by `codegen-rules.md` / the diagnostic-script corpus; repr.md specifies the invariant, not the flag.

---

## §13 Non-Negotiable Invariants

These are the load-bearing facts of the representation layer; each is (or will be) cross-referenced from `CANON:§7`:

1. **`ReprPlan` uniqueness.** One `ReprPlan` per fully-resolved `Idx`, content-addressed by type (`RP-2`).
2. **Determinism.** Same type pool + same `#repr` attributes + same target → byte-identical `ReprPlan`s (`RP-3`).
3. **Salsa-safe data.** `ReprPlan` carries stable identifiers (`Idx`, named constants), never LLVM handles or other process-bound state (`RP-4`).
4. **Layout SSOT flow.** Every layout decision flows through `ReprPlan`; codegen sites that recompute layout independently are violations (`RV-4`).
5. **Storage-boundary narrowing.** Narrowing is consumed only at storage sites. Iterator pipelines, trampolines, and ABI boundaries use canonical widths (`RN-1`, `RN-3`, `RP-52`).
6. **Disable-flag soundness.** `ORI_NO_REPR_OPT=1` produces observably-identical behavior to the optimized form (`RN-5`, `RV-12`).
7. **RC header schema lives in `CG:RT-2`.** repr.md does not redefine header geometry; codegen uses named constants (`RH-1`, `RH-2`).
8. **`#repr(...)` scope.** The attribute applies only to struct declarations; newtypes are implicitly transparent; `#repr("c")` + `#repr("aligned", N)` is the sole permitted multi-attribute combination (`RP-30`, `RP-32`).
9. **No phase bleeding.** repr consumes typed/canonicalized/ARC-realized IR and the type pool and AIMS state; it does NOT invoke upstream phases or emit downstream instructions (`RV-6`).
10. **Attribute validation precedes layout.** Invalid `#repr(...)` is a type-checker error (`E2041`); layout computation SHALL NOT see invalid attributes (`RP-31`, `RV-1`).

---

## §14 Cross-References

- **Pipeline map**: `canon.md §1` (phase table), §4 (per-phase output invariants), §6 (SSOT table), §7 (non-negotiable invariants).
- **Parser**: `parse.md` (attribute parsing).
- **Type checker**: `typeck.md` (attribute validation, error catalog entry `E2041`, phase contracts §PC, soundness lemma `SL-1`).
- **Type pool**: `types.md` §TY (storage), §RG (registries), §TL (type surface), §PC (contracts).
- **AIMS**: `aims-rules.md` §1 (lattice dimensions; §1.5 Locality, §1.6 Shape), §5 (contracts), §8 (realization), §1.8 (lattice properties; `L-9` scalar sentinel).
- **Codegen**: `codegen-rules.md` §1 `TR-*` (type mapping, fat pointer, closure), §2 `AB-*` (ABI), §3 `NR-*` (narrowing emission), §4 `TM-*` (trampolines), §5 `RE-*` (RC emission), §6 `IT-*` (iterators), §7 `RT-*` (runtime contract; `RT-2` is the RC header SSOT), §8 `AT-*` (LLVM attributes), §9 `VR-*` (verification).
- **LLVM**: `llvm.md` (IR shape and verification).
- **Runtime**: `runtime.md` (`ori_rt` internals consuming the `CG:RT-2` header schema and the `CG:TR-4` fat-pointer shape).
- **Hygiene**: `impl-hygiene.md` §Single Source of Truth (SSOT), §Side Logic — Root of Architectural Decay, §Phase Boundaries, §Finding Categories (LEAK / DRIFT / GAP / WASTE).
- **Language surface**: `.claude/rules/ori-syntax.md` §FFI (`#repr`), §Types (primitives), §Fixed-Capacity Lists, §Existential Types, §Prelude (`Value`).
- **Spec**: `docs/ori_lang/v2026/spec/` — Clause 8.1 Primitive Types (with 8.1.1, 8.1.2, 8.1.3), Clause 8.2 Compound Types (List, Fixed-Capacity List, Map, Set, Tuple, Function, Range), Clause 8.4 Built-in Types (Ordering), Clause 8.6 User-Defined Types (Struct, Sum Type, Newtype, Derive), Clause 8.8 Trait Objects, Clause 21 Memory Model, Clause 26.4.9 `#repr` attribute, Annex E §Numeric Types, §Strings, §Collections, §Representation Optimization (with §Canonical Representations, §Permitted Optimizations, §Guarantees, §Non-Guarantees), §ARC Runtime (with §Heap Object Layout, §Runtime Functions, §Drop Functions, §Built-in Type Representations).

Navigation rule: if a fact is stated twice in this directory, one location is the SSOT and the other is a pointer. repr.md is the SSOT for `ReprPlan`, narrowing scope policy, niche-encoding policy, and `#repr(...)` layout semantics. It is a pointer for primitive LLVM mapping (owned by `CG:TR-1`), fat-pointer and closure shapes (owned by `CG:TR-4`, `CG:TR-5`), RC header geometry (owned by `CG:RT-2`), ABI passing modes (owned by `codegen-rules.md §2 AB-*`), and the AIMS lattice dimensions (owned by `aims-rules.md §1`).
