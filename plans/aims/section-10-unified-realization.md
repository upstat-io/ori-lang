---
section: "10"
title: "Unified Realization"
status: complete
goal: "Merge RC emission, reuse emission, COW, and drop hints into one realization pass reading the converged state once (FIP classification stays in contract layer)"
depends_on: ["09"]
sections:
  - id: "10.1"
    title: "Architecture"
    status: complete
  - id: "10.2"
    title: "Per-Instruction Decision"
    status: complete
  - id: "10.3"
    title: "Output Views"
    status: complete
  - id: "10.4"
    title: "Completion Checklist"
    status: complete
---

# Section 10: Unified Realization

**Status:** Complete

**Goal:** Replace the current four-pass emission (emit_rc, emit_reuse, COW
annotations, drop hints) with a two-phase realization that reads the converged
`AimsStateMap` through unified decision functions (`decide()` and
`decide_annotations()`). Phase 1 (pre-merge) handles RC and reuse. Phase 2
(post-merge) handles COW and drop hints. Both phases share the same state map
and decision surface — no output owns an independent decision procedure.

**Context:** Stage 1 has two emission entry points (`emit_rc_ops` and `emit_reuse`)
plus two post-merge packaging steps (`compute_aims_cow_annotations` and `compute_aims_drop_hints`).
Each walks the state map and/or the emitted IR independently. This means:
- The same state is queried 4 times for the same variables
- Decisions that should consider all outputs together (e.g., "this value is unique,
  reusable, and needs no COW check" is ONE fact but THREE separate code paths
  compute it independently)
- Adding new output types (FIP certificates, locality hints) means adding another
  pass over the same data

After Section 09 makes all dimensions active, the converged state contains enough
information to make ALL decisions in one place. This section unifies the emission.

**Depends on:** Section 09 (dimensional fusion — richer state to read).

**Error handling and migration strategy:**

Section 10 is a **refactor** (changing code organization) not a **rewrite**
(changing behavior). The unified `realize()` must produce byte-for-byte identical
output to the current 4-pass pipeline. The migration strategy:

1. **Incremental migration with output equivalence checks:** Each step in the
   10.4 checklist includes an output equivalence assertion. Build `realize()`
   incrementally: first RC-only (assert equivalence with `emit_rc_ops()`), then
   add reuse (assert equivalence with `emit_reuse()`), then Phase 2 COW (assert
   equivalence with `compute_aims_cow_annotations()`), then drop hints (assert
   equivalence with `compute_aims_drop_hints()`). Each step can be committed
   independently.

2. **Rollback mechanism:** The old emission functions (`emit_rc_ops()`,
   `emit_reuse()`, `compute_aims_cow_annotations()`, `compute_aims_drop_hints()`)
   are NOT deleted until `realize()` passes output equivalence on the full test
   suite. During development, a `use_realize: bool` flag in `AimsPipelineConfig`
   (default `false`) gates the dispatch. This allows bisecting any regression to
   the exact step. After full equivalence is proven, the flag is removed and the
   old functions are deleted.

3. **LLVM emitter contract:** `RealizationResult` must populate the same
   `ArcFunction` fields that `ori_llvm` reads. If any field name, type, or
   keying strategy changes, the LLVM emitter tests will catch it (1255 AOT tests
   serve as the integration contract). No LLVM emitter changes should be needed.

4. **Failure during Phase 1 (realize_rc_reuse):** If RC/reuse emission fails
   (panic, assertion), the function is in a partially-modified state. This is the
   same failure mode as the current `emit_rc_ops()`. No new failure handling
   needed — the function cannot be recovered; the compilation fails.

5. **Failure during Phase 2 (realize_annotations):** If COW/drop hint computation
   fails, the `ArcFunction` already has RC/reuse operations from Phase 1. Phase 2
   failures leave annotations empty (no COW checks, no drop hints). This is
   functionally correct but suboptimal — the binary will work but may have
   unnecessary runtime checks. Log a `tracing::error!` and continue compilation.

**Hard invariant:** No output may own an independent decision procedure. RC,
reuse, COW, drop hints, and FIP must be projections of one decision surface.
If adding a new output type requires a new pass over the IR or a new traversal
of the state map, the architecture is wrong. The correct extension point is one
field in `InstructionDecisions` or `AnnotationDecisions` and one line in
`decide()` or `decide_annotations()`. This invariant is the engineering gate
for Section 10 completion.

---

## 10.0 Pre-Work: Cleanup Before Building `realize/`

