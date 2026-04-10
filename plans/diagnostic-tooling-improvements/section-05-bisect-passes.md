---
section: "05"
title: "AIMS Pass Bisection"
status: in-progress
reviewed: true
goal: "Create bisect-passes.sh that automatically identifies which AIMS pipeline phase introduced an RC imbalance or structural change"
success_criteria:
  - "Mandatory file split: aims_pipeline.rs extracted into submodules BEFORE any checkpoint code is added (file is 590 lines, above the 500-line limit)"
  - "Per-phase trace checkpoints emit after each logical AIMS pipeline step — including inside helpers (normalize_with_trmc, verify_and_merge, emit_postprocess) at each sub-step boundary"
  - "Checkpoints use existing rc_count::count_rc_ops() (returns RcOpCount { inc, dec }) — no new count_rc_incs/count_rc_decs helpers"
  - "Checkpoints emit RC counts AND structural metrics (block count, var count) to detect phases that change behavior without changing RC totals"
  - "bisect-passes.sh runs a program, captures per-phase snapshots, and reports which phase first introduces an RC divergence or structural change"
  - "Output shows: phase name, RC incs, RC decs, balance, block count, var count, delta-from-previous — no claim of per-phase program execution"
inspired_by:
  - "Swift sil-opt-pass-count — bisects optimization passes to find the one that introduced a bug"
  - "Lean 4 trace.compiler.ir.rc — per-phase RC tracing"
depends_on: []
third_party_review:
  status: resolved
  updated: 2026-04-10
sections:
  - id: "05.PRE"
    title: "Mandatory file split of aims_pipeline.rs"
    status: complete
  - id: "05.1"
    title: "Add per-phase trace checkpoints to AIMS pipeline"
    status: complete
  - id: "05.2"
    title: "Create bisect-passes.sh shell driver"
    status: complete
  - id: "05.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "05.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 05: AIMS Pass Bisection

**Status:** Not Started
**Goal:** When a program crashes, leaks, or produces wrong output through the AOT pipeline, `bisect-passes.sh` answers "which AIMS pipeline phase broke it?" Currently this requires manual trace-log spelunking with `ORI_LOG=ori_arc::aims::realize=trace` -- slow, error-prone, and expertise-dependent. The existing `trace_phase_snapshot` in `emit_unified.rs` only covers post-walk realization subphases (step 5 substeps), not the top-level pipeline steps or the steps inside helper functions like `normalize_with_trmc()`, `verify_and_merge()`, and `emit_postprocess()`.

**Success Criteria:**
- [ ] `aims_pipeline.rs` (590 lines, above 500-line limit) split into submodules BEFORE any checkpoint instrumentation
- [ ] All logical AIMS pipeline phases emit a named checkpoint to tracing with per-phase RC operation counts AND structural metrics (block count, var count)
- [ ] Checkpoints fire inside helper functions at each sub-step boundary -- not just at the top-level `run_aims_pipeline()` call sites
- [ ] Checkpoints use the existing `rc_count::count_rc_ops()` helper (returns `RcOpCount { inc, dec }`) -- no new counting helpers are created
- [ ] `bisect-passes.sh file.ori` compiles once with full tracing, parses the per-phase checkpoint events, and reports which phase first changed RC balance or structural metrics
- [ ] Output is a human-readable table: Phase | RcInc | RcDec | Balance | Blocks | Vars | Delta
- [ ] The script does NOT claim to execute intermediate phase results -- it analyzes the tracing output from a single full compilation

**Scope clarification:** This is a **trace-analysis bisector**, not a stop-after-phase executor. The AIMS pipeline runs all phases in a single compilation. The shell driver analyzes the per-phase snapshots to find where the balance first diverges or structural metrics change unexpectedly. It cannot execute the program at intermediate pipeline states -- that would require a stop-after-phase compiler surface which is out of scope. The trace-analysis approach is still enormously valuable: it answers "which phase changed the RC balance" and "which phase changed the block/var structure" without manual log spelunking.

**Why structural metrics matter:** Phases like `merge_blocks()`, tail-call rewrite, and TRMC normalization change program behavior without changing RC totals. A script that only tracks RC counts has blind spots for these phases. Tracking block count and var count alongside RC counts ensures the bisector detects ALL structural changes, not just RC changes.

**Context:** Both Codex and Gemini flagged the absence of automated AIMS bisection. Codex's analysis: existing `trace_phase_snapshot` in `emit_unified.rs` only covers the post-walk realization subpipeline (after step 5), not the top-level steps or the steps inside helpers. Gemini referenced Swift's `sil-opt-pass-count` as the model. The implementation requires a small Rust change (adding checkpoints) plus a shell driver.

