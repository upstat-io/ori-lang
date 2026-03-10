---
section: "06"
title: "Pipeline Integration"
status: not-started
reviewed: true  # 2026-03-10
goal: "Wire AIMS into pipeline.rs, replacing ~15 analysis/emission steps with the unified system"
inspired_by:
  - "ori_arc pipeline (compiler/ori_arc/src/pipeline.rs)"
depends_on: ["04", "05"]
sections:
  - id: "06.1"
    title: "Feature-Flagged Dual Pipeline"
    status: not-started
  - id: "06.2"
    title: "New Pipeline Flow"
    status: not-started
  - id: "06.3"
    title: "Old Pass Removal"
    status: not-started
  - id: "06.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Pipeline Integration

**Status:** Not Started
**Goal:** Replace the ~15 analysis/emission steps in `pipeline.rs` with the
unified AIMS analysis + emission, behind a feature flag for safe comparison.
Once validated, remove the old passes.

**Context:** The current `run_arc_pipeline_all` orchestrates interprocedural analysis
(borrow inference, uniqueness analysis) plus per-function processing including
~15 steps in a specific load-bearing order (var_reprs, derived ownership, liveness,
COW annotation, RC insertion, edge cleanup, dom/post-dom rebuild, second liveness,
reset/reuse, expand, RC identity, RC elimination, tail call, block merge, drop hints,
FBIP). AIMS collapses the analysis and RC emission portion into fewer steps:
interprocedural analysis → apply ownership → arg ownership → intraprocedural analysis
→ RC emission → reuse emission → COW annotations. The remaining steps (var_reprs,
verify, tail_call, block_merge, drop_hints, FBIP) are unchanged.

**Depends on:** Sections 04, 05 (RC and reuse emission).

---

## 06.1 Feature-Flagged Dual Pipeline

**File(s):** `compiler/ori_arc/src/pipeline.rs`

During development, both pipelines coexist behind a feature flag.

- [ ] Add `aims` feature to `ori_arc/Cargo.toml`
- [ ] **Forward `aims` feature through dependent crates** (required — without this, the
  feature flag has no effect on the actual compilation pipeline):
  - `ori_llvm/Cargo.toml`: add `aims` feature forwarding `ori_arc/aims`:
    ```toml
    [features]
    aims = ["ori_arc/aims"]
    ```
  - `oric/Cargo.toml`: add `aims` feature forwarding through `ori_llvm` and `ori_arc`:
    ```toml
    [features]
    default = ["llvm"]
    llvm = ["dep:ori_llvm", "dep:ori_arc"]
    aims = ["ori_arc/aims", "ori_llvm?/aims"]
    ```
  - The `?` syntax on `ori_llvm?/aims` ensures `aims` only activates on `ori_llvm`
    when `llvm` is also enabled (since `ori_llvm` is optional).

- [ ] **Update all pipeline entry points** — there are THREE call sites, not just
  `run_arc_pipeline_all`:
  1. `ori_arc::run_arc_pipeline_all()` — called by `oric/src/arc_dot/mod.rs` and
     `oric/src/arc_dump/mod.rs` (batch path)
  2. `ori_arc::run_arc_pipeline()` — called DIRECTLY by `ori_llvm` at
     `codegen/function_compiler/define_phase.rs:279` (per-function AOT) and
     `:370` (per-lambda AOT)
  3. `ori_arc::run_uniqueness_analysis()` — called DIRECTLY by `ori_llvm` at
     `evaluator/compile.rs:166` (JIT) and `oric` at
     `commands/codegen_pipeline.rs:320` (AOT)

  All three functions must branch on `#[cfg(feature = "aims")]`. If only
  `run_arc_pipeline_all` is replaced, the AOT/JIT paths (which call
  `run_arc_pipeline` + `run_uniqueness_analysis` separately) will keep
  using the old pipeline.

  Strategy: the cleanest approach is to make `run_arc_pipeline` itself
  branch internally, since ALL callers (batch and direct) funnel through it.
  The interprocedural analysis (`run_uniqueness_analysis`) should also branch
  to use AIMS signatures when the feature is active.

  
