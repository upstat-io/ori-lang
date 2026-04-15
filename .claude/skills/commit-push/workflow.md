# /commit-push — Full Workflow

This is the protocol the sub-agent dispatched by `SKILL.md` executes. The SKILL.md parent should NOT execute these steps inline — it dispatches an isolated Sonnet Agent that reads this file end-to-end and runs it.

Stage, commit, and push all changes to the remote repository using conventional commit format.

## Usage (slash-level, for reference)

```
/commit-push           # Commit and push immediately (no confirmation)
/commit-push preview   # Show summary and ask for confirmation before committing
```

Arguments flow into this workflow as `<ARGS>` (substituted by SKILL.md at dispatch time).

---

## Workflow

**IMPORTANT:** Execute each step in order. Do not skip steps.

### Step 1: Check Git Status

Run these commands to see what will be committed:

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

**If the args contain `preview`:**
1. Show the user a summary of files changed and the proposed commit message.
2. Ask: "Shall I proceed with this commit?"
3. **Do NOT commit until user confirms.**

**Otherwise (default):** Skip directly to Step 4 — no confirmation needed.

### Step 4: Pre-Format (fixes the restaging issue)

**Run `./fmt-all.sh` BEFORE staging.** This formats every file in the tree so the formatter's output is captured in the snapshot you're about to stage — NOT produced as a side-effect after staging.

```bash
./fmt-all.sh
```

**Why this ordering matters:** Lefthook's pre-commit `fmt` step also runs `fmt-all.sh` with `stage_fixed: true`, but `stage_fixed` only restages files that were *already* in the index at the moment the hook fired. Any files the formatter touches that weren't intentionally staged get left as unstaged dirt in the working tree *post-commit* — the "restaging issue." Running `fmt-all.sh` here, before `git add`, means:

1. All formatter changes (on our target files AND any incidental fixes elsewhere) land in the working tree first.
2. `git add -A` then stages a fully-formatted snapshot.
3. Lefthook's `fmt` step becomes a no-op (idempotent — nothing left to fix).
4. The commit lands with a clean working tree — no post-commit dirt to chase.

`fmt-all.sh` is fast on an already-formatted tree (`cargo fmt --check` short-circuits) and harmless when there's nothing to fix, so running it unconditionally is cheap insurance.

### Step 5: Stage and Commit

```bash
git add -A
git commit -m "$(cat <<'EOF'
<commit message here>
EOF
)"
```

### Step 6: Post-Commit Dirty-Tree Check

After the commit lands, verify the working tree is clean:

```bash
git status --short
```

**If the output is empty**, proceed to push.

**If the output is non-empty**, the pre-commit hook's later steps (`full-check`, `version-sync`) or some other process modified files after staging. STOP and report:

- What's dirty (`git status --short`)
- Possible causes: (1) a hook step modified files unexpectedly, (2) an untracked file was never staged, (3) a file changed between `git add -A` and `git commit`
- Ask the user how to resolve: follow-up commit, stash, or discard. Do NOT auto-amend (per CLAUDE.md: always create NEW commits rather than amending).

Only proceed to Step 7 if the tree is clean.

### Step 7: Push

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
- [ ] `./fmt-all.sh` ran BEFORE staging (Step 4 — prevents post-commit dirt)
- [ ] Main changes staged + committed (Step 5)
- [ ] Post-commit dirty-tree check passed (Step 6)
- [ ] Changes pushed (Step 7)

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

- Always run `git status` before committing.
- Default mode: commit and push without confirmation (user trusts the process).
- Preview mode (`/commit-push preview`): show summary and wait for confirmation.
- Never force push or use destructive git operations.
- Keep the first line of commit message under 72 characters.
- Do NOT include `Co-Authored-By` lines in commit messages.
- **NEVER discard uncommitted work**: Do NOT use `git checkout -- <file>`, `git restore <file>`, `git reset --hard`, or `git clean` to "clean up" unrelated changes. The user runs parallel sessions — uncommitted files may be active work. Stage only your files with `git add <specific files>` instead of `git add -A` when other files are dirty. Dirty commits with extra files are acceptable; lost work is not.
- **When pre-commit hooks fail on files you didn't touch**: Stage only your specific files, not `-A`. If hooks still fail on unstaged files, report to the user — never discard.

---

## Return to parent

When the push succeeds (or preview is declined, or an error prevents push), return a concise final report:

- Commit SHA (short) and subject line
- Remote branch pushed to
- Any warnings surfaced by hooks
- If stopped short (dirty-tree check failed, preview declined, hook failure): what's pending and why

The parent has no context from this run — your summary is the only record it sees.
