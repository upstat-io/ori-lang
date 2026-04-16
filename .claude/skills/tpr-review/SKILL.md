---
name: tpr-review
description: "Dual-source third-party review (codex + gemini) in parallel, with verification-against-code, iterative fix-and-re-run until both reviewers return clean. Reviews code, plans, skills, docs, or any custom objective. TRIGGER proactively after completing ANY non-trivial work: bug fixes, features, refactors, multi-file changes, compiler changes, codegen changes, test additions, plan implementations, or anything touching correctness-sensitive code. When in doubt, run it."
allowed-tools: Read, Write, Edit, Bash, Glob, Grep, Agent, AskUserQuestion, Skill
---

# /tpr-review

Dispatches the **codex** and **gemini** CLIs as parallel sub-agents per round. Reads their plain-text `<<<TPR-REPORT>>>` output in the main context. Verifies every finding against actual code. Fixes accepted findings. Re-runs until both reviewers return clean or a stop gate fires.

## §1 — Invocation

Inspect the first token of `ARGS`:

- empty or `--skill review-work` → **work mode**. Scope: current working-tree changes.
- `--skill review-plan <section-path>` → **plan mode**. Scope: the named plan section. Findings file into the section's `third_party_review` frontmatter per §10.
- any other value → **custom-objective mode**. The entire `ARGS` becomes the review objective.

Ambiguous input → `AskUserQuestion`. Do not guess.

## §2 — Grounding (MANDATORY before round 1)

```
Read: CLAUDE.md
Bash: ls .claude/rules/*.md
Read: every file the ls enumerated
```

The `ls` output is the authoritative rule manifest. Do not hand-select. Reviewers perform their own grounding inside their sub-agents (see `tp_agent_prompt.md`).

## §3 — Spec/grammar gate (MANDATORY, all modes, pre-round-1)

```
Bash: git diff --name-only HEAD -- docs/ori_lang/v2026/spec/ docs/ori_lang/v2026/spec/grammar.ebnf
```

If the output is **non-empty**, check `docs/ori_lang/proposals/approved/` for a proposal whose body mentions any of the touched files. If no matching proposal is found, emit a synthetic pre-round-0 finding and prepend it to the round-0 verified set:

```
{
  id: "SPEC-GATE-001",
  severity: "critical",
  path: "<first spec/grammar file touched>",
  line: 1,
  title: "Spec/grammar modified without approved proposal",
  evidence: "<git diff output listing all touched spec/grammar files>",
  rule_violated: ".claude/rules/spec.md §Enforcement",
  recommended_fix: "Either revert the spec/grammar changes OR create + approve a proposal under docs/ori_lang/proposals/approved/ and reference it in the commit message"
}
```

The finding is CRITICAL, NOT meta, and verified-by-construction (git diff is ground truth). The user must fix it (revert or approve a proposal) before the round's reviewers can complete.

## §4 — Trust tiers (verification posture)

Every finding — codex or gemini — is verified against actual code before acting. Reviewer claims are hypotheses. Trust tier sets depth.

- **Codex — HIGH trust.** For each codex finding: Read the cited file around the cited line (±20 lines). Confirm the quoted evidence exists verbatim. If it matches, accept for classification (§6). If the quote doesn't match, drop silently.
- **Gemini — LOWER trust.** For each gemini finding: Read the cited file IN FULL. Trace the code path end-to-end. Confirm the claimed behavior matches the code. Drop any finding that fails verification. Gemini URL citations are never authoritative — verify the underlying claim against the actual code.

Verification happens BEFORE classification. An unverified finding never reaches the classifier.

## §5 — Round loop

Two counters bound the loop:

- `iteration_counter` — finding-fixing rounds completed. Cap: **5**.
- `meta_only_streak` — consecutive rounds where every verified finding was meta. Cap: **2**.

Stop conditions (check in order):

1. Both reviewers `status: clean` AND `verified` is empty → exit clean.
2. `meta_only_streak == 2` → exit (juice not worth the squeeze).
3. `iteration_counter == 5` → exit (iteration cap).
4. Both-reviewer failure twice → §9 escalation.