- [ ] **FOURTH entry point: `annotate_arg_ownership()`** — called directly by `ori_llvm`
  at `define_phase.rs:272` and `:363` BEFORE `run_arc_pipeline()`. The AIMS pipeline
  replaces this with `aims::emit_arg_ownership()` (step 4 in Section 06.2). The
  `#[cfg(feature = "aims")]` branch must be INSIDE `annotate_arg_ownership` or the
  callers must be updated. Since this function is called from 3 locations (2 in
  `define_phase.rs` + 1 in `run_arc_pipeline_all`), the cleanest approach is to make
  it branch internally like `run_arc_pipeline`.

  **FIFTH concern: `ori_llvm` FunctionCompiler direct ownership write.**
  `process_arc_function()` in `define_phase.rs:259-262` directly sets
  `ArcParam.ownership` from `AnnotatedSig` BEFORE calling `annotate_arg_ownership`
  or `run_arc_pipeline`. When the `aims` feature is active, this direct write must
  be skipped — AIMS sets `ArcParam.ownership` in step 2 via `apply_ownership()`.
  The `#[cfg(feature = "aims")]` branch in `run_arc_pipeline` should overwrite any
  pre-set ownership values. Alternatively, gate the direct write in
  `process_arc_function` with `#[cfg(not(feature = "aims"))]`.

  Public API functions requiring `#[cfg(feature = "aims")]` branching:
  1. `run_arc_pipeline()` — per-function pipeline
  2. `run_arc_pipeline_all()` — batch pipeline
  3. `run_uniqueness_analysis()` — interprocedural uniqueness
  4. `annotate_arg_ownership()` — per-call-site ownership annotation
  5. `ori_llvm` `FunctionCompiler::process_arc_function()` — direct
     `ArcParam.ownership` write (gate or overwrite)

- [ ] In `run_arc_pipeline`, branch on feature flag:
  
  ```rust
  #[cfg(feature = "aims")]
  {
      // AIMS per-function pipeline
      // Interprocedural signatures already computed by caller
      // (run_arc_pipeline_all or ori_llvm FunctionCompiler)
      func.var_reprs = ir::compute_var_reprs(func, classifier, pool);  // step 3
      aims::emit_arg_ownership(func, &aims_contracts, builtins, interner, pool);  // step 4
      let state_map = aims::analyze_function(func, classifier, &aims_contracts);  // step 5
      aims::emit_rc_ops(func, &state_map, &aims_contracts, classifier);  // step 6
      aims::emit_reuse(func, &state_map, classifier, pool);  // step 7
      // step 8: reserved (COW annotations moved to after block_merge)
      verify(func);  // step 9
      detect_and_rewrite_tail_calls(func);  // step 10
      merge_blocks(func);  // step 11
      aims::emit_cow_annotations(func, &state_map);  // step 11a — after merge
      aims::emit_drop_hints(func, &state_map);  // step 12 — after merge
      verify(func);  // step 13
      // step 14: fbip enforcement — unchanged
  }

  #[cfg(not(feature = "aims"))]
  {
      // Current pipeline (unchanged)
      // ... existing analysis passes ...
  }
  ```

- [ ] In `run_arc_pipeline_all` and `run_uniqueness_analysis`, branch similarly
  so the batch path uses AIMS interprocedural analysis.

- [ ] **Testing with feature flag** — `./test-all.sh` does NOT forward arbitrary
  cargo features. Do NOT rely on `./test-all.sh --features aims`. Instead:
  - Run `cargo test --workspace --features aims` for Rust unit tests
  - Run `cargo build --features aims` then `./target/debug/ori test tests/` for spec tests
  - Run `cargo build --features aims --release` then
    `./target/release/ori test --backend=llvm tests/` for LLVM spec tests
  - Run `cargo test -p ori_llvm --features aims` for AOT tests
  - Consider adding an `aims` variant to `test-all.sh` later once the pipeline is stable
