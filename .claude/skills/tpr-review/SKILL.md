---
name: tpr-review
description: "Dual-source third-party review (codex + gemini) in parallel, with verification-against-code, iterative fix-and-re-run until both reviewers return clean. Reviews code, plans, skills, docs, or any custom objective. TRIGGER proactively after completing ANY non-trivial work: bug fixes, features, refactors, multi-file changes, compiler changes, codegen changes, test additions, plan implementations, or anything touching correctness-sensitive code. When in doubt, run it."
allowed-tools: Read, Write, Edit, Bash, Glob, Grep, Agent, AskUserQuestion, Skill
---

# Dual-Source Third-Party Review (Codex + Gemini)

`/tpr-review` dispatches the **Codex** and **Gemini** CLIs as parallel sub-agents per round, reads their plain-text `<<<TPR-REPORT>>>` output directly in the main context, verifies every finding against actual code, edits code to fix accepted findings, and re-runs until both reviewers return clean or a stop gate fires. The orchestrator runs in the caller's main context — there is no `context: fork`, no envelope layer, no polling loop. Per-round summaries are printed directly to the user before any state-branching decision.

## §1 — Invocation surface

`ARGS` selects one of three modes:

- **Work mode (default)** — empty `ARGS` or `ARGS == "--skill review-work"`. Review the current working-tree changes. Scope = `git diff HEAD~5`.
- **Plan mode** — `ARGS` begins with `--skill review-plan <section-path>`. Review the named plan section for design/spec/implementation coherence. Findings file into the section's `third_party_review` frontmatter per §10.
- **Custom-objective mode** — any other `ARGS` value. The entire ARGS string becomes the review objective. Scope is inferred from the objective or passed as `--scope <path>` after the objective text.

Mode detection is mechanical: inspect the first token of `ARGS`. Ambiguity is an input error — `AskUserQuestion` rather than guess.

## §2 — Mandatory grounding (orchestrator)

Before round 1, the orchestrator reads its own grounding:

```
Read: CLAUDE.md
Bash: ls .claude/rules/*.md
Read: every file the ls enumerated
```

