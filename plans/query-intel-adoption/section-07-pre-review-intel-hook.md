---
section: "07"
title: "Hook-heavy ambient automation — pre-review-intel.sh"
status: not-started
reviewed: false
goal: "Inject a bounded Intelligence Summary automatically into review-family slash-command prompts via a UserPromptSubmit hook"
success_criteria:
  - "`.claude/hooks/pre-review-intel.sh` exists, is executable, and runs under 2s on typical inputs"
  - "`.claude/settings.json` (or `.claude/settings.local.json`) registers the hook under `hooks.UserPromptSubmit` with a matcher limited to review-family commands"
  - "Hook respects graph availability — `scripts/intel-query.sh status != ok` causes silent yield with zero context injection"
  - "Hook has a `--preview` mode for operator testing that prints what would be injected without actually injecting"
  - "Satisfies mission criterion: pre-review-intel.sh fires on UserPromptSubmit for review-family, injects bounded summary, degrades silently"
inspired_by:
  - "`.claude/hooks/block-banned-commands.sh` — existing PreToolUse hook pattern: python3 JSON parse + hookSpecificOutput response"
  - "`.claude/hooks/classify-review-command.py` — review-family slash-command matcher (21KB file; has matcher logic to reuse)"
  - "`.claude/skills/dual-tpr/compose-intel-summary.md` — the bounded-summary format the hook emits (§03)"
  - "TPR findings codex-009 [medium], gemini-010 [medium]"
depends_on: ["03"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "07.1"
    title: "Write pre-review-intel.sh with --preview dry-run"
    status: not-started
  - id: "07.2"
    title: "Register in settings.json + test live firing"
    status: not-started
  - id: "07.3"
    title: "Graceful-degradation verification"
    status: not-started
  - id: "07.4"
    title: "SSOT registry drift detector (scripts/ssot-registry-audit.py)"
    status: not-started
  - id: "07.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "07.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 07: Hook-heavy ambient automation

**Status:** Not Started
**Goal:** The user explicitly chose hook-heavy automation (per AskUserQuestion on 2026-04-14). This section implements the `pre-review-intel.sh` hook that fires on `UserPromptSubmit` for review-family slash-commands and injects a bounded Intelligence Summary into the conversation context BEFORE the model sees the prompt. Maximum adoption pressure — the operator sees intel summaries by default, no invocation required.

**Context:** Today, `.claude/hooks/` has 4 hooks (block-banned-commands.sh, block-spec-edits.sh, classify-review-command.py, verify-hook.sh), all `PreToolUse` matchers on Bash. None surface the graph. The `UserPromptSubmit` hook event is a perfect fit — it fires BEFORE the prompt reaches the model, and returning `hookSpecificOutput.additionalContext` injects into the prompt stream without modifying the user's message. Combined with a review-family matcher (reusing `classify-review-command.py`'s logic), the hook makes every `/tpr-review`, `/review-work`, `/review-plan`, `/independent-review`, `/review-bugs`, `/tp-help`, `/fix-bug`, `/fix-next-bug` invocation start with a graph summary already in the model's context. TPR findings codex-009 and gemini-010 both flagged this gap.

**Reference implementations:**
- **Ori** `.claude/hooks/block-banned-commands.sh` (5KB): PreToolUse hook shape, python3 JSON parse, `hookSpecificOutput` response envelope — this is the template we adapt
- **Ori** `.claude/hooks/classify-review-command.py` (22KB): review-family matcher already exists; §07's hook imports/reuses its pattern rather than reimplementing
- **Ori** `.claude/skills/dual-tpr/compose-intel-summary.md` (§03): the bounded ≤500-char summary format — hook output matches this exactly so §07-injected and §03-embedded summaries are indistinguishable

**Depends on:** Section 03 (the hook emits the SSOT summary format).

---

## 07.1 Write pre-review-intel.sh with --preview dry-run

**File(s):** `.claude/hooks/pre-review-intel.sh` (new, ~150 lines)