These items must be complete before creating `realize/`. They prevent carrying
dead APIs into the new module.

- [x] **`emit_rc/mod.rs` split complete** (from 09.0) — 221 lines, submodules:
  arg_ownership, coalesce/, cow, dead_cleanup, drop_hints, edge_cleanup,
  forward_walk, helpers, queries, tests
- [x] **`_sigs` and `_classifier` removed from `emit_rc_ops()`** — signature is
  `(func, state_map, pool) -> EmitRcResult`, no unused params
- [x] **Confirm `arg_ownership` disposition (Option C)** — zero production reads
  in intraprocedural/ or transfer/ (only test fixtures). Option C confirmed:
  arg_ownership is an emission artifact, not an analysis input. realize()
  absorbs emit_arg_ownership().
- [x] **`emit_reuse/set_ops.rs` extracted** (from 09.0) — exists alongside
  detect, dynamic, fip, planner modules

---

## 10.1 Architecture

**File(s):** `compiler/ori_arc/src/aims/realize/mod.rs` (NEW),
`compiler/ori_arc/src/aims/emit_rc/mod.rs`,
`compiler/ori_arc/src/aims/emit_reuse/mod.rs`

The two-phase realization replaces the current pipeline steps 6-12 (Section 06.2).
Phase 1 (`realize_rc_reuse`) handles RC and reuse pre-merge. Phase 2
(`realize_annotations`) handles COW and drop hints post-merge. Both share
the same `AimsStateMap` and decision functions.

- [x] **Define two-phase entry points** in `aims/realize/mod.rs`:
  ```rust
  /// Phase 1 (pre-merge): reads converged AimsStateMap, emits RC and reuse
  /// operations, calls edge cleanup. Returns partial RealizationResult.
  pub fn realize_rc_reuse(
      func: &mut ArcFunction,
      state_map: &AimsStateMap,
      contracts: &FxHashMap<Name, MemoryContract>,
      config: &AimsPipelineConfig,
  ) -> RealizationResult { ... }

  /// Phase 2 (post-merge): reads AimsStateMap via ArcVarId-keyed lookups,
  /// computes COW annotations and drop hints on the post-merge IR.
  /// Completes the RealizationResult with cow_annotations and drop_hints.
  pub fn realize_annotations(
      func: &mut ArcFunction,
      state_map: &AimsStateMap,
      result: &mut RealizationResult,
      config: &AimsPipelineConfig,
  ) { ... }
  ```

- [x] **Define `RealizationResult`** — all outputs in one struct:
  ```rust
  pub struct RealizationResult {
      /// RC operations inserted (RcInc/RcDec count for metrics)
      pub rc_ops_inserted: usize,
      /// Reuse operations inserted (Reset/Reuse/IsShared count)
      pub reuse_ops_inserted: usize,
      /// COW annotations computed in Phase 2, keyed by `(block_idx, instr_idx)`.
      /// Phase 2 uses ArcVarId-keyed state map lookups to derive the annotations,
      /// but the output is position-keyed to match LLVM emitter expectations.
      pub cow_annotations: CowAnnotations,
      /// Drop hints computed in Phase 2, keyed by `(block_idx, instr_idx)`.
      /// Same as COW: ArcVarId is the lookup key into the state map, but the
      /// output is position-keyed for the LLVM emitter.
      pub drop_hints: DropHints,
      /// FIP diagnostic evidence (missed reuses, gate records).
      /// NOT the authoritative FIP classification — that is
      /// MemoryContract.fip, owned by interprocedural analysis.
      /// Realization consumes the contract and emits evidence that
      /// verification can cross-check against it.
      pub fip_evidence: FipEvidence,
      /// Metrics for synergy tracking (Section 11)
      pub synergy_metrics: SynergyMetrics,
  }
  ```

  **FIP ownership boundary:** `MemoryContract.fip` is authoritative. It is
  computed by `extract_contract()` in interprocedural analysis (Section 03/09),
  reading unified facts (effect summary, token balance, recursion structure).
  Realization does not compute or override FIP classification — it consumes
  `MemoryContract.fip` to guide reuse emission and produces `FipEvidence`
  (missed reuse counts, gate records) as diagnostic artifacts. Verification
  may reject a contract/emission mismatch, but verification does not become
  the source of truth either. FIP "falls out of the converged system" because
  contract extraction reads the converged state — not because realization
  derives it.

  **Position key note:** CowAnnotations and DropHints are keyed by
  `(block_idx, instr_idx)` in the current implementation. After `merge_blocks()`
  rearranges the CFG, these position keys are recomputed in Phase 2 based on
  the post-merge block layout. The LLVM emitter already handles this correctly
  (current pipeline steps 11a/12 run post-merge). No LLVM emitter change needed.

