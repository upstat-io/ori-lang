# Round Triage Protocol

Read by an **Opus** sub-agent dispatched from `/tpr-review` after round setup returns with merged findings. Not a registered skill.

The triage agent owns every judgment-writing step per `SKILL.md` §Model Policy: verifying each reviewer finding against the actual code, judging reviewer thoroughness, routing findings (plan-owned vs bug-tracker), and fixing them. It reads `/tmp/tpr-{run}/round-{N}/merged.json` (produced by setup) plus the run-state JSON from the coordinator, and writes `/tmp/tpr-{run}/round-{N}/triage.json` with:

```json
{
  "accepted": N,
  "rejected": M,
  "actionable_after_triage": K,
  "thoroughness_ok": true | false,
  "strengthened_language_required": true | false,
  "fixes_committed": true | false,
  "escalate": false | true,
  "escalation_reason": null | "cap-hit-findings" | "cap-hit-wasted" | "user-judgment-needed",
  "round_summary": "…markdown string, see §Round Summary Rendering below…"
}
```

The coordinator uses the decision flags to decide whether to loop, exit clean, or escalate to the user. It prints `round_summary` verbatim to the user between rounds so the user sees a rich rendering of what each round surfaced and how it was disposed of — without the coordinator having to re-read `merged.json` itself.


## Round Summary Rendering (MANDATORY field `round_summary`)

After §7 completes (file + fix + commit), you MUST populate `round_summary` with a user-facing markdown block. This is the ONLY rendering of per-finding detail the user sees between rounds — the coordinator deliberately does NOT read `merged.json`, so if you omit detail here, the user is left with the bare counts from the other JSON fields and cannot track progress across rounds.

### Required structure

```md
### Round {N} Summary

**Dispatch**: codex {codex_findings} / gemini {gemini_findings} / agreements {agreements} / merged actionable {merged_actionable}
**Thoroughness**: ASYMMETRY {LOW|MODERATE|HIGH} — {one-sentence rationale citing files_read / rules_consulted / verification basis}
**Triage**: accepted {accepted} / rejected {rejected} / actionable after triage {actionable_after_triage}
**Fix commit**: {commit_sha or "none — no actionable findings this round"}

**Findings this round:**
- `[ID][severity]` `path:line` — title. Disposition: {fixed in {sha} | rejected: {one-line verification note} | handed off to /fix-bug BUG-XX-NNN}.
- … one bullet per merged finding entry (agreement findings produce ONE bullet, cross-referencing both halves) …

**Next round will confirm**: {one sentence — what the next round should verify, e.g. "that the N fixes hold and no regressions were introduced" OR "that strengthened-language re-review surfaces the depth this round lacked" OR "loop exiting clean"}.
```

### Rules

- **One bullet per merged finding.** For agreement findings, ONE bullet covering both halves (cite both reviewer-tagged IDs inline). Do NOT produce two bullets for the same agreement entry — that visually doubles the round's apparent size.
- **Disposition is mandatory.** Every bullet MUST end with `Disposition:` stating exactly one of: `fixed in {commit_sha}`, `rejected: {verification note}`, or `handed off to /fix-bug BUG-XX-NNN`. A bullet without a disposition is a bug in this rendering — the whole point is that the user can see at a glance what happened to each finding.
- **Rejections are rare and must be justified.** Per §5, the ONLY valid rejection is verification that proves the finding wrong. Quote the specific check that disproved it (one line max).
- **Keep bullets terse.** One line per finding, ≤120 characters. The user can `cat merged.json` for full evidence — this rendering is the summary, not the dossier.
- **Clean-pass rounds still render this block.** On a clean pass, the `Findings this round:` list is empty or says `(none — both reviewers returned zero actionable findings)`, and `Next round will confirm` becomes `loop exiting clean`. The user still wants to see the thoroughness verdict and the dispatch counts.
- **Pure-waste rounds (zero findings + thin) MUST include the rejection rationale** in the Thoroughness line — cite the specific signals (walltime ratio, empty `rules_consulted`, etc.) that triggered `thoroughness_ok = False`. This is the same evidence §6e requires you to write down for eventual escalation; reusing it here costs nothing and makes the waste visible to the user in real time.

### Example (findings + thorough round)

