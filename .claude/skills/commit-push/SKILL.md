---
name: commit-push
description: Stage, commit, and push all changes to the remote using conventional commit format.
argument-hint: "[preview]"
---

# Commit and Push

`/commit-push` — stage, commit, and push all changes.
`/commit-push preview` — show summary first and wait for confirmation.

## How this skill runs

Execute the workflow inline. The full protocol lives in `workflow.md` — read it via `@` include and follow it end-to-end.

Args: `<ARGS>` from the user (empty = default mode, `preview` = confirm before commit).

@.claude/skills/commit-push/workflow.md

## Rules

- Follow Steps 1 through 7 literally. Do NOT skip fmt-all.sh (Step 4).
- Never use destructive git operations: no force push, no reset --hard, no checkout --, no restore, no clean.
- Never bypass hooks (no --no-verify, no SKIP_TESTS). If a hook fails, investigate and report.
- Never amend. Always create NEW commits if a follow-up is needed.
- Do NOT include Co-Authored-By lines in commit messages.
- Keep the first line under 72 characters. Valid types: `feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert`.
- If the post-commit dirty-tree check (Step 6) fails, STOP and report.
- If preview mode and the user declines, STOP cleanly without committing.

## Consumers

Many skills and commands invoke `/commit-push` as their commit mechanism (e.g., `/fix-bug`, `/review-plan`, `/tpr-review`, `/continue-roadmap`, `/rosetta-test`).