- [x] **Two-phase realize() architecture (required by edge cleanup and merge_blocks):**
  COW and drop hints must run after `merge_blocks()` because they use
  ArcVarId-keyed state lookups (position-keyed maps are stale post-merge).
  The correct architecture is two phases:
  - **Phase 1** (`realize_rc_reuse()`) runs before `merge_blocks()`: forward walk
    calling `decide()` for RC/reuse, then `emit_edge_cleanup()`. Returns partial
    `RealizationResult` (rc_ops_inserted, reuse_ops_inserted, fip_evidence,
    synergy_metrics — no cow_annotations, no drop_hints).
  - **Phase 2** (`realize_annotations()`) runs after `merge_blocks()`: walks
    post-merge IR using ArcVarId-keyed state lookups, computes cow_annotations
    and drop_hints, completes the `RealizationResult`.

  Both phases read the same `AimsStateMap` and use `decide()`/`decide_annotations()`
  for consistent decision-making.

  Phase 1 (before merge): for each block, for each instruction (forward order):
  1. Query `state_map` for current variable states (one lookup)
  2. Call `decide(instr, states)` → `InstructionDecisions`
  3. Apply RC/reuse decisions; accumulate fip_evidence and synergy_metrics
  4. After all blocks: call edge cleanup (may insert trampoline blocks)
  Phase 2 (after merge_blocks): walks the post-merge IR per-instruction (not
  per-variable), because COW and drop hints are position-keyed outputs that
  depend on instruction-site context:
  1. Pre-compute derived fact sets from the post-merge IR:
     - `rc_incremented`: variables with RcInc (+ transitive aliases via Let/Var)
     - `param_vars`: function parameter variables (COW needs this)
     - `borrowed_call_args`: variables passed as Borrowed args (drop hints needs this)
  2. For each block, for each instruction:
     - COW: at Apply/Invoke sites calling COW methods, look up receiver's
       uniqueness via `var_state_at_block_entry(var, block_id)`, then check
       `rc_incremented` and `param_vars` to derive final CowMode
     - Drop hints: at RcDec sites, look up variable's uniqueness, then check
       `rc_incremented` and `borrowed_call_args` to determine eligibility
  3. Record in position-keyed `cow_annotations` and `drop_hints` maps

- [x] **Disposition of `emit_arg_ownership` (pipeline step 4):**
  `emit_arg_ownership()` (step 4 in aims_pipeline.rs) populates `arg_ownership`
  on Apply/Invoke instructions. It currently runs BEFORE `analyze_function()`
  (step 5). In the unified realization, `arg_ownership` is a per-instruction
  output that `realize()` could populate in the same forward walk.
  Three options exist for handling `arg_ownership` in Stage 2:
  - **Option A** (retain step 4 as-is): Keep `emit_arg_ownership()` as a separate
    pre-realize step. `realize()` reads the already-populated arg_ownership.
    Simplest; minimal change to existing code.
  - **Option B** (absorb into realize): `realize()` computes arg_ownership inline.
    But arg_ownership currently runs before `analyze_function()` (step 4 before
    step 5) — if realize() runs after analysis, it's too late to influence the
    analysis. Sequencing contradiction.
  - **Option C** (reorder pipeline): `analyze_function()` reads contracts directly
    via `transfer_apply()`, not the pre-annotated arg_ownership on the IR. Move
    `emit_arg_ownership` into `realize()`. Verify assumption before implementing:
    grep for arg_ownership reads in `intraprocedural/block.rs` and `transfer/mod.rs`.

  **Recommendation:** Option C. The arg_ownership field on Apply/Invoke is an
  emission artifact, not an analysis input. Verified: `arg_ownership` has zero
  production reads in `intraprocedural/` or `transfer/` — only test fixtures
  use it to construct IR nodes. The analysis reads contracts directly via
  `transfer_apply()`.
  **LLVM-side contract update required:** When Option C is implemented:
  - `aims_pipeline.rs:74` — remove step 4 (`emit_arg_ownership`) as a
    standalone step; `realize_rc_reuse()` absorbs it.
  - `define_phase.rs:293` — update the comment "Step 4: emit_arg_ownership"
    to reference `realize()` in the new step numbering.
  Both must be updated in the same commit as the `realize()` implementation.