```md
### Round 0 Summary

**Dispatch**: codex 7 / gemini 8 / agreements 3 / merged actionable 12
**Thoroughness**: ASYMMETRY LOW — both envelopes read 11 files + CLAUDE.md/impl-hygiene.md/tests.md, codex ran ./test-all.sh, walltime ratio 1.3x.
**Triage**: accepted 12 / rejected 0 / actionable after triage 12
**Fix commit**: 353864a5

**Findings this round:**
- `[TPR-04-001-codex+gemini][high]` `ori_arc/src/lower/iter.rs:218` — missing dec on early-exit. Disposition: fixed in 353864a5.
- `[TPR-04-002-gemini][medium]` `library/std/prelude.ori:5` — replace println with tracing::debug. Disposition: fixed in 353864a5.
- `[TPR-04-003-codex][low]` `ori_parse/src/expr/binary.rs:142` — tighten operator-precedence error span. Disposition: fixed in 353864a5.
- … (9 more) …

**Next round will confirm**: that the 12 fixes hold and no regressions were introduced; strengthening flag cleared (thorough review).
```

### Example (zero findings + thin — pure waste)

```md
### Round 2 Summary

**Dispatch**: codex 0 / gemini 0 / agreements 0 / merged actionable 0
**Thoroughness**: ASYMMETRY HIGH — gemini walltime 3.8x codex, gemini rules_consulted empty, gemini files_read 2 on a 14-file diff.
**Triage**: accepted 0 / rejected 0 / actionable after triage 0
**Fix commit**: none — no actionable findings this round

**Findings this round:** (none — both reviewers returned zero actionable findings, but gemini's review was skimming, not clean)

**Next round will confirm**: that gemini actually investigates the scope when given the Thoroughness Re-review Directive (thoroughness_reject_counter now 2/3).
```



## ABSOLUTE: You May NEVER Reason Out of Findings

**There is NO circumstance under which you may dismiss, rationalize, scope-note, or defer a TPR finding.** The ONLY valid responses to a finding are:

1. **Fix it NOW** — write code, write tests, verify, commit
2. **Create a plan and execute it** — if too large for inline fix, create concrete implementation steps, then implement them
3. **AskUserQuestion** — if genuinely blocked (need user decision, missing domain knowledge)

**BANNED responses to findings — using ANY of these is a violation:**
- "Pre-existing issue" / "was already broken"
- "Architectural limitation" / "requires major refactor"
- "Out of scope" / "not a §03 deliverable"
- "Conservative/safe" / "only precision loss"
- "Not a regression" / "not introduced by this work"
- "Future improvement" / "tracked for later"
- "Scoped as known limitation"
- Marking `[x] Resolved:` with an explanation instead of a code fix

**The size of the fix is irrelevant.** If the correct fix requires cross-crate refactoring across 10 files, that IS the work. "Requires architectural change" is not a reason to skip — it IS the work.

**"Future improvement" requires a concrete artifact.** If you ever say something will be tracked, you MUST in the same response create: a bug-tracker entry (`/add-bug`), plan section `- [ ]` item, or roadmap checkbox. Ask yourself: "When would this get done? Who would find it?" If nobody/never, fix it now.

## ABSOLUTE: Correct Architectural Solutions Only

**Before fixing ANY finding, read `.claude/rules/impl-hygiene.md`.** This is non-negotiable. The hygiene rules define SSOT (Single Source of Truth), No Side Logic, canonical homes, phase boundaries, and finding categories (LEAK, DRIFT, GAP, etc.). Every fix must respect these principles.

**Fixes must be the correct, proper architectural solution — never quick fixes, workarounds, counters, flags, or hacks.** Specifically:

- **SSOT**: if the finding reveals scattered knowledge or duplicated dispatch, the fix is to establish/use the canonical home — not to patch each copy
- **No Side Logic**: if logic lives outside its canonical home, the fix is to move it — not to add another copy that "works"
- **Canonical Homes**: every behavioral decision has exactly ONE file that defines it. If a fix would create a second source of truth, it is wrong
- **Phase Boundaries**: fixes must not bleed phase responsibilities. If fixing a codegen bug requires adding type-checking logic to the codegen pass, that's the wrong fix — the type checker should provide the information
- **Registry as Source of Truth**: builtin type behavior (methods, operators, memory) lives in `ori_registry`. Fixes that hardcode type behavior outside the registry are LEAKs
- **Enforcement**: when a fix adds a new variant, sync point, or dispatch arm, it MUST have enforcement (exhaustive match, exhaustiveness test, or registry-driven generation) to prevent future drift

**The "quick fix" test**: if your fix would not survive a code review by someone who has read `impl-hygiene.md`, it's wrong. The correct fix may touch 10 files across 3 crates — that IS the fix. A workaround that passes tests is not a fix.



### 5. Classify merged findings (and VERIFY each one independently)

**Reviewer findings are hypotheses, not facts.** For every actionable finding, Claude MUST independently verify the claim against the actual code BEFORE acting on it — regardless of which reviewer produced it.

#### Verification protocol (mandatory for every finding)

For each merged finding:

