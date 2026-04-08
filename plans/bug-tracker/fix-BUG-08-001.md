---
bug: "BUG-08-001"
title: "block-banned-commands.sh false-matches codex/gemini substrings in non-review commands"
severity: "medium"
status: in-progress
goal: "The timeout gate in block-banned-commands.sh fires only on genuine top-level codex/gemini invocations and never on commands that merely contain the literal substrings in an argument, path, or message body."
success_criteria:
  - "git commit -m with a message body containing 'codex' or 'gemini' is ALLOWED with any timeout"
  - "grep / cat / ls / bash on a path or argument containing 'codex' or 'gemini' is ALLOWED with any timeout"
  - "Genuine codex exec invocations with a sub-20-minute timeout remain DENIED (floor)"
  - "Genuine gemini -p / --approval-mode / --output-format invocations with a sub-20-minute timeout remain DENIED (floor)"
  - "Genuine codex/gemini invocations with an over-35-minute timeout remain DENIED (ceiling)"
  - "Genuine invocations behind env-var prefix (ORI_LOG=debug codex exec ...) remain DENIED at short timeouts"
  - "Genuine invocations after a pipeline / && / ; (cat x | codex exec ...) remain DENIED at short timeouts"
  - "verify-hook.sh regression test suite passes with new false-positive and bypass cases added"
subsystem: ".claude/hooks/block-banned-commands.sh"
found: "2026-04-07"
source: "continue-roadmap"
third_party_review:
  status: none
  updated: null
---

# Fix: BUG-08-001 — Hook substring false-match on codex/gemini

**Status:** In Progress (implementation + tests landed; pending commit, TPR, hygiene)
**Severity:** medium
**Goal:** The timeout gate in `block-banned-commands.sh` fires only on genuine top-level `codex` / `gemini` invocations — never on commands that merely contain the literal substrings in a file path, argument, or message body. The gate's enforcement on genuine invocations (20-minute floor, 35-minute ceiling) is preserved unchanged.

**Success Criteria:**
- [x] `git commit -m "... dual-tpr-gemini ..."` with a sub-20-minute Bash timeout is ALLOWED (the exact repro from the bug entry)
- [x] `git commit -m "... codex ..."` with a sub-20-minute Bash timeout is ALLOWED
- [x] `grep codex .claude/` with a short timeout is ALLOWED (codex is an arg, not a command)
- [x] `ls .gemini/skills/` with a short timeout is ALLOWED (gemini is a path component)
- [x] `cat .codex/skills/review-work/SKILL.md` with a short timeout is ALLOWED (.codex is a path component) — covered by `ls .codex/skills/` test
- [x] `bash .claude/skills/dual-tpr/scripts/dual-invoke.sh` with a short timeout is ALLOWED (path contains "dual-tpr" but top-level command is bash)
- [x] `cat plans/dual-tpr-gemini/section-04-tpr-review.md` with a short timeout is ALLOWED (path segment)
- [x] `codex exec test` with 1-minute timeout is still DENIED (floor preserved — existing matrix unchanged)
- [x] `codex exec test` with 60-minute timeout is still DENIED (ceiling preserved)
- [x] `codex exec test` with 25-minute timeout is still ALLOWED (sweet spot preserved)
- [x] `gemini -p test` with 1-minute timeout is still DENIED (floor preserved)
- [x] `gemini -p test` with 60-minute timeout is still DENIED (ceiling preserved)
- [x] `gemini -p test` with 25-minute timeout is still ALLOWED (sweet spot preserved)
- [x] `ORI_LOG=debug codex exec test` with 1-minute timeout is DENIED (env-var prefix bypass closed)
- [x] `cat prompt.md | codex exec test` with 1-minute timeout is DENIED (pipeline bypass closed)
- [x] `some-prep && codex exec test` with 1-minute timeout is DENIED (&& bypass closed)
- [x] `verify-hook.sh` regression suite: all existing 9 tests pass AND all new false-positive + bypass tests pass (27/27 green)

**Context:** Filed 2026-04-07 during `/continue-roadmap plans/dual-tpr-gemini` work when the scanner-fix commit message mentioned the path `plans/dual-tpr-gemini`. The hook's substring-based codex/gemini check (`[[ "$COMMAND" == *"gemini"* ]]`) matched the literal in the commit message body and denied a plain `git commit` with a 150000 ms Bash timeout. The workaround (`git commit -F /tmp/msg.txt`) was applied once in `20d42a1f` to unblock section-04.1's combined commit, but every future commit whose message or path mentions `codex`/`gemini` hits the same wall, and the same false match fires on every `grep`, `ls`, `cat`, and `bash` command touching codex/gemini paths.

