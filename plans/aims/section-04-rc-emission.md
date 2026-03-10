---
section: "04"
title: "RC Emission"
status: not-started
reviewed: true  # 2026-03-10
goal: "Emit minimal RcInc/RcDec operations, COW annotations, and drop hints from converged AimsStateMap"
inspired_by:
  - "Perceus dup/drop insertion (Reinking et al., PLDI 2021)"
  - "Lean 4 ExplicitRC (src/Lean/Compiler/IR/RC.lean)"
  - "ori_arc rc_insert (compiler/ori_arc/src/rc_insert/mod.rs)"
depends_on: ["01", "02", "03"]
sections:
  - id: "04.1"
    title: "RC Emission Algorithm"
    status: not-started
  - id: "04.2"
    title: "Emission at Function Boundaries"
    status: not-started
  - id: "04.3"
    title: "COW Annotations"
    status: not-started
  - id: "04.4"
    title: "Drop Hints"
    status: not-started
  - id: "04.5"
    title: "Locality and Effect Reading"
    status: not-started
  - id: "04.6"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: RC Emission

**Status:** Not Started
**Goal:** Read the converged `AimsStateMap` from Section 02 and emit the minimal
set of `RcInc` and `RcDec` instructions into the `ArcFunction`. Also compute
`CowAnnotations` and `DropHints` from the state map. This replaces `rc_insert`,
`rc_elim`, `rc_identity`, and the COW/drop-hint computation passes.

**Context:** The current system first inserts RC operations (`rc_insert/`,
specifically `insert_rc_ops_with_ownership` and `insert_external_invoke_cleanup`),
then normalizes identity through projections (`rc_identity/`), then removes
redundant operations via dataflow analysis (`rc_elim/eliminate_rc_ops_dataflow`).
This insert-then-remove pattern is inherently wasteful. AIMS emits only the
necessary RC operations from the start, because the converged state map already
encodes the analysis facts that make most operations redundant.

**Relationship to the state map:** The `AimsStateMap` is the analysis fact source.
COW annotations and drop hints are **derived packaging artifacts** — they combine
analysis facts (keyed by `ArcVarId`, position-independent) with final IR positions
(from a post-merge walk) to produce the `(block_idx, instr_idx)`-keyed maps that
the LLVM emitter expects. The analysis proves facts; the packaging step maps them
to the emitter's layout. This is a derivation, not a second source of truth.

**Reference implementations:**
- **Perceus** PLDI 2021: Syntax-directed dup/drop insertion from linear resource calculus
- **Lean 4** `RC.lean`: Backward liveness-driven RC insertion with last-use optimization
- **ori_arc** `rc_insert/`: Current liveness-driven RC insertion (to be replaced).
  Module contains `annotate.rs`, `block_rc.rs`, `edge_cleanup.rs`, `insert.rs`, `mod.rs`.

**Depends on:** Sections 01, 02, 03 (converged state map + interprocedural signatures).

---

## 04.1 RC Emission Algorithm

**File(s):** `compiler/ori_arc/src/aims/emit_rc.rs` (NEW)

> **Warning: File size.** This section covers RC emission, function boundary handling,
> arg_ownership population, edge cleanup, COW annotations, and drop hints. Estimated
> ~1,100 lines exceeds the 500-line limit. **Must split into submodules:**
> - `aims/emit_rc/mod.rs` — `emit_rc_ops()` entry point (~200 lines)
> - `aims/emit_rc/boundaries.rs` — function entry/exit/call-site RC (~200 lines, from 04.2)
> - `aims/emit_rc/arg_ownership.rs` — `emit_arg_ownership()` (~250 lines, from 04.1)
> - `aims/emit_rc/cow.rs` — COW annotation computation (~150 lines, from 04.3)
> - `aims/emit_rc/drop_hints.rs` — drop hint computation (~100 lines, from 04.4)

**LLVM stability constraint (Change 5):** The first AIMS integration must preserve
these stable LLVM-facing outputs unchanged:
- `ArcParam.ownership` — consumed by function prologue emission
- `Apply.arg_ownership` / `Invoke.arg_ownership` — consumed by RC emission in emitter
- `ArcFunction.cow_annotations` — consumed by `emitter_utils.rs`
- `ArcFunction.drop_hints` — consumed by `rc_ops.rs`
- `ArcFunction.tail_calls` — consumed by tail call lowering

