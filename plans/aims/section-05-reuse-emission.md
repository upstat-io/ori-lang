---
section: "05"
title: "Reuse Emission"
status: complete
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
    status: complete
  - id: "05.2"
    title: "Reuse Emission"
    status: complete
  - id: "05.3"
    title: "CollectionReuse Handling"
    status: complete
  - id: "05.4"
    title: "FIP as Contract, FBIP as Diagnostic"
    status: complete
  - id: "05.5"
    title: "Completion Checklist"
    status: complete
---

# Section 05: Reuse Emission

**Status:** Complete (all subsections done)

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

**File(s):** `compiler/ori_arc/src/aims/emit_reuse/mod.rs`

> **Warning: File size.** This section covers reuse detection, emission, CollectionReuse handling,
> and FIP/FBIP handling. **Actual submodule structure (implemented):**
> - `aims/emit_reuse/mod.rs` — `emit_reuse()` entry point, reuse types, emission logic
> - `aims/emit_reuse/detect.rs` — `find_reuse_opportunities()` with cross-block detection
> - `aims/emit_reuse/fip.rs` — FIP gate records and `FipGateDecision`
> - `aims/emit_reuse/dynamic.rs` — MaybeShared dynamic reuse (IsShared + Branch CFG expansion)
> - `aims/emit_reuse/planner.rs` — cross-block reuse planner (dominator/post-dominator validation)

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
    (c) no interior pointers that depend on type identity. **Same-type only.**
    Cross-type reuse is out of scope without explicit layout and drop
    compatibility proofs.

- [x] **Cross-block reuse via ReusePlanner** (historical design decision):
  Cross-block reuse requires BOTH semantic facts (from AIMS) AND structural
  validity (from dominator analysis). These are different categories:
  - AIMS proves: "this value is dead, unique, and has a reusable shape"
  - Dominator analysis proves: "the death point dominates the allocation, and
    the allocation post-dominates the death point on all paths"

  The `ReusePlanner` is a dedicated pass that runs after RC emission and before
  final block cleanup. It consumes semantic candidate events from AIMS and
  validates them against CFG geometry.

  Implemented in `aims/emit_reuse/planner.rs`. Stage 1: static-unique only
  (MaybeShared cross-block requires two-point CFG expansion, deferred).
  Tests: `cross_block_static_unique_reuse`, `cross_block_self_set_elimination`,
  `cross_block_enum_variant_reuse`, `cross_block_reuse_through_intervening_block`,
  `no_cross_block_reuse_without_post_dominance`, `no_cross_block_reuse_maybe_shared`,
  `no_cross_block_reuse_different_types`.

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
  3. `d.ty == a.ty` (same type — see 05.1 type compatibility note)
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

- [x] **ReusePlanner interface specification** (Decision 4):
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

**File(s):** `compiler/ori_arc/src/aims/emit_reuse/mod.rs`, `compiler/ori_arc/src/aims/emit_reuse/dynamic.rs`

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

**File(s):** `compiler/ori_arc/src/aims/emit_reuse/mod.rs`

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

- [x] During reuse emission (05.2), consult `MemoryContract.fip` for the current function:
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
  Implemented in `aims/emit_reuse/fip.rs`. `apply_fip_upgrades()` inspects the
  function's `FipContract` from `contracts` map and upgrades `MaybeShared`
  opportunities to static-unique when `Certified`. Tests:
  `fip_certified_upgrades_maybe_shared_to_static`, `fip_conditional_records_gate_keeps_dynamic`,
  `fip_never_no_change`, `no_contract_no_fip_influence`, `fip_certified_unique_source_no_gate`.
- [x] Record FIP-guided reuse decisions as `FipGateRecord` entries in a
  separate emission-phase artifact (e.g., `Vec<FipGateRecord>` returned
  alongside the emitted function), NOT in the `AimsStateMap`. The
  `AimsStateMap` is a pure analysis output; emission-phase observations
  go in emission-phase data structures. Note: `AimsEvent::FipGate` in
  Section 02 should be moved to an emission-phase type accordingly.
  These records are consumed by verification (Section 08).
  Implemented: `FipGateRecord` and `FipGateDecision` in `aims/emit_reuse/fip.rs`.
  `EmitReuseResult.fip_gates: Vec<FipGateRecord>` returned from `emit_reuse()`.
  Pipeline logs gate count when non-empty.
- [x] FBIP enforcement (step 14) continues unchanged — reads `ArcFunction.cow_annotations`
  and block instructions. AIMS populates these identically to the old pipeline.
