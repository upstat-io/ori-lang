---
section: "06"
title: "Plan schema — mandatory Intelligence Reconnaissance subsection"
status: not-started
reviewed: false
goal: "Make `{NN}.0 Intelligence Reconnaissance` a required subsection in every new plan section, enforced by plan_corpus.py validation"
success_criteria:
  - "`.claude/skills/create-plan/plan-schema.md` defines `{NN}.0 Intelligence Reconnaissance` as a required subsection in the Section File Template"
  - "`scripts/plan_corpus.py check` emits a WARNING (not error during transition) when a plan section lacks the subsection"
  - "Retrofit policy (A/B/C) is applied per user decision and documented in this section's frontmatter"
  - "Satisfies mission criterion: plan-schema mandates Intelligence Reconnaissance; validator emits transition-period WARNING"
inspired_by:
  - "Existing plan-schema `{NN}.1 {Subsection}` structure — the new subsection is inserted at index 0"
  - "`scripts/plan_corpus.py check` existing frontmatter validation — extended with a body-level subsection check"
  - "TPR findings codex-029 [high], 030 [medium], gemini-005 [medium]"
depends_on: ["03"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "06.1"
    title: "Edit plan-schema.md to add {NN}.0 template"
    status: not-started
  - id: "06.2"
    title: "Extend plan_corpus.py validator"
    status: not-started
  - id: "06.3"
    title: "Retrofit active plans per user policy (A/B/C)"
    status: not-started
  - id: "06.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "06.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Plan schema — mandatory Intelligence Reconnaissance

**Status:** Not Started
**Goal:** Every new plan section must begin with a `{NN}.0 Intelligence Reconnaissance` subsection that runs the §03 SSOT helper to establish graph context BEFORE the section's implementation subsections begin. Plan-corpus validation catches sections that skip it — as a WARNING during the transition period, upgradable to an error once retrofit converges.

**Context:** Plan sections today are the primary unit of compiler work. Each has goal, success_criteria, subsections, completion checklist — but no required reconnaissance. A section about the ARC pipeline can land with zero graph queries, no blast-radius context, and no cross-repo prior art beyond whatever the author happened to remember. Making reconnaissance a schema requirement turns the graph from "something you remember to do" into "something the plan corpus enforces." TPR finding TPR-XX-029 (codex, high severity) is the root driver; TPR-XX-030 extends to retrofit coverage; gemini-005 concurs.

**Reference implementations:**
- **Ori** `.claude/skills/create-plan/plan-schema.md` existing Section File Template — the target of this edit
- **Ori** `scripts/plan_corpus.py` existing validation logic — extended with a subsection check
- **TPR finding provenance**: codex-029, codex-030, gemini-005

**Depends on:** Section 03 (the subsection template cites the SSOT helper).

---

## 06.1 Edit plan-schema.md to add {NN}.0 template

**File(s):** `.claude/skills/create-plan/plan-schema.md`

The existing Section File Template has subsections `{NN}.1`, `{NN}.2`, ..., `{NN}.R`, `{NN}.N`. The new `{NN}.0` slot goes BEFORE all implementation subsections because reconnaissance should precede design.

- [ ] Add to the Section File Template in `plan-schema.md` (insert after the section header block, before `## {NN}.1 {Subsection Title}`):

  ```markdown
  ## {NN}.0 Intelligence Reconnaissance (MANDATORY)

  Before diving into implementation, establish graph context:

  @.claude/skills/dual-tpr/compose-intel-summary.md

  Target the section's declared scope — which crates, types, or files the
  section touches. Specific queries to run:

  - `scripts/intel-query.sh --human <preset>` where `<preset>` matches this
    section's subsystem (see `.claude/rules/intelligence.md` §Subsystem
    Mapping — e.g., `ori-arc`, `ori-inference`, `ori-codegen`, `ori-patterns`,
    `ori-diagnostics`, or a bare `search "<key terms>"` for mixed scope)
  - `scripts/intel-query.sh --human file-symbols "<path-fragment>" --repo ori`
    for every path this section will edit
  - `scripts/intel-query.sh --human callers "<symbol>" --repo ori` for every
    public API this section will change
  - `scripts/intel-query.sh --human similar "<symbol>" --repo rust,swift,koka,lean4 --limit 5`
    for cross-repo prior art on the section's design decisions

  Condense results to a bounded paragraph (≤500 chars) and paste into this
  subsection. Future sections that consume this section's output see the
  reconnaissance baseline.

  If the graph is unavailable, document: "Intelligence reconnaissance
  skipped: graph unavailable at {date}. Implementation proceeds without
  graph context." Do NOT silently skip.
  ```

- [ ] Update the mandatory-subsection-structure comment block in `plan-schema.md` (currently at template line ~315) to include `{NN}.0` alongside `{NN}.1`, `{NN}.2`, etc.

- [ ] Update the `sections:` frontmatter list example in the template to include the `{NN}.0` row:

  ```yaml
  sections:
    - id: "{NN}.0"
      title: "Intelligence Reconnaissance"
      status: not-started
    - id: "{NN}.1"
      title: "{Subsection}"
      status: not-started
    # ...
  ```

- [ ] **Subsection close-out (06.1)**:
  - [ ] Template changes land; `plan-schema.md` renders with the new `{NN}.0` block in the canonical example
  - [ ] Update `06.1` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 06.1** — was editing plan-schema.md more painful than it should have been? (It's a long file; a schema-linter for its own structure could help.) Commit via `build(tooling): ...` or note negative.
  - [ ] **Run `/sync-claude` on 06.1** — `plan-schema.md` is the SSOT for plan shape; any `.claude/rules/*.md` that references plan structure (e.g., `tests.md` "plans must have TDD matrix") may need a note about the new `{NN}.0` slot.
  - [ ] **Repo hygiene check**.

---

## 06.2 Extend plan_corpus.py validator

**File(s):** `scripts/plan_corpus.py` (or `scripts/plan_corpus/validate.py` if the package shape uses submodules)

- [ ] Add a body-level check: for every section file, if its body does not contain `## {NN}.0 Intelligence Reconnaissance` OR the subsection exists but is empty / stubbed, emit a WARNING.

- [ ] Gate severity: WARNING by default for the transition period. Add a `--strict-recon` flag that escalates to ERROR for new plans (opt-in for `/create-plan` post-§06; opt-out for legacy plans during retrofit).

- [ ] Update `scripts/plan_corpus.py discover` output to report per-plan reconnaissance coverage: "plans/foo/ — 3/5 sections have Intelligence Reconnaissance" so operators see retrofit progress.

- [ ] Write a unit test in `tests/plan-audit/` covering:
  - Positive: a section with `{NN}.0 Intelligence Reconnaissance` body passes
  - Warning: a section missing the subsection emits a WARNING (not error)
  - Strict: `--strict-recon` promotes the warning to an error

- [ ] **Subsection close-out (06.2)**:
  - [ ] Validator extension + tests land
  - [ ] Update `06.2` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 06.2** — does the validator error message include a pointer to the SSOT helper? ("Section 03.2 lacks `{NN}.0 Intelligence Reconnaissance`. Add the subsection per `.claude/skills/create-plan/plan-schema.md` §Section File Template and run the queries listed there.") If the message is terse, improve it. Commit via `build(tooling): ...`.
  - [ ] **Run `/sync-claude` on 06.2** — CLAUDE.md §Commands mentions `plan_corpus.py check`. Verify the description still matches post-edit.
  - [ ] **Repo hygiene check**.

---

## 06.3 Retrofit active plans per user policy (A/B/C)

**File(s):** Multiple (list depends on user-selected policy from the plan overview's open question)

Policy choices (from the approved plan file's open question):
- **Option A (aggressive)**: retrofit every active plan section, including in-progress
- **Option B (moderate, recommended)**: retrofit only `status: not-started` sections
- **Option C (passive)**: no retrofit; new sections only

- [ ] Before executing this subsection, invoke AskUserQuestion with the three options AND the rationale from the overview's open question.

- [ ] Based on the selected option:
  - **Option A**: for every `plans/*/section-*.md` file (excluding `completed/`), inject the `{NN}.0` subsection. For in-progress sections, note the retrofit in a close-out addendum.
  - **Option B**: for every `status: not-started` section under `plans/*/`, inject the subsection.
  - **Option C**: no work; document the policy in `plan-schema.md` as "retrofit passive — new sections only."

- [ ] For each retrofitted section, re-run `plan_corpus.py check` and confirm the WARNING no longer fires.

- [ ] Update this plan's own section files (§01-§08) to include their `{NN}.0` subsection — meta-dogfood. The current plan was written before §06 existed; retrofitting it is proof-of-work.

- [ ] **Subsection close-out (06.3)**:
  - [ ] Retrofit complete per selected policy
  - [ ] Update `06.3` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 06.3** — was the retrofit mechanical enough that `scripts/plan_corpus.py inject-recon --plan <dir>` should exist? Commit via `build(tooling): ...` if matured.
  - [ ] **Run `/sync-claude` on 06.3** — plan corpus changed en masse; CLAUDE.md Key Paths mentions `plans/` — no change needed. Verify no rule file references a specific plan section number that shifted due to retrofit.
  - [ ] **Repo hygiene check**.

---

## 06.R Third Party Review Findings

- None.

---

## 06.N Completion Checklist

- [ ] `plan-schema.md` includes the `{NN}.0 Intelligence Reconnaissance` template
- [ ] `plan_corpus.py check` emits a WARNING for sections missing the subsection; `--strict-recon` promotes to error
- [ ] New unit tests in `tests/plan-audit/` pass
- [ ] Retrofit policy applied (A/B/C per user choice); retrofit coverage documented in `00-overview.md` of each touched plan
- [ ] This plan's own sections (§01-§08) include their `{NN}.0` subsections (meta-dogfood)
- [ ] `./test-all.sh` green (including the new plan-audit tests)
- [ ] `python scripts/plan_corpus.py check plans/query-intel-adoption/section-06-plan-schema-recon.md` returns 0 errors and 0 reconnaissance warnings
- [ ] **Plan sync**:
  - [ ] Section frontmatter → `complete`
  - [ ] `00-overview.md` Quick Reference and mission criteria updated
  - [ ] `index.md` updated
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed — verify `{NN}.0` template does NOT inline the SSOT helper; it `@`-includes it
- [ ] `/improve-tooling` section-close sweep
- [ ] `/sync-claude` section-close doc sync
- [ ] `diagnostics/repo-hygiene.sh --check`

**Exit Criteria:** Every plan section created AFTER §06 lands has an `{NN}.0 Intelligence Reconnaissance` subsection. Retrofit coverage per user policy is in place. `plan_corpus.py` warns on drift. `./test-all.sh` green.
