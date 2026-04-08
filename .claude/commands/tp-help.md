---
name: tp-help
description: "Get third-party help from Codex CLI. Use this proactively when stuck on a problem, need a second opinion, want help debugging, or want to verify reasoning. Full auto-trigger conditions and workflow live in the canonical skill file."
allowed-tools: Bash, Read, Grep, Glob
argument-hint: "[question or context]"
---

# /tp-help — Third-Party Help

The canonical implementation of `/tp-help` lives in the skill file at
`.claude/skills/tp-help/SKILL.md`. When the `/tp-help` slash command
is invoked, load and follow that skill file exactly.

See `.claude/skills/tp-help/SKILL.md` for:
- Auto-trigger conditions (8 concrete triggers + negative examples)
- Workflow (prompt construction, background codex invocation, response parsing)
- DO NOT list (foreground invocation, wrapping in Agent, heredoc quoting)
- Failure handling

This file is a thin pointer maintained to preserve the slash-command
dispatcher contract (`name`, `description`, `allowed-tools`,
`argument-hint`). All operational content lives in the skill file to
satisfy the single-source-of-truth rule (resolves the R10 SSOT
violation from the dual-tpr-gemini plan).
