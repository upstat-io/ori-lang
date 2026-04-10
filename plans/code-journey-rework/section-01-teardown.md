---
section: "01"
title: "Tear Down Broken Pipeline"
status: not-started
reviewed: true
goal: "Delete the ~5,000-line Python analysis pipeline and scoring system that was proven broken by dual-source TPR review"
success_criteria:
  - "All Python metric extraction files deleted from .claude/skills/code-journey/"
  - "SCHEMA.md deleted (replaced by JSON schema in Section 02)"
  - "score.py, extract-metrics.py, and all metric modules deleted"
  - "rescore-all.sh deleted"
  - "Python test files and golden test data deleted"
  - "SKILL.md stripped to reusable Steps 0-2 skeleton (scoring sections removed)"
  - "No references to deleted files remain in the codebase"
  - "cargo test and ./test-all.sh still green (Python files are not Rust-tested)"
inspired_by: []
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Delete Python Pipeline Files"
    status: not-started
  - id: "01.2"
    title: "Strip SKILL.md to Reusable Skeleton"
    status: not-started
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Tear Down Broken Pipeline

**Status:** Not Started
**Goal:** Delete the broken Python analysis pipeline, scoring system, and schema. Preserve the reusable execution infrastructure (Steps 0-2 phase capture) in SKILL.md.

**Success Criteria:**
- [ ] All Python files in `.claude/skills/code-journey/` deleted except none (all go)
- [ ] SCHEMA.md deleted (replaced by JSON schema in Section 02)
- [ ] SKILL.md reduced to reusable skeleton (Steps 0-2 + modes preserved, scoring sections deleted)
- [ ] `rescore-all.sh` deleted
- [ ] All Python test files and golden data deleted
- [ ] No dangling references to deleted files anywhere in the codebase
- [ ] Satisfies mission criterion: "The ~5,000 lines of broken Python analysis pipeline are deleted"

**Context:** A dual-source TPR review (codex + gemini, 2026-04-09) independently confirmed that the Python analysis pipeline is fundamentally broken: flat RC summation hides path-dependent leaks, rc_state.py is dead code, effect_summaries.py has wrong COW effects, instruction metrics use a circular "ideal" definition, and the scoring system is a rubber stamp (18/20 at 10.0). The pipeline produces false confidence while real bugs go undetected. Deletion is the correct fix — the diagnostic tools in `diagnostics/` are the real bug-finders.

**Depends on:** Nothing — this is Phase 0.

---

## 01.1 Delete Python Pipeline Files

**File(s):** `.claude/skills/code-journey/`

Delete ALL Python files, their tests, and supporting data:

- [ ] Delete Python metric modules:
  - `.claude/skills/code-journey/arc_metrics.py` (452 lines — flat RC summation, broken)
  - `.claude/skills/code-journey/rc_state.py` (397 lines — dead code, never called)
  - `.claude/skills/code-journey/instruction_metrics.py` (192 lines — circular ideal metric)
  - `.claude/skills/code-journey/control_flow_metrics.py` (142 lines)
  - `.claude/skills/code-journey/attribute_metrics.py` (322 lines)
  - `.claude/skills/code-journey/binary_metrics.py` (128 lines)
  - `.claude/skills/code-journey/effect_summaries.py` (196 lines — wrong COW effects)
  - `.claude/skills/code-journey/ir_parser.py` (331 lines)
  - `.claude/skills/code-journey/ir_parser_internal.py` (252 lines)
  - `.claude/skills/code-journey/ir_utils.py` (106 lines)
  - `.claude/skills/code-journey/extract_ir_from_results.py` (104 lines)

- [ ] Delete scoring and extraction scripts:
  - `.claude/skills/code-journey/score.py` (630 lines — arbitrary thresholds, rubber stamp)
  - `.claude/skills/code-journey/extract-metrics.py` (271 lines — wires broken arc_metrics)
  - `.claude/skills/code-journey/rescore-all.sh` (204 lines — batch re-scorer for broken system)

