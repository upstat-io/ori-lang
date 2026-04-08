---
section: "07"
title: "/tp-help dual-source + consolidation"
status: in-progress
reviewed: true
goal: "Rewrite .claude/skills/tp-help/SKILL.md for dual-source AND consolidate with .claude/commands/tp-help.md (resolving R10 SSOT violation). /tp-help uses CONCATENATION mode (not the findings envelope, not synthesis) — raw perspectives from both reviewers returned to the user. Requires raw-text parsers (parse-codex-raw.py, parse-gemini-raw.py) AND a minimal §02-owned API change: make `--schema` OPTIONAL in `.claude/skills/dual-tpr/scripts/dual-invoke.sh` (the variable is already dead code inside the script after BUG-08-003 removed `--output-schema` passthrough; the only reason it cannot be dropped in concat mode today is the required-flag check at line 40). This cross-section touch is intentional and co-owned per CLAUDE.md 'Plan boundaries = implementation boundaries' — §02's plan MUST be updated to reflect the schema-optional contract before §07.2 implementation begins. Verify that ALL THREE downstream consumers (/impl-hygiene-review Phase 4, /review-plan 4-agent pipeline, and /create-plan orchestrator) continue to work correctly with the dual-source concatenated response format. Own the `.claude/commands/review-plan.md` byte-identical regression contract (inherited from the removed Section 06) since §07.3 Scenario 2 already exercises the command file; capture a frozen baseline hash at §07.1 start so mid-section drift is catchable even if reverted before §07.N."
success_criteria:
  - ".claude/skills/tp-help/SKILL.md rewritten for dual-source using concatenation mode (not findings envelope)"
  - ".claude/commands/tp-help.md consolidated with the skill file as a thin pointer — single source of truth for /tp-help content (R10 resolved); frontmatter preserves the required fields (description, allowed-tools, argument-hint)"
  - "`.claude/skills/dual-tpr/scripts/dual-invoke.sh` makes `--schema` OPTIONAL (the variable is already dead code inside the script after BUG-08-003; the only reason it cannot be dropped in concat mode today is the mandatory-flag check at line 40). All existing callers that DO pass `--schema` continue to work unchanged (backward compatible)."
  - "New raw-mode parsers exist: parse-codex-raw.py and parse-gemini-raw.py (schema-less, sentinel-less; reusable for any concat-mode consumer)"
  - "Both reviewers' raw responses are concatenated into the output using HTML-comment sentinel attribution markers (e.g. `<!-- tp-help-reviewer: codex -->` / `<!-- /tp-help-reviewer: codex -->`) that CANNOT collide with Markdown H1/H2/H3 headers in downstream-consumer prose"
  - "Gemini prompt includes a read-only-reviewer preamble (HARD RULES: no file writes, no state-mutating shell commands, no git state changes) as the sole guardrail — there is no dedicated .gemini/skills/tp-help/ file"
  - "Inline worktree-guard check (git status --porcelain before/after) wired into the skill to catch prompt-discipline violations"
  - "ORI_TPR_REVIEWERS={codex|gemini|both} runtime toggle honored in dual-invoke.sh from §07.2's landing (moved from §08.2 via the §07.0 cross-section touch — see §07.0's §08 plan update). §08.2 is downgraded to verification-only for the toggle and retains the merge-findings.py single-reviewer work. One canonical location for the toggle wiring means no sibling script to keep in sync."
  - "/impl-hygiene-review Phase 4 cross-check still functions correctly under dual-source /tp-help — verified by §07.3 Scenario 1 integration test (lines 319-360, invocation prose at 327 and 344)"
  - "/review-plan 4-agent pipeline still functions correctly under dual-source /tp-help — verified by §07.3 Scenario 2 (Step 3B at line 95, Midpoint Check at line 305)"
  - "/create-plan orchestrator still functions correctly under dual-source /tp-help — verified by §07.3 Scenario 4 across all 4 internal call sites (Phase 1 lines 143 and 166, Phase 3 line 526, Step 8B line 584)"
  - "§07.3 uses a stub-binary test harness (`validate-tp-help-consumers.sh`, analogue of §04's `validate-dual-tpr.sh`) to exercise the downstream consumers against deterministic concatenation-mode outputs WITHOUT paying the 80-120 minute real-run penalty for every iteration, PLUS at least one real-run end-to-end per consumer at section close-out for full verification"
  - "At least 1 real /tp-help scenario runs successfully with both reviewers producing responses"
  - ".claude/commands/review-plan.md is BYTE-IDENTICAL to its pre-plan state — verified by `git diff --exit-code .claude/commands/review-plan.md` returning 0 in §07.N AND by comparison against the frozen §07.PRE baseline hash (section-07-review-plan-baseline.sha1, captured in the Section-Entry Preflight BEFORE §07.0). This contract moved into §07 on 2026-04-08 when Section 06 was removed; §07 is the natural owner because §07.3 Scenario 2 already runs `/review-plan` against the command file as part of the downstream-consumer integration test. The baseline capture lives in §07.PRE (not §07.1) so §07.0's script edits cannot accidentally mutate review-plan.md before the baseline lands."
  - "§07.PRE pre-files the `/create-plan` `--root`/`ORI_PLAN_ROOT` blocker via `/add-bug`; the assigned BUG-ID is recorded in `section-07-scenario4-blocker.txt` so §07.3 Scenario 4 can pick Mode A (preferred) when the bug is closed or Mode B (fallback) when it remains open. Mode B uses a deterministic slug under `plans/` with collision pre-check + exact-path cleanup."
