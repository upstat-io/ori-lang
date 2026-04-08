---
section: "08"
title: "Integration tests + runtime toggle + cleanup"
status: not-started
reviewed: true
goal: "End-to-end integration tests for all four dual-source review skills, wire the ORI_TPR_REVIEWERS=codex|gemini|both runtime toggle as an operational escape hatch, update CLAUDE.md line 141 and .claude/skills/create-plan/SKILL.md line 56 to reflect the new reality, and perform the final plan-annotation cleanup across all sections."
success_criteria:
  - "Integration tests exist that run all four dual-source wrappers end-to-end against real scenarios"
  - "ORI_TPR_REVIEWERS environment variable is honored in all four wrappers: 'codex' skips gemini launch, 'gemini' skips codex launch, 'both' (default) runs both in parallel"
  - "CLAUDE.md line 141 (REVIEW/AGENT TIMEOUTS) updated to mention gemini alongside codex"
  - ".claude/skills/create-plan/SKILL.md line 56 (sequencing wording) updated to reflect that /tp-help now has internal dual-source parallelism while remaining sequential from the orchestrator's perspective"
  - "All plan annotations (TPR-XX-YYY, CROSS-XX-YYY, Phase X, Section XX refs) are stripped from source files — only spec references remain per CLAUDE.md's plan-annotation rule"
  - "Final documentation pass: any README files, docs/ entries, or comments that reference the review skills are updated to mention dual-source"
  - "/test-all.sh green after all changes"
  - "The complete 00-overview.md Quick Reference table shows all 8 sections as complete"