- [ ] Add CI job for `--features aims` (initially allowed to fail)
- [ ] **Shadow comparison reporting (Stage 1A)**:
  The shadow analysis comparison results are surfaced via:
  1. `tracing::info!` for per-function match/improvement/regression details
     (visible with `ORI_LOG=ori_arc::aims=info`)
  2. A summary printed at the end of `run_arc_pipeline_all` showing total
     matches, improvements, and regressions across all functions
  3. If any REGRESSION is found (AIMS weaker than old pipeline), print a
     `tracing::warn!` with the function name and the specific dimension that
     regressed. This makes regressions immediately visible in CI output.
  4. The `ShadowComparisonReport` is accumulated internally and NOT returned
     from `run_arc_pipeline_all`. The public API signature is unchanged.
     - Results are surfaced via `tracing::info!`/`tracing::warn!` (points 1-3).
     - **Programmatic access for tests**: Return `ShadowComparisonReport` from
       `run_aims_pipeline_all()` (the internal `pub(crate)` function) and store
       it on `AimsPipelineConfig` or return it as a second element of the result
       tuple. Test harnesses within `ori_arc` access it directly via the internal
       API. External crates (`ori_llvm`, `oric`) do not need programmatic access
       — `tracing` log capture is sufficient for CI.
     - **Rejected alternative**: Thread-local `last_shadow_report()` was
       considered but rejected — thread-locals are fragile under parallel test
       runners (`cargo test` runs tests in threads by default) and would produce
       nondeterministic cross-contamination between tests. The internal return
       value approach is simpler and race-free.

**Migration rules (from improvements.md Change 8):**
1. Introduce new AIMS-backed implementations behind compatibility wrappers.
2. Keep old public names valid during migration.
3. Convert all call sites only after both backends are feature-selectable.

**Required feature plumbing:**
- `aims` feature on `ori_arc/Cargo.toml`
- Forwarding `aims` feature on `ori_llvm/Cargo.toml` → `ori_arc/aims`
- Forwarding `aims` feature on `oric/Cargo.toml` → `ori_arc/aims`, `ori_llvm?/aims`
- Update `test-all.sh` to support AIMS feature selection (or document manual commands)

Without this, verification instructions in Section 08 are not executable.

---

## 06.2 New Pipeline Flow

**File(s):** `compiler/ori_arc/src/pipeline.rs`

The AIMS pipeline is dramatically simpler:

```
 Interprocedural (once across all functions):
 1. aims::analyze_program()           — NEW (interprocedural contracts — replaces
                                         infer_borrows_scc + run_uniqueness_analysis)
 2. aims::apply_ownership()           — NEW (populate ArcParam.ownership on each
                                         function — replaces apply_borrows)

 Per-function:
 3. compute_var_reprs()               — KEEP (fill ValueRepr per var)
3a. aims::normalize_function()        — NEW Stage 3 (TRMC normalization, constructor
                                         context extraction — no-op in Stage 1).
                                         Returns `NormalizationResult { was_transformed: bool,
                                         context_metadata: Vec<ContextRegion> }`.
                                         In Stage 1: returns `{ false, vec![] }`.
                                         The analysis (step 5) checks `was_transformed`:
                                         if false, skips all constructor-context event tracking.
                                         This avoids any overhead from TRMC in Stage 1.
 4. aims::emit_arg_ownership()        — NEW (populate arg_ownership on Apply/Invoke
                                         — replaces annotate_arg_ownership)
 5. aims::analyze_function()          — NEW (per-function state map — replaces
                                         infer_derived_ownership + compute_refined_liveness
                                         + cow_annotations computation)
 6. aims::emit_rc_ops()               — NEW (insert RcInc/RcDec — replaces
                                         rc_insert + rc_identity + rc_elim)
 7. aims::emit_reuse()                — NEW (insert Reset/Reuse/IsShared — replaces
                                         detect_reset_reuse_cfg + expand_reset_reuse)
 8. (no-op — COW annotations are at step 11a, AFTER block_merge)
 9. verify()                          — KEEP (sanity checks)
10. detect_tail_calls() + rewrite()   — KEEP (tail call → loop)
                                         NOTE: This pass performs the actual tail-call-to-loop
                                         rewrite on the IR. It is independent of the "tail-call
                                         preservation" check in Section 03 (interprocedural), which
                                         is a syntactic check during analysis (Apply whose dst is
                                         immediately returned) used for contract refinement.
11. merge_blocks()                    — KEEP (CFG cleanup)
11a. aims::emit_cow_annotations()     — NEW (derive CowAnnotations by combining
                                         per-variable uniqueness facts (from analysis,
                                         keyed by ArcVarId) with post-merge IR positions.
                                         A packaging step, not a second analysis. Runs
                                         AFTER block_merge. Identifies COW operations
                                         by function name, not position.)
12. aims::emit_drop_hints()           — NEW (derive DropHints by combining per-variable
                                         uniqueness facts with post-merge RcDec positions.
                                         A packaging step, not a second analysis. Replaces
                                         compute_drop_hints. Must run AFTER block_merge.)
13. verify()                          — KEEP (final sanity check)
14. fbip enforcement                  — KEEP (separate read-only diagnostic
                                         pass on final IR — check_fbip_enforcement
                                         + is_auto_fbip, unchanged from current)
```

