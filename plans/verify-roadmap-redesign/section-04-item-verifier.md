---
section: "04"
title: "Item-Level Verifier Preservation"
status: not-started
reviewed: false
goal: "Extract the existing item-level verifier from the 900-line command file into a reusable phase that integrates as Phase 4 of the new pipeline"
success_criteria:
  - "Phase 1/Phase 2 agent architecture (review agents + update agents) extracted from .claude/commands/verify-roadmap.md"
  - "All existing verification criteria preserved: matrix coverage, semantic pins, test quality, hygiene audit, gap analysis"
  - "Verifier invocable as Phase 4 of the new pipeline (after cross-plan analysis)"
  - "Direct invocation supported: /verify-roadmap --section plans/roadmap/section-01-type-system.md"
  - "Existing verification behavior unchanged -- no regressions in item-level checks"
inspired_by: []
depends_on:
  - "01"
  - "02"
  - "03"
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Extract Core Verifier"
    status: not-started
  - id: "04.2"
    title: "Phase Integration"
    status: not-started
  - id: "04.3"
    title: "Targeted Invocation"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Item-Level Verifier Preservation

**Status:** Not Started
**Goal:** Extract the existing item-level verifier from the 900-line `.claude/commands/verify-roadmap.md` command file into a reusable module that integrates as Phase 4 of the new 5-phase pipeline. The existing verifier is the skill's strongest feature -- it assesses matrix coverage, semantic pins, test quality, and hygiene. This section preserves that capability while making it composable with the new cross-plan analysis.

**Success Criteria:**
- [ ] Core verifier logic extracted and modularized
- [ ] All existing verification criteria preserved without regression
- [ ] Integrates as Phase 4 in the new pipeline
- [ ] Supports direct invocation for targeted section verification

**Context:** The current `/verify-roadmap` command (`.claude/commands/verify-roadmap.md`, ~900 lines) uses a two-phase agent architecture: Phase 1 dispatches review agents to assess individual roadmap sections, and Phase 2 dispatches update agents to write findings back. The review agents check matrix coverage (type x pattern x feature), semantic pin presence, test quality, hygiene violations, and gap analysis. This is valuable work that the redesigned skill must retain. The extraction decouples item-level verification from the monolithic command file so it can be invoked selectively -- only on sections that the cross-plan analysis (Phases 1-3) identifies as needing deep verification.

**Depends on:** Section 01 (Frontmatter Schema) -- the verifier reads section frontmatter to determine status and locate checkboxes.

---

## 04.1 Extract Core Verifier

**File(s):** `.claude/skills/verify-roadmap/item-verifier.md` (new, extracted from `.claude/commands/verify-roadmap.md`)

Extract the core verification logic from the monolithic command file into a self-contained module. The module must be invocable independently and from the pipeline.

- [ ] Inventory the existing verification criteria in `.claude/commands/verify-roadmap.md`:
  - Matrix coverage assessment: does the section have type x pattern x feature test coverage?
  - Semantic pin verification: does the section have tests that would fail if the fix were reverted?
  - Test quality audit: are tests testing behavior (not implementation)?
  - Hygiene audit: dead code, stale references, missing docs
  - Gap analysis: what is missing from the section's claimed scope?
  - Checkbox item verification: are checked items actually complete?

- [ ] Extract the review agent prompt template:
  - The review agent receives a section file and produces a findings list
  - Preserve the prompt structure that tells the agent what to check
  - Parameterize the section path (currently hardcoded to `plans/roadmap/section-*.md`)
  - Make the agent prompt work for ANY plan's section files, not just the master roadmap

