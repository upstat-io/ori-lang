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

- [x] `[BUG-08-001][medium]` **block-banned-commands.sh false-matches codex/gemini substrings in non-review commands** — found by continue-roadmap.
  Repro: invoke Bash tool with `git commit -m "feat: dual codex + gemini transport"` and `timeout: 150000` — the hook blocks with "Blocked: timeout (X ms) on codex/gemini command is too short. Reviews need 20-35 minutes" even though the command is a plain git commit, not a reviewer invocation. Same false-match hits `grep codex .claude/`, `ls .gemini/skills/`, `cat .codex/skills/review-work/SKILL.md`, and any Bash command whose arguments/paths/messages contain the literal substrings `codex` or `gemini`.
  Root cause: `.claude/hooks/block-banned-commands.sh:70` uses `[[ "$COMMAND" == *"codex"* || "$COMMAND" == *"gemini"* ]]` — a naive substring match over the entire command line, with no word boundary or command-position check. Every occurrence of the literal in an argument, file path, commit message body, or grep pattern is treated as a reviewer invocation.
  Resolved 2026-04-08 by commit `81ff576b` (`fix(hooks): narrow codex/gemini match to shell command position`). Replaced the substring check with a shell-command-position regex that only matches when codex/gemini appears as an invoked command — at start-of-command, after a compound operator (`|`/`;`/`&`/`(`), optionally behind env-var prefixes, always followed by trailing whitespace. The general banned-pattern substring list (`--no-verify`, `git stash`, etc.) is untouched because those patterns have no legitimate non-invocation use. Extended `verify-hook.sh` from 9 to 27 cases: 10 false-positive suppression tests, 8 bypass-closure tests (env-var prefix / pipeline / && / sequence / subshell / flag-form gemini), plus the original 9 preserved unchanged. Also fixed a latent JSON-encoding bug in `run_test` that would have corrupted commands containing `"` or `\`. Verified end-to-end: the scanner-fix commit `463eb082` — whose commit message references `plans/dual-tpr-gemini` three times — landed cleanly through the fixed hook. Fix section: `plans/bug-tracker/fix-BUG-08-001.md`. TPR/hygiene reviews skipped by user decision (shell hook with deterministic 27-test matrix; lefthook full-check gate ran green on commit).
  Subsystem: .claude/hooks/block-banned-commands.sh
  Found: 2026-04-07 | Source: continue-roadmap
