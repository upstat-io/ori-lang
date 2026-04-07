---
section: "01"
title: "Hygiene & Targeted Fixes"
status: not-started
reviewed: true
goal: "Split bloated files so downstream sections have clean, right-sized modules to extend (BUG-04-029 shift overflow already fixed)"
success_criteria:
  - "ori_types/src/registry/traits/mod.rs is under 500 lines after extraction"
  - "ori_arc/src/pipeline/aims_pipeline.rs is under 500 lines after extraction"
  - "ori_llvm/src/codegen/arc_emitter/instr_dispatch.rs is under 500 lines after extraction"
  - "BUG-04-029 shift overflow checks pass in both debug and release"
  - "Satisfies mission criterion: ./test-all.sh green"
inspired_by:
  - "Ori codegen-purity plan section-01 (block merging extraction pattern)"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Split TraitRegistry (traits/mod.rs)"
    status: not-started
  - id: "01.2"
    title: "Split AIMS Pipeline (aims_pipeline.rs)"
    status: not-started
  - id: "01.3"
    title: "Split instr_dispatch.rs"
    status: not-started
  - id: "01.4"
    title: "Fix BUG-04-029 Shift Overflow Checks"
    status: not-started
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Hygiene & Targeted Fixes

**Status:** Not Started
**Goal:** Split three bloated files that downstream sections must extend (traits/mod.rs at 765 lines, aims_pipeline.rs at 590 lines, instr_dispatch.rs at 587 lines), and fix BUG-04-029 (shift overflow checks) which is a correctness prerequisite for LLVM metadata work. After this section, all files touched by the plan are under the 500-line limit and the LLVM backend produces correct shift behavior.

**Success Criteria:**

- [ ] `compiler/ori_types/src/registry/traits/mod.rs` is under 500 lines (currently 765) — `wc -l` verification
- [ ] `compiler/ori_arc/src/pipeline/aims_pipeline.rs` is under 500 lines (currently 590) — `wc -l` verification
- [ ] `compiler/ori_llvm/src/codegen/arc_emitter/instr_dispatch.rs` is under 500 lines (currently 587) — `wc -l` verification
- [ ] Verified BUG-04-029 shift overflow checks are already implemented (`checked_shl`/`checked_shr` in `checked_ops.rs`) — just needs test verification
- [ ] `./test-all.sh` green — no regressions from file splits or shift fix

**Context:** Section 07 (Algebra Law Schema) adds `AlgebraLawSet` fields to `TraitEntry` and `ImplEntry` in `traits/mod.rs`. Adding ~50 lines of algebra metadata to a 765-line file would push it well over the limit. Section 09 adds a normalization step to the AIMS pipeline, requiring modifications to `aims_pipeline.rs`. Section 04 adds TBAA metadata attachment at `ArcInstr::Project` emission sites in `instr_dispatch.rs`. All three files must be split first. BUG-04-029 (shift overflow) was already fixed — `checked_shl()`/`checked_shr()` in `checked_ops.rs` now emit negative count, width, and overflow checks. This section only verifies the fix, not implements it.

**Depends on:** Nothing.

---

## 01.1 Split TraitRegistry (traits/mod.rs)

**File(s):** `compiler/ori_types/src/registry/traits/mod.rs` (766 lines)

The `TraitRegistry` module currently contains: registry data structures (`TraitEntry`, `ImplEntry`, method storage), registration logic (adding traits/impls), resolution logic (looking up methods, checking satisfaction), and object safety checking. Extract resolution into a submodule.

- [ ] Create `compiler/ori_types/src/registry/traits/resolution.rs` — extract method lookup, trait satisfaction checking, and `resolve_impl_method` family of functions
- [ ] Create `compiler/ori_types/src/registry/traits/object_safety.rs` — extract `compute_object_safety_violations` and related helpers
- [ ] Keep `mod.rs` as the data structure home: `TraitRegistry`, `TraitEntry`, `ImplEntry`, `TraitMethodDef`, `ImplMethodDef`, and the `register_*` methods
- [ ] Update `mod.rs` to re-export all public items from submodules so external consumers are unaffected
- [ ] Verify: `wc -l compiler/ori_types/src/registry/traits/mod.rs` < 500
- [ ] Verify: `cargo c` compiles cleanly, no unused import warnings
- [ ] Verify: `timeout 150 cargo t -p ori_types` passes

