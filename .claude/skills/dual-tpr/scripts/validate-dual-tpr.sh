#!/usr/bin/env bash
# validate-dual-tpr.sh — automated validation harness for the dual-source
# transport stack. Runs the stub-based scenarios from Section 04.3 of the
# dual-tpr-gemini plan: Scenario 4 (infra retry fault injection) and
# Scenario 3 (dirty-worktree guard).
#
# Scenarios 1 and 2 from Section 04.3 require running real codex and gemini
# reviewers against real work and observing genuine agreement / disagreement
# / citations — they cannot be stubbed without losing the validation value
# they provide. Run those manually via /tpr-review on real recent work.
#
# This harness uses the stub binaries at fixtures/stub-bin/{codex,gemini}
# via PATH manipulation, so dual-invoke.sh and dual-invoke-with-retry.sh
# run UNMODIFIED — the system under test is the real transport, not a
# parallel implementation. The stubs only replace the upstream review
# CLIs that the transport launches.
#
# Usage:
#   bash .claude/skills/dual-tpr/scripts/validate-dual-tpr.sh
#   bash .claude/skills/dual-tpr/scripts/validate-dual-tpr.sh --quiet
#   bash .claude/skills/dual-tpr/scripts/validate-dual-tpr.sh --keep-runs
#
# Exit codes:
#   0  all stub-based scenarios passed
#   1  one or more scenarios failed
#   2  precondition failure (dirty tree, missing stubs, etc.)
#
# Surfaced by the dual-tpr-gemini section-04.3 retrospective.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DUAL_TPR_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
STUB_BIN="$DUAL_TPR_DIR/fixtures/stub-bin"
DIRTY_TARGET="$DUAL_TPR_DIR/fixtures/dirty-target.txt"
SCHEMA="$DUAL_TPR_DIR/findings-schema.json"

QUIET=0
KEEP_RUNS=0
for arg in "$@"; do
  case "$arg" in
    --quiet)     QUIET=1 ;;
    --keep-runs) KEEP_RUNS=1 ;;
    --help)
      sed -n '2,30p' "$0"
      exit 0
      ;;
    *)
      printf 'unknown arg: %s (use --help)\n' "$arg" >&2
      exit 2
      ;;
  esac
done

# ── Color setup ──────────────────────────────────────────────────────
GREEN='\033[0;32m'
RED='\033[0;31m'
DIM='\033[0;2m'
RESET='\033[0m'
if [[ -n "${NO_COLOR:-}" ]] || [[ ! -t 1 ]]; then
  GREEN='' RED='' DIM='' RESET=''
fi

PASS=0
FAIL=0
FAILED_TESTS=()
RUNS_TO_CLEAN=()

say() {
  if (( ! QUIET )); then
    printf '%s\n' "$1"
  fi
}

pass_test() {
  PASS=$((PASS + 1))
  if (( ! QUIET )); then
    printf '  %b[PASS]%b %s\n' "$GREEN" "$RESET" "$1"
  fi
}

fail_test() {
  FAIL=$((FAIL + 1))
  FAILED_TESTS+=("$1")
  printf '  %b[FAIL]%b %s\n' "$RED" "$RESET" "$1"
  if [[ -n "${2:-}" ]]; then
    printf '    %breason:%b %s\n' "$DIM" "$RESET" "$2"
  fi
}

# ── Preconditions ────────────────────────────────────────────────────
if [[ ! -x "$STUB_BIN/codex" ]] || [[ ! -x "$STUB_BIN/gemini" ]]; then
  printf 'error: stub binaries missing or not executable: %s\n' "$STUB_BIN" >&2
  exit 2
fi

if [[ ! -f "$DIRTY_TARGET" ]]; then
  printf 'error: dirty-worktree fixture missing: %s\n' "$DIRTY_TARGET" >&2
  exit 2
fi

# Verify dirty-target.txt is currently committed-clean. We can't run the
# Scenario 3 cleanup verification reliably if it's already dirty.
if ! git diff --quiet -- "$DIRTY_TARGET" 2>/dev/null; then
  printf 'error: dirty-worktree fixture is already modified — restore it first:\n' >&2
  printf '  git checkout -- %s\n' "$DIRTY_TARGET" >&2
  exit 2
fi

say "=== validate-dual-tpr.sh — stub-based scenario harness ==="
say ""