- [ ] Delete the scoring schema:
  - `.claude/skills/code-journey/SCHEMA.md` (823 lines — score-centric, replaced by JSON schema in Section 02)

- [ ] Delete Python test files and golden data:
  - `.claude/skills/code-journey/tests/test_arc_metrics.py`
  - `.claude/skills/code-journey/tests/test_effect_summaries.py`
  - `.claude/skills/code-journey/tests/test_extract_metrics.py`
  - `.claude/skills/code-journey/tests/test_instruction_metrics.py`
  - `.claude/skills/code-journey/tests/test_rc_state.py`
  - `.claude/skills/code-journey/tests/test_attribute_metrics.py`
  - `.claude/skills/code-journey/tests/test_control_flow_metrics.py`
  - `.claude/skills/code-journey/tests/test_binary_metrics.py`
  - `.claude/skills/code-journey/tests/test_extract_ir.py`
  - `.claude/skills/code-journey/tests/golden/` (all golden test data)

- [ ] Verify no dangling references:
  - Grep codebase for references to deleted filenames
  - Check `.claude/commands/` for any references to deleted scripts
  - Check `plans/code-journeys/overview.md` for references to scoring tools

- [ ] **Subsection close-out (01.1)** — MANDATORY before starting 01.2:
  - [ ] All tasks above are `[x]` and deletions verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 01.2 Strip SKILL.md to Reusable Skeleton

**File(s):** `.claude/skills/code-journey/SKILL.md`

The current SKILL.md (692 lines) has reusable infrastructure (Steps 0-2: build run list, run both paths, capture phase dumps) and broken sections (scoring instructions, background agent scoring template, deep scrutiny framework). Strip it to a skeleton that Section 03 will rebuild.

- [ ] Preserve SKILL.md frontmatter (name, description, argument-hint)
- [ ] Preserve "CRITICAL: Scenario Preservation" section (NEVER modify existing .ori files)
- [ ] Preserve "CRITICAL: Autonomous Execution" section (no user prompts)
- [ ] Preserve "CRITICAL: Context Conservation" section (background agent delegation)
- [ ] Preserve Step 0: Build the Run List (lines 66-108 — journey discovery, modes)
- [ ] Preserve Step 1: Run Both Paths (lines 112-139 — eval + AOT execution)
- [ ] Preserve Step 2: phase dump capture (lines 141-177 — env vars, temp files)
- [ ] Preserve "Adding New Scenarios" section (lines 89-108 — gap-filling logic)
- [ ] **DELETE** the scoring instructions section (lines 478-610)
- [ ] **DELETE** the old background agent prompt template (tied to scoring)
- [ ] **DELETE** references to SCHEMA.md, score.py, extract-metrics.py
- [ ] **REPLACE** "CRITICAL: Schema Compliance" with a note: "Schema replaced — see Section 02 of plans/code-journey-rework/ for the JSON results schema"
- [ ] Add a `<!-- PLACEHOLDER: Section 03 will add the new orchestrator logic here -->` marker where the new Steps 3-5 will go

- [ ] **Subsection close-out (01.2)** — MANDATORY before marking section complete:
  - [ ] All tasks above are `[x]` and SKILL.md skeleton verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 01.R Third Party Review Findings

- None.

---

## 01.N Completion Checklist

- [ ] All Python pipeline files deleted (~5,000 lines)
- [ ] SCHEMA.md deleted
- [ ] SKILL.md stripped to reusable skeleton
- [ ] No dangling references to deleted files in codebase
- [ ] `timeout 150 ./test-all.sh` green (Python files are not Rust-tested, but verify no build dependencies)
- [ ] `/tpr-review` — dual-source review of teardown work
- [ ] `/impl-hygiene-review` — verify no DRIFT (stale references), no WASTE (dead code left behind)
- [ ] `/improve-tooling` section-close sweep — verify per-subsection retrospectives ran; add cross-subsection findings