- [ ] **Subsection close-out (01.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-01.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 01.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 01.2 Split AIMS Pipeline (aims_pipeline.rs)

**File(s):** `compiler/ori_arc/src/pipeline/aims_pipeline.rs` (590 lines)

The AIMS pipeline file contains: `AimsPipelineConfig`, `AimsPipelineResult`, the main `run_aims_pipeline` function, TRMC normalization logic (`normalize_with_trmc`), interprocedural pipeline orchestration (`run_aims_pipeline_all`), and ownership application. Extract TRMC normalization and interprocedural orchestration.

- [ ] Create `compiler/ori_arc/src/pipeline/trmc_normalize.rs` — extract `normalize_with_trmc` and its helpers
- [ ] Create `compiler/ori_arc/src/pipeline/interprocedural.rs` — extract `run_aims_pipeline_all`, `compute_aims_contracts`, and ownership application
- [ ] Keep `aims_pipeline.rs` as the per-function pipeline: `AimsPipelineConfig`, `AimsPipelineResult`, `run_aims_pipeline` (steps 3-12)
- [ ] Update `compiler/ori_arc/src/pipeline/mod.rs` to re-export from new submodules — `run_arc_pipeline` and `run_arc_pipeline_all` signatures unchanged
- [ ] Verify: `wc -l compiler/ori_arc/src/pipeline/aims_pipeline.rs` < 500
- [ ] Verify: `cargo c` compiles cleanly
- [ ] Verify: `timeout 150 cargo t -p ori_arc` passes

- [ ] **Subsection close-out (01.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-01.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 01.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 01.3 Split instr_dispatch.rs (BLOAT)

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/instr_dispatch.rs` (587 lines — over 500-line limit)

Section 04 will add TBAA metadata attachment at `ArcInstr::Project` emission sites in this file. It's already over the limit and will grow further. Split now.

- [ ] Verify current line count: `wc -l compiler/ori_llvm/src/codegen/arc_emitter/instr_dispatch.rs` > 500
- [ ] Extract `ArcInstr::Construct` emission arm and related helpers into `compiler/ori_llvm/src/codegen/arc_emitter/emit_construct.rs`
- [ ] Extract `ArcInstr::Apply` / `ArcInstr::ApplyIndirect` emission into `compiler/ori_llvm/src/codegen/arc_emitter/emit_apply.rs` (if large enough — check arm size)
- [ ] Keep `instr_dispatch.rs` as the top-level `emit_instruction()` dispatcher with small arms calling into submodules
- [ ] Verify: `wc -l compiler/ori_llvm/src/codegen/arc_emitter/instr_dispatch.rs` < 500
- [ ] Verify: `cargo c` compiles cleanly
- [ ] Verify: `timeout 150 cargo t -p ori_llvm` passes

- [ ] **Subsection close-out (01.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-01.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 01.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 01.4 ~~Fix BUG-04-029 Shift Overflow Checks~~ (ALREADY FIXED)

**Status:** Already implemented in `compiler/ori_llvm/src/codegen/ir_builder/checked_ops.rs`.

BUG-04-029 was fixed in a prior commit. `IrBuilder::checked_shl()` and `checked_shr()` now emit:
1. `icmp slt count, 0` → panic "shift by negative count"
2. `icmp sge count, 64` → panic "shift count overflow"
3. For shl: roundtrip check `(result >> count) != lhs` → panic "integer overflow"

The strategy dispatch (`operators/strategy.rs`) already calls `checked_shl`/`checked_shr` (lines 136-137).

- [ ] Verify existing implementation passes in debug: `timeout 150 ./test-all.sh`
- [ ] Verify existing implementation passes in release: `cargo b --release && timeout 150 ./test-all.sh`
- [ ] Update BUG-04-029 in bug tracker: change `- [ ]` to `- [x]` in `plans/bug-tracker/section-04-codegen-llvm.md` line 137 and add `<!-- resolved-by: plans/semantic-optimization-pipeline/section-01 -->`. As of 2026-04-04, the fix exists in `checked_ops.rs` but the bug tracker entry is still unchecked.

- [ ] **Subsection close-out (01.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-01.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 01.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 01.R Third Party Review Findings

- None.

---

## 01.N Completion Checklist

- [ ] `wc -l compiler/ori_types/src/registry/traits/mod.rs` < 500
- [ ] `wc -l compiler/ori_arc/src/pipeline/aims_pipeline.rs` < 500
- [ ] `wc -l compiler/ori_llvm/src/codegen/arc_emitter/instr_dispatch.rs` < 500
- [ ] BUG-04-029 shift tests verified passing in debug and release (already implemented)
- [ ] `timeout 150 ./test-all.sh` green (debug AND release — `cargo b --release && timeout 150 ./test-all.sh`)
- [ ] No spurious warnings in normal compilation
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 01` returns 0 annotations
- [ ] All intermediate TPR checkpoint findings resolved
- [ ] **Plan sync** — update plan metadata to reflect this section's completion:
  - [ ] This section's frontmatter `status` → `complete`, subsection statuses updated
  - [ ] `00-overview.md` Quick Reference table status updated
  - [ ] `00-overview.md` mission success criteria checkboxes updated
  - [ ] `index.md` section status updated
  - [ ] `plans/bug-tracker/section-04-codegen-llvm.md` BUG-04-029 updated with `<!-- resolved-by: ... -->`
  - [ ] Next section's `depends_on` verified
- [ ] `/tpr-review` passed (final, full-section)
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria:** All three target files are under 500 lines (traits/mod.rs, aims_pipeline.rs, instr_dispatch.rs). `timeout 150 ./test-all.sh` passes with 0 failures in both debug and release builds. BUG-04-029 shift overflow checks verified passing in both builds.
