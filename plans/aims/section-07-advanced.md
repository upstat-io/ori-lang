---
section: "07"
title: "Advanced Optimizations"
status: not-started
reviewed: false
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
    status: not-started
  - id: "07.2"
    title: "Static RC Coalescing"
    status: not-started
  - id: "07.3"
    title: "Cross-Optimization Synergies"
    status: not-started
  - id: "07.4"
    title: "Future: Runtime and Representation Follow-ons"
    status: not-started
  - id: "07.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 07: Advanced Optimizations

**Status:** Not Started
**Goal:** Implement optimizations that are only possible (or significantly easier)
with the unified AIMS lattice, demonstrating the architectural advantage over
separate passes.

**Context:** These optimizations are the "stretch goals" that justify AIMS beyond
architectural cleanliness. Each one either requires cross-dimensional information
(impossible with separate passes) or becomes trivial with the unified state map
(awkward to retrofit into the current pipeline).

**Note:** TRMC and constructor contexts have been moved out of this section. They
are now part of the **Opportunity Creation** stage (Stage 3 in the implementation
sequence), implemented in `aims/normalize/`. Basic TRMC normalization is a
pre-analysis pass, not a post-pipeline optimization.

**Depends on:** Section 06 (working AIMS pipeline).

---

## 07.1 Immortal Objects

**File(s):** `compiler/ori_arc/src/aims/lattice.rs`, `compiler/ori_rt/src/rc/mod.rs`

Mark common constants as immortal (RC = MAX), skipping all RC operations on them.

- [ ] Implement immortal detection as an **extension hook outside `AimsState`**,
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
- [ ] Identify immortal values:
  - Boolean literals (`true`, `false`) — already `ArcClass::Scalar`, no RC needed
  - Small integer constants (0-255) — already `ArcClass::Scalar`, no RC needed
  - Empty string literal `""` — currently `ArcClass::DefiniteRef` (str is heap-allocated).
    Making this immortal requires runtime support for an immortal empty string singleton.
  - Unit value `()` — already `ArcClass::Scalar`
  - `None` value — may or may not be scalar depending on the Option's type parameter.
    `Option<int>` → Scalar. `Option<str>` → DefiniteRef. Only pure `None` (no RC
    payload) can be immortal.
- [ ] In emit_rc: skip `RcInc`/`RcDec` for immortal values
- [ ] Runtime support: `ori_rc_inc` already checks `MAX_REFCOUNT` — ensure it's a no-op.
  Note: `ori_rc_dec` must also check and skip deallocation for immortal objects.
- [ ] Pre-allocate immortal objects in `ori_rt` initialization

---

## 07.2 Static RC Coalescing

**File(s):** `compiler/ori_arc/src/aims/emit_rc.rs`

When a value undergoes multiple RC changes in a sequence (e.g., inc then dec from
different operations), only the net effect matters.

- [ ] Detect coalesceable sequences:
  - `RcInc(x)` followed by `RcDec(x)` with no intervening alias → cancel both
  - `RcInc(x); RcInc(x)` → single `RcInc(x, count: 2)` (already supported by IR)
  - `RcDec(x)` followed by `Construct` reusing x → fuse into `Reset`

- [ ] With the unified state map, coalescing is trivial:
  - Count the net RC change per variable across a basic block
  - Emit the net result instead of individual operations
  - Current `rc_elim` does this but only within a single block and after insertion
  - **Ordering constraint:** Net-effect coalescing is only valid within straight-line
    sequences with no intervening calls, aliases, or control flow. RC operations around
    call boundaries, drop points, and alias creation points must preserve their ordering.
    The optimization applies to sequences of RcInc/RcDec on the same variable with no
    intervening operations that could observe the reference count.

---

## 07.3 Cross-Optimization Synergies

**File(s):** `compiler/ori_arc/src/aims/intraprocedural.rs`

Optimizations that emerge from cross-dimensional information in the unified lattice.

- [ ] **COW-aware borrowing**:
  - If a function parameter has `(Owned, Linear, Once, Unique)` state (owned, consumed
    once, unique — ideal for COW mutation), the callee knows: no RC inc at call site
    AND no COW check needed. The current system needs borrow inference + uniqueness
    analysis to reach this conclusion separately.

- [ ] **Uniqueness-preserving borrows**:
  - When `Project(dst, src, field)` creates a borrow, the state map knows src
    stays `Unique`. This means src can still participate in COW or reuse while
    the borrow is live. The current system loses this information at the
    borrow/uniqueness boundary.

- [ ] **Demand-driven RC elimination**:
  - If a callee's parameter cardinality is `Absent` (the callee never uses the
    parameter), skip the RC inc at the call site AND skip the RC dec at the
    callee entry. Currently, dead parameters still get RC operations.

---

## 07.4 Future: Runtime and Representation Follow-ons

These are independent efforts that consume AIMS facts but should not block the
core AIMS replacement. They belong in Stage 5 of the implementation sequence.

- [ ] **Whole-Program Mutability** (Morphic):
  - Cross-function uniqueness tracking beyond SCC boundaries
  - Track value flow from creation to final consumption across entire program
  - Requires whole-program compilation (incompatible with separate compilation)

- [ ] **SCC-Frozen Cyclic RC** (Parkinson et al., ISMM 2024):
  - For deeply immutable frozen graphs, reference counting can be lifted to the
    level of strongly connected components
  - Relevant only if Ori later adds explicit freeze or immutable cyclic heaps
  - Keep as a future extension note

- [ ] **Concurrent RC Strategies** (CIRC — Jung et al., PLDI 2024):
  - Combines SMR-style deferral with RC, immediately applies decrements,
    defers only reclamation
  - Not core to current AIMS unless Ori commits to concurrent shared heaps
  - Preserve `RcStrategy` / runtime hook abstraction so the compiler can later
    target concurrent RC primitives

- [ ] **Representation Optimization** (Double-Ended Bit-Stealing — Elsman, ICFP 2024):
  - Uses both low and high pointer bits for unboxed ADT representation
  - Sister project to AIMS, not AIMS-core
  - AIMS should expose enough shape/locality hints that a future representation
    optimizer can consume them: not shared, not escaping, constructor-only usage,
    hot allocation site

---

## 07.5 Completion Checklist

- [ ] Immortal objects: `RcInc`/`RcDec` count for immortal candidates (empty string,
  static allocations) drops to 0 in ARC IR dump
- [ ] Static coalescing: net RC operation count for a basic-block-heavy test case
  (e.g., list builder with 20+ allocations) is strictly lower than pre-coalescing count
- [ ] COW-aware borrowing: at least one test case where AIMS eliminates a COW check
  that the old pipeline emitted as `CowMode::Dynamic`
- [ ] Cross-optimization synergy: at least one test case where two AIMS dimensions
  combine to eliminate an RC operation that neither could eliminate alone (document
  which dimensions and why)
- [ ] Performance: `scripts/perf-baseline.sh` shows no regression; at least one
  benchmark improves by >= 5% in generated binary performance

**Exit Criteria:** At least two advanced optimizations implemented. Each must show
measurable improvement via `ORI_DUMP_AFTER_ARC` RC operation counts or
`scripts/perf-baseline.sh` binary performance. Results documented in a comparison
table.