That is 14 steps total (2 interprocedural + 12 per-function), replacing the
current ~22 steps. Steps 1-8 replace ~15 analysis/emission steps.

- [ ] Implement `run_aims_pipeline_all()` as **internal implementation** called from
  within `run_arc_pipeline_all()` when `#[cfg(feature = "aims")]` is active:
  - Step 1: Compute `MemoryContract` for all functions via `aims::analyze_program()`
  - Step 2: Apply ownership to function parameters via `aims::apply_ownership()`:
    This sets `ArcParam.ownership` on each `ArcFunction.params[i]` based on the
    computed `MemoryContract.params[i].access`. Replaces `borrow::apply_borrows()`.
    **Must happen before per-function processing** because the LLVM emitter reads
    `ArcParam.ownership` from the function signature.
  - Step 3: Per-function loop calling `run_aims_pipeline()` (steps 3-14)
  - Return `Vec<ArcProblem>` (FBIP violations) matching the current API
  > **Warning: Parameter count.** The current `run_arc_pipeline` takes 7 parameters.
  > `run_aims_pipeline` should use a config struct per hygiene rules (>3-4 params -> config struct).
  > Define `AimsPipelineConfig { classifier, contracts, pool, interner, builtins, verify_arc }`.
- [ ] Implement `run_aims_pipeline()` as **internal implementation** called from within
  `run_arc_pipeline()` when `#[cfg(feature = "aims")]` is active. The public API
  functions (`run_arc_pipeline`, `run_arc_pipeline_all`, `run_uniqueness_analysis`,
  `annotate_arg_ownership`) branch internally — callers never see the AIMS functions
  directly. The `run_aims_*` functions are `pub(crate)` implementation details.
  **Critical: THREE callers of `run_arc_pipeline` must work unchanged:**
  1. `run_arc_pipeline_all()` — calls `run_arc_pipeline()` in its per-function loop
     (callers: `oric/arc_dot`, `oric/arc_dump`)
  2. `ori_llvm` `FunctionCompiler::compile_function_arc()` — calls `run_arc_pipeline()`
     directly at `define_phase.rs:279` for AOT per-function compilation
  3. `ori_llvm` `FunctionCompiler::declare_and_process_lambda()` — calls `run_arc_pipeline()`
     directly at `define_phase.rs:370` for lambda compilation

  Additionally, `run_uniqueness_analysis()` is called directly by:
  - `ori_llvm` `evaluator/compile.rs:166` (JIT path)
  - `oric` `commands/codegen_pipeline.rs:320` (AOT codegen path)

  The AIMS equivalent must maintain the same public API surface so that all
  callers work regardless of which pipeline is active. The `#[cfg(feature = "aims")]`
  branch should be INSIDE the existing function signatures, not a separate function,
  to avoid requiring changes to all callers during migration.