New AIMS outputs (locality hints, FIP certification metadata, shape annotations)
should be added as **internal analysis artifacts first**, not new mandatory fields
on `ArcFunction`. If new fields are later needed, they must be:
- optional or derived
- `#[serde(skip)]` when cache-incompatible
- invisible to old consumers until wired

Traverse the function forward, reading the state map to decide where RC operations
are needed. The key insight: an RC operation is needed exactly when a variable's
state transitions between states that imply different reference count contributions.

- [ ] Implement `emit_rc_ops(func, state_map, sigs, classifier)`:
  - Walk blocks in order
  - For each instruction, check variables' states before and after
  
  - **Access check first (Decision 1)**: Skip all RC for variables with
    `access == Borrowed` (the source handles their lifetime). Skip all RC for
    `Scalar` variables. This is the primary filter — most variables in typical
    code are either scalar or borrowed, so this eliminates the majority of
    candidates before examining consumption or cardinality.
  - For `access == Owned` variables:
    - Emit `RcInc` when a variable is used and its cardinality is `Many`. The
      lattice only proves `Many` (more than once), not an exact use count. The
      emission pass discovers exact use sites during the forward walk: it
      maintains a per-variable use counter, skips the first use (consumes the
      existing reference), and emits `RcInc` before each subsequent use. The
      total number of incs equals (actual uses - 1), determined by the forward
      walk, not by the lattice.
    - Emit `RcDec` when a variable transitions to `Dead` — regardless of its
      consumption mode (`Linear`, `Affine`, OR `Unrestricted`). Every owned value
      that dies must be released. `Linear` values that are consumed at their sole
      use site may have the dec elided (the consumption transfers ownership), but
      `Linear` values that die WITHOUT being consumed (e.g., dead parameter at
      function entry) still need dec. `Unrestricted` values always need dec when
      they die (they had incs for extra uses, and the final reference must be freed).
  - The `access` dimension is the first-order filter; `consumption` and
    `cardinality` determine the specific RC operations needed for owned values.
  - **BorrowSource consultation** (Decision 1): When a borrowed variable's
    source dies (transitions to Dead), the borrow becomes dangling. The emission
    pass must check `state_map.borrow_source(var)`:
    - If `Exact(src)` and `src` is still live → no action needed on the borrow
    - If `Exact(src)` and `src` transitions to Dead → the borrow's lifetime
      ends with the source; no RC dec on the borrow (it was never RC'd), but
      the source's RC dec handles the actual deallocation
    - If `Unknown` → conservative treatment (no optimization based on provenance)
    - **Uniqueness-preserving borrows**: When `borrow_source(var) == Exact(src)`
      and `src.uniqueness == Unique`, the borrow does not increment the reference
      count, so `src` remains RC == 1. However, in-place mutation of `src` (COW
      fast path) is only valid when `src.uniqueness == Unique` AND no borrows
      derived from `src` are currently live. The emission pass must verify borrow
      liveness by checking whether any variable with `BorrowSource::Exact(src)`
      has cardinality > `Absent` at the mutation point. If any derived borrow is
      live, the COW operation must use the dynamic path (runtime check) even if
      the source is statically unique — mutating the source would invalidate the
      borrow's view of the data.

- [ ] Determine `RcStrategy` for each emitted operation:
  - Use `RcStrategy::from_var()` (in `ir/repr.rs`) which derives the strategy from
    `ValueRepr` (stored in `func.var_reprs`, indexed by `ArcVarId`) and the `Pool` type
  - **Dependency**: `func.var_reprs` MUST be populated (pipeline step 3:
    `compute_var_reprs`) BEFORE RC emission (step 6). The pipeline ordering in
    Section 06.2 guarantees this. If `var_reprs` is empty, `RcStrategy::from_var()`
    will panic — add a debug_assert at the start of `emit_rc_ops` verifying
    `!func.var_reprs.is_empty()`.

- [ ] Handle invoke (panicking calls):
  - Emit cleanup RC operations on the unwind edge
  - Variables that are live across the invoke need RC dec on the unwind path
  - The current system does this in `insert_external_invoke_cleanup()` — AIMS
    must replicate this behavior from the state map

- [ ] **Populate `arg_ownership` on Apply/Invoke instructions** (runs as a separate
  step BEFORE RC emission -- see Section 06.2 step 4):
  > **Warning: High complexity.** The current `annotate_arg_ownership()` in
  > `rc_insert/annotate.rs` (250 lines) implements type-qualified method dispatch
  > (e.g., `str.add` borrows while `list.add` consumes), protocol builtin handling,
  > and 5 distinct ownership sets from `BuiltinOwnershipSets`. This is NOT a simple
  > "copy `MemoryContract.params` to `arg_ownership`" operation -- the type-qualification
  > logic must be preserved. Review `rc_insert/annotate.rs` in detail before
  > implementing.
  The LLVM `ArcIrEmitter` reads `Apply.arg_ownership` and `Invoke.arg_ownership`
  to decide per-argument RC behavior at call sites. AIMS must populate these.
  - For each `Apply { args, .. }`: set `arg_ownership[i]` based on callee's
    `MemoryContract.params[i].access` (Borrowed → `ArgOwnership::Borrowed`, Owned → `ArgOwnership::Owned`)
  - For each `Invoke { args, .. }`: same mapping
  - **Builtins require `BuiltinOwnershipSets` during Stage 1** — method calls to
    builtins use the consuming/borrowing sets to determine per-arg ownership.
    This is a pragmatic migration compromise, NOT a contradiction of "one truth":
    the "one truth" principle applies to analysis RESULTS (the converged state map
    and contracts), not to how builtin signatures are SPECIFIED. During Stage 1,
    `BuiltinOwnershipSets` is the INPUT source for builtin ownership rules, and
    `aims/builtins.rs` translates them into `MemoryContract` entries that feed
    into the unified analysis.
  - **Source of `BuiltinOwnershipSets`**: constructed once in
    `run_arc_pipeline_all` (or its AIMS equivalent) and passed through to
    `emit_arg_ownership`. The sets are populated from `borrow/builtins/mod.rs`
    which is RETAINED during AIMS migration (it encodes type-qualified ownership
    rules: e.g., `add`/`concat` are borrowing for `str` but consuming for `list`).
  - **Post-Stage 1**: The sets should be absorbed into `aims/builtins.rs`
    `MemoryContract` definitions, making `BuiltinOwnershipSets` redundant. This
    completes the "one truth" migration for builtins.

- [ ] **Edge cleanup for critical edges**:
  The current `rc_insert` module includes `edge_cleanup.rs` which splits critical
  edges (edges from a block with multiple successors to a block with multiple
  predecessors) to ensure RC operations are correctly placed. AIMS emission must
  either: (a) perform the same edge splitting, or (b) prove that emitting RC ops
  at the block entry/exit is sufficient without splitting. Document the decision.
  Note: `edge_cleanup.rs` is ~328 production lines (plus tests) and creates trampoline blocks. If AIMS
  avoids edge splitting, correctness must be proven for all control flow patterns.

  **Recommended strategy**: Perform edge splitting as part of step 6 (RC emission).
  The state map's per-block-entry/exit granularity means some RC operations need
  to happen on a specific edge (e.g., RcDec for a variable live in one successor
  but dead in another). Without a trampoline block on that edge, the RcDec would
  either be placed at the predecessor's exit (affecting ALL successors) or the
  successor's entry (affecting ALL predecessors). Edge splitting creates a
  dedicated block for edge-specific operations.

  **Implementation**: The current edge cleanup function is
  `rc_insert::edge_cleanup::insert_edge_cleanup(func, classifier, liveness,
  borrowed_params, global_borrows, pool)` — note it is `pub(super)` (only
  accessible within `rc_insert`). AIMS must either: (a) promote it to
  `pub(crate)` and move to `graph/` or a shared module, or (b) reimplement
  edge splitting in `aims/emit_rc/`. Option (a) is preferred — the logic is
  general-purpose (gap detection + trampoline insertion) and ~328 lines. Call
  it at the START of `emit_rc_ops()`, before inserting any RC instructions.
  After splitting, the CFG may have new trampoline blocks — the state map's
  block indices become stale for these blocks. Solution: trampoline blocks
  inherit the EDGE-SPECIFIC state — the state flowing along the particular
  edge that was split. This is not the full predecessor exit state but the
  edge-specific subset: the variables that need RC cleanup on THIS edge
  (live in predecessor, dead in this particular successor). The edge-specific
  state is computed during analysis as part of the per-successor demand
  computation. Their entry state is trivially derived without re-running the
  analysis.

  **Pipeline interaction (Section 06.2)**: Edge cleanup is internal to step 6,
  not a separate pipeline step. The dominator tree (if needed for reuse in step 7)
  must be built AFTER edge cleanup, since splitting can change the CFG topology.

---

## 04.2 Emission at Function Boundaries

**File(s):** `compiler/ori_arc/src/aims/emit_rc.rs`

Function entry and exit require special RC handling based on parameter ownership.

- [ ] At function entry:
  - For each parameter with `ParamContract.access == Borrowed`: no RC operations
  - For each parameter with `ParamContract.access == Owned` and `consumption == Linear`:
    the parameter is consumed once; if it's dead in the body (never used), emit
    immediate `RcDec` at function entry
  - For each parameter with `ParamContract.access == Owned` and `consumption == Affine`:
    the parameter may or may not be used; if dead in the body, emit `RcDec` at entry;
    if used, the consumption handles it (no inc needed since used at most once)
  - For each parameter with `ParamContract.access == Owned` and `consumption == Unrestricted`:
    emit `RcInc` at each **use site** beyond the first (not at function entry —
    incs are placed where the extra uses occur, guided by cardinality from the state map)

- [ ] At call sites:
  - Read callee's `MemoryContract` for each argument position
  - If callee borrows the param and caller is at last use: no RC ops (optimal)
  - If callee consumes the param and caller is at last use: no RC ops (transfer)
  - If callee consumes the param and caller still needs it: emit `RcInc` before call
  - If callee borrows the param and caller is NOT at last use: no RC ops (borrow)

- [ ] At return:
  - No RC operations on the returned value (ownership transfers to caller)
  - Emit `RcDec` for any owned variables still live but not returned

---

## 04.3 COW Annotations

**File(s):** `compiler/ori_arc/src/aims/emit_rc.rs`

COW annotations are derived directly from the uniqueness dimension of the state map.

- [ ] For each COW operation in the function (identified by instruction type):
  - If state map says `Uniqueness::Unique` at that point → `CowMode::StaticUnique`
  - If state map says `Uniqueness::Shared` at that point → `CowMode::StaticShared`
  - If state map says `Uniqueness::MaybeShared` → `CowMode::Dynamic`

- [ ] Store computed `CowAnnotations` in `ArcFunction.cow_annotations`
  **CRITICAL**: COW annotations are keyed by `(block_idx, instr_idx)` where the
  indices refer to the FINAL instruction layout — after RC ops, reuse ops, and
  block_merge. The LLVM emitter tracks `current_block_idx` and
  `current_instr_idx` as it walks through the emitted blocks. A keying scheme
  mismatch would cause the LLVM emitter to look up COW mode at the wrong
  position, defaulting to `Dynamic` and eliminating the static uniqueness benefit.

  **DECIDED: Compute AFTER block_merge (pipeline step 11a — see Section 06.2).**
  Walk the final merged IR to identify COW operation instructions by semantic
  content (they are `Apply`/`Invoke` calling known COW builtin methods,
  identifiable by function name via the interner). For each COW instruction,
  look up the receiver variable's uniqueness from the converged analysis state.
  Key the annotation using the post-merge `(block_idx, instr_idx)`.

  **Analysis facts vs. derived packaging**: `AimsStateMap` is the analysis
  fact source (per-variable ownership, uniqueness, cardinality, etc., keyed
  by `ArcVarId`). COW annotations and drop hints are **derived packaging
  artifacts** — they combine analysis facts with final IR positions (from
  the post-merge walk) to produce `(block_idx, instr_idx)`-keyed maps for
  the LLVM emitter. The walk provides position keys; the state map provides
  semantic facts. Truth lives in the analysis; packaging maps it to layout.

  **Per-variable uniqueness and merge**: Per-variable `AimsState` facts keyed
  by `ArcVarId` are position-independent and survive merge. For variables with
  uniform uniqueness across all uses within a function, the per-variable fact
  suffices for COW annotation. For variables whose uniqueness varies across
  different program points (e.g., a variable used in a loop where uniqueness
  degrades from Unique to MaybeShared after the first COW operation),
  per-instruction state reconstruction via backward replay within the relevant
  block is required. The emission pass detects this case by checking whether
  the variable's uniqueness at block entry differs from block exit.

  **Stage 1B comparison note**: Stage 1B shadow comparison should compare COW
  annotation SEMANTICS (which variables get which `CowMode`), not positional
  keys. The old pipeline computes COW annotations pre-merge; AIMS computes
  them post-merge. Positional keys will differ; semantic content should match.
  Note that `CowAnnotations::remap()` exists in the current codebase and could
  be used to normalize positions for comparison if needed.

---

## 04.4 Drop Hints

**File(s):** `compiler/ori_arc/src/aims/emit_rc.rs`

Drop hints identify `RcDec` operations where the collection is provably unique,
enabling the LLVM emitter to call `ori_buffer_drop_unique` instead of
`ori_buffer_rc_dec`.

- [ ] For each emitted `RcDec` on a collection type:
  - If state map says `Uniqueness::Unique` for that variable at the dec point →
    add to `DropHints`

- [ ] Store computed `DropHints` in `ArcFunction.drop_hints`
  **CRITICAL**: Like COW annotations, drop hints are keyed by `(block_idx, instr_idx)`
  referring to the FINAL instruction layout. The current pipeline computes drop hints
  AFTER `block_merge` (step 12 in the pipeline) because merge renumbers blocks and
  instructions. AIMS should follow the same ordering: compute drop hints after all
  emission and CFG cleanup is complete.

  Drop hints are computed by walking the final IR (post-merge) and looking up
  per-variable uniqueness from the analysis (keyed by `ArcVarId`, not by position).
  Each `RcDec` instruction in the final IR identifies its target variable; the
  uniqueness of that variable determines whether the dec qualifies as a drop hint.
  No positional state map lookup is needed — the derivation uses `ArcVarId`-keyed
  analysis facts combined with post-merge position keys (same pattern as COW
  annotations in Section 04.3).

---

## 04.5 Locality and Effect Reading (v1: Hints Only)

**File(s):** `compiler/ori_arc/src/aims/emit_rc/mod.rs`

In v1, RC emission reads `Locality` and `EffectClass` from the state map but does
NOT emit stack allocation directives or modify the ARC IR structure based on them.
Instead, it records locality hints as internal annotations that a future Stage 4
pass may consume.

- [ ] Read `Locality::FunctionLocal` / `BlockLocal` from state map at allocation points
- [ ] Record locality hints into a separate `Vec<LocalAllocCandidate>` returned
  alongside the emitted function (NOT written back into the `AimsStateMap`, which is
  a pure analysis artifact). This preserves the analysis/emission separation: analysis
  produces the state map, emission reads it and produces both IR mutations and derived
  hint artifacts.
- [ ] Do NOT add new fields to `ArcFunction` for these hints in v1
- [ ] Read `EffectClass` for potential FIP fast-path identification. Per-function
  `EffectClass` states contribute to the function-level `EffectSummary`, which is
  computed during interprocedural analysis (Section 03). By the time Section 04
  runs, `FipContract` is already computed. Section 04 READS `FipContract` (to guide
  fast-path decisions); it does not feed `FipContract`. The `EffectClass` read here
  is consumed by reuse emission (Section 05) for FIP-guided fast-path decisions.

---

## 04.6 Completion Checklist

- [ ] `emit_rc_ops` correctly inserts `RcInc`/`RcDec` operations
- [ ] No redundant RC pairs emitted (what rc_elim currently removes)
- [ ] Function entry/exit RC handling correct for all access/consumption combinations
- [ ] Call site RC handling correct for all callee signature combinations
- [ ] `arg_ownership` populated on all Apply/Invoke instructions
- [ ] Invoke unwind edge cleanup RC operations correct
- [ ] Edge cleanup strategy decided and documented (recommended: promote
  `insert_edge_cleanup` to `pub(crate)` and call at start of `emit_rc_ops`)
- [ ] Trampoline blocks from edge splitting have correct (inherited) state
- [ ] COW annotations computed from state map
- [ ] Drop hints computed from state map
- [ ] `CollectionReuse` instructions preserved (not replaced — they are self-contained)
- [ ] Emitted `ArcFunction` passes `ori_arc::verify` checks
- [ ] RC operation count tracked and compared against current pipeline output
  (goal: equal or fewer; regressions investigated but not automatic blockers
  during Stage 1C — correctness gates are behavioral equivalence + verify)

**Exit Criteria:** `cargo t -p ori_arc -- aims::emit_rc` passes. Emitted RC operations
are verified correct by `verify::check_function`. RC operation counts are measured
and compared — AIMS should produce equal or fewer operations, but the hard gate
is correctness (verify + behavioral equivalence), not RC count parity.