1. **Read the cited code** — open the file at the cited line number, read the surrounding context (not just the one line)
2. **Confirm the claim matches reality** — does the code actually say what the finding claims? Does it actually behave the way the finding describes?
3. **Trace the reasoning** — if the finding says "X is unreachable" / "Y is broken" / "Z is missing", prove it by walking the code yourself. Use `scripts/intel-query.sh callers "<symbol>" --repo ori` and `callees` to trace the call chain (faster and more complete than grep), then check the test coverage.
4. **Check the required_plan_update** — does the proposed fix actually address the root cause, or is it a surface patch that would leave the underlying issue?

If verification proves the finding is wrong, mark it `[x]` with a verification note explaining what you checked and what you found — this is the ONLY valid way to reject a finding. Rejecting without verification is banned; accepting without verification is banned.

#### Trust tiers (set verification depth, not pass/fail)

Both reviewers can be wrong. The trust tier sets how deep the verification goes:

- **Codex: HIGH trust.** Codex tends to cite accurate file/line numbers and its claims usually match the code. Spot-check each finding: read the cited lines, confirm the specific claim, move on if it holds.
- **Gemini: LOWER trust.** Gemini is more prone to confabulation — invented line numbers, misquoted code, claims about behavior that don't match reality, and "positive observations" that reframe correct code as findings. Every gemini finding needs FULL verification: read the cited file in full (not just the cited line), trace the code path end-to-end, and confirm against what the code actually does. This is especially important for:
  - Claims about untested code paths (gemini may miss the test that covers it)
  - Claims about architectural issues (gemini may not have read the canonical home)
  - Claims involving specific line numbers (gemini sometimes invents them)
  - Positive confirmations (e.g. "the fix is correctly done") — only useful if actually correct

Never treat gemini's `citations` URLs as authoritative — if gemini cites a spec or external doc, verify the claim independently instead of trusting the URL as truth.

#### Actionability

After verification confirms a finding is real:

- **Actionable finding**: real code issue — bug, hygiene violation, missing test, incorrect behavior, file size limit exceeded, precision regression, dead code path, etc. Must be fixed.
- **Non-actionable observation**: style preference or observation that isn't a defect, precision loss, or dead code. Note it but don't block the loop on it.
- **Informational finding** (`severity: "informational"`): non-actionable by definition. The reviewer had no actionable findings but wanted to note an observation. Treat as non-actionable — do not fix, do not block the loop. The merge summary's `actionable` count already excludes these.

**IMPORTANT: Err on the side of "actionable"** (after verification). The following are ALWAYS actionable:
- Dead code paths (code that can never execute)
- Precision regressions (over-approximation that loses optimization opportunities)
- Missing tests for plumbed-through data
- Name collisions or aliasing that cause incorrect behavior
- Pipeline gaps where data is computed but never consumed

**Agreement is a priority signal, not a filter.** When an entry has `agreement: true`, both reviewers independently flagged the same `(location, title)` — the strongest possible signal, so prioritize these fixes. When an entry is tagged `-codex` or `-gemini` only (`agreement: false`), the finding is STILL real after verification — provenance is not severity. Single-reviewer findings get fixed just like agreement findings.

**Agreement is not a substitute for verification.** Two reviewers can be wrong about the same thing — agreement amplifies the hypothesis but doesn't prove it. Verify the claim against the code even when both reviewers flagged it.

### 6. Thoroughness Judgment (MANDATORY — runs EVERY round, regardless of findings)

**This step is MANDATORY on every loop iteration**, not only zero-finding rounds. Thoroughness is about whether the reviewers actually DID THE WORK — a separate question from whether they found anything. A round can produce findings AND still be thin (the reviewers caught one obvious bug but skimmed the rest), and a round can produce no findings AND still be thorough (the reviewers investigated deeply and confirmed the code is sound). Both dimensions matter independently.

"No findings" conflates two structurally different cases:

1. ✅ **Genuine clean**: reviewers did a thorough investigation and correctly found nothing to fix. Clean pass, exit the loop.
2. ❌ **Shallow skim (zero-findings thin)**: reviewers did a superficial pass and therefore found nothing *because they did not look hard enough*. Nothing was captured — the round is pure waste. Mandatory re-run.

"Findings present" conflates two structurally different cases:

1. ✅ **Thorough review with findings**: reviewers did the work AND surfaced real issues. Fix them, clear the strengthening flag, proceed to the next round normally.
2. ❌ **Thin review with findings**: reviewers caught one or two obvious issues but skimmed the rest. The findings they DID catch are real and must be fixed. But the reviewers probably missed more. KEEP the strengthening flag True so the re-review of the fixed code still demands deeper investigation.

Case ❌ in either table is the failure mode this step exists to catch. If Claude accepts a thin round as "good enough," the /tpr-review ceremony becomes a rubber-stamp. If Claude *discards* findings from a thin round because "the review was thin anyway," the one piece of real work from that round is wasted. Neither is acceptable.