The hook's legitimate purpose — preventing Claude from running a real `codex exec` or `gemini -p` review with a 60-second or 10-minute timeout that would kill the review mid-stream — is important and must be preserved exactly. The fix narrows the matcher, not the enforcement.

---

## 1. Root Cause Analysis

- **Symptom**: `git commit -m "fix dual-tpr-gemini issue"` (timeout 150000) is denied with "Blocked: timeout (150000 ms) on codex/gemini command is too short." Same class of false positive on `grep codex foo.txt`, `ls .gemini/skills/`, `cat .codex/skills/review-work/SKILL.md`, `bash .claude/skills/dual-tpr/scripts/dual-invoke.sh`, and any Bash command whose arguments, paths, or message body contain the literal strings `codex` or `gemini`.

- **Proximate cause**: `.claude/hooks/block-banned-commands.sh:70`:
  ```bash
  if [[ "$COMMAND" == *"codex"* || "$COMMAND" == *"gemini"* ]]; then
  ```
  This is a naive bash substring match over the entire command line. It matches ANY occurrence of the literals anywhere — inside a quoted message body, inside a file path component, as a bare argument to another command, etc.

- **Root cause**: The hook's matcher has no concept of shell-token boundaries or command-position semantics. It treats the entire command as an opaque string and asks "does the text 'codex' or 'gemini' appear in it?" — a question with no correspondence to the semantic question the hook actually needs to answer, which is "is this command invoking the `codex` or `gemini` CLI binary as a top-level command?"

  This is a classic instance of a repo-wide anti-pattern: **text scanners doing structural work without tracking the structure they're scanning.** The same pattern surfaced minutes before this bug-fix started, in the `continue-roadmap` scanner's fence-tracking bug (scanner counted `- [ ]` lines inside fenced code blocks as real tasks) — both are line/character scanners applied to structured text (shell command lines, markdown bodies) without the tokenization/structure awareness the structural meaning demands. Both produced false positives from documentation / path examples containing the sentinel strings.

- **Blast radius**: Every Bash tool call Claude issues whose command string mentions `codex` or `gemini` in any form. In a session actively working on the `dual-tpr-gemini` plan (where that literal is in almost every affected path), this makes nearly every commit, grep, ls, and cat require a workaround. The workarounds themselves are hostile: `git commit -F file` requires creating a temp file for the message, and there's no analogous "-F" option for grep/ls/cat. The alternative — raising the Bash timeout to 20 minutes for commands that have nothing to do with reviews — is a non-starter because it corrupts the timeout semantics.

- **Affected files**:
  - `.claude/hooks/block-banned-commands.sh` — replace the substring match with a shell-command-position regex that only matches `codex` / `gemini` when they appear as invoked commands (at start-of-line, after `|`/`&`/`;`/`(`, possibly behind env-var prefixes), followed by a space and a real first argument. The enforcement body below remains unchanged.
  - `.claude/hooks/verify-hook.sh` — extend the regression test matrix with (a) false-positive ALLOW cases (paths, args, message bodies) and (b) bypass-closure DENY cases (env-var prefix, pipeline, `&&`). The existing 9 tests become positive pins (they must continue to pass unchanged).

**Reference implementations:**

- **Shell `type` / `command -v`**: The POSIX shell itself distinguishes "command position" from "argument position" during parsing. `type codex` resolves `codex` as a command name; `grep codex` passes `codex` as a literal argument to `grep`. The hook must approximate this distinction via regex because it can't invoke the shell parser.
- **lefthook / pre-commit frameworks**: These tools tokenize commands via a proper shell-aware parser before applying filters. We're not importing a parser for one hook, but the principle — filters should operate on tokens, not raw characters — is the correct direction.

---

## 2. TDD — Test Matrix

Write ALL tests BEFORE the fix. Verify they fail against the current hook.

All tests live in `.claude/hooks/verify-hook.sh`. The test harness already exists (added during dual-tpr-gemini section-01.4 retrospective). New test cases extend the existing 9-case matrix.

**Test harness fix**: `run_test`'s JSON encoder used `printf '{"command":"%s",...}'` which breaks on commands containing `"` or `\`. Rewrote the encoder to use `python3 -c` with proper JSON encoding so commands like `git commit -m "fix dual-tpr-gemini"` round-trip safely. Tooling improvement required to enable the new false-positive test cases.