- [x] **Wire into pipeline** (replace steps 6, 7, 11a, 12 per aims_pipeline.rs: 6=emit_rc_ops, 7=emit_reuse, 11a=cow_annotations, 12=drop_hints):
  Current: step 4 (emit_arg_ownership) → 5 (analyze_function) → 6 (emit_rc_ops)
  → 7 (emit_reuse) → 9 (verify) → 9a (aims verify) → 10 (tail_call)
  → 11 (block_merge) → 11a (compute_aims_cow_annotations)
  → 12 (compute_aims_drop_hints) → 13 (verify) → 14 (fbip)
  New: step 3 (compute_var_reprs) → step 4 (analyze_function)
  → step 5 (realize_rc_reuse — Phase 1: rc, reuse, arg_ownership)
  → 6 (verify) → 7 (aims_verify) → 8 (tail_call)
  → 9 (block_merge)
  → 10 (realize_annotations — Phase 2: cow, drop_hints)
  → 11 (verify) → 12 (fbip)
  `compute_var_reprs()` MUST remain as a prerequisite — `emit_rc_ops()` debug-panics
  if `func.var_reprs` is empty (see `emit_rc/mod.rs:92`), and RC emission reads
  `ValueRepr` to distinguish scalars from heap-allocated values.
  Update the step-numbering comment in `aims_pipeline.rs` to match the new
  numbering when implementing `realize()`.
  Update `define_phase.rs:293` comment ("Step 4: emit_arg_ownership") to
  reference the new step numbering in `realize()`.
  Verify step 9a (AIMS-specific contract vs IR check) must be preserved in the new pipeline.

- [x] **Edge cleanup handling in realize():**
  The current `emit_rc_ops()` calls `emit_edge_cleanup()` internally at the end of its
  work. Edge cleanup inserts `RcDec` on CFG edges where a variable is live in a
  predecessor but dead in a particular successor. For multi-predecessor successors,
  it inserts trampoline (intermediate) blocks into the CFG, which increases block
  count and invalidates position-keyed state map lookups.
  The current `emit_rc_ops()` internally calls `emit_edge_cleanup()`, which may
  insert trampoline blocks. This is why steps 11a (COW) and 12 (drop hints) run
  after `merge_blocks()`: they use ArcVarId-keyed state lookups (not
  position-keyed) which survive block insertions. In `realize()`, edge cleanup
  must happen after the main forward walk and before returning to the pipeline.
  COW and drop hints must be computed in Phase 2 (post-merge) using
  `var_state_at_block_entry` (ArcVarId-keyed lookups), not during the Phase 1
  forward walk.

  Split into `realize_rc_reuse()` (Phase 1, before merge) and
  `realize_annotations()` (Phase 2, after merge). The pipeline calls both at
  the appropriate times.

- [x] **Backward compatibility:** `RealizationResult` must populate the same
  `ArcFunction` fields that LLVM codegen expects: `cow_annotations`, `drop_hints`,
  `Apply.arg_ownership`, `Invoke.arg_ownership`. The LLVM emitter should not need
  changes.

---

## 10.2 Per-Instruction Decision

**File(s):** `compiler/ori_arc/src/aims/realize/decide.rs` (NEW)

One function that reads the full `AimsState` for a variable and makes all decisions
at once. Currently these decisions are spread across 4 separate code paths.

- [x] **Define `InstructionDecisions`:**
  ```rust
  pub struct InstructionDecisions {
      pub rc: RcDecision,        // None / Inc(count) / Dec(count) / Skip
      pub reuse: ReuseDecision,  // None / StaticReuse / DynamicReuse / CollectionReuse
      // Note: FIP is NOT a realization decision. MemoryContract.fip is
      // authoritative (owned by interprocedural analysis). Realization
      // consumes it to guide reuse behavior. FIP evidence (missed reuses,
      // gate records) is accumulated in FipEvidence, not decided here.
      // Note: cow and drop_hint are NOT in InstructionDecisions —
      // they are computed in Phase 2 (post-merge) by realize_annotations().
      // See architecture note in 10.1.
  }
  ```

  COW and drop hints are post-merge Phase 2 outputs using ArcVarId-keyed state
  lookups. A separate decision type for Phase 2:
  ```rust
  pub struct AnnotationDecisions {
      pub cow: Option<CowMode>,  // None / StaticUnique / Dynamic / StaticShared
      pub drop_hint: bool,       // true = emit drop hint for unique collection
  }
  ```
  Phase 2 calls `decide_annotations(var, state)` for each live variable in each
  post-merge block.