**Thoroughness judgment is Claude's call**, not a static threshold. No set of numeric rules can perfectly distinguish "fast but thorough" from "fast because skimming" — some scopes really are quickly reviewable, and event count is noisy. The tooling (`status-check.sh`) surfaces the signals; Claude reads them and decides.

#### 6a. Gather thoroughness signals

Run `status-check.sh` one more time against the final `$RUN` for the asymmetry block:

```
Bash:
  .claude/skills/dual-tpr/scripts/status-check.sh "$RUN" --events 10
```

The script's "thoroughness comparison:" block renders walltime, event count, and byte count for both reviewers side-by-side with max/min ratios. Each dimension is flagged:

- **`r >= 3.0x`**: red flag — very likely the faster reviewer skipped depth
- **`r >= 2.0x` and `r < 3.0x`**: yellow — worth a spot-check
- **`r < 2.0x`**: comparable depth

And the aggregate verdict:

- **`ASYMMETRY: HIGH`** (2+ dimensions red) — thin-candidate
- **`ASYMMETRY: MODERATE`** (1 red or 2+ yellow) — spot-check before judging
- **`ASYMMETRY: LOW`** (all dimensions < 2.0x) — thorough-candidate

Also read both envelopes directly (both are fully written by this point — completion notification has arrived):

```
Read: $RUN/codex.envelope.json
Read: $RUN/gemini.envelope.json
```

From each envelope, extract:

- `scope_actually_reviewed.files_read` — how many files did the reviewer actually read?
- `scope_actually_reviewed.rules_consulted` — did the reviewer read the grounding rules?
- `scope_actually_reviewed.specs_consulted` — did the reviewer check the spec for affected behavior?
- `scope_actually_reviewed.expanded_beyond_packet` — did the reviewer broaden the scope as the methodology requires?
- `verification.tests_rerun` — did the reviewer run any tests?
- `verification.diagnostics_run` — did the reviewer run any diagnostic scripts?

#### 6b. Make the judgment — outputs `thoroughness_ok`

Using the signals from 6a, evaluate a boolean: `thoroughness_ok`. Evaluate this on EVERY round, regardless of how many findings the round produced. The flag is NOT a terminal decision — it feeds into the finding-count branches in 6c below.

**Judge `thoroughness_ok = False` (thin review) when ANY of these are true:**

- `status-check.sh` shows `ASYMMETRY: HIGH` (2+ red dimensions) AND the faster reviewer's envelope has thin `files_read` (e.g., fewer than a small handful of files on a non-trivial scope) OR empty `rules_consulted`.
- `status-check.sh` shows `ASYMMETRY: MODERATE` AND the faster reviewer's envelope has OTHER symptoms of skimming: empty `rules_consulted`, empty `specs_consulted` on a subsystem the spec governs, or `files_read` clearly shorter than the diff's file list.
- Either envelope has **empty `rules_consulted`** when the grounding block required at least `CLAUDE.md` + `.claude/rules/impl-hygiene.md` + `.claude/rules/tests.md`. Grounding skipped = methodology skipped; this alone justifies `thoroughness_ok = False` even with `ASYMMETRY: LOW`.
- Either envelope's `files_read` is **clearly shorter than the diff's file list** on a non-trivial scope. The methodology requires reading whole changed files, not just diff hunks.
- Either envelope's `verification` block is empty when the scope clearly warranted a `fresh_verification` basis (e.g., a codegen change with no diagnostic run).
- The faster reviewer's event stream (tail of `status-check.sh`) shows almost no `tool_use` / `command_execution` events — i.e., the reviewer "read nothing and ran nothing" before emitting the envelope.

**Judge `thoroughness_ok = True` (thorough review) when ALL of these are true:**

- `status-check.sh` shows `ASYMMETRY: LOW` OR `MODERATE` with thorough `files_read` / `rules_consulted` on both sides.
- Both envelopes have non-empty `rules_consulted` covering at least `CLAUDE.md` + the relevant `.claude/rules/*.md`.
- Both envelopes have `files_read` consistent with the scope (not obviously truncated).
- At least one envelope has a `verification` basis (tests or diagnostics run) when the scope warranted it.

**Judgment calls** between the two extremes — use sense. The goal is to filter out skimming, not to manufacture non-existent problems. A genuinely small scope with a short but correct review should be judged thorough; a broad scope with a quick skim should not.

#### 6c. Apply the judgment — the four-cell decision matrix

Branch on finding count × thoroughness. Every round falls into exactly one of these four cells:

| | `thoroughness_ok = True` | `thoroughness_ok = False` |
|---|---|---|
| **Zero actionable findings** | **CLEAN PASS** — exit the loop. Report per §6d. | **Mandatory re-review** — nothing to preserve. Increment `thoroughness_reject_counter`, set `strengthened_language_required = True`, `continue` loop. See §6e. |
| **Actionable findings exist** | **Fix and re-run (normal)** — proceed to §7. Clear `strengthened_language_required`. Reset `thoroughness_reject_counter` (it's already 0 in this sub-sequence). | **Fix and re-run WITH strengthening** — proceed to §7. Findings are filed and fixed normally (they are NOT discarded). KEEP `strengthened_language_required = True` so the re-review of the fixed code still demands deeper investigation. Reset `thoroughness_reject_counter` (findings = progress, even if depth was thin). See §6f. |

**CRITICAL RULE: findings are NEVER discarded on a thin review.** When a round produces actionable findings AND thoroughness was thin, the fix path in §7 still runs — file, fix, commit. The thin-depth signal propagates to the next round via `strengthened_language_required`, NOT by throwing away findings that were already captured. Discarding real findings because "the review was thin anyway" would waste the only output that round produced and punish the reviewers for having caught anything at all.

#### 6d. CLEAN PASS — zero findings + thorough

Report to the user:
- "Dual-source TPR review passed clean — both reviewers returned zero actionable findings and Claude's thoroughness judgment accepted the round."
- Iteration count (e.g. "clean on iteration 1" or "clean on iteration 3 after fixing N findings and 1 thin round").
- Thoroughness summary: `ASYMMETRY: {LOW|MODERATE}` + one-sentence rationale referencing the envelopes' `files_read` / `rules_consulted` counts.
- Merge summary from the final iteration (`codex_findings`, `gemini_findings`, `agreements`).
- **This is the ONLY clean exit from the loop.**

#### 6e. Mandatory re-review — zero findings + thin (pure waste)

Do NOT treat a thin-no-findings round as a clean pass. Do NOT file "no findings" anywhere (there's nothing to file). This is the one cell where the round is PURE WASTE — no captured work of any kind. Handle it as follows:

1. Increment `thoroughness_reject_counter` (separate from `iteration_counter`; cap = 3). This counter is the "consecutive wasted rounds" safety net — it only grows on this one cell, because this is the only cell where a round produced literally nothing: no findings AND no verified depth.
2. Set `strengthened_language_required = True` so the next round's prompts include the Thoroughness Re-review Directive.
3. Write a brief rejection note for yourself — which signals triggered the reject (walltime ratio, empty `rules_consulted`, etc.) — so your eventual escalation report has specifics to cite.
4. `continue` the loop back to Step 1 — a new `$RUN`, new prompts, new transport invocation. Do NOT increment `iteration_counter`; nothing was fixed.
5. If `thoroughness_reject_counter` reaches 3, escalate via §8b — three consecutive wasted rounds means prompt discipline alone is not coaxing depth and the user needs to intervene.

#### 6f. Fix path with persisted strengthening — findings + thin

Proceed to §7 (file, fix, commit, re-run) EXACTLY as you would for a thorough round — findings are real captured work and MUST NOT be discarded. The only differences from the thorough branch are what happens to the flags AFTER §7 runs:

1. After §7 commits the fixes, KEEP `strengthened_language_required = True` (do NOT clear it). The next round's re-review of the fixed code will still receive the Thoroughness Re-review Directive, because the reviewers were thin on this round and may have missed findings beyond the ones they caught.
2. Reset `thoroughness_reject_counter = 0` (findings = progress, regardless of depth — this counter only tracks "wasted rounds," and this round was NOT wasted).
3. Increment `iteration_counter` normally (this is a finding-fixing round).
4. `continue` the loop.

The net effect: thin-findings rounds make progress on the findings they DID catch, while the next iteration keeps hunting for the findings they missed. The flag acts as a persistent "look harder" signal that only clears when a round finally runs thoroughly — whether or not that round produces additional findings.

**Why the counter and the flag measure different things.** The counter (`thoroughness_reject_counter`) tracks "consecutive wasted rounds" — rounds that produced literally nothing. The flag (`strengthened_language_required`) tracks "depth of the last round" for the NEXT round's prompting. A thin-findings round is NOT wasted (findings got fixed) but IS thin (next round still needs strengthening), so the counter resets but the flag stays True. Conflating these two was a design error fixed in this version of §6.

**Why "wasted" is narrower than "thin".** The escalation cap should fire only when the loop is producing *nothing*, not when it's producing *some* things thinly. A round that caught even one bug on a thin review is forward progress — the counter resets. Only the zero-findings + thin cell (§6e) truly represents "we ran the loop and got nothing," which is the condition the 3-count cap is designed to catch.


### 7. If Actionable Findings Exist -> Fix and Re-run

#### 7a. File Findings

For each validated finding, decide where it lives:

1. **Is there an owning plan section?** — check whether an active plan (roadmap or reroute) has a section covering the affected code.
2. **If yes** — record the entry (or both halves of an agreement) in that section's `## {NN}.R Third Party Review Findings` block using the reviewer-tagged IDs from `merge-findings.py` verbatim:
   ```md
   - [ ] `[TPR-04-001-codex][high]` `compiler/foo.rs:123` — Add dec on early-exit branch.
     Evidence: ... Impact: ... Required plan update: ...
     Basis: fresh_verification. Confidence: high.
   - [ ] `[TPR-04-001-gemini][high]` `compiler/foo.rs:123` — Add dec on early-exit branch.
     Evidence: ... Impact: ... Required plan update: ...
     Basis: direct_file_inspection. Confidence: high. Citations: [{url: "...", description: "..."}]
   ```
   Update plan metadata (`third_party_review.status: findings`, `updated: {today}`).

3. **If no owning plan exists** — file as a bug in `plans/bug-tracker/` under the appropriate subsystem section using the **canonical `BUG-{section}-{ordinal}` format — no reviewer suffix**. Reviewer provenance lives in the body, not the ID. This is the SSOT contract enforced by `.claude/skills/add-bug/SKILL.md:75`, `plans/bug-tracker/00-overview.md:41`, `.claude/commands/review-work.md:108`, and consumed by `/fix-bug BUG-XX-NNN`, `/review-bugs`, and `fix-BUG-XX-NNN.md` filenames. Suffixed IDs would create a shadow bug-ID home that breaks all of those downstream consumers.

   **For an agreement finding** (both reviewers flagged the same `(location, title)`), file ONE BUG entry covering both reviewers' observations — the agreement doesn't require two bug entries:
   ```md
   - [ ] `[BUG-{section}-{ordinal}][{severity}]` **{Short title}** — found by tpr-review (dual-source).
     Repro: {evidence summary from both reviewers}
     Subsystem: {crate/file path}
     Found: {YYYY-MM-DD} | Source: tpr-review | Reviewers: codex + gemini (agreement)
     Fix: `plans/bug-tracker/fix-BUG-{section}-{ordinal}.md` (via `/fix-bug`)
   ```

   **For a single-reviewer finding** (only one reviewer flagged it — `agreement: false`), file ONE BUG entry and note which reviewer surfaced it:
   ```md
   - [ ] `[BUG-{section}-{ordinal}][{severity}]` **{Short title}** — found by tpr-review.
     Repro: {evidence from the single reviewer}
     Subsystem: {crate/file path}
     Found: {YYYY-MM-DD} | Source: tpr-review | Reviewer: codex
     Fix: `plans/bug-tracker/fix-BUG-{section}-{ordinal}.md` (via `/fix-bug`)
   ```

   Each BUG entry gets ONE ordinal regardless of how many reviewers found it — the ordinal space belongs to the subsystem section, not the reviewers. This preserves the canonical `BUG-XX-NNN` ID shape that all downstream tooling expects.

Subsystem mapping (unchanged from single-source version):
- `ori_parse`/`ori_lexer` -> section-01
- `ori_types` -> section-02
- `ori_eval`/`ori_patterns` -> section-03
- `ori_llvm`/`ori_arc` -> section-04
- `ori_rt` -> section-05
- `library/std`/`ori_registry` -> section-06
- `oric`/`ori_fmt`/`ori_diagnostic` -> section-07
- `docs/`/`.claude/`/`plans/` -> section-08

#### 7b. Fix Each Finding — branch by destination

**YOU (Claude) fix the code.** Actual implementation — not just filing, not scope notes, not rationalizations. CODE CHANGES. **The fix path differs based on where the finding was filed in Step 7a** — plan-owned findings are fixed inline; bug-tracker findings hand off to `/fix-bug`. Do NOT conflate the two paths — bug-tracker findings that skip the `/fix-bug` handoff bypass the mandatory TDD matrix, TPR review, and hygiene review per `.claude/skills/fix-bug/SKILL.md` and `CLAUDE.md` §"Bug fix rigor with `/fix-bug`".

##### 7b-i. Plan-owned findings (filed in `## {NN}.R Third Party Review Findings`)

Fix inline with the same rigor as the owning plan section:

- **Read `.claude/rules/impl-hygiene.md` before fixing** — SSOT, canonical homes, no side logic, phase boundaries. Every fix must be the correct architectural solution.
- Read the affected code and understand the issue
- Identify the **canonical home** for the knowledge/logic involved — the fix must respect it
- Follow TDD when appropriate (failing test -> fix -> test passes)
- Run `timeout 150 ./test-all.sh` after fixes
- **Self-check**: would this fix survive `/impl-hygiene-review`? If it introduces scattered knowledge, duplicated dispatch, or a shadow source of truth, it's wrong — find the proper architectural fix
- Mark the filed TPR finding as `[x]` resolved in the plan with a note referencing the code fix:
  ```md
  - [x] `[TPR-04-001-codex][high]` ...
    Resolved: Fixed on YYYY-MM-DD. [description of CODE fix].
  - [x] `[TPR-04-001-gemini][high]` ...
    Resolved: Fixed on YYYY-MM-DD. Same fix as [TPR-04-001-codex] (agreement).
  ```

##### 7b-ii. Bug-tracker findings (filed in `plans/bug-tracker/section-NN-*.md`)

**DO NOT fix inline. Hand off to `/fix-bug BUG-{section}-{ordinal}` for each bug.**

The `/fix-bug` skill creates a fix-section file (`plans/bug-tracker/fix-BUG-{section}-{ordinal}.md`) with full plan-section rigor: investigation, root cause analysis, TDD matrix (semantic + negative pins), implementation, and a completion checklist that includes `test-all.sh`, `/tpr-review`, and `/impl-hygiene-review`. This rigor is non-negotiable per `CLAUDE.md` §"Bug fix rigor with `/fix-bug`": "No ad-hoc bug fixes — every bug gets a fix section, even 'obvious' ones."

For each bug-tracker entry filed in Step 7a:
1. Invoke the Skill tool: `Skill: fix-bug BUG-{section}-{ordinal}`
2. Wait for `/fix-bug` to complete its workflow (which includes its own commit via `/commit-push` AND updates the bug-tracker entry to `[x]` resolved per `.claude/skills/fix-bug/SKILL.md:169` "Update the bug entry")
3. **Verify — do not re-edit.** After `/fix-bug` returns, check that the bug-tracker entry is already `[x]` and uses the canonical `Resolved: Fixed on YYYY-MM-DD` + `Fix: plans/bug-tracker/fix-BUG-XX-NNN.md` form from `plans/bug-tracker/00-overview.md:52`. If the entry is correctly updated, the wrapper's job is done for that bug — **do NOT re-author or edit the entry**. Bug-entry closure is `/fix-bug`'s canonical responsibility; duplicating it in the wrapper is a LEAK (scattered knowledge).
4. If the entry is somehow NOT updated after `/fix-bug` returns (rare — would indicate a bug in `/fix-bug` itself), file a follow-up bug against `/fix-bug` rather than patching the entry manually. Manual patches create drift from the canonical form.

**Why the wrapper must not edit the entry**: `.claude/skills/fix-bug/SKILL.md` owns bug-entry-closure logic as a single source of truth. If the wrapper re-edits the entry after `/fix-bug` completes, it creates a second copy of closure logic that can drift from the canonical form (as a prior version of this wrapper did — see `plans/dual-tpr-gemini/section-05-review-work.md §05.R [TPR-05-003-codex]`). The wrapper's contract is: invoke `/fix-bug`, then verify its output — nothing more.

**Why the hand-off matters**: skipping `/fix-bug` leaves no fix-section record, no TDD matrix, no TPR validation, and no hygiene review for the bug. It also leaves `/review-bugs` to report the lifecycle gap, and breaks `/fix-next-bug` autopilot which expects fix-sections to exist. The canonical contract exists precisely because bug-tracker bugs are often cross-cutting and benefit from the extra investigation rigor that a fix-section provides.

**If a bug-tracker finding genuinely requires zero investigation** (a typo fix or a single-line change with obvious root cause), the `/fix-bug` skill itself handles this efficiently — it still produces a fix-section, but the investigation/TDD phases are lightweight. The fix-section is the permanent record, not a gate.

#### 7c. Commit Fixes

Run `/commit-push` to commit the fixes. The commit message should reference the reviewer-tagged TPR IDs fixed (e.g. `fix(arc): release iterator on early break — [TPR-04-001-codex] [TPR-04-001-gemini]`).

#### 7d. Re-run the Dual-Source Transport (GO TO STEP 1)

Go back to Step 1. BOTH reviewers re-review the FIXED code to confirm the issues are actually resolved and no new issues were introduced by the fixes. **This re-run is not optional, and a partial re-run (only one reviewer) is not a valid clean pass.**


## Merged Finding Format

This section specifies how merged findings are written into the owning plan's `## {NN}.R Third Party Review Findings` block (or the bug-tracker, if there is no owning plan). Claude produces these entries in Step 7a above; the format is load-bearing because future `/tpr-review` runs, `/review-bugs`, and plan audits depend on it.

### Ordinal numbering is independent per reviewer

`merge-findings.py` assigns ordinals by **insertion order within each reviewer's envelope**, independently. The first finding in the codex envelope is `-001-codex`, the first in the gemini envelope is `-001-gemini`. There is NO shared ordinal space: `[TPR-04-001-codex]` and `[TPR-04-001-gemini]` are not required to be the same finding — the `agreement: true` flag from the merger is the authoritative cross-reference.

### Agreement case — both reviewers flagged the same (location, title)

When `merge-findings.py` reports `agreement: true`, both halves are filed adjacent with a cross-reference annotation. Both entries point at each other via the `Agreement:` line so the plan's TPR block preserves the independence contract while still making the convergence visible:

```md
- [ ] `[TPR-04-001-codex][high]` `compiler/ori_arc/src/lower/iter.rs:218` — Add dec on early-exit branch of iterator loop.
  Evidence: On `break` inside `for x in iter do ...`, the iterator value's RC is never decremented before the loop exits, leaving a leaked reference on the remaining elements. Reproduced via `tests/valgrind/iter_break.ori`.
  Impact: Memory leak on every early-exit iteration; severity scales with iterator payload size.
  Required plan update: Add a `dec` emission to the early-exit branch in `ori_arc/src/lower/control_flow/for_loop.rs`; verify via `ORI_CHECK_LEAKS=1` on the matrix tests.
  Basis: fresh_verification. Confidence: high.
  Agreement: [TPR-04-001-gemini] (both reviewers flagged this location/title)
- [ ] `[TPR-04-001-gemini][high]` `compiler/ori_arc/src/lower/iter.rs:218` — Add dec on early-exit branch of iterator loop.
  Evidence: The lowering pass emits an iterator inc on loop entry but the early-exit path in `lower_break` does not call the matching dec. Verified against Swift's SILOptimizer/ARC/ARCContract.cpp, which explicitly handles the analogous case.
  Impact: Same as above (agreement finding).
  Required plan update: Same as above.
  Basis: direct_file_inspection. Confidence: high.
  Citations: [{url: "https://github.com/apple/swift/blob/main/lib/SILOptimizer/ARC/ARCContract.cpp", description: "Swift's equivalent arc contract pass, for cross-reference"}]
  Agreement: [TPR-04-001-codex] (both reviewers flagged this location/title)
```

**Why both halves are filed** — filing only the codex half would erase the gemini reviewer's independent observation (and its citations), which violates the dual-source independence contract. Filing only the gemini half would erase the codex finding's ordinal continuity. Both are recorded; the `Agreement:` cross-reference makes the convergence clear to any human or tool auditing the block.

### Gemini-only case — a finding with no codex counterpart

```md
- [ ] `[TPR-04-002-gemini][medium]` `library/std/prelude.ori:5` — Replace println with tracing::debug.
  Evidence: The prelude emits a `println` on module load to report version info. `println` writes to stdout, which pollutes test captures; the project convention is to use `tracing::debug` with the `ori_*` target (see CLAUDE.md §Tracing).
  Impact: Test snapshot churn on every prelude load; violates the "no println" rule.
  Required plan update: Switch to `tracing::debug!(target: "ori_prelude", ...)` and add a `#[tracing::instrument]` on the prelude loader.
  Basis: inference. Confidence: medium. (Gemini-only finding — no codex counterpart.)
```

**Why single-tag is still actionable** — per Step 5 (Classify), provenance is not severity. A gemini-only finding gets fixed the same way as an agreement finding; the tag is audit metadata, not a filter.

### Codex-only case — symmetric to gemini-only

```md
- [ ] `[TPR-04-003-codex][low]` `compiler/ori_parse/src/expr/binary.rs:142` — Tighten error span on operator-precedence mismatch.
  Evidence: The current error points at the left operand; the spec's operator-rules.md §B.PRECEDENCE example shows the caret should sit on the operator itself.
  Impact: Diagnostic UX regression in a code path that already has a regression test; trivial fix.
  Required plan update: Update `binary.rs:142` to emit the span on the operator token.
  Basis: direct_file_inspection. Confidence: high. (Codex-only finding — no gemini counterpart.)
```

### Resolution format — always preserve the reviewer tag

When a finding is fixed (Step 7b), mark the entry `[x]` and append a `Resolved:` line referencing the code fix. For agreement findings, both halves are resolved together — the second resolution can reference the first rather than duplicating the fix description:

```md
- [x] `[TPR-04-001-codex][high]` ...
  Resolved: Fixed on 2026-04-07 in commit abc123. Added `dec` emission in `lower_break` early-exit branch; verified via `ORI_CHECK_LEAKS=1 cargo t -p ori_arc iter_break`.
- [x] `[TPR-04-001-gemini][high]` ...
  Resolved: Fixed on 2026-04-07 in commit abc123. Same fix as [TPR-04-001-codex] (agreement).
```

**NEVER delete a resolved finding.** Mark it `[x]` with a resolution note — deletion erases the audit trail and invites re-filing by the next review pass.
