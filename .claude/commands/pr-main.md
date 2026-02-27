# PR to Main

Commit, push, create a PR to main/master, and enable auto-merge. The PR will automatically merge once CI passes. Streamlines the dev → master workflow into a single command.

## Usage

```
/pr-main
```

---

## Workflow

**IMPORTANT:** Execute each step in order. Do not skip steps.

### Step 1: Check Current Branch and Status

**ACTION:** Verify we're not already on main/master:

```bash
git branch --show-current
git status
git diff --stat
```

If on `main` or `master`, STOP and inform the user they need to be on a feature/dev branch.

### Step 2: Sync with Master

**ACTION:** Merge master into the current branch so the PR won't be "out of date":

```bash
git fetch origin master
git merge origin/master --no-edit
```

If there are merge conflicts, stop and inform the user. Otherwise continue.

### Step 3: Run Commit-Push Workflow

Follow the `/commit-push` workflow:
1. Check git status and diff
2. Draft a conventional commit message
3. **Get user confirmation** before committing
4. Stage, commit, and push changes

### Step 4: Analyze Changes for PR

After pushing, analyze the commits that will be in the PR:

```bash
git log master..HEAD --oneline
git diff master..HEAD --stat
```

### Step 5: Check Past PRs for Context

**ACTION:** Fetch recent merged PRs to avoid repeating previous summaries:

```bash
gh pr list --base master --state merged --limit 5 --json number,title,body
```

Read the titles and summaries. The new PR must only describe **what changed since the last merged PR** — do not re-describe work that was already covered.

### Step 6: Draft PR Title and Summary

Create a PR title and summary based on the commits, informed by what past PRs already covered:

**PR Title:** Short description (under 70 chars), following the pattern:
- If single commit: Use the commit message subject
- If multiple commits: Summarize the theme (e.g., "Feature: Add X" or "Fix: Resolve Y issues")

**PR Summary:** Include:
- `## Summary` - 1-3 bullet points of key changes
- Only describe work **not already covered** by a previous PR
- If a past PR mentioned "Types V2 migration", don't repeat it — focus on what's new

### Step 7: Present PR Details and Get Confirmation

Show the user:
1. The branch being merged (e.g., `dev` → `master`)
2. Number of commits included
3. PR title and summary

Ask: "Shall I create this PR with auto-merge enabled?"

**Do NOT create the PR until user confirms.**

### Step 8: Create PR and Enable Auto-Merge

After user confirms:

```bash
gh pr create --base master --title "<title>" --body "$(cat <<'EOF'
## Summary
<bullet points>
EOF
)"
```

Then enable auto-merge (PR will merge automatically when CI passes):

```bash
gh pr merge --auto --merge --delete-branch=false
```

Note: `--auto` queues the PR to merge once CI passes. `--delete-branch=false` keeps the dev branch.

Report success with the PR URL and note that it will auto-merge when CI passes.

---

## Checklist

Before completing, verify:

- [ ] Confirmed not on main/master branch (Step 1)
- [ ] Synced with master (Step 2)
- [ ] Changes committed and pushed (Step 3)
- [ ] Past PRs checked to avoid repetition (Step 5)
- [ ] PR title and summary drafted (Step 6)
- [ ] User confirmed before creating PR (Step 7)
- [ ] PR created and auto-merge enabled (Step 8)

---

## Example PR

**Title:** `feat(typeck): add exhaustiveness checking for match expressions`

**Body:**
```
## Summary
- Add exhaustiveness analysis for match patterns
- Report missing variants with helpful suggestions
- Handle guard clauses correctly
```

---

## Rules

- Never run on main/master branch
- Always get user confirmation before creating the PR
- Always use `--merge` strategy (not squash or rebase) to preserve history
- Keep the feature branch after merge for continued development
- Do NOT include `Co-Authored-By` lines