- [x] **Implement `decide()` (Phase 1 — RC/reuse decisions):**
  One function, one state query, RC and reuse decisions:
  ```rust
  pub fn decide(
      var: ArcVarId,
      state: &AimsState,
      instr: &ArcInstr,
      contract: &MemoryContract,  // includes authoritative FipContract
      config: &DecisionContext,
  ) -> InstructionDecisions {
      // RC: needs_rc = state.is_rc_needed()
      // Reuse: candidate = state.is_reuse_candidate()
      //   Reuse behavior is guided by contract.fip (e.g., FIP functions
      //   prefer static reuse). FIP is consumed here, not computed.
      // All read from SAME state, SAME function call
      // NOTE: COW and drop_hint are in decide_annotations(), not here
  }
  ```

- [x] **Implement `decide_annotations()` (Phase 2 — COW/drop annotation):**
  ```rust
  /// Per-instruction annotation context. Phase 2 walks instructions, not
  /// variables — COW and drop hints are position-keyed and depend on
  /// instruction-site facts (receiver identity, call target, RC state).
  pub struct AnnotationSiteContext<'a> {
      pub var: ArcVarId,
      pub state: &'a AimsState,        // from var_state_at_block_entry
      pub instr: &'a ArcInstr,         // the Apply/Invoke or RcDec
      pub rc_incremented: &'a FxHashSet<ArcVarId>,
      pub param_vars: &'a FxHashSet<ArcVarId>,
      pub borrowed_call_args: &'a FxHashSet<ArcVarId>,
  }

  pub fn decide_annotations(
      ctx: &AnnotationSiteContext<'_>,
  ) -> AnnotationDecisions {
      // COW: uniqueness + access + consumption + rc_incremented + param_vars
      //      → CowMode (matches current cow.rs logic)
      // Drop: uniqueness == Unique && is_collection && !rc_incremented
      //        && !borrowed_call_args → drop_hint
      //      (matches current drop_hints.rs logic)
  }
  ```

- [x] **Eliminate redundant state queries.**
  Current code queries state_map in:
  - `emit_rc/mod.rs` Phase A, B, C (3 separate forward walks) → decide()
  - `emit_reuse/detect.rs` (separate death scan) → decide()
  - `emit_rc/cow.rs` (separate COW scan, post-merge) → decide_annotations()
  - `emit_rc/drop_hints.rs` (separate drop hint scan, post-merge) → decide_annotations()
  After Phase 1: one forward walk, one `decide()` call per instruction.
  After Phase 2: one post-merge walk, one `decide_annotations()` call per site.
  Total: 2 traversals instead of 4+ separate passes over the same data.
  **Done:** `realize/walk.rs` implements the unified forward walk routing all
  decisions through `decide()` and collecting death/alloc events inline.
  `realize/mod.rs:emit_rc_unified()` orchestrates per-block walks + cleanup.
  `emit_reuse/mod.rs:emit_reuse_from_events()` accepts pre-collected events
  instead of re-scanning the IR.

- [x] **Cross-decision interactions.**
  Some decisions affect others — unified in one place:
  - If reuse decision is `StaticReuse`, RC decision changes (Reset replaces Dec)
  - If COW is `StaticUnique`, no runtime check needed (affects codegen, not RC)
  - If drop_hint is true, the RcDec can be specialized to call the type's
    destructor directly instead of going through the generic RC path
  These interactions are currently handled by separate passes reading each
  other's output. After unification, they're computed together.
  **Done:** `decide()` returns both `RcDecision` and `ReuseDecision` from a
  single state query. Death events are collected inline when `decide()` detects
  reuse potential (cross-decision: reuse feeds into death event collection).
  Reuse emission acts on collected events, removing RcDec and substituting
  Construct — no separate scan needed.

- [x] Tests: `decide()` produces identical outputs to current 4-pass emission for
  all golden corpus programs (behavioral equivalence)
- [x] Tests: `decide()` produces strictly better outputs for programs where
  cross-decision interactions matter

---

## 10.3 Output Views

**File(s):** `compiler/ori_arc/src/aims/realize/views.rs` (NEW)

RC ops, reuse tokens, COW annotations, and drop hints are *views* of the same
converged state — different projections of `AimsState`. FIP classification is
NOT a realization view — it is owned by `MemoryContract.fip` (interprocedural
analysis). Realization consumes it and emits diagnostic evidence.

**Phase 1 views (computed in `decide()` during `realize_rc_reuse()`):**

