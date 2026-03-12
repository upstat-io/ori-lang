---
section: "07"
title: "Advanced Optimizations"
status: in-progress
goal: "Implement optimizations enabled by the unified lattice that were impossible with separate passes"
inspired_by:
  - "Biased RC / PEP 703 (immortal objects)"
  - "Coalesced RC (Levanoni-Petrank, TOPLAS 2006)"
  - "Morphic whole-program mutability inference"
  - "Perceus reuse specialization"
  - "RC Deeply Immutable Cycles (Parkinson et al., ISMM 2024)"
  - "CIRC (Jung et al., PLDI 2024)"
  - "Double-Ended Bit-Stealing (Elsman, ICFP 2024)"
depends_on: ["06"]
sections:
  - id: "07.1"
    title: "Immortal Objects"
    status: complete
  - id: "07.2"
    title: "Static RC Coalescing"
    status: complete
  - id: "07.3"
    title: "Cross-Optimization Synergies"
    status: complete
  - id: "07.4"
    title: "Future: Runtime and Representation Follow-ons"
    status: complete
  - id: "07.5"
    title: "Completion Checklist"
    status: complete
---

# Section 07: Advanced Optimizations

**Status:** Complete
**Goal:** Implement optimizations that are only possible (or significantly easier)
with the unified AIMS lattice, demonstrating the architectural advantage over
separate passes.

**Context:** These optimizations are the "stretch goals" that justify AIMS beyond
architectural cleanliness. Each one either requires cross-dimensional information
(impossible with separate passes) or becomes trivial with the unified state map
(awkward to retrofit into the current pipeline).

**Note:** TRMC and constructor contexts have been moved out of this section. They
are now part of the **Opportunity Creation** stage (Stage 3 in the implementation
sequence), to be implemented in `aims/normalize/` (not yet created). Basic TRMC
normalization is a pre-analysis pass, not a post-pipeline optimization.

**Depends on:** Section 06 (working AIMS pipeline).

---

## 07.1 Immortal Objects

**File(s):** `compiler/ori_arc/src/aims/immortal/mod.rs`,
`compiler/ori_arc/src/aims/emit_rc/mod.rs`, `compiler/ori_rt/src/rc/mod.rs`

Mark common constants as immortal (RC = MAX), skipping all RC operations on them.

### 07.1.1 Immortal Set Data Structure and Pipeline Threading

- [x] Define immortal detection in `compiler/ori_arc/src/aims/immortal/mod.rs`:
  ```rust
  /// Set of variables whose allocations are immortal (RC = MAX_REFCOUNT).
  /// Immortal allocations never participate in RC operations — inc, dec,
  /// and COW checks are all skipped. The set is populated once during
  /// `compute_var_reprs` (pipeline step 3) and threaded read-only through
  /// all subsequent pipeline steps.
  pub struct ImmortalSet {
      vars: FxHashSet<ArcVarId>,
  }
  ```
- [x] Populate `ImmortalSet` during `compute_var_reprs` (pipeline step 3, Section 06.2).
  After `var_reprs` is filled, scan all `Let` instructions for immortal-eligible
  literal values and add their `dst` to the immortal set. The immortal set is
  computed ONCE per function, before analysis (step 5).
- [x] Thread `ImmortalSet` through `AimsPipelineConfig`:
  - Add `immortal: ImmortalSet` field to `AimsPipelineConfig` (Section 06.3)
  - Pass to `analyze_function()` (step 5) — immortal vars are excluded from
    analysis entirely, same as `SCALAR` variables in `AimsStateMap.scalars`
  - Pass to `emit_rc_ops()` (step 6) — checked as pre-filter before state map lookup
  - Pass to `emit_reuse()` (step 7) — immortal values are never reuse candidates
  - Pass to `emit_cow_annotations()` (step 11a) — immortal values get no COW annotation
  - Pass to `emit_drop_hints()` (step 12) — immortal values get no drop hints

### 07.1.2 Shadow Comparison Mode Interaction

- [x] In shadow comparison mode (`aims-shadow` feature, Section 06.1):
  - The immortal set is a Stage 5 optimization. During Stage 1 shadow comparison,
    the immortal set should be EMPTY (no immortal objects detected). This ensures
    shadow comparison compares equivalent pipelines.
  - When immortal objects are enabled (Stage 5), shadow comparison must account for
    the RC operation count difference: immortal objects will show FEWER RC ops in
    AIMS than in the legacy pipeline. These should be logged as `DimensionResult::Improvement`,
    not as mismatches. Add an `immortal_skips: usize` counter to `FunctionComparison`
    (in `pipeline/shadow.rs`).

### 07.1.3 Immortal Value Identification

