---
section: "12"
title: "Double-Ended Bit-Stealing for Algebraic Data Types"
status: complete
goal: "Identify which representation facts should become future Shape/repr outputs downstream of AIMS"
paper:
  title: "Double-Ended Bit-Stealing for Algebraic Data Types"
  doi: "https://doi.org/10.1145/3674628"
  venue: "ICFP 2024"
  authors: "Elsman"
depends_on: ["01", "02", "03", "04", "05", "06", "07", "08", "09", "10", "11"]
sections:
  - id: "12.1"
    title: "Paper Thesis"
    status: complete
  - id: "12.2"
    title: "What AIMS Should Adopt"
    status: complete
  - id: "12.3"
    title: "What AIMS Should Not Adopt"
    status: complete
  - id: "12.4"
    title: "Plan Edits"
    status: complete
  - id: "12.5"
    title: "Code Changes (Later)"
    status: complete
  - id: "12.6"
    title: "Lens Shift"
    status: complete
  - id: "12.7"
    title: "Open Risk"
    status: complete
---

# Section 12: Double-Ended Bit-Stealing for Algebraic Data Types

**Status:** Complete
**Goal:** Identify which representation facts should become future `ShapeClass`/repr
outputs and which parts belong in a repr-opt pass rather than AIMS proper.
Representation work should sit downstream of AIMS, not distort the memory-analysis core.