The `ls` output is the authoritative rule manifest for this invocation. Do not hand-select. Do not `@`-include (not a documented skills-file syntax per https://code.claude.com/docs/en/skills — inline the reads instead).

Reviewers perform their own grounding inside their sub-agents (see `tp_agent_prompt.md` §Step 1). Grounding is duplicated deliberately: reviewer sub-agents have fresh context, and the orchestrator's grounding is load-bearing for finding verification (§4).

## §3 — Spec/grammar gate (pre-round-1)

**Runs in ALL modes.** Custom-objective reviews can still touch spec/grammar (e.g., "review these spec edits"); the gate is cheap (one `git diff` + one `ls` of approved proposals) and the rule it enforces (`spec.md §Enforcement`) has no mode exemption. Running it universally closes the custom-objective coverage gap.

Before dispatching round 1, unconditionally:

```
Bash: git diff --name-only HEAD -- docs/ori_lang/v2026/spec/ docs/ori_lang/v2026/spec/grammar.ebnf
```

If the output is **non-empty**, check `docs/ori_lang/proposals/approved/` for a proposal whose body mentions any of the touched files. If no matching proposal is found, per `.claude/rules/spec.md §Enforcement` (`/tpr-review` MUST flag any spec/grammar diff without a proposal reference as a **CRITICAL** finding), the orchestrator emits a synthetic pre-round-0 finding that enters the normal round flow:

```
synthetic_finding = {
  id: "SPEC-GATE-001",
  severity: "critical",
  path: "<first spec/grammar file touched>",
  line: 1,
  title: "Spec/grammar modified without approved proposal",
  evidence: "<git diff output listing all touched spec/grammar files>",
  rule_violated: ".claude/rules/spec.md §Enforcement",
  recommended_fix: "Either revert the spec/grammar changes OR create + approve a proposal under docs/ori_lang/proposals/approved/ and reference it in the commit message"
}

# Prepend to round-0 verified set BEFORE dispatching reviewers. The
# finding is already verified-by-construction (git diff is ground truth)
# and is NEVER meta (spec invariant).
```

The finding is treated like any other critical actionable finding per §7: the user must fix it (revert or approve a proposal) before round 0's reviewers can complete their work. Because it is CRITICAL and NOT meta, the normal fix-or-plan-or-AskUserQuestion dispositions apply — in practice the orchestrator will typically need `AskUserQuestion` for the user's revert/proposal decision, but that decision point is now inside the documented finding-handling flow, not a skill-level short-circuit.

## §4 — Trust tiers (orchestrator-side verification posture)

Every finding — codex or gemini — is **verified against actual code** before acting. The reviewer's claim is a hypothesis. The trust tier sets verification depth, not pass/fail.

- **Codex — HIGH trust.** Codex tends to cite accurate paths and lines. For each codex finding: Read the cited file around the cited line (±20 lines), confirm the quoted evidence exists verbatim. If it matches, accept the finding for classification (§6). If the quote doesn't match, drop the finding silently (mis-cite is rare but invalidates the claim).
- **Gemini — LOWER trust.** Gemini is prone to confabulation: invented line numbers, misquoted code, "positive observations" reframed as findings. For each gemini finding: Read the cited file IN FULL (not just ±20 lines), trace the code path end-to-end, confirm the claimed behavior matches what the code actually does. Gemini citations to external URLs are never authoritative — verify the underlying claim independently. Drop any finding that fails verification.

Verification happens BEFORE meta/actionable classification (§6). An unverified finding never reaches the classifier.

## §5 — Round loop state machine

Two counters bound the loop:

- `iteration_counter` — number of finding-fixing rounds completed. Cap: **5**.
- `meta_only_streak` — consecutive rounds where every verified finding was meta (§6). Cap: **2**.

Stop conditions (in order of check):

1. **Consensus** — both reviewers returned `status: clean` AND `verified` is empty → exit clean.
2. **Meta-only streak cap** — `meta_only_streak == 2` → exit ("juice not worth the squeeze").
3. **Iteration cap** — `iteration_counter == 5` → exit (escalate via summary).
4. **Both-reviewer transport failure twice** — handled in §9, escalates via `AskUserQuestion`.

Pseudocode:

```
iteration_counter = 0
meta_only_streak = 0
last_actionable_count = None
ever_verified_findings = []         # §10: accumulate across ALL rounds for
                                    # plan-mode frontmatter write. Using the
                                    # last round's verified list would lose
                                    # findings from earlier rounds that were
                                    # already fixed inline.

while iteration_counter < 5 and meta_only_streak < 2:
    template = Read(".claude/skills/tpr-review/tp_agent_prompt.md")

    codex_prompt  = fill(template, REVIEWER=codex,  TRUST_TIER=HIGH,
                         OBJECTIVE=OBJ, SCOPE=SCOPE)
    gemini_prompt = fill(template, REVIEWER=gemini, TRUST_TIER=LOWER,
                         OBJECTIVE=OBJ, SCOPE=SCOPE)

    # ── SINGLE assistant message with TWO Agent tool calls — see §8 ──
    [codex_out, gemini_out] = dispatch_parallel(codex_prompt, gemini_prompt)

    codex_report  = parse_tpr_report(codex_out)
    gemini_report = parse_tpr_report(gemini_out)

    if codex_report.status == "failed": codex_report  = retry_or_survivor(codex)
    if gemini_report.status == "failed": gemini_report = retry_or_survivor(gemini)

    all_findings = codex_report.findings + gemini_report.findings
    verified     = [f for f in all_findings if verify_against_code(f)]  # §4
    meta         = [f for f in verified if classify_meta(f)]             # §6
    actionable   = [f for f in verified if f not in meta]
    ever_verified_findings.extend(verified)                              # §10 accumulator

    # ── Fix FIRST, then render summary — the render contract (§11)
    #    requires a populated `Fix commit: {sha}` field, which is only
    #    available after fix_and_commit returns. Render before the
    #    exit/continue state-branching decisions below, not before fix.
    commit_sha = None
    if len(actionable) > 0:
        commit_sha = fix_and_commit(actionable)                          # §7
        meta_only_streak = 0
        last_actionable_count = len(actionable)
    elif len(verified) > 0:                                               # meta-only round
        meta_only_streak += 1
        last_actionable_count = 0

    print_round_summary(iteration_counter, codex_report, gemini_report,
                        verified, meta, actionable, commit_sha)           # §11

    # ── State-branching decisions AFTER the render ──
    if len(verified) == 0 and codex_report.status == "clean" and gemini_report.status == "clean":
        exit_reason = "clean"
        break

    iteration_counter += 1

else:
    exit_reason = "meta_cap_reached" if meta_only_streak >= 2 else "iter_cap_reached"

emit_final_report(exit_reason, iteration_counter, last_actionable_count)

if mode == "review-plan":
    write_plan_frontmatter(section_path, exit_reason,
                           ever_verified_findings)                        # §10
```

## §6 — Meta-only classification checklist

Runs AFTER verification (§4). A finding is **meta** if and only if ALL of the following apply:

- Its category is purely one of: wording/phrasing, cosmetic/formatting, already-documented-elsewhere, or exact duplicate of a prior-round finding.
- It does NOT touch: correctness, invariants (AIMS / SSOT / phase boundaries / registry drift), tests, security, spec conformance, error paths, API contracts, memory safety.
- Its `recommended_fix` is a pure-doc edit, a rename, or a whitespace change.

Any doubt → NOT meta. The classifier is intentionally conservative — one extra round of fixing real issues is cheaper than a missed invariant violation.

`meta_only_streak` increments ONLY when the entire verified-findings set of the round is meta. A single non-meta finding in the set resets the streak to 0.

## §7 — Finding-handling policy (ABSOLUTE)

Tied directly to `CLAUDE.md §The One Rule` — correctness above all other concerns. There is NO circumstance under which the orchestrator may dismiss, rationalize, scope-note, or defer an actionable finding. The ONLY valid dispositions are:

1. **Fix it NOW** — edit code in the main context, run affected tests, commit the fix via `/commit-push`.
2. **Create a plan and execute it** — if too large for inline fix, run `/create-plan` and implement the resulting sections. No "tracked for later" without an anchor.
3. **`AskUserQuestion`** — genuinely blocked on a user decision or missing domain knowledge.

**BANNED response phrases** (using ANY is a violation; the orchestrator MUST NOT generate these in round summaries, commit messages, or plan updates):

- "pre-existing" / "was already broken"
- "architectural limitation" / "requires major refactor"
- "out of scope" / "not a §NN deliverable"
- "conservative/safe" / "only precision loss"
- "not a regression" / "not introduced by this work"
- "future improvement" / "tracked for later"
- "known limitation"

The size of the fix is irrelevant. If correctness requires cross-crate refactoring across 10 files, that IS the work. "Requires architectural change" is not a reason to skip — it IS the assignment per `CLAUDE.md §The One Rule`.

**Filing discipline.** For plan-owned findings (a plan section covers the affected code), append `- [ ]` items to that section's `## {NN}.R Third Party Review Findings` block using the tagged-ID form `[TPR-{NN}-{ordinal}-{reviewer}][severity]`. For unowned findings, file to `plans/bug-tracker/` under the appropriate subsystem using the canonical `BUG-{NN}-{ordinal}` format (NO reviewer suffix — reviewer provenance lives in the body). Agreement findings (same location + title from both reviewers) file ONE bug entry, not two.

## §8 — Parallel dispatch pattern (canonical template)

The documented parallel-sub-agent pattern per https://code.claude.com/docs/en/sub-agents is "multiple tool calls in a single assistant message run concurrently." The orchestrator MUST dispatch both reviewers in one assistant message. Foreground only — do NOT set `run_in_background: true`. Per-tool completion callbacks are not documented; assume batch completion (wall-clock = max(codex, gemini)).

Exact template (fill placeholders before dispatch):

```
# — Single assistant message with TWO Agent tool calls —

Agent({
  subagent_type: "general-purpose",
  model: "sonnet",
  description: "tpr-review codex reviewer round {N}",
  prompt: <contents of tp_agent_prompt.md with {REVIEWER}=codex,
           {TRUST_TIER}=HIGH, {OBJECTIVE}=<obj>, {SCOPE}=<scope>>
})

Agent({
  subagent_type: "general-purpose",
  model: "sonnet",
  description: "tpr-review gemini reviewer round {N}",
  prompt: <contents of tp_agent_prompt.md with {REVIEWER}=gemini,
           {TRUST_TIER}=LOWER, {OBJECTIVE}=<obj>, {SCOPE}=<scope>>
})
```

The `model: "sonnet"` override is a documented field on the Agent tool. Both sub-agents inherit the prompt template verbatim — the only differences are the four substituted placeholders.

## §9 — Failure handling

**Single reviewer returns `status: failed`.** Retry that reviewer once — re-dispatch just the failed Agent (single tool call in a follow-up message; partner already completed). If the retry also returns `status: failed`, proceed in **survivor mode** for this round: use only the surviving reviewer's report, set `survivor_mode: true` in the round summary header, continue the loop normally.

**Both reviewers return `status: failed`.** The round produced nothing usable. Retry ONCE (parallel dispatch again). If both fail a second time, escalate:

```
AskUserQuestion:
  "Both reviewers failed twice. What should I do?"
    1. Retry once more (will cost ~{N} minutes)
    2. Abort this /tpr-review invocation (code is unchanged, no findings filed)
    3. Proceed without review (NOT RECOMMENDED — only use if you will review manually)
```

The orchestrator never silently exits without producing either a round summary or an escalation. If `AskUserQuestion` is interrupted, treat as option 2 (abort).

## §10 — Plan-TPR integration (plan mode only)

When `ARGS` begins with `--skill review-plan <section-path>`, after the loop terminates:

1. **Read** the section file's YAML frontmatter.
2. **Set** `third_party_review.status`:
   - `clean` if `exit_reason == "clean"` and no verified findings occurred in any round.
   - `findings` if any verified findings occurred (even if all were fixed inline).
   - `escalated` if `exit_reason` was `meta_cap_reached`, `iter_cap_reached`, or `both_reviewer_failure`.
3. **Set** `third_party_review.updated` to today's date (YYYY-MM-DD).
4. **For `findings` status** — append each accepted finding as `- [ ]` items under the section's `## {NN}.R Third Party Review Findings` block (create the block if missing). Use the canonical shape from `.claude/skills/verify-tpr/SKILL.md` — `/verify-tpr` is the reader and its input contract is unchanged:

   ```md
   - [ ] `[TPR-{NN}-{ordinal}-{reviewer}][{severity}]` `{path}:{line}` — {title}.
     Evidence: {evidence}
     Impact: {one-line impact summary}
     Required plan update: {recommended_fix}
     Basis: fresh_verification | direct_file_inspection. Confidence: {high|medium|low}.
   ```

   For **agreement findings** (same location + title from both reviewers), file BOTH halves with an `Agreement:` cross-reference line pointing at each other (verify-tpr expects this shape). For single-reviewer findings, file ONE entry noting the reviewer.

5. **Write** via the `Edit` tool. Section `status` stays `in-progress` while `third_party_review.status: findings` (this constraint is owned by `/verify-tpr` and `/continue-roadmap` — do not override).

## §11 — Coordinator rendering contract (MANDATORY)

After EVERY round, the orchestrator MUST print a round summary as a direct assistant message. The render happens AFTER `fix_and_commit` (so the commit sha is available for the mandatory `Fix commit:` field) but BEFORE any state-branching / loop-continuation decisions (exit-clean, meta-cap, iter-cap, round N+1 dispatch). This is the ONLY per-round user-facing surface — there are no persistent artifacts the user can read later in this design.

Required structure:

```md
### Round {N} Summary

**Dispatch**: codex {codex_findings} / gemini {gemini_findings} / survivor_mode: {true|false}
**Verification**: verified {verified_count} / dropped {dropped_count}
**Classification**: actionable {actionable_count} / meta {meta_count}
**Fix commit**: {sha or "none — no actionable findings this round"}

**Findings this round:**
- `[TPR-{NN}-{ordinal}-{reviewer}][severity]` `path:line` — title. Disposition: {fixed in {sha} | handed off to /fix-bug BUG-XX-NNN | classified meta: {reason} | dropped at verification: {reason}}.
- ... one bullet per verified finding (agreement findings produce ONE bullet cross-referencing both reviewer IDs) ...

**Next round will confirm**: {one sentence — what the next round should verify, or "loop exiting {reason}"}.
```

**Rules:**
- Every bullet MUST end with a `Disposition:` line. A bullet without a disposition is a contract violation.
- Agreement findings produce ONE bullet, not two. Cross-reference both reviewer-tagged IDs inline.
- Clean-pass rounds still render the block — `Findings this round:` becomes `(none — both reviewers returned clean)` and `Next round will confirm` becomes `loop exiting clean`.
- Dropped findings (failed verification) appear in the bullet list with `Disposition: dropped at verification: <reason>`. The user sees what reviewers claimed and why it was rejected.
- Keep bullets terse: ≤120 characters per line.

## §12 — Model policy

- **Orchestrator** — inherits the caller's model (no pinning). Typically Opus when invoked from `/fix-bug`, `/roadmap-work`, or `/continue-roadmap`. The skill body does NOT declare a `model:` frontmatter field: skill-level model binding is undocumented per https://code.claude.com/docs/en/skills, and the Opus-for-judgment property is achieved by the caller context, not the skill.
- **Reviewer sub-agents** — pinned to Sonnet via the Agent tool's documented `model` field (https://code.claude.com/docs/en/sub-agents). Dispatch discipline + external CLI wrapping is mechanical enough for Sonnet; reviewer depth comes from the external CLIs (codex, gemini), not the sub-agent wrapper.

## §13 — What this skill does NOT do

- **No envelope, no JSON schema, no merger script.** Reviewers emit plain-text `<<<TPR-REPORT>>>` blocks; the orchestrator parses them inline.
- **No polling, no status-check.sh, no background processes.** Agent dispatches are foreground; results arrive when the tool completes.
- **No cross-session state.** Each invocation starts fresh. Session-resume is not supported.
- **No cross-session circuit breaker.** Transient reviewer failures are handled per-round (§9).
- **No `context: fork`.** The orchestrator runs in the caller's main context so the user sees every tool call, every finding, every edit.
- **No `@`-includes.** Policy is inlined; rule files are read via `Read` in §2.

## When to Trigger — Bias Toward Running

Run this skill after ANY of:
- Bug fixes (any severity), new features, refactors, multi-file changes (2+ files).
- Changes to compiler crates (ori_arc, ori_types, ori_llvm, ori_eval, ori_parse, ori_patterns).
- Test matrix additions, stdlib changes, registry changes, diagnostics changes.
- Plan section implementations, docs touching invariants.

**Also run when** unsure whether a change warrants review (default: run it), the change touches code paths shared across subsystems, or a fix surfaced interfering behavior elsewhere.

**Run with a custom objective when** iterating on any artifact (skill, doc, design) with multi-agent consensus.

**Skip only for** single-line typo fixes, comment edits, or formatting-only changes. When in doubt, run it — the cost of an unnecessary review is near zero; the cost of a missed correctness bug is high.