```
iteration_counter = 0
meta_only_streak = 0
last_actionable_count = None
ever_verified_findings = []

while iteration_counter < 5 and meta_only_streak < 2:
    template = Read(".claude/skills/tpr-review/tp_agent_prompt.md")
    codex_prompt  = fill(template, REVIEWER=codex,  TRUST_TIER=HIGH,
                         OBJECTIVE=OBJ, SCOPE=SCOPE)
    gemini_prompt = fill(template, REVIEWER=gemini, TRUST_TIER=LOWER,
                         OBJECTIVE=OBJ, SCOPE=SCOPE)

    [codex_out, gemini_out] = dispatch_parallel(codex_prompt, gemini_prompt)

    codex_report  = parse_tpr_report(codex_out)
    gemini_report = parse_tpr_report(gemini_out)

    if codex_report.status == "failed":  codex_report  = retry_or_survivor(codex)
    if gemini_report.status == "failed": gemini_report = retry_or_survivor(gemini)

    all_findings = codex_report.findings + gemini_report.findings
    verified     = [f for f in all_findings if verify_against_code(f)]  # §4
    meta         = [f for f in verified if classify_meta(f)]             # §6
    actionable   = [f for f in verified if f not in meta]
    ever_verified_findings.extend(verified)                              # §10

    commit_sha = None
    if len(actionable) > 0:
        commit_sha = fix_and_commit(actionable)                          # §7
        meta_only_streak = 0
        last_actionable_count = len(actionable)
    elif len(verified) > 0:
        meta_only_streak += 1
        last_actionable_count = 0

    print_round_summary(iteration_counter, codex_report, gemini_report,
                        verified, meta, actionable, commit_sha)           # §11

    if len(verified) == 0 and codex_report.status == "clean" and gemini_report.status == "clean":
        exit_reason = "clean"
        break

    iteration_counter += 1

else:
    exit_reason = "meta_cap_reached" if meta_only_streak >= 2 else "iter_cap_reached"

emit_final_report(exit_reason, iteration_counter, last_actionable_count)

if mode == "review-plan":
    write_plan_frontmatter(section_path, exit_reason, ever_verified_findings)  # §10
```

## §6 — Meta classification (AFTER verification)

A finding is **meta** if and only if ALL of these apply:

- Category is purely wording/phrasing, cosmetic/formatting, already-documented-elsewhere, or an exact duplicate of a prior-round finding.
- Does NOT touch: correctness, invariants (AIMS / SSOT / phase boundaries / registry drift), tests, security, spec conformance, error paths, API contracts, memory safety.
- `recommended_fix` is a pure-doc edit, rename, or whitespace change.

Any doubt → NOT meta.

`meta_only_streak` increments ONLY when the entire verified-findings set of the round is meta. A single non-meta finding resets the streak to 0.

## §7 — Finding-handling policy (ABSOLUTE)

There is NO circumstance under which the orchestrator may dismiss, rationalize, scope-note, or defer an actionable finding. Tied to `CLAUDE.md §The One Rule`.

Valid dispositions:

1. **Fix NOW** — edit in main context, run affected tests, commit via `/commit-push`.
2. **Create a plan and execute it** — if too large for inline fix, run `/create-plan` and implement the resulting sections.
3. **`AskUserQuestion`** — genuinely blocked on a user decision or missing domain knowledge.

**BANNED phrases** (MUST NOT appear in round summaries, commit messages, plan updates):

- "pre-existing" / "was already broken"
- "architectural limitation" / "requires major refactor"
- "out of scope" / "not a §NN deliverable"
- "conservative/safe" / "only precision loss"
- "not a regression" / "not introduced by this work"
- "future improvement" / "tracked for later"
- "known limitation"

The size of the fix is irrelevant. Cross-crate refactoring across 10 files is the work, not a reason to defer.