**Paper:** Elsman, "Double-Ended Bit-Stealing for Algebraic Data Types," ICFP 2024.
[DOI: 10.1145/3674628](https://doi.org/10.1145/3674628)

**Why read this last:** Representation work matters, but it should consume AIMS facts
(shape, locality, uniqueness) as inputs, not feed back into the analysis. This paper
defines the boundary between AIMS and a future repr optimizer.

**Pause questions:**
- Which representation facts should become future Shape/repr outputs?
- Which parts belong in repr-opt rather than AIMS proper?

**AIMS context:**
- `ShapeClass` tracks allocation shape (NonReusable, ReusableCtor, CollectionBuffer, ContextHole)
- `SizeClass` tracks allocation byte size (for cross-type reuse)
- `ValueRepr` classifies variables (Scalar, DefiniteRef, PossibleRef)
- `ArcClass` is the fundamental classification driving all RC decisions
- Stage 4 mentions "Representation optimization consuming AIMS shape/locality facts"

---

## 12.1 Paper Thesis

Elsman observes that on modern 64-bit architectures, heap pointers use far fewer than
64 bits. On x86_64 with canonical addresses, the top 16 bits of any user-space pointer
are always zero (bit 47 is sign-extended to bits 48-63). Meanwhile, heap allocators
guarantee alignment (typically 8 or 16 bytes), leaving the bottom 3-4 bits always zero.
The paper exploits *both* ends simultaneously --- hence "double-ended" --- to encode
constructor tags directly in the pointer word, avoiding separate tag words and enabling
unboxed representations of algebraic data types.

The technique defines three boxity classes for type representations:

- **`box`**: Traditional boxed. Values are pointers to heap blocks with a tag word. All
  64 bits of the pointer word carry the address. This is the conservative fallback.
- **`lub` (low-unboxed)**: Constructor tags are encoded in the *low bits* of the pointer
  (the bits freed by alignment). This is the classical bit-stealing technique used for
  list cons/nil discrimination. Low-unboxed values occupy the same number of bits as
  pointers.
- **`hub` (high-unboxed)**: Constructor tags are encoded in the *high bits* of the pointer
  (the bits unused by the virtual address space). High-unboxed values may occupy all 64
  bits of a machine word. With H=16 available high bits on x86_64, up to 2^16 + n
  constructors can be accommodated (where n is the number of argument-taking constructors
  that fit in the remaining bits).

The compiler infers boxity decisions via a fixpoint algorithm over mutually recursive
data-type declarations. The algorithm first attempts the most optimistic boxity (`hub`
for the whole group), checks well-formedness, and falls back to `box` if needed. The
key well-formedness rules are:

- **[HUB]**: A type with boxity `hub` can have m argument-taking constructors (each with
  argument types that have boxity `lub` or better) and n nullary constructors, provided
  m + n < 2^H. The argument types must fit in the remaining bits after the high tag.
- **[LUB]**: A type with boxity `lub` can have at most one argument-taking constructor
  (whose argument is `box`) and any number of nullary constructors.
- **[ENUM]**: A type with only nullary constructors gets boxity `lub` (pure enumeration).
- **[BOX]**: The conservative fallback. Any type can be `box`.

**Constructors are classified by arity.** The paper treats all non-nullary constructors
as "unary" --- a multi-argument constructor `C of a * b * c` is understood as taking a
single tuple argument `C of (a * b * c)`. This is a representation choice (the MLKit
compiler follows this convention), not a fundamental limitation. The distinction between
nullary and unary-with-argument is the primary classification axis.

### Performance results

Benchmarks on the MLKit Standard ML compiler show:

| Benchmark | Speedup (vs low-only) | Memory decrease |
|-----------|----------------------|-----------------|
| `uf` (union-find) | 26.3% | 23.9% |
| `patricia` | 23.8% | 19.5% |
| `logic` | 17.5% | 5.6% |
| `calc` | 3.4% | 7.7% |
| `vliw` | 1.8% | 20.4% |
| `lexgen` | 3.2% | 14.5% |
| `kbc` | 1.4% | 15.1% |
| `DLX` | 2.4% | 0% |
| `nucleic` | 0% | 0% |

Compiling the MLKit compiler itself: 9.7% speedup, 20.2% memory reduction.
Compiling MLton: 9.5% speedup, 11.5% memory reduction.

The technique **never** produces slowdowns. The gains come from: eliminated tag-word
allocations, fewer indirections, reduced memory footprint, and improved cache behavior.
The largest wins occur in data-structure-heavy programs (union-find, Patricia trees) where
ADT constructors dominate allocation.

### GC/RC interaction

The paper is implemented in MLKit, which uses region-based memory management with
optional reference-tracing GC. The technique preserves the GC invariant by ensuring
tagged pointers remain distinguishable: low-tagged pointers use the alignment bits
(GC already knows about this for list cons/nil), and high-tagged pointers use bits the
GC knows are always zero in valid heap pointers. The paper states this "can be safely
integrated with garbage collection."

For reference counting specifically: the tag bits must not interfere with RC field
access. Since the RC header is accessed via the pointer (after masking off tag bits),
the masking step is required before any dereference. This is a codegen concern, not
an analysis concern.

### What the paper does NOT address

The paper does not discuss interaction with reuse analysis, uniqueness types, or
ownership. Boxity decisions are made purely from type declarations (constructor arity,
payload types, mutual recursion structure). No dataflow analysis is involved. The
inference is a type-level fixpoint, not a program-point analysis.

---

## 12.2 What AIMS Should Adopt

### Keep

AIMS should expose sufficient information in its output artifacts for a downstream
representation optimizer to make boxity/unboxing decisions. The specific facts needed:

1. **Constructor arity and payload classification.** A repr optimizer needs to know, for
   each constructor of a sum type: (a) how many fields, (b) whether each field is scalar
   or ref-counted, (c) the field's type size. Today `ShapeClass` tracks `ReusableCtor(kind)`
   where `kind` is `Struct` or `EnumVariant`, but does not carry arity or field-type
   information. This is fine --- AIMS should NOT carry repr-level detail in its lattice.
   But the *type registry* (Pool, ArcClassifier) must be queryable for constructor metadata
   after AIMS completes, and the pipeline must preserve this access path into Stage 4.

2. **Allocation-site hotness.** Elsman's benchmarks show the biggest wins on
   data-structure-heavy code. A repr optimizer benefits from knowing which constructors
   are allocated in hot loops. AIMS's `Locality` and `Cardinality` dimensions provide
   proxies: a constructor that is `BlockLocal` + `Many` (allocated many times in a tight
   scope) is a strong unboxing candidate. AIMS should ensure these facts survive into
   the post-analysis artifact consumed by Stage 4.

3. **Return uniqueness at type boundaries.** Elsman's boxity inference operates on
   data-type declarations (type-level). But in Ori, AIMS has richer information:
   `MemoryContract` knows whether a function returns a unique value. If a freshly
   constructed ADT value is provably unique at its creation site, a repr optimizer knows
   it can safely apply an unboxed representation without worrying about aliased pointers
   carrying different tag encodings. AIMS should preserve `ReturnContract.uniqueness` in
   the Stage 4 interface.

4. **Enum-variant-count metadata.** The boxity inference algorithm's key input is the
   number of constructors (nullary vs argument-taking) for each sum type. This is purely
   type-level information that already exists in Ori's type registry. AIMS does not need
   to compute or carry this --- but the Stage 4 interface should document that the repr
   optimizer will query it from the Pool/type system, not from AIMS.

### New Invariants

**Boundary invariant: AIMS analysis is representation-agnostic.** The 7-dimensional
lattice, transfer functions, canonicalize rules, and all convergence properties must
remain independent of how values are physically represented. AIMS reasons about
*ownership semantics* (who owns it, how many times it is used, whether it escapes),
not *bit layouts* (where the tag is, how many bits the pointer occupies).

This means:
- `AimsState` must never acquire a "representation" dimension or a "boxity" field.
- `ShapeClass` distinguishes *allocation shapes for reuse* (struct vs enum-variant vs
  collection), not *encoding strategies* (boxed vs low-unboxed vs high-unboxed).
- `SizeClass` measures *allocation size for reuse compatibility*, not *word-level encoding
  width*.
- Transfer functions must never branch on representation. An `RcInc` on a `hub`-encoded
  value and an `RcInc` on a `box`-encoded value are the same operation from AIMS's
  perspective. The pointer-masking to access the RC header is a codegen detail.

**Boundary invariant: repr-opt consumes AIMS, never feeds back.** The data flow is
strictly one-directional:

```
Type declarations (arity, fields, mutual recursion)
  + AIMS converged state (uniqueness, locality, cardinality, shape)
  --> Repr optimizer (boxity decisions, tag encoding, unboxing)
  --> Codegen (pointer masking, tag extraction, RC with masked pointers)
```

If a repr decision needs to feed back into AIMS (e.g., "this value is now unboxed, so
it is effectively scalar"), that feedback should happen via `ArcClass` reclassification
*before* AIMS runs, not as a mid-analysis mutation. Concretely: if a repr optimizer
determines that `Option<int>` can be represented as a tagged integer (no heap
allocation), the correct action is to classify it as `ArcClass::Scalar` during
`compute_var_reprs`, so AIMS never sees it as a ref-counted value. This preserves
the one-directional flow.

---

## 12.3 What AIMS Should Not Adopt

### Reject

1. **Boxity inference inside AIMS.** Elsman's boxity inference is a type-level fixpoint
   over data-type declarations. It examines constructor arity, payload types, and mutual
   recursion structure. It does not need or use any program-point analysis, ownership
   tracking, liveness, or cardinality information. Placing it inside AIMS would violate
   the "derived from `AimsStateMap` + `MemoryContract` alone" litmus test (Section 00
   of the AIMS plan). Boxity inference belongs in a pre-AIMS type-level pass or in a
   post-AIMS Stage 4 optimizer.

   **Where it belongs:** Either in `ori_arc/src/repr/` (new module, pre-pipeline) or in
   a future `ori_repr` crate. It should run after type checking but before ARC lowering,
   so that `compute_var_reprs` and `ArcClassifier` can consume the boxity decisions.
   Alternatively, it can run post-AIMS in Stage 4 as a representation refinement pass
   that re-classifies variables.

2. **Tag encoding logic.** Low-bit masking, high-bit shifting, BIBOP page classification,
   and pointer-tag composition are codegen concerns. They belong in `ori_llvm`'s
   `arc_emitter` (or a new `repr_emitter`), not in `ori_arc/src/aims/`.

3. **Constructor-kind refinement in `ShapeClass`.** The paper classifies constructors as
   nullary vs unary (single-argument). It would be tempting to add
   `ShapeClass::NullaryCtor` or `ShapeClass::UnaryCtor(boxity)` to AIMS. This is wrong.
   `ShapeClass` is a flat lattice for *reuse compatibility*. Adding repr-level detail
   would increase the lattice height, complicate join, and couple AIMS to representation
   strategy. Constructor arity is type-level metadata, not a per-program-point analysis
   fact.

4. **Platform-specific bit counts.** Elsman's H=16 (available high bits on x86_64) and
   3-4 low bits (from alignment) are architecture-dependent constants. AIMS must remain
   target-independent. Platform-specific constants belong in the repr optimizer's
   configuration, not in the lattice.

5. **Unboxing as a `Consumption` or `Uniqueness` refinement.** One might argue that if a
   value is provably unboxed (no heap allocation), its `AccessClass` should be `Borrowed`
   (no RC obligation). This conflates representation with ownership. The correct path is
   `ArcClass::Scalar` reclassification before AIMS, not an AIMS-internal shortcut.

---

## 12.4 Plan Edits

### Stage 4 scope (`plans/aims/00-overview.md`, lines 404-408)

Current text is deliberately vague:
```
Stage 4 -- Locality Realization + Representation
  - Use Locality facts to produce backend hints for stack or local allocation
  - Representation optimization consuming AIMS shape/locality facts
  - Keep hint-based first; do not redesign ARC IR around stack allocation yet
  Deliverable: LLVM may consume locality hints, representation optimizer has data
```

**Recommended additions** (not changes --- Stage 4 is future work, keep it brief):
- Add a bullet: "Boxity inference (Elsman ICFP 2024) as a pre-pipeline or post-analysis
  pass consuming type-level constructor metadata + AIMS uniqueness/locality facts"
- Add a bullet: "Reclassification feedback: repr optimizer may reclassify unboxed ADTs
  as `ArcClass::Scalar` in a second pipeline run, or via a pre-AIMS repr pass that
  adjusts `compute_var_reprs` output"
- Add a bullet: "Platform-specific tag-bit constants (H=16 on x86_64, alignment bits)
  belong in repr optimizer config, not in AIMS"

### Section 07.4 (`plans/aims/section-07-advanced.md`, lines 389-401)

The existing bullet on Representation Optimization is accurate but light. Recommended
additions to the AIMS prerequisite paragraph:

- Document the one-directional data flow: type declarations + AIMS facts --> repr
  optimizer --> codegen. No feedback into AIMS analysis.
- Note that `AimsStateMap` is currently consumed during emission and not preserved. A
  Stage 4 repr optimizer needs either: (a) preserved state map, or (b) a function-level
  summary materialized during emission. Option (b) is preferred (smaller footprint,
  cleaner API). The summary should include: per-variable uniqueness at construction
  site, per-variable locality, per-constructor allocation frequency proxy (from
  cardinality).
- Add "constructor arity and payload-type metadata" to the list of facts the repr
  optimizer needs, noting these come from the type registry (Pool), not from AIMS.

### `ShapeClass` extensibility (`compiler/ori_arc/src/aims/lattice/dimensions.rs`)

No changes needed now. `ShapeClass` is correctly scoped for reuse compatibility, not
representation. The flat lattice with `NonReusable` as top is the right design. If
Stage 4 needs a richer shape, it should define its own `ReprShape` type that reads
`ShapeClass` as one input, not extend `ShapeClass` itself.

### `SizeClass` and alignment (`compiler/ori_arc/src/aims/lattice/mod.rs`)

`SizeClass` currently stores a `u32` byte count. The paper raises the question of
whether alignment should be tracked alongside size. Answer: **not in AIMS.** `SizeClass`
exists for cross-type reuse matching (Stage 2+). Alignment is a repr-level property
consumed by the repr optimizer and codegen. If a future `ReprSizeClass` needs
`(size, alignment)`, it should be defined in the repr module, not in the AIMS lattice.

---

## 12.5 Code Changes (Later)

All items below are Stage 4+ work. None should be implemented during Stage 1 or Stage 2.

1. **`AimsStateSummary` export** (new type, `aims/mod.rs` or `aims/summary.rs`):
   - Materialized during emission (Section 10 unified realization) as a compact
     per-function summary of AIMS facts relevant to downstream consumers.
   - Fields: per-constructor-site uniqueness (`Unique`/`MaybeShared`/`Shared`),
     per-constructor-site locality (`BlockLocal`/`FunctionLocal`/`HeapEscaping`),
     per-variable cardinality at key points.
   - Stored on `ArcFunction` after emission, consumed by Stage 4 repr optimizer.
   - This avoids preserving the full `AimsStateMap` past emission.

2. **Pre-pipeline boxity pass** (new module, `ori_arc/src/repr/` or separate crate):
   - Input: type declarations from Pool (constructor count, field types, mutual
     recursion groups).
   - Input: platform config (available high bits H, alignment bits).
   - Output: `BoxityDecision` per type (`Hub`/`Lub`/`Box`/`Enum`/`Single`).
   - Consumed by `compute_var_reprs` to reclassify unboxed ADTs as `ArcClass::Scalar`.

3. **Codegen support** (`ori_llvm/src/codegen/arc_emitter/`):
   - Pointer masking before RC header access (mask off low/high tag bits).
   - Tag extraction/insertion instructions for pattern matching on tagged pointers.
   - Modified `Construct` emission for unboxed constructors (inline tag, no heap alloc).
   - Modified `Project` emission for unboxed fields (mask and shift, no GEP).

4. **`SizeClass` remains unchanged.** If the repr optimizer needs `(size, alignment)`,
   define a separate `ReprLayout` type. Do not extend `SizeClass`.

5. **`ShapeClass` remains unchanged.** If the repr optimizer needs constructor arity or
   boxity, define a separate `ReprShape` type that consults the type registry and
   `ShapeClass` together. Do not extend `ShapeClass`.

---

## 12.6 Lens Shift

This is the final paper in the 12-paper review sequence. The cumulative lens shift
across all 12 papers reshapes AIMS along these axes:

**1. AIMS is a fact-proving system, not an optimization system.**
Papers 01-05 (OxCaml, FP2, FIPTree, TRMC, Perceus-for-OCaml) established that AIMS
proves ownership, demand, locality, and shape facts. Papers 06-09 (linearity/uniqueness,
QTT, Lean4, GHC demand) provided the theoretical grounding for each lattice dimension.
Papers 10-11 (concurrent RC, cyclic RC) showed that AIMS must preserve interface
assumptions for future runtime strategies. Paper 12 (bit-stealing) completes the picture:
AIMS proves facts that downstream passes --- repr optimization, codegen, runtime ---
consume. AIMS itself does not optimize; it enables optimization by collapsing what would
otherwise be separate analyses into one converged state.

**2. The Stage 4 boundary is now well-defined.**
Before the review, Stage 4 ("Locality Realization + Representation") was deliberately
vague. After 12 papers, the boundary is sharp:
- AIMS proves: ownership, demand, uniqueness, locality, shape, effect (7 dimensions).
- Stage 4 repr optimizer consumes: type-level constructor metadata (arity, fields,
  mutual recursion) + AIMS's per-variable uniqueness/locality/cardinality + platform
  config (bit counts, alignment).
- Stage 4 repr optimizer produces: boxity decisions, tag encoding strategy, unboxing
  classification.
- Codegen consumes: repr decisions + AIMS RC/reuse/COW annotations.
- Feedback: repr decisions may reclassify some types as `ArcClass::Scalar`, triggering
  a second AIMS pass or (better) a pre-AIMS repr pass that adjusts inputs before the
  single AIMS pass runs.

**3. `ShapeClass` and `SizeClass` are correctly scoped.**
Multiple papers (FP2 for reuse credits, FIPTree for context holes, TRMC for tail
contexts, bit-stealing for boxity) could have motivated expanding these types. The
review confirms they should stay narrow: `ShapeClass` for reuse compatibility,
`SizeClass` for allocation size matching. Repr-level detail belongs in a separate
`ReprShape`/`ReprLayout` that reads AIMS outputs as one of several inputs.

**4. AIMS's target-independence is load-bearing.**
Bit-stealing depends on x86_64 canonical addresses (H=16) and platform alignment (3-4
bits). AIMS must remain target-independent so the same analysis serves all backends
(LLVM x86, LLVM ARM, future WASM). Platform-specific constants live in repr config or
codegen, never in the lattice.

**5. The one-directional data flow is sacrosanct.**
Every paper that touches representation (bit-stealing, FIPTree contexts, TRMC rewrites,
OxCaml stack allocation) reinforces the same architecture: analysis proves facts,
downstream passes consume them. No downstream pass feeds back into the analysis mid-run.
Pre-analysis normalization (TRMC, Stage 3) and pre-analysis reclassification (repr-driven
`ArcClass::Scalar`) are the correct feedback mechanisms, not analysis-internal mutations.

**6. The lattice height (15) and dimension count (7) need not change.**
None of the 12 papers motivates adding an 8th dimension or increasing the height of any
existing dimension. Papers 01-05 validated the dimension decomposition. Papers 06-09
validated the theoretical grounding of each dimension's order. Papers 10-12 validated
that downstream concerns (concurrent RC, cyclic RC, representation) should be consumers,
not additional dimensions. The 7D product lattice with chain height 15 is stable.

---

## 12.7 Open Risk

1. **`AimsStateMap` lifetime.** Currently, the state map is consumed during emission and
   not preserved. Section 07.4 already flags this. A Stage 4 repr optimizer needs
   per-variable facts from the converged analysis. The risk is that emission destroys
   this information before repr-opt can consume it. Mitigation: define an
   `AimsStateSummary` type materialized during emission (Section 12.5 item 1), or
   restructure the pipeline so repr-opt runs as part of Phase C realization (Section 10)
   before the state map is dropped.

2. **Reclassification feedback loop.** If the repr optimizer determines that a type is
   unboxed (e.g., `Option<int>` becomes a tagged integer), it should be reclassified as
   `ArcClass::Scalar`. But AIMS has already run with the old classification. A second
   AIMS pass is wasteful. The cleaner design is a pre-AIMS repr pass that adjusts
   `compute_var_reprs` output, but this creates a chicken-and-egg: the repr optimizer
   wants AIMS facts (locality, uniqueness), but AIMS wants repr decisions (scalar vs
   ref). Resolution: run repr inference in two tiers. Tier 1 (pre-AIMS) uses only
   type-level information (constructor arity, payload types) to make conservative boxity
   decisions and reclassify obvious scalars. Tier 2 (post-AIMS, Stage 4) uses AIMS facts
   to refine remaining decisions. This mirrors Elsman's algorithm, which uses only
   type-level info and does not need dataflow analysis.

3. **Cross-module boxity.** Elsman notes that MLKit does not split mutually recursive
   data-type declarations into SCCs for boxity inference, which can cause suboptimal
   decisions. Ori's type checker already computes SCCs for type declarations. A future
   repr optimizer should reuse this SCC structure, not recompute it. Ensure the SCC
   information is accessible from the repr optimizer's position in the pipeline.

4. **Polymorphic types.** Elsman's boxity inference assigns `hub` boxity to type variables
   (ensuring no tag bits are used for the type parameter's representation, since the
   parameter could be instantiated to any type). Ori has monomorphization, which
   eliminates type variables before ARC analysis. Post-monomorphization, every type is
   concrete, and boxity decisions can be made per-monomorphized-instance. This is
   strictly more powerful than Elsman's approach (which must be conservative for
   polymorphic types). Risk: monomorphization may produce many instances of the same
   generic type with different boxities, increasing code size. Monitor this if repr-opt
   is implemented.

5. **AIMS design choices that may interfere with future repr optimization.** The main
   risk is `ShapeClass`'s flat lattice join: any two distinct non-`NonReusable` shapes
   join to `NonReusable`. If a variable holds either a `ReusableCtor(Struct)` or a
   `ReusableCtor(EnumVariant)` depending on control flow, the join loses all shape
   information. A repr optimizer might want to know "this is definitely a constructor
   of type T" even after the join. Since AIMS tracks shape per-variable-per-block (not
   per-type), this is inherent to the per-program-point design. The repr optimizer
   should query type-level metadata (which is stable) rather than relying on
   per-program-point `ShapeClass` for boxity decisions. This is already the recommended
   architecture (type-level boxity inference, not dataflow-based).