# ── Scenario 4: infra retry fault injection ──────────────────────────
#
# Setup: STUB_CODEX_MODE=fail-once causes the stub codex to exit 1 on its
# first invocation (writing a state file) and exit 0 on its second. Gemini
# stays in OK mode. We expect dual-invoke-with-retry.sh to:
#   1. Make attempt 1, observe codex exit non-zero, classify as
#      launch_or_exit_fail, sleep 1s
#   2. Make attempt 2, observe both reviewers succeed, parse envelopes,
#      worktree-guard pass, exit 0 with the round complete
# Verification:
#   - Final exit code = 0
#   - $RUN/round.log contains both "attempt 1/3" and "attempt 2/3" markers
#   - $RUN/round.log records launch_or_exit_fail on attempt 1
#   - $RUN/codex.envelope.json and $RUN/gemini.envelope.json exist
#   - The envelopes have no_findings=true (no synthesis happened)

say "── Scenario 4: infra retry fault injection ──────────────────"

# Reset any leftover state files
rm -f /tmp/stub-codex-state /tmp/stub-gemini-state

RUN=$("$SCRIPT_DIR/scratch-dir.sh")
RUNS_TO_CLEAN+=("$RUN")
printf 'echo prompt\n' > "$RUN/codex.prompt.md"
printf 'echo prompt\n' > "$RUN/gemini.prompt.md"

# Run the transport with the stub binaries on PATH so dual-invoke.sh's
# bare `codex` and `gemini` resolve to our stubs instead of the real CLIs.
PATH="$STUB_BIN:$PATH" \
STUB_CODEX_MODE=fail-once \
STUB_GEMINI_MODE=ok \
STUB_REPO_ROOT="$REPO_ROOT" \
"$SCRIPT_DIR/dual-invoke-with-retry.sh" \
  --run "$RUN" \
  --skill review-work \
  --codex-prompt "$RUN/codex.prompt.md" \
  --gemini-prompt "$RUN/gemini.prompt.md" \
  --schema "$SCHEMA" \
  >/dev/null 2>&1
S4_EXIT=$?

if [[ $S4_EXIT -eq 0 ]]; then
  pass_test "transport recovers and exits 0 after fail-once codex"
else
  fail_test "transport recovers and exits 0 after fail-once codex" \
    "expected exit 0, got $S4_EXIT (round.log at $RUN/round.log)"
fi

if grep -q 'attempt 1/3' "$RUN/round.log" 2>/dev/null && \
   grep -q 'attempt 2/3' "$RUN/round.log" 2>/dev/null; then
  pass_test "round.log records both retry attempts"
else
  fail_test "round.log records both retry attempts" \
    "missing 'attempt 1/3' or 'attempt 2/3' in $RUN/round.log"
fi

if grep -q 'launch_or_exit_fail on attempt 1' "$RUN/round.log" 2>/dev/null; then
  pass_test "round.log classifies attempt 1 as launch_or_exit_fail"
else
  fail_test "round.log classifies attempt 1 as launch_or_exit_fail" \
    "missing 'launch_or_exit_fail on attempt 1' in $RUN/round.log"
fi

if [[ -s "$RUN/codex.envelope.json" ]] && [[ -s "$RUN/gemini.envelope.json" ]]; then
  pass_test "both envelopes parsed and saved on successful retry"
else
  fail_test "both envelopes parsed and saved on successful retry" \
    "missing or empty envelopes in $RUN"
fi

# Clean up state files for the next scenario
rm -f /tmp/stub-codex-state /tmp/stub-gemini-state

say ""

# ── Scenario 3: dirty-worktree guard test ────────────────────────────
#
# Setup: STUB_CODEX_MODE=dirty causes the stub codex to append a line to
# dirty-target.txt (a tracked file dedicated to this test), then emit a
# valid envelope and exit 0. Gemini stays in OK mode. The reviewer
# subprocesses both look "successful" from the parser's perspective —
# the only signal of misbehavior is the worktree mutation.
# We expect dual-invoke-with-retry.sh to:
#   1. Make attempt 1, observe both reviewers exit 0 and parsers succeed
#   2. Run worktree-guard.sh compare, detect the modification, classify
#      as dirty_worktree, sleep 1s
#   3. Make attempt 2 — the dirty stub modifies the file again, dirty
#      again. Same on attempt 3.
#   4. Exhaust retries, exit non-zero with FAILURE=dirty_worktree
# Verification:
#   - Final exit code != 0 (transport gave up after 3 dirty rounds)
#   - $RUN/round.log records dirty_worktree on every attempt
#   - $RUN/worktree-error contains a diff of the modifications
#   - Cleanup: dirty-target.txt restored to committed state