- [ ] Extract the update agent prompt template:
  - The update agent receives findings and writes them back to the section file
  - Preserve the write-back format (findings go into the section's TPR findings block)
  - Ensure the update agent respects the canonical frontmatter schema from Section 01

- [ ] Modularize the extracted logic:
  - Create `.claude/skills/verify-roadmap/item-verifier.md` with the extracted templates
  - Define clear inputs: section file path, verification scope (full or quick)
  - Define clear outputs: list of findings in the Section 03 report format
  - Document the module's interface so the pipeline (Section 05 SKILL.md) can invoke it

- [ ] Verify no functionality lost:
  - Compare the extracted verifier's output against the original command's output on 2-3 test sections
  - All verification criteria from the inventory must be present in the extracted module

- [ ] **Subsection close-out (04.1)** -- MANDATORY before starting 04.2:
  - [ ] All tasks above are `[x]` and extraction verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** -- check whether changes
        invalidated any CLAUDE.md, `.claude/rules/*.md`, or `canon.md`
        claims. If no changes, document briefly. Fix any drift NOW.

---

## 04.2 Phase Integration

**File(s):** `.claude/skills/verify-roadmap/item-verifier.md`, pipeline integration

Make the extracted verifier invocable as Phase 4 of the new 5-phase pipeline. In the full pipeline, Phase 4 runs after the cross-plan analysis (Phases 1-3) has identified which sections need deep verification.

- [ ] Define the Phase 4 invocation contract:
  - Input: list of section file paths selected by Phases 1-3 for deep verification
  - Selection criteria: sections with BLOCKED, CONFLICT, or SUPERSEDED findings get deep verification
  - Sections without cross-plan findings can optionally be verified (controlled by `--deep-all` flag)
  - Output: item-level findings appended to the Phase 3 findings report

- [ ] Implement the selection logic:
  - In `--full` mode: Phase 4 runs on all sections flagged by Phase 3 classifiers
  - In `--quick` mode: Phase 4 is skipped entirely (cross-plan check only)
  - In `--deep-all` mode: Phase 4 runs on every section in the corpus (original behavior)
  - In `--section <path>` mode: Phase 4 runs on the specified section only (targeted)

- [ ] Integrate Phase 4 findings with Phase 3 report format (reusing 01.3 SSOT — do NOT redefine):
  - Item-level findings use the `Finding` dataclass imported from `plan_corpus` (01.3) — no shadow type
  - Phase 4 findings carry `category = FindingCategory.ITEM_VERIFICATION` (defined in 01.3, NOT added here)
  - Subtypes: `MISSING_MATRIX_COVERAGE`, `MISSING_SEMANTIC_PIN`, `MISSING_NEGATIVE_PIN`, `WEAK_TEST`, `HYGIENE_VIOLATION`, `INCOMPLETE_CHECKBOX`, `SCOPE_GAP` — all enumerated in 01.3's `FindingSubtype` under the `ITEM_VERIFICATION` category. Adding a new subtype requires editing 01.3 (the SSOT), not Section 04 (CODEX-01-004 closure).
  - Severity mapping (encoded in 01.3 alongside the subtype definitions): `MISSING_SEMANTIC_PIN` = high, `MISSING_NEGATIVE_PIN` = high, `MISSING_MATRIX_COVERAGE` = medium, `WEAK_TEST` = medium, `HYGIENE_VIOLATION` = low, `INCOMPLETE_CHECKBOX` = high, `SCOPE_GAP` = medium
  - The construction-time invariant (`subtype MUST belong to category`, enforced in `Finding.__post_init__`) prevents Phase 4 from accidentally stamping an `ITEM_VERIFICATION` subtype onto a different category

- [ ] **Subsection close-out (04.2)** -- MANDATORY before starting 04.3:
  - [ ] All tasks above are `[x]` and pipeline integration tested
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** -- check whether changes
        invalidated any CLAUDE.md, `.claude/rules/*.md`, or `canon.md`
        claims. If no changes, document briefly. Fix any drift NOW.

---

## 04.3 Targeted Invocation

**File(s):** `.claude/skills/verify-roadmap/SKILL.md` (targeted mode)

Support direct invocation for targeted section verification without running the full cross-plan pipeline. This preserves the most common use case of the original command: verifying a single section after completing work on it.

- [ ] Implement `--section <path>` argument:
  - Accepts a path to a specific section file (e.g., `plans/roadmap/section-01-type-system.md`)
  - Skips Phases 1-3 (schema validation, DAG construction, conflict classification)
  - Runs Phase 4 (item-level verification) directly on the specified section
  - Runs Phase 5 (report) with findings from the single section only

- [ ] Implement `--plan <name>` argument:
  - Accepts a plan directory name (e.g., `roadmap`, `repr-opt`)
  - Runs Phases 1-3 scoped to that plan only (faster than full corpus)
  - Runs Phase 4 on all sections of the specified plan
  - Useful for verifying plan-internal coherence without cross-plan analysis

- [ ] Document usage patterns:
  - After completing a roadmap section: `/verify-roadmap --section plans/roadmap/section-05-traits.md`
  - After modifying a reroute plan: `/verify-roadmap --plan repr-opt`
  - Full corpus audit: `/verify-roadmap --full`
  - Quick pre-check for `/continue-roadmap`: `/verify-roadmap --quick`
  - Default (no args): equivalent to `--full`

- [ ] **Subsection close-out (04.3)** -- MANDATORY before marking section complete:
  - [ ] All tasks above are `[x]` and targeted invocation tested
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** -- check whether changes
        invalidated any CLAUDE.md, `.claude/rules/*.md`, or `canon.md`
        claims. If no changes, document briefly. Fix any drift NOW.
  - [ ] **Repo hygiene check** -- run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] Core verifier extracted from `.claude/commands/verify-roadmap.md` into reusable module
- [ ] All 6 existing verification criteria preserved: matrix coverage, semantic pins, test quality, hygiene, gap analysis, checkbox verification
- [ ] Phase 4 integration with selection logic (`--full`, `--quick`, `--deep-all`, `--section`)
- [ ] Targeted invocation via `--section` and `--plan` arguments
- [ ] No regression in item-level verification output compared to original command
- [ ] `timeout 150 ./test-all.sh` green -- no regressions
- [ ] `/tpr-review` -- dual-source review of extraction completeness and phase integration
- [ ] `/impl-hygiene-review` -- verify no verification criteria lost, module interface clean
- [ ] `/improve-tooling` section-close sweep -- verify per-subsection retrospectives ran; add cross-subsection findings
- [ ] `/sync-claude` section-close sweep -- verify CLAUDE.md and rules reflect new invocation modes
