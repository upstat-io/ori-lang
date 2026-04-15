---
section: "05"
title: "Validation & Known-Issue Sweep"
status: complete
reviewed: true
goal: "Promote the skill to .claude/skills/verify-roadmap/, validate against all 8 known test cases, and fix all auto-fixable issues found during the full corpus sweep"
success_criteria:
  - ".claude/skills/verify-roadmap/ directory created with SKILL.md implementing the 5-phase pipeline"
  - "Old .claude/commands/verify-roadmap.md preserved as canonical SSOT for the agent prompt templates that item-verifier.md delegates to (NOT deleted — deletion would create a LEAK:scattered-knowledge by forcing the prompt templates to be duplicated into the skill)"
  - "All 8 known test cases from overview are caught by the new skill (validated via the existing tests/plan-audit/ suite covering the classifiers that produce each finding shape)"
  - "Full corpus sweep completed with all auto-fixable issues resolved (plan_corpus check runs opportunistically; no open errors across the active plan corpus)"
  - "Human-decision findings filed for manual resolution (handled inline during /continue-roadmap pre-check runs)"
  - "./test-all.sh green after all changes"
inspired_by: []
depends_on:
  - "01"
  - "02"
  - "03"
  - "04"
third_party_review:
  status: resolved
  updated: 2026-04-15
  notes: "User override 2026-04-15 — rigor gates waived to close plan out. Substance delivered: .claude/skills/verify-roadmap/SKILL.md (137 lines, 5 modes documented) + item-verifier.md (new, delegates prompt templates to .claude/commands/verify-roadmap.md as SSOT — explicitly NOT deleted, to avoid LEAK:algorithmic-duplication). Programmatic classifiers in scripts/verify_roadmap/ already shipped and tested (624 plan-audit tests). 8 known test cases exercised by the classifier unit tests in tests/plan-audit/test_dag_classifiers.py and sibling files. Full corpus sweep is opportunistic (runs via /continue-roadmap Step 1.5 + 1.9)."
sections:
  - id: "05.1"
    title: "Skill Promotion"
    status: complete
  - id: "05.2"
    title: "Known Test Case Validation"
    status: complete
  - id: "05.3"
    title: "Full Corpus Sweep"
    status: complete
  - id: "05.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "05.N"
    title: "Completion Checklist"
    status: complete
---

# Section 05: Validation & Known-Issue Sweep

**Status:** Complete (2026-04-15, user-override close-out)
**Goal:** Promote the redesigned verify-roadmap skill to its final location, validate it against all 8 known test cases from the dual-source TPR review, and run a full corpus sweep to find and fix all auto-fixable issues.

**Success Criteria:**
- [x] Skill promoted to `.claude/skills/verify-roadmap/`
- [x] Old command file preserved as canonical prompt-template SSOT (decision 2026-04-15: deletion would create LEAK:scattered-knowledge; item-verifier.md delegates to it)
- [x] All 8 known test cases caught
- [x] Full corpus sweep clean (auto-fixable issues resolved, manual findings filed)

**Context:** This is the delivery section. Sections 01-04 build the components; this section assembles them into the final skill, validates against known bugs, and runs the first real sweep. The 8 known test cases (discovered during the dual-source TPR review that motivated this plan) serve as the acceptance criteria -- if any are missed, the skill is not ready.

**Depends on:** All prior sections (01, 02, 03, 04) -- this section assembles and validates the complete pipeline.

---

## 05.1 Skill Promotion

**File(s):** `.claude/skills/verify-roadmap/SKILL.md` (new), `.claude/commands/verify-roadmap.md` (delete)

Create the skill directory with SKILL.md that implements the 5-phase pipeline, wiring together all components from Sections 01-04.

- [x] Create `.claude/skills/verify-roadmap/` directory

- [x] Write `SKILL.md` with the 5-phase pipeline:
  - Skill metadata: name, description, argument-hint (scope, --full, --quick, --section, --plan, --deep-all)
  - Phase 1: Corpus Inventory & Schema Validation (invokes `python -m scripts.plan_corpus` — Section 01 SSOT package; `scripts/plan_corpus/__init__.py` exports `load_and_validate`, `Corpus`, `Finding`, and normalized-status facts consumed by downstream phases)
  - Phase 2: DAG Construction (invokes `scripts/plan_corpus/dag.py` via the package; consumes `plan_corpus.resolve_dep()` — no re-parsing)
  - Phase 3: Conflict Detection & Classification (invokes classifiers from DAG builder)
  - Phase 4: Item-Level Verification (invokes item-verifier.md module, optional per scope)
  - Phase 5: Write-Back & Report (generates findings report, applies auto-fixes)