depends_on: ["04"]
touches_sections: ["02", "08"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "07.PRE"
    title: "Section-Entry Preflight (baseline capture + sentinel uniqueness grep)"
    status: complete
  - id: "07.0"
    title: "Cross-section touch: make `dual-invoke.sh --schema` optional (updates §02 plan and script)"
    status: complete
  - id: "07.1"
    title: "Consolidate .claude/commands/tp-help.md with .claude/skills/tp-help/SKILL.md (R10)"
    status: complete
  - id: "07.2"
    title: "Rewrite for dual-source concatenation mode (not findings envelope)"
    status: complete
  - id: "07.3"
    title: "Verify downstream consumers still work with dual-source /tp-help"
    status: not-started
  - id: "07.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "07.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 07: /tp-help dual-source + consolidation

**Status:** Not Started
**Goal:** Apply dual-source to `/tp-help` with two twists: (1) `/tp-help` uses a LIGHTER response format (concatenation, not the findings envelope — per the Step 1E design decision that "when you're stuck and asking for help, you want raw perspectives from two models, not a smoothed merge"), and (2) this section ALSO consolidates the R10 SSOT violation where `.claude/commands/tp-help.md` (179 lines) duplicates `.claude/skills/tp-help/SKILL.md` (121 lines).

**Success Criteria:**

- [ ] `.claude/skills/tp-help/SKILL.md` rewritten for dual-source using concatenation mode — the output is BOTH reviewers' raw responses, clearly attributed via HTML-comment sentinel markers, NOT a findings envelope
- [ ] `.claude/commands/tp-help.md` consolidated with the skill file — the skill file is the source of truth, the command file is a thin pointer with frontmatter preserving the required slash-command loader fields (`description`, `allowed-tools`, `argument-hint`)
- [ ] R10 (the two-sources-of-truth SSOT violation) is resolved — verified by grepping for divergent content between the two files (there should be no divergence because one is now a pointer)
- [ ] `.claude/skills/dual-tpr/scripts/dual-invoke.sh` `--schema` flag is OPTIONAL (the $SCHEMA variable is already dead code after BUG-08-003 removed `--output-schema` passthrough — see lines 28/35/40 of the current script. The only blocker to concat-mode reuse today is the mandatory-flag check at line 40). This is the §02-owned change that §07.0 performs. §02's plan frontmatter is updated in the same edit pass to reflect the schema-optional contract.
- [ ] New concat-mode parsers exist and are exercised: `parse-codex-raw.py` and `parse-gemini-raw.py` in `.claude/skills/dual-tpr/scripts/` — these are raw-mode siblings of the envelope parsers (NOT `--raw` flags on the existing parsers, because the envelope parsers' validation logic is substantial and branching on `--raw` inside them would halve the test coverage for each mode)
- [ ] Gemini prompt includes a read-only-reviewer preamble because dual-source `/tp-help` has NO dedicated `.gemini/skills/tp-help/` file — the prompt IS the guardrail
- [ ] Inline worktree-guard check (before/after `git status --porcelain`) wired into the skill to catch any prompt-discipline violations at the skill level (the skill invokes `dual-invoke.sh` directly without the retry wrapper since concat mode is one-shot, so worktree-guard.sh cannot be inherited from `dual-invoke-with-retry.sh` — it is called inline from the skill)
- [ ] `ORI_TPR_REVIEWERS={codex|gemini|both}` runtime toggle honored in `dual-invoke.sh` from day one of §07.2 landing. Because §07 and §08 both touch `dual-invoke.sh`'s toggle wiring, §07.2 adds the toggle wiring (the toggle was previously scheduled for §08.2) and §08.2 is downgraded to verification-only. §08's plan is updated in the same §07.0 cross-section edit pass.
- [ ] Both reviewers' raw responses are included in the output with HTML-comment sentinel attribution markers: opening `<!-- tp-help-reviewer: codex -->` + content + closing `<!-- /tp-help-reviewer: codex -->` (and the same for gemini). These markers are HTML comments which renderers treat as invisible but which text-search tooling can locate unambiguously — they CANNOT collide with any Markdown header level. The §07.3 leakage check verifies the literal sentinels are NOT present in any written plan files after downstream-consumer runs
- [ ] `/impl-hygiene-review` Phase 4 cross-check (which invokes `/tp-help` internally — empirically verified at `.claude/skills/impl-hygiene-review/SKILL.md:319-360`; the `### Phase 4: Third-Party Cross-Check` header is on line 319, with invocation prose at line 327 under `#### 4a. Validate Findings` and line 344 under `#### 4b. Probe Blind Spots`) continues to work with dual-source `/tp-help` responses — the impl-hygiene-review receives both reviewers' feedback and can incorporate it with its existing `[TP-CONFIRMED]` and `[TP-SURFACED]` tagging
  - [ ] `.claude/commands/review-plan.md` internal `/tp-help` calls (lines 95 and 305 of the command file, under the "Step 3B: Third-Party Blind Spot Check via /tp-help" and "Midpoint Check: /tp-help Between Agent 2 and Agent 3" sections; invocation prose at lines 99 and 309) continue to work with dual-source `/tp-help` responses — regression test. The command file is promised to remain byte-identical (contract owned by this section — see the byte-identical success criterion above and the §07.N regression check), so these internal calls will start receiving concatenated codex+gemini output after Section 07 lands. Verify the command file's 4-agent pipeline still parses and incorporates the new dual-source response format without breakage; if it breaks, fix the incompatibility in 07.2's concatenation format (do NOT modify `.claude/commands/review-plan.md`).
  - [ ] `.claude/skills/create-plan/SKILL.md` internal `/tp-help` calls (lines 143 and 166 in Phase 1 research loop, line 526 in Phase 3 architectural sanity check, line 584 in Step 8B architecture sanity check — plus line 56 general rule and line 743 meta-reference) continue to work with dual-source `/tp-help` responses — regression test. create-plan is a THIRD downstream consumer. The dual-source concatenation format must be consumable as free-form prose by create-plan's orchestrator without leaking attribution headers into the final plan overview.
- [ ] At least 1 real `/tp-help` scenario runs successfully with a real question, both reviewers respond, and the concatenated output is returned to the user

**Context:** `/tp-help` is the only review skill in this plan that does NOT use the findings envelope schema. Per the Step 1E architectural decision, it uses concatenation: both reviewers' raw responses are returned adjacent with clear attribution, giving the user two independent perspectives without an editorial synthesis layer. This is because `/tp-help` is called when the user is stuck — they want raw opinions, not a smoothed consensus that might hide useful disagreement.

This section also resolves R10 (the SSOT violation between the command file and the skill file). Both currently exist and divergently describe the same `/tp-help` workflow. The consolidation picks ONE file as the source of truth and makes the other a thin pointer that references it. Per the Step 1E decision, the content belongs in the skill file (because that's where the auto-trigger behavior lives); the command file becomes a thin pointer.

The downstream consumer verification is important and THREE files are affected:

1. **`.claude/skills/impl-hygiene-review/SKILL.md`** calls `/tp-help` internally under the `### Phase 4: Third-Party Cross-Check` header. Empirically verified: Phase 4 is lines 319-360, with invocation prose at line 327 (under `#### 4a. Validate Findings`) and line 344 (under `#### 4b. Probe Blind Spots`); line 323 is the phase intro that announces the cross-check. Earlier drafts cited lines 291-308 and 322/343 — both are stale snapshots; the 319-360 range and 327/344 invocation lines are re-verified against the current file contents. When `/tp-help` becomes dual-source, that internal call starts receiving concatenated responses from two reviewers. The impl-hygiene-review wrapper needs to continue processing these responses correctly, tagging findings with `[TP-CONFIRMED]` / `[TP-SURFACED]` as it already does. No changes to impl-hygiene-review are needed — just verification that the change doesn't break it.

2. **`.claude/commands/review-plan.md`** (the 595-line 4-agent Claude pipeline — the only Claude-side `/review-plan` entrypoint in the plan, which this section leaves byte-identical) ALSO calls `/tp-help` internally: once at line 95 under "Step 3B: Third-Party Blind Spot Check via /tp-help" (invocation prose at line 99: "call `/tp-help` to identify blind spots...") and once at line 305 under "Midpoint Check: /tp-help Between Agent 2 and Agent 3" (invocation prose at line 309: "call `/tp-help` for a midpoint structural check..."). These internal calls will start receiving concatenated codex+gemini responses after this section lands. Because the command file is promised to remain byte-identical (the byte-identical contract is owned by this section — inherited from the removed Section 06 on 2026-04-08, see success criteria and §07.N), we CANNOT modify the command file to adapt its parsing; the concatenation format must be backward-compatible with whatever text-parsing logic the command file already uses. If the command file relies on single-source response assumptions, we either (a) adjust the concatenation format in 07.2 to preserve backward compatibility, or (b) escalate to the user as a scope expansion decision.

3. **`.claude/skills/create-plan/SKILL.md`** (the 1149-line create-plan orchestrator) calls `/tp-help` at MANY sites across its workflow — this is the third downstream consumer. The empirically-verified call sites are: line 56 (general sequencing rule — "All `/tp-help` and `/tpr-review` invocations MUST run in the foreground"), line 132 (Phase 1 research loop — "Every `/tp-help` call in this loop MUST run in the foreground"), line 143 ("Build a `/tp-help` prompt that includes"), line 166 ("Call `/tp-help` again with"), line 524 (Phase 3 architectural sanity check — "This `/tp-help` call MUST run in the foreground"), line 526 ("call `/tp-help` to get a second opinion"), line 528 ("Build a `/tp-help` prompt that includes"), line 580 (Step 8B header "Architecture Sanity Check via /tp-help"), line 582 ("SEQUENTIAL & FOREGROUND — MANDATORY"), line 584 ("call `/tp-help` to sanity-check"), line 586 ("Build a `/tp-help` prompt that includes"), and line 743 (meta-reference noting that `/review-plan` itself calls `/tp-help`). The create-plan orchestrator dispatches `/tp-help` via foreground bash and consumes its output as prose context for downstream decisions. When `/tp-help` becomes dual-source, create-plan's many call sites will start receiving doubled concatenated output. If create-plan's downstream reasoning assumes a single-source text shape, it will silently degrade — concatenation-mode output must remain coherent when consumed as free-form prose by a planning orchestrator that does NOT parse envelopes. This is a THIRD regression test in §07.3.

Three downstream consumers × one format change → four scenarios in §07.3: Scenario 1 (impl-hygiene-review), Scenario 2 (review-plan command file), Scenario 3 (byte-identity re-check for the command file after Scenario 2), and Scenario 4 (create-plan orchestrator). Scenarios 1, 2, and 4 exercise a distinct downstream consumer; Scenario 3 is the byte-identity guard that must pass after Scenario 2 runs.

**Reference implementations:**
- Existing `.claude/skills/tp-help/SKILL.md` (121 lines) and `.claude/commands/tp-help.md` (179 lines) — the two files being consolidated
- `.claude/skills/impl-hygiene-review/SKILL.md:319-360` — Phase 4 Third-Party Cross-Check, the downstream consumer that invokes `/tp-help` (invocation prose at lines 327 (4a Validate Findings) and 344 (4b Probe Blind Spots); line 323 is the phase intro)
- `.claude/commands/review-plan.md:95-135` (Step 3B) and `:305-340` (Midpoint Check) — the 595-line 4-agent Claude pipeline downstream consumer
- `.claude/skills/create-plan/SKILL.md` — the third downstream consumer (lines 132, 143, 166 in Phase 1 research loop; lines 524-528 in Phase 3 architectural sanity check; lines 580-586 in Step 8B architecture sanity check; line 56 general rule; line 743 meta-reference to `/review-plan` which itself calls `/tp-help`)
- Section 04's dual-source transport pattern — the same transport scripts, different output mode

**Depends on:** Section 04 (validated dual-source pattern).

---

## 07.PRE Section-Entry Preflight (MANDATORY — runs BEFORE §07.0)

**Context:** The frozen baseline hash for `.claude/commands/review-plan.md` MUST be captured BEFORE any §07 subsection edits ANY file. If §07.0 has a bug that accidentally touches `.claude/commands/review-plan.md` before the baseline lands, the baseline would be captured against drifted content and the byte-identical contract would silently break.

This preflight block is the literal first thing that runs when §07 starts. All §07.X subsections treat these baselines as invariants. Any violation at a subsection boundary is a hard stop.

Tasks (run IN ORDER, before §07.0 task 1):

- [x] **Capture the frozen `.claude/commands/review-plan.md` baseline — BEFORE ANY OTHER §07 WORK.** Run from the plan working dir (`plans/dual-tpr-gemini/`):
  ```bash
  git hash-object .claude/commands/review-plan.md > section-07-review-plan-baseline.sha1
  git rev-parse HEAD                               >> section-07-review-plan-baseline.sha1
  ```
  The first line is the blob hash of the command file at section start; the second line is the commit the baseline was captured against (for postmortem if the baseline file is ever compared against the wrong tree). Any edit that touches `.claude/commands/review-plan.md` between now and §07.N MUST compare the current blob hash against this baseline and fail the section if they differ. Re-run this comparison after every `git add`/`git commit` inside §07. The §07.N regression check alone is too late — it catches drift, it doesn't prevent it.

- [x] **Verify the baseline was captured BEFORE any §07 subsection started.** The working directory must match `HEAD` at capture time (no §07.0 edits yet applied). If the working dir is dirty with §07.0 changes, STOP — revert the §07.0 changes, capture the baseline, then re-apply §07.0.

- [x] **Attribution-sentinel uniqueness preflight.** Before any §07.2 work lands the sentinel strings, verify NO pre-existing occurrence of the sentinel exists anywhere in the repo outside plan text:
  ```bash
  # Expected: zero matches outside plans/ (plan docs MAY cite the sentinel as reference)
  rg 'tp-help-reviewer' -l -g '!plans/' || echo "OK: no pre-existing sentinel matches outside plans/"
  ```
  If ANY match surfaces outside `plans/`, the sentinel string `tp-help-reviewer` has a collision risk. Pick a more unique sentinel (e.g., add a longer GUID-like discriminator) and update §07.2's Attribution format decision + Steps tasks to use the new sentinel. Record the decision in §07.R.

- [x] **Pre-file the §07.3 Scenario 4 Mode A blocker bug** — `/create-plan` currently has NO `--root` / `ORI_PLAN_ROOT` / `plan_root` override (empirically verified against `.claude/skills/create-plan/SKILL.md`). Scenario 4's preferred Mode A execution path requires that override to redirect plan creation into a tmpdir; without it, Scenario 4 falls back to Mode B (deterministic slug under `plans/` with collision pre-check + exact-path cleanup). To prevent §07.3 from stalling on a missing prerequisite, file the blocker NOW via `/add-bug`:
  ```
  /add-bug create-plan: add --root/ORI_PLAN_ROOT override for test harnesses
    severity: high
    repro: /create-plan currently writes plan files unconditionally under plans/<slug>/.
           No env var or flag exists to redirect output for test harnesses.
    impact: blocks dual-tpr-gemini §07.3 Scenario 4 Mode A (the preferred mode);
            forces fallback to Mode B which writes a slug under plans/ that
            must be cleaned by exact path with no safety net beyond a slug
            collision pre-check.
    suggested fix: add ORI_PLAN_ROOT env var (or --root flag) that shadows the
                   plans/ prefix used by create-plan's directory creation step
                   (see .claude/skills/create-plan/SKILL.md Step 10 "Create
                   Directory Structure" around line 618).
    surfaced by: dual-tpr-gemini §07.PRE preflight, pre-filed before §07.3
                 Scenario 4 to avoid mid-section stall.
  ```
  Record the assigned BUG-ID into a §07-local file:
  ```bash
  echo "BUG-NN-NNN" > section-07-scenario4-blocker.txt   # replace BUG-NN-NNN with the assigned ID
  ```
  This file is consumed by §07.3 Scenario 4's Mode A/B decision. Filing the bug here is NOT deferral — it creates a tracked artifact that `/review-bugs` will triage independently. If the bug is fixed before §07.3 starts, Scenario 4 uses Mode A; if still open, Scenario 4 uses Mode B.

- [x] **Preflight close-out** — MANDATORY before starting §07.0:
  - [x] `section-07-review-plan-baseline.sha1` exists and contains two lines (hash `66250250e8030a5e880ceaf4bf40f9409178a375` + HEAD `f775e98c049662abf4b023f499cb3bf3bd278ad4`)
  - [x] Working directory is clean or only contains unrelated changes (no §07 edits yet)
  - [x] Sentinel uniqueness grep returned zero non-plan matches (0 matches outside `plans/dual-tpr-gemini/`)
  - [x] `section-07-scenario4-blocker.txt` exists and contains the BUG-ID for the create-plan root-override prerequisite (BUG-08-010, filed in `plans/bug-tracker/section-08-spec-docs.md`)

---

## 07.0 Cross-section touch: make `dual-invoke.sh --schema` optional (updates §02 plan and script)

**File(s):** `.claude/skills/dual-tpr/scripts/dual-invoke.sh` (small change), `plans/dual-tpr-gemini/section-02-transport.md` (plan sync), `plans/dual-tpr-gemini/section-08-integration-cleanup.md` (plan sync — reflect that §07.2 now performs the ORI_TPR_REVIEWERS wiring that §08.2 was originally scheduled to do)

**Context:** An earlier draft of this section proposed creating a sibling script `dual-invoke-concat.sh` to avoid modifying §02's transport API. Closer reading revealed a load-bearing inaccuracy in that rationale: the `$SCHEMA` variable is already **dead code** inside `dual-invoke.sh`. BUG-08-003 removed the `--output-schema` passthrough to codex (see the comment block at lines 82-94 of the current script). The variable is collected at line 28, parsed at line 35, required at line 40 — and then **never referenced again**. The only reason concat-mode consumers cannot call `dual-invoke.sh` today is the mandatory-flag check at line 40 asserting `-z "$SCHEMA"` is false.

The architectural tradeoff:

| Option | Cost | Correctness |
|---|---|---|
| **A (new)**: Make `--schema` optional in `dual-invoke.sh` — 2-line change | 1 line removed from line-40 check, 1 line of comment added; §02's plan must be updated to reflect the schema-optional contract (cross-section touch) | ✓ Correct — removes dead weight, no duplication |
| **B**: Pass a placeholder schema value in concat mode | 1 line of plan-caller disguise | ✗ Architectural lie — callers declare a dependency they don't have |
| **C (rejected)**: Sibling `dual-invoke-concat.sh` (~100 lines) | Permanent duplication of subshell pattern, trap, wait-both, ORI_TPR_REVIEWERS branching, BUG-08-004/BUG-08-005/TPR-04-002-gemini concurrency fixes. Both scripts must stay in lock-step forever. | ✗ Permanent drift risk; doubles the surface area for concurrency bugs |

Per CLAUDE.md §"The One Rule: Correctness Above All" — "When you see two possible fixes — one simpler and one more correct — the simpler one does not exist... If the correct fix requires architectural change, that IS the work." The correct fix (Option A) is also the smallest: make `--schema` optional. Option C (sibling script) was initially considered but rejected on closer reading: while `$SCHEMA` is "never referenced after line 35," the mandatory check at line 40 is ALSO dead weight since the variable has no live use. Per CLAUDE.md §"Plan boundaries = implementation boundaries": the §02 plan and script must be updated together, and §02's `status: complete` does not block the touch — it means the §02 plan frontmatter must be updated in the same edit.

This §07.0 subsection is the dedicated place where the cross-section touch happens. It lands FIRST, before §07.1 and §07.2, so later subsections can assume the schema-optional launcher exists.

Tasks:

- [x] **Modify `.claude/skills/dual-tpr/scripts/dual-invoke.sh`** to make `--schema` optional:
  1. Leave `SCHEMA=""` initialization at line 28 (preserves arg-parse symmetry for backward-compatible callers).
  2. Leave the `--schema)` case at line 35 (backward-compatible — existing callers still parse their arg).
  3. Edit the required-flag check at line 40: **remove `|| -z "$SCHEMA"`** from the `[[ ... ]]` expression and update the usage string to `usage: dual-invoke.sh --run DIR --skill NAME --codex-prompt FILE --gemini-prompt FILE [--schema FILE]` (square brackets indicate optional).
  4. Add a 2-line comment block immediately above the check explaining why `--schema` is optional (BUG-08-003 removed `--output-schema` passthrough; the flag is preserved for caller-signature backward compatibility but is not consumed).
  5. Run `bash -n .claude/skills/dual-tpr/scripts/dual-invoke.sh` to verify syntax.

- [x] **Verify backward compatibility — 4-cell test matrix**. The schema-optional change is a flag-parsing contract change; backward compat requires ALL of the following cells pass. Add the matrix to `transport-tests.sh` (as a new `--test-only schema_optional` category) so the regression is permanent, not a one-shot manual check:
  ```bash
  # Cell 1 — old caller path: --schema FILE present, must still parse and reach launch
  bash -n .claude/skills/dual-tpr/scripts/dual-invoke.sh
  RUN=$(mktemp -d) ; printf 'p\n' > "$RUN/c.md" ; printf 'p\n' > "$RUN/g.md"
  # Stub the codex/gemini binaries via PATH so the launch doesn't actually spawn real CLIs
  PATH="$(dirname "$0")/../fixtures/stub-bin-tp-help:$PATH" \
    bash .claude/skills/dual-tpr/scripts/dual-invoke.sh \
      --run "$RUN" --skill test --codex-prompt "$RUN/c.md" --gemini-prompt "$RUN/g.md" \
      --schema .claude/skills/dual-tpr/findings-schema.json
  # Cell 1 PASS: round.log shows "dual-invoke start"; both .exit files exist; exit 0
  rm -rf "$RUN"

  # Cell 2 — new caller path: --schema OMITTED, must parse and reach launch
  RUN=$(mktemp -d) ; printf 'p\n' > "$RUN/c.md" ; printf 'p\n' > "$RUN/g.md"
  PATH="$(dirname "$0")/../fixtures/stub-bin-tp-help:$PATH" \
    bash .claude/skills/dual-tpr/scripts/dual-invoke.sh \
      --run "$RUN" --skill test --codex-prompt "$RUN/c.md" --gemini-prompt "$RUN/g.md"
  # Cell 2 PASS: same shape as Cell 1 — round.log shows "dual-invoke start", both .exit files exist
  rm -rf "$RUN"

  # Cell 3 — round.log invariant: Cell 1 and Cell 2 produce the SAME log entries
  # (the schema flag must not appear in any log line; if it does, dead-code drift remains)
  ! grep -q -- '--schema' "$RUN/round.log"

  # Cell 4 — dual-invoke-with-retry.sh wrapper still extracts --schema at its own arg parse
  # (the wrapper has its own --schema handling; this proves the wrapper-side contract is intact)
  grep -q -- '--schema' .claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh
  ```
  These 4 cells together pin: (a) backward compat for old callers, (b) forward compat for concat-mode callers, (c) round.log content invariant, (d) wrapper-script contract preserved. Add them as a permanent test category in `transport-tests.sh`. **Failing tests first** (per CLAUDE.md TDD): if Cell 2 passes BEFORE the line-40 edit lands, the precondition is wrong — investigate. The expected pre-edit state is Cell 1 PASS, Cell 2 FAIL (with the explicit `usage:` error), Cell 3 PASS (vacuously — `--schema` is in the usage error not round.log), Cell 4 PASS. Post-edit: all 4 PASS.

- [x] **Confirm §02 precondition (plan-sync edits were applied during the pre-implementation review on 2026-04-08).** This is a PRECONDITION CHECK — not a verify-then-re-apply loop. At §07.0 implementation time, read §02's frontmatter and body ONCE and confirm the three preconditions hold. If any precondition fails, STOP and escalate to the user (do NOT silently re-apply — those edits were already reviewed and a mismatch signals something went wrong between the review and implementation):
  1. The `success_criteria` block contains a criterion describing the `--schema` optional contract.
  2. §02.1's body has a "NOTE (2026-04-08...)" amendment near the top referencing this §07.0 touch.
  3. §02's `status: complete` is preserved (the amendment is a metadata correction, not a reopening).

  If the preconditions hold (expected case), check this task off and proceed. If they do NOT hold, escalate: something in the plan drifted between the review and §07.0 implementation — use `AskUserQuestion` to report the drift and get a decision before proceeding.

- [x] **Confirm §08 precondition (the §08.2 downgrade is still present).** NOTE (2026-04-08): precondition check found a minor drift — §08.2's frontmatter `sections:` entry title at line 26 still said "Wire ORI_TPR_REVIEWERS runtime toggle" while the §08.2 body header at line 124 correctly said "Verify ORI_TPR_REVIEWERS runtime toggle + merger single-reviewer case". Agent 3 updated the body header during the pre-implementation review but missed the frontmatter entry in the same pass. Fixed inline as part of §07.0's cross-section touch (with user approval via AskUserQuestion). No escalation needed — the drift was a trivial miss, not a "review vs implementation" semantic divergence. Same pattern — precondition check, not verify-then-reapply:
  1. §08.2's title is "Verify ORI_TPR_REVIEWERS runtime toggle + merger single-reviewer case" (not "Wire ORI_TPR_REVIEWERS runtime toggle").
  2. §08.2's context block contains the "Wiring-scope update (2026-04-08, via §07.0 cross-section touch)" note.
  3. §08.2's task list has `dual-invoke.sh` verification tasks (NOT implementation tasks) and retains the `merge-findings.py` single-reviewer update.

  If any precondition fails, escalate via `AskUserQuestion`. Do not silently re-apply.

- [x] **Subsection close-out (07.0)** — MANDATORY before starting 07.1:
  - [x] `dual-invoke.sh` `--schema` is optional; `bash -n` passes (verified after edit)
  - [x] `transport-tests.sh --test-only schema_optional` passes all 4 cells (old caller path with `--schema`, new caller path without, round.log invariant, retry-wrapper contract). Full suite 23/23 PASS, no regressions.
  - [x] §02's plan frontmatter (schema-optional success criterion) and §02.1 body (NOTE block) confirmed present from Agent 2's pre-implementation review; precondition check PASS.
  - [x] §08.2's task list downgrade confirmed; §08.2 frontmatter subsection title drift fixed inline.
  - [x] Update this subsection's `status` to `complete`
  - [x] Run `/improve-tooling` retrospectively — addressed TWO concrete items:
    1. **[IMPLEMENTED 2026-04-08]** **`lint-transport-contract.sh` for dead script args.** Created `.claude/skills/dual-tpr/scripts/lint-transport-contract.sh` (113 lines) — a static dead-arg linter that parses `case "$1" in --xxx) VAR=...` patterns from transport scripts, strips comments via `sed 's/#.*$//'` (critical — prose mentions of `$VAR` in comment blocks would otherwise hide a truly dead variable, as almost happened during §07.0's own implementation), and reports each arg as LIVE, KNOWN-DEAD (with `# lint-transport-contract: known-dead <reason>` annotation), or DEAD (UNANNOTATED). Supports `--check` mode for CI enforcement. First run correctly identified `$SCHEMA` as DEAD in `dual-invoke.sh` — the linter immediately validated its own purpose by catching the exact bug class that inspired it. Annotated `--schema)` in `dual-invoke.sh` with `# lint-transport-contract: known-dead BUG-08-003 (--output-schema removed, flag kept for caller backward compat)`. `dual-invoke-with-retry.sh` uses a for-loop arg-parse pattern the linter does not recognize; it prints `(no case-pattern args found — scanner does not cover this script)` transparently so the coverage limitation is visible. Future transport scripts that use the recognized pattern get automatic coverage. The §02 transport plan said "schema required" at a time when the schema was genuinely validated by codex's Structured Outputs API. BUG-08-003 made it dead code, and nothing detected the drift for multiple sections. Implement a tiny bash-static-analysis linter at `.claude/skills/dual-tpr/scripts/lint-transport-contract.sh` that: (a) parses `dual-invoke.sh` + `dual-invoke-with-retry.sh` arg declarations (the `--xxx)` case statements), (b) greps the body for references to each arg variable, (c) flags any arg whose variable is assigned but never referenced outside the assignment + required-flag check. **Trigger to implement NOW (not defer)**: the $SCHEMA dead-code drift went undetected for 2+ sections — this is exactly the class of bug a dead-arg linter catches. Per "ALWAYS improve tooling", the linter IS the work. File and implement inside §07.0's retrospective slot.
    2. **[DECIDED 2026-04-08 — update §02 style rule]** **`dual-invoke.sh` line count.** §02 has a "each script ≤ 100 lines target" style rule (§02.1 line 103). `dual-invoke.sh` is currently 170 lines and §07.0 + §07.2 add ~15 more (ORI_TPR_REVIEWERS wiring, optional schema comment). The 100-line target was retrospectively accepted to drift by §02.4's retrospective (see §02.4 line 886). Re-evaluate at §07.0 close-out: if the script has grown past 190 lines with no clean extraction candidate, formally UPDATE the §02 style-rule target to 200 lines (with rationale: the concurrency fixes from BUG-08-004/005 + TPR-04-002-gemini + the ORI_TPR_REVIEWERS wiring are all load-bearing and cannot be extracted without adding an indirection layer that the retrospectives have already rejected). Alternatively, extract a `reviewer-launch.sh` helper that encapsulates the per-reviewer subshell + trap + wait pattern (one helper, called twice with `codex` / `gemini` arguments). The extraction is a medium-size refactor; do not attempt it during §07.0 unless the retrospective explicitly decides to. Record the decision either way.

---

## 07.1 Consolidate .claude/commands/tp-help.md with .claude/skills/tp-help/SKILL.md (R10)

**File(s):** `.claude/commands/tp-help.md` (rewrite as thin pointer), `.claude/skills/tp-help/SKILL.md` (becomes canonical source)

**Context:** The two files currently divergently describe the same workflow. The skill file is smaller (121 lines) and cleaner; the command file (179 lines) has more aggressive auto-trigger documentation but the core workflow is the same. Consolidation: make the skill file the canonical source (incorporating the useful auto-trigger documentation from the command file), and reduce the command file to a thin pointer.

Tasks:

- [x] **Re-verify the frozen baseline from §07.PRE is still intact at §07.1 start.** The baseline was captured in the Section-Entry Preflight (before §07.0), not here. Before any §07.1 file edits, run:
  ```bash
  current=$(git hash-object .claude/commands/review-plan.md)
  baseline=$(head -1 section-07-review-plan-baseline.sha1)
  if [[ "$current" != "$baseline" ]]; then
    echo "FAIL: review-plan.md drifted between §07.PRE and §07.1 start (expected $baseline, got $current)" >&2
    exit 1
  fi
  ```
  If this fails, §07.0 accidentally touched `.claude/commands/review-plan.md`. Revert §07.0's edits immediately, investigate the root cause, and re-run §07.PRE before restarting.

- [x] Read both files in full to identify divergence points.

- [x] Update `.claude/skills/tp-help/SKILL.md` to be the canonical source. **This step is CONSOLIDATION ONLY — do NOT yet rewrite the workflow for dual-source. 07.2 handles the dual-source rewrite as a separate edit pass so any breakage in the consolidation can be diagnosed without confounding it with transport changes.**
  - Incorporate the aggressive auto-trigger documentation from the command file (concrete trigger conditions, example scenarios that MUST trigger auto-invoke)
  - Add explicit mention that this file is the canonical source for `/tp-help` workflow content
  - Preserve the existing single-source codex workflow verbatim — 07.2 rewrites it for dual-source
  - **Preserve valid frontmatter** in the new skill file: the YAML frontmatter MUST have at minimum `name:` and `description:` (the existing skill file already has both; Claude Code's skill loader requires both). Verify after the edit that the frontmatter is valid YAML and the loader accepts it (run `/tp-help` and confirm no frontmatter-parse error in stderr).

- [x] Rewrite `.claude/commands/tp-help.md` as a thin pointer. **The frontmatter MUST preserve all fields the Claude Code slash-command loader requires** — at minimum `description`, `allowed-tools`, and `argument-hint` (the existing command file has all three; a thin pointer that drops any of them may fail to register as a slash command). Do NOT drop the `name` field — keep it for parity with the existing file even though the loader may infer it from the filename. Suggested thin-pointer shape:
  ```markdown
  ---
  name: tp-help
  description: Get third-party help from Codex CLI + Gemini CLI (dual-source). Use this proactively when stuck on a problem, unsure about an implementation approach, want a second opinion on code you just wrote, need help debugging, or want to verify your reasoning.
  allowed-tools: Bash, Read, Grep, Glob
  argument-hint: "[question or context]"
  ---

  # /tp-help — Third-Party Help

  The canonical implementation of `/tp-help` lives in the skill file at
  `.claude/skills/tp-help/SKILL.md`. When the `/tp-help` slash command
  is invoked, load and follow that skill file exactly.

  See `.claude/skills/tp-help/SKILL.md` for:
  - Auto-trigger conditions
  - Workflow (prompt construction, dual-source transport invocation, response handling)
  - Output format (concatenated reviewer responses with attribution)
  - Failure handling
  ```

- [x] Verify R10 is resolved: grep for any divergent implementation content between the two files. There should be no operational detail duplicated — only the thin pointer in `commands/tp-help.md` and the canonical implementation in `skills/tp-help/SKILL.md`.
  ```bash
  wc -l .claude/commands/tp-help.md .claude/skills/tp-help/SKILL.md
  # Expected: commands/tp-help.md is ~25 lines (thin pointer), skills/tp-help/SKILL.md is the canonical source
  ```

- [x] **Post-consolidation smoke test matrix (before 07.2's dual-source rewrite)**: the thin-pointer rewrite can break `/tp-help` dispatch independently of any transport changes, so we MUST verify the consolidation is wired correctly BEFORE 07.2 changes the output format. The matrix dimensions are: (invocation style) × (caller path) × (frontmatter validation). Cells 1-2 are direct invocations exercising shell-escaping paths; cells 3-5 are dispatch-only grep checks for the THREE downstream callers (impl-hygiene-review, review-plan command file, create-plan); cell 6 is YAML frontmatter validation. Cells 3-5 are intentionally DISPATCH-ONLY (not full skill runs) to avoid 30-90 min wall time and avoid the write side effects of running `/impl-hygiene-review` / `/review-plan` / `/create-plan` inside §07.1. Full REAL integration runs happen in §07.3 with disposable-target + cleanup discipline. Run ALL of the following and require every cell to pass:

  **Cell 1 — Trivial direct invocation:** `/tp-help what is 2+2` — verify the command reaches the canonical skill file and a codex response is returned.

  **Cell 2 — Multi-line prompt path:** Build a realistic prompt (a paragraph of context + a specific question + a code snippet, total ~40 lines of markdown including fenced code blocks). Invoke `/tp-help "<the full multi-line prompt>"`. Verify:
  - The shell quoting survives (no premature termination at embedded quotes or backticks)
  - The prompt file written to `/tmp/tp-help-prompt.md` (or equivalent per-run scratch file) contains the full multi-line content
  - Codex returns a response that demonstrates it read the full prompt, not a truncated prefix
  - Multi-line path is the most common real-world usage — a trivial one-liner would not have caught this

  **Cell 3 — Dispatch-only check for `impl-hygiene-review` Phase 4** (a full `/impl-hygiene-review` run is 30-45 min and writes to the plan/bug tracker; we only need to verify the `/tp-help` dispatch path, not a full hygiene pass). Instead of running the whole skill, verify dispatch with a minimal synthetic harness:
  ```bash
  bash -c '
    set -e
    # The Claude Code slash-command loader resolves /tp-help via the command file.
    # Verify the thin-pointer body references the canonical skill file by path
    # (anywhere in the body, not pinned to start-of-line — the thin-pointer
    # template wraps the path in backticks inside a prose sentence).
    grep -qF ".claude/skills/tp-help/SKILL.md" .claude/commands/tp-help.md \
      && echo "Cell 3a: thin-pointer references canonical skill file"
    # Verify the canonical skill file exists and is readable.
    test -r .claude/skills/tp-help/SKILL.md \
      && echo "Cell 3b: canonical skill file readable"
    # Verify impl-hygiene-review Phase 4 prose still cites /tp-help (no drift).
    count=$(grep -c "/tp-help" .claude/skills/impl-hygiene-review/SKILL.md)
    [[ "$count" -ge 3 ]] && echo "Cell 3c: impl-hygiene-review Phase 4 still references /tp-help ($count times)"
  '
  ```
  If any of the three Cell 3 checks fails, the thin-pointer path is broken for internal callers even if direct invocation works. The full `/impl-hygiene-review` REAL integration test runs in §07.3 Scenario 1 (with disposable scope + cleanup discipline); §07.1 only verifies dispatch.

  **Cell 4 — Dispatch-only check for `.claude/commands/review-plan.md` Step 3B + Midpoint Check** (the full 4-agent pipeline is 60-90 min and `/review-plan` WRITES findings to section TPR blocks, which would mutate any "completed" plan used as the test target; dispatch-only is safe and fast):
  ```bash
  bash -c '
    set -e
    # Verify review-plan.md still cites /tp-help at the expected positions (the
    # file is byte-identical by contract per §07.PRE baseline, so the grep just
    # confirms the plan text still references /tp-help at Step 3B + Midpoint).
    grep -q "Step 3B: Third-Party Blind Spot Check via /tp-help" .claude/commands/review-plan.md \
      && echo "Cell 4a: Step 3B header present"
    grep -q "Midpoint Check: /tp-help Between Agent 2 and Agent 3" .claude/commands/review-plan.md \
      && echo "Cell 4b: Midpoint Check header present"
    # Baseline invariant re-check: review-plan.md has NOT been touched by §07.1
    current=$(git hash-object .claude/commands/review-plan.md)
    baseline=$(head -1 section-07-review-plan-baseline.sha1)
    [[ "$current" == "$baseline" ]] && echo "Cell 4c: baseline invariant holds"
  '
  ```
  If Cell 4c fails, §07.1 accidentally touched `review-plan.md` — revert and investigate. The full `/review-plan` REAL integration test runs in §07.3 Scenario 2 (with disposable test target + cleanup discipline); §07.1 only verifies dispatch + baseline invariant.

  **Cell 5 — Dispatch-only check for `create-plan`** (`/create-plan` writes an entire plan directory under `plans/`, which would pollute the repo tree; dispatch-only grep is safe):
  ```bash
  bash -c '
    set -e
    # Verify create-plan still cites /tp-help at all known call sites.
    count=$(grep -c "/tp-help" .claude/skills/create-plan/SKILL.md)
    [[ "$count" -ge 10 ]] && echo "Cell 5a: create-plan cites /tp-help $count times"
  '
  ```
  The full create-plan REAL integration test runs in §07.3 Scenario 4 (with throwaway tmpdir + cleanup discipline); §07.1 only verifies dispatch.

  **Cell 6 — Frontmatter preservation check:** After the rewrite, run:
  ```bash
  python3 -c "
  import yaml, sys
  with open('.claude/commands/tp-help.md') as f:
      first = f.read()
  if not first.startswith('---\n'):
      print('MISSING frontmatter fence'); sys.exit(1)
  end = first.find('\n---\n', 4)
  if end == -1:
      print('UNCLOSED frontmatter'); sys.exit(1)
  fm = yaml.safe_load(first[4:end])
  required = {'description', 'allowed-tools', 'argument-hint'}
  missing = required - set(fm.keys())
  if missing:
      print(f'MISSING fields: {missing}'); sys.exit(1)
  print('OK:', list(fm.keys()))
  "
  ```
  Fails if the rewrite dropped any required field. `name` is recommended but not strictly required (the loader infers it from the filename). The three REQUIRED fields are `description`, `allowed-tools`, and `argument-hint` — empirically verified from the existing file's frontmatter.

  If ANY of these cells fails, STOP. The consolidation itself is broken — fix it before starting 07.2. Mixing consolidation bugs with dual-source rewrite bugs produces unbounded debugging. The six cells land in sequence: consolidate, smoke-test all six cells, then rewrite.

- [x] **Post-smoke-test baseline re-check:** After all six smoke-test cells pass, re-run the baseline hash check:
  ```bash
  current=$(git hash-object .claude/commands/review-plan.md)
  baseline=$(head -1 section-07-review-plan-baseline.sha1)
  if [[ "$current" != "$baseline" ]]; then
    echo "FAIL: review-plan.md drifted during §07.1 (expected $baseline, got $current)" >&2
    exit 1
  fi
  ```
  This is a trip-wire — if ANY smoke-test cell accidentally wrote to `review-plan.md` via a reviewer hallucination (the byte-identical contract would be violated), the hash check catches it immediately and before §07.2 starts.

- [x] **Subsection close-out (07.1)** — MANDATORY before starting 07.2:
  - [x] §07.PRE baseline re-verified at §07.1 start AND again at §07.1 close (both checks passed: `git hash-object .claude/commands/review-plan.md` matches `head -1 section-07-review-plan-baseline.sha1` = `66250250e8030a5e880ceaf4bf40f9409178a375`)
  - [x] Consolidation done: `.claude/skills/tp-help/SKILL.md` (179 lines) is canonical; `.claude/commands/tp-help.md` (24 lines) is a thin pointer. Net savings: 300 → 203 lines (-97 lines, -32%).
  - [x] Frontmatter on the thin-pointer command file passes Cell 6 YAML validation: `name`, `description`, `allowed-tools`, `argument-hint` all present and well-formed.
  - [x] R10 resolved — `grep` confirms no operational workflow content (`codex exec`, `python3`, `Step N:`, `run_in_background`) in `.claude/commands/tp-help.md`.
  - [x] All six smoke-test cells passed: (1) trivial invocation via codex — response "2 + 2 = 4", (2) multi-line prompt via codex — 40-line prompt with fenced code block survived shell quoting, codex confirmed Rust snippet validity, (3) impl-hygiene-review cites `/tp-help` 7 times (dispatch-only grep), (4) review-plan.md has Step 3B + Midpoint Check headers + baseline invariant holds, (5) create-plan cites `/tp-help` 12 times, (6) frontmatter YAML validation PASS for both files. Note: cells 1+2 were verified via a combined codex call using a scratch `mktemp -d` to avoid the shared `/tmp/tp-help-*` collision risk that §07.2 will fix permanently via per-run scratch dirs.
  - [x] Post-smoke-test baseline re-check passed (`review-plan.md` hash unchanged — blob `66250250e8030a5e880ceaf4bf40f9409178a375` still matches baseline).
  - [x] Update this subsection's `status` to `complete`
  - [x] Run `/improve-tooling` retrospectively — answered via dedicated bug file. Analysis: only 2 command-skill pairs exist (`tp-help` uses thin-pointer consolidation; `review-work` uses intentionally-parallel workflows per 00-overview line 32 — command file = Claude self-reviews directly, skill file = dual-source codex wrapper, both canonical for different use cases). A general `lint-command-skill-pairs.sh` would need to classify each pair by pattern (thin-pointer vs parallel-workflow) BEFORE validating, which is more design work than §07.1's retrospective should absorb. Filed as BUG-08-011 in `plans/bug-tracker/section-08-spec-docs.md` for future implementation. Recorded in §07-local retrospective notes: the thin-pointer pattern worked cleanly for `tp-help`; no tooling gap was hit during §07.1 that a linter would have caught.

---

## 07.2 Rewrite for dual-source concatenation mode (not findings envelope)

**File(s):** `.claude/skills/tp-help/SKILL.md` (continue from 07.1's edit), `.claude/skills/dual-tpr/scripts/parse-codex-raw.py` (new), `.claude/skills/dual-tpr/scripts/parse-gemini-raw.py` (new), `.claude/skills/dual-tpr/scripts/dual-invoke.sh` (add ORI_TPR_REVIEWERS wiring — §07.0 already made `--schema` optional, so no further launcher changes are needed for concat-mode reuse)

**Context:** `/tp-help` does NOT use the findings envelope — it uses concatenation. Both reviewers are still launched in parallel via the existing `dual-invoke.sh` (now with optional `--schema` per §07.0), but the parsing is simpler: extract the reviewer's entire response text (not just a JSON envelope), and concatenate both responses with HTML-comment sentinel attribution markers that cannot collide with consumer-side Markdown.

The codex-side parsing: extract the final `agent_message.text` as raw text (no JSON parsing, no schema validation — it's a response, not findings). The gemini-side parsing: concatenate all `delta: true` assistant message fragments in arrival order (same as the parser from Section 02.3), then extract the text WITHOUT sentinel extraction (there are no BEGIN-ORI-DUAL-TPR-V1 sentinels because there's no JSON envelope).

**Transport reuse:** §07.0 made `dual-invoke.sh --schema` optional, so `/tp-help` invokes the existing transport script directly — no sibling launcher, no duplication of the subshell/trap/wait-both pattern, no second copy of the BUG-08-004/BUG-08-005/TPR-04-002-gemini concurrency fixes to keep in lock-step. The only `/tp-help`-specific additions at the transport layer are the two raw-mode parsers. `/tp-help` deliberately skips the retry wrapper (`dual-invoke-with-retry.sh`) because concat mode is one-shot — infra failures surface directly to the user without retry. This means `/tp-help` does NOT inherit `worktree-guard.sh` (which lives in the retry wrapper); instead, the skill file performs its own inline worktree-guard check via `git status --porcelain` snapshots before/after the `dual-invoke.sh` call.

**Attribution format decision:** Attribution uses HTML comment sentinels, NOT Markdown H2 headers. Rationale: Markdown H1/H2/H3 headers can collide with consumer-side prose (`.claude/commands/review-plan.md` has its own `### Step 3B` / `#### 4a` / `### Phase 4` headers that structure the 4-agent pipeline, and `.claude/skills/create-plan/SKILL.md` has numerous `## Phase N` and `### Step N` headers that structure the orchestrator). If `/tp-help` emits `## Codex response`, any downstream consumer that renders the tp-help output inline into its own prose would have a heading-level collision that could break TOC generation, heading-based parsers, or simple visual scanning. HTML comments are invisible to Markdown renderers but preserve perfect search/parse semantics for any consumer that wants to strip attribution at a later processing stage.

Canonical attribution format:
```
<!-- tp-help-reviewer: codex -->
<codex raw response text>
<!-- /tp-help-reviewer: codex -->

<!-- tp-help-reviewer: gemini -->
<gemini raw response text>
<!-- /tp-help-reviewer: gemini -->
```

Consumers that want human-visible labels MAY add a single prose line immediately after the opening sentinel (e.g., `**Codex says:**`), but the sentinels themselves are the authoritative machine-readable attribution. The §07.3 leakage check greps downstream-consumer outputs for the literal strings `<!-- tp-help-reviewer: codex -->` and `<!-- /tp-help-reviewer: codex -->` (and the gemini equivalents) — if they appear in a downstream-written plan file, attribution is leaking and §07.2 must tighten the consumer-side prose integration in `/tp-help`'s skill file (e.g., the skill could be instructed to strip attribution sentinels before returning the final concatenated text in some contexts).

Tasks:

- [x] **Wire `ORI_TPR_REVIEWERS` toggle into `dual-invoke.sh`** (moved from §08.2 — see §07.0's §08 plan update). Modify `.claude/skills/dual-tpr/scripts/dual-invoke.sh` to read `$ORI_TPR_REVIEWERS` immediately after arg-parse and branch the launch block:
  ```bash
  REVIEWERS="${ORI_TPR_REVIEWERS:-both}"
  if [[ "$REVIEWERS" != "codex" && "$REVIEWERS" != "gemini" && "$REVIEWERS" != "both" ]]; then
    echo "invalid ORI_TPR_REVIEWERS: $REVIEWERS (must be codex|gemini|both)" >&2
    exit 2
  fi
  ```
  Then gate the codex background launch behind `[[ "$REVIEWERS" == "codex" || "$REVIEWERS" == "both" ]]` and the gemini background launch behind `[[ "$REVIEWERS" == "gemini" || "$REVIEWERS" == "both" ]]`. The wait-both logic must also be gated — only wait on PIDs that were actually launched (use empty-PID checks: `[[ -n "$CODEX_PID" ]] && wait "$CODEX_PID"`). The cleanup trap must similarly skip empty PIDs.

  Verify backward compatibility: existing callers that do NOT set `ORI_TPR_REVIEWERS` see `REVIEWERS=both` (the default), which matches pre-toggle behavior exactly. The `unset ORI_TPR_REVIEWERS` → `REVIEWERS=both` fallback is the critical backward-compat invariant; every existing §04/§05 wrapper MUST continue to launch both reviewers without modification.

  Test matrix (run after the wiring lands):
  ```bash
  # Default (unset) = both
  unset ORI_TPR_REVIEWERS
  bash -n .claude/skills/dual-tpr/scripts/dual-invoke.sh

  # codex-only path
  ORI_TPR_REVIEWERS=codex bash -n .claude/skills/dual-tpr/scripts/dual-invoke.sh

  # gemini-only path
  ORI_TPR_REVIEWERS=gemini bash -n .claude/skills/dual-tpr/scripts/dual-invoke.sh

  # Invalid value rejected with exit 2
  ORI_TPR_REVIEWERS=invalid bash .claude/skills/dual-tpr/scripts/dual-invoke.sh --run /tmp --skill x --codex-prompt /tmp/x --gemini-prompt /tmp/x
  ```

- [x] **Update `dual-invoke-with-retry.sh` to skip parsing for reviewers that weren't launched.** When `ORI_TPR_REVIEWERS=codex`, only parse `$RUN/codex.jsonl`; gemini's envelope is absent and the merger step handles the single-reviewer case. When `ORI_TPR_REVIEWERS=gemini`, mirror the inverse. This wiring was originally scheduled for §08.2 and is now moved into §07.2 along with the `dual-invoke.sh` wiring above — §08.2 retains the `merge-findings.py` single-reviewer update (see §07.0's §08 plan update).

- [x] **Write `.claude/skills/dual-tpr/scripts/parse-codex-raw.py`** — a tiny parser that reads `$RUN/codex.jsonl` and emits the final `agent_message.text` to stdout (no JSON envelope extraction, no schema validation). The existing `parse-codex.py` from §02 cannot be reused: it takes a mandatory `--schema` arg, parses `agent_message.text` as JSON, validates against the schema via `jsonschema`, applies `envelope_invariants.py` semantic checks, and emits the envelope. None of that applies to concat mode — the `agent_message.text` IS the final answer in raw prose. A `--raw` flag on `parse-codex.py` would be worse than a sibling because it would branch across validation/emission logic in the most complex parser in the pipeline, doubling the test surface for each mode. Sibling parser: Structure:
  ```python
  #!/usr/bin/env python3
  # parse-codex-raw.py — extract raw agent_message text from codex JSONL.
  # Usage: parse-codex-raw.py --jsonl PATH
  # Emits: the LAST agent_message.text on stdout (or exits non-zero if none found).
  import argparse, json, sys
  ap = argparse.ArgumentParser()
  ap.add_argument("--jsonl", required=True)
  args = ap.parse_args()
  last = None
  with open(args.jsonl) as f:
      for line in f:
          line = line.strip()
          if not line: continue
          try:
              obj = json.loads(line)
          except json.JSONDecodeError:
              continue
          if obj.get('type') == 'item.completed' and obj.get('item', {}).get('type') == 'agent_message':
              last = obj['item'].get('text')
  if last is None:
      print("missing_agent_message", file=sys.stderr)
      sys.exit(1)
  print(last)
  ```

- [x] **Write `.claude/skills/dual-tpr/scripts/parse-gemini-raw.py`** — a tiny parser that reads `$RUN/gemini.jsonl` and emits the concatenated assistant content to stdout (NO sentinel extraction — there are no sentinels in concat mode). Reuses the delta-concatenation logic from §02's `parse-gemini.py` but skips the BEGIN/END sentinel extraction step. Structure:
  ```python
  #!/usr/bin/env python3
  # parse-gemini-raw.py — extract concatenated assistant content from gemini stream-json.
  # Usage: parse-gemini-raw.py --jsonl PATH
  # Emits: the concatenated delta:true assistant content on stdout.
  # Waits for a terminal result(status=success) event; exits non-zero on missing
  # terminator, missing assistant content, or parse errors.
  import argparse, json, sys
  ap = argparse.ArgumentParser()
  ap.add_argument("--jsonl", required=True)
  args = ap.parse_args()
  chunks = []
  terminated = False
  with open(args.jsonl) as f:
      for line in f:
          line = line.strip()
          if not line: continue
          try:
              obj = json.loads(line)
          except json.JSONDecodeError:
              print("parse_fail", file=sys.stderr); sys.exit(1)
          t = obj.get('type')
          if t == 'message' and obj.get('role') == 'assistant' and obj.get('delta', False):
              chunks.append(obj.get('content', ''))
          elif t == 'result' and obj.get('status') == 'success':
              terminated = True
              break
  if not terminated:
      print("missing_terminator", file=sys.stderr); sys.exit(1)
  if not chunks:
      print("missing_assistant_content", file=sys.stderr); sys.exit(1)
  sys.stdout.write(''.join(chunks))
  ```

- [x] **Parser unit-test fixtures and matrix — TDD discipline (write tests FIRST, then implement parsers).** Per CLAUDE.md TDD: tests precede code. Create the fixture matrix BEFORE the parser code lands. Fixtures live in `.claude/skills/dual-tpr/fixtures/raw-mode/` (sibling of the existing `fixtures/` dir from §02). Test runner extends `transport-tests.sh` with a new category `raw_parsers`.

  **parse-codex-raw.py fixture matrix** (codex JSONL inputs → expected stdout):
  | Cell | Fixture | Input shape | Expected stdout | Expected exit |
  |---|---|---|---|---|
  | C1 | `codex-raw-happy.jsonl` | one `item.completed` with `agent_message.text="hello world"` | `hello world\n` | 0 |
  | C2 | `codex-raw-multi-message.jsonl` | three `item.completed` agent_message events with text "first", "second", "third" | `third\n` (LAST one wins per parser contract) | 0 |
  | C3 | `codex-raw-empty.jsonl` | empty file | `` (no stdout) + `missing_agent_message` on stderr | 1 |
  | C4 | `codex-raw-malformed.jsonl` | line `not-json` then a valid `item.completed` | valid agent_message text on stdout (parser must skip malformed lines, not abort) | 0 |
  | C5 | `codex-raw-no-agent-message.jsonl` | several non-agent_message events (item.started, tool_call) but no agent_message | `missing_agent_message` on stderr | 1 |
  | C6 | `codex-raw-truncated.jsonl` | one valid agent_message followed by a half-line (no trailing newline + incomplete JSON) | text on stdout (parser must tolerate truncation at the tail) | 0 |

  **parse-gemini-raw.py fixture matrix** (gemini stream-json inputs → expected stdout):
  | Cell | Fixture | Input shape | Expected stdout | Expected exit |
  |---|---|---|---|---|
  | G1 | `gemini-raw-happy.jsonl` | three `delta:true` chunks "hello ", "world", "!" then `result status=success` | `hello world!` | 0 |
  | G2 | `gemini-raw-single-chunk.jsonl` | one chunk "complete answer" then `result status=success` | `complete answer` | 0 |
  | G3 | `gemini-raw-no-terminator.jsonl` | three valid chunks but NO `result` event | `` + `missing_terminator` on stderr | 1 |
  | G4 | `gemini-raw-empty-content.jsonl` | `result status=success` only (no chunks) | `` + `missing_assistant_content` on stderr | 1 |
  | G5 | `gemini-raw-malformed.jsonl` | invalid JSON line followed by valid chunks | `parse_fail` on stderr | 1 |
  | G6 | `gemini-raw-non-delta.jsonl` | `delta:false` messages mixed with `delta:true` chunks | only `delta:true` chunks concatenated | 0 |
  | G7 | `gemini-raw-result-failure.jsonl` | chunks then `result status=failure` (NOT success) | `` + `missing_terminator` on stderr (loop never sees a `success` terminator) | 1 |

  Test runner shape (add to `transport-tests.sh` under `--test-only raw_parsers`):
  ```bash
  raw_parsers_test() {
    local script="$1" fixture="$2" expected_stdout="$3" expected_stderr="$4" expected_rc="$5"
    local got_stdout got_stderr got_rc
    got_stdout=$("$script" --jsonl "$fixture" 2>/tmp/stderr.$$) ; got_rc=$?
    got_stderr=$(cat /tmp/stderr.$$) ; rm -f /tmp/stderr.$$
    [[ "$got_stdout" == "$expected_stdout" ]] && \
    [[ "$got_stderr" == *"$expected_stderr"* ]] && \
    [[ "$got_rc" == "$expected_rc" ]]
  }
  # Cells C1-C6 + G1-G7 invoked one per call; PASS/FAIL accumulated.
  ```

  **TDD ordering (per CLAUDE.md)**:
  1. Create fixture files first (this is the spec — pin behavior in fixtures, not in parser source comments).
  2. Add `raw_parsers` test category to `transport-tests.sh` calling the placeholder parsers (which don't exist yet).
  3. Run `transport-tests.sh --test-only raw_parsers` — ALL 13 cells must FAIL with "script not found" (NOT spurious-pass; if any cell passes, the harness is broken).
  4. Implement `parse-codex-raw.py` (Cells C1-C6 should pass after this).
  5. Implement `parse-gemini-raw.py` (Cells G1-G7 should pass after this).
  6. Re-run `transport-tests.sh --test-only raw_parsers` — all 13 cells must PASS.
  7. **Semantic pin**: at least one cell per parser MUST be a behavior that ONLY passes with the new parser semantics (e.g., Cell C2 — "last agent_message wins" — would fail if the parser concatenated all messages or returned the first). Cell C2 is the codex semantic pin; Cell G1 (multi-chunk concatenation in arrival order) is the gemini semantic pin.
  8. **Negative pin**: at least one cell per parser MUST reject the broken behavior (Cell C5 — "no agent_message → exit 1" — proves the parser doesn't silently fall through; Cell G3 — "no terminator → exit 1" — proves the parser doesn't accept truncated streams as success).

- [x] **Update the skill file's Steps section** to use `dual-invoke.sh` directly (NOT `dual-invoke-with-retry.sh`) with concatenation-mode parsing and HTML-comment attribution:
  1. Snapshot the worktree state via `git status --porcelain > "$RUN/worktree.before"` (inline worktree-guard START — the skill file is the guardrail in concat mode because `dual-invoke.sh` itself doesn't run the guard; the retry wrapper is the normal home of `worktree-guard.sh`, but concat mode deliberately skips the retry wrapper)
  2. Create per-run scratch dir via `scratch-dir.sh`
  3. Write codex prompt (no `envelope-only` keyword needed — `/tp-help` doesn't have plan-write mode for codex to avoid)
  4. Write gemini prompt with the read-only-reviewer preamble (see next task)
  5. Invoke `dual-invoke.sh` in background bash (NOT `dual-invoke-with-retry.sh` — tp-help is one-shot concat mode, infra failure surfaces directly to the user without retry). Pass the scratch dir, `--skill tp-help`, and both prompt files. **Do NOT pass `--schema`** — §07.0 made the flag optional, and passing a schema in concat mode would be architecturally misleading (there is no envelope to validate). Wait for the completion notification.
  6. Parse both outputs as RAW TEXT via the new parsers: `parse-codex-raw.py --jsonl $RUN/codex.jsonl` and `parse-gemini-raw.py --jsonl $RUN/gemini.jsonl`
  7. Concatenate with HTML-comment sentinel attribution (NOT H2 headers — see the Attribution format decision in the §07.2 context block):
     ```
     <!-- tp-help-reviewer: codex -->
     <codex raw text>
     <!-- /tp-help-reviewer: codex -->

     <!-- tp-help-reviewer: gemini -->
     <gemini raw text>
     <!-- /tp-help-reviewer: gemini -->
     ```
  8. Snapshot the worktree state again via `git status --porcelain > "$RUN/worktree.after"` and `diff` the before/after files. If they differ, surface the diff to the user and fail the invocation — either reviewer violated prompt discipline (inline worktree-guard END).
  9. Return the concatenated output to the user. On launch failure or parser failure from either reviewer, surface the failure and both partial outputs to the user (do NOT silently drop one side).

- [x] **Gemini prompt discipline — MANDATORY.** Since dual-source `/tp-help` has NO dedicated gemini skill file under `.gemini/skills/` (unlike `/review-work` and `/review-plan` which each got one in §03), gemini is invoked as a generic assistant with `--approval-mode yolo`. Its only guardrail is the prompt text. The gemini prompt MUST include the following read-only-reviewer preamble before the user's question, to preserve prompt-discipline parity with the dedicated gemini reviewer skills in §03:
  ```
  You are being consulted for a third-party opinion on a specific problem.

  HARD RULES — DO NOT VIOLATE:
  - DO NOT modify any source files. You have NO permission to edit, create, or delete files.
  - DO NOT run shell commands that mutate state. You MAY run read-only commands for verification: `grep`, `rg`, `find`, `cat`, `head`, `tail`, `git log`, `git diff`, `git blame`, `git show`, `git status`.
  - DO NOT run build commands, test commands, or anything that touches the working tree (no `cargo build`, `cargo test`, `./test-all.sh`, `npm`, `pnpm`, `pip install`, `mv`, `cp`, `rm`, `touch`, `mkdir`, `>`, `>>`, etc.).
  - DO NOT commit, push, pull, checkout, reset, stash, or otherwise touch git state.
  - Your ONLY job is to read the context, reason about it, and return your opinion as free-form prose to stdout.

  This is a third-party consultation, not an autonomous task. Prompt discipline violations are tracked.

  ---
  ```
  The codex-side prompt does NOT need this preamble because codex runs under `--full-auto` with the worktree guard from §02 catching any drift. Gemini has no equivalent guard at launch time, so the prompt IS the guard.

- [x] **Prompt-discipline verification:** After writing the skill file, add a post-run assertion in the skill's Steps (already described in steps 1 + 8 of the Steps rewrite task above) that checks `git status --porcelain` BEFORE and AFTER the `/tp-help` invocation. If either reviewer modified the working tree, report the diff and fail the invocation. This mirrors §02's `worktree-guard.sh` snapshot/compare pattern but is inlined into the skill because `/tp-help` invokes `dual-invoke.sh` directly (NOT via `dual-invoke-with-retry.sh` which is where the retry wrapper normally composes worktree-guard.sh into the pipeline). The inline check is cheap (two `git status --porcelain` calls) and catches the "gemini ignored the prompt preamble" failure mode at exactly one layer above the launcher.

- [x] Note that dual-source tp-help does NOT have a dedicated gemini skill file — it uses gemini as a generic assistant governed by the read-only-reviewer preamble above. This is consistent with the concatenation mode: no envelope = no schema = no activation ceremony. The gemini prompt is the read-only preamble + the user's question + the codex-side prompt context.

- [x] Update auto-trigger documentation in the skill file to mention that dual-source tp-help is ~10x slower than codex-only tp-help due to gemini's wall time. Document the `ORI_TPR_REVIEWERS=codex` escape hatch for cases where the user wants the faster single-source version. NOTE: The runtime toggle wiring in `dual-invoke.sh` is IN SCOPE for §07.2 (moved from §08.2 — see the first two task bullets of this subsection). Because there is no sibling launcher to keep in sync, the `ORI_TPR_REVIEWERS` branching lives in exactly ONE place (`dual-invoke.sh`) and ALL four wrappers (including `/tp-help`) honor it uniformly.

- [x] **Subsection close-out (07.2)** — MANDATORY before starting 07.3:
  - [x] `dual-invoke.sh` has `ORI_TPR_REVIEWERS` branching wired; `bash -n` passes; all three values + unset default verified
  - [x] `dual-invoke-with-retry.sh` skips parsing for reviewers that weren't launched
  - [x] `parse-codex-raw.py` and `parse-gemini-raw.py` exist, are executable, and the full 13-cell `raw_parsers` test category in `transport-tests.sh` passes (6 codex cells + 7 gemini cells, including semantic pin C2 and negative pins C5/G3/G7)
  - [x] Skill file rewrite done; /tp-help returns concatenated dual-source output via HTML-comment sentinel attribution
  - [x] Attribution format is HTML-comment sentinels, NOT H2 headers (explicit negative test: `grep -c '^## Codex response' "$output"` returns 0; `grep -c '<!-- tp-help-reviewer: codex -->' "$output"` returns ≥1)
  - [x] Gemini prompt includes the read-only-reviewer preamble
  - [x] Inline worktree-guard check (step 1 and step 8 of the Steps rewrite above) wired into the skill
  - [x] `ORI_TPR_REVIEWERS={codex|gemini|both}` toggle honored in `dual-invoke.sh` and skips parsing in `dual-invoke-with-retry.sh` for un-launched reviewers
  - [x] §02 and §08 plan files updated in §07.0 still reflect the schema-optional and toggle-moved contracts — re-read them after §07.2 lands to confirm no drift
  - [x] Update this subsection's `status` to `complete`
  - [x] Run `/improve-tooling` retrospectively — was the HTML-comment attribution format clear? Should attribution be richer (include wall time per reviewer inside the sentinel block)? Was the inline worktree-guard duplication painful (diff against `worktree-guard.sh` for the N-th time — should we extract a `worktree-guard-inline.sh` library that both the retry wrapper and tp-help can source)? If yes, implement now. **Retrospective 07.2**: TDD-first fixture matrix + dedicated `raw_parsers_cell` helper proved highly effective — all 13 cells passed on first parser implementation with zero rework. No tooling gaps surfaced for THIS subsection: attribution format was clear (sentinels preserve perfect grep semantics while being invisible to renderers), worktree-guard inlining is small (2x `git status --porcelain` + `diff`) and duplicating it into the skill is cheaper than building a `worktree-guard-inline.sh` library that would have to ship its own activation contract. Deferred: if §07.3's real integration tests reveal that real reviewers violate the preamble with any frequency, revisit the library extraction then. Commit in this subsection is the per-subsection artifact.

---

## 07.3 Verify downstream consumers still work with dual-source /tp-help

**File(s):** Validation only (NO modifications to `.claude/skills/impl-hygiene-review/SKILL.md`, NO modifications to `.claude/commands/review-plan.md`, NO modifications to `.claude/skills/create-plan/SKILL.md`)

**Context:** THREE downstream consumers invoke `/tp-help` internally. When `/tp-help` becomes dual-source, all three of those internal call sets start receiving concatenated responses from two reviewers. None of the three downstream consumers can be modified — `impl-hygiene-review` is out-of-scope for modification in this section, `.claude/commands/review-plan.md` is byte-identical by contract (owned by this section — §07 inherited the byte-identical regression guard on 2026-04-08 when the originally-planned Section 06 was removed as redundant with §07's dual-source `/tp-help`), and `create-plan` is out-of-scope for modification in this section (§08.3 owns the line 56 wording update, and even that is a surgical 1-line touch, not a workflow rewrite). This subsection verifies ALL THREE consumers continue to work correctly with the new response format.

Downstream consumer inventory (empirically verified against file contents):

1. **`.claude/skills/impl-hygiene-review/SKILL.md`** — invokes `/tp-help` under `### Phase 4: Third-Party Cross-Check` (lines 319-360). The phase header is on line 319; the invocation prose is at line 327 (`#### 4a. Validate Findings`) and line 344 (`#### 4b. Probe Blind Spots`). Line 323 announces the phase; lines 330, 347, and 353 are example prompt templates shown to the user, not actual call sites. The wrapper's existing logic tags confirmed findings with `[TP-CONFIRMED]` and surfaces findings with `[TP-SURFACED]` (per line 364).

2. **`.claude/commands/review-plan.md`** — invokes `/tp-help` twice: once at line 95 under `### Step 3B: Third-Party Blind Spot Check via /tp-help` (blind-spot check before the 4 review agents launch; invocation prose at line 99: "call `/tp-help` to identify blind spots the review should focus on") and once at line 305 under `#### Midpoint Check: /tp-help Between Agent 2 and Agent 3` (invocation prose at line 309: "call `/tp-help` for a midpoint structural check before the executability and testing passes"). Both calls are SEQUENTIAL and FOREGROUND by explicit contract in the command file (lines 97 and 307; also line 578 general rule).

3. **`.claude/skills/create-plan/SKILL.md`** — the 1149-line create-plan orchestrator invokes `/tp-help` from FIVE distinct workflow positions (plus supporting rules/references). Empirically verified call-site list:
   - Line 56: general sequencing rule ("All `/tp-help` and `/tpr-review` invocations MUST run in the foreground (NOT `run_in_background`). MUST wait for each to complete and read its output before proceeding.")
   - Line 132: Phase 1 research loop enforcement ("Every `/tp-help` call in this loop MUST run in the foreground")
   - Line 143: Phase 1 prompt construction ("Build a `/tp-help` prompt that includes")
   - Line 166: Phase 1 refinement call ("Call `/tp-help` again with")
   - Line 524: Phase 3 architectural sanity check header + enforcement ("This `/tp-help` call MUST run in the foreground")
   - Line 526: Phase 3 actual call ("After ALL research passes complete, call `/tp-help` to get a second opinion on the architectural direction before committing to it")
   - Line 528: Phase 3 prompt construction ("Build a `/tp-help` prompt that includes")
   - Line 580: Step 8B header ("### Step 8B: Architecture Sanity Check via /tp-help")
   - Line 582: Step 8B enforcement ("This `/tp-help` call MUST run in the foreground")
   - Line 584: Step 8B actual call ("Before presenting to the user, call `/tp-help` to sanity-check the written overview architecture")
   - Line 586: Step 8B prompt construction ("Build a `/tp-help` prompt that includes")
   - Line 743: meta-reference to `/review-plan`'s internal `/tp-help` calls ("The `/review-plan` skill internally calls `/tp-help` multiple times")

Three downstream consumers × one format change = four scenarios in §07.3 (one per consumer + a dedicated byte-identity re-check for the command file after Scenario 2 runs).

Tasks:

- [ ] Read `.claude/skills/impl-hygiene-review/SKILL.md` lines 319-360 to understand exactly how it calls `/tp-help` and processes the response.

- [ ] Read `.claude/commands/review-plan.md` lines 95-135 (Step 3B) and lines 305-340 (Midpoint Check) to understand how the command file builds its `/tp-help` prompts and consumes the responses.

- [ ] Read `.claude/skills/create-plan/SKILL.md` lines 56, 130-170 (Phase 1 research loop), 520-540 (Phase 3 architectural sanity check), and 578-600 (Step 8B architecture sanity check) to understand how create-plan builds and consumes `/tp-help` responses at each call site. Note the common pattern: create-plan calls `/tp-help` in the foreground, waits for the full response, then includes the response text (or a summarized extract of it) as context for the next research pass or the next workflow phase. Because §07.2 uses HTML-comment sentinel attribution (NOT H2 headers), the concerns around heading-level collision are avoided BY CONSTRUCTION — the sentinels cannot appear as Markdown structure. Scenario 4 still verifies end-to-end: it must confirm (a) create-plan's prose rendering correctly integrates the raw reviewer text, (b) no HTML-comment sentinels leak into the final written plan document, and (c) no doubled-text or truncation artifacts appear.

- [ ] **Write `.claude/skills/dual-tpr/scripts/validate-tp-help-consumers.sh`** — a stub-binary test harness (Concern 1 resolution). Rationale: a real end-to-end run of all four downstream scenarios with `ORI_TPR_REVIEWERS=both` costs ~80-120 minutes per iteration; running Scenario 4 (four `/create-plan` tp-help calls) alone at ~10-15 min per call takes 40-60 min. Iterating on the attribution format, the sentinel strings, or the leakage detector in that budget is prohibitively slow. This harness mirrors the precedent set by §04's `validate-dual-tpr.sh` — stub `codex` and `gemini` binaries under `$SCRIPT_DIR/../fixtures/stub-bin-tp-help/`, use PATH manipulation to make the transport scripts launch the stubs instead of the real CLIs, and assert on the stubbed outputs. The stubs emit deterministic concatenation-mode outputs with known content so consumer-side leakage and integration can be verified without paying the real wall-time cost.

  Stub binary contract (both `codex` and `gemini`):
  - Accept any args (both real CLIs have rich arg surfaces; the stubs ignore them)
  - Emit a deterministic JSONL stream matching the format of the real CLI:
    - `codex` stub: one `item.completed` event with `item.type=agent_message` and `item.text=<deterministic codex response>`
    - `gemini` stub: a sequence of `{"type":"message","role":"assistant","content":"<chunk>","delta":true}` events followed by a terminal `{"type":"result","status":"success"}` event
  - The deterministic response content MUST include the literal string `STUB_CODEX_RESPONSE_MARKER` (or `STUB_GEMINI_RESPONSE_MARKER`) so downstream leakage detection can grep for it in any written consumer outputs
  - Stubs exit 0 immediately (no wall-time cost)

  Harness scenarios (mirror Scenarios 1-4 below but use the stubs). Each stub scenario MUST assert specific outputs (positive + negative pins), NOT just "no error":

  1. **Stub Scenario 1 — `/impl-hygiene-review` Phase 4 with stubbed tp-help**:
     - Positive pin: `STUB_CODEX_RESPONSE_MARKER` present in the Phase 4 cross-check intermediate output (proves codex stub reached the call site)
     - Positive pin: `STUB_GEMINI_RESPONSE_MARKER` present in the same output (proves gemini stub reached the call site — this is the dual-source pin: a single-source `/tp-help` would only show ONE marker)
     - Positive pin: at least one finding tagged `[TP-CONFIRMED]` or `[TP-SURFACED]` in the merged findings list (proves the impl-hygiene-review wrapper processed the prose, not just received it)
     - Negative pin: NO `<!-- tp-help-reviewer:` sentinel literal in `.claude/skills/impl-hygiene-review/SKILL.md` after the stub run (proves the stub didn't write to the consumer file — the file must remain unchanged)

  2. **Stub Scenario 2 — `/review-plan` on a tiny fixture plan with stubbed tp-help**:
     - Positive pin: `STUB_CODEX_RESPONSE_MARKER` and `STUB_GEMINI_RESPONSE_MARKER` both appear in the Step 3B blind-spot output captured to scratch
     - Positive pin: same dual-marker assertion at the Midpoint Check between Agents 2 and 3
     - Positive pin: 4-agent pipeline runs to completion (each agent's exit code is 0; no parse errors in the wrapper output)
     - Negative pin: `git diff --exit-code .claude/commands/review-plan.md` returns 0 (byte-identical contract intact)
     - Negative pin: NO `<!-- tp-help-reviewer:` literal sentinel in any plan file under the fixture plan dir (proves no leakage into written plan output)

  3. **Stub Scenario 3 — byte-identity guard**: identical to live Scenario 3 (`git diff --exit-code .claude/commands/review-plan.md` AND `git hash-object` baseline match)

  4. **Stub Scenario 4 — `/create-plan` on a throwaway request with stubbed tp-help**:
     - Positive pin: each of the 4 internal `/tp-help` call sites reached the stub (assert by counting `STUB_CODEX_RESPONSE_MARKER` occurrences in the create-plan working notes — must be ≥4)
     - Positive pin: `STUB_GEMINI_RESPONSE_MARKER` count also ≥4 (dual-source pin)
     - Positive pin: the final written 00-overview.md exists and parses as valid Markdown
     - Negative pin: NO `<!-- tp-help-reviewer:` literal sentinel in the final written 00-overview.md or any section file (sentinel-leakage check — this is the load-bearing assertion for create-plan, since its prose-rendering pipeline is the most likely to leak)
     - Negative pin: NO `STUB_CODEX_RESPONSE_MARKER` or `STUB_GEMINI_RESPONSE_MARKER` in the final 00-overview.md (raw stub markers in the user-facing plan would mean create-plan failed to summarize and just pasted prose — the wrong failure mode)

  **Stub semantic pin** (cross-cutting): the harness MUST verify dual-source by checking that BOTH stub markers are present in EVERY scenario's intermediate outputs. A single-source regression (e.g., gemini path silently fails and codex prose is the only output) would cause `STUB_GEMINI_RESPONSE_MARKER` to be missing — the harness exits non-zero with `dual_source_regression: gemini stub marker missing in scenario N` if so.

  Harness exits 0 only when all 4 stub scenarios pass ALL their positive AND negative pins. Runtime target: <60 seconds total (no real CLI wall time).

- [ ] **Scenario 1 — impl-hygiene-review integration test**: Run `/impl-hygiene-review` on a small test scope (a single file or small module). Verify:
  - **Positive pin (dual-source)**: Phase 4 invokes `/tp-help` internally (at both line 327 and line 344 prose call sites) and receives the dual-source concatenated response with content from BOTH reviewers. Assert by capturing the Phase 4 output to scratch and grepping for prose patterns characteristic of each reviewer (e.g., codex tends to start with bulleted analysis; gemini frequently cites web sources via `google_web_search` if applicable). At minimum, assert the Phase 4 output is at least ~2x the length of a single-source baseline captured before §07.2 landed.
  - **Positive pin (consumer integration)**: at least one finding in the merged hygiene results is tagged `[TP-CONFIRMED]` or `[TP-SURFACED]` — proves the wrapper's existing prose-processing logic still works on the new dual-source format.
  - **Negative pin (no consumer mutation)**: `git diff --exit-code .claude/skills/impl-hygiene-review/SKILL.md` returns 0 — the consumer file was NOT modified during the run.
  - **Negative pin (no sentinel leakage)**: grep the final hygiene-review output for `<!-- tp-help-reviewer:` and `<!-- /tp-help-reviewer:` — must return zero matches in any file the hygiene review wrote to (the wrapper's prose pipeline must strip or carry attribution sentinels without leaking them into final findings).
  - No errors, no aborted phases, exit 0 from the hygiene review wrapper.

- [ ] **Scenario 2 — `.claude/commands/review-plan.md` command-file integration test**: Run `/review-plan` (which dispatches to `.claude/commands/review-plan.md` — the 595-line 4-agent Claude pipeline, which is the only Claude-side `/review-plan` entrypoint in the plan since Section 06 was removed).
  - **Disposable target discipline**: `/review-plan` WRITES findings to the target plan's section TPR blocks. Do NOT point it at `plans/completed/<something>` — that would mutate a frozen plan. Instead, create a throwaway test plan in a scratch directory:
    ```bash
    TPDIR=$(mktemp -d -t tp-help-scenario2-XXXXXX)
    cp -r plans/completed/<smallest-frozen-plan>/* "$TPDIR/"
    # Run /review-plan against $TPDIR (NOT the original).
    ```
    Pick the SMALLEST frozen plan under `plans/completed/` (pick one with only 1-2 sections) to minimize wall time. The tmpdir copy is the write target.
  - **Cleanup is MANDATORY, not optional**: after Scenario 2 passes or fails, run `rm -rf "$TPDIR"`. If the scenario aborts abnormally, the tmpdir leak is a FAIL — record it and manually clean up.
  - Verify:
    - **Positive pin (dual-source at Step 3B)**: Step 3B's blind-spot `/tp-help` call (line 99 invocation prose) succeeds and the 4 review agents consume its output without parse errors. Capture the Step 3B intermediate output to `$TPDIR/step3b-tphelp.txt` and assert it contains text segments from BOTH reviewers (length > single-source baseline; explicit prose patterns differ between codex and gemini).
    - **Positive pin (dual-source at Midpoint Check)**: the midpoint `/tp-help` call (line 309 invocation prose — between Agent 2 and Agent 3) succeeds the same way; same dual-source assertion on the captured intermediate output.
    - **Positive pin (pipeline integrity)**: the command file's 4-agent pipeline runs to completion (each agent's exit signal received, no partial state).
    - **Negative pin (byte-identity)**: `git diff --exit-code .claude/commands/review-plan.md` still returns 0 after the run (command file unchanged — the byte-identical contract holds).
    - **Negative pin (baseline match)**: the §07.PRE frozen baseline hash still matches: `[[ "$(git hash-object .claude/commands/review-plan.md)" == "$(head -1 section-07-review-plan-baseline.sha1)" ]]`
    - **Negative pin (no repo drift)**: `git status --porcelain` shows NO modifications to any tracked file outside `$TPDIR` (the tmpdir is outside the repo, so it won't appear in `git status` at all — that's the point).
    - **Negative pin (no sentinel leakage)**: grep `$TPDIR` for `<!-- tp-help-reviewer:` and `<!-- /tp-help-reviewer:` — the 4-agent pipeline must NOT write attribution sentinels into the target plan files. If the pipeline rendered the raw concatenated tp-help output verbatim, the sentinels would leak; this assertion catches it.
    - No errors, no aborted phases, exit 0 from `/review-plan`.
    - `rm -rf "$TPDIR"` completed successfully.
  - **FAIL conditions**: any test that leaves artifacts in the repo tree outside `$TPDIR` is a hard FAIL. Any test that mutates the original `plans/completed/<plan>` is a hard FAIL. The byte-identical check on `review-plan.md` is a hard FAIL.

- [ ] **Scenario 3 — byte-identity regression test for `.claude/commands/review-plan.md`**: After running Scenario 2 verify:
  ```bash
  git diff --exit-code .claude/commands/review-plan.md
  echo "review-plan.md exit=$?"  # expected: 0
  # Also verify against the frozen §07.PRE baseline:
  current=$(git hash-object .claude/commands/review-plan.md)
  baseline=$(head -1 section-07-review-plan-baseline.sha1)
  [[ "$current" == "$baseline" ]] && echo "baseline matches" || { echo "DRIFT: $current vs $baseline" >&2; exit 1; }
  ```
  The baseline compare catches the case where the file was drifted-and-reverted during the section (git diff only catches current drift, not transient drift that was cleaned up).

- [ ] **Scenario 4 — `.claude/skills/create-plan/SKILL.md` integration test**: Run `/create-plan` on a tiny throwaway plan request (e.g., "create a plan for adding a one-line hello world test").
  - **Disposable target discipline**: `/create-plan` WRITES an entire plan directory under `plans/<slug>/` and currently has NO root-override flag (empirically verified against `.claude/skills/create-plan/SKILL.md` — no `--root` / `ORI_PLAN_ROOT` / `plan_root` hooks). This means Scenario 4 has two mutually exclusive execution modes and the plan MUST pick one before implementation begins:

    **Mode A (preferred) — ADD a root-override flag to create-plan FIRST, then run Scenario 4.** Mode A's prerequisite — a `--root`/`ORI_PLAN_ROOT` override on `/create-plan` — is **pre-filed in §07.PRE** as a tracked bug. The §07.PRE preflight files the bug via `/add-bug` and records the assigned BUG-ID in `section-07-scenario4-blocker.txt`; Scenario 4 cannot execute Mode A until that bug is fixed and merged. Once the bug fix lands, Scenario 4 uses:
    ```bash
    TPDIR=$(mktemp -d -t tp-help-scenario4-XXXXXX)
    ORI_PLAN_ROOT="$TPDIR" /create-plan "adding a one-line hello world test"
    # ... verification ...
    rm -rf "$TPDIR"
    ```
    **Mode B (fallback — ONLY if Mode A bug is still open at §07.3 start)** — use a DETERMINISTIC throwaway plan slug under `plans/` that the scenario KNOWS to delete by exact path. **Pre-check the slug does NOT already exist** to prevent nuking an unrelated plan:
    ```bash
    SCENARIO4_SLUG="tp-help-scenario4-$(date +%s)-$$"
    SCENARIO4_DIR="plans/${SCENARIO4_SLUG}"
    # COLLISION PRE-CHECK — refuse to run if the slug dir already exists
    if [[ -e "$SCENARIO4_DIR" ]]; then
      echo "FAIL: slug directory already exists: $SCENARIO4_DIR — refusing to clobber" >&2
      exit 1
    fi
    # Capture pre-run snapshot
    git status --porcelain plans/ > /tmp/scenario4-pre.txt
    /create-plan "adding a one-line hello world test (plan slug: ${SCENARIO4_SLUG})"
    # ... verification ...
    # Delete by EXACT path — NEVER use `git clean -fd`, which would nuke unrelated untracked files
    [[ -d "$SCENARIO4_DIR" ]] && rm -rf "$SCENARIO4_DIR"
    # Verify nothing else under plans/ was modified
    git status --porcelain plans/ > /tmp/scenario4-post.txt
    diff /tmp/scenario4-pre.txt /tmp/scenario4-post.txt || { echo "FAIL: plans/ tree drifted outside the scenario slug" >&2; exit 1; }
    ```
    Mode B is LAST RESORT because it relies on the scenario author correctly identifying and removing the slug directory. The collision pre-check + `$$` PID suffix make accidental clobber extremely unlikely; missed cleanup is a hard FAIL.
    **DECISION POINT** at §07.3 implementation time:
    1. Read `section-07-scenario4-blocker.txt` to get the BUG-ID filed in §07.PRE.
    2. Check whether that bug is closed. If yes → Mode A. If no → Mode B (record the decision and the open bug ID in working notes).
    3. Do NOT silently fall through from Mode A to Mode B mid-scenario.
  - **Cleanup is MANDATORY, not optional**: after Scenario 4 passes or fails, the scratch target (Mode A: `$TPDIR`; Mode B: `$SCENARIO4_DIR` by exact path) MUST be removed and `git status --porcelain plans/` MUST match the pre-run snapshot. If the scenario aborts abnormally, the leak is a FAIL — record and manually clean up.
  - Let it reach each of its `/tp-help` call sites in order and verify each dispatches correctly:
  - **Phase 1 call #1 (line 143 prompt construction + dispatch)**: create-plan builds a research prompt and calls `/tp-help`. Verify the dual-source concatenated response returns to create-plan and create-plan uses the text (or a summary of it) in its research notes. Look for HTML-comment attribution sentinels (`<!-- tp-help-reviewer: codex -->`, `<!-- /tp-help-reviewer: codex -->`, and the gemini equivalents) in create-plan's recorded research output — if they bleed into the final plan document, 07.2's attribution scheme needs adjustment.
  - **Phase 1 call #2 (line 166 refinement)**: create-plan calls `/tp-help` a second time with refined context. Verify the same dispatch + response-handling path.
  - **Phase 3 call (line 526 architectural sanity check)**: create-plan calls `/tp-help` before committing to the architecture. Verify the dual-source concatenated response informs create-plan's architectural decision without leaking attribution sentinels into the architecture section.
  - **Step 8B call (line 584 architecture sanity check before user presentation)**: create-plan calls `/tp-help` one last time before presenting the plan to the user. Verify the dispatch succeeds and create-plan integrates the response.
  - **Post-run baseline checks**: verify no drift in `.claude/commands/review-plan.md` (baseline hash match), no drift in `.claude/skills/create-plan/SKILL.md` (git diff exit 0 — create-plan is not on the byte-identical contract, but this scenario must not modify it since we are testing, not editing), no drift in `.claude/skills/impl-hygiene-review/SKILL.md`.
  - **Attribution-sentinel leakage check**: grep the final written plan overview for the literal strings `<!-- tp-help-reviewer: codex -->` and `<!-- /tp-help-reviewer: codex -->` (and the gemini equivalents). If ANY of the four sentinels appears, the attribution scheme is leaking through create-plan's prose rendering and §07.2 must adjust (e.g., add a per-invocation random suffix to the sentinel, or instruct the skill to strip attribution before returning in certain contexts). Record the result either way.
  - **Runtime budget — mandatory full-run closeout**: create-plan is the LONGEST downstream consumer — a full run with 4 `/tp-help` calls at ~10x dual-source wall time means ~80-120 minutes total. **This scenario MUST be run in full at least once at §07.3 close-out** with `ORI_TPR_REVIEWERS=both` (the default). Using `ORI_TPR_REVIEWERS=codex` as a development shortcut is permitted during iteration but does NOT count toward the close-out — the stub harness covers the "fast iteration" need, so shortcutting the real run via env-var is SCOPING DOWN. Both the stub harness run and the real `both` run MUST be recorded in working notes before the subsection can be marked `complete`.
  - **Budget math for the real run**: at ~10-15 min per dual-source `/tp-help` call × 4 calls = 40-60 min wall time for Scenario 4 alone, plus the rest of the create-plan orchestrator work. Budget a 2-hour block; use the stub harness for any iteration work inside that block.

- [ ] If ANY downstream consumer breaks because of the new response format: flag immediately. Do NOT silently fix `impl-hygiene-review`, modify `.claude/commands/review-plan.md`, or modify `.claude/skills/create-plan/SKILL.md` as part of this section — that would be scope creep and/or violate the byte-identity contract. Either (a) adjust the concatenation format in 07.2 to match what the downstream consumers expect (e.g., add a per-invocation random suffix to the HTML-comment sentinels if collision is detected), or (b) escalate to the user as a separate scope expansion decision.

- [ ] Record all four scenario results in working notes. The working notes MUST include: which scenarios passed (stub AND real), which failed, any attribution-sentinel leakage observed, the wall time for each real scenario (to calibrate user expectations for dual-source `/tp-help` consumers), and confirmation that the Scenario 4 real run used `ORI_TPR_REVIEWERS=both` (NOT `=codex` as a shortcut — the stub harness is where fast iteration happens).

- [ ] **Subsection close-out (07.3)** — MANDATORY before section completion:
  - [ ] `validate-tp-help-consumers.sh` stub harness exists, runs all 4 stub scenarios in under 60s, exits 0
  - [ ] impl-hygiene-review REAL integration test passes (Scenario 1 with `ORI_TPR_REVIEWERS=both`)
  - [ ] `.claude/commands/review-plan.md` REAL command-file integration test passes (Scenario 2 with `ORI_TPR_REVIEWERS=both`)
  - [ ] `.claude/commands/review-plan.md` byte-identity check passes (Scenario 3) — both `git diff --exit-code` AND the frozen §07.1 baseline-hash compare
  - [ ] `.claude/skills/create-plan/SKILL.md` REAL integration test passes (Scenario 4 with `ORI_TPR_REVIEWERS=both`, full-run mandatory) across all four internal `/tp-help` call sites
  - [ ] No HTML-comment attribution sentinel leakage into create-plan's final plan overview (grep for all four literal sentinel strings in the written plan files)
  - [ ] No modifications to `impl-hygiene-review`, `.claude/commands/review-plan.md`, or `.claude/skills/create-plan/SKILL.md` required
  - [ ] Update this subsection's `status` to `complete`
  - [ ] Run `/improve-tooling` retrospectively — should `validate-tp-help-consumers.sh` be folded into `transport-tests.sh` as a new category, or kept as a separate harness? Should the per-invocation random suffix for sentinels be added preemptively, or only after a real collision is observed? If the HTML-comment sentinel format proved too intrusive despite being invisible to renderers (e.g., some downstream consumer's markdown-to-html pipeline strips the comments on the way in and loses attribution), document the failure and file a follow-up bug via `/add-bug` for a more robust attribution scheme.

---

## 07.R Third Party Review Findings

- None.

---

## 07.N Completion Checklist

- [ ] All five subsections (07.PRE, 07.0, 07.1, 07.2, 07.3) marked `complete`
- [ ] `.claude/skills/dual-tpr/scripts/dual-invoke.sh` `--schema` is OPTIONAL (§07.0)
- [ ] §02's plan file reflects the schema-optional contract (§07.0 cross-section touch)
- [ ] §08's plan file reflects the `ORI_TPR_REVIEWERS` wiring move from §08.2 into §07.2 (§07.0 cross-section touch)
- [ ] `.claude/skills/tp-help/SKILL.md` is canonical; `.claude/commands/tp-help.md` is a thin pointer with valid frontmatter (`description`, `allowed-tools`, `argument-hint`)
- [ ] R10 resolved (no operational duplication between the two files)
- [ ] `parse-codex-raw.py` and `parse-gemini-raw.py` exist, executable, and pass their smoke tests
- [ ] `validate-tp-help-consumers.sh` stub harness exists and exits 0 (4 stub scenarios pass in <60s)
- [ ] `/tp-help` returns concatenated dual-source output with HTML-comment sentinel attribution markers (NOT H2 headers)
- [ ] `dual-invoke.sh` has `ORI_TPR_REVIEWERS` branching wired (§07.2, moved from §08.2)
- [ ] `dual-invoke-with-retry.sh` skips parsing for un-launched reviewers (§07.2, moved from §08.2)
- [ ] Gemini prompt includes the read-only-reviewer preamble
- [ ] Inline worktree-guard check wired into the skill's Steps section (catches prompt-discipline violations; the skill IS the guardrail in concat mode because the retry wrapper is skipped)
- [ ] `/impl-hygiene-review` Phase 4 cross-check passes REAL integration test (§07.3 Scenario 1 with `ORI_TPR_REVIEWERS=both`)
- [ ] `/review-plan` 4-agent pipeline passes REAL integration test (§07.3 Scenario 2 with `ORI_TPR_REVIEWERS=both`)
- [ ] `/create-plan` passes REAL integration test across all four internal `/tp-help` call sites (§07.3 Scenario 4 with `ORI_TPR_REVIEWERS=both` — mandatory full run, no `=codex` shortcut)
- [ ] No HTML-comment attribution sentinel leakage into create-plan's final plan overview (grep for the four literal sentinel strings in any written plan files)
- [ ] At least 1 real /tp-help scenario passes
- [ ] `.claude/commands/review-plan.md` is BYTE-IDENTICAL to its pre-plan state: `git diff --exit-code .claude/commands/review-plan.md` exits 0 AND `git hash-object .claude/commands/review-plan.md` matches the `section-07-review-plan-baseline.sha1` captured in the §07.PRE Section-Entry Preflight _(byte-identical contract inherited from the removed Section 06 on 2026-04-08; §07.3 Scenario 2 exercises the command file, so §07 is the natural regression-guard owner; the frozen baseline makes mid-section drift catchable even if it was reverted before §07.N; the baseline capture lives in §07.PRE — BEFORE §07.0 — so §07.0's script edits cannot accidentally mutate review-plan.md before the baseline lands)_
- [ ] `timeout 150 ./test-all.sh` green
- [ ] Plan annotation cleanup clean
- [ ] **Plan sync** — update plan metadata to reflect §07's completion:
  - [ ] §07 frontmatter `status` → `complete`; all subsection statuses (07.PRE, 07.0, 07.1, 07.2, 07.3, 07.R, 07.N) → `complete`
  - [ ] `00-overview.md` Quick Reference table row for §07 status → `Complete`
  - [ ] `00-overview.md` mission success criteria checkboxes updated (check off the §07-delivered criteria: `/tp-help` dual-source, command file consolidation, byte-identical review-plan.md, impl-hygiene-review Phase 4 still works, `ORI_TPR_REVIEWERS` honored — except where §08 verification is still pending)
  - [ ] `00-overview.md` Known Bugs row for R10 marked resolved (the SSOT violation between `commands/tp-help.md` and `skills/tp-help/SKILL.md`)
  - [ ] `index.md` §07 status updated
  - [ ] §02 plan frontmatter and §02.1 amendment from §07.0 are still intact (re-grep `--schema` optional success criterion)
  - [ ] §08 plan task list reflects the §07.2 toggle wiring move (§08.2 is verification-only, not implementation)
  - [ ] §08 `depends_on: ["05", "07"]` verified — no stale assumptions from §07's work that would break §08's preconditions
- [ ] `section-07-review-plan-baseline.sha1` removed (it is §07-local scaffolding, not a permanent artifact)
- [ ] `section-07-scenario4-blocker.txt` removed (also §07-local scaffolding; the bug it tracked has either been closed or remains a tracked open bug in the bug-tracker independent of this plan)
- [ ] `/tpr-review` (dual-source) passed
- [ ] `/impl-hygiene-review` passed — note: this is now the SECOND run of impl-hygiene-review in this session (first was in 07.3's integration test; second is the section-close verification)
- [ ] `/improve-tooling` section-close sweep done

**Exit Criteria:** `/tp-help` is dual-source with concatenation output (HTML-comment sentinel attribution, no sibling launcher, reusing `dual-invoke.sh` via the schema-optional contract from §07.0); the R10 SSOT violation is fixed; the `ORI_TPR_REVIEWERS` toggle is wired in one canonical location; `/impl-hygiene-review` Phase 4 cross-check, `/review-plan` 4-agent pipeline, and `/create-plan` orchestrator all consume the new format correctly. Section 08 (integration + cleanup) can begin — its scope is reduced by the §07.2 toggle wiring move.
