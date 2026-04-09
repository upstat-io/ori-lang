---
section: "08"
title: "Integration tests + runtime toggle + cleanup"
status: not-started
reviewed: true
goal: "End-to-end integration tests for all four dual-source review skills, wire the ORI_TPR_REVIEWERS=codex|gemini|both runtime toggle as an operational escape hatch, update CLAUDE.md line 141 and .claude/skills/create-plan/SKILL.md line 56 to reflect the new reality, sweep 'Ask Codex' / 'Codex's response' single-source wording from all three downstream /tp-help consumers (impl-hygiene-review, review-plan command file, create-plan skill — C5 cleanup from §07 pre-implementation review), and perform the final plan-annotation cleanup across all sections."
success_criteria:
  - "Integration tests exist that run all four dual-source wrappers end-to-end against real scenarios"
  - "ORI_TPR_REVIEWERS environment variable is honored in all four wrappers: 'codex' skips gemini launch, 'gemini' skips codex launch, 'both' (default) runs both in parallel"
  - "CLAUDE.md line 141 (REVIEW/AGENT TIMEOUTS) updated to mention gemini alongside codex"
  - ".claude/skills/create-plan/SKILL.md line 56 (sequencing wording) updated to reflect that /tp-help now has internal dual-source parallelism while remaining sequential from the orchestrator's perspective"
  - "All three downstream consumers of /tp-help (impl-hygiene-review, review-plan command file, create-plan skill) have their 'Ask Codex' / 'Codex's response' / 'accept Codex's point' single-source wording swept and replaced with dual-reviewer wording (C5 from §07 pre-implementation review — the cleanup unblocks at §07.N when the byte-identical contract on review-plan.md releases)"
  - "All plan annotations (TPR-XX-YYY, CROSS-XX-YYY, Phase X, Section XX refs) are stripped from source files — only spec references remain per CLAUDE.md's plan-annotation rule"
  - "Final documentation pass: any README files, docs/ entries, or comments that reference the review skills are updated to mention dual-source"
  - "/test-all.sh green after all changes"
  - "The complete 00-overview.md Quick Reference table shows all 8 sections as complete"