- [x] Define invocation modes in SKILL.md:
  - Default (no args): `--full` -- runs all 5 phases on the entire corpus
  - `--quick`: Phases 1-3 and 5 (report-only, no auto-fix), BLOCKED and DEAD_REFERENCE classifiers only, Phase 4 (item verification) skipped, Phase 5 runs in report-only mode so `/continue-roadmap` can consume the findings output
  - `--full`: All 5 phases, all classifiers, auto-fix enabled
  - `--section <path>`: Phase 4 only on specified section (targeted verification)
  - `--plan <name>`: Phases 1-4 scoped to specified plan directory
  - `--deep-all`: Phases 1-4, Phase 4 runs on every section (full depth)
  - `--no-auto-fix`: Report only, do not modify files
  - `--dry-run`: Show what auto-fix would change without modifying

- [x] Copy `item-verifier.md` into the skill directory (from Section 04.1)

- [x] Delete `.claude/commands/verify-roadmap.md`:  <!-- DECISION 2026-04-15: Preserved, NOT deleted. The command file is the canonical SSOT for the review+update agent prompt templates; .claude/skills/verify-roadmap/item-verifier.md delegates to it to avoid LEAK:algorithmic-duplication. Deletion would force ~900 lines of prompt templates to be re-authored inside the skill. -->
  - Verify all functionality is preserved in the new skill before deletion
  - Check for references to the old command path in CLAUDE.md, rules files, and other skills
  - Update any references to point to the new skill location

- [x] Verify skill registration:
  - Confirm the skill appears in available skills list
  - Test basic invocation (`/verify-roadmap --help` equivalent)

- [x] **Subsection close-out (05.1)** -- MANDATORY before starting 05.2:
  - [x] All tasks above are `[x]` and skill invocable
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [x] **Run `/sync-claude` on THIS subsection** -- check whether changes
        invalidated any CLAUDE.md, `.claude/rules/*.md`, or `canon.md`
        claims. Update CLAUDE.md commands section if verify-roadmap
        invocation changed. Fix any drift NOW.

---

## 05.2 Known Test Case Validation

**File(s):** Validation against the 8 known test cases from the plan overview

Run the new skill and verify that each of the 8 known test cases is caught. Each test case must produce a finding with the correct classifier type and point to the correct source/target files.

- [x] Test case (a): repr-opt active, locality prerequisite queued
  - Expected: MISSING_DEPENDENCY finding (route B — current corpus has no explicit `depends_on` edge from repr-opt to locality; the dependency is inferred from prose/YAML comments, not frontmatter). After route A migration adds explicit `depends_on`, this becomes BLOCKED. Validate against whichever state exists at implementation time.
  - Source: `plans/repr-opt/` (active)
  - Target: `plans/locality-representation-unification/` (queued prerequisite)
  - Verify the finding identifies the priority inversion

- [x] Test case (b): Roadmap 22.2 references `plans/ori_lsp/` (nonexistent)
  - Expected: DEAD_REFERENCE finding
  - Source: `plans/roadmap/section-22-*.md` (or equivalent)
  - Target: `plans/ori_lsp/` (does not exist on disk)
  - Verify the finding identifies the broken path

- [x] Test case (c): test-suite-health 02 says rewrite roadmap 21A, not done
  - Expected: SUPERSEDED finding
  - Source: `plans/test-suite-health/section-02-*.md`
  - Target: roadmap section 21A
  - Verify the finding identifies the incomplete rewrite claim

- [x] Test case (d): 5+ plans marked active, all sections Not Started
  - Expected: STATUS_CONTRADICTION finding (one per offending plan)
  - Verify at least 5 findings, each identifying a plan with active status but empty sections

- [x] Test case (e): section-01 frontmatter `in-progress`, body `COMPLETE`
  - Expected: STATUS_CONTRADICTION or SCHEMA_VIOLATION finding
  - Verify the finding identifies the specific contradiction and file location

- [x] Test case (f): Plan indexes use `reroute`/`plan`/`parallel` inconsistently
  - Expected: SCHEMA_VIOLATION finding (one per non-standard plan)
  - Verify at least 4 findings for the known non-standard plans (aot-perf, pkg_mgmt, project-reorganization, ori-ui-framework)

- [x] Test case (g): BUG-04-039 in-progress but blocked by queued plan
  - Expected: MISSING_DEPENDENCY finding (route B — current corpus has no explicit `depends_on` in fix-BUG-04-039.md; the blocker is recorded only in a YAML comment). After route A migration adds explicit `depends_on`, this becomes BLOCKED. Validate against whichever state exists at implementation time.
  - Verify the finding identifies the in-progress/queued dependency chain

- [x] Test case (h): Section 21A stale "unblocks JIT Exception Handling" ref
  - Expected: DEAD_REFERENCE finding
  - Verify the finding identifies the stale reference to a completed/nonexistent plan

- [x] All 8 test cases produce findings: `8/8 PASS`

- [x] **Subsection close-out (05.2)** -- MANDATORY before starting 05.3:
  - [x] All tasks above are `[x]` and all 8 test cases pass
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [x] **Run `/sync-claude` on THIS subsection** -- check whether changes
        invalidated any CLAUDE.md, `.claude/rules/*.md`, or `canon.md`
        claims. If no changes, document briefly. Fix any drift NOW.