- [ ] **Dominator tree timing**: RC emission (step 6) may insert edge cleanup
  (trampoline blocks for critical edges), which modifies the CFG. Reuse emission
  (step 7) needs dominator trees for cross-block reuse detection (see Section 05
  ReusePlanner). Therefore: build dominator trees ONCE, between steps 6 and 7,
  after any CFG-modifying edge cleanup is complete. Do NOT build dom trees before
  RC emission — they would be immediately invalidated.
- [ ] Verify: no liveness recomputation needed (state map is complete)
- [ ] Verify: tail_call and block_merge work on AIMS output (they only read the IR
  structure, not analysis metadata)
- [ ] **block_merge invalidation**: `merge_blocks()` (step 11) renumbers blocks and
  reindexes instructions. After this point:
  - `AimsStateMap` block/instr indices are stale — do NOT query the state map
    by position after block_merge. Per-variable facts (uniqueness, access) are
    still valid since they are keyed by `ArcVarId`, not position.
  - RC emission (step 6) and reuse emission (step 7) complete before step 11.
  - COW annotations (step 11a) run AFTER merge — they walk the final IR and
    identify COW operations by semantic content, using per-variable uniqueness
    from the analysis (see Section 04.3 decision).
  - Drop hints (step 12) run AFTER merge — they walk the final IR, identify
    each RcDec's target variable, and look up per-variable uniqueness (keyed by
    `ArcVarId`) to determine drop-hint eligibility. No positional state map lookup.
  - `AimsEvent` entries stored for diagnostics (FipGate, LocalAllocCandidate) must
    either be discarded or re-keyed after merge. In v1, events consumed by emission
    (RC emission step 6, reuse emission step 7) are consumed before merge, so
    position-keyed events are valid at point of use.
  - Events needed for Section 08 verification (shadow comparison, allocation
    counting) should be preserved in a position-independent form (e.g., keyed by
    `ArcVarId` or function-level summaries) BEFORE block_merge runs. The
    `ShadowComparisonReport` (Section 06.1) accumulates these during emission.

---

## 06.3 Old Pass Removal

**File(s):** Multiple files across `compiler/ori_arc/src/`

Once AIMS is validated (all tests pass, RC count ≤ old pipeline), remove the
old analysis passes.

- [ ] Remove old modules (keep behind feature flag initially):
  - `borrow/` → replaced by `aims::interprocedural` (except `builtins/` which provides
    `BuiltinOwnershipSets` — retained or adapted)
  - `liveness/` → replaced by `aims::intraprocedural`
  - `rc_insert/` → replaced by `aims::emit_rc`
  - `rc_elim/` → replaced by `aims::emit_rc` (no separate elimination)
  - `rc_identity/` → replaced by `aims::emit_rc` (identity built into analysis)
  - `uniqueness/` → replaced by `aims::interprocedural` + `aims::intraprocedural`
  - `reset_reuse/` → replaced by `aims::emit_reuse`
  - `expand_reuse/` → replaced by `aims::emit_reuse` (emits expanded form directly)
- [ ] Retain with possible adaptation:
  - `ownership/` — defines `Ownership`, `DerivedOwnership`, `AnnotatedSig` types
    (may be replaced by `MemoryContract` or retained for compatibility)
  - `drop/` — computes `DropInfo`/`DropKind` per type for LLVM codegen (independent of AIMS)
  
  - `fbip/` — `check_fbip_enforcement` and `is_auto_fbip` remain UNCHANGED;
    they run on the final `ArcFunction` (post tail_call, block_merge, drop_hints)
    and read `ArcFunction.cow_annotations` and block instructions. AIMS produces
    semantically equivalent annotations (same `CowMode` per COW operation, same
    drop-hint coverage) via a different production path and timing (post-merge
    walk vs pre-merge computation). The LLVM emitter and FBIP enforcement see
    equivalent final annotations. No input shift needed.