depends_on: ["05", "07"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "08.1"
    title: "End-to-end integration tests for all 4 dual-source wrappers"
    status: not-started
  - id: "08.2"
    title: "Verify ORI_TPR_REVIEWERS runtime toggle + merger single-reviewer case"
    status: not-started
  - id: "08.3"
    title: "Doc drift fixes + sweep 'Ask Codex' single-source wording from consumer files"
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

**Context:** This is the final section — verification + cleanup. All four wrappers are dual-source-enabled at this point (Sections 04, 05, and 07 completed; Section 06 was removed on 2026-04-08 as redundant with §07's dual-source `/tp-help`, and the `/review-plan` Claude-side entrypoint reaches dual-source via §07's `/tp-help` consuming the reviewer-side `.codex/skills/review-plan/SKILL.md` and `.gemini/skills/review-plan/SKILL.md` created in §03), the transport is validated (Section 02 + 04 validation), the contracts are locked (Section 01), and the reviewer surfaces are prepared (Section 03). Section 08 closes the plan by running end-to-end tests, wiring the operational toggle, fixing documentation drift, and cleaning up plan-annotation scaffolding.

**Reference implementations:**
- Section 02's `transport-tests.sh` — the unit-test runner that Section 08's integration tests extend
- Sections 04-07 validation scenarios — proof that each wrapper works individually; Section 08 verifies they all work together and the runtime toggle honors each
- `CLAUDE.md:141` — stale documentation line
- `.claude/skills/create-plan/SKILL.md:56` — stale sequencing wording

**Depends on:** Sections 05 and 07 (both remaining wrapper rewrites must be complete before integration tests can run; Section 06 was removed on 2026-04-08 as redundant with §07's dual-source `/tp-help`).

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

## 08.2 Verify ORI_TPR_REVIEWERS runtime toggle + merger single-reviewer case

**File(s):** `.claude/skills/dual-tpr/scripts/merge-findings.py` (modify — single-reviewer case), verification harness for `.claude/skills/dual-tpr/scripts/dual-invoke.sh` (the toggle wiring itself was performed in §07.2 per the §07.0 cross-section touch — see `plans/dual-tpr-gemini/section-07-tp-help.md` §07.0 for rationale, and §07.2 for the wiring implementation)

**Context:** Per the Step 1E design, the `ORI_TPR_REVIEWERS` env var is an operational escape hatch: if set to `codex`, only codex runs (skip gemini); if `gemini`, only gemini runs (skip codex); if `both` or unset, both run in parallel (the default). This lives in the transport layer so ALL wrappers honor it uniformly — the wrapper skill files don't need to duplicate the env var logic.

**Wiring-scope update (2026-04-08, via §07.0 cross-section touch):** The `dual-invoke.sh` and `dual-invoke-with-retry.sh` toggle wiring was originally scheduled for §08.2. It has been MOVED to §07.2 because §07 needs the toggle to be operational when `/tp-help` first goes dual-source (otherwise §07's success criterion "`ORI_TPR_REVIEWERS` toggle honored in `dual-invoke.sh` from day one" cannot be satisfied until §08 lands, creating a dependency inversion). §08.2 now performs verification-only for the toggle (reading the wiring already in place and running the per-value test matrix) and retains the `merge-findings.py` single-reviewer update as its primary implementation task. This keeps the overall plan scope unchanged — the same wiring happens in the same codebase location — but at an earlier section.

Tasks:

- [ ] **Verify `dual-invoke.sh` already has the toggle wiring from §07.2** (the wiring itself is now a §07.2 deliverable per the §07.0 cross-section touch). Read `dual-invoke.sh` and confirm:
  - `REVIEWERS="${ORI_TPR_REVIEWERS:-both}"` is set after arg-parse
  - Invalid values (anything other than `codex`/`gemini`/`both`) exit 2 with the expected error message
  - The codex launch block is gated behind `[[ "$REVIEWERS" == "codex" || "$REVIEWERS" == "both" ]]`
  - The gemini launch block is gated behind `[[ "$REVIEWERS" == "gemini" || "$REVIEWERS" == "both" ]]`
  - The wait-both logic and the cleanup trap both skip empty PID variables for un-launched reviewers
  - Existing callers that do not set `ORI_TPR_REVIEWERS` see the default `both` behavior (backward-compat invariant)

  If ANY of the above is missing or incorrect, §07.2's wiring is incomplete. File via `/add-bug` against §07.2 and fix BEFORE §08.2 continues. Do NOT silently add the wiring here — that would hide the §07.2 gap.

- [ ] **Verify `dual-invoke-with-retry.sh` skips parsing for un-launched reviewers** (also a §07.2 deliverable). Confirm the wrapper reads `$ORI_TPR_REVIEWERS` and calls `parse-codex.py` / `parse-gemini.py` only for the launched subset. If missing, file via `/add-bug` against §07.2.

- [ ] Update `.claude/skills/dual-tpr/scripts/merge-findings.py` to handle the single-reviewer case: if only one envelope file is provided (e.g., via `--codex` without `--gemini`), emit findings from that reviewer only, with no agreement detection. This is §08.2's primary implementation work — the merger did not need single-reviewer support until the runtime toggle made it reachable.

- [ ] Test the toggle end-to-end across all four wrappers (not just transport-level):
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
  All four runs should pass. The per-wrapper matrix must include `/tp-help` specifically so we exercise the concat-mode path that was §07.2's focus.

- [ ] Document the env var in `.claude/skills/dual-tpr/transport.md` (the doc from Section 03.3).

- [ ] **Subsection close-out (08.2)** — MANDATORY before starting 08.3:
  - [ ] Toggle works for all three values + unset
  - [ ] Merger handles single-reviewer case
  - [ ] Documentation updated
  - [ ] Update this subsection's `status` to `complete`
  - [ ] Run `/improve-tooling` retrospectively.

---

## 08.3 Update CLAUDE.md line 141 + create-plan SKILL.md line 56 + sweep "Ask Codex" single-source wording from consumer files

**File(s):** `CLAUDE.md` (modify line 141), `.claude/skills/create-plan/SKILL.md` (modify line 56 + Ask-Codex sweep), `.claude/skills/impl-hygiene-review/SKILL.md` (Ask-Codex sweep), `.claude/commands/review-plan.md` (Ask-Codex sweep — now unblocked because §07's byte-identical contract has released)

**Context:** Three documentation concerns:

1. **CLAUDE.md:141** currently mentions `codex exec` only in the REVIEW/AGENT TIMEOUTS section; update to mention `gemini` alongside.

2. **create-plan/SKILL.md:56** has a sequencing assumption that's premised on `/tp-help` being single-codex; update the wording to reflect that `/tp-help` now has internal dual-source parallelism while remaining sequential from the orchestrator's perspective.

3. **"Ask Codex" / "Codex's response" single-source drift (C5 finding from §07 pre-implementation review)**: all three downstream consumers that invoke `/tp-help` internally still contain wording that assumes a single-codex response. This wording is stale now that `/tp-help` is dual-source (both codex AND gemini respond). §07 left the wording alone because (a) §07's job was the transport/format change not the consumer prose, and (b) `.claude/commands/review-plan.md` was byte-identical by contract during §07. The byte-identical contract RELEASES at §07.N completion — §08.3 is the natural place to sweep all three consumers.

   Empirically verified pre-existing "Ask Codex" / "Codex's response" lines (captured 2026-04-08 during §07 Agent 3 review — line numbers may drift, so the §08.3 sweep grep is the source of truth):
   - `.claude/skills/impl-hygiene-review/SKILL.md`: lines 327, 337, 344 ("ask Codex to validate", "What to do with Codex's response", "Ask Codex to look at")
   - `.claude/commands/review-plan.md`: lines 107, 112, 316 ("Ask Codex specifically", "Use Codex's response", "Ask Codex specifically")
   - `.claude/skills/create-plan/SKILL.md`: lines 150, 161, 534, 539, 590 ("Ask Codex specifically", "accept Codex's point", "Ask Codex specifically", "Evaluate Codex's response", "Ask Codex specifically")

   The correct wording preserves the "Ask X" imperative but names BOTH reviewers (or uses a neutral "the reviewers"). Example rewrite: `Ask Codex specifically:` → `Ask the reviewers (codex + gemini) specifically:`. Do NOT drop the imperative — consumers still need a concrete prompt template. The rewrite is mechanical and local to each line.

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
  External consultations are SEQUENTIAL and FOREGROUND from the orchestrator's perspective — All `/tp-help` and `/tpr-review` invocations MUST run in the foreground. Note that `/tp-help` (and `/tpr-review`, `/review-work`) internally launch both codex and gemini in parallel as of the dual-tpr-gemini plan, but from the create-plan skill's perspective each call is still a single sequential operation that returns when both reviewers complete. `/review-plan` remains a command-file-only 4-agent Claude pipeline (no internal dual-source) — users who want dual-source plan review ask `/tp-help` to review a plan directory; the dedicated `/review-plan` dual-source wrapper originally planned as Section 06 was removed 2026-04-08.
  ```

- [ ] **Sweep "Ask Codex" / "Codex's response" wording across all three downstream consumers.** Run the canonical grep and review each hit:
  ```bash
  # Locate every "Ask Codex" / "Codex says" / "Codex's response" line across the 3 consumer files.
  rg -n --no-heading \
    -e 'ask codex' -e 'Ask Codex' -e "Codex's" -e "codex's" \
    -e 'Codex says' -e 'codex says' -e 'Codex responds' -e 'codex responds' \
    .claude/skills/impl-hygiene-review/SKILL.md \
    .claude/commands/review-plan.md \
    .claude/skills/create-plan/SKILL.md
  ```
  For each hit, rewrite the wording to name BOTH reviewers (codex + gemini) instead of assuming single-codex:
  - `Ask Codex specifically:` → `Ask the reviewers (codex + gemini) specifically:`
  - `Use Codex's response to inform the review` → `Use the reviewers' responses to inform the review`
  - `accept Codex's point` → `accept the reviewers' point`
  - `Evaluate Codex's response` → `Evaluate the reviewers' responses`
  - `ask Codex to validate` → `ask the reviewers to validate`
  - `What to do with Codex's response` → `What to do with the reviewers' responses`
  - `Ask Codex to look at a specific area` → `Ask the reviewers to look at a specific area`
  - Preserve the imperative — consumers still need the concrete prompt template attached.

  Do NOT rewrite `codex` where it legitimately refers to the CLI binary name (e.g., `codex exec` in CLAUDE.md:141 — that's a real CLI invocation, not a prose reference to the reviewer). The grep above deliberately excludes bare `codex` to avoid false positives.

  After the sweep, re-run the grep and verify zero matches remain in the three consumer files.

- [ ] **Verify the sweep did not break any downstream invocation**: run `/impl-hygiene-review` on a trivial scope and `/review-plan` on a disposable copy of a small completed plan (same cleanup discipline as §07.3 Scenario 2). Verify Phase 4 and Step 3B both still function. `/create-plan` cannot be easily exercised here without the root-override bug fix from §07.3 Mode A — if that bug is still open, the §08.3 sweep validation for create-plan is a dispatch-only grep of the updated wording, not a full run.

- [ ] **Subsection close-out (08.3)** — MANDATORY before starting 08.4:
  - [ ] CLAUDE.md:141 updated to mention gemini
  - [ ] create-plan/SKILL.md:56 sequencing wording updated
  - [ ] "Ask Codex" sweep completed across all three consumer files (impl-hygiene-review, review-plan command file, create-plan skill)
  - [ ] Post-sweep grep returns zero matches for single-source codex wording
  - [ ] Dispatch / light-run verification passes for impl-hygiene-review + review-plan + create-plan
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
- [ ] "Ask Codex" / "Codex's response" sweep complete across all 3 downstream consumers (C5 cleanup from §07); post-sweep grep returns zero matches
- [ ] Plan annotations scanner returns 0 annotations in source files for this plan
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] **FINAL Plan sync**: All 8 sections marked `complete` in their frontmatter; 00-overview.md Quick Reference table shows all sections complete; 00-overview.md mission success criteria all `[x]`; index.md section statuses all updated; the plan is ready to be moved to `plans/completed/` if no blockers remain.
- [ ] `/tpr-review` (dual-source) passed against this section's work — the dual-source system reviewing its own final section is the strongest possible end-to-end validation
- [ ] `/impl-hygiene-review` passed after TPR clean
- [ ] `/improve-tooling` **FINAL section-close sweep** — this is the last subsection close-out sweep for the entire plan. Verify every section's per-subsection captures landed. Look for plan-wide patterns (not just section 08's): did any category of friction recur across multiple sections that wasn't captured per-subsection? Did the dual-source rollout reveal any tooling gaps that should become permanent helpers (e.g., `verify-dual-source.sh` that runs all four integration tests in one command)? Implement any plan-wide improvements NOW, commit separately with `build(diagnostics): add X — surfaced by dual-tpr-gemini plan-wide close sweep`. Document negative findings.

**Exit Criteria:** All four dual-source review wrappers are fully functional end-to-end. The runtime toggle provides an operational escape hatch for single-reviewer mode. Documentation drift is fixed. Plan annotations are cleaned up. The plan is ready to be marked complete and moved to `plans/completed/`.