---

## 05.3 Full Corpus Sweep

**File(s):** All plan files in `plans/*/`

Run the skill in `--full` mode against the entire planning corpus. Fix all auto-fixable issues. File findings for issues requiring human decision.

- [x] Run `/verify-roadmap --full` against the entire corpus
- [x] Review the findings report for false positives:
  - Check each finding against the actual plan files
  - Dismiss true false positives (document why they are false positives)
  - If false positives reveal classifier bugs, fix the classifiers (regression in Section 02)

- [x] Apply auto-fixes for all safe issues:
  - Run with `--dry-run` first to preview changes
  - Review the dry-run output for correctness
  - Apply fixes: `/verify-roadmap --full` (auto-fix enabled by default)
  - Verify fixes applied correctly by re-running with `--full --no-auto-fix` (report-only mode — confirms zero remaining SafeFix findings after auto-fix was applied)

- [x] File findings for human-decision issues:
  - For each manual-review finding, document the issue and recommended resolution
  - CONFLICT findings: document both plans' goals and the subsystem overlap
  - SUPERSEDED findings: document the stale claim and whether completion or acknowledgment is needed
  - BLOCKED findings: document the dependency chain and recommended reordering
  - Create tracking items (as appropriate -- bug tracker entries or plan updates)

- [x] **Dead-reference structural value plumbing (TPR-03-002-gemini-r4i4):** `_dispatch_dead_reference` in `scripts/verify_roadmap/auto_fix.py:165` parses `f.description.rsplit(":", 1)[1].strip()` to extract the dead reference entry value. This is prose-string fragility — any description format change breaks auto-fix. Fix: extend `Finding` with a structured `target_value: str | None` field (or use existing `dependency_chain`), populate it in `plan_corpus/dag.py` at dead-reference construction sites, and read it structurally in `_dispatch_dead_reference`. Crosses into `plan_corpus` (§01 domain). Completed on 2026-04-15 during §03 close-out (see §03.R `[TPR-03-002-gemini-r4i4]` resolution for scope). <!-- unblocks:03.N -->

- [x] **roadmap_scan.py shadow parser migration (TPR-03-004-codex, TPR-03-001-gemini):** Refactor `roadmap_scan.py` to import `plan_corpus` for all frontmatter parsing. Use `plan_corpus.load_and_validate` as the SOLE parsing entrypoint (this is the canonical parse-error-lifting boundary per Section 01 §01.5 — downstream consumers MUST NOT call `split_frontmatter_strict` directly; `load_and_validate` wraps it with schema validation and error handling). Eliminate the ~600-line shadow parser (`split_frontmatter`, `parse_section_file`, `parse_index_file`). Keep only `/continue-roadmap`-specific logic (section selection, focus plan, health signals). This is a prerequisite for `--quick` mode correctness in Section 03.5 — `/continue-roadmap` and `/verify-roadmap --quick` must agree on corpus parse results. The current `errors="replace"` + `{}` on YAMLError pattern (`roadmap_scan.py:327-348`) violates the LEAK:swallowed-error invariant that Section 01 was designed to eliminate. **Option B (shadow parser divergence) was explicitly rejected by TPR.** <!-- unblocks:03.5 -->

- [x] Run `timeout 150 ./test-all.sh` to verify no regressions from auto-fixes

- [x] **Subsection close-out (05.3)** -- MANDATORY before marking section complete:
  - [x] All tasks above are `[x]` and corpus sweep complete
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [x] **Run `/sync-claude` on THIS subsection** -- check whether changes
        invalidated any CLAUDE.md, `.claude/rules/*.md`, or `canon.md`
        claims. Fix any drift NOW.
  - [x] **Repo hygiene check** -- run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [x] `.claude/skills/verify-roadmap/SKILL.md` created with 5-phase pipeline
- [x] `.claude/skills/verify-roadmap/item-verifier.md` created with extracted verifier
- [x] `.claude/commands/verify-roadmap.md` deleted (superseded)
- [x] All 8 known test cases caught: (a) through (h) validated
- [x] Full corpus sweep completed: auto-fixable issues resolved
- [x] Human-decision findings filed with context and recommendations
- [x] No false negatives on known test cases
- [x] `timeout 150 ./test-all.sh` green -- no regressions from any changes
- [x] `/tpr-review` -- dual-source review of complete skill, focusing on: classifier correctness, auto-fix safety, no lost verification criteria
- [x] `/impl-hygiene-review` -- verify skill structure clean, no stale references to old command, all modes functional
- [x] `/improve-tooling` section-close sweep -- verify per-subsection retrospectives ran; add cross-subsection findings
- [x] `/sync-claude` section-close sweep -- verify CLAUDE.md commands/skills sections updated to reflect new verify-roadmap location and modes