- [ ] Create the hook with the following behavior:

  ```bash
  #!/usr/bin/env bash
  # UserPromptSubmit hook: inject bounded Intelligence Summary for review-family
  # slash-commands. Matches the format specified by
  # .claude/skills/dual-tpr/compose-intel-summary.md (§03 SSOT).
  #
  # Usage (live, via Claude Code harness):
  #   hook receives JSON on stdin with `prompt` and `cwd` fields
  #   emits JSON on stdout with hookSpecificOutput.additionalContext
  #
  # Usage (manual dry-run for operator testing):
  #   echo '{"prompt": "/tpr-review my changes", "cwd": "..."}' \
  #     | ./pre-review-intel.sh --preview
  #
  # Exit codes: 0 — always (graceful degradation matches block-banned-commands.sh)

  set -euo pipefail

  PREVIEW=0
  [[ "${1:-}" == "--preview" ]] && PREVIEW=1

  INPUT=$(cat)
  PROMPT=$(printf '%s' "$INPUT" | python3 -c \
    "import sys, json; print(json.load(sys.stdin).get('prompt',''))")

  # 1. Review-family matcher (reuse classify-review-command.py if available)
  REVIEW_FAMILY_RE='^/(tpr-review|review-work|review-plan|independent-review|review-bugs|tp-help|fix-bug|fix-next-bug)\b'
  if [[ ! "$PROMPT" =~ $REVIEW_FAMILY_RE ]]; then
    # Not a review-family command; emit empty context (no injection)
    printf '{}\n'
    exit 0
  fi

  # 2. Availability check (silent degradation)
  if ! STATUS=$(scripts/intel-query.sh status 2>/dev/null); then
    printf '{}\n'  # graph unreachable
    exit 0
  fi
  case "$STATUS" in
    *'"status":"ok"'*) ;;
    *) printf '{}\n'; exit 0 ;;
  esac

  # 3. Extract changed file paths (git + explicit mentions in prompt)
  CHANGED_FILES=$(git diff --name-only HEAD 2>/dev/null | head -5)
  # Also honor any files explicitly mentioned in the prompt (compiler/foo/bar.rs style)
  PROMPT_FILES=$(printf '%s' "$PROMPT" | grep -oE '(compiler|library|tests|scripts|docs|\.claude)/[A-Za-z0-9_/.-]+\.(rs|ori|md|py|sh)' | head -5)
  ALL_FILES=$(printf '%s\n%s\n' "$CHANGED_FILES" "$PROMPT_FILES" | sort -u | head -5)

  if [[ -z "$ALL_FILES" ]]; then
    printf '{}\n'  # nothing to query against
    exit 0
  fi

  # 4. Run file-symbols for each (bounded at 5 files)
  SUMMARY_LINES=()
  while IFS= read -r FILE; do
    [[ -z "$FILE" ]] && continue
    # Extract path fragment for file-symbols query
    FRAGMENT=$(basename "$FILE" .rs | head -c 40)
    RESULT=$(timeout 3 scripts/intel-query.sh --human file-symbols "$FRAGMENT" --repo ori 2>/dev/null | head -3)
    [[ -n "$RESULT" ]] && SUMMARY_LINES+=("- [$FILE] $RESULT")
  done <<< "$ALL_FILES"

  # 5. Build bounded summary (≤500 chars)
  if [[ ${#SUMMARY_LINES[@]} -eq 0 ]]; then
    printf '{}\n'
    exit 0
  fi

  SUMMARY="**Intelligence Summary (from intelligence graph):**"$'\n'
  for LINE in "${SUMMARY_LINES[@]}"; do
    SUMMARY+="$LINE"$'\n'
  done
  # Truncate to 500 chars
  if [[ ${#SUMMARY} -gt 500 ]]; then
    SUMMARY="${SUMMARY:0:497}…"
  fi

  # 6. Emit response
  if [[ $PREVIEW -eq 1 ]]; then
    printf '=== DRY RUN — would inject: ===\n%s\n' "$SUMMARY"
    exit 0
  fi

  python3 -c "
  import json, sys
  print(json.dumps({
      'hookSpecificOutput': {
          'hookEventName': 'UserPromptSubmit',
          'additionalContext': sys.argv[1]
      }
  }))
  " "$SUMMARY"
  ```

- [ ] Make executable: `chmod +x .claude/hooks/pre-review-intel.sh`.

- [ ] Unit-test via `--preview`: manually craft stdin JSON for each review-family command, verify the dry-run output is a valid Intelligence Summary ≤500 chars.

- [ ] Error path tests:
  - Graph unavailable → empty `{}` response
  - No changed files / no prompt-file mentions → empty `{}` response
  - Non-review-family command → empty `{}` response

- [ ] **Subsection close-out (07.1)**:
  - [ ] Hook script written, executable, and passes `--preview` smoke tests
  - [ ] Update `07.1` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 07.1** — did debugging the hook reveal any `.claude/hooks/` patterns that should be extracted to a shared library? (E.g., a `shell_lex.py` already exists for `classify-review-command.py`; consider whether hook shell scaffolding belongs in a similar helper.) Commit via `build(hooks): ...`.
  - [ ] **Run `/sync-claude` on 07.1** — CLAUDE.md §Commands already mentions `.claude/hooks/block-banned-commands.sh`. Add a note about `pre-review-intel.sh` under §Commands or §Key Paths. Commit via `docs: ...`.
  - [ ] **Repo hygiene check**.

---

## 07.2 Register in settings.json + test live firing

**File(s):** `.claude/settings.json` (or `.claude/settings.local.json` per project convention)