**Reference implementations:**
- **Swift** `sil-opt-pass-count` -- bisects SIL optimization passes (docs/DebuggingTheCompiler.md)
- **Lean 4** `trace.compiler.ir.rc` -- per-phase RC tracing with configurable granularity

**Depends on:** None.

---

## 05.PRE Mandatory file split of aims_pipeline.rs

**File(s):** `compiler/ori_arc/src/pipeline/aims_pipeline.rs` (590 lines), `compiler/ori_arc/src/pipeline/mod.rs`

Per CLAUDE.md coding guidelines: "500 line limit (excl. tests). Stop and split before exceeding." The file is already at 590 lines -- above the limit. Adding checkpoint instrumentation would push it further. The split is mandatory BEFORE any new code is added, not conditional on exceeding 600 lines.

**Split strategy:** The file contains four distinct responsibilities:
1. `AimsPipelineConfig` + `AimsPipelineResult` structs + `run_aims_pipeline()` (pipeline orchestration) -- stays in `aims_pipeline.rs`
2. `normalize_with_trmc()` + `verify_trmc_soundness()` + `detect_immortals()` (TRMC normalization loop) -- extract to `aims_pipeline/trmc.rs`
3. `verify_and_merge()` + `emit_postprocess()` + `check_fbip()` (post-emission processing) -- extract to `aims_pipeline/postprocess.rs`
4. `run_aims_pipeline_all()` + `run_second_pass()` + `apply_aims_ownership()` + `param_contract_to_ownership()` (batch orchestration + second pass) -- extract to `aims_pipeline/batch.rs`

- [x] Convert `aims_pipeline.rs` to `aims_pipeline/mod.rs` with submodules
- [x] Extract `normalize_with_trmc()`, `verify_trmc_soundness()`, `detect_immortals()` into `aims_pipeline/trmc.rs` (146 lines)
- [x] Extract `verify_and_merge()`, `emit_postprocess()`, `check_fbip()` into `aims_pipeline/postprocess.rs` (75 lines)
- [x] Extract `run_aims_pipeline_all()`, `run_second_pass()`, `apply_aims_ownership()`, `param_contract_to_ownership()` into `aims_pipeline/batch.rs` (229 lines)
- [x] Keep `AimsPipelineConfig`, `AimsPipelineResult`, `run_aims_pipeline()` in `aims_pipeline/mod.rs` (176 lines)
- [x] Update `mod.rs` to reference `aims_pipeline` as a directory module (unchanged — `mod aims_pipeline;` resolves to directory automatically)
- [x] Verify: `timeout 150 cargo t -p ori_arc` passes -- 1094 passed, 0 failed
- [x] Verify: `cargo clippy -p ori_arc` passes (clippy clean, 0 warnings)
- [x] Verify: no file in the split exceeds 300 lines (max: batch.rs at 229)