depends_on: ["05", "06", "07"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "08.1"
    title: "End-to-end integration tests for all 4 dual-source wrappers"
    status: not-started
  - id: "08.2"
    title: "Wire ORI_TPR_REVIEWERS runtime toggle"
    status: not-started
  - id: "08.3"
    title: "Update CLAUDE.md line 141 + create-plan SKILL.md line 56"
    status: not-started
  - id: "08.4"
    title: "Plan annotation cleanup across source files"
    status: not-started
  - id: "08.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "08.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 08: Integration tests + runtime toggle + cleanup

**Status:** Not Started
**Goal:** Integration tests + runtime toggle + doc drift fixes + plan annotation cleanup. This section closes the plan by verifying end-to-end behavior, adding the operational escape hatch, fixing the stale documentation, and stripping ephemeral plan-annotation scaffolding from source files.

**Success Criteria:**

- [ ] End-to-end integration tests for all 4 dual-source wrappers exist and pass: `/tpr-review`, `/review-work`, `/review-plan`, `/tp-help`
- [ ] `ORI_TPR_REVIEWERS=codex|gemini|both` runtime toggle is honored in all 4 wrappers
- [ ] Default is `both` when the env var is unset
- [ ] Setting to `codex` runs only codex (gemini launch path is skipped)
- [ ] Setting to `gemini` runs only gemini (codex launch path is skipped)
- [ ] `CLAUDE.md:141` updated: REVIEW/AGENT TIMEOUTS section mentions `gemini` alongside `codex`, referring to the hook enforcement from Section 01
- [ ] `.claude/skills/create-plan/SKILL.md:56` sequencing wording updated to reflect dual-source `/tp-help`
- [ ] All code annotations referencing this plan's sections (TPR-01-XXX, TPR-02-XXX, etc.) are stripped from source files: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan dual-tpr-gemini` returns 0 annotations
- [ ] `./test-all.sh` green
- [ ] `00-overview.md` Quick Reference table shows all 8 sections as `complete`
- [ ] `00-overview.md` mission success criteria checkboxes are all `[x]`
- [ ] Section 08 frontmatter `status: complete`

**Context:** This is the final section — verification + cleanup. All four wrappers are dual-source-enabled at this point (Sections 04, 05, 06, 07 completed), the transport is validated (Section 02 + 04 validation), the contracts are locked (Section 01), and the reviewer surfaces are prepared (Section 03). Section 08 closes the plan by running end-to-end tests, wiring the operational toggle, fixing documentation drift, and cleaning up plan-annotation scaffolding.

**Reference implementations:**
- Section 02's `transport-tests.sh` — the unit-test runner that Section 08's integration tests extend
- Sections 04-07 validation scenarios — proof that each wrapper works individually; Section 08 verifies they all work together and the runtime toggle honors each
- `CLAUDE.md:141` — stale documentation line
- `.claude/skills/create-plan/SKILL.md:56` — stale sequencing wording

**Depends on:** Sections 05, 06, 07 (all three wrapper rewrites must be complete before integration tests can run).

---

## 08.1 End-to-end integration tests for all 4 dual-source wrappers

**File(s):** `.claude/skills/dual-tpr/scripts/integration-tests.sh` (new)

**Context:** Section 02's `transport-tests.sh` covers unit tests for individual transport components with fixtures. Section 08's integration tests cover full end-to-end flows — a wrapper invoked on a real scenario, with both reviewers running, findings merged, and output written (for /tpr-review and /review-work) or presented (for /review-plan and /tp-help).

Tasks:

- [ ] Write `.claude/skills/dual-tpr/scripts/integration-tests.sh` that runs each wrapper against a small test scenario. Structure:
  ```bash
  #!/usr/bin/env bash
  set -uo pipefail

  PASS=0; FAIL=0; FAILED=()

  test_wrapper() {
    local name="$1"
    local scenario="$2"
    # Run the wrapper against the scenario; check exit code and output shape
    # ...
  }

  # /tpr-review integration test
  test_wrapper "tpr-review" "small-test-scenario-1"

  # /review-work integration test
  test_wrapper "review-work" "small-test-scenario-2"

  # /review-plan integration test
  test_wrapper "review-plan" "plans/completed/small-test-plan"

  # /tp-help integration test
  test_wrapper "tp-help" "what is 2+2"

  echo "PASS: $PASS, FAIL: $FAIL"
  [[ $FAIL -eq 0 ]] || exit 1
  ```

- [ ] Run the integration test suite and verify all four wrappers pass end-to-end.

- [ ] Note: integration tests invoke REAL codex and gemini, so they're slow (estimated 5-20 minutes for all four). They're NOT run as part of `./test-all.sh` — they're a separate `integration-tests.sh` run manually or in CI on-demand.

- [ ] **Subsection close-out (08.1)** — MANDATORY before starting 08.2:
  - [ ] Integration test script exists, runs all 4 wrappers, reports pass/fail
  - [ ] All 4 wrappers pass
  - [ ] Update this subsection's `status` to `complete`
  - [ ] Run `/improve-tooling` retrospectively.

---

## 08.2 Wire ORI_TPR_REVIEWERS runtime toggle

**File(s):** `.claude/skills/dual-tpr/scripts/dual-invoke.sh` (modify), `.claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh` (modify)

**Context:** Per the Step 1E design, the `ORI_TPR_REVIEWERS` env var is an operational escape hatch: if set to `codex`, only codex runs (skip gemini); if `gemini`, only gemini runs (skip codex); if `both` or unset, both run in parallel (the default). This lives in the transport layer so ALL wrappers honor it uniformly — the wrapper skill files don't need to duplicate the env var logic.

Tasks:

- [ ] Modify `.claude/skills/dual-tpr/scripts/dual-invoke.sh` to read `$ORI_TPR_REVIEWERS` and branch:
  ```bash
  REVIEWERS="${ORI_TPR_REVIEWERS:-both}"
  if [[ "$REVIEWERS" != "codex" && "$REVIEWERS" != "gemini" && "$REVIEWERS" != "both" ]]; then
    echo "invalid ORI_TPR_REVIEWERS: $REVIEWERS (must be codex|gemini|both)" >&2
    exit 2
  fi

  if [[ "$REVIEWERS" == "codex" || "$REVIEWERS" == "both" ]]; then
    # launch codex in bg
  fi

  if [[ "$REVIEWERS" == "gemini" || "$REVIEWERS" == "both" ]]; then
    # launch gemini in bg
  fi

  # wait for whichever reviewers were launched
  ```

- [ ] Update `dual-invoke-with-retry.sh` to skip parsing for reviewers that weren't launched. When `ORI_TPR_REVIEWERS=codex`, only parse `$RUN/codex.jsonl`; gemini's envelope is absent and the merger step handles the single-reviewer case.

- [ ] Update `.claude/skills/dual-tpr/scripts/merge-findings.py` to handle the single-reviewer case: if only one envelope file is provided (e.g., via `--codex` without `--gemini`), emit findings from that reviewer only, with no agreement detection.

- [ ] Test the toggle:
  ```bash
  # codex-only
  ORI_TPR_REVIEWERS=codex bash .claude/skills/dual-tpr/scripts/integration-tests.sh
  # gemini-only
  ORI_TPR_REVIEWERS=gemini bash .claude/skills/dual-tpr/scripts/integration-tests.sh
  # both (default)
  ORI_TPR_REVIEWERS=both bash .claude/skills/dual-tpr/scripts/integration-tests.sh
  # unset (should default to both)
  unset ORI_TPR_REVIEWERS
  bash .claude/skills/dual-tpr/scripts/integration-tests.sh
  ```
  All four runs should pass.

- [ ] Document the env var in `.claude/skills/dual-tpr/transport.md` (the doc from Section 03.3).

- [ ] **Subsection close-out (08.2)** — MANDATORY before starting 08.3:
  - [ ] Toggle works for all three values + unset
  - [ ] Merger handles single-reviewer case
  - [ ] Documentation updated
  - [ ] Update this subsection's `status` to `complete`
  - [ ] Run `/improve-tooling` retrospectively.

---

## 08.3 Update CLAUDE.md line 141 + create-plan SKILL.md line 56

**File(s):** `CLAUDE.md` (modify line 141), `.claude/skills/create-plan/SKILL.md` (modify line 56)

**Context:** Two small documentation fixes. CLAUDE.md:141 currently mentions `codex exec` only in the REVIEW/AGENT TIMEOUTS section; update to mention `gemini` alongside. create-plan/SKILL.md:56 has a sequencing assumption that's premised on `/tp-help` being single-codex; update the wording to reflect that `/tp-help` now has internal dual-source parallelism while remaining sequential from the orchestrator's perspective.

Tasks:

- [ ] Read `CLAUDE.md:141` and verify the current content. Update to mention gemini:
  ```
  # Before:
  REVIEW/AGENT TIMEOUTS: Review/analysis tasks (... `codex exec` ...) legitimately take 5–35 minutes...
  The `.claude/hooks/block-banned-commands.sh` hook enforces this: it blocks any `timeout` under 300000 ms (5 min) or over 2100000 ms (35 min) on codex commands.

  # After:
  REVIEW/AGENT TIMEOUTS: Review/analysis tasks (... `codex exec`, `gemini` review invocations ...) legitimately take 5–35 minutes...
  The `.claude/hooks/block-banned-commands.sh` hook enforces this: it blocks any `timeout` under 300000 ms (5 min) or over 2100000 ms (35 min) on codex AND gemini commands.
  ```

- [ ] Read `.claude/skills/create-plan/SKILL.md:56` and verify the current content. The line is about the "External consultations are SEQUENTIAL and FOREGROUND" rule for `/tp-help` invocations during plan creation. Update the wording:
  ```
  # Before:
  External consultations are SEQUENTIAL and FOREGROUND — All `/tp-help` and `/tpr-review` invocations MUST run in the foreground...

  # After:
  External consultations are SEQUENTIAL and FOREGROUND from the orchestrator's perspective — All `/tp-help` and `/tpr-review` invocations MUST run in the foreground. Note that `/tp-help` (and `/tpr-review`, `/review-work`, `/review-plan`) internally launch both codex and gemini in parallel as of the dual-tpr-gemini plan, but from the create-plan skill's perspective each call is still a single sequential operation that returns when both reviewers complete.
  ```

- [ ] **Subsection close-out (08.3)** — MANDATORY before starting 08.4:
  - [ ] Both doc fixes applied
  - [ ] Update this subsection's `status` to `complete`
  - [ ] Run `/improve-tooling` retrospectively.

---

## 08.4 Plan annotation cleanup across source files

**File(s):** Any source files with `TPR-01-*`, `TPR-02-*`, ..., `TPR-08-*` annotations from this plan

**Context:** Per CLAUDE.md's "Plan annotations are temporary scaffolding" rule, all code annotations referencing this plan's sections must be removed when the plan completes. This is the mandatory cleanup step. Spec references (`Spec: Clause N.M`) are permanent; plan-section annotations are ephemeral.

Tasks:

- [ ] Run the plan-annotations scanner to find any remaining TPR-XX-YYY references from this plan:
  ```bash
  bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan dual-tpr-gemini
  ```

- [ ] For each annotation found in source files (not plan docs), remove it. Plan documents themselves may still cite section IDs; the rule applies only to code files.

- [ ] Re-run the scanner to verify 0 remaining annotations in source files.

- [ ] Run `timeout 150 ./test-all.sh` after annotation removal to verify no regressions.

- [ ] **Subsection close-out (08.4)** — MANDATORY before section completion:
  - [ ] Plan-annotations scanner reports 0 annotations in source files
  - [ ] test-all.sh green
  - [ ] Update this subsection's `status` to `complete`
  - [ ] Run `/improve-tooling` retrospectively.

---

## 08.R Third Party Review Findings

- None.

---

## 08.N Completion Checklist

- [ ] All four subsections (08.1, 08.2, 08.3, 08.4) marked `complete`
- [ ] Integration test suite exists and all 4 wrappers pass end-to-end
- [ ] ORI_TPR_REVIEWERS toggle honored in all 4 wrappers
- [ ] CLAUDE.md:141 updated to mention gemini
- [ ] .claude/skills/create-plan/SKILL.md:56 updated to reflect dual-source /tp-help
- [ ] Plan annotations scanner returns 0 annotations in source files for this plan
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] **FINAL Plan sync**: All 8 sections marked `complete` in their frontmatter; 00-overview.md Quick Reference table shows all sections complete; 00-overview.md mission success criteria all `[x]`; index.md section statuses all updated; the plan is ready to be moved to `plans/completed/` if no blockers remain.
- [ ] `/tpr-review` (dual-source) passed against this section's work — the dual-source system reviewing its own final section is the strongest possible end-to-end validation
- [ ] `/impl-hygiene-review` passed after TPR clean
- [ ] `/improve-tooling` **FINAL section-close sweep** — this is the last subsection close-out sweep for the entire plan. Verify every section's per-subsection captures landed. Look for plan-wide patterns (not just section 08's): did any category of friction recur across multiple sections that wasn't captured per-subsection? Did the dual-source rollout reveal any tooling gaps that should become permanent helpers (e.g., `verify-dual-source.sh` that runs all four integration tests in one command)? Implement any plan-wide improvements NOW, commit separately with `build(diagnostics): add X — surfaced by dual-tpr-gemini plan-wide close sweep`. Document negative findings.

**Exit Criteria:** All four dual-source review wrappers are fully functional end-to-end. The runtime toggle provides an operational escape hatch for single-reviewer mode. Documentation drift is fixed. Plan annotations are cleaned up. The plan is ready to be marked complete and moved to `plans/completed/`.