- [ ] Use the `/update-config` skill to add a `UserPromptSubmit` hook entry:

  ```json
  {
    "hooks": {
      "UserPromptSubmit": [
        {
          "matcher": "",
          "hooks": [
            {
              "type": "command",
              "command": "$CLAUDE_PROJECT_DIR/.claude/hooks/pre-review-intel.sh",
              "timeout": 5000
            }
          ]
        }
      ]
    }
  }
  ```

  (The matcher is empty — the hook script itself does the review-family check internally. Centralizing the matcher in the script keeps the settings file simple.)

- [ ] Start a fresh session (new conversation) and invoke `/tpr-review` with some staged changes; verify the first reviewer prompt's context includes an Intelligence Summary block.

- [ ] Invoke a NON-review command (e.g., `/commit-push`); verify no Intelligence Summary is injected — confirms the matcher is scoped correctly.

- [ ] **Subsection close-out (07.2)**:
  - [ ] Registration successful; live firing verified on review-family AND correctly skipped on non-review
  - [ ] Update `07.2` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 07.2** — is there a way to test hook wiring without starting a new session? (A `scripts/test-hook.sh <hook> <input>` harness might help.) Commit via `build(diagnostics): ...`.
  - [ ] **Run `/sync-claude` on 07.2** — `.claude/settings.json` changed; `.claude/rules/*.md` that describe hook conventions may need a note on the `UserPromptSubmit` hook family (this is the first hook of that type in the project).
  - [ ] **Repo hygiene check**.

---

## 07.3 Graceful-degradation verification

**File(s):** N/A (verification-only)

- [ ] Simulate graph unavailability (`docker stop lang-intelligence`), invoke `/tpr-review`, verify no Intelligence Summary appears and no error surfaces to the operator.

- [ ] Bring the graph back up; verify summary injection resumes on the next invocation.

- [ ] Confirm: in no scenario does the hook's output corrupt the prompt, cause the review skill to fail, or surface a visible error to the operator. Intelligence is ADDITIVE, never blocking.

- [ ] **Subsection close-out (07.3)**:
  - [ ] All degradation scenarios pass
  - [ ] Update `07.3` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 07.3** — is there a way to make graph-availability state visible to the operator without being noisy? (E.g., a status-line indicator?) Cross-reference `.claude/skills/update-config/` — that's where status-line configuration lives.
  - [ ] **Run `/sync-claude` on 07.3** — hook coverage is now stable; verify `.claude/rules/intelligence.md` §Availability section still accurately describes the degradation contract.
  - [ ] **Repo hygiene check**.

---

## 07.4 SSOT registry drift detector (`scripts/ssot-registry-audit.py`)

**File(s):** `scripts/ssot-registry-audit.py` (new).

**Surfaced by:** `plans/query-intel-adoption` §03 section-close sweep (2026-04-14). The §03 SSOT consolidation used a per-consumer registry (Step F in `compose-intel-summary.md`) documenting each consumer's exact `scripts/intel-query.sh` query set. Across 3 TPR rounds, 4 of the 6 total findings were Step F vs consumer query-set mismatches — a single automated audit would have caught them all in one pass. The Registry Contract clause in §03's SSOT commits this contract in prose; §07.4 commits it in code.

- [ ] Create `scripts/ssot-registry-audit.py`:
  - Reads an SSOT registry file (default: `.claude/skills/dual-tpr/compose-intel-summary.md`)
  - Parses Step F (or any designated registry section) to extract per-consumer query sets
  - For each listed consumer, opens the consumer file and greps for `scripts/intel-query.sh --human <subcommand>` invocations
  - Compares the registry's documented query set against the consumer's actual invocations
  - Reports mismatches: queries in registry not in consumer (over-documentation), queries in consumer not in registry (DRIFT).
- [ ] Output shape: human by default, `--json` for CI/hook consumption. Exit 0 if all entries match; non-zero with a mismatch count otherwise.
- [ ] `--registry <path>` flag to audit registries other than `compose-intel-summary.md` (future SSOTs may adopt the same pattern).
- [ ] `--fix` flag (stretch): auto-update the registry to match consumer reality (with `--dry-run` preview). Dangerous — gate behind explicit `--apply`.
- [ ] Rust/Python: Python is fine; this runs pre-commit, not in the compiler hot path.
- [ ] Integration options (pick one in close-out): (a) invoke from §07.1's hook as an additional pre-submission check, (b) lefthook pre-commit entry matching `.claude/skills/dual-tpr/compose-intel-summary.md` or any Step F-style section, (c) a standalone script invoked manually + in CI.

- [ ] **Subsection close-out (07.4)**:
  - [ ] Audit detects the 2026-04-14 TPR findings if the SSOT is reverted to pre-`dc1086ce` state (regression safety check).
  - [ ] Update `07.4` status to `complete`.

---

## 07.R Third Party Review Findings

- None.

---

**Exit Criteria:** `pre-review-intel.sh` fires on review-family prompts, injects §03-format summaries into the conversation context, and degrades silently when the graph is unavailable. No visible regression in any review workflow. `./test-all.sh` green.
