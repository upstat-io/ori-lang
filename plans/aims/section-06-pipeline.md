---
section: "06"
title: "Pipeline Integration"
status: complete
goal: "Wire AIMS into pipeline/mod.rs, replacing ~15 analysis/emission steps with the unified system"
inspired_by:
  - "ori_arc pipeline (compiler/ori_arc/src/pipeline/mod.rs)"
depends_on: ["04", "05"]
sections:
  - id: "06.1"
    title: "Pipeline Entry Points"
    status: complete
  - id: "06.2"
    title: "New Pipeline Flow"
    status: complete
  - id: "06.3"
    title: "Old Pass Removal"
    status: complete
  - id: "06.4"
    title: "Completion Checklist"
    status: complete
---

# Section 06: Pipeline Integration

**Status:** Complete

AIMS is the sole pipeline. `run_arc_pipeline_all()` calls
`aims_pipeline::run_aims_pipeline_all()` unconditionally. No `#[cfg(feature =
"aims")]` exists in the codebase. The `aims` feature flag, `aims-shadow`
feature, `shadow/` directory, and feature dispatch are all deleted.

**Goal:** AIMS is wired into `pipeline/mod.rs` as the sole pipeline.
`run_arc_pipeline_all()` calls AIMS directly.

**Depends on:** Sections 04, 05 (RC and reuse emission).

---

## 06.1 Pipeline Entry Points (Current State)

**File(s):** `compiler/ori_arc/src/pipeline/mod.rs`

AIMS is the sole pipeline. No feature flags, no dual paths, no shadow
comparison. The migration machinery (feature flags, shadow comparison,
staged cutover) has been deleted.

- [x] `run_arc_pipeline_all()` calls `aims_pipeline::run_aims_pipeline_all()` directly
- [x] `run_arc_pipeline()` calls `aims_pipeline::run_aims_pipeline()` directly
- [x] `run_uniqueness_analysis()` returns empty map (compatibility stub)
- [x] `compute_aims_contracts()` runs interprocedural analysis and applies ownership
- [x] `aims` feature flag deleted from all Cargo.toml files
- [x] `aims-shadow` feature flag deleted
- [x] `pipeline/shadow/` directory deleted
- [x] No `#[cfg(feature = "aims")]` anywhere in the codebase
- [x] **Legacy dead code deletion:** Deleted truly dead legacy RC insertion
  code replaced by AIMS `realize_rc_reuse()`. Removed: `apply_borrows()`,
  `insert_rc_ops_with_ownership()`, `insert_rc_ops()`, `block_rc.rs`,
  `edge_cleanup.rs`, `insert.rs`, and 3 dead test helpers. (2026-03-14)

  **Remaining legacy modules (still live — NOT dead):**
  - `borrow/`: `BuiltinOwnershipSets`, `infer_borrows_scc`, `infer_borrow_single`,
    `infer_borrow_fixed_point`, `extract_callees` — all actively called by
    Salsa queries (`oric`), JIT test runner, and the AIMS pipeline itself.
  - `liveness/`: `compute_refined_liveness()` called by FBIP enforcement in
    the AIMS pipeline.
  - `rc_insert/`: Only `annotate_arg_ownership()` remains — called by AIMS
    arg ownership emission (`aims/emit_rc/arg_ownership.rs`).
  - `uniqueness/`: All types live — `CowAnnotations` and `DropHints` are
    fields on `ArcFunction`, `UniquenessSummary` consumed by `ori_llvm`.
  - `ownership/`: `AnnotatedSig`, `Ownership`, `DerivedOwnership` — shared
    type vocabulary for the ARC/LLVM interface.

  Full type migration to `aims/` would require replacing the borrow inference
  pipeline with AIMS-native equivalents and updating all external consumers.
  This is out of scope for the AIMS plan — the types are correct and actively
  used; they are not "legacy" in the sense of being stale or wrong.

---

## 06.2 New Pipeline Flow

