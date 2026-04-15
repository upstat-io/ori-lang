# Phase 4 — Third-Party Cross-Check

Read by a Sonnet sub-agent dispatched from `/impl-hygiene-review`. Not a registered skill. Dispatches `/tp-help` or `/tpr-review` to probe blind spots and validate ambiguous findings from Phase 3. Orchestration-only — the cross-check itself runs inside the external reviewer contexts.

Consumes `/tmp/impl-hygiene-{run}/phase-3.json`. Writes `/tmp/impl-hygiene-{run}/phase-4.json` with: cross-check outcome, any new findings surfaced by reviewers, any Phase 3 findings reviewers rejected.

---

### Phase 4: Third-Party Cross-Check

**MANDATORY for full project mode. Recommended for all other modes.**

After Phase 3 agents return their findings, use `/tp-help` to cross-check the work. This creates a **three-brain review**: you found the patterns, now BOTH Codex AND Gemini independently validate them and look for what you missed. `/tp-help` is dual-source concat mode — a single call returns Codex + Gemini responses concatenated with attribution sentinels. Silently ignoring one reviewer's half of the response is a contract violation.

**Trust tiers (per the global reviewer-grounding rule):**
- **Codex** — HIGH trust: spot-check findings against actual code, move on if they hold
- **Gemini** — LOWER trust: confabulation-prone; independently verify EVERY claim against actual code before acting. Gemini is valuable for catching angles Codex missed, not as an authoritative source.
- The `/tp-help` prompt MUST instruct both reviewers to read `CLAUDE.md` and all `.claude/rules/*.md` (especially `impl-hygiene.md`) FIRST before reviewing.

#### 4a. Validate Findings

Invoke `/tp-help` with a focused question. Pass a summary of 5-10 of the most significant findings (not all — pick the ones that are most ambiguous or architecturally significant) and ask both reviewers to validate:

```
/tp-help BEFORE reviewing, read CLAUDE.md and .claude/rules/impl-hygiene.md. I'm running a hygiene review of [scope]. Here are my top findings — validate whether these are real violations or false positives, and tell me if I'm missing anything obvious in these areas:

[List of 5-10 findings with file:line and brief description]

Key files involved: [list the main files]
```

**What to do with the response (evaluate Codex and Gemini INDEPENDENTLY first, then look for cross-reviewer patterns):**

Per-reviewer evaluation — for Codex AND Gemini separately:
- If the reviewer confirms a finding: verify the confirmation against code (spot-check for Codex, full verification for Gemini per trust tier), then increase confidence and keep the finding
- If the reviewer challenges a finding: re-read the code, check if you misunderstood the pattern. Update or drop the finding ONLY if code verification shows the challenge is correct
- If the reviewer surfaces NEW findings you missed: verify each one against actual code, then add the verified ones to the findings list

Cross-reviewer pattern analysis:
- **Both reviewers confirm the same finding**: highest-signal agreement — lock in, prioritize in severity calibration
- **Both reviewers challenge the same finding**: STRONG signal you misread the pattern — re-verify against code before dropping
- **Reviewers disagree with each other on the same finding**: investigate deeper — read the code end-to-end, determine which framing holds, and do NOT silently pick the answer you prefer
- **One reviewer surfaces a finding the other missed**: treat as valid after your own code verification — Gemini often catches angles Codex doesn't and vice versa (that's the whole point of dual-source)

#### 4b. Probe Blind Spots

After validating findings, use `/tp-help` again to probe areas you might have under-examined. Ask both reviewers to look at a specific area you didn't go deep on:

```
/tp-help BEFORE reviewing, read CLAUDE.md and .claude/rules/impl-hygiene.md. I reviewed [scope] and found [N] findings, but I'm worried I may have missed algorithmic duplication in [specific area]. Can you compare [file A] and [file B] structurally and tell me if their control-flow skeletons are duplicated?
```

Or for cross-backend duplication:

```
/tp-help BEFORE reviewing, read CLAUDE.md and .claude/rules/impl-hygiene.md. Compare the eval path for [feature] in [eval file] with the LLVM codegen path in [llvm file]. Are these maintaining parallel dispatch tables that should be unified?
```

Read BOTH reviewers' sections of the concatenated response in full — do not skim one to "confirm" the other.

**When to probe:**
- Any crate that yielded zero findings (suspiciously clean — likely under-examined)
- Cross-backend code (eval ↔ LLVM) — hardest to catch because it requires reading two codebases in parallel
- Large match/dispatch functions — easy to skim past structural duplication when arms look "different enough"
- Code paths you traced superficially (read the entry point but not the helpers)

#### 4c. Integrate Cross-Check Results

Merge BOTH reviewers' validated and newly-surfaced findings back into the main findings list. Tag findings that any reviewer confirmed with `[TP-CONFIRMED-codex]`, `[TP-CONFIRMED-gemini]`, or `[TP-CONFIRMED-both]` (when both independently confirmed it). Tag findings that either reviewer surfaced with `[TP-SURFACED-codex]` or `[TP-SURFACED-gemini]` — attribution matters both for prioritization and for the severity bump in §5c. Per trust tiers: every Gemini-originated claim (confirmed or surfaced) must be verified against actual code before being integrated — do NOT pass through unverified Gemini claims.