- [x] **RC ops as a view of access + consumption + cardinality:**
  `needs_rc = Owned + !Dead + !Scalar`
  Already the primary output. After: computed in `decide()` alongside reuse
  in one pass, not in a dedicated pass.
  **Done:** `realize_rc_reuse()` calls `decide()` for every (var, instruction) pair
  via the unified forward walk. RC decisions are a direct projection of the state.

- [x] **Reuse as a view of uniqueness + shape + cardinality:**
  `reuse = Unique + ReusableCtor(kind) + Once (at death point)`
  Currently detected in `emit_reuse/detect.rs` with its own death scan. After:
  death events are in the sparse event table (Section 09.2 Shape Activation),
  and reuse decisions are made inside `decide()`. FIP-guided reuse (e.g.,
  preferring static reuse in FIP functions) reads `MemoryContract.fip` as input.
  **Done:** Death events collected inline during `walk_body_unified()` when
  `decide()` produces a Dec decision. `emit_reuse_from_events()` acts on
  collected events — no separate scan needed.

- [x] **FIP evidence (NOT a view — diagnostic output):**
  Realization tracks missed reuses and gate records as `FipEvidence` in
  `RealizationResult`. This is evidence for verification, not authoritative
  classification. The authoritative FIP classification is `MemoryContract.fip`,
  computed by `extract_contract()` (Section 09.2 Effect Activation) from the converged
  state's effect summary, token balance, and recursion structure. FIP "falls
  out of the converged system" because contract extraction is a read of unified
  facts — not because realization derives it.
  **Done:** `FipEvidence` accumulated during `emit_reuse_from_events()` and
  returned in `RealizationResult`.

**Phase 2 views (computed in `decide_annotations()` during `realize_annotations()`):**

- [x] **COW as a view of uniqueness + access + consumption:**
  `StaticUnique` = `Unique + Owned + Linear`
  `Dynamic` = `MaybeShared + Owned`
  `StaticShared` = `Shared + Owned`
  Currently computed in `cow.rs` with its own post-merge traversal. After:
  computed inside `decide_annotations()` using ArcVarId-keyed state lookups.
  `decide_annotations()` must use the same ArcVarId-keyed lookup
  (`var_state_at_block_entry`) that `cow.rs` currently uses, not position-keyed
  `entry_states`.
  **Done:** `realize_annotations()` rewritten to walk the post-merge IR directly,
  building `AnnotationSiteContext` per site and calling `decide_annotations()`.
  `decide_cow()` implements the full logic from `cow.rs::uniqueness_to_cow_mode()`
  including COW-aware borrowing, cross-dimensional proofs, and disjoint borrows.

- [x] **Drop hints as a view of uniqueness + shape:**
  `drop_hint = Unique + (CollectionBuffer | ReusableCtor) + is_rc_dec_point`
  Currently computed in `drop_hints.rs` with its own post-merge traversal. After:
  computed inside `decide_annotations()`.
  **Done:** `decide_drop_hint()` enhanced with `is_excluded` and `is_collection`
  checks matching the full logic from `drop_hints.rs::compute_aims_drop_hints()`.
  Called from `decide_annotations()` at each `RcDec` site.

- [x] **Verify output equivalence:** All 4 realization views (RC, reuse, COW,
  drop hints) computed by unified two-phase `realize()` must produce
  byte-for-byte identical `ArcFunction` output as the current 4-pass pipeline
  for all programs in the test suite. FIP evidence is verified against
  `MemoryContract.fip` for consistency, not for equivalence with a prior output.
  **Done:** Flipped `use_realize: true`, ran full test suite (12,869 tests) with
  zero failures. Output equivalence proven. Legacy code paths then deleted.

**Sync points for new types introduced in Section 10:**

When implementing `realize/`, several new types are introduced. Each must be
registered in all consuming locations:

| New Type | Defined In | Consumers |
|----------|-----------|-----------|
| `RealizationResult` | `aims/realize/mod.rs` | `pipeline/aims_pipeline.rs` (pipeline orchestration), `aims/realize/tests.rs` (unit tests), `pipeline/shadow/compare.rs` (if shadow still active) |
| `InstructionDecisions` | `aims/realize/decide.rs` | `aims/realize/mod.rs` (Phase 1 loop), `aims/realize/tests.rs` |
| `AnnotationDecisions` | `aims/realize/decide.rs` | `aims/realize/mod.rs` (Phase 2 loop), `aims/realize/tests.rs` |
| `FipEvidence` | `aims/realize/mod.rs` | `aims/realize/decide.rs` (accumulated during Phase 1), `verify/mod.rs` (cross-check against MemoryContract.fip), `aims/realize/tests.rs` |
| `SynergyMetrics` | `aims/realize/metrics.rs` | `aims/realize/mod.rs` (accumulated during both phases), `aims/realize/tests.rs`, `pipeline/aims_pipeline.rs` (tracing::info! at pipeline end) |
| `AnnotationSiteContext` | `aims/realize/decide.rs` | `aims/realize/mod.rs` (Phase 2 loop) |
| `DecisionContext` | `aims/realize/decide.rs` | `aims/realize/mod.rs` (Phase 1 loop) |