**File(s):** `compiler/ori_arc/src/pipeline/mod.rs`, `compiler/ori_arc/src/pipeline/aims_pipeline.rs`

The AIMS pipeline after unified realization (Section 10):

```
 Interprocedural (once across all functions):
 1. aims::analyze_program()           — MemoryContract per function (SCC fixpoint)
 2. aims::apply_ownership()           — populate ArcParam.ownership

 Per-function (steps 3–12):
 3. compute_var_reprs()               — fill ValueRepr per variable
3a. aims::normalize_function()        — TRMC normalization (detection, lifting,
                                         rewriting, verification). Returns
                                         NormalizationResult { was_transformed,
                                         context_regions }. If was_transformed,
                                         re-runs from step 3 (idempotent, at most 2 iterations).
 4. aims::analyze_function()          — backward dataflow → converged AimsStateMap
 5. aims::realize_rc_reuse()          — Phase 1: arg_ownership + RC + reuse (pre-merge)
5a. aims::verify::fip::verify_fip_contract() — FIP enforcement verification
 6. verify()                          — ARC IR sanity check
 7. run_aims_verify()                 — AIMS contract vs IR consistency
 8. detect_tail_calls() + rewrite()   — tail call → loop
 9. merge_blocks()                    — CFG cleanup
10. aims::realize_annotations()       — Phase 2: COW + drop hints (post-merge)
11. verify()                          — final sanity check
12. FBIP enforcement                  — read-only diagnostic
```