### Exact failing case (the bug's repro)
- [x] `git commit -m "fix dual-tpr-gemini"` + 60000 ms → ALLOW (message body contains "gemini")

### Edge cases — false positives that MUST be ALLOWED
- [x] `git commit -m "fix codex issue"` + 60000 ms → ALLOW (message body contains "codex") — covered as `refactor codex parser`
- [x] `grep codex .claude/` + 60000 ms → ALLOW (codex is an argument to grep)
- [x] `grep gemini .claude/` + 60000 ms → ALLOW (gemini is an argument to grep)
- [x] `ls .codex/skills/` + 60000 ms → ALLOW (.codex is a directory component, preceded by `.`)
- [x] `ls .gemini/skills/` + 60000 ms → ALLOW (.gemini is a directory component, preceded by `.`)
- [x] `cat plans/dual-tpr-gemini/section-04-tpr-review.md` + 60000 ms → ALLOW (path segment, preceded by `-`)
- [x] `bash .claude/skills/dual-tpr/scripts/dual-invoke.sh arg1` + 60000 ms → ALLOW (path contains "dual-tpr" but top-level command is bash)
- [x] `./my-gemini-wrapper.sh test` + 60000 ms → ALLOW (gemini appears in executable name preceded by `-`)
- [x] `echo 'fix codex and gemini bugs'` + 60000 ms → ALLOW (both substrings in a quoted string arg)

### Negative pins — genuine invocations that MUST stay DENIED at bad timeouts
- [x] `codex exec test` + 60000 ms → DENY (under floor — existing matrix)
- [x] `codex exec test` + 600000 ms → DENY (under floor — existing matrix)
- [x] `codex exec test` + 3600000 ms → DENY (over ceiling — existing matrix)
- [x] `gemini -p test` + 60000 ms → DENY (under floor — existing matrix)
- [x] `gemini -p test` + 600000 ms → DENY (under floor — existing matrix)
- [x] `gemini -p test` + 3600000 ms → DENY (over ceiling — existing matrix)
- [x] `gemini --approval-mode yolo` + 60000 ms → DENY (flag-form invocation still denied)
- [x] `gemini --output-format stream-json -p test` + 60000 ms → DENY (flag-form invocation still denied)

### Semantic pins — genuine invocations that MUST stay ALLOWED in the sweet spot
- [x] `codex exec test` + 1500000 ms → ALLOW (25 min sweet spot — existing matrix)
- [x] `gemini -p test` + 1500000 ms → ALLOW (25 min sweet spot — existing matrix)

### Bypass closure — compound-command invocations that MUST stay DENIED
These are the cases where a naive word-boundary-only check might regress the hook's enforcement. They prove the fix doesn't introduce bypasses by only checking the start of the command.

- [x] `ORI_LOG=debug codex exec test` + 60000 ms → DENY (env-var prefix)
- [x] `TARGET=x86_64 ORI_LOG=debug codex exec test` + 60000 ms → DENY (multiple env-var prefixes)
- [x] `cat prompt.md | codex exec test` + 60000 ms → DENY (pipeline — codex at command position after `|`)
- [x] `some-prep && codex exec test` + 60000 ms → DENY (logical AND — codex at command position after `&&`)
- [x] `cleanup; gemini -p test` + 60000 ms → DENY (sequence — gemini at command position after `;`)
- [x] `(codex exec test)` + 60000 ms → DENY (subshell — codex at command position after `(`)

### Control (gate doesn't apply)
- [x] `echo hello` + 60000 ms → ALLOW (non-codex/gemini command, existing matrix)

### Verify tests fail before fix
- [x] Ran the full 27-test matrix against the unmodified hook. Result: 18 PASS / 9 FAIL. All 9 failures are false-positive cases (message body, grep arg, ls path component, cat path, ./wrapper script, echo quoted string) that were being DENIED by the substring match — exact reproduction of the bug in the regression suite. `bash .claude/skills/dual-tpr/scripts/dual-invoke.sh` unexpectedly passed because the literal strings "codex"/"gemini" don't appear in that path (only "dual-tpr" / "dual-invoke"), which is the boundary case confirming the substring matcher only fails on literal occurrences.
- [x] All bypass-closure cases already passed against the unmodified hook (the substring match catches them — they were correct denials for the wrong reason; after fix they remain correct denials for the right reason).