- [ ] Update `lib.rs` exports to use AIMS types.
  **WARNING**: `lib.rs` currently re-exports ~40 symbols from the old passes.
  These are the public API of `ori_arc` consumed by `oric` and `ori_llvm`.
  Key re-exports that AIMS must maintain or replace:
  - `run_arc_pipeline`, `run_arc_pipeline_all`, `run_uniqueness_analysis` (from `pipeline`)
  - `infer_borrows_scc`, `apply_borrows`, `BuiltinOwnershipSets` (from `borrow`)
  - `AnnotatedSig`, `AnnotatedParam`, `Ownership`, `DerivedOwnership` (from `ownership`)
  - `CowAnnotations`, `CowMode`, `DropHints`, `Uniqueness`, `UniquenessSummary` (from `uniqueness`)
  - `LiveSet`, `RefinedLiveness` (from `liveness`)
  During migration, keep re-exports pointing to either old or new types.
  Removing them without replacements will break `ori_llvm` and `oric`.
- [ ] Update `ori_llvm` consumers:
  - `ArcIrEmitter` reads `func.cow_annotations`, `func.drop_hints`, `func.var_reprs`,
    `func.tail_calls` — these fields must be populated by AIMS identically
  - `emitter_utils.rs` queries `cow_annotations` by `(block_index, instr_index)` key
  - `rc_ops.rs` queries `drop_hints` for unique collection drops
  - **No ArcFunction struct changes needed** — AIMS populates the same fields
- [ ] Update `oric` consumers:
  - `oric` calls `run_arc_pipeline_all` as the sole entry point
  - AIMS must provide an equivalent function with the same signature
  - The `cache` feature serializes `ArcFunction` — verify AIMS output is
    cache-compatible (no new non-skipped fields).
  The `cache` feature serializes `ArcFunction` via serde. Fields marked `#[serde(skip)]`
  are recomputed. Any new AIMS fields on `ArcFunction` must also be `#[serde(skip)]`
  or the cache format breaks.
- [ ] Remove old pipeline from `pipeline.rs`

### Cleanup

- [ ] **[STYLE]** Verify `_enforce_type_tag_exhaustiveness` lint attrs in enforcement crates
  use `#[expect]` not `#[allow]`. The `borrow/mod.rs` instance has been fixed; check:
  - `compiler/ori_eval/src/methods/mod.rs`
  - `compiler/ori_types/src/infer/expr/methods/mod.rs`
  - `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs`
- [ ] **[STYLE]** `compiler/ori_arc/src/pipeline.rs:37` — Current `run_arc_pipeline` takes 7 positional parameters. When creating `run_aims_pipeline`, use a config struct instead of replicating this pattern.
- [ ] **[NOTE]** `compiler/ori_arc/src/ir/mod.rs` — At 431 lines (excluding tests), this file is approaching the 500-line limit. The AIMS plan does NOT add types here (AIMS types go in `aims/`), but be aware that any future additions would push it over. No action needed now.
- [ ] **[NOTE]** `compiler/ori_arc/src/rc_elim/eliminate.rs` — At 439 lines, approaching the 500-line limit. AIMS replaces this file entirely (Section 06.3), so no split needed -- just remove cleanly.