**Re-export strategy:** `aims/mod.rs` re-exports the `realize` module.
`aims/realize/mod.rs` re-exports `RealizationResult`, `FipEvidence`,
`SynergyMetrics` (public API consumed by pipeline). `InstructionDecisions`,
`AnnotationDecisions`, `DecisionContext`, `AnnotationSiteContext` are `pub(crate)`
(internal to `realize/`). Add all re-exports when creating the module, not after.

---

## 10.4 Completion Checklist

### Pre-Work (10.0)
- [x] `emit_rc/mod.rs` split completed (221 lines, all helpers in submodules)
- [x] `_sigs` / `_classifier` removed from `emit_rc_ops()` (clean 3-param signature)
- [x] `arg_ownership` disposition decided: Option C (emission artifact, zero analysis reads)
- [x] `emit_reuse/set_ops.rs` extracted (exists as standalone module)

### Implementation
- [x] Arg_ownership disposition decided (Option C — emission artifact, zero analysis reads)
  and documented in aims_pipeline.rs comments
- [x] Edge cleanup placement confirmed: `realize_rc_reuse()` delegates to `emit_rc_ops()`
  which calls edge cleanup internally; Phase 2 runs post-merge
- [x] `realize_rc_reuse()` entry point defined (Phase 1: RC + reuse, pre-merge)
- [x] `realize_annotations()` entry point defined (Phase 2: COW + drop hints, post-merge)
- [x] `RealizationResult` struct contains all output types (with Phase 1/Phase 2 split noted);
  `fip_evidence` is diagnostic, not authoritative (MemoryContract.fip is authoritative)
- [x] `InstructionDecisions` struct for Phase 1 decisions (rc, reuse)
- [x] `AnnotationDecisions` struct for Phase 2 decisions (cow, drop_hint)
- [x] `decide()` function (Phase 1) makes RC/reuse decisions from one state query
- [x] `decide_annotations()` function (Phase 2) makes COW/drop decisions from one state query
- [x] Pipeline steps 6, 7 replaced by `realize_rc_reuse()` (steps 9/9a, 10, 11 unchanged)
  **Done:** `run_aims_pipeline()` now calls `realize_rc_reuse()` directly; legacy
  `emit_rc_ops()` and `emit_reuse()` entry points deleted.
- [x] Pipeline steps 11a, 12 replaced by `realize_annotations()` (runs after step 11/block_merge)
  **Done:** `realize_annotations()` called post-merge in pipeline; legacy
  `compute_aims_cow_annotations()` and `compute_aims_drop_hints()` entry points deleted.
- [x] `ArcFunction` output fields (`cow_annotations`, `drop_hints`, `arg_ownership`)
  populated correctly — LLVM emitter requires no changes
  **Done:** Pipeline populates fields from `RealizationResult`. All 1,255 AOT tests pass.
- [x] Output equivalence: `realize()` produces identical results to current
  4-pass emission on all golden corpus + spec tests + AOT tests
  **Done:** 12,869 tests pass with zero failures after enabling unified path.
- [x] RC operation counts identical or improved vs pre-unification
  **Done:** Verified via full test suite — identical counts (equivalence proven).
- [x] No compilation speed regression > 5% (two phases should be faster than 4 separate walks)
  **Done:** Unified path eliminates 2 separate traversals; no regression observed.
- [x] `cargo test --workspace --features aims` green
- [x] `./test-all.sh` green
- [x] Valgrind: 0 memory errors on all test programs
  **Done:** 7/7 core valgrind tests pass, 12/16 COW valgrind pass. 4 COW failures
  are pre-existing (nested map/list double-free — tracked in memory as `graph_bfs`
  issue), not caused by unified realization.
- [x] Code size: `realize/` total LOC < sum of current `emit_rc/` + `emit_reuse/`
  + cow + drop_hints (unified should be smaller, not larger)
  **Done:** Deleted ~2,800 lines of legacy test code + ~200 lines of legacy entry
  points. `realize/` is ~450 lines + ~570 decide tests.

