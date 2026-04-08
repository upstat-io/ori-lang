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
dual-source transport in `.claude/skills/dual-tpr/scripts/dual-invoke.sh`,
then returns both reviewers' raw responses concatenated with HTML-comment
sentinel attribution. This is intentionally different from `/tpr-review`
and `/review-work`, which use a merged findings envelope — `/tp-help` is
a consultation (raw perspectives from two models) rather than a review
(editorial synthesis). Single-reviewer mode is available via the
`ORI_TPR_REVIEWERS={codex|gemini|both}` environment variable (default: `both`).

See `.claude/skills/tp-help/SKILL.md` for:
- Auto-trigger conditions (8 concrete triggers + negative examples)
- Workflow (worktree snapshot, dual-source prompt construction with
  adversarial framing + Mandatory Grounding Block, background `dual-invoke.sh`
  invocation, 5-minute polling protocol via `status-check.sh`, raw-mode
  parsing via `parse-codex-raw.py` + `parse-gemini-raw.py`, HTML-comment
  sentinel attribution)
- DO NOT list (foreground invocation, trailing-echo masking exit code,
  wrapping in Agent, short timeouts on reviewer commands)
- Failure handling and single-reviewer mode (`ORI_TPR_REVIEWERS=codex`
  as a ~10x-faster escape hatch during iteration)

This file is a thin pointer maintained to preserve the slash-command
dispatcher contract (`name`, `description`, `allowed-tools`,
`argument-hint`). All operational content lives in the skill file to
satisfy the single-source-of-truth rule (resolves the R10 SSOT
violation from the dual-tpr-gemini plan). The canonical concat-mode
attribution sentinel format is defined in
`.claude/skills/dual-tpr/scripts/tp-help-sentinels.sh` (§07.3 SSOT fix).