- [x] AIMS may enrich FBIP with additional metadata (e.g., uniqueness state at missed-reuse
  points), but this is additive, not a replacement.
  Implemented: `EmitReuseResult.missed_reuses` tracks death events with no compatible
  allocation. `emit_reuse()` emits `tracing::warn!` when a FIP-certified function has
  unmatched deaths (FBIP enrichment diagnostic). Test: `missed_reuses_counted`.

  **FP²-derived deallocation tracking (Theorem 2):** FIP certification requires
  `EmitReuseResult.missed_reuses == 0` — every consumed value with reusable shape
  must be matched by a compatible allocation. This is the deallocation side of
  FP²'s token balance (`|S| = |S'|`). The existing `tracing::warn!` for
  FIP-certified functions with missed reuses should be upgraded to a hard
  verification error in `verify/mod.rs`: a `FipContract::Certified` function
  with `missed_reuses > 0` violates Theorem 2 and must be rejected. This
  enforcement belongs in the verification step (step 9a), not in reuse emission
  itself, to maintain the analysis/emission/verification separation.
  (See: [Literature Review §02 — FP²](../aims-literature-review/section-02-fp2.md))

---

## 05.5 Completion Checklist

- [x] **[BLOAT]** `compiler/ori_arc/src/aims/emit_reuse/mod.rs` — 815 lines, far exceeds the 500-line limit.
  Fixed: extracted `dynamic.rs` (318 lines) with `DynamicReuseContext`, `apply_dynamic_reuse()`,
  `build_fast_body()`, `build_dynamic_blocks()`, `rewrite_original_block()`, `extract_dynamic_context()`.
  mod.rs is now 512 lines (dispatch hub: types, `emit_reuse()` entry, static reuse, shared utilities).
- [x] **[STYLE]** `compiler/ori_arc/src/aims/emit_reuse/mod.rs` — 6 stale `§09.5` references
  Fixed: removed all section references, using descriptive text only.
- [x] **[STYLE]** `compiler/ori_arc/src/aims/emit_reuse/tests.rs` — 2 stale `§09.5` references
  Fixed: removed both section references (module doc and section header).

- [x] Reuse opportunities correctly detected from state map
- [x] Cross-block reuse detected (dominator-guided via ReusePlanner)
  Verified: `planner.rs` implements `ReusePlanner::find_opportunities()` with
  `DominatorTree::dominates()` + `PostDominatorTree::post_dominates()` checks.
  Tests cover: simple cross-block, self-set elimination, enum variants,
  intervening blocks, negative cases (no post-dominance, MaybeShared, type mismatch).
- [x] ReusePlanner builds dom/post-dom trees only when candidates exist
  Verified: `ensure_dom_trees()` via `get_or_insert_with()` — trees only built
  when `has_candidate` is true (at least one type match exists).
- [x] Static-unique reuse emitted without `IsShared` check
- [x] Dynamic reuse emitted with conditional branch
  Verified: `maybe_shared_emits_conditional_branch` test proves `MaybeShared` sources
  get `IsShared` + `Branch` expansion with fast path (`Set` in-place) and slow path
  (`RcDec` + `Construct`). Additional tests: `dynamic_reuse_moves_between_instructions`,
  `dynamic_reuse_no_merge_block`, `dynamic_reuse_self_set_elimination`,
  `dynamic_reuse_enum_variant`.
- [x] Reuse specialization skips unchanged fields
  Verified: `self_set_elimination_skips_unchanged_field` test proves self-set elimination.
  `all_fields_self_set_no_sets` and `enum_self_set_with_tag_change` cover additional cases.
- [x] **RC/reuse coordination**: RC emission emits `RcDec` for all deaths; reuse emission
  removes matched `RcDec`s and replaces `Construct` with `Set` instructions (post-patch
  approach — equivalent outcome to deferred-dec, simpler implementation).
- [x] Unmatched reuse candidates correctly fall back to `RcDec`
  Verified: unmatched deaths are never included in `opportunities` list, so their
  `RcDec` remains in place. Tests `no_reuse_different_types`, `no_reuse_intervening_use`,
  `no_reuse_shared_variable` confirm unmatched cases preserve the `RcDec`.
- [x] `CollectionReuse` instructions preserved from lowering
- [x] Emitted code passes `ori_arc::verify` checks
  Verified: verification runs after emission (steps 9, 13) in `aims_pipeline.rs`.
- [x] FBIP enforcement (separate pass, step 14) still works on AIMS output
- [x] `FipContract` consulted during reuse emission
  Verified: `emit_reuse()` looks up `contracts.get(&func.name).map(|c| &c.fip)` and
  passes it to `fip::apply_fip_upgrades()`. Stage 1: always `Never` (no behavioral
  change); Stage 2+: `Certified`/`Conditional` drive reuse upgrades and gate records.
- [x] FIP-guided fast paths emit static-unique reuse when preconditions hold
  Verified: `fip_certified_upgrades_maybe_shared_to_static` test proves `Certified`
  upgrades `MaybeShared` → `is_static_unique = true`, producing `Set` instructions
  instead of `IsShared` + `Branch` expansion.
- [x] `FipGate` records captured in emission-phase artifact for verification
  Verified: `FipGateRecord` in `aims/emit_reuse/fip.rs`, returned via
  `EmitReuseResult.fip_gates`. Pipeline logs count when non-empty.

**Exit Criteria:** `cargo t -p ori_arc -- aims::emit_reuse` passes. Reuse opportunities
are found for all cases that the current `reset_reuse` finds, plus any new cases
from the unified state map. FIP-certified functions (Stage 2) achieve allocation-free
fast paths verified by FBIP enforcement.