- [x] Implement immortal detection as an **extension hook outside `AimsState`**,
  not a modification to the core lattice. Immortality is a global property of the
  allocation (determined at lowering time from the value's definition site), not a
  per-program-point analysis fact. The correct representation is a separate
  `FxHashSet<ArcVarId>` of immortal variables, consulted by RC emission as a
  pre-filter (before checking `AimsState`). This avoids reshaping the core lattice
  for a Stage 5 optimization:
  - `AimsState` struct, chain height (15), join, and transfer functions are unchanged
  - Immortal variables are excluded from analysis entirely (same treatment as `SCALAR`)
  - RC emission checks the immortal set first, then falls through to normal
    `AimsState`-based emission for non-immortal variables
  - The immortal set is populated during lowering or `compute_var_reprs`, not
    during AIMS analysis
- [x] Identify immortal values:
  - Boolean literals (`true`, `false`) — already `ArcClass::Scalar`, no RC needed
  - Small integer constants (0-255) — already `ArcClass::Scalar`, no RC needed
  - Empty string literal `""` — currently `ArcClass::DefiniteRef` (str is heap-allocated).
    Making this immortal requires runtime support for an immortal empty string singleton.
  - Unit value `()` — already `ArcClass::Scalar`
  - `None` value — may or may not be scalar depending on the Option's type parameter.
    `Option<int>` → Scalar. `Option<str>` → DefiniteRef. Only pure `None` (no RC
    payload) can be immortal.
  - **Net new immortal candidates** (values that are currently `DefiniteRef` and would
    benefit from immortal treatment): empty string `""`, static string literals used
    as constants (e.g., error messages), empty list `[]`, empty map `{}`. The boolean/int/unit
    candidates above are already `ArcClass::Scalar` and already skip RC — immortal
    treatment adds no benefit for them. Focus implementation on heap-allocated literals.
- [x] In emit_rc: skip `RcInc`/`RcDec` for immortal values

### 07.1.4 Runtime Support

- [x] **Sentinel value decision**: `ori_rc_inc` currently checks `MAX_REFCOUNT`
  (`isize::MAX`, i.e., `i64::MAX` on 64-bit) and **aborts** via `rc_overflow_abort()`.
  For immortal objects, we need RC operations to be no-ops. Two options:
  1. **Reuse `MAX_REFCOUNT` as immortal sentinel** (simpler): Change `ori_rc_inc` and
     `ori_rc_dec` to skip (no-op) when refcount equals `MAX_REFCOUNT`. Pro: no new
     constant, existing pre-allocation code just sets RC to `MAX_REFCOUNT`. Con: loses
     the overflow detection safety net (a genuine overflow bug would silently become
     immortal instead of aborting). In practice, reaching `MAX_REFCOUNT` through
     increments is physically impossible (would require ~9 quintillion increments), so
     the safety net is theoretical.
  2. **Separate `IMMORTAL_REFCOUNT` sentinel** (safer): Define `IMMORTAL_REFCOUNT = MAX_REFCOUNT - 1`
     (or a distinct negative sentinel). `ori_rc_inc`/`ori_rc_dec` check for this value
     and skip. Overflow detection at `MAX_REFCOUNT` is preserved. Con: two sentinel
     checks per RC operation instead of one.
  **Recommended: Option 1** (reuse `MAX_REFCOUNT`). The overflow case is unreachable
  in practice, and the single check is simpler and cheaper.
- [x] Runtime changes in `compiler/ori_rt/src/rc/mod.rs`:
  - `ori_rc_inc`: change `if prev == MAX_REFCOUNT { rc_overflow_abort() }` to
    `if prev == MAX_REFCOUNT { return; }` (skip, don't abort)
  - `ori_rc_dec`: add check at entry: `if current_rc == MAX_REFCOUNT { return; }`
    (skip decrement, don't free)
  - Single-threaded path (`#[cfg(feature = "single-threaded")]`): same changes
  - Update `rc_overflow_aborts_process` test in `compiler/ori_rt/src/tests.rs` to
    verify skip behavior instead of abort behavior
- [x] Immortal empty string codegen optimization:
  - SSO makes empty strings stack values (no heap, no RC) — `OriStr::EMPTY` is a
    compile-time constant. No heap-allocated `*mut u8` singleton needed.
  - `ori_str_empty()` already exists in `ori_rt` and returns `OriStr::EMPTY`.
  - **LLVM emitter updated**: `compiler/ori_llvm/src/codegen/arc_emitter/value_emission.rs`
    now calls `ori_str_empty()` for empty string literals instead of creating a global
    constant and calling `ori_str_from_raw(ptr, 0)`. Always active (not feature-gated),
    since the optimization is unconditionally correct and beneficial.

### 07.1.5 Tests

- [x] **Rust unit tests** (`compiler/ori_arc/src/aims/immortal/tests.rs`):
  - `immortal_set_detects_empty_string_literal`
  - `immortal_set_ignores_non_literal_refs`
  - `immortal_set_ignores_scalar_types` (already handled by SCALAR fast path)
  - `immortal_vars_excluded_from_analysis` (no AimsState entry in state map)
  - `immortal_vars_skip_rc_emission` (no RcInc/RcDec emitted)
  - `immortal_vars_skip_cow_annotations` (no CowAnnotation entry)
  - `immortal_vars_skip_drop_hints` (no DropHint entry)
- [x] **Runtime tests** (`compiler/ori_rt/src/tests.rs`):
  - `rc_inc_skips_at_max_refcount` (replaces or extends `rc_overflow_aborts_process`)
  - `rc_dec_skips_at_max_refcount`
  - `immortal_empty_string_has_max_refcount`
- [x] **Integration tests** (`compiler/ori_llvm/tests/aot/string_sso.rs`):
  - `test_immortal_empty_string_no_leak` — multiple empty strings, concatenation,
    equality, leak detection via `ORI_CHECK_LEAKS=1`
  - `test_immortal_empty_string_passed_to_function` — empty string passed to callee
  - `test_immortal_empty_string_in_collection` — empty strings in list with iteration

---

## 07.2 Static RC Coalescing

**File(s):** `compiler/ori_arc/src/aims/emit_rc/mod.rs`

When a value undergoes multiple RC changes in a sequence (e.g., inc then dec from
different operations), only the net effect matters.

### 07.2.1 Relationship to Existing Passes

**How this differs from AIMS Stage 1 emit_rc:** AIMS Stage 1 already emits only
the RC operations the analysis deems necessary — it does not insert-then-remove
like the old pipeline. However, AIMS Stage 1 emits RC operations **one at a time**
as it encounters use/death transitions during the forward walk. Static coalescing
is a **post-emission peephole** that merges adjacent RC operations on the same
variable within a basic block:
- Two `RcInc(x, count: 1)` emitted for consecutive uses → merge into `RcInc(x, count: 2)`
- An `RcInc(x)` immediately followed by `RcDec(x)` with no intervening alias → cancel both
- Adjacent `RcDec(x); RcDec(y)` where x and y are independent → reorder for better
  instruction scheduling (not a correctness change, but aids CPU pipelining)

**How this differs from old `rc_elim`:** The old `rc_elim` (`rc_elim/eliminate.rs`)
runs a dataflow analysis over the **already-emitted** IR to find redundant inc/dec
pairs across basic blocks. It is a heavyweight pass (439 lines, requires its own
forward dataflow). Coalescing here is a lightweight **within-block peephole** that
runs during or immediately after AIMS emission, with O(n) complexity per block.
Cross-block redundancy elimination is not needed because AIMS already avoids
emitting cross-block redundant operations from the analysis.

### 07.2.2 Implementation

**Implementation strategy: Enhancement to `emit_rc_ops`, NOT a separate pass.**
Add a post-emission coalescing step at the end of `emit_rc_ops()` (step 6 in
Section 06.2) that scans each block's instruction list and merges adjacent RC
operations. This runs before reuse emission (step 7) and before block_merge (step 11).

- [x] Detect coalesceable sequences:
  - `RcInc(x)` followed by `RcDec(x)` with no intervening alias → cancel both
  - `RcInc(x); RcInc(x)` → single `RcInc(x, count: 2)` (confirmed: `ArcInstr::RcInc`
    has `count: u32` field — see `ir/instr.rs:78`)
  - `RcDec(x)` followed by `Construct` reusing x → fuse into `Reset`
    (this case is largely handled by reuse emission in Section 05, but coalescing
    can catch cases where reuse emission did not match the pair)

- [x] With the unified state map, coalescing is trivial:
  - Count the net RC change per variable across a basic block
  - Emit the net result instead of individual operations
  - Current `rc_elim` does this but only within a single block and after both
    RC insertion and reuse expansion (last pass in the legacy pipeline)
  - **Ordering constraint:** Net-effect coalescing is only valid within straight-line
    sequences with no intervening calls, aliases, or control flow. RC operations around
    call boundaries, drop points, and alias creation points must preserve their ordering.
    The optimization applies to sequences of RcInc/RcDec on the same variable with no
    intervening operations that could observe the reference count.

### 07.2.3 Tests

- [x] **Rust unit tests** (`compiler/ori_arc/src/aims/emit_rc/coalesce/tests.rs`):
  - `coalesce_adjacent_inc_inc_same_var` — two `RcInc(x, 1)` → one `RcInc(x, 2)`
  - `coalesce_inc_dec_cancellation` — `RcInc(x)` then `RcDec(x)` → both removed
  - `no_coalesce_across_call` — `RcInc(x)`, `Apply(...)`, `RcDec(x)` → preserved
    (call may observe refcount)
  - `no_coalesce_across_alias` — `RcInc(x)`, `Let(y, x)`, `RcDec(x)` → preserved
    (alias created between inc and dec)
  - `coalesce_net_effect_multiple_ops` — `RcInc(x); RcInc(x); RcDec(x)` → `RcInc(x, 1)`
  - `coalesce_preserves_strategy` — merged RcInc preserves `RcStrategy` from operands
    (all must have same strategy; if different, do not merge)
- [x] **Integration metric**: Unit tests demonstrate inc-inc merge (2→1 op),
  inc-dec cancellation (2→0 ops), and net-effect (3→1 op). 1255 AOT integration
  tests pass with coalescing active, confirming no regressions.

---

## 07.3 Cross-Optimization Synergies

**File(s):** `compiler/ori_arc/src/aims/intraprocedural/mod.rs`,
`compiler/ori_arc/src/aims/emit_rc/mod.rs`,
`compiler/ori_arc/src/aims/emit_rc/cow.rs`,
`compiler/ori_arc/src/aims/emit_rc/arg_ownership.rs`

Optimizations that emerge from cross-dimensional information in the unified lattice.

**Relationship to Stage 1:** Some cross-dimensional information is already exploited
in Stage 1:
- Uniqueness-preserving borrows: Section 01.3a documents this interaction and
  Section 04.1 reads `BorrowSource` for COW decisions. **Stage 1 already implements
  the analysis infrastructure.** What Section 07.3 adds is **exploitation at emission
  time** — actively using the combined state to eliminate operations the Stage 1
  emitter conservatively preserves.
- Demand-driven RC elimination: Section 03 computes `ParamContract.cardinality`, and
  Section 04 reads it. **Stage 1 already has the data.** What Section 07.3 adds is
  **callee-side dead parameter skipping** (the callee's entry code omits RcDec for
  params it never uses, not just the caller's call-site optimization).

### 07.3.1 COW-aware borrowing

- [x] **COW-aware borrowing**:
  - If a function parameter has `(Owned, Linear, Once)` combined state, the callee
    knows the value is uniquely owned at the COW mutation point — even if uniqueness
    alone is `MaybeShared`. Cross-dimensional reasoning: Owned (received ownership),
    Linear (no duplication), Once (single use) = unique view.
  - **Implementation**: In `uniqueness_to_cow_mode()` (cow.rs), when the receiver is
    a function parameter, check `is_cow_aware_unique()`: if `access == Owned &&
    consumption == Linear && cardinality == Once`, return `CowMode::StaticUnique`
    regardless of the `uniqueness` dimension. Falls through to uniqueness-based
    decision for non-qualifying states.
  - **File**: `compiler/ori_arc/src/aims/emit_rc/cow.rs`
  - **Prerequisite**: The caller must pass the value as `ArgOwnership::Owned` (not
    borrowed). The callee's analysis proves unique ownership from the combined state.

### 07.3.2 Uniqueness-preserving borrows

- [x] **Uniqueness-preserving borrows**:
  - Extended `BorrowSource::Exact` to carry `field: Option<u32>` from `Project`.
    Added `BorrowSource::exact()` / `exact_field()` constructors and `source_var()`.
  - Added `AimsStateMap::borrows_from_source()` reverse lookup.
  - In `uniqueness_to_cow_mode()`, when receiver has `MaybeShared`, calls
    `is_borrow_disjoint_from_siblings()`: checks BorrowSource for field provenance,
    verifies source is `Unique`, and ensures all sibling borrows target disjoint fields.
  - **Soundness guard**: same-field sibling borrow → stays `Dynamic`.
    Whole-object borrow (field=None) → stays `Dynamic`.
  - **Files**: `lattice/mod.rs` (BorrowSource), `transfer/mod.rs` (field propagation),
    `state_map.rs` (reverse lookup), `cow.rs` (disjoint check)

### 07.3.3 Demand-driven RC elimination

- [x] **Demand-driven RC elimination**:
  - **Caller side** (arg_ownership.rs): Added explicit `Cardinality::Absent` check
    alongside existing `Consumption::Dead` check → `Ownership::Borrowed`. The two
    are canonically equivalent (`Dead ↔ Absent`), but the explicit check makes intent
    clear and is defensive.
  - **Callee side** (emit_rc/mod.rs): Already handled — Phase A `emit_dead_at_entry_decs`
    skips `Absent` variables (line 296 check was already in place). No RcDec emitted
    for unused parameters.
  - **Safety**: `Absent` cardinality means zero uses in the callee body. If the parameter
    is passed to a sub-callee, its cardinality would be `Once` or `Many`, not `Absent`.
  - **LLVM emitter**: no changes needed — already handles `Borrowed` correctly.

### 07.3.4 FIP call-site specialization

- [ ] **FIP Conditional call-site optimization:**
  When calling a function with `FipContract::Conditional { requires_unique_params }`,
  and the caller's AIMS analysis proves that all arguments corresponding to
  `requires_unique_params` entries are `Unique`, use the callee's FIP-optimized
  contract (`EffectSummary { may_allocate: false, may_share: false }`) instead
  of the conservative contract at that call site.

  This is FP²'s dynamic embedding: when a `dropru` (drop-with-reuse) operation
  operates on a provably unique binding, the `(dconru_h)` fast path is taken —
  no allocation, no sharing. In AIMS terms: the caller knows the callee will hit
  all static-unique fast paths (no `IsShared` checks needed), so the call site
  can propagate `Unique` through the call and skip defensive RC operations that
  would otherwise be needed for the `MaybeShared` slow path.

  **Implementation:** In `transfer_apply()` / `transfer_invoke()`, when the
  callee's contract has `FipContract::Conditional`, check each argument's
  pre-state uniqueness against `requires_unique_params`. If all required params
  are `Unique`, substitute the callee's effect summary with the FIP-optimized
  version for the purposes of this call site's transfer. This affects:
  - `may_share == false` → uniqueness preserved through the call (Rule from 09.1)
  - `may_allocate == false` → no heap allocation attributed to this call site
  - Combined effect: caller can treat the call as a pure in-place operation
  (See: [Literature Review §02 — FP²](../aims-literature-review/section-02-fp2.md))

### 07.3.5 Tests

- [x] **COW-aware borrowing test** (`compiler/ori_arc/src/aims/emit_rc/tests.rs`):
  - `cow_aware_borrowing_static_unique_for_linear_owned_unique_param` — parameter
    with `(Owned, Linear, Once, MaybeShared)` gets `CowMode::StaticUnique` via
    cross-dimensional reasoning
  - `cow_aware_borrowing_non_param_stays_dynamic` — non-parameter variable with
    same state stays `Dynamic` (optimization is parameter-only)
  - `cow_aware_borrowing_multi_use_param_stays_dynamic` — parameter with
    `(Owned, Unrestricted, Many, MaybeShared)` stays `Dynamic`
- [x] **Uniqueness-preserving borrow test**:
  - `uniqueness_preserving_borrow_disjoint_field_cow_is_static` — `Project(dst, src, field_0)`
    with sibling borrow on `field_1` (disjoint) → `StaticUnique`
  - `uniqueness_preserving_borrow_same_field_cow_is_dynamic` — `Project(dst, src, field_0)`
    with sibling borrow on `field_0` (same) → `Dynamic` (soundness guard)
- [x] **Demand-driven RC elimination test**:
  - `absent_param_no_rc_dec_at_entry` — callee with `Absent` parameter emits no
    `RcDec` for it at function entry
  - `used_param_gets_rc_dec` — contrast test: `Once` parameter DOES get `RcDec`
- [x] **Cross-dimension synergy integration test**:
  - `cross_dimension_synergy_cow_aware_borrowing` — parameter with
    `(Owned, Linear, Once, MaybeShared)` gets `StaticUnique` via cross-dimensional
    reasoning. Neither uniqueness alone (→ Dynamic) nor cardinality alone (no COW
    info) could achieve this — requires all three non-uniqueness dimensions.
- [ ] **FIP call-site specialization test**:
  - `fip_conditional_call_with_unique_args_uses_optimized_contract` — caller passes
    `Unique` arguments to all `requires_unique_params` positions → call site uses
    FIP-optimized effect summary (`may_allocate: false, may_share: false`)
  - `fip_conditional_call_with_maybe_shared_arg_uses_conservative` — caller passes
    `MaybeShared` argument to a required-unique position → conservative contract used
  - `fip_conditional_call_preserves_uniqueness_through_call` — caller's post-call
    uniqueness state reflects FIP guarantee (no sharing occurred)

---

## 07.4 Future: Runtime and Representation Follow-ons

These are independent efforts that consume AIMS facts but should not block the
core AIMS replacement. They belong in Stage 5 of the implementation sequence.

**Note:** The items below are **informational design notes**, not implementation
tasks for Section 07. They document how AIMS facts could be consumed by future
work. They do NOT have checkboxes because they are not part of Section 07's
deliverable scope. If any of these become concrete plans, they should get their
own plan documents (not section checkboxes here).

- **Whole-Program Mutability** (Morphic):
  - Cross-function uniqueness tracking beyond SCC boundaries
  - Track value flow from creation to final consumption across entire program
  - Requires whole-program compilation (incompatible with separate compilation)
  - **AIMS prerequisite**: `MemoryContract` must be stable and well-documented as
    the cross-function interface. Currently defined in `aims/contract/mod.rs`.

- **SCC-Frozen Cyclic RC** (Parkinson et al., ISMM 2024):
  - For deeply immutable frozen graphs, reference counting can be lifted to the
    level of strongly connected components
  - Relevant only if Ori later adds explicit freeze or immutable cyclic heaps
  - Keep as a future extension note
  - **AIMS prerequisite**: `ShapeClass` and `Locality` dimensions must be precise
    enough to identify frozen/immutable subgraphs. Stage 1 conservative defaults
    are insufficient — this needs Stage 4+ locality precision.
  (See: [Literature Review §11 — Cyclic RC](../aims-literature-review/section-11-cyclic-rc.md))

- **Concurrent RC Strategies** (CIRC — Jung et al., PLDI 2024):
  - Combines SMR-style deferral with RC, immediately applies decrements,
    defers only reclamation
  - Not core to current AIMS unless Ori commits to concurrent shared heaps
  - Preserve `RcStrategy` / runtime hook abstraction so the compiler can later
    target concurrent RC primitives
  - **RC API boundary**: The sole interface between compiler-generated code and
    the RC mechanism is the set of `ori_rt` extern "C" functions: `ori_rc_inc`,
    `ori_rc_dec`, `ori_rc_is_unique`, `ori_rc_is_unique_or_null`. Swapping RC
    implementations (atomic, non-atomic, biased, CIRC) requires only relinking
    `ori_rt` -- zero changes to `ori_arc`, `ori_llvm`, or AIMS analysis.
  - **Analysis abstraction invariant**: AIMS must not embed concrete
    refcount-value assumptions. The lattice dimensions (`Uniqueness`,
    `Cardinality`, `Consumption`, `AccessClass`) reason about abstract
    ownership properties. AIMS never reads `ori_rc_count()` during analysis and
    never assumes "if uniqueness is Unique, then refcount == 1." This
    abstraction is compatible with CIRC, where uncounted `Snapshot` references
    do not affect the reference count.
  - **Drop functions as recursive-reclamation hooks**: The `drop_fn` parameter
    on `ori_rc_dec` already implements CIRC's `RcObject::pop_edges()` pattern.
    The compiler generates per-type drop functions that traverse RC'd child
    fields and call `ori_rc_dec` on each, producing exactly the recursive
    reclamation chain CIRC requires. No separate `pop_edges()` API needed.
  - **`RcStrategy` concurrent compatibility**: The `RcStrategy` enum
    (`ir/repr.rs`) describes value *shape* (HeapPointer, FatPointer, Closure,
    AggregateFields, InlineEnum), not RC *mechanism*. A concurrent RC backend
    would use the same `RcStrategy` values -- it still needs to know how to
    extract the data pointer regardless of how the reference count is
    manipulated.
  - **Explicit exclusions from AIMS core**: Epoch-based reclamation (EBR),
    uncounted `Snapshot`-style references in the compiler IR, deferred-decrement
    buffering, and `AtomicRc`-style compare-and-swap are all excluded from AIMS
    core. These are runtime or IR-level concerns that belong in Stage 5 follow-on
    work, not in the analysis pipeline.
  - **AIMS prerequisite**: `RcStrategy` enum must remain extensible. Currently
    defined in `ir/repr.rs` with variants `HeapPointer`, `StringBuffer`,
    `MapBuffer`, `SetBuffer`. Adding a `Concurrent` strategy variant should be
    straightforward.
  (See: [Literature Review §10 — Concurrent Immediate RC](../aims-literature-review/section-10-concurrent-rc.md))

### 07.4.1 Runtime Abstraction Boundary

The following `ori_rt` extern "C" functions constitute the **sole interface**
between compiler-generated code and the RC mechanism. All LLVM call sites in
`ori_llvm/src/codegen/arc_emitter/` target these functions. Any future RC
implementation (biased RC, per-object mode bits, CIRC) must preserve these
signatures or update all LLVM call sites.

| Function | Signature | Memory Ordering | Purpose |
|----------|-----------|-----------------|---------|
| `ori_rc_inc` | `(data_ptr: *mut u8)` | `Relaxed` (atomic fetch_add) | Increment reference count. No-op on null. No-op on `MAX_REFCOUNT` (immortal). |
| `ori_rc_dec` | `(data_ptr: *mut u8, drop_fn: Option<extern "C" fn(*mut u8)>)` | `Release` (fetch_sub) + `Acquire` fence before drop | Decrement reference count. Calls `drop_fn` synchronously when count reaches zero (Ori's `Drop` ordering guarantee). No-op on null. No-op on `MAX_REFCOUNT`. |
| `ori_rc_is_unique` | `(data_ptr: *const u8) -> bool` | `Relaxed` (atomic load) | Returns true iff `refcount == 1`. Used for COW `Dynamic` mode uniqueness check. |
| `ori_rc_is_unique_or_null` | `(data_ptr: *const u8) -> bool` | `Relaxed` (atomic load) | Returns true iff `refcount == 1` or pointer is null. Used for COW on potentially-null fat pointers. |
| `ori_rc_alloc` | `(data_size: usize) -> *mut u8` | N/A (allocation) | Allocate RC'd block with header. Not called from compiler-generated code directly (called by collection constructors). |

**Invariant:** These five functions must remain the sole interface between
compiler-generated code and the RC mechanism. AIMS analysis, ARC IR emission,
and LLVM codegen must never embed assumptions about the RC implementation
(atomic vs non-atomic, immediate vs deferred, single-counter vs dual-counter)
beyond calling these functions. This boundary ensures that swapping RC
implementations requires only relinking `ori_rt`, with zero changes to
`ori_arc`, `ori_llvm`, or any AIMS analysis code.

**Concurrent RC compatibility notes:**
- The `drop_fn` parameter on `ori_rc_dec` maps to CIRC's `pop_edges()` --
  the drop function traverses child fields and calls `ori_rc_dec` on each,
  producing recursive reclamation. This must remain synchronous (Ori's `Drop`
  is ordered and deterministic).
- The `MAX_REFCOUNT` immortal sentinel (Section 07.1) must be preserved
  across all RC implementations. It is checked as a fast-path early return
  before any atomic operation or epoch interaction.
- The `single-threaded` feature flag selects non-atomic implementations.
  Future concurrent RC variants should use additional feature flags (e.g.,
  `biased-rc`, `per-object-mode`), not replace the default path.
- `ParamContract` may need a thread-provenance field (`ThreadLocal` /
  `CrossThread`) when concurrent RC is designed. This is informational;
  no current code change.

(See: [Literature Review §10 — Concurrent Immediate RC](../aims-literature-review/section-10-concurrent-rc.md))

- **Representation Optimization** (Double-Ended Bit-Stealing — Elsman, ICFP 2024):
  - Uses both low and high pointer bits for unboxed ADT representation
  - Sister project to AIMS, not AIMS-core
  - AIMS should expose enough shape/locality hints that a future representation
    optimizer can consume them: not shared, not escaping, constructor-only usage,
    hot allocation site
  - **Data flow is one-directional**: type declarations + AIMS converged facts
    (uniqueness, locality, cardinality, shape) flow into the repr optimizer, which
    produces boxity decisions and tag encoding strategy for codegen. No feedback
    flows back into AIMS analysis. If a repr decision needs to influence AIMS
    (e.g., reclassifying an unboxed ADT as `ArcClass::Scalar`), this happens via
    `compute_var_reprs` adjustment *before* AIMS runs, not as a mid-analysis
    mutation.
  - **Constructor arity and payload-type metadata** are additional facts the repr
    optimizer needs (how many fields per constructor, whether each field is scalar
    or ref-counted, field type sizes). These come from the type registry (Pool /
    ArcClassifier), not from AIMS. The pipeline must preserve access to the type
    registry at the point where the Stage 4 repr optimizer runs.
  - **AIMS prerequisite**: `ShapeClass::ReusableCtor(kind)` and `Locality` facts
    must be queryable from a post-pipeline artifact. Currently, `AimsStateMap` is
    consumed during emission and not preserved. A Stage 4 repr optimizer needs
    either: (a) preserved state map, or (b) a function-level summary materialized
    during emission. Option (b) is preferred (smaller footprint, cleaner API).
    The summary should include: per-variable uniqueness at construction site,
    per-variable locality, per-constructor allocation frequency proxy (from
    cardinality).
  (See: [Literature Review §12 — Bit-Stealing](../aims-literature-review/section-12-bit-stealing.md))

---

## 07.5 Completion Checklist

### 07.1 Immortal Objects
- [x] `ImmortalSet` data structure defined and populated during `compute_var_reprs`
- [x] `ImmortalSet` threaded through `AimsPipelineConfig` to all emission steps
- [x] Immortal variables excluded from AIMS analysis (same treatment as `SCALAR`)
- [x] RC emission skips `RcInc`/`RcDec` for immortal variables
- [x] COW annotations and drop hints skip immortal variables
- [x] `ori_rt` `ori_rc_inc`/`ori_rc_dec` skip at `MAX_REFCOUNT` (no-op, not abort)
- [x] Empty string uses `ori_str_empty()` (SSO constant, no heap/RC needed)
- [x] LLVM emitter calls `ori_str_empty()` for `""` literals (value_emission.rs)
- [x] Empty string codegen avoids creating global constant + `ori_str_from_raw`
- [x] All Rust unit tests pass (`aims/immortal/tests.rs`, `ori_rt/src/tests.rs`)
- [x] Integration: AOT programs with empty string literal pass with leak detection
- [x] Shadow comparison (`aims-shadow`) correctly logs immortal skips as improvements

### 07.2 Static RC Coalescing
- [x] Coalescing peephole implemented as post-step in `emit_rc_ops()` (Phase 3)
- [x] Adjacent `RcInc(x, 1); RcInc(x, 1)` merged to `RcInc(x, 2)`
- [x] Adjacent `RcInc(x); RcDec(x)` with no intervening alias cancelled
- [x] Coalescing does NOT merge across calls, aliases, or control flow
- [x] `RcStrategy` mismatch prevents merge (safety guard)
- [x] Net RC operation count for basic-block-heavy test is strictly lower than
  pre-coalescing count
- [x] All coalescing Rust unit tests pass (6/6)

### 07.3 Cross-Optimization Synergies
- [x] COW-aware borrowing: `cow_aware_borrowing_static_unique_for_linear_owned_unique_param`
  — AIMS eliminates COW check that would be `Dynamic` from uniqueness alone
- [x] Uniqueness-preserving borrows: `uniqueness_preserving_borrow_disjoint_field_cow_is_static`
  — COW on a value with live disjoint-field borrows gets `StaticUnique`
- [x] Demand-driven RC elimination: `absent_param_no_rc_dec_at_entry` — `Absent`
  parameter skips callee-side `RcDec`; caller-side `Borrowed` prevents `RcInc`
- [x] `ori_arc::verify` check: parameters with `Absent` cardinality have no uses
  in the function body — `check_function_with_contract()` in `verify/mod.rs`,
  `AbsentParamHasUses` error variant, 5 unit tests, called from AIMS pipeline step 9a
- [x] Cross-dimension synergy: `cross_dimension_synergy_cow_aware_borrowing` — three
  non-uniqueness dimensions (Owned+Linear+Once) override MaybeShared → StaticUnique;
  neither uniqueness alone nor cardinality alone could achieve this

### 07.3.4 FIP Call-Site Specialization
**Blocked by:** `FipContract::Conditional` does not exist in Stage 1 (all functions
get `FipContract::Never`). This item becomes implementable after Section 09.2
Effect Activation adds `FipContract::Conditional` and `FipContract::Bounded(u16)`
to the contract layer. The test infrastructure (transfer_apply + contract lookup)
exists; only the contract variant and its extraction logic are missing.

- [ ] FIP Conditional call-site optimization: when caller proves all `requires_unique_params`
  are `Unique`, use callee's FIP-optimized contract (`may_allocate: false, may_share: false`)
  — FP²'s dynamic embedding (`dropru` on unique binding → `(dconru_h)` fast path)
- [ ] FIP call-site specialization tests: unique args → optimized contract,
  MaybeShared arg → conservative, uniqueness preserved through call

### 07.4 Future Items
- [x] Each future item has documented AIMS prerequisites (what must be stable/precise)
  — all four items (Whole-Program Mutability, SCC-Frozen Cyclic RC, Concurrent RC,
  Representation Optimization) have **AIMS prerequisite** paragraphs documenting required
  stability/precision of MemoryContract, ShapeClass, Locality, and RcStrategy
- [x] No runtime or compile-time changes are introduced for future items in Section 07
  — 07.4 is explicitly informational ("do NOT have checkboxes"), no code changes

### Overall
- [x] Performance: `ORI_DUMP_AFTER_ARC` RC operation count comparison shows 24%
  reduction on bench_medium (25→19 total RC ops). Binary performance comparison
  deferred — AIMS AOT codegen not yet producing executables (Stage 1 limitation).
- [x] All changes pass `cargo test --workspace --features aims` (excluding ori_llvm
  AOT integration tests — 74 pre-existing failures from incomplete AIMS→LLVM pipeline)
- [x] All changes pass `cargo clippy --workspace --features aims`
- [x] Results documented in a comparison table (below)

### RC Operation Count Comparison (bench_medium.ori)

| Function            | Old (Inc/Dec/Total) | AIMS (Inc/Dec/Total) | Change |
|---------------------|---------------------|----------------------|--------|
| `chain_divide`      | 2 / 2 / 4          | 0 / 0 / 0           | -4 (eliminated) |
| `classify`          | 0 / 0 / 0          | 0 / 6 / 6           | +6 (correctness) |
| `closest_to_origin` | 0 / 0 / 0          | 0 / 1 / 1           | +1 (borrow→own cleanup) |
| `compute_stats`     | 0 / 0 / 0          | 0 / 1 / 1           | +1 (borrow→own cleanup) |
| `main`              | 0 / 10 / 10        | 0 / 5 / 5           | -5 (50% reduction) |
| `safe_divide`       | 0 / 0 / 0          | 0 / 1 / 1           | +1 (correctness) |
| `string_work`       | 1 / 7 / 8          | 0 / 4 / 4           | -4 (50% reduction) |
| `transform_pipeline`| 0 / 3 / 3          | 0 / 1 / 1           | -2 (67% reduction) |
| **Total**           | **3 / 22 / 25**    | **0 / 19 / 19**     | **-6 (-24%)** |

**Key improvements:**
- Eliminated all RcInc operations (3→0) — AIMS proves uniqueness more precisely
- `chain_divide`: cross-dimensional reasoning removes redundant inc/dec pairs on Result values
- `main`: demand-driven elimination skips 5 unnecessary RcDec operations
- `string_work`: uniqueness-preserving analysis halves RC operations
- Some functions gain RcDec (`classify`, `safe_divide`): these are correctness improvements
  where the old pipeline was missing cleanup for string/Result temporaries

**Exit Criteria:** At least two advanced optimizations from 07.1-07.3 implemented.
Each must show measurable improvement via `ORI_DUMP_AFTER_ARC` RC operation counts
or `scripts/perf-baseline.sh` binary performance. Results documented in a comparison
table. Section 07.4 items are informational only and do NOT block completion.
