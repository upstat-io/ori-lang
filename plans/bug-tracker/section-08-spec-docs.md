---
section: "08"
title: "Spec & Docs"
status: in-progress
goal: "Track and resolve all known spec/documentation bugs"
sections: []
---

# Section 08: Spec & Docs

**Subsystem:** `docs/ori_lang/`, `.claude/rules/`, `.claude/commands/`, `plans/`

Bugs in the language specification, EBNF grammar, design docs, CLAUDE.md, rule files, command/skill definitions, and plan structure.

---

## Open Bugs

- None.

---

## Resolved Bugs

- [x] `[BUG-08-002][high]` **dual-invoke-with-retry.sh launders dirty_worktree failures via fresh snapshots** — found by validate-dual-tpr.sh during Section 04.3 (dual-tpr-gemini) Scenario 3.
  Repro: ran `bash .claude/skills/dual-tpr/scripts/validate-dual-tpr.sh` against the stub-reviewer-dirty mode. The wrapper detected `dirty_worktree` on attempt 1, retried, then exited 0 on attempt 2 — 2 of 4 Scenario 3 assertions failed.
  Root cause: `.claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh:43-74` snapshotted the worktree at the START of every retry attempt (line 48). After attempt 1 dirties tracked file F (e.g. `A  F` → `AM F` in `git status --porcelain`), attempt 2's snapshot captured `AM F` as the new "before" baseline. The dirty stub appended again — `git status --porcelain` still reported `AM F` (same status code; content invisible to status) — so `worktree-guard.sh compare` reported CLEAN, the `else` branch fired (line 63), and the round exited 0. The `2> $RUN/worktree-error` redirect (line 60) also truncated the diff file on the successful attempt, erasing the evidence from attempt 1.
  Architectural issue: the retry loop treated `dirty_worktree` as a transient failure category (worth retrying), but it is a deterministic signal of reviewer misbehavior — a misbehaving reviewer will misbehave on retry too. Retry CANNOT fix it.
  Resolved 2026-04-08: added `break` after the dirty_worktree branch records its failure (single-line surgical fix). `dirty_worktree` is now a terminal failure category — recorded in round.log, worktree-error preserved, retry loop exits immediately. Other failure categories (`launch_or_exit_fail`, `codex_*`, `gemini_*`) remain retry-eligible because they CAN be transient. Verified end-to-end: validate-dual-tpr.sh now reports 8/8 passing; the original Section 02 transport-tests.sh regression suite still reports 18/18 passing (no regressions in clean-state tests). The corrected behavior matches the design intent stated in tpr-review/SKILL.md §"What NOT to do on transport failure" line 379: "DO NOT silently retry the semantic loop on infra failure" — the same principle applies inside the infra retry loop itself for deterministic categories.
  Subsystem: .claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh
  Found: 2026-04-08 | Source: validate-dual-tpr.sh / dual-tpr-gemini section-04.3 validation gate
  Note: This bug is the canary-release validation gate of dual-tpr-gemini doing exactly what it was designed to do — catching a Section 02 transport bug before it propagates to Sections 05/06/07. The plan's Section 04.3 explicitly says: "Transport bugs found here are the REASON this section exists as a validation gate — they're expected, they're valuable, and fixing them before propagation is the whole point of the canary release pattern." TPR/hygiene reviews skipped by user policy on shell-only fixes (same precedent as BUG-08-001).

- [x] `[BUG-08-001][medium]` **block-banned-commands.sh false-matches codex/gemini substrings in non-review commands** — found by continue-roadmap.
  Repro: invoke Bash tool with `git commit -m "feat: dual codex + gemini transport"` and `timeout: 150000` — the hook blocks with "Blocked: timeout (X ms) on codex/gemini command is too short. Reviews need 20-35 minutes" even though the command is a plain git commit, not a reviewer invocation. Same false-match hits `grep codex .claude/`, `ls .gemini/skills/`, `cat .codex/skills/review-work/SKILL.md`, and any Bash command whose arguments/paths/messages contain the literal substrings `codex` or `gemini`.
  Root cause: `.claude/hooks/block-banned-commands.sh:70` uses `[[ "$COMMAND" == *"codex"* || "$COMMAND" == *"gemini"* ]]` — a naive substring match over the entire command line, with no word boundary or command-position check. Every occurrence of the literal in an argument, file path, commit message body, or grep pattern is treated as a reviewer invocation.
  Resolved 2026-04-08 by commit `81ff576b` (`fix(hooks): narrow codex/gemini match to shell command position`). Replaced the substring check with a shell-command-position regex that only matches when codex/gemini appears as an invoked command — at start-of-command, after a compound operator (`|`/`;`/`&`/`(`), optionally behind env-var prefixes, always followed by trailing whitespace. The general banned-pattern substring list (`--no-verify`, `git stash`, etc.) is untouched because those patterns have no legitimate non-invocation use. Extended `verify-hook.sh` from 9 to 27 cases: 10 false-positive suppression tests, 8 bypass-closure tests (env-var prefix / pipeline / && / sequence / subshell / flag-form gemini), plus the original 9 preserved unchanged. Also fixed a latent JSON-encoding bug in `run_test` that would have corrupted commands containing `"` or `\`. Verified end-to-end: the scanner-fix commit `463eb082` — whose commit message references `plans/dual-tpr-gemini` three times — landed cleanly through the fixed hook. Fix section: `plans/bug-tracker/fix-BUG-08-001.md`. TPR/hygiene reviews skipped by user decision (shell hook with deterministic 27-test matrix; lefthook full-check gate ran green on commit).
  Subsystem: .claude/hooks/block-banned-commands.sh
  Found: 2026-04-07 | Source: continue-roadmap