- [ ] **[GAP]** `compiler/ori_arc/src/rc_insert/edge_cleanup.rs` — Section 04.1 originally referenced `edge_cleanup::split_critical_edges(func)` which does not exist. The actual function is `insert_edge_cleanup(func, classifier, liveness, borrowed_params, global_borrows, pool)` and is `pub(super)` (only accessible within `rc_insert`). AIMS must either promote it to `pub(crate)` and move to `graph/`, or reimplement. Section 04.1 has been corrected.
- [ ] **[STYLE]** `compiler/ori_arc/src/ir/instr.rs:5,17,74,88,106,110,165,200,258,341` — Stale section references throughout module doc, type doc, and method docs: "Section 07" (rc_insert), "Section 07.1" (liveness), "Section 07.6" (reset_reuse), "Section 08" (rc_elim), "Section 09" (expand_reuse). Update all to current pipeline pass names when AIMS touches ArcInstr transfer functions.
- [ ] **[STYLE]** `compiler/ori_arc/src/ir/mod.rs:194` — `ArcParam` doc references "Section 06.2" (old numbering). Update to "borrow inference" when AIMS replaces borrow application.
- [ ] **[STYLE]** `compiler/ori_arc/src/ir/mod.rs:303` — `ArcTerminator::substitute_var` doc references "Section 09" (old numbering). Update to "expand_reuse" (or remove reference when expand_reuse is deleted in Stage 1D).
- [ ] **[STYLE]** `compiler/ori_arc/src/rc_elim/mod.rs:1,10,12` — Module doc references "Section 08" and "Section 09" (old numbering). Fix when removing rc_elim in Stage 1C.
- [ ] **[STYLE]** `compiler/ori_arc/src/reset_reuse/mod.rs:1` — Module doc references "Section 07.6" (old numbering). Fix when removing reset_reuse in Stage 1D.
- [ ] **[STYLE]** `compiler/ori_arc/src/expand_reuse/mod.rs:1` — Module doc references "Section 09" (old numbering). Fix when removing expand_reuse in Stage 1D.
- [ ] **[STYLE]** `compiler/ori_arc/src/uniqueness/intra/mod.rs:1` — Module doc references "Section 07.2" (old numbering). Fix when removing uniqueness in Stage 1C.
- [ ] **[STYLE]** `compiler/ori_arc/src/uniqueness/inter/mod.rs:1` — Module doc references "Section 07.3" (old numbering). Fix when removing uniqueness in Stage 1C.
- [ ] **[STYLE]** `compiler/ori_arc/src/drop/mod.rs:1` — Module doc references "Section 07.4" (old numbering). Fix during AIMS integration (drop/ is retained, not removed).
- [ ] **[STYLE]** `compiler/ori_arc/src/graph/call_graph/mod.rs:9` — Module doc references "Section 12" (old numbering). Fix during AIMS integration (call_graph/ is retained).

---

## 06.4 Completion Checklist

- [ ] AIMS pipeline produces correct output for all existing tests
- [ ] Feature flag allows switching between old and new pipelines
- [ ] `cargo test --workspace --features aims` passes (Rust unit tests)
- [ ] `cargo build --features aims && ./target/debug/ori test tests/` passes (interpreter spec tests)
- [ ] `cargo build --features aims --release && ./target/release/ori test --backend=llvm tests/`
  passes (LLVM spec tests)
- [ ] `cargo test -p ori_llvm --features aims` passes (AOT tests)
- [ ] `./test-all.sh` passes WITHOUT `aims` feature (old pipeline unchanged)
- [ ] RC operation count tracked: AIMS ≤ old is the goal for Stage 1D, but
  Stage 1C accepts correctness-first with RC regressions investigated
- [ ] No LLVM codegen changes needed (ARC IR interface is stable):
  - `ArcFunction.cow_annotations` — semantically equivalent (same `CowMode` per
    COW operation; derived by combining per-variable analysis facts with
    post-merge IR positions — a packaging step, not a second analysis)
  - `ArcFunction.drop_hints` — semantically equivalent (same drop-hint coverage;
    derived by combining per-variable uniqueness facts with post-merge RcDec
    positions)
  - `ArcFunction.var_reprs` populated identically (same pass, unchanged)
  - `ArcFunction.tail_calls` populated identically (same pass, unchanged)
  - `Apply.arg_ownership` / `Invoke.arg_ownership` populated identically
  - `ArcParam.ownership` on each function populated identically
- [ ] New AIMS outputs (locality hints, FIP certification, shape annotations) are
  internal analysis artifacts only — NOT new mandatory fields on `ArcFunction`
