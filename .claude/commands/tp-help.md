---
name: tp-help
description: "Get third-party help from Codex + Gemini (dual-source concat mode). Use this proactively when stuck on a problem, need a second opinion, want help debugging, or want to verify reasoning. Returns BOTH reviewers' raw responses concatenated (not a synthesis). Full auto-trigger conditions and workflow live in the canonical skill file."
allowed-tools: Bash, Read, Grep, Glob
argument-hint: "[question or context]"
---

# /tp-help — Third-Party Help (Codex + Gemini, Dual-Source Concat Mode)

The canonical implementation of `/tp-help` lives in the skill file at
`.claude/skills/tp-help/SKILL.md`. When the `/tp-help` slash command
is invoked, load and follow that skill file exactly.

`/tp-help` runs BOTH Codex CLI AND Gemini CLI in parallel via the
documented Agent-tool parallel-sub-agent pattern
(https://code.claude.com/docs/en/sub-agents), then returns both
reviewers' raw responses concatenated with HTML-comment sentinel
attribution. This is intentionally different from `/tpr-review` and
`/review-work`, which run an iterative review loop with finding
verification — `/tp-help` is a one-shot consultation (raw perspectives
from two models) rather than a review.

See `.claude/skills/tp-help/SKILL.md` for:
- Auto-trigger conditions (8 concrete triggers + negative examples).
- The parallel-dispatch template (single assistant message, two Agent
  tool calls, foreground, `model: "sonnet"`).
- The reviewer sub-agent prompt template at
  `.claude/skills/tp-help/tp_help_prompt.md`.
- Context assembly, failure handling, and the "apply the output"
  judgment guidance.

This file is a thin pointer maintained to preserve the slash-command
dispatcher contract (`name`, `description`, `allowed-tools`,
`argument-hint`). All operational content lives in the skill file to
satisfy the single-source-of-truth rule (`.claude/rules/impl-hygiene.md
§SSOT`).