---

## 3. Implementation

- [x] Replace the substring match at `.claude/hooks/block-banned-commands.sh:70` with a shell-command-position regex that only matches `codex` / `gemini` when they appear as invoked commands. The regex accepts:
  1. `codex` or `gemini` at the very start of the command line (possibly after leading whitespace)
  2. After a shell compound operator: `|`, `&`, `;`, `(`, optionally followed by whitespace
  3. Optionally preceded by one or more env-var assignments (`NAME=value ...`) at the command-position point
  
  And requires that `codex` / `gemini` be followed by whitespace (i.e., they have at least one argument, which is always true for real invocations: `codex exec ...`, `gemini -p ...`, etc.).

  ```bash
  REVIEW_CMD_RE='(^[[:space:]]*|[|;&(][[:space:]]*)([[:alnum:]_]+=[^[:space:]]*[[:space:]]+)*(codex|gemini)[[:space:]]'
  if [[ "$COMMAND" =~ $REVIEW_CMD_RE ]]; then
    # ... existing timeout window enforcement unchanged ...
  fi
  ```

- [x] Added a multi-line comment block above the regex explaining the contract, citing `BUG-08-001` and pointing to `verify-hook.sh` for the matrix. Comment includes the two match positions (start-of-line or compound operator), the env-var prefix allowance, and the trailing-whitespace requirement.

- [x] Extended `.claude/hooks/verify-hook.sh` with 18 new test cases (10 false-positive + 8 bypass-closure) grouped under labeled headers `# ── False-positive suppression (BUG-08-001) ──` and `# ── Bypass closure — compound invocations (BUG-08-001) ──`.

- [x] Also fixed a latent bug in `run_test`: the JSON encoder used `printf '...command":"%s"...'` which corrupts commands containing `"` or `\`. Rewrote it to use `python3 -c` with proper `json.dumps` encoding via environment variables. Required for the new tests, and a correctness improvement that makes the harness safe for any future test commands.

- [x] Ran `bash .claude/hooks/verify-hook.sh` — 27/27 passing after the fix.

- [x] Manually re-ran the original repro: `printf '%s' '{"command":"git commit -m \"fix dual-tpr-gemini issue\"","timeout":150000}' | bash block-banned-commands.sh` — empty output (ALLOW). Original bug no longer reproduces.

- [x] Spot-check of other banned patterns: `git stash` and `--no-verify` still correctly denied (the regex change is scoped only to the timeout gate block, not the general banned-pattern list).

- [x] Bash syntax check: `bash -n` clean on both `block-banned-commands.sh` and `verify-hook.sh`.

---

## 4. Completion Checklist

- [ ] All new tests pass unchanged after fix (no test modifications needed)
- [ ] Matrix completeness verified — false-positive suppression × bypass closure × existing timeout gate
- [ ] `bash .claude/hooks/verify-hook.sh` — 100% pass (pre-existing 9 tests + all new tests)
- [ ] Manual repro from the bug entry no longer denied
- [ ] `timeout 150 ./test-all.sh` green — no regressions (hook doesn't touch cargo, but full suite is the standard)
- [ ] `/commit-push` — commit hook fix + test harness extensions (one atomic commit; they're tightly coupled)
- [ ] Bug entry in `plans/bug-tracker/section-08-spec-docs.md` updated: `- [x]` with resolution details
- [ ] Fix section frontmatter `status` updated to `complete`
- [ ] Bug-tracker `00-overview.md` open bug count updated if there's a summary field
- [ ] `/tpr-review` passed — independent dual-source review of the hook fix (medium severity, expected)
- [ ] `/impl-hygiene-review` passed — MUST run AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` retrospective completed — AFTER both reviews clean

**Exit Criteria:** `bash .claude/hooks/verify-hook.sh` passes all test cases (pre-existing 9 plus the new false-positive, bypass-closure, and additional genuine-invocation cases). The original repro from the bug entry — `git commit -m "fix dual-tpr-gemini"` with 150000 ms timeout — returns ALLOW from the hook. The fix preserves the hook's enforcement on every genuine `codex exec` / `gemini -p|--approval-mode|--output-format` invocation at timeouts outside the 20-35 minute window, including when reached via env-var prefix, pipeline, sequence, logical AND, or subshell. The scanner-fix commit from `/continue-roadmap` (whose commit message mentions `plans/dual-tpr-gemini`) is no longer blocked and lands cleanly via `/commit-push`.