**Filing.** Plan-owned findings → append `- [ ]` items to that section's `## {NN}.R Third Party Review Findings` block, tagged `[TPR-{NN}-{ordinal}-{reviewer}][severity]`. Unowned findings → file to `plans/bug-tracker/` under the appropriate subsystem using `BUG-{NN}-{ordinal}` (no reviewer suffix — reviewer provenance in the body). Agreement findings file ONE bug entry, not two.

## §8 — Parallel dispatch (canonical template)

Dispatch BOTH reviewers in a SINGLE assistant message. Foreground only — never `run_in_background: true`.

```
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

## §9 — Failure handling

**One reviewer `status: failed`.** Retry that reviewer once (single tool call in a follow-up message; partner already completed). If retry fails, **survivor mode**: use only the surviving report, set `survivor_mode: true` in the round summary, continue.

**Both reviewers `status: failed`.** Retry ONCE (parallel dispatch again). If both fail a second time, escalate:

```
AskUserQuestion:
  "Both reviewers failed twice. What should I do?"
    1. Retry once more
    2. Abort this /tpr-review invocation (code unchanged, no findings filed)
    3. Proceed without review (NOT RECOMMENDED)
```

Never silently exit without producing either a round summary or an escalation.

## §10 — Plan-TPR integration (plan mode only)

After the loop terminates when `ARGS` began with `--skill review-plan <section-path>`:

1. Read the section file's YAML frontmatter.
2. Set `third_party_review.status`:
   - `clean` if `exit_reason == "clean"` and zero verified findings across all rounds.
   - `findings` if any verified findings occurred (even if all were fixed inline).
   - `escalated` if `exit_reason` was `meta_cap_reached`, `iter_cap_reached`, or `both_reviewer_failure`.
3. Set `third_party_review.updated` to today's date (YYYY-MM-DD).
4. For `findings` status — append each accepted finding as `- [ ]` items under the section's `## {NN}.R Third Party Review Findings` block (create the block if missing):

   ```md
   - [ ] `[TPR-{NN}-{ordinal}-{reviewer}][{severity}]` `{path}:{line}` — {title}.
     Evidence: {evidence}
     Impact: {one-line impact summary}
     Required plan update: {recommended_fix}
     Basis: fresh_verification | direct_file_inspection. Confidence: {high|medium|low}.
   ```

   Agreement findings (same location + title from both reviewers) → file BOTH halves with an `Agreement:` cross-reference line pointing at each other. Single-reviewer findings → file ONE entry noting the reviewer.

5. Write via `Edit`. Section `status` stays `in-progress` while `third_party_review.status: findings`.

## §11 — Coordinator rendering (MANDATORY)

After EVERY round, print a round summary as a direct assistant message. Render AFTER `fix_and_commit` (so `Fix commit: {sha}` is populated) but BEFORE any state-branching (exit-clean, meta-cap, iter-cap, round N+1 dispatch).

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
- Every bullet MUST end with `Disposition:`. A bullet without disposition is a contract violation.
- Agreement findings produce ONE bullet, not two.
- Clean-pass rounds still render the block — `Findings this round:` becomes `(none — both reviewers returned clean)` and `Next round will confirm` becomes `loop exiting clean`.
- Dropped findings appear as `Disposition: dropped at verification: <reason>`.
- Bullets ≤120 characters per line.

## When to Trigger — Bias Toward Running

Run after ANY of:

- Bug fixes (any severity), new features, refactors, multi-file changes (2+ files).
- Changes to compiler crates (`ori_arc`, `ori_types`, `ori_llvm`, `ori_eval`, `ori_parse`, `ori_patterns`).
- Test matrix additions, stdlib changes, registry changes, diagnostics changes.
- Plan section implementations, docs touching invariants.

**Also run when** unsure whether a change warrants review, the change touches code paths shared across subsystems, or a fix surfaced interfering behavior elsewhere.

**Run with a custom objective when** iterating on any artifact (skill, doc, design) with multi-agent consensus.

**Skip only for** single-line typo fixes, comment edits, or formatting-only changes. When in doubt, run it.
