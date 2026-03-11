---
section: "05"
title: "Reuse Emission"
status: in-progress
reviewed: true  # 2026-03-10
goal: "Emit Reset/Reuse/IsShared operations from converged AimsStateMap"
inspired_by:
  - "Drop-guided reuse (Lorenzen & Leijen, ICFP 2022)"
  - "FP2 reuse credits (Lorenzen et al., ICFP 2023)"
  - "Lean 4 ResetReuse (src/Lean/Compiler/LCNF/ResetReuse.lean)"
  - "ori_arc reset_reuse + expand_reuse"
depends_on: ["01", "02", "03"]
sections:
  - id: "05.1"
    title: "Reuse Opportunity Detection"
    status: in-progress
  - id: "05.2"
    title: "Reuse Emission"
    status: complete
  - id: "05.3"
    title: "CollectionReuse Handling"
    status: complete
  - id: "05.4"
    title: "FIP as Contract, FBIP as Diagnostic"
    status: in-progress
  - id: "05.5"
    title: "Completion Checklist"
    status: in-progress
---

# Section 05: Reuse Emission

**Status:** Not Started

**Goal:** Detect reuse opportunities from the converged `AimsStateMap` and emit
`Reset`, `Reuse`, `IsShared`, `Set`, `SetTag` instructions. This replaces
`reset_reuse` and `expand_reuse` from the current system. FBIP enforcement
remains a separate post-pipeline check (see Section 06, step 14).

**Context:** The current system detects reuse opportunities in a separate pass
AFTER RC insertion, then expands them into conditional branches. AIMS detects
reuse during analysis (a consumed unique value near a same-sized allocation is
a reuse candidate) and emits the expanded form directly.

The research literature suggests drop-guided reuse (Lorenzen & Leijen, ICFP 2022)
is simpler and produces better results than the original Perceus reuse. In AIMS,
reuse is naturally drop-guided because the state map tracks when values transition
to `Dead` with `Unique` uniqueness.

**Reference implementations:**
- **Lorenzen & Leijen** ICFP 2022: Drop-guided reuse — perform reuse analysis AFTER
  RC insertion, guided by drop positions. Provably frame-limited.
- **Lean 4** `src/Lean/Compiler/IR/ExpandResetReuse.lean` and
  `src/Lean/Compiler/LCNF/ResetReuse.lean`: Two-pass (strict then relaxed) reuse detection
- **ori_arc** `reset_reuse/mod.rs` + `reset_reuse/cross_block.rs`: Current CFG-based
  reuse detection (including cross-block reuse via dominator/post-dominator trees)

**Depends on:** Sections 01, 02, 03 (converged state map).

---

## 05.1 Reuse Opportunity Detection

**File(s):** `compiler/ori_arc/src/aims/emit_reuse.rs` (NEW)

> **Warning: File size.** This section covers reuse detection, emission, CollectionReuse handling,
> and FBIP enforcement. Estimated ~800 lines exceeds the 500-line limit. **Split into submodules:**
> - `aims/emit_reuse/mod.rs` — `emit_reuse()` entry point, reuse emission (~250 lines)
> - `aims/emit_reuse/detect.rs` — `find_reuse_opportunities()` with cross-block detection (~300 lines)
> - `aims/emit_reuse/fbip.rs` — FBIP enforcement, auto-FBIP detection (~150 lines)

A reuse opportunity exists when:
1. A variable transitions to `Dead` with `Unique` or `MaybeShared` uniqueness, AND
   has a reusable `ShapeClass` (its allocation is potentially reclaimable — `Unique`
   enables static reuse, `MaybeShared` enables dynamic reuse with `IsShared` check)
2. A subsequent `Construct` of the **same type** in the same or dominated block

- [x] Implement `find_reuse_opportunities(func, state_map) -> Vec<ReuseOpportunity>`:
  ```rust
  pub struct ReuseOpportunity {
      /// The variable being consumed (source of the reuse token)
      pub source_var: ArcVarId,
      /// The block where the source dies
      pub source_block: ArcBlockId,
      /// The Construct instruction that can reuse
      pub target_instr: (ArcBlockId, usize),
      /// Whether the source is provably unique (skip IsShared check)
      pub is_static_unique: bool,
  }
  ```