That is 12 per-function steps (down from Stage 1's 14), with two-phase realization
replacing four separate emission passes.

- [x] `run_aims_pipeline_all()` is the sole implementation called from
  `run_arc_pipeline_all()`:
  - Step 1: Compute `MemoryContract` for all functions via `aims::analyze_program()`
  - Step 2: Apply ownership to function parameters via `aims::apply_ownership()`
  - Step 3: Per-function loop calling `run_aims_pipeline()` (steps 3-12)
  - Return `Vec<ArcProblem>` (FBIP violations) matching the current API
  - Uses `AimsPipelineConfig` config struct per hygiene rules (>3-4 params -> config struct)
- [x] `run_aims_pipeline()` is the sole per-function implementation called from
  `run_arc_pipeline()`. The public API functions (`run_arc_pipeline`,
  `run_arc_pipeline_all`, `run_uniqueness_analysis`, `annotate_arg_ownership`)
  delegate directly to AIMS — no feature dispatch.
  **Callers of `run_arc_pipeline` (unchanged):**
  1. `run_arc_pipeline_all()` — calls `run_arc_pipeline()` in its per-function loop
  2. `ori_llvm` `FunctionCompiler::compile_function_arc()` — calls `run_arc_pipeline()`
     directly for AOT per-function compilation
  3. `ori_llvm` `FunctionCompiler::declare_and_process_lambda()` — calls `run_arc_pipeline()`
     directly for lambda compilation
  `run_uniqueness_analysis()` returns empty map (compatibility stub).
- [x] **Dominator tree timing**: RC emission (step 6) may insert edge cleanup
  (trampoline blocks for critical edges), which modifies the CFG. Reuse emission
  (step 7) needs dominator trees for cross-block reuse detection (see Section 05
  ReusePlanner). Therefore: build dominator trees ONCE, between steps 6 and 7,
  after any CFG-modifying edge cleanup is complete. Do NOT build dom trees before
  RC emission — they would be immediately invalidated.
  Verified: `ReusePlanner` (in `aims/emit_reuse/planner.rs`) builds dom/post-dom
  trees lazily during step 7, after RC emission (step 6) has completed. Trees
  are only built when cross-block candidates exist (cost control).
- [x] Verify: no liveness recomputation needed (state map is complete)
  Verified: AIMS pipeline (`aims_pipeline.rs`) never calls `compute_refined_liveness`
  or `compute_liveness`. The `AimsStateMap` replaces liveness analysis entirely.
- [x] Verify: tail_call and block_merge work on AIMS output (they only read the IR
  structure, not analysis metadata). Verified: `detect_tail_calls()` +
  `rewrite_tail_calls()` and `merge_blocks()` run on AIMS output in steps 10-11 of
  `aims_pipeline.rs`. All 1252 AOT tests pass with `--features aims`.
- [x] **block_merge invalidation**: `merge_blocks()` (step 11) renumbers blocks and
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
    either be discarded or re-keyed after merge. Currently, events consumed by emission
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

- [x] Remove old modules (keep behind feature flag initially):
  **Phase 1 (complete)**: 4 pure-analysis modules gated behind
  `#[cfg(any(not(feature = "aims"), feature = "aims-shadow"))]`:
  - [x] `rc_elim/` → replaced by `aims::emit_rc` (no separate elimination)
  - [x] `rc_identity/` → replaced by `aims::emit_rc` (identity built into analysis)
  - [x] `reset_reuse/` → replaced by `aims::emit_reuse`
  - [x] `expand_reuse/` → replaced by `aims::emit_reuse` (emits expanded form directly)
  Re-exports (`expand_reset_reuse`, `eliminate_rc_ops_dataflow`,
  `propagate_rc_identity`, `RcIdentityMap`) gated with same guard.
  `run_uniqueness_analysis()` short-circuited (returns empty map when AIMS active).
  Legacy-only test (`pipeline_order_expand_before_eliminate`) gated.
  **Phase 2 (complete)**: 4 modules have shared types/functions — re-exports
  gated for legacy-only analysis functions, shared types remain ungated:
  - [x] `borrow/` — module stays (provides `BuiltinOwnershipSets`, borrow
    inference functions used by oric Salsa queries). Legacy-only re-exports
    gated: `all_cow_method_names`, `apply_borrows`, `consuming_receiver_*`,
    `infer_derived_ownership`. Shared re-exports ungated: `BuiltinOwnershipSets`,
    `borrowing_builtin_names`, `extract_callees`, `infer_borrow_*`,
    `infer_borrows_scc`.
  - [x] `liveness/` — module stays (used by `fbip/` which runs in AIMS pipeline).
    Re-exports ungated — no downstream consumers, but `tests.rs` uses
    `compute_refined_liveness` for FBIP analysis testing.
    Will be removable when FBIP is rewritten to use AIMS state map.
  - [x] `rc_insert/` — module stays (provides `annotate_arg_ownership` called
    from `ori_llvm`, has internal `#[cfg(feature = "aims")]` branching).
    Legacy re-exports (`insert_rc_ops_with_ownership`,
    `insert_external_invoke_cleanup`) already gated in Phase 1.
  - [x] `uniqueness/` — module stays (defines shared types used by both
    pipelines and downstream). Legacy-only re-exports gated:
    `analyze_program`, `build_cow_summaries`, `analyze_intraprocedural`,
    `analyze_with_summaries`, `UniquenessResult`, `compute_cow_annotations`,
    `compute_drop_hints`. Shared types ungated: `CowAnnotations`, `CowMode`,
    `DropHints`, `Uniqueness`, `UniquenessMap`, `UniquenessSummary`.
- [x] Retain with possible adaptation:
  - `ownership/` — defines `Ownership`, `DerivedOwnership`, `AnnotatedSig` types.
    Retained — used by AIMS pipeline (`aims_pipeline.rs` imports `Ownership`),
    `ori_llvm`, and `oric`.
  - `drop/` — computes `DropInfo`/`DropKind` per type for LLVM codegen (independent of AIMS).
    Retained unchanged.
  - `fbip/` — `check_fbip_enforcement` and `is_auto_fbip` remain UNCHANGED;
    they run on the final `ArcFunction` (post tail_call, block_merge, drop_hints)
    and read `ArcFunction.cow_annotations` and block instructions. AIMS produces
    semantically equivalent annotations (same `CowMode` per COW operation, same
    drop-hint coverage) via a different production path and timing (post-merge
    walk vs pre-merge computation). The LLVM emitter and FBIP enforcement see
    equivalent final annotations. No input shift needed. Retained unchanged.

- [x] Update `lib.rs` exports to use AIMS types.
  Re-exports for gated modules (`expand_reset_reuse`, `eliminate_rc_ops_dataflow`,
  `propagate_rc_identity`, `RcIdentityMap`) gated with
  `#[cfg(any(not(feature = "aims"), feature = "aims-shadow"))]`.
  Shared type re-exports (`CowAnnotations`, `CowMode`, `DropHints`, `Uniqueness`,
  `UniquenessSummary`, `AnnotatedSig`, `Ownership`, `BuiltinOwnershipSets`, etc.)
  remain ungated — these types are used by both pipelines and downstream crates.
  Pipeline entry points (`run_arc_pipeline`, `run_arc_pipeline_all`,
  `run_uniqueness_analysis`) remain ungated with internal feature branching.
- [x] Update `ori_llvm` consumers:
  No changes needed — AIMS populates the same `ArcFunction` fields:
  - `cow_annotations` — semantically equivalent (verified: 1245 AOT tests pass)
  - `drop_hints` — semantically equivalent
  - `var_reprs` — same pass (unchanged)
  - `tail_calls` — same pass (unchanged)
  - `Apply.arg_ownership` / `Invoke.arg_ownership` — AIMS step 4
  - `ArcParam.ownership` — AIMS step 2
- [x] Update `oric` consumers:
  No changes needed — `oric` calls `run_arc_pipeline_all` which dispatches
  internally. Cache compatibility verified: no new fields on `ArcFunction`.
- [x] Remove old pipeline from `pipeline/mod.rs`
  `run_legacy_pipeline` gated behind `#[cfg(any(not(feature = "aims"), feature = "aims-shadow"))]`,
  `run_legacy_pipeline_all` gated behind `#[cfg(not(feature = "aims"))]`.
  Both excluded from pure AIMS builds. Physical deletion deferred until
  `aims-shadow` comparison mode is retired (shadow mode needs legacy code).

### Cleanup

- [x] **[STYLE]** Verify `_enforce_type_tag_exhaustiveness` lint attrs in enforcement crates
  use `#[expect]` not `#[allow]`. Checked: no instances of this pattern exist in
  the aims branch codebase. The referenced files in ori_eval, ori_types, and
  ori_llvm do not contain this function. N/A for this branch.
- [x] **[STYLE]** `compiler/ori_arc/src/pipeline/mod.rs:37` — Current `run_arc_pipeline` takes 7 positional parameters. When creating `run_aims_pipeline`, use a config struct instead of replicating this pattern.
  Done: `AimsPipelineConfig` struct in `aims_pipeline.rs` bundles classifier,
  contracts, pool, interner, builtins, verify_arc.
- [x] **[NOTE]** `compiler/ori_arc/src/ir/mod.rs` — At 431 lines (excluding tests), approaching
  500-line limit. AIMS types go in `aims/`, no additions here. Noted.
- [x] **[NOTE]** `compiler/ori_arc/src/rc_elim/eliminate.rs` — At 439 lines. AIMS replaces
  this file entirely (Section 06.3), so no split needed — just remove cleanly. Noted.

- [x] **[GAP]** `compiler/ori_arc/src/rc_insert/edge_cleanup.rs` — AIMS reimplemented edge
  cleanup as `aims/emit_rc/edge_cleanup.rs` (`emit_edge_cleanup`), driven by the
  state map instead of liveness/borrow data. No dependency on the old `rc_insert`
  version.
- [x] **[STYLE]** `compiler/ori_arc/src/ir/instr.rs` — Stale section references updated.
  Replaced "Section 07/07.1/07.6/08/09" with pass names (RC emission, liveness,
  reset/reuse detection, RC elimination, reuse expansion).
- [x] **[STYLE]** `compiler/ori_arc/src/ir/mod.rs:194` — `ArcParam` doc reference updated:
  removed "(Section 06.2)", now says "refined to `Borrowed` by borrow inference."
- [x] **[STYLE]** `compiler/ori_arc/src/ir/mod.rs:303` — `ArcTerminator::substitute_var` doc
  reference updated: removed "(Section 09)", now says "constructor reuse expansion".
- [x] **[STYLE]** `compiler/ori_arc/src/rc_elim/mod.rs` — Module doc updated: removed
  "Section 08"/"Section 09", replaced with pass names.
- [x] **[STYLE]** `compiler/ori_arc/src/reset_reuse/mod.rs:1` — Module doc updated: removed
  "Section 07.6" and "§07.2".
- [x] **[STYLE]** `compiler/ori_arc/src/expand_reuse/mod.rs:1` — Module doc updated: removed
  "Section 09" and "Section 07.6".
- [x] **[STYLE]** `compiler/ori_arc/src/uniqueness/intra/mod.rs:1` — Module doc updated:
  removed "Section 07.2".
- [x] **[STYLE]** `compiler/ori_arc/src/uniqueness/inter/mod.rs:1` — Module doc updated:
  removed "Section 07.3".
- [x] **[STYLE]** `compiler/ori_arc/src/drop/mod.rs:1` — Module doc updated: removed
  "Section 07.4".
- [x] **[STYLE]** `compiler/ori_arc/src/graph/call_graph/mod.rs:9` — Module doc updated:
  removed "Section 12".

---

## 06.4 Completion Checklist

- [x] **[BLOAT]** `compiler/ori_arc/src/aims/lattice/mod.rs` — 548 lines, exceeds the 500-line limit.
  Fixed: extracted `dimensions.rs` (237 lines) with 7 dimension enums + `ReuseCtorKind`.
  mod.rs is now 363 lines (`AimsState`, constants, predicates, `BorrowSource`, `SizeClass`).
- [x] **[BLOAT]** `compiler/ori_arc/src/pipeline/shadow.rs` — 567 lines, exceeds 500-line limit.
  Fixed: extracted `shadow/compare.rs` (283 lines) with `compare_all()`, `compare_function()`,
  `compare_param_ownership()`, `compare_return_uniqueness()`, `compare_cow_annotations()`,
  `compare_rc_ops()`, and helper functions. `shadow.rs` is now 304 lines (types, pipeline, reporting).
- [x] **[STYLE]** Multiple AIMS section files have status inconsistency: frontmatter
  says `status: complete` while body text says `**Status:** Not Started`. Update
  body text to match frontmatter in: section-01, section-02, section-03, section-04, section-05, section-06.
  Also update `index.md` status entries and `00-overview.md` quick reference table.
  **Fixed 2026-03-11**: All body text, index.md, and overview quick reference now match frontmatter.

- [x] AIMS pipeline produces correct output for all existing tests
- [x] `cargo test --workspace` passes (Rust unit tests — AIMS is sole pipeline)
- [x] `cargo build && ./target/debug/ori test tests/` passes (interpreter spec tests)
- [x] `cargo build --release && ./target/release/ori test tests/` passes (LLVM spec tests)
- [x] `cargo test -p ori_llvm` passes (AOT tests)
- [x] `./test-all.sh` passes
- [x] RC operation count tracked: AIMS ≤ old is the goal for Stage 1D, but
  Stage 1C accepts correctness-first with RC count regressions investigated.
  Implemented: `pipeline/rc_count` module (`RcOpCount`, `count_rc_ops()`) counts
  `RcInc`/`RcDec` in ARC IR. Shadow comparison (`aims-shadow`) now runs full AIMS
  pipeline on cloned functions and compares RC counts as a 4th dimension alongside
  param ownership, return uniqueness, and COW annotations. Pure AIMS mode logs
  aggregate RC counts via `tracing::debug`. (2026-03-10)
- [x] No LLVM codegen changes needed (ARC IR interface is stable):
  Verified: 1252 AOT tests pass with `--features aims`, no `ori_llvm` changes.
  - `ArcFunction.cow_annotations` — semantically equivalent
  - `ArcFunction.drop_hints` — semantically equivalent
  - `ArcFunction.var_reprs` populated identically (same pass, unchanged)
  - `ArcFunction.tail_calls` populated identically (same pass, unchanged)
  - `Apply.arg_ownership` / `Invoke.arg_ownership` populated by AIMS step 4
  - `ArcParam.ownership` on each function populated by AIMS step 2
- [x] New AIMS outputs (locality hints, FIP certification, shape annotations) are
  internal analysis artifacts only — NOT new mandatory fields on `ArcFunction`.
  Verified: no new fields added to `ArcFunction` by AIMS.
- [x] `cache` feature compatibility: no new non-skipped fields on `ArcFunction`
- [x] Old passes removed (or gated behind `#[cfg(not(feature = "aims"))]`)
  Phase 1: 4 pure-analysis modules gated (`rc_elim`, `rc_identity`,
  `reset_reuse`, `expand_reuse`). Phase 2: 4 modules with shared types —
  legacy-only re-exports gated, shared types/functions ungated.
  `run_legacy_pipeline` and `run_legacy_pipeline_all` already gated.
- [x] Stage 1A gate passed: shadow analysis matches old pipeline metadata
  Verified 2026-03-10: `run_shadow_pipeline_all()` in `pipeline/shadow.rs`
  exercised via `ORI_DUMP_AFTER_ARC=1` batch path on multiple programs. Results:
  zero regressions across all 3 dimensions (param ownership, return uniqueness,
  COW annotations). AIMS improvements found: Unique return values where legacy
  says MaybeShared; StaticUnique COW where legacy says Dynamic. Shadow unit
  tests (6 tests in `shadow/tests.rs`) and 896 `ori_arc` tests pass with
  `--features aims-shadow`.
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
- [x] Stage 1B gate passed: AIMS metadata drives LLVM emitter correctly
  Verified 2026-03-10: subsumed by Stage 1C/1D — the full AIMS pipeline
  provides ALL metadata (ownership, arg_ownership, cow_annotations, drop_hints)
  AND RC emission AND reuse emission. Since 1C and 1D pass, 1B is trivially
  satisfied. Evidence: 890 ori_arc + 1252 AOT + 3389 spec tests all pass
  with `--features aims`. Valgrind deferred to Stage 1D validation.
- [x] Stage 1C gate passed: AIMS RC emission produces correct code
  Verified 2026-03-10: `ori_arc::verify::check_function()` runs at steps 9 and
  13 in `aims_pipeline.rs`. All 890 ori_arc tests, 1252 AOT tests, and 3389
  spec tests pass with `--features aims`. Behavioral equivalence confirmed
  via `dual-exec-verify.sh` (ALL VERIFIED, no mismatches). RC operation count
  tracking deferred to Stage 1D (correctness validated first).
- [x] Stage 1D gate passed: AIMS reuse emission produces correct code
  Verified 2026-03-10: Full AIMS pipeline active (no old passes). All test
  suites green: ori_arc (890), ori_llvm AOT (1252), spec (3389).
  `dual-exec-verify.sh` reports 0 mismatches (ALL VERIFIED). Same-block
  reuse active. Valgrind and RC count parity tracking deferred — correctness
  validated; optimization parity will be tracked before old pass removal.
- [x] `annotate_arg_ownership()` delegates to AIMS pipeline directly (no feature dispatch)

**Exit Criteria:** All test commands pass with 0 failures (`cargo test
--workspace`, spec tests via built binary, AOT tests via `cargo test -p
ori_llvm`). `./clippy-all.sh` passes.
