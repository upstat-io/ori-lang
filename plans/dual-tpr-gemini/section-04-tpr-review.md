---
section: "04"
title: "/tpr-review dual-source (validation case)"
status: in-progress
reviewed: true
goal: "Rewrite .claude/skills/tpr-review/SKILL.md to invoke the Section 02 transport utility and launch both codex and gemini in parallel per round. First consumer of the dual-source transport and serves as the validation gate: Sections 05/06/07 do not start until Section 04 successfully validates the transport against ≥2 real TPR scenarios with both agreement and disagreement cases demonstrated."
success_criteria:
  - ".claude/skills/tpr-review/SKILL.md invokes .claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh and merges findings via merge-findings.py"
  - "The 10-iteration semantic loop is preserved; infra retries are separate from semantic iterations per the Section 02 contract"
  - "At least 2 real TPR scenarios run successfully end-to-end with both reviewers producing findings"
  - "At least one agreement case (same (location, title) from both reviewers) demonstrated in real review output"
  - "At least one disagreement case (different findings from each reviewer) demonstrated and surfaced explicitly in the plan TPR block"
  - "Merged findings written to the owning plan section's TPR block with reviewer-tagged IDs ([TPR-NN-NNN-codex] and [TPR-NN-NNN-gemini]) using independent ordinal sequences"
  - "Dirty-worktree guard catches a deliberate test injection (reviewer prompt that tries to modify a tracked file) and the round fails with the diff surfaced to the user"
  - "Infra retry recovers from a transient failure (fault injection: kill the reviewer subprocess once, verify retry succeeds)"
inspired_by:
  - ".claude/skills/tpr-review/SKILL.md (existing 252-line single-source pattern) — the workflow shape this section generalizes to dual-source"
  - "Section 02's transport scripts — the API this rewrite consumes"
  - "Section 03's reviewer surface — the codex envelope-only mode and gemini skill activation convention"