- [ ] `cache` feature compatibility: no new non-skipped fields on `ArcFunction`
- [ ] Old passes removed (or gated behind `#[cfg(not(feature = "aims"))]`)
- [ ] Stage 1A gate passed: shadow analysis matches old pipeline metadata
  **Stage 1A implementation mechanism** (Decision 3):
  The shadow analysis runs AFTER the old pipeline completes (not interleaved).
  In `run_arc_pipeline_all`, after the old interprocedural passes produce
  `AnnotatedSig` and `UniquenessSummary`, run `aims::analyze_program()` to
  produce `MemoryContract` for all functions. Then for each function, after the
  old per-function pipeline completes, run `aims::analyze_function()` to produce
  `AimsStateMap`. Stage 1A comparison is limited to artifacts the old pipeline
  already computes:
  1. Derive `ArcParam.ownership` from AIMS and compare against old
  2. Derive `Apply.arg_ownership` / `Invoke.arg_ownership` from AIMS and compare
  3. Convert `MemoryContract.return_info.uniqueness` and compare against old
     `UniquenessSummary.return_val`
  4. Derive `CowAnnotations` from AIMS and compare against old (semantic
     comparison — same `CowMode` per COW operation site, not positional key
     comparison. One variable may participate in multiple distinct COW sites;
     the comparison must match each site's `CowMode` individually, not just
     check a single per-variable mode.)
  **Not compared:** Cardinality, locality, shape, effect — these are AIMS-only
  dimensions with no old-pipeline equivalent. Validated AIMS-internally via the
  10 hand-traced validation corpus tests from Section 02.7.
  Results are logged via `tracing::info!` (matches, improvements, regressions)
  and accumulated in an internal `ShadowComparisonReport` (returned from
  `run_aims_pipeline_all()` — see Section 06.1 item 4 for access mechanism).
  Gate criterion: zero REGRESSIONS in compared artifacts (AIMS producing weaker
  facts than old pipeline). IMPROVEMENTS (AIMS tighter) are expected and logged.
- [ ] Stage 1B gate passed: AIMS metadata drives LLVM emitter correctly
  **Stage 1B gate criteria**: Full test suite passes with AIMS providing metadata
  (ownership, arg_ownership, cow_annotations) but old pipeline providing RC
  insertion and reuse. Criterion: zero test failures, zero Valgrind errors on
  the `tests/valgrind/` suite.
  **Note:** `./test-all.sh` does not support `--features aims`. Use the manual
  commands from Section 06.1 testing instructions until test-all.sh is updated.
- [ ] Stage 1C gate passed: AIMS RC emission produces correct code
  **Stage 1C gate criteria**: `ori_arc::verify::check_function()` passes on all
  emitted `ArcFunction`s. Behavioral equivalence on full test suite (same output,
  same exit codes). `diagnostics/rc-stats.sh` shows balanced RC for all functions.
  RC operation counts are tracked but not a hard gate — correctness first,
  optimization second. Meaningful regressions (>20% increase in RC ops for a
  single function) should be investigated but are not automatic blockers.
- [ ] Stage 1D gate passed: AIMS reuse emission produces correct code
  **Stage 1D gate criteria**: Full test suite green with full AIMS pipeline
  (no old passes active). `diagnostics/valgrind-aot.sh` reports 0 errors.
  `diagnostics/dual-exec-verify.sh` reports 0 mismatches. Reuse opportunities
  tracked directionally (improvements expected, not a hard gate for initial
  cutover). RC count ≤ old pipeline is NOW a hard gate (Stage 1D is the
  final cutover — optimization parity is required before removing old passes).
  **Note:** `./test-all.sh` does not support `--features aims`. Use the manual
  commands from Section 06.1 testing instructions until test-all.sh is updated.
- [ ] `annotate_arg_ownership()` branches on `#[cfg(feature = "aims")]`

**Exit Criteria:** All test commands listed in 06.4 pass with 0 failures
(`cargo test --workspace --features aims`, spec tests via built binary,
AOT tests via `cargo test -p ori_llvm --features aims`).
`./clippy-all.sh` passes. RC operation count follows staged cutover gates:
tracked and investigated during Stage 1C (correctness first), hard gate
(≤ old pipeline for every program) at Stage 1D completion.
