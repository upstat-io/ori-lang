---
name: commit-push
description: Stage, commit, and push all changes to the remote using conventional commit format. Thin dispatcher — launches the full workflow in an isolated Sonnet sub-agent via Agent({}) so git/fmt/hook output never pollutes the parent context. Runs manually when the user invokes /commit-push.
argument-hint: "[preview]"
---

# Commit and Push

`/commit-push` — stage, commit, and push all changes.
`/commit-push preview` — show summary first and wait for confirmation.

## How this skill runs

SKILL.md is a thin dispatcher. The full protocol (fmt pre-pass, staging, commit message drafting, dirty-tree check, push) lives in `workflow.md` and is executed by a dispatched Sonnet sub-agent — NOT inline. Running the workflow in an isolated Agent keeps `git status`, `git diff`, `fmt-all.sh`, and lefthook output out of the parent's context window.

## Caller action (the ONLY inline action)

Before any other tool call, invoke the Agent tool. Substitute `<ARGS>` with the user's `/commit-push` arguments (empty string if none, `preview` if the user passed preview mode):

```
Agent({
  description: "commit-push stage + commit + push",
  subagent_type: "general-purpose",
  model: "sonnet",
  prompt: `
You are the commit-push sub-agent. Read .claude/skills/commit-push/workflow.md
in full and execute it end-to-end.

Args from the user: <ARGS>
(Empty string means default mode — commit and push immediately without
confirmation. "preview" means show the proposed commit message first and
wait for user confirmation before committing.)

Rules:
- Follow Steps 1 through 7 literally. Do NOT skip fmt-all.sh (Step 4) —
  it is load-bearing for the restaging-issue fix.
- Never use destructive git operations: no force push, no reset --hard,
  no checkout --, no restore, no clean. The user runs parallel sessions;
  uncommitted files may be active work.
- Never bypass hooks (no --no-verify, no SKIP_TESTS). If a hook fails,
  investigate and report — don't circumvent.
- Never amend. Always create NEW commits if a follow-up is needed.
- Do NOT include Co-Authored-By lines in commit messages.
- Keep the first line under 72 characters and use a valid conventional
  commit type (feat|fix|docs|style|refactor|perf|test|build|ci|chore|
  revert — "tools" is NOT valid; use "build" or "chore").
- If the post-commit dirty-tree check (Step 6) fails, STOP and report —
  do NOT auto-amend, do NOT auto-discard.
- If preview mode is active and the user declines, STOP cleanly without
  committing and report.

Return a concise summary per workflow.md "Return to parent":
- Commit SHA (short) + subject
- Remote branch pushed to
- Any hook warnings
- Or, if stopped short: what's pending and why
  `
})
```

## After the sub-agent returns

Relay the sub-agent's summary to the user verbatim (or condensed to 1–2 lines if the user wants terse output). No further parent-side action is required — commit and push are either done, or stopped short with a reason the user now sees.

## Files in this skill

- `SKILL.md` (this file) — thin dispatcher.
- `workflow.md` — the full commit-push protocol executed by the sub-agent.

`workflow.md` is not registered as a skill. It is a reference document read by the dispatched Agent.

## Consumers

Many skills and commands invoke `/commit-push` as their commit mechanism (e.g., `/fix-bug`, `/review-plan`, `/tpr-review`, `/continue-roadmap`, `/rosetta-test`). The slash name is unchanged — only the implementation location moved from `.claude/commands/commit-push.md` into this skill directory. Existing consumers continue to work without modification.