- [x] Match reuse pairs:
  - Scan for variables transitioning to `Dead` + `Unique` in the state map
  - For each, find the nearest Construct of compatible size in dominated blocks
  - Prefer same-type reuse over cross-type reuse (Lean 4's strict-then-relaxed)
  - Only reuse within the same function (no cross-function reuse)
  - **Type compatibility**: reuse requires **same type** (not just same size). This
    matches the current `reset_reuse` implementation which checks `typeof(dec_var) == ty`
    of the Construct. Same-type ensures layout compatibility (field offsets, alignment,
    padding) and drop compatibility (the drop glue for the reused allocation matches
    the new value's type). Cross-type reuse (different types, same size) is unsound
    without proving: (a) identical layout including alignment and padding, (b) drop
    glue compatibility (the old type's drop must not be called on the new value), and
    (c) no interior pointers that depend on type identity. **v1: same-type only.**
    Cross-type reuse may be explored in Stage 5 (Section 07) with explicit layout
    and drop compatibility proofs.

- [ ] **Cross-block reuse via ReusePlanner** (solutions.md Decision 4):
  Cross-block reuse requires BOTH semantic facts (from AIMS) AND structural
  validity (from dominator analysis). These are different categories:
  - AIMS proves: "this value is dead, unique, and has a reusable shape"
  - Dominator analysis proves: "the death point dominates the allocation, and
    the allocation post-dominates the death point on all paths"

  The `ReusePlanner` is a dedicated pass that runs after RC emission and before
  final block cleanup. It consumes semantic candidate events from AIMS and
  validates them against CFG geometry.

- [x] Define `SizeClass` for allocation size matching:
  Implemented in `aims/lattice/mod.rs`. Added `SizeClass(u32)` with `UNKNOWN`
  constant, `from_bytes()`, `bytes()`. Populated as `SizeClass::UNKNOWN` in
  `DeathEvent` and `AllocEvent` (Stage 1 uses same-type matching; size matching
  deferred to Stage 2+).

- [x] Define candidate event types for the ReusePlanner.
  Note: `DeathEvent` and `AllocEvent` are **local to the ReusePlanner**, NOT stored
  in the `AimsStateMap` event table. They are collected transiently during RC
  emission (step 6) and consumed by reuse emission (step 7) in the same pipeline
  invocation. The `AimsEvent` enum in the state map (Section 02.1) tracks
  long-lived analysis artifacts; these are short-lived emission-phase data:
  ```rust
  pub struct DeathEvent {
      pub var: ArcVarId,
      pub block: ArcBlockId,
      pub instr_idx: usize,
      pub uniqueness: Uniqueness,
      pub ty: Idx,           // ARC IR type index — required for same-type matching
      pub shape: ShapeClass,
      pub size_class: SizeClass,
  }

  pub struct AllocEvent {
      pub block: ArcBlockId,
      pub instr_idx: usize,
      pub dst: ArcVarId,
      pub ty: Idx,           // ARC IR type index — required for same-type matching
      pub shape: ShapeClass,
      pub size_class: SizeClass,
  }
  ```

- [x] Matching rule for `DeathEvent d` and `AllocEvent a`:
  1. `d.uniqueness` is `Unique` or `MaybeShared`
  2. `d.shape` and `a.shape` are reuse-compatible
  3. `d.ty == a.ty` (same type — v1 requirement; see 05.1 type compatibility note)
  4. `d.block` dominates `a.block` (from `DominatorTree`)
     - 4a. For same-block matches (`d.block == a.block`): `d.instr_idx < a.instr_idx`
       (death must precede allocation in program order). Without this constraint,
       a death at instruction 5 could incorrectly match an allocation at instruction 3.
  5. `a.block` post-dominates `d.block` (from `PostDominatorTree`)
  6. No earlier chosen match has already consumed the death token

- [x] Selection strategy:
  - Prefer same-block matches (no dom/post-dom needed)
  - Then nearest dominated/post-dominating target by dominator depth
  - Prefer same-shape over merely same-size

- [x] **Cost control**: only build dominator/post-dominator trees if the function
  has at least one death event with reusable shape AND at least one compatible
  allocation event. For functions with no reuse candidates, no structural pass cost.

- [ ] **ReusePlanner interface specification** (Decision 4):
  ```rust
  /// Cross-block reuse planner. Consumes semantic facts from AIMS
  /// and validates them against CFG geometry.
  ///
  /// Runs as step 7 in the pipeline (Section 06.2), AFTER RC emission
  /// (step 6) and BEFORE COW annotations (step 11a). This ordering is
  /// critical: RC emission may insert trampoline blocks (edge cleanup),
  /// which changes the CFG. The ReusePlanner must see the post-edge-cleanup
  /// CFG. Note: block_merge() runs later (step 11) and may simplify the
  /// CFG by merging trivial jump chains, but this does not invalidate
  /// reuse instructions — Reset/Reuse/Set/IsShared survive merge because
  /// they are regular instructions within blocks.
  pub struct ReusePlanner<'a> {
      func: &'a ArcFunction,
      death_events: Vec<DeathEvent>,
      alloc_events: Vec<AllocEvent>,
      /// Built lazily — only constructed if there are candidate pairs.
      dom_tree: Option<DominatorTree>,
      /// Built lazily — only constructed if there are cross-block candidates.
      post_dom_tree: Option<PostDominatorTree>,
  }
  ```
  - **Dominator tree source**: Use `DominatorTree::build(func)` and
    `PostDominatorTree::build(func)` (re-exported from `graph/mod.rs`,
    defined in `graph/dominator.rs` and `graph/post_dominator.rs`).
    These are the same types used by the current `reset_reuse/cross_block.rs`.
    Built lazily — only when cross-block candidates exist.
  - **Event population**: `DeathEvent`s are collected during RC emission
    (step 6) — whenever `emit_rc_ops` would emit an `RcDec` for a variable
    with `Unique` or `MaybeShared` uniqueness and a reusable `ShapeClass`,
    it records a `DeathEvent` instead of (or in addition to) emitting the dec.
    `AllocEvent`s are collected by scanning the function's `Construct`
    instructions before emission.
  - **Output**: `Vec<ReuseOpportunity>` consumed by `emit_reuse()` to insert
    Reset/Reuse/IsShared instructions.
  - **Pipeline position in Section 06.2**: step 7 (`aims::emit_reuse`) calls
    `ReusePlanner::plan()` internally. The planner is NOT a separate pipeline
    step — it is an implementation detail of reuse emission.

- [x] Prioritize reuse:
  - Pattern match arms (scrutinee → constructor in branch) are the highest priority
  - This is the "resurrection hypothesis" — deconstructed values precede same-shaped
    constructions

---

## 05.2 Reuse Emission

**File(s):** `compiler/ori_arc/src/aims/emit_reuse.rs`

Emit reuse instructions in one pass. AIMS eliminates the old pipeline's two-pass
pattern (detect abstract opportunities, then expand into concrete instructions).

**Stage 1 emission strategy — intentional dual-form design:**

Stage 1 deliberately uses **two different emission strategies** depending on
uniqueness. This is a conscious transitional complexity, not a clean unified
design. The rationale:

- **Static-unique path** (`Uniqueness::Unique`): Emit `Reset` + `Reuse` + `Set`
  instructions. These are ARC IR intermediates consumed directly by the LLVM
  emitter — no second expansion pass is needed. `Reset` marks the allocation for
  reuse, `Set` updates changed fields, `Reuse` constructs the new value in the
  reused allocation. This path reuses existing IR instruction types and LLVM
  emitter support unchanged.
- **Dynamic path** (`Uniqueness::MaybeShared`): Emit expanded CFG directly:
  `IsShared` → `Branch` → fast block (`Set` for in-place mutation) / slow block
  (`RcDec` + `Construct`). No intermediate `Reset`/`Reuse` on this path — the
  conditional structure IS the final form. This avoids the old pipeline's
  `expand_reuse` second pass entirely.

The dual form exists because the static path can use simpler IR (the LLVM emitter
already handles `Reset`/`Reuse`), while the dynamic path must emit the conditional
CFG structure directly to avoid needing a second expansion pass. Unifying these
into a single intermediate form would require either: (a) making `Reset`/`Reuse`
carry conditional semantics (complicates the IR), or (b) always emitting the
expanded CFG even for static-unique cases (wastes code and loses the optimization
signal). Neither is worth the complexity in Stage 1. A future simplification pass
(post-Stage 1) could unify the forms if the dual strategy proves burdensome.

- [x] For each `ReuseOpportunity`:
  **Critical coordination with RC emission (Section 04)**: When a `DeathEvent`
  is paired with a `ReuseOpportunity`, the `RcDec` that RC emission (step 6)
  would normally emit at the death site must be SUPPRESSED. Instead, the reuse
  emission (step 7) emits a `Reset` at that site (which conditionally drops or
  reuses the allocation). Two implementation strategies:
  1. **Deferred-dec approach** (recommended): RC emission records death events
     but does NOT emit `RcDec` for variables with `is_reuse_candidate() == true`.
     Reuse emission then either emits `Reset` (if matched) or falls back to
     `RcDec` (if no compatible allocation found). This avoids needing to delete
     already-emitted instructions.
  2. **Post-patch approach**: RC emission emits all `RcDec`s, and reuse emission
     replaces matched `RcDec`s with `Reset`. Requires an IR mutation pass.

  Strategy 1 is preferred because it maintains the "emit once, emit right"
  principle. Implementation: `emit_rc_ops` returns a `Vec<SuppressedDeath>` of
  reuse-candidate deaths that were suppressed. `emit_reuse` consumes this list.
  Using `Vec<SuppressedDeath>` instead of `FxHashSet<ArcVarId>` preserves full
  site identity — one variable can have multiple death sites on different control
  flow edges:
  ```rust
  pub struct SuppressedDeath {
      pub var: ArcVarId,
      pub block: ArcBlockId,
      pub instr_idx: usize,
  }
  ```

  - If `is_static_unique` (state map says `Unique`):
    - Emit `Reset { var: source, token: fresh_var }` — mark for reuse
    - Emit `Reuse { token, dst, ty, ctor, args }` — construct using reuse token
    - On the fast path (unique), `Reuse` reuses the allocation via in-place `Set`
      instructions for changed fields; on the (impossible here) slow path, it
      allocates fresh. Since source is statically unique, the slow path is dead code.
  - If NOT `is_static_unique` (state map says `MaybeShared`):
    - Emit `IsShared { dst: shared_flag, var: source }` — runtime RC == 1 check
    - Emit `Branch { cond: shared_flag, then_block: slow, else_block: fast }`
    - Fast block: `Set` instructions for changed fields (in-place mutation)
    - Slow block: `RcDec { var: source }` + `Construct { dst, ty, ctor, args }`
    - Merge block: phi-like `Jump` with the result from whichever path executed
    - This is the current expand_reuse pattern but emitted directly (no intermediate
      Reset/Reuse that needs a second expansion pass)
  - If no compatible allocation found for a suppressed death → emit `RcDec`
    as fallback (the deferred dec from RC emission)

- [x] Reuse specialization (from Perceus — self-set elimination):
  Implemented in `aims/emit_reuse/mod.rs`. `apply_static_reuse` builds a
  `ProjMap` from `Project` instructions before the death site, then emits
  `Set` only for fields where the `Construct` arg differs from the projected
  value. Unchanged fields (self-sets) are skipped entirely. For static-unique
  reuse, the function emits direct `Set`/`SetTag` instructions instead of
  `Reset`/`Reuse` intermediates — no expansion pass needed. `EmitReuseResult`
  tracks `fields_skipped` for diagnostics. 6 new tests cover: basic self-set
  elimination, no-projection case, all-self-set case, enum variant with
  `SetTag`, enum self-set with tag change, and span rebuilding.

---

## 05.3 CollectionReuse Handling

**File(s):** `compiler/ori_arc/src/aims/emit_reuse.rs`

`CollectionReuse` is a self-contained instruction for list/set buffer reuse that
is separate from the struct Reset/Reuse system. It is emitted during lowering
(not by AIMS), and AIMS must preserve it.

- [x] **Do NOT replace existing CollectionReuse instructions** — they are emitted
  by the lowerer and handle their own uniqueness checking at runtime via
  `ori_list_reset_buffer`. AIMS should not duplicate this logic.
- [x] AIMS analysis must correctly track CollectionReuse:
  - `old_var` is consumed (RC handled internally by the runtime function)
  - `dst` is fresh (Unique)
  - `args` are consumed (stored into the new buffer)
- [x] **Stage 2+ only**: AIMS may detect NEW collection reuse opportunities
  (an `RcDec` on a list followed by a `Construct(ListLiteral)` of similar size)
  and emit `CollectionReuse` to replace the pair. This is an optimization
  opportunity beyond what the current pipeline does. **In Stage 1, AIMS
  preserves existing `CollectionReuse` instructions and does NOT emit new
  ones.** This keeps the migration surface smaller and avoids introducing new
  optimization behavior during the pipeline transition.

---

## 05.4 FIP as Contract, FBIP as Diagnostic

**FIP and FBIP serve distinct roles:**

- **FIP** (`FipContract` on `MemoryContract`) is an **analysis-time contract** computed
  during interprocedural analysis (Section 03). It answers: "can this function run
  fully in-place (no allocation, no deallocation, constant stack) given that the
  specified parameters are unique?" This is based on the FP² certification criterion
  (Lorenzen et al., ICFP 2023).

- **FBIP** (`check_fbip_enforcement` in `fbip/mod.rs`) is a **post-pipeline read-only
  diagnostic** that runs AFTER the full pipeline (step 14, see Section 06.2). It checks
  the FINAL IR state and answers: "did this function actually achieve in-place reuse
  for all its COW operations?"

**FIP drives reuse emission; FBIP validates the result.**

- [ ] During reuse emission (05.2), consult `MemoryContract.fip` for the current function:
  - If `FipContract::Certified` — all reuse paths should be static-unique
  - If `FipContract::Conditional { requires_unique_params }` — the function emits
    standard dynamic reuse (IsShared checks). FIP benefits are realized at **call
    sites**: when the caller's AIMS analysis proves that the arguments corresponding
    to `requires_unique_params` are `Unique`, the caller knows the callee will hit
    all fast paths (no allocation). This enables the caller to propagate `Unique`
    through the call and to skip defensive RC ops. No function specialization is
    needed — the same compiled code handles both satisfying and non-satisfying call
    sites via the existing IsShared conditional paths.
  - If `FipContract::Never` — standard dynamic reuse
- [ ] Record FIP-guided reuse decisions as `FipGateRecord` entries in a
  separate emission-phase artifact (e.g., `Vec<FipGateRecord>` returned
  alongside the emitted function), NOT in the `AimsStateMap`. The
  `AimsStateMap` is a pure analysis output; emission-phase observations
  go in emission-phase data structures. Note: `AimsEvent::FipGate` in
  Section 02 should be moved to an emission-phase type accordingly.
  These records are consumed by verification (Section 08).
- [x] FBIP enforcement (step 14) continues unchanged — reads `ArcFunction.cow_annotations`
  and block instructions. AIMS populates these identically to the old pipeline.
- [ ] AIMS may enrich FBIP with additional metadata (e.g., uniqueness state at missed-reuse
  points), but this is additive, not a replacement.

---

## 05.5 Completion Checklist

- [x] Reuse opportunities correctly detected from state map
- [ ] Cross-block reuse detected (dominator-guided via ReusePlanner) <!-- deferred: Stage 2 -->
- [ ] ReusePlanner builds dom/post-dom trees only when candidates exist <!-- deferred: Stage 2 -->
- [x] Static-unique reuse emitted without `IsShared` check
- [ ] Dynamic reuse emitted with conditional branch <!-- deferred: Stage 2 -->
- [ ] Reuse specialization skips unchanged fields <!-- deferred: Stage 2 -->
- [ ] **RC/reuse coordination**: RC emission suppresses `RcDec` for reuse candidates;
  reuse emission handles them (Reset if matched, fallback `RcDec` if unmatched) <!-- deferred: Stage 2 -->
- [ ] Unmatched reuse candidates correctly fall back to `RcDec` <!-- deferred: Stage 2 -->
- [x] `CollectionReuse` instructions preserved from lowering
- [x] Emitted code passes `ori_arc::verify` checks
  Verified: verification runs after emission (steps 9, 13) in `aims_pipeline.rs`.
- [x] FBIP enforcement (separate pass, step 14) still works on AIMS output
- [ ] `FipContract` consulted during reuse emission (Stage 2) <!-- deferred: Stage 2 -->
- [ ] FIP-guided fast paths emit static-unique reuse when preconditions hold <!-- deferred: Stage 2 -->
- [ ] `FipGate` records captured in emission-phase artifact for verification <!-- deferred: Stage 2 -->

**Exit Criteria:** `cargo t -p ori_arc -- aims::emit_reuse` passes. Reuse opportunities
are found for all cases that the current `reset_reuse` finds, plus any new cases
from the unified state map. FIP-certified functions (Stage 2) achieve allocation-free
fast paths verified by FBIP enforcement.