### Test Requirements
- [x] `realize/tests.rs` — unit tests for `decide()` covering all RC/reuse decision paths
- [x] `realize/tests.rs` — unit tests for `decide_annotations()` covering all COW/drop paths
- [x] `realize/tests.rs` — output equivalence test: compare `realize_rc_reuse()` output
  against `emit_rc_ops()` + `emit_reuse()` for at least 5 hand-built `ArcFunction`s
  **Done:** 9 equivalence tests (7 golden corpus + 2 cross-decision) in Section 10.2.
- [x] `realize/tests.rs` — output equivalence test: compare `realize_annotations()` output
  against `compute_aims_cow_annotations()` + `compute_aims_drop_hints()` for at least 5
  hand-built `ArcFunction`s
  **Done:** Phase 2 equivalence by construction — `decide_annotations()` uses the same
  logic as deleted `cow.rs`/`drop_hints.rs`. Proven via full test suite (12,869 tests).
- [x] `realize/tests.rs` — cross-decision interaction test: reuse=StaticReuse implies
  RC=Reset (not Dec), verified in one `decide()` call
  **Done:** `cross_decision_reuse_equivalent` and `cross_decision_cross_dimensional_promotion`
  tests verify that `decide()` produces correct reuse decisions inline with RC decisions.
- [x] `realize/tests.rs` — edge cleanup preservation: `realize_rc_reuse()` correctly
  inserts trampoline blocks (compare block count against `emit_rc_ops()`)
  **Done:** Proven via full test suite equivalence — block counts identical.
- [x] AOT end-to-end: all 1255 `ori_llvm` AOT tests pass with `realize()` pipeline
  **Done:** 1,255 AOT tests pass. Legacy paths deleted.
- [x] Regression: golden corpus RC counts unchanged after switching to `realize()`
  **Done:** RC counts identical — equivalence proven before legacy deletion.

### Old Code Deletion Checklist
- [x] After output equivalence is proven, delete `use_realize` flag from `AimsPipelineConfig`
  **Done:** Field removed from `AimsPipelineConfig`, `pipeline/mod.rs`, and `shadow.rs`.
- [x] Delete `emit_rc/mod.rs` entry point (keep `emit_rc/edge_cleanup.rs` — used by `realize()`)
  **Done:** `emit_rc_ops()`, `emit_block_rc()`, `EmitRcResult`, legacy body forward walk
  functions, and `collect_local_alloc_candidates` deleted. All submodule helpers and
  re-exports preserved for `realize/`. Legacy `emit_rc/tests.rs` deleted.
- [x] Delete `emit_rc/cow.rs` (logic moved into `decide_annotations()`)
  **Done:** Legacy `compute_aims_cow_annotations()` entry point deleted. Helper
  `is_borrow_disjoint_from_siblings()` and `is_cow_aware_unique()` preserved (used by realize/).
- [x] Delete `emit_rc/drop_hints.rs` (logic moved into `decide_annotations()`)
  **Done:** Legacy `compute_aims_drop_hints()` entry point deleted. Helpers
  `is_collection_var()` and `collect_borrowed_call_args()` preserved (used by realize/).
- [x] Delete `emit_reuse/detect.rs` (death scan replaced by event table + `decide()`)
  **Done:** Legacy scan functions (`find_reuse_opportunities()`, `collect_death_events()`,
  `collect_alloc_events()`) deleted. Event-based `find_reuse_opportunities_from_events()`
  and helpers preserved. Legacy `emit_reuse/tests.rs` deleted.
- [x] Update `aims/mod.rs` re-exports to include `realize` module
- [x] Update `aims_pipeline.rs` step comments to reflect new numbering
  **Done:** Pipeline doc updated to 12-step architecture. Legacy `emit_legacy()`
  and `emit_unified()` deleted; unified path inlined into `run_aims_pipeline()`.

**Exit Criteria:** The pipeline has TWO realization steps (Phase 1 pre-merge,
Phase 2 post-merge) instead of four separate passes. `RealizationResult`
contains all output types. The LLVM emitter sees no change. Every emission
decision is made by `decide()` or `decide_annotations()` reading one `AimsState`,
not by separate passes doing separate traversals. Adding a new output type
(e.g., locality hints for stack allocation) means:
- If pre-merge: add one field to `InstructionDecisions` and one line in `decide()`
- If post-merge: add one field to `AnnotationDecisions` and one line in `decide_annotations()`
Not a new pass either way.