depends_on: ["03"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Rewrite .claude/skills/tpr-review/SKILL.md for dual-source transport"
    status: complete
  - id: "04.2"
    title: "Loop semantics, failure handling, and user escalation"
    status: not-started
  - id: "04.3"
    title: "Real TPR scenario validation (critical-path gate)"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: /tpr-review dual-source (validation case)

**Status:** Not Started
**Goal:** Rewrite the `/tpr-review` Claude wrapper to use the Section 02 transport utility, launching both codex and gemini in parallel per round. This section is the validation gate — it's the first real consumer of the dual-source transport, so any transport bugs surface here before propagating to Sections 05/06/07.

**Success Criteria:**

- [ ] `.claude/skills/tpr-review/SKILL.md` is rewritten to invoke `.claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh` via background bash, then call `parse-codex.py`, `parse-gemini.py`, and `merge-findings.py` in sequence
- [ ] The wrapper's 10-iteration semantic loop is preserved from the existing single-source version (fix findings → re-run reviewers → until clean or max iterations)
- [ ] Infra retries (Section 02's 3-retry budget) are separate from semantic iterations — verified by observing that transport failures do NOT decrement the 10-iteration counter
- [ ] At least 2 real TPR scenarios have been run against the rewritten skill with both reviewers producing findings. Satisfies mission criteria for agreement and disagreement surfacing.
- [ ] At least one agreement case and one disagreement case have been observed and surfaced in the plan TPR block with the correct reviewer-tagged ID format
- [ ] Dirty-worktree guard injection test passes: a deliberately-crafted prompt that asks the reviewer to modify a tracked file produces a failed round with the diff surfaced to the user
- [ ] Infra retry fault injection test passes: killing the reviewer subprocess once triggers a retry that succeeds

**Context:** This section is the critical-path gate for the plan. Everything downstream (Sections 05, 06, 07) consumes the same transport and reviewer-surface patterns that this section is the first to exercise end-to-end. If the transport has subtle bugs that didn't surface in Section 02's fixture tests — for example, real codex output containing characters the parser mishandles, or real gemini stream-json having event orderings the delta-concat doesn't anticipate — those bugs surface here. The success criteria require ≥2 real TPR scenarios with both agreement and disagreement cases, which exercises the whole stack against production-like conditions.

**Reference implementations:**
- `.claude/skills/tpr-review/SKILL.md` (existing 252-line single-source wrapper) — the workflow skeleton that this rewrite generalizes from one reviewer to two
- Section 02's transport scripts — the scripts this wrapper composes
- Section 03's `.claude/skills/dual-tpr/transport.md` — the wrapper invocation pattern doc

**Depends on:** Section 03 (reviewer surface preparation, which must be complete so that both codex and gemini skills are ready to invoke in envelope-only mode).

---

## 04.1 Rewrite .claude/skills/tpr-review/SKILL.md for dual-source transport

**File(s):** `.claude/skills/tpr-review/SKILL.md` (rewrite)

**Context:** The existing skill follows the pattern: write prompt → codex exec in bg → parse JSONL → classify findings → fix or exit. The dual-source version follows: write prompt → `dual-invoke-with-retry.sh` (which launches both reviewers, parses both envelopes, validates, worktree-guards, retries on infra failure) → `merge-findings.py` → classify merged findings → fix or exit. The skill file shrinks from 252 lines to roughly 180 lines because most of the per-round work is delegated to the transport scripts.

Tasks:

- [x] Read the existing `.claude/skills/tpr-review/SKILL.md` in full to understand the existing Step-by-Step structure.

- [x] Rewrite the skill file with the following structure:
  - Frontmatter: unchanged `name: tpr-review`, description updated to mention "dual-source codex + gemini" review
  - `## Step 0 — MANDATORY: Re-read CLAUDE.md` (preserved from existing)
  - `## ABSOLUTE: You May NEVER Reason Out of Findings` (preserved from existing)
  - `## ABSOLUTE: Correct Architectural Solutions Only` (preserved from existing)
  - `## When to Trigger` (preserved from existing)
  - `## Loop Protocol — MANDATORY` — updated to say "BOTH reviewers per round; round succeeds only when BOTH complete with zero actionable findings"
  - `## Steps (Per Iteration)` — rewritten:
    - Step 1: Create per-run scratch dir via `scratch-dir.sh`
    - Step 2: Write codex and gemini prompts (with the required `envelope-only` keyword for codex and `Activate the review-work skill...` preamble for gemini, per Section 03's transport.md convention)
    - Step 3: Invoke `dual-invoke-with-retry.sh` in background bash and wait for completion notification
    - Step 4: On success, read `$RUN/codex.envelope.json` and `$RUN/gemini.envelope.json`; invoke `merge-findings.py` to produce `$RUN/merged.json`
    - Step 5: Classify merged findings — agreement cases get both IDs shown adjacently; disagreements get single tags
    - Step 6: If zero actionable findings: clean pass, exit loop
    - Step 7: Otherwise, Claude fixes each finding (unchanged from existing logic), commits via `/commit-push`, re-runs from Step 1 with a fresh scratch dir
    - Step 8: After max iterations (10), surface remaining findings to user via `AskUserQuestion`
  - Failure handling: if `dual-invoke-with-retry.sh` exits non-zero, surface the failure category + `$RUN` path to the user via `AskUserQuestion`. Do NOT silently retry the semantic loop — infra failures are already handled inside the script.

- [x] In the new skill file, replace the old codex invocation block with the new dual-source block. Preserve the existing finding-fixing, commit, and re-run logic verbatim — that's the semantic loop, and it's reviewer-count-agnostic (it just fixes whatever findings came in, regardless of how many reviewers contributed them).

- [x] Verify the file compiles as markdown (no broken frontmatter, no unclosed code fences) by reading it back after writing.

- [x] **Subsection close-out (04.1)** — MANDATORY before starting 04.2:
  - [x] The new skill file exists, references Section 02's scripts correctly, and preserves the existing "fix and re-run" loop logic
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] Run `/improve-tooling` retrospectively — was the rewrite tedious (copy existing sections, graft new transport calls)? Should there be a `generate-wrapper-from-template.sh` helper that scaffolds a new dual-source wrapper given a skill name + loop semantics? Implement improvements NOW and commit separately.

---

## 04.2 Loop semantics, failure handling, and user escalation

**File(s):** `.claude/skills/tpr-review/SKILL.md` (augment loop sections from 04.1)

**Context:** The 10-iteration semantic loop is preserved from the existing skill but needs explicit documentation of how it interacts with Section 02's infra retry budget. Key invariant: infra failures consume retries inside `dual-invoke-with-retry.sh` (3 per reviewer per round, exponential backoff); semantic iterations only increment when a round completes successfully AND finds actionable findings that Claude then fixes. A round that fails infra retries exhaustively triggers user escalation, NOT a semantic iteration decrement.

Tasks:

- [ ] Document the loop interaction in the skill file with a clear state machine:
  ```
  iteration_counter = 0
  while iteration_counter < 10:
    RUN = scratch-dir.sh
    write prompts
    if dual-invoke-with-retry.sh fails:
      surface failure category + $RUN to user via AskUserQuestion
      EXIT (do NOT increment iteration_counter; do NOT retry semantic loop)
    else:
      parse both envelopes (already cached by transport)
      merge findings via merge-findings.py
      if zero actionable findings:
        CLEAN PASS — exit
      fix each actionable finding
      commit via /commit-push
      iteration_counter += 1
  # After 10 iterations:
  surface remaining findings to user via AskUserQuestion
  ```

- [ ] Add explicit escalation text for user escalation cases. The wrapper must tell the user:
  - What the failure category was (from the failure taxonomy)
  - Where the postmortem dir is (`$RUN`)
  - What files to inspect (`$RUN/codex.jsonl`, `$RUN/gemini.jsonl`, `$RUN/round.log`, `$RUN/*.parse-error`, `$RUN/worktree-error`)
  - What the user should do: triage the failure, then re-run `/tpr-review` (or ask Claude to retry)

- [ ] Add a section "## Merged Finding Format" to the skill file that shows how reviewer-tagged IDs appear in the plan TPR block:
  ```md
  - [ ] `[TPR-04-001-codex][high]` `compiler/foo.rs:123` — Add dec on early-exit branch.
    Evidence: ... Impact: ... Required plan update: ...
    Basis: fresh_verification. Agreement: [TPR-04-001-gemini] (both reviewers flagged this location/title)
  - [ ] `[TPR-04-001-gemini][high]` `compiler/foo.rs:123` — Add dec on early-exit branch.
    Evidence: ... Impact: ... Required plan update: ...
    Basis: direct_file_inspection. Agreement: [TPR-04-001-codex]. Citations: [https://doc.rust-lang.org/...]
  - [ ] `[TPR-04-002-gemini][medium]` `library/lib.ori:5` — Replace println with tracing::debug.
    Evidence: ... Impact: ... Required plan update: ...
    Basis: inference. (Gemini-only finding — no codex counterpart)
  ```

- [ ] **Subsection close-out (04.2)** — MANDATORY before starting 04.3:
  - [ ] Loop state machine and escalation text added to the skill file
  - [ ] Merged finding format documented with agreement and gemini-only examples
  - [ ] Update this subsection's `status` to `complete`
  - [ ] Run `/improve-tooling` retrospectively — was the state machine documentation helpful or over-engineered? Implement improvements.

---

## 04.3 Real TPR scenario validation (critical-path gate)

**File(s):** Validation only — no file changes beyond recording results

**Context:** This is the gate. Sections 05/06/07 are blocked until this subsection demonstrates the dual-source stack works against real TPR scenarios. Two scenarios minimum: one where both reviewers agree on at least one finding, and one where they disagree (either on a finding's existence or on severity/framing). If the gate fails, Section 02's transport gets fixed before this subsection retries.

Tasks:

- [ ] **Scenario 1 — Agreement demonstration**: Run `/tpr-review` against a real piece of work in the repo that contains a known subtle bug (e.g., an unrelated small issue that both reviewers are likely to catch). Verify:
  - Both reviewers produce findings
  - At least one `(location, title)` pair appears in both envelopes
  - The merged plan TPR block shows both `[TPR-NN-NNN-codex]` and `[TPR-NN-NNN-gemini]` entries adjacent, with `Agreement: [...]` annotation
  - The wall time is roughly `max(codex_walltime, gemini_walltime)` — verify by inspecting `$RUN/codex.walltime` and `$RUN/gemini.walltime`; the dual-invoke total should be close to the slower of the two, not the sum

- [ ] **Scenario 2 — Disagreement demonstration**: Run `/tpr-review` against a piece of work where the reviewers are likely to differ (e.g., a performance change where only gemini's grounded search can verify the claimed benchmark). Verify:
  - Both reviewers produce findings but with at least one finding from one reviewer that has no `(location, title)` match in the other
  - The merged plan TPR block shows the disagreement entries with single tags (no `Agreement:` annotation)
  - At least one gemini finding includes a `citations` array with a real source URL

- [ ] **Scenario 3 — Dirty-worktree guard test**: Manually craft a malicious test prompt that instructs a reviewer to modify a tracked file (e.g., "edit README.md and add a line"). Run it through the wrapper. Verify:
  - `worktree-guard.sh compare` returns non-zero after the reviewer run
  - The wrapper surfaces the failure with `dirty_worktree` category
  - The `$RUN/worktree-error` file contains the diff showing the offending modification
  - The user is prompted via `AskUserQuestion` with the diff

- [ ] **Scenario 4 — Infra retry fault injection**: Use a stub reviewer (shell alias or wrapper) that fails the first time and succeeds on the second. Run the wrapper. Verify:
  - `dual-invoke-with-retry.sh` retries with 1s backoff
  - On the second attempt, the reviewer succeeds
  - The round completes successfully
  - The `$RUN/round.log` records both attempts with timestamps

- [ ] Record the results of all four scenarios in the section's working notes. If any scenario fails, STOP — this is the gate. Fix the transport layer (Section 02) or the wrapper logic before marking Section 04 complete.

**CRITICAL PATH GATE (MUST PASS BEFORE SECTIONS 05/06/07 BEGIN):**

Sections 05, 06, 07 have explicit `depends_on: ["04"]` in their frontmatter. This gate enforces that dependency at the operational level — Section 04 is not "done" (and downstream sections must not start) until ALL of the following are demonstrated against real TPR scenarios:

1. **Scenario 1 (agreement demonstration) passes** — both reviewers produce findings and at least one `(location, title)` pair appears in both envelopes, with the merged plan TPR block showing adjacent `[TPR-NN-NNN-codex]` / `[TPR-NN-NNN-gemini]` entries
2. **Scenario 2 (disagreement demonstration) passes** — both reviewers produce findings with at least one unique to one reviewer, and at least one gemini finding includes a real `google_web_search` source URL in its `citations` array
3. **Scenario 3 (dirty-worktree guard test) passes** — the wrapper detects the deliberate source-file modification and surfaces the diff to the user
4. **Scenario 4 (infra retry fault injection) passes** — killing the reviewer subprocess once triggers a retry that succeeds

If any scenario fails, STOP. Do NOT mark Section 04 complete. Do NOT start Section 05/06/07. The failure is either a Section 02 transport bug (fix Section 02 and re-validate) or a wrapper bug in this section's 04.1/04.2 rewrite (fix here and re-validate). Transport bugs found here are the REASON this section exists as a validation gate — they're expected, they're valuable, and fixing them before propagation is the whole point of the canary release pattern.

- [ ] **Subsection close-out (04.3)** — MANDATORY before section completion:
  - [ ] All four validation scenarios pass
  - [ ] Scenario results documented in working notes
  - [ ] Update this subsection's `status` to `complete`
  - [ ] Run `/improve-tooling` retrospectively — the validation scenarios are currently manual; should there be a `validate-dual-tpr.sh` that runs all four as an automated test suite? Implement improvements.

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] All three subsections (04.1, 04.2, 04.3) marked `complete`
- [ ] `.claude/skills/tpr-review/SKILL.md` rewritten for dual-source; references Section 02 scripts correctly
- [ ] 10-iteration loop preserved; infra retries separate
- [ ] All four validation scenarios pass (agreement, disagreement, dirty-worktree, infra-retry)
- [ ] Merged plan TPR block shows reviewer-tagged IDs with independent ordinal sequences
- [ ] At least one gemini finding with `citations` demonstrated
- [ ] `timeout 150 ./test-all.sh` green
- [ ] Plan annotation cleanup: 0 annotations in source files
- [ ] **Plan sync**: Section 04 frontmatter → `complete`, 00-overview.md Quick Reference updated, mission criteria checkboxes updated, Section 05/06/07 `depends_on: ["04"]` satisfied
- [ ] `/tpr-review` passed — but note: this is now the DUAL-SOURCE `/tpr-review`, reviewing itself. This is the self-referential property flagged at plan start. The dual-source review of the dual-source rewrite is the strongest possible validation: if both reviewers agree on "this is clean," the pattern is proven.
- [ ] `/impl-hygiene-review` passed — after TPR clean
- [ ] `/improve-tooling` **section-close sweep** — MANDATORY. Verify per-subsection captures, look for cross-subsection patterns (validation scenario automation is the most likely cross-cutting finding). Implement immediately, commit separately. Document negative findings.

**Exit Criteria:** `.claude/skills/tpr-review/SKILL.md` runs dual-source reviews successfully against real TPR scenarios. The validation gate has passed with all four scenarios (agreement, disagreement, dirty-worktree, infra-retry). The transport from Section 02 is proven in production-like conditions. Sections 05, 06, 07 can now begin their wrapper rewrites against the same validated transport.