- [x] **Subsection close-out (05.PRE)** -- MANDATORY before starting 05.1:
  - [x] All tasks above are `[x]` and verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`

---

## 05.1 Add per-phase trace checkpoints to AIMS pipeline

**File(s):** `compiler/ori_arc/src/pipeline/aims_pipeline/mod.rs` (post-split), `compiler/ori_arc/src/pipeline/aims_pipeline/trmc.rs`, `compiler/ori_arc/src/pipeline/aims_pipeline/postprocess.rs`, `compiler/ori_arc/src/pipeline/aims_pipeline/batch.rs`

The AIMS pipeline orchestration runs steps 3-12. The top-level `run_aims_pipeline()` calls into helper functions that bundle multiple steps: `normalize_with_trmc()` bundles steps 3/3.5/3a, `verify_and_merge()` bundles steps 6/7/8/8.5/9, and `emit_postprocess()` bundles steps 11/12. Checkpoints must go INSIDE these helpers at each logical pass boundary, not just at the top-level call sites.

### 05.1.1 Create checkpoint helper

- [x] Create a `trace_pipeline_checkpoint()` helper function in `aims_pipeline/mod.rs`. Uses `info` on `ori_arc::aims::pipeline` target, `tracing::enabled!` early-return guard, `rc_count::count_rc_ops()` for RC, `blocks.len()`/`var_types.len()` for structural metrics, `interner.lookup()` for function name.

### 05.1.2 Add checkpoints inside helpers

Checkpoints must be placed INSIDE the helper functions at each logical sub-step boundary. The following is the complete list of checkpoint locations:

All checkpoint calls pass `config.interner` (or the interner from the enclosing scope) so the function name is emitted on every event.

**Inside `normalize_with_trmc()` (trmc.rs):**
- [x] After `compute_var_reprs()` (step 3): `trace_pipeline_checkpoint(func, "compute_var_reprs", interner)`
- [x] After `detect_immortals()` (step 3.5): `trace_pipeline_checkpoint(func, "detect_immortals", interner)`
- [x] After `normalize_function()` (step 3a): `trace_pipeline_checkpoint(func, "normalize_function", interner)`

**Inside `run_aims_pipeline()` (mod.rs) -- between top-level calls:**
- [x] After `normalize_with_trmc()` returns: `trace_pipeline_checkpoint(func, "normalize_with_trmc_complete", config.interner)`
- [x] After `analyze_function()` (step 4): `trace_pipeline_checkpoint(func, "analyze_function", config.interner)`
- [x] After `verify_trmc_soundness()` (step 4a): `trace_pipeline_checkpoint(func, "verify_trmc_soundness", config.interner)`
- [x] After `realize_rc_reuse()` (step 5): `trace_pipeline_checkpoint(func, "realize_rc_reuse", config.interner)`
- [x] After FIP enforcement pre-check (step 5a): `trace_pipeline_checkpoint(func, "verify_fip_contract", config.interner)`

**Inside `verify_and_merge()` (postprocess.rs) -- interner accessed via `config.interner` (AimsPipelineConfig):**
- [x] After `run_verify()` (step 6): `trace_pipeline_checkpoint(func, "verify_post_emission", interner)`
- [x] After `run_aims_verify()` (step 7): `trace_pipeline_checkpoint(func, "aims_verify", interner)`
- [x] After `detect_tail_calls()` + `rewrite_tail_calls()` (step 8): `trace_pipeline_checkpoint(func, "tail_calls", interner)`
- [x] After `add_invoke_unwind_cleanup()` (step 8.5): `trace_pipeline_checkpoint(func, "unwind_cleanup", interner)`
- [x] After `merge_blocks()` (step 9): `trace_pipeline_checkpoint(func, "merge_blocks", interner)`

**Back in `run_aims_pipeline()` after `verify_and_merge()` returns:**
- [x] After `realize_annotations()` (step 10): `trace_pipeline_checkpoint(func, "realize_annotations", config.interner)`

**Inside `emit_postprocess()` (postprocess.rs) -- interner accessed via `config.interner` (AimsPipelineConfig):**
- [x] After `verify()` (step 11): `trace_pipeline_checkpoint(func, "verify_final", interner)`
- [x] After `check_fbip()` (step 12): `trace_pipeline_checkpoint(func, "fbip_enforcement", interner)`

### 05.1.3 Verify and test

- [x] Use `info` level on `ori_arc::aims::pipeline` target with `tracing::enabled!` early-return guard. Intentionally different from `trace_phase_snapshot` (trace! on `ori_arc::aims::realize`).
- [x] Checkpoint function verified callable via end-to-end `cargo run` testing (no `tracing-test` dep available; 1094 existing tests exercise the full pipeline).
- [x] Verify: `ORI_LOG=ori_arc::aims::pipeline=info cargo run -- build diagnostics/fixtures/clean.ori` shows 16 checkpoint lines for `main` function — covers all phases.
- [x] Verify: checkpoint events include `function`, `rc_incs`, `rc_decs`, `blocks`, and `vars` fields (confirmed).
- [x] Verify: multi-function fixture (`chain.ori`) produces distinct `function=` values: `"__lambda_main_0"` and `"main"`.
- [x] `timeout 150 cargo t -p ori_arc` — 1094 passed, 0 failed, no regressions.

- [x] **Subsection close-out (05.1)** -- MANDATORY before starting 05.2:
  - [x] All tasks above are `[x]` and verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 05.2 Create bisect-passes.sh shell driver

**File(s):** `diagnostics/bisect-passes.sh` (new), `diagnostics/self-test.sh`, `diagnostics/README.md`

Create a shell script that runs a program with `ORI_LOG=ori_arc::aims::pipeline=info`, parses the checkpoint events, and reports which phase first introduces a problem.

### 05.2.1 Core script

- [x] Created `diagnostics/bisect-passes.sh` with all specified behavior: --help/--no-color/--color/--function/--rc-only, sources _common.sh, mktemp+trap cleanup, ORI_LOG pipeline tracing, per-function tables with Balance/Delta, divergence highlighting, ORI_CHECK_LEAKS runtime check, exit codes 0/1/2.
- [x] Script reads structured tracing output from canonical checkpoint events — no LLVM IR regex matching.

### 05.2.2 Self-test entries

- [x] Added self-test entries to `diagnostics/self-test.sh`: --help shows "Usage:", fixtures/simple.ori runs with "Phase" in output, fixtures/clean.ori shows "realize_rc_reuse", --function main filters to "Function: main", no-args correctly fails. All 51 self-tests pass.
- [x] No fixture existence check section applicable (bisect-passes uses existing fixtures via _common.sh discovery).

### 05.2.3 Documentation

- [x] Added `bisect-passes.sh` to `diagnostics/README.md`: overview table entry + usage section with purpose, example invocations, and workflow integration note.
- [x] `diagnostics/self-test.sh` passes: 51 passed, 0 failed.

### 05.2.4 Cross-section plan update: Section 06 fixture coverage

Per `impl-hygiene.md`: "Cross-section fixes require cross-section plan updates." Section 06 was updated during plan review (2026-04-10) to include `bisect-passes.sh`:

- [x] Edit `plans/diagnostic-tooling-improvements/section-06-fixtures.md` — added `bisect-passes.sh` to Section 06.2's diagnostic script coverage matrix (2026-04-10 plan review)
- [x] Added explicit self-test items in Section 06.2 for `bisect-passes.sh` against fixtures (2026-04-10 plan review)
- [x] Added `bisect-passes.sh` to Section 06's success criteria (2026-04-10 plan review)

- [x] **Subsection close-out (05.2)** -- MANDATORY before starting 05.R:
  - [x] All tasks above are `[x]` and verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 05.R Third Party Review Findings

- [x] `[TPR-05-001-codex][high]` `section-05-bisect-passes.md:110` — Add function identity to AIMS checkpoint events.
  Resolved: Fixed on 2026-04-10. Added `interner: &ori_ir::StringInterner` parameter and `function = interner.lookup(func.name)` field to all checkpoint calls.
- [x] `[TPR-05-001-gemini][high]` `section-05-bisect-passes.md:67` — Include function name in trace checkpoints for multi-function bisection.
  Resolved: Fixed on 2026-04-10. Same fix as [TPR-05-001-codex] (functionally identical finding).
- [x] `[TPR-05-002-codex][medium]` `section-05-bisect-passes.md:222` — Anchor bisect-passes fixture coverage in Section 06.
  Resolved: Fixed on 2026-04-10. Changed 05.2.4 from passive note to mandatory cross-section plan update requiring direct edit of section-06-fixtures.md.
- [x] `[TPR-05-002-gemini][medium]` `section-05-bisect-passes.md:152` — Mandate explicit cross-section plan update.
  Resolved: Fixed on 2026-04-10. Same fix as [TPR-05-002-codex] (functionally identical finding).
- [x] `[TPR-05-003-codex][medium]` `section-05-bisect-passes.md:191` — Constrain bisect-passes builds to a temporary output path.
  Resolved: Fixed on 2026-04-10. Added tmpdir/mktemp/trap cleanup requirements to shell script behavior spec.
- [x] `[TPR-05-004-codex][low]` `section-05-bisect-passes.md:129` — Correct the checkpoint tracing target contract.
  Resolved: Fixed on 2026-04-10. Corrected precedent claim (trace_phase_snapshot uses trace!, not info!), documented new target as intentionally different, added arc.md and CLAUDE.md doc update items to completion checklist.

**Iteration 2 findings:**
- [x] `[TPR-05-005-codex][medium]` `section-06-fixtures.md:42` — Add bisect-passes to Section 06 fixture coverage.
  Resolved: Fixed on 2026-04-10. Directly edited section-06-fixtures.md: added bisect-passes.sh to success criteria and self-test matrix.
- [x] `[TPR-05-006-codex][low]` `section-07-integration.md:136` — Add pipeline tracing target docs to Section 07.
  Resolved: Fixed on 2026-04-10. Added ori_arc::aims::pipeline tracing target doc items to Section 07.4 remaining tasks.

---

## 05.N Completion Checklist

- [ ] All subsections (05.PRE, 05.1, 05.2) complete
- [ ] `aims_pipeline.rs` has been split and no submodule exceeds 300 lines
- [ ] `timeout 150 cargo t -p ori_arc` passes
- [ ] `timeout 150 cargo cl -p ori_arc` passes (clippy clean)
- [ ] `diagnostics/self-test.sh` passes (including new bisect-passes.sh entries)
- [ ] `timeout 150 ./test-all.sh` green -- no regressions
- [ ] **Doc update (SSOT):**
  - [ ] Add `bisect-passes.sh` to `@diagnostic.md` (`.claude/rules/diagnostic.md`) Diagnostic Scripts table with flags (`--function`, `--rc-only`, `--no-color`, `--color`)
  - [ ] Add `bisect-passes.sh` to `diagnostics/README.md` usage section and workflow
  - [ ] Update CLAUDE.md Diagnostic scripts reference if it lists scripts explicitly
  - [ ] Add `ori_arc::aims::pipeline` tracing target to `.claude/rules/arc.md` §Debugging section (alongside existing `ori_arc::aims::realize` target documentation)
  - [ ] Update CLAUDE.md §Tracing with `=ori_arc::aims::pipeline=info` example for pipeline bisection
- [x] **Cross-section:** Section 06 updated — `bisect-passes.sh` added to success criteria and self-test matrix (done during plan review 2026-04-10)
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] **`/improve-tooling` section-close sweep**
