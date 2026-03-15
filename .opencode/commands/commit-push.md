---
description: Stage, commit, and push all changes using conventional commit format
---

# Commit and Push All Changes

Stage, commit, and push all changes to the remote repository using conventional commit format.

## Usage

```
/commit-push           # Commit and push immediately (no confirmation)
/commit-push preview   # Show summary and ask for confirmation before committing
```

**Arguments:** `$ARGUMENTS`

---

## Workflow

**IMPORTANT:** Execute each step in order. Do not skip steps.

### Step 1: Check Git Status

**ACTION:** Run these commands to see what will be committed:

```bash
git status
git diff --stat
```

### Step 2: Analyze and Draft Commit Message

Review the changes and create a commit message following conventional commit format:

```
<type>(<scope>): <description>

<body>
```

**Valid types:**
| Type | Description |
|------|-------------|
| `feat` | A new feature |
| `fix` | A bug fix |
| `docs` | Documentation only changes |
| `style` | Code style changes (formatting, etc) |
| `refactor` | Code change that neither fixes a bug nor adds a feature |
| `perf` | Performance improvement |
| `test` | Adding or correcting tests |
| `build` | Changes to build system or dependencies |
| `ci` | Changes to CI configuration |
| `chore` | Other changes that don't modify src or test files |
| `revert` | Reverts a previous commit |

**Scope** is optional. Use the primary module affected (e.g., `typeck`, `parser`, `llvm`).

### Step 3: Preview Mode (only if `preview` argument is passed)

**If `$ARGUMENTS` contains `preview`:**
1. Show the user a summary of files changed and the proposed commit message
2. Ask: "Shall I proceed with this commit?"
3. **Do NOT commit until user confirms.**

**Otherwise (default):** Skip directly to Step 4 -- no confirmation needed.

### Step 4: Commit Main Changes

```bash
git add -A
git commit -m "$(cat <<'EOF'
<commit message here>
EOF
)"
```

### Step 5: Push

```bash
git push
```

Report success or any errors.

---

## Checklist

Before completing, verify:

- [ ] `git status` was checked (Step 1)
- [ ] Commit message follows conventional format (Step 2)
- [ ] If preview mode: user confirmed before committing (Step 3)
- [ ] Main changes committed (Step 4)
- [ ] Changes pushed (Step 5)

---

## Example Commit Message

```
perf(typeck): optimize line lookup and hash map usage

- Add LineOffsetTable for O(log n) line lookups instead of O(n)
- Switch to FxHashMap/FxHashSet in type checker components
- Add index for O(1) associated type lookups
- Optimize diagnostic queue sorting
```

---

## Rules

- Always run `git status` before committing
- Default mode: commit and push without confirmation (user trusts the process)
- Preview mode (`/commit-push preview`): show summary and wait for confirmation
- Never force push or use destructive git operations
- Keep the first line of commit message under 72 characters
- Do NOT include `Co-Authored-By` lines in commit messages
