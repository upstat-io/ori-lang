# `/tpr-review` + `/tp-help` — Design Notes and Improvement Log

**Purpose of this file.** This is the institutional memory for the 2026-04-16 rewrite of `/tpr-review` and `/tp-help`. It captures the **design philosophy** (so future edits don't regress the architecture), the **load-bearing invariants** (things you must NOT change without very good reason and a concrete plan), and a **running log of improvements and bugs** found during real use.

**Context.** On 2026-04-16 the dual-TPR surface was rewritten from ~12,800 lines (envelope/polling/merger/sub-agent-triage infrastructure) to ~850 lines using a main-context orchestrator + parallel-Agent dispatch pattern. See `plans/use-plan-tool-to-sprightly-cat.md` for the approved plan (held at `/home/eric/.claude/plans/use-plan-tool-to-sprightly-cat.md`).

**When to update this file.** Any time you encounter a bug, surprise, or improvement idea while running `/tpr-review`, `/tp-help`, or any of their consumers (`/fix-bug`, `/review-work`, `/review-plan`, `/roadmap-work`, `/rosetta-test`, `/continue-roadmap`). Add a `- [ ]` item under §Improvement Log. When you find design-violating behavior in the wild, add it to §Regressions To Watch For.

---

## §1 — Core Design Philosophy (KEEP THIS)

1. **Main-context orchestrator.** `/tpr-review`'s round loop runs in the caller's main conversation. No `context: fork`. Every tool call, every finding, every edit is visible to the user in real time. The old system hid this behind envelope/polling/triage-sub-agent; the rewrite's whole point was visibility.

2. **Parallel Agent dispatch per reviewer.** Each round: ONE assistant message with TWO `Agent({subagent_type: "general-purpose", model: "sonnet", prompt: <filled tp_agent_prompt.md>})` calls. They run concurrently per https://code.claude.com/docs/en/sub-agents. Wall-clock = `max(codex, gemini)`. No per-tool completion callbacks (undocumented); treat results as batch-complete.

3. **Plain-text transport.** Reviewers emit a fenced `<<<TPR-REPORT … TPR-REPORT>>>` block at the end of their CLI output. The orchestrator parses it inline. No JSON envelope, no schema, no merger script, no polling, no background process.

4. **Grounding by reference, not by embedding.** The inner prompt passed to codex/gemini tells the CLI to `ls .claude/rules/*.md` and `cat` each one itself. Do NOT embed `CLAUDE.md` + 28 rule files into the prompt argv — that's ~700KB and blows past Linux `ARG_MAX` (~128KB) with `Argument list too long`.

5. **Verification-against-code discipline.** Every reviewer finding is a **hypothesis**, not a fact. The orchestrator (main context) reads the cited file, confirms the quote exists verbatim, and drops unverifiable findings before classification. Trust tier sets verification **depth**, not pass/fail:
   - Codex HIGH → spot-check (read ±20 lines of cited range)
   - Gemini LOWER → full-verify (read cited file in full, trace behavior end-to-end)

6. **One main-context edit path.** When actionable findings land, the orchestrator edits code directly. No hand-off to a fix sub-agent, no separate edit phase. Feedback loop stays tight and visible.

7. **No `@`-includes in the new skill files.** `@`-includes are undocumented per the Claude Code skills reference. The rewrite deliberately avoided them. Exception: `compose-intel-summary.md` is still `@`-included by 20 OTHER consumers — that's legitimate pre-existing usage outside the rewrite's scope.

8. **Per-invocation unique scratch dirs.** Use `mktemp -d -t <prefix>-XXXXXXXX` in reviewer sub-agents. Hard-coded `/tmp/...` subdirs COLLIDE across parallel Claude sessions (the user routinely runs concurrent sessions). This is load-bearing — see §Regressions To Watch For.

9. **CLAUDE.md-compliant timeouts.** `timeout: 2700000` (45 min) on the foreground Bash call to the reviewer CLI. The `.claude/hooks/block-banned-commands.sh` hook blocks any `timeout` under `1200000` ms on codex/gemini commands. Never set `600000` or lower.

10. **Always-on spec/grammar gate.** `/tpr-review` §3 runs in ALL modes (work / plan / custom-objective). The rule it enforces — `.claude/rules/spec.md §Enforcement` — has no mode exemption. The gate is cheap (one git diff + one ls) and gating it by mode creates a coverage hole that custom-objective reviews can drive through.

## §2 — Load-Bearing Invariants

Changing any of these without a concrete plan risks re-introducing the bugs the dogfood runs surfaced. Re-read §4 ("Lessons from the dogfood runs") before changing.

| # | Invariant | Why (which failure mode it prevents) |
|---|-----------|--------------------------------------|
| I1 | Per-invocation `mktemp -d` scratch dirs | Parallel-session collision; user runs concurrent sessions routinely |
| I2 | Foreground Bash, `timeout: 2700000` | Hook-blocked under 1,200,000 ms; `run_in_background` breaks the return-in-same-turn contract |
| I3 | NO embedded CLAUDE.md/rules in reviewer prompt | ARG_MAX (~128KB) causes `Argument list too long` on Linux |
| I4 | `<<<TPR-REPORT … TPR-REPORT>>>` sentinels exact | Orchestrator parser is sentinel-based; changing them breaks extraction |
| I5 | Single assistant message, two Agent calls | Parallel-concurrency pattern; multiple messages serialize them |
| I6 | Orchestrator inherits caller's model, not pinned | Callers use Opus for judgment; skill-level `model:` is undocumented binding |
| I7 | Sub-agents dispatched with `model: "sonnet"` explicitly | Documented Agent-tool override; keeps reviewer dispatch cheap |
| I8 | Spec gate runs in ALL modes (not mode-gated) | Custom-objective reviews can still touch spec; `spec.md §Enforcement` has no exemption |
| I9 | `ever_verified_findings` accumulates across all rounds for plan-mode frontmatter write | Last-round-only write loses earlier-round findings |
| I10 | Render round summary AFTER `fix_and_commit`, BEFORE state-branching | The render contract mandates `Fix commit: {sha}`; pre-fix render has no sha |
| I11 | Banned-response phrases enforced in both orchestrator and reviewer prompt | "pre-existing", "out of scope", etc. — reasoning out of findings is banned per CLAUDE.md §One Rule |
| I12 | Verify findings BEFORE meta-classification | Can't classify a finding as meta if its quote doesn't exist — that's confabulation, drop it |
| I13 | User-facing choice points use `AskUserQuestion`, never plain-text numbered options | Prose options look identical to round-summary narration; the harness's structured-choice UI only engages on `AskUserQuestion` — prose asks force the user to type and invite them to skip reading. User flagged this after post-Round-1 dogfood. |

## §3 — File Inventory (canonical)

Active files (2026-04-16):

| Path | Lines (~) | Role |
|------|-----------|------|
| `.claude/skills/tpr-review/SKILL.md` | 300 | Main orchestrator contract (13 sections) |
| `.claude/skills/tpr-review/tp_agent_prompt.md` | 180 | Reviewer sub-agent prompt template |
| `.claude/skills/tp-help/SKILL.md` | 160 | Help orchestrator with parallel Agent dispatch |
| `.claude/skills/tp-help/tp_help_prompt.md` | 150 | Help sub-agent prompt template |
| `.claude/skills/review-work/SKILL.md` | 50 | Thin delegator to `/tpr-review` |
| `.claude/skills/query-intel/compose-intel-summary.md` | 280 | Intel-query SSOT (moved from `dual-tpr/`) — used by 20 OTHER consumers |

Deleted on 2026-04-16 (~62 files total):

- `.claude/skills/dual-tpr/` — entire tree (30 scripts, 21 fixtures, 7 docs, `__pycache__`)
- `.claude/skills/tpr-review/step-1-round-setup.md`, `step-2-round-triage.md`, `step-3-final-report.md`
- `.claude/skills/tp-help/workflow.md`

## §4 — Lessons from the Dogfood Runs (2026-04-16)

Three rounds of `/tpr-review` run against the rewrite itself. **14 real issues surfaced** across the rounds. Capture these so future changes don't re-introduce them.

### Round 0 findings (codex 6, gemini clean)

1. **Spec gate short-circuited to `AskUserQuestion`** instead of emitting a CRITICAL TPR finding through normal flow. Violated `spec.md §Enforcement`. Fixed: §3 now emits a synthetic finding.
2. **Round summary rendered BEFORE `fix_and_commit`**, so the mandatory `Fix commit: {sha}` field was always empty. Fixed: pseudocode reordered (fix → render → branch).
3. **Plan-mode frontmatter write** only passed the LAST round's `verified` list, losing earlier-round findings. Fixed: added `ever_verified_findings` accumulator.
4. **`/tp-help` §6 contradicted §7** — "Claude's interpretation" in §6 vs "No synthesis" in §7. Fixed: §6 rewritten as raw-plus-attribution-only.
5. **`compose-intel-summary.md` header said "24 consumers"** after the file was moved but the count was stale (actual: 20). Fixed: recounted + updated.
6. **`review-work/SKILL.md` said "12-section orchestrator contract"** but actual is 13. Fixed.

### Round 1 findings (codex 4, gemini 4, plus user-flagged critical)

7. **`/tmp/tpr-inner/` and `/tmp/tphelp-inner/` race across parallel Claude sessions** (user-flagged, critical). Fixed: `mktemp -d -t <prefix>-XXXXXXXX` per-invocation.
8. **`timeout: 600000` (10 min) violates CLAUDE.md REVIEW/AGENT TIMEOUTS** (forbids <1,200,000 ms). Both reviewers agreed. Fixed: raised to `2700000` (45 min). **Critical: would have been hook-blocked by `.claude/hooks/block-banned-commands.sh` — why the first dispatches "succeeded" anyway is still partially a mystery; the sub-agents may have auto-promoted to background. Future bug to watch for.**
9. **Spec gate skipped for custom-objective mode** — custom reviews that touch spec files bypassed the mandatory gate. Fixed: §3 now "runs in ALL modes".
10. **Stale consumer rosters at `compose-intel-summary.md:247` and `:109`** still referenced dropped consumers. Fixed: collapsed to single 20-consumer roster.

### Round 2 findings (codex 5, gemini clean)

11. **Explanatory prose still cited `/tmp/tpr-inner/` literal** after the mktemp fix. Fixed: reworded to "Shared `/tmp` subdirectories".
12. **Step 3 text still said "10-minute timeout"** in tp_help_prompt.md after the fix raised it to 45. Fixed.
13. **Example failure text said "Bash 600s timeout exceeded"** in tp_agent_prompt.md. Fixed.
14. **Step F registry still listed `/tp-help` as active SSOT consumer** after it dropped its `@`-include. Fixed: marked as dropped with historical note.

### Cross-round patterns (generalizable)

- **Architectural fixes land cleanly; doc drift lingers.** Round 0 fixed architecture (gemini clean). Round 1 caught the *next* architectural issues (gemini agreed on timeout/spec-gate). Round 2 found only doc-text drift (gemini clean again). The sequence of findings tracks the code's actual maturation.
- **Gemini LOWER-trust is worth it.** Gemini was clean 2/3 rounds, which is a strong convergence signal rather than a weakness. When both reviewers agree on a finding, it's almost certainly a real architectural issue. When only codex flags, it's usually doc consistency.
- **Trust-tier aware verification is load-bearing.** Codex cited accurate line numbers in 100% of findings across 3 rounds. Gemini's citations (when it had any) also matched — but the LOWER-trust discipline of "re-read file in full" caught drift in the round-1 timeout issue that codex's spot-check alone would have missed on a different codebase.
- **Doc-string drift is the long tail.** After the architectural fixes, the remaining findings (rounds 2) were all doc text that referenced the OLD behavior (10-min timeout, old `/tmp` path). When fixing an architectural invariant, grep the ENTIRE file for references to the old value and update them all at once.

## §5 — Regressions To Watch For

When editing `tp_agent_prompt.md`, `tp_help_prompt.md`, `tpr-review/SKILL.md`, or `tp-help/SKILL.md`, check for these in order:

- `- [ ]` Any `/tmp/tpr-inner/` or `/tmp/tphelp-inner/` hard-coded literal → should be `$(mktemp -d -t ...-XXXXXXXX)`
- `- [ ]` Any `timeout: 600000` (or <1,200,000) → should be `2700000`
- `- [ ]` Any "10-minute timeout" / "10 min" in prose → should be "45-minute" / "45 min"
- `- [ ]` Any "Bash 600s" / "600s timeout" in example text → should be "2700s" / "45 min"
- `- [ ]` Any `cat CLAUDE.md >> prompt.md` or loop over rule files embedding content → DELETE; tell CLI to read them itself
- `- [ ]` Any `run_in_background: true` on the reviewer Bash call → REMOVE; must be foreground
- `- [ ]` Any "context: fork" in `tpr-review/SKILL.md` frontmatter → REMOVE; orchestrator runs in main context
- `- [ ]` Any `@.claude/skills/dual-tpr/...` reference anywhere → path is dead; the whole directory was deleted
- `- [ ]` Any `@.claude/skills/` in the new tpr-review or tp-help files → the new files deliberately have NO `@`-includes; inline the policy
- `- [ ]` Any `model:` in `tpr-review/SKILL.md` or `tp-help/SKILL.md` frontmatter → REMOVE; skill-level binding is undocumented; sub-agents use Agent-tool `model: "sonnet"` instead
- `- [ ]` Any render-before-fix ordering in `§5 round loop pseudocode` → fix must come first (Round 0 F2)
- `- [ ]` Any "work mode and plan mode only" on the spec gate → should be "runs in ALL modes" (Round 1 F9)

## §6 — Improvement Log

Add items here as you encounter them in real use. Format: `- [ ] [priority] <description>. Context: <where/when you hit it>. Suggested fix: <one sentence>.`

Priorities: `p0` (blocks all future runs), `p1` (frequent user-visible bug), `p2` (nice-to-have), `p3` (cosmetic).

### Open items

- [ ] **[p1] Finish skill cleanup: remove remaining timeframe + model-editorial + doc-URL references from skill files.** Context: user flagged on 2026-04-16 that timeframes and model-policy commentary in skills bias judgment toward skipping process, and doc-URL citations ("per https://code.claude.com/docs/...") + "is undocumented per docs" rationale are meta-documentation that muddies instructions. First pass cleaned the most visible surfaces (all skill `description` frontmatter fields that show in the skills list; `/tpr-review`, `/tp-help`, `/verify-tpr`, `/fix-bug` runtime-expectation lines; `/impl-hygiene-review`, `/roadmap-work`, `/review-plan` descriptions + top sections; both prompt templates rewritten clean). Remaining work: (a) `fix-bug/SKILL.md` body paragraphs still carry "(Sonnet)"/"(Opus)" phase labels at ~lines 25, 27, 33, 114, 116, 120, 464, 465; (b) `fix-next-bug/SKILL.md` line 23 "entirely in the parent (Opus) context"; (c) `create-plan/SKILL.md` lines 66-97 contain a whole model-policy assignment table that is essentially editorial; (d) `improve-tooling/SKILL.md` has "10 minutes" heuristics at lines ~263, 296, 343; (e) `impl-hygiene-review/SKILL.md` body has "(Sonnet)"/"(Opus)" phase labels beyond the description; (f) `roadmap-work/SKILL.md` body has "Opus"/"Sonnet" commentary at lines 140, 141, 158; (g) any remaining `https://code.claude.com/docs/...` URL citations; (h) `CLAUDE.md §Commands — REVIEW/AGENT TIMEOUTS` paragraph has wall-clock rationale ("20-45 min in practice", "cold-starts of 8-10 min") that should be trimmed to just the hook's numeric bounds. Rule of thumb: skills contain instructions only ("do X", "MUST Y"); rationale belongs in this design log. `model:` in Agent dispatch code blocks is a technical parameter and stays; `model:` in skill frontmatter is a harness directive and stays.

- [ ] **[p2] Sub-agent status visibility during long reviews.** Context: during round 0's first run, the sub-agent returned "Waiting for the codex review to complete" as an intermediate message and the actual TPR-REPORT never made it back (BUG-B in the dogfood). The fix was to mandate foreground Bash + `timeout: 2700000`, but this leaves no mid-stream visibility during 20+ min reviews. The old design had 5-min polling with `status-check.sh`; the new design has none. Consider: (a) allow the sub-agent to emit intermediate tool_use messages showing CLI events, or (b) have the orchestrator spawn a lightweight "tail the CLI jsonl stream" background task to render heartbeats. Both need prototyping.

- [ ] **[p3] Reviewer transcript persistence.** Context: the old `/tpr-review` persisted per-round artifacts to `{run_id}/round-{N}/` for post-hoc forensic review. The new design is transcript-only. If the user ever wants to retroactively inspect "what did gemini actually say in round 1 of that bug fix two weeks ago?", they can't — it's gone with context compaction. Consider: optional `ORI_TPR_ARCHIVE=1` env var that writes each round's raw CLI stdout to `$HOME/.cache/ori-tpr-archive/<date>/round-N/<reviewer>.stdout`.

- [ ] **[p2] Convergence heuristic to exit early.** Context: the current design caps at 5 iteration rounds + 2 meta-only rounds. The old design had a 5-condition "convergence gate" (§6c.1). We dropped it in favor of simpler caps. If in practice rounds 2+ consistently find only doc-hygiene residue (as happened during the dogfood), consider adding back: "exit after round N if all findings are LOW severity + strictly decreasing trajectory + category in {wording, cosmetic}". Only add this if the current caps produce obvious waste.

- [ ] **[p1] `/tp-help` has never been dogfooded with the new pattern.** Context: this rewrite was run through `/tpr-review` three times but `/tp-help` was not directly invoked. The first real `/tp-help` call (via `/fix-bug` Phase 1.75 or ad-hoc) will be the first live test. Suggested fix: on first invocation, watch for the same BUG-A (ARG_MAX) and BUG-B (sub-agent return contract) patterns; if seen, reuse the same fixes as `/tpr-review`.

- [ ] **[p3] `plans/use-plan-tool-to-sprightly-cat.md` lives outside the project** (in `/home/eric/.claude/plans/`). Reviewers get a 404 when instructed to read it as part of the objective. The objective prompts currently say "may 404; skip" but this is fragile. Consider: copy the plan file into the project tree at `plans/rewrites/tpr-review-2026-04-16.md` for local reference, OR stop referencing it in objectives and describe the rewrite inline.

- [ ] **[p2] Spec gate only checks `HEAD`, not staged/unstaged variants.** Context: `.claude/skills/tpr-review/SKILL.md §3` uses `git diff --name-only HEAD`. If a spec file is ONLY modified in the unstaged working tree (not yet added), this diff catches it. But if spec is staged only (added but not committed), does the diff still catch it? Verify with a targeted test; may need `git diff --name-only HEAD --cached --staged` or similar.

- [ ] **[p3] No plan-TPR round-aggregation test.** Context: the F3 fix (`ever_verified_findings` accumulator) has never been exercised with a multi-round plan review that had findings in round 1 but cleaned up by round 3. The plan-mode frontmatter write happens after the loop, consuming `ever_verified_findings` — untested pathway.

### Recently closed (retrospective record)

- [x] **2026-04-16 — Round 0 F1:** spec gate short-circuit → fixed via synthetic finding in §3
- [x] **2026-04-16 — Round 0 F2:** render-before-fix ordering → fixed via pseudocode reorder
- [x] **2026-04-16 — Round 0 F3:** plan-mode loses earlier-round findings → fixed via `ever_verified_findings`
- [x] **2026-04-16 — Round 0 F4:** tp-help synthesis contradiction → fixed by rewriting §6
- [x] **2026-04-16 — Round 0 F5:** SSOT consumer count stale → recounted to 20
- [x] **2026-04-16 — Round 0 F6:** 12 vs 13 section reference → fixed
- [x] **2026-04-16 — Round 1 USER-flagged:** scratch-dir collision → fixed via `mktemp -d`
- [x] **2026-04-16 — Round 1 F7/F8:** timeout 600000 → raised to 2700000
- [x] **2026-04-16 — Round 1 F9 (codex):** spec gate mode-gated → now always-on
- [x] **2026-04-16 — Round 1 F9/F10 (gemini) + F10 (codex):** stale consumer rosters → collapsed
- [x] **2026-04-16 — Round 2 F1–F5:** doc-consistency drift (prose literals, 10-min/600s, Step F registry) → fixed
- [x] **2026-04-16 — Post-rewrite dogfood USER-flagged:** orchestrator emitted prose "1. Continue / 2. Exit / 3. Abort" options at post-Round-1 inflection point instead of using `AskUserQuestion`. User correctly flagged that structured choice points must use the harness's structured-choice UI, not plain prose bullets that look identical to narration. Fixed via new SKILL.md §11.5 "User-interaction discipline (MANDATORY)" enumerating five choice points (post-round inflection, ambiguous input, transport-failure retry, spec-gate resolution, cap-exit escalation) that require `AskUserQuestion`; prose summaries explicitly allowed only for informational renders where the next assistant turn is identical regardless of user reaction. Load-bearing invariant added as I13. Design philosophy principle 11 added.

## §7 — How To Use This File In Future Sessions

**On every surprise.** When you run `/tpr-review` or `/tp-help` and something unexpected happens (failure mode, bad output, confusion about what the skill is doing), come here first. Check §Regressions To Watch For — is the surprise a known regression pattern? Check §Improvement Log — is it already filed? If neither, add it.

**Before editing any of the 6 canonical files in §3.** Re-read §Load-Bearing Invariants. Every invariant there is defended by a specific failure mode from the dogfood runs. Breaking an invariant without understanding its failure mode means re-running the dogfood to re-discover what this file already records.

**When `/fix-bug` or `/tpr-review` on a real bug fails.** If the failure is in the *skill* (not the bug being fixed), log it here with the specific invocation that triggered it. This is the correct home for skill-level bugs — it's a file dedicated to this skill pair, inside the improve-tooling skill folder (which itself is the canonical home for "improve the tooling, don't work around it" per `CLAUDE.md §Continuous improvement everywhere`).

**When the design evolves.** If you change §1 Core Design Philosophy or §2 Load-Bearing Invariants, update both SKILL.md files to match AND add a dated entry under §4 Lessons documenting why the invariant changed. Don't silently mutate the design — the whole point of this file is that the design doesn't get lost.