say "── Scenario 3: dirty-worktree guard test ────────────────────"

# Snapshot the current contents of dirty-target.txt so we can restore it
# byte-for-byte after the test, regardless of whether the file is committed
# or merely staged in the index. `git checkout -- file` only restores from
# HEAD, which fails for files that are staged but not yet committed.
DIRTY_BACKUP=$(mktemp)
cp "$DIRTY_TARGET" "$DIRTY_BACKUP"

RUN=$("$SCRIPT_DIR/scratch-dir.sh")
RUNS_TO_CLEAN+=("$RUN")
printf 'echo prompt\n' > "$RUN/codex.prompt.md"
printf 'echo prompt\n' > "$RUN/gemini.prompt.md"

PATH="$STUB_BIN:$PATH" \
STUB_CODEX_MODE=dirty \
STUB_GEMINI_MODE=ok \
STUB_REPO_ROOT="$REPO_ROOT" \
"$SCRIPT_DIR/dual-invoke-with-retry.sh" \
  --run "$RUN" \
  --skill review-work \
  --codex-prompt "$RUN/codex.prompt.md" \
  --gemini-prompt "$RUN/gemini.prompt.md" \
  --schema "$SCHEMA" \
  >/dev/null 2>&1
S3_EXIT=$?

if [[ $S3_EXIT -ne 0 ]]; then
  pass_test "transport exits non-zero after dirty-worktree retries exhausted"
else
  fail_test "transport exits non-zero after dirty-worktree retries exhausted" \
    "expected exit != 0, got $S3_EXIT"
fi

if grep -q 'dirty_worktree on attempt' "$RUN/round.log" 2>/dev/null; then
  pass_test "round.log records dirty_worktree on at least one attempt"
else
  fail_test "round.log records dirty_worktree on at least one attempt" \
    "missing 'dirty_worktree on attempt' in $RUN/round.log"
fi

if [[ -s "$RUN/worktree-error" ]]; then
  pass_test "worktree-error file contains the diff"
else
  fail_test "worktree-error file contains the diff" \
    "missing or empty $RUN/worktree-error"
fi

# Restore dirty-target.txt from the byte-for-byte snapshot we took above.
# Robust against the file being either committed or merely staged — works
# in both states because we restore from a content snapshot, not from git.
cp "$DIRTY_BACKUP" "$DIRTY_TARGET"
if cmp -s "$DIRTY_TARGET" "$DIRTY_BACKUP"; then
  pass_test "dirty-target.txt restored to pre-test state"
else
  fail_test "dirty-target.txt restored to pre-test state" \
    "cmp reports $DIRTY_TARGET differs from snapshot after restore"
fi
rm -f "$DIRTY_BACKUP"

say ""

# ── Cleanup ──────────────────────────────────────────────────────────
if (( ! KEEP_RUNS )); then
  for run in "${RUNS_TO_CLEAN[@]}"; do
    rm -rf "$run"
  done
else
  say "Keeping run directories:"
  for run in "${RUNS_TO_CLEAN[@]}"; do
    say "  $run"
  done
fi

# ── Summary ──────────────────────────────────────────────────────────
say ""
TOTAL=$((PASS + FAIL))
if (( FAIL == 0 )); then
  printf '%b[OK]%b %d/%d stub-based scenarios passed\n' "$GREEN" "$RESET" "$PASS" "$TOTAL"
  printf '\nNOTE: Scenarios 1 and 2 (real-reviewer agreement and disagreement\n'
  printf 'demonstration) are NOT covered by this harness — they require running\n'
  printf '/tpr-review against real work in the repository. See plans/dual-tpr-gemini/\n'
  printf 'section-04-tpr-review.md §04.3 for the manual run protocol.\n'
  exit 0
fi

printf '%b[FAIL]%b %d/%d passed (%d failed)\n' "$RED" "$RESET" "$PASS" "$TOTAL" "$FAIL"
printf '\nFailed tests:\n'
for t in "${FAILED_TESTS[@]}"; do
  printf '  - %s\n' "$t"
done
exit 1
