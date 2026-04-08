#!/usr/bin/env bash
# validate-tp-help-consumers.sh — stub harness for §07.3 of the dual-tpr-gemini
# plan. Exercises the tp-help dual-source concat-mode pipeline under the call
# patterns of the four downstream consumers, using stub codex/gemini binaries
# so scenarios run deterministically in <60 seconds instead of 80-120 minutes
# of real CLI wall time.
#
# The four scenarios mirror the real Scenarios 1-4 from §07.3:
#   Scenario 1: impl-hygiene-review Phase 4 (one /tp-help call pattern)
#   Scenario 2: review-plan Step 3B + Midpoint Check (two /tp-help calls)
#   Scenario 3: review-plan.md byte-identity guard
#   Scenario 4: create-plan Phase 1 (×2) + Phase 3 + Step 8B (four /tp-help calls)
#
# Each scenario:
#   1. Launches the REAL dual-invoke.sh with PATH=stub-bin-tp-help:$PATH so
#      dual-invoke.sh runs unmodified — the system under test is the real
#      transport, not a parallel implementation.
#   2. Parses the stubbed JSONL output with the REAL parse-codex-raw.py +
#      parse-gemini-raw.py.
#   3. Concatenates the two reviewer responses with the canonical HTML-comment
#      sentinel attribution format (matching the SKILL.md Step 7 contract).
#   4. Writes the concatenated output to a scratch file (simulating the
#      consumer receiving /tp-help's response).
#   5. Asserts positive pins (both stub markers present) and negative pins
#      (no sentinel leakage into any tracked file).
#
# Caveat: stub scenarios exercise the TRANSPORT-layer concat pipeline under
# consumer-like call patterns. They do NOT run the actual Claude skills
# (/impl-hygiene-review, /review-plan, /create-plan) because those are
# slash commands inside Claude Code, not shell processes. The plan's
# close-out criteria additionally require REAL integration runs where a
# user invokes each skill in a fresh Claude session with
# ORI_TPR_REVIEWERS=both; that is a separate manual step precedented by
# §04's validate-dual-tpr.sh scoping of Scenarios 1-2 as "run manually".
#
# Usage:
#   bash .claude/skills/dual-tpr/scripts/validate-tp-help-consumers.sh
#   bash .claude/skills/dual-tpr/scripts/validate-tp-help-consumers.sh --quiet
#   bash .claude/skills/dual-tpr/scripts/validate-tp-help-consumers.sh --keep-runs
#
# Exit codes:
#   0  all stub scenarios passed
#   1  one or more scenarios failed
#   2  precondition failure (dirty tree, missing stubs, missing parsers)
#
# Surfaced by §07.3 of the dual-tpr-gemini plan.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DUAL_TPR_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
STUB_BIN="$DUAL_TPR_DIR/fixtures/stub-bin-tp-help"
STUB_BIN_SPOOF="$DUAL_TPR_DIR/fixtures/stub-bin-tp-help-spoof"
CODEX_RAW="$SCRIPT_DIR/parse-codex-raw.py"
GEMINI_RAW="$SCRIPT_DIR/parse-gemini-raw.py"
REVIEW_PLAN_MD="$REPO_ROOT/.claude/commands/review-plan.md"
SECTION_07_BASELINE="$REPO_ROOT/plans/dual-tpr-gemini/section-07-review-plan-baseline.sha1"

# Source the canonical sentinel format (§07.3 SSOT fix). Provides
# TP_HELP_SENTINEL_PREFIX (for cross-cutting leakage greps),
# tp_help_make_token (per-invocation token generator), and
# tp_help_emit_block (attributed-block writer). §07.3 TPR v2 refactored
# from static TP_HELP_OPEN_* constants to a tokenized API so the
# sentinels are spoofing-resistant — the harness references these
# functions instead of hardcoding the literal strings.
# shellcheck source=./tp-help-sentinels.sh
source "$SCRIPT_DIR/tp-help-sentinels.sh"

QUIET=0
KEEP_RUNS=0
for arg in "$@"; do
  case "$arg" in
    --quiet)     QUIET=1 ;;
    --keep-runs) KEEP_RUNS=1 ;;
    --help)
      sed -n '2,38p' "$0"
      exit 0
      ;;
    *)
      printf 'unknown arg: %s (use --help)\n' "$arg" >&2
      exit 2
      ;;
  esac
done

# ── Color setup (match validate-dual-tpr.sh) ─────────────────────────
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
  printf 'error: tp-help stub binaries missing or not executable: %s\n' "$STUB_BIN" >&2
  exit 2
fi

if [[ ! -x "$STUB_BIN_SPOOF/codex" ]] || [[ ! -x "$STUB_BIN_SPOOF/gemini" ]]; then
  printf 'error: tp-help spoof stub binaries missing or not executable: %s\n' "$STUB_BIN_SPOOF" >&2
  exit 2
fi

if [[ ! -x "$CODEX_RAW" ]]; then
  printf 'error: parse-codex-raw.py missing or not executable: %s\n' "$CODEX_RAW" >&2
  exit 2
fi

if [[ ! -x "$GEMINI_RAW" ]]; then
  printf 'error: parse-gemini-raw.py missing or not executable: %s\n' "$GEMINI_RAW" >&2
  exit 2
fi

if [[ ! -f "$REVIEW_PLAN_MD" ]]; then
  printf 'error: review-plan.md not found: %s\n' "$REVIEW_PLAN_MD" >&2
  exit 2
fi

if [[ ! -f "$SECTION_07_BASELINE" ]]; then
  printf 'error: §07.PRE baseline file missing: %s\n' "$SECTION_07_BASELINE" >&2
  exit 2
fi

say "=== validate-tp-help-consumers.sh — §07.3 stub scenario harness ==="
say ""

# ── Helper: run one tp-help transport call with stubs ────────────────
#
# Args: $1 = scenario name (for log attribution)
# Env:  CONSUMER_CAPTURE_FILE = scratch file to write concatenated output to
#
# Exercises the FULL tp-help concat-mode pipeline:
#   1. Launch dual-invoke.sh with PATH override → stubs run instead of real CLIs
#   2. Parse codex.jsonl via parse-codex-raw.py
#   3. Parse gemini.jsonl via parse-gemini-raw.py
#   4. Concatenate with HTML-comment sentinel attribution (matching the
#      canonical format from .claude/skills/tp-help/SKILL.md Step 7)
#   5. Write the concatenated output to $CONSUMER_CAPTURE_FILE (simulating
#      the downstream consumer receiving /tp-help's response)
#
# Returns: 0 on success (stubs succeeded, parsers succeeded, output written)
#          1 on failure (any step returned non-zero)
run_one_tp_help_call() {
  local scenario="$1"
  local run_dir codex_raw gemini_raw token
  local reviewers="${ORI_TPR_REVIEWERS:-both}"
  # Allow callers to override the stub-bin directory for scenarios that
  # need a different stub profile (e.g., Scenario 6 uses the spoof stubs
  # to inject a stale sentinel into the reviewer body).
  local stub_bin="${TP_HELP_STUB_BIN:-$STUB_BIN}"

  run_dir=$("$SCRIPT_DIR/scratch-dir.sh")
  RUNS_TO_CLEAN+=("$run_dir")
  # Export the run dir + token so the caller can assert on state
  # artifacts ($run_dir/{codex,gemini}.{exit,walltime,skipped},
  # round.log, etc.) AND on the per-invocation token used in the
  # sentinels. §07.3 TPR v2 fix: without this, Scenario 5 could only
  # inspect the capture file and couldn't verify the .skipped marker
  # creation that was the actual bug being fixed. See GAP findings in
  # the second-pass codex + gemini reviews.
  LAST_RUN_DIR="$run_dir"

  # Per-invocation token for spoofing-resistant sentinels (§07.3 TPR
  # v2 EXPOSURE fix). Generated AFTER the scratch dir and BEFORE the
  # reviewer prompts — so the prompts do NOT contain the token and a
  # cooperative reviewer cannot accidentally quote it in its body.
  token=$(tp_help_make_token)
  LAST_RUN_TOKEN="$token"

  # Write placeholder prompts (stubs ignore prompt content but the script
  # requires the files to exist).
  printf 'tp-help stub prompt for scenario: %s\n' "$scenario" > "$run_dir/codex.prompt.md"
  printf 'tp-help stub prompt for scenario: %s\n' "$scenario" > "$run_dir/gemini.prompt.md"

  # Launch dual-invoke.sh with stub PATH override. Concat mode skips --schema
  # per §07.0 — no --schema arg passed here. ORI_TPR_REVIEWERS is propagated
  # into the transport's environment (§07.3 fix: harness must honor the env
  # var so single-reviewer mode can be tested end-to-end).
  PATH="$stub_bin:$PATH" \
  ORI_TPR_REVIEWERS="$reviewers" \
    bash "$SCRIPT_DIR/dual-invoke.sh" \
      --run "$run_dir" \
      --skill tp-help \
      --codex-prompt "$run_dir/codex.prompt.md" \
      --gemini-prompt "$run_dir/gemini.prompt.md" >/dev/null 2>&1 || return 1

  # Parse and concat only the reviewers that were actually launched.
  # The `.skipped` marker files are written by dual-invoke.sh when a
  # reviewer is filtered out; absence of the marker means the reviewer
  # ran. Mirror dual-invoke.sh's gating logic exactly.
  if [[ "$reviewers" == "codex" || "$reviewers" == "both" ]]; then
    codex_raw=$("$CODEX_RAW" --jsonl "$run_dir/codex.jsonl" 2>/dev/null) || return 1
  fi
  if [[ "$reviewers" == "gemini" || "$reviewers" == "both" ]]; then
    gemini_raw=$("$GEMINI_RAW" --jsonl "$run_dir/gemini.jsonl" 2>/dev/null) || return 1
  fi

  # Concatenate with HTML-comment sentinel attribution via the canonical
  # SSOT helper (tp-help-sentinels.sh). Only emit blocks for reviewers
  # that were actually launched — single-reviewer mode must NOT emit an
  # empty block for the skipped reviewer. Both blocks share the same
  # per-invocation token so a consumer pairing open/close by token sees
  # them as two siblings of the same run.
  {
    if [[ "$reviewers" == "codex" || "$reviewers" == "both" ]]; then
      tp_help_emit_block codex "$token" "$codex_raw"
    fi
    if [[ "$reviewers" == "both" ]]; then
      printf '\n'
    fi
    if [[ "$reviewers" == "gemini" || "$reviewers" == "both" ]]; then
      tp_help_emit_block gemini "$token" "$gemini_raw"
    fi
  } >> "$CONSUMER_CAPTURE_FILE"

  return 0
}

# ── Scenario 1: impl-hygiene-review Phase 4 (ONE tp-help call) ───────
#
# Mirrors the call pattern of .claude/skills/impl-hygiene-review/SKILL.md
# lines 319-360, which invokes /tp-help internally ONCE per phase 4 run.
# The consumer wrapper's job (outside this stub) is to process the
# concatenated prose and tag findings with [TP-CONFIRMED]/[TP-SURFACED];
# this stub harness verifies only the transport layer — that both reviewer
# markers reach the concatenated output and that the impl-hygiene-review
# SKILL.md file itself is NOT mutated during a tp-help call.
say ""
say "--- Scenario 1: impl-hygiene-review Phase 4 (1 tp-help call) ---"
HYGIENE_SKILL_MD="$REPO_ROOT/.claude/skills/impl-hygiene-review/SKILL.md"
HYGIENE_SKILL_BEFORE=$(git hash-object "$HYGIENE_SKILL_MD")

SCEN1_CAPTURE=$(mktemp -t tp-help-scen1-XXXXXX)
RUNS_TO_CLEAN+=("$SCEN1_CAPTURE")

CONSUMER_CAPTURE_FILE="$SCEN1_CAPTURE" run_one_tp_help_call scenario-1 \
  && scen1_ok=1 || scen1_ok=0

if (( scen1_ok )); then
  pass_test "scenario 1: dual-invoke.sh + parsers succeeded"
else
  fail_test "scenario 1: transport pipeline failed"
fi

if grep -q 'STUB_CODEX_RESPONSE_MARKER' "$SCEN1_CAPTURE"; then
  pass_test "scenario 1 positive pin: STUB_CODEX_RESPONSE_MARKER present in output"
else
  fail_test "scenario 1 positive pin: STUB_CODEX_RESPONSE_MARKER missing" \
    "codex stub did not reach the concat output — codex leg of dual-source broken"
fi

if grep -q 'STUB_GEMINI_RESPONSE_MARKER' "$SCEN1_CAPTURE"; then
  pass_test "scenario 1 positive pin: STUB_GEMINI_RESPONSE_MARKER present (dual-source)"
else
  fail_test "scenario 1 positive pin: STUB_GEMINI_RESPONSE_MARKER missing" \
    "gemini stub did not reach the concat output — dual-source regression"
fi

HYGIENE_SKILL_AFTER=$(git hash-object "$HYGIENE_SKILL_MD")
if [[ "$HYGIENE_SKILL_BEFORE" == "$HYGIENE_SKILL_AFTER" ]]; then
  pass_test "scenario 1 negative pin: impl-hygiene-review SKILL.md unchanged during run"
else
  fail_test "scenario 1 negative pin: impl-hygiene-review SKILL.md was MODIFIED" \
    "pre=$HYGIENE_SKILL_BEFORE post=$HYGIENE_SKILL_AFTER"
fi

# Sentinel leakage check: the captured concat output MUST contain exactly the
# canonical sentinels (once for each reviewer pair). The negative pin is that
# the consumer-side would NOT leak them into tracked files — since the stub
# harness doesn't actually run the consumer skill, we verify the sentinels
# are present in the CAPTURED output (positive) but do a repo-wide grep to
# confirm they are not present in any tracked file (negative).
# Format pin: check for tokenized sentinels using a pattern that matches
# the per-invocation token format from tp-help-sentinels.sh. Token is a
# hex string (or timestamp fallback) prefixed with '@'.
if grep -qE '<!-- tp-help-reviewer: codex @[a-zA-Z0-9-]+ -->'  "$SCEN1_CAPTURE" \
   && grep -qE '<!-- /tp-help-reviewer: codex @[a-zA-Z0-9-]+ -->' "$SCEN1_CAPTURE" \
   && grep -qE '<!-- tp-help-reviewer: gemini @[a-zA-Z0-9-]+ -->' "$SCEN1_CAPTURE" \
   && grep -qE '<!-- /tp-help-reviewer: gemini @[a-zA-Z0-9-]+ -->' "$SCEN1_CAPTURE"; then
  pass_test "scenario 1 format pin: tokenized HTML-comment sentinels present in output"
else
  fail_test "scenario 1 format pin: tokenized HTML-comment sentinels missing" \
    "concat format drift from tp-help-sentinels.sh canonical definition (expected tokenized @<hex> format)"
fi

# Token pairing pin: ALL four sentinels in a single run MUST share the
# same token. This is the spoofing-resistance contract — if the parser
# uses the same token to match open/close, the reviewer's body cannot
# inject a fake close sentinel without knowing the token. Extract the
# token from the first codex open sentinel and verify all four carry it.
scen1_token=$(grep -oE '<!-- tp-help-reviewer: codex @[a-zA-Z0-9-]+ -->' "$SCEN1_CAPTURE" | head -1 | sed -E 's/.*@([a-zA-Z0-9-]+).*/\1/')
if [[ -n "$scen1_token" ]] \
   && grep -qF "<!-- tp-help-reviewer: codex @$scen1_token -->"  "$SCEN1_CAPTURE" \
   && grep -qF "<!-- /tp-help-reviewer: codex @$scen1_token -->" "$SCEN1_CAPTURE" \
   && grep -qF "<!-- tp-help-reviewer: gemini @$scen1_token -->" "$SCEN1_CAPTURE" \
   && grep -qF "<!-- /tp-help-reviewer: gemini @$scen1_token -->" "$SCEN1_CAPTURE"; then
  pass_test "scenario 1 token pin: all four sentinels share the same per-invocation token ($scen1_token)"
else
  fail_test "scenario 1 token pin: sentinels have inconsistent tokens (spoofing-resistance broken)"
fi

# ── Scenario 2: review-plan.md Step 3B + Midpoint Check (TWO calls) ──
#
# Mirrors the call pattern of .claude/commands/review-plan.md lines 95 and 305,
# which invokes /tp-help TWICE during the 4-agent pipeline: once at Step 3B
# (blind-spot check before agents launch) and once at the Midpoint Check
# (between Agent 2 and Agent 3). Both calls should succeed with both stub
# markers present, and the command file MUST remain byte-identical throughout.
say ""
say "--- Scenario 2: review-plan Step 3B + Midpoint Check (2 tp-help calls) ---"
REVIEW_PLAN_BEFORE=$(git hash-object "$REVIEW_PLAN_MD")

SCEN2_CAPTURE=$(mktemp -t tp-help-scen2-XXXXXX)
RUNS_TO_CLEAN+=("$SCEN2_CAPTURE")

# Call 1: Step 3B blind-spot check
CONSUMER_CAPTURE_FILE="$SCEN2_CAPTURE" run_one_tp_help_call scenario-2-step3b \
  && scen2_call1_ok=1 || scen2_call1_ok=0

# Call 2: Midpoint check (between Agent 2 and Agent 3)
CONSUMER_CAPTURE_FILE="$SCEN2_CAPTURE" run_one_tp_help_call scenario-2-midpoint \
  && scen2_call2_ok=1 || scen2_call2_ok=0

if (( scen2_call1_ok && scen2_call2_ok )); then
  pass_test "scenario 2: both tp-help calls (Step 3B + Midpoint) succeeded"
else
  fail_test "scenario 2: tp-help call failure (Step 3B=$scen2_call1_ok, Midpoint=$scen2_call2_ok)"
fi

# Both calls should produce TWO occurrences of each marker in the captured file
codex_count=$(grep -c 'STUB_CODEX_RESPONSE_MARKER' "$SCEN2_CAPTURE" || true)
gemini_count=$(grep -c 'STUB_GEMINI_RESPONSE_MARKER' "$SCEN2_CAPTURE" || true)
if [[ "$codex_count" -ge 2 ]]; then
  pass_test "scenario 2 positive pin: STUB_CODEX_RESPONSE_MARKER appears $codex_count times (>=2)"
else
  fail_test "scenario 2 positive pin: STUB_CODEX_RESPONSE_MARKER count=$codex_count (expected >=2)" \
    "one or both review-plan /tp-help calls didn't reach codex"
fi
if [[ "$gemini_count" -ge 2 ]]; then
  pass_test "scenario 2 positive pin: STUB_GEMINI_RESPONSE_MARKER appears $gemini_count times (>=2, dual-source)"
else
  fail_test "scenario 2 positive pin: STUB_GEMINI_RESPONSE_MARKER count=$gemini_count (expected >=2)" \
    "one or both review-plan /tp-help calls didn't reach gemini — dual-source regression"
fi

REVIEW_PLAN_AFTER=$(git hash-object "$REVIEW_PLAN_MD")
if [[ "$REVIEW_PLAN_BEFORE" == "$REVIEW_PLAN_AFTER" ]]; then
  pass_test "scenario 2 negative pin: review-plan.md byte-identical during run"
else
  fail_test "scenario 2 negative pin: review-plan.md MUTATED during run" \
    "pre=$REVIEW_PLAN_BEFORE post=$REVIEW_PLAN_AFTER — byte-identical contract broken"
fi

# ── Scenario 3: review-plan.md byte-identity vs §07.PRE baseline ─────
#
# Same check as live Scenario 3 — verifies the §07.PRE frozen baseline
# still matches. This guards against "drifted-and-reverted" transient
# changes that git diff alone wouldn't catch.
say ""
say "--- Scenario 3: review-plan.md byte-identity vs §07.PRE baseline ---"
current=$(git hash-object "$REVIEW_PLAN_MD")
baseline=$(head -1 "$SECTION_07_BASELINE")
if [[ "$current" == "$baseline" ]]; then
  pass_test "scenario 3: review-plan.md blob matches §07.PRE baseline ($current)"
else
  fail_test "scenario 3: review-plan.md DRIFTED from §07.PRE baseline" \
    "current=$current baseline=$baseline"
fi

# Also verify git diff --exit-code is clean (catches current drift even if
# baseline compare somehow disagreed).
if git diff --exit-code "$REVIEW_PLAN_MD" >/dev/null 2>&1; then
  pass_test "scenario 3: git diff --exit-code clean on review-plan.md"
else
  fail_test "scenario 3: git diff reports drift on review-plan.md"
fi

# ── Scenario 4: create-plan Phase 1 ×2 + Phase 3 + Step 8B (4 calls) ─
#
# Mirrors the call pattern of .claude/skills/create-plan/SKILL.md which
# invokes /tp-help FOUR times: Phase 1 line 143, Phase 1 line 166
# refinement, Phase 3 line 526 architectural sanity, Step 8B line 584
# architecture sanity before user presentation. Each call should reach
# both reviewers, producing 4 occurrences of each stub marker total.
say ""
say "--- Scenario 4: create-plan Phase 1×2 + Phase 3 + Step 8B (4 tp-help calls) ---"
CREATE_PLAN_MD="$REPO_ROOT/.claude/skills/create-plan/SKILL.md"
CREATE_PLAN_BEFORE=$(git hash-object "$CREATE_PLAN_MD")

SCEN4_CAPTURE=$(mktemp -t tp-help-scen4-XXXXXX)
RUNS_TO_CLEAN+=("$SCEN4_CAPTURE")

scen4_ok=1
for call_name in phase1-call1 phase1-call2 phase3-arch-sanity step8b-arch-sanity; do
  CONSUMER_CAPTURE_FILE="$SCEN4_CAPTURE" run_one_tp_help_call "scenario-4-$call_name" \
    || { scen4_ok=0; fail_test "scenario 4 call $call_name: transport failure"; break; }
done

if (( scen4_ok )); then
  pass_test "scenario 4: all 4 tp-help calls succeeded"
fi

codex_count=$(grep -c 'STUB_CODEX_RESPONSE_MARKER' "$SCEN4_CAPTURE" || true)
gemini_count=$(grep -c 'STUB_GEMINI_RESPONSE_MARKER' "$SCEN4_CAPTURE" || true)
if [[ "$codex_count" -eq 4 ]]; then
  pass_test "scenario 4 positive pin: STUB_CODEX_RESPONSE_MARKER appears exactly 4 times"
else
  fail_test "scenario 4 positive pin: STUB_CODEX_RESPONSE_MARKER count=$codex_count (expected 4)" \
    "one or more of create-plan's 4 /tp-help calls didn't reach codex"
fi
if [[ "$gemini_count" -eq 4 ]]; then
  pass_test "scenario 4 positive pin: STUB_GEMINI_RESPONSE_MARKER appears exactly 4 times (dual-source)"
else
  fail_test "scenario 4 positive pin: STUB_GEMINI_RESPONSE_MARKER count=$gemini_count (expected 4)" \
    "one or more of create-plan's 4 /tp-help calls didn't reach gemini — dual-source regression"
fi

CREATE_PLAN_AFTER=$(git hash-object "$CREATE_PLAN_MD")
if [[ "$CREATE_PLAN_BEFORE" == "$CREATE_PLAN_AFTER" ]]; then
  pass_test "scenario 4 negative pin: create-plan SKILL.md unchanged during run"
else
  fail_test "scenario 4 negative pin: create-plan SKILL.md was MODIFIED" \
    "pre=$CREATE_PLAN_BEFORE post=$CREATE_PLAN_AFTER"
fi

# ── Scenario 5: ORI_TPR_REVIEWERS toggle (3 modes: both, codex, gemini) ─
#
# §07.3 TPR fix: previously the harness unconditionally parsed both JSONLs
# and emitted both reviewer blocks, so it false-failed under single-reviewer
# mode (codex TPR finding). This scenario verifies all three modes produce
# the correct marker-presence profile:
#   - both:   both STUB_CODEX_RESPONSE_MARKER AND STUB_GEMINI_RESPONSE_MARKER present
#   - codex:  STUB_CODEX present, STUB_GEMINI absent
#   - gemini: STUB_GEMINI present, STUB_CODEX absent
# A harness that emits both blocks regardless of ORI_TPR_REVIEWERS would
# fail the codex-only and gemini-only cases because gemini's marker would
# be missing (codex-only) or codex's marker would be missing (gemini-only).
say ""
say "--- Scenario 5: ORI_TPR_REVIEWERS toggle (both | codex | gemini) ---"

# Mode 1: ORI_TPR_REVIEWERS=both (default behavior — baseline)
SCEN5_BOTH_CAPTURE=$(mktemp -t tp-help-scen5-both-XXXXXX)
RUNS_TO_CLEAN+=("$SCEN5_BOTH_CAPTURE")
CONSUMER_CAPTURE_FILE="$SCEN5_BOTH_CAPTURE" ORI_TPR_REVIEWERS=both \
  run_one_tp_help_call scenario-5-both \
  && scen5_both_ok=1 || scen5_both_ok=0
if (( scen5_both_ok )) \
   && grep -q 'STUB_CODEX_RESPONSE_MARKER'  "$SCEN5_BOTH_CAPTURE" \
   && grep -q 'STUB_GEMINI_RESPONSE_MARKER' "$SCEN5_BOTH_CAPTURE"; then
  pass_test "scenario 5 both: both markers present (baseline)"
else
  fail_test "scenario 5 both: expected both markers in ORI_TPR_REVIEWERS=both output"
fi

# Mode 2: ORI_TPR_REVIEWERS=codex (gemini skipped)
SCEN5_CODEX_CAPTURE=$(mktemp -t tp-help-scen5-codex-XXXXXX)
RUNS_TO_CLEAN+=("$SCEN5_CODEX_CAPTURE")
CONSUMER_CAPTURE_FILE="$SCEN5_CODEX_CAPTURE" ORI_TPR_REVIEWERS=codex \
  run_one_tp_help_call scenario-5-codex \
  && scen5_codex_ok=1 || scen5_codex_ok=0
SCEN5_CODEX_RUN="$LAST_RUN_DIR"
if (( scen5_codex_ok )) \
   && grep -q 'STUB_CODEX_RESPONSE_MARKER'  "$SCEN5_CODEX_CAPTURE" \
   && ! grep -q 'STUB_GEMINI_RESPONSE_MARKER' "$SCEN5_CODEX_CAPTURE"; then
  pass_test "scenario 5 codex: codex marker present, gemini marker absent (single-reviewer codex mode)"
else
  fail_test "scenario 5 codex: expected only codex marker under ORI_TPR_REVIEWERS=codex" \
    "codex-only mode leaked gemini marker or lost codex marker"
fi

# §07.3 TPR v2 fix (codex + gemini agreed the marker-only checks above
# don't prove single-reviewer mode end-to-end). Assert on the actual
# transport state artifacts that were the bug being fixed:
#   - codex.exit + codex.walltime exist (the launched reviewer)
#   - gemini.skipped exists with the filter value (the skipped reviewer)
#   - gemini.exit MUST NOT exist (would falsely suggest gemini ran)
#   - round.log contains "gemini skipped" entry (historical record)
if [[ -f "$SCEN5_CODEX_RUN/codex.exit" && -f "$SCEN5_CODEX_RUN/codex.walltime" ]]; then
  pass_test "scenario 5 codex state: codex.exit + codex.walltime created (launched reviewer)"
else
  fail_test "scenario 5 codex state: codex.exit or codex.walltime missing — launched reviewer state artifacts not recorded"
fi
# §07.3 TPR v3 GAP fix (codex): the LAUNCHED reviewer MUST NOT have
# a .skipped marker. status-check.sh gives .skipped precedence over
# .exit, so a regression writing both files would misreport state.
# Without this negative pin, such a regression would slip through.
if [[ ! -f "$SCEN5_CODEX_RUN/codex.skipped" ]]; then
  pass_test "scenario 5 codex state: codex.skipped absent on LAUNCHED reviewer (precedence regression guard)"
else
  fail_test "scenario 5 codex state: codex.skipped present on LAUNCHED reviewer — status-check.sh precedence regression" \
    "the launched reviewer must never have a .skipped marker; dual-invoke.sh may be writing both"
fi
if [[ -f "$SCEN5_CODEX_RUN/gemini.skipped" ]]; then
  skip_marker_body=$(cat "$SCEN5_CODEX_RUN/gemini.skipped")
  if [[ "$skip_marker_body" == "ORI_TPR_REVIEWERS=codex" ]]; then
    pass_test "scenario 5 codex state: gemini.skipped present with correct filter value"
  else
    fail_test "scenario 5 codex state: gemini.skipped has wrong content" \
      "got='$skip_marker_body' expected='ORI_TPR_REVIEWERS=codex'"
  fi
else
  fail_test "scenario 5 codex state: gemini.skipped marker missing" \
    "the .skipped marker is what distinguishes skipped from still-running in status-check.sh"
fi
if [[ ! -f "$SCEN5_CODEX_RUN/gemini.exit" ]]; then
  pass_test "scenario 5 codex state: gemini.exit absent (skipped reviewer did NOT write exit file)"
else
  fail_test "scenario 5 codex state: gemini.exit present in codex-only mode — skipped reviewer wrote exit file (regression)"
fi
if grep -qF 'gemini skipped (ORI_TPR_REVIEWERS=codex)' "$SCEN5_CODEX_RUN/round.log" 2>/dev/null; then
  pass_test "scenario 5 codex state: round.log records 'gemini skipped' entry"
else
  fail_test "scenario 5 codex state: round.log missing skip entry" \
    "gemini skip was not recorded in $SCEN5_CODEX_RUN/round.log"
fi

# Mode 3: ORI_TPR_REVIEWERS=gemini (codex skipped)
SCEN5_GEMINI_CAPTURE=$(mktemp -t tp-help-scen5-gemini-XXXXXX)
RUNS_TO_CLEAN+=("$SCEN5_GEMINI_CAPTURE")
CONSUMER_CAPTURE_FILE="$SCEN5_GEMINI_CAPTURE" ORI_TPR_REVIEWERS=gemini \
  run_one_tp_help_call scenario-5-gemini \
  && scen5_gemini_ok=1 || scen5_gemini_ok=0
SCEN5_GEMINI_RUN="$LAST_RUN_DIR"
if (( scen5_gemini_ok )) \
   && ! grep -q 'STUB_CODEX_RESPONSE_MARKER'  "$SCEN5_GEMINI_CAPTURE" \
   && grep -q 'STUB_GEMINI_RESPONSE_MARKER' "$SCEN5_GEMINI_CAPTURE"; then
  pass_test "scenario 5 gemini: gemini marker present, codex marker absent (single-reviewer gemini mode)"
else
  fail_test "scenario 5 gemini: expected only gemini marker under ORI_TPR_REVIEWERS=gemini" \
    "gemini-only mode leaked codex marker or lost gemini marker"
fi

# Symmetric state assertions for gemini-only mode.
if [[ -f "$SCEN5_GEMINI_RUN/gemini.exit" && -f "$SCEN5_GEMINI_RUN/gemini.walltime" ]]; then
  pass_test "scenario 5 gemini state: gemini.exit + gemini.walltime created (launched reviewer)"
else
  fail_test "scenario 5 gemini state: gemini.exit or gemini.walltime missing — launched reviewer state artifacts not recorded"
fi
# Mirror §07.3 TPR v3 GAP fix for gemini-only mode (see codex branch).
if [[ ! -f "$SCEN5_GEMINI_RUN/gemini.skipped" ]]; then
  pass_test "scenario 5 gemini state: gemini.skipped absent on LAUNCHED reviewer (precedence regression guard)"
else
  fail_test "scenario 5 gemini state: gemini.skipped present on LAUNCHED reviewer — status-check.sh precedence regression" \
    "the launched reviewer must never have a .skipped marker; dual-invoke.sh may be writing both"
fi
if [[ -f "$SCEN5_GEMINI_RUN/codex.skipped" ]]; then
  skip_marker_body=$(cat "$SCEN5_GEMINI_RUN/codex.skipped")
  if [[ "$skip_marker_body" == "ORI_TPR_REVIEWERS=gemini" ]]; then
    pass_test "scenario 5 gemini state: codex.skipped present with correct filter value"
  else
    fail_test "scenario 5 gemini state: codex.skipped has wrong content" \
      "got='$skip_marker_body' expected='ORI_TPR_REVIEWERS=gemini'"
  fi
else
  fail_test "scenario 5 gemini state: codex.skipped marker missing"
fi
if [[ ! -f "$SCEN5_GEMINI_RUN/codex.exit" ]]; then
  pass_test "scenario 5 gemini state: codex.exit absent (skipped reviewer did NOT write exit file)"
else
  fail_test "scenario 5 gemini state: codex.exit present in gemini-only mode — skipped reviewer wrote exit file (regression)"
fi
if grep -qF 'codex skipped (ORI_TPR_REVIEWERS=gemini)' "$SCEN5_GEMINI_RUN/round.log" 2>/dev/null; then
  pass_test "scenario 5 gemini state: round.log records 'codex skipped' entry"
else
  fail_test "scenario 5 gemini state: round.log missing skip entry"
fi

# ── Scenario 6: sentinel spoofing resistance (per-invocation token) ──
#
# §07.3 TPR v2 EXPOSURE fix (both reviewers agreed). The first C8/G9
# fixture cells only tested that the parser passes sentinel-in-body
# through without crashing — they did NOT test the actual vulnerability:
# a reviewer whose prose quotes the tp-help sentinel format (e.g., when
# discussing /tp-help itself) produces a concat output with multiple
# "close" markers, and a naive consumer doing greedy open-to-close
# matching would prematurely terminate the reviewer's block.
#
# This scenario uses stub binaries (stub-bin-tp-help-spoof/) whose
# bodies contain a STALE sentinel with token @STALE_COLLIDE_ABCD1234.
# Verify that the fresh run's per-invocation token differs from the
# stale token AND that the concat output contains both the stale
# sentinel (in the body, as prose) and the fresh sentinel (at the
# block boundary, as attribution). The fresh token must NOT be the
# literal "STALE_COLLIDE_ABCD1234" string.
say ""
say "--- Scenario 6: sentinel spoofing resistance (per-invocation token) ---"
SCEN6_CAPTURE=$(mktemp -t tp-help-scen6-XXXXXX)
RUNS_TO_CLEAN+=("$SCEN6_CAPTURE")
CONSUMER_CAPTURE_FILE="$SCEN6_CAPTURE" \
  TP_HELP_STUB_BIN="$STUB_BIN_SPOOF" \
  ORI_TPR_REVIEWERS=both \
  run_one_tp_help_call scenario-6-spoof \
  && scen6_ok=1 || scen6_ok=0

if (( scen6_ok )); then
  pass_test "scenario 6: dual-invoke succeeded with spoof stubs"
else
  fail_test "scenario 6: dual-invoke failed with spoof stubs"
fi

# The stale sentinel MUST appear in the body (proves spoof stub ran).
if grep -qF '@STALE_COLLIDE_ABCD1234' "$SCEN6_CAPTURE"; then
  pass_test "scenario 6 positive pin: stale sentinel present in reviewer body (spoof stub ran)"
else
  fail_test "scenario 6 positive pin: stale sentinel missing (spoof stub may not have run)"
fi

# The fresh run's token MUST be different from the stale token.
# Extract the fresh token from the OUTERMOST open sentinel — the one
# that appears at column 0 of a line (not quoted inside body prose).
# The spoof stub's stale sentinel is inline prose, not at column 0.
scen6_fresh_token=$(grep -oE '^<!-- tp-help-reviewer: codex @[a-zA-Z0-9-]+ -->' "$SCEN6_CAPTURE" | head -1 | sed -E 's/.*@([a-zA-Z0-9-]+).*/\1/')
if [[ -n "$scen6_fresh_token" && "$scen6_fresh_token" != "STALE_COLLIDE_ABCD1234" ]]; then
  pass_test "scenario 6 spoofing-resistance pin: fresh token ($scen6_fresh_token) differs from stale token"
else
  fail_test "scenario 6 spoofing-resistance pin: fresh token missing or collides with stale" \
    "fresh='$scen6_fresh_token' stale='STALE_COLLIDE_ABCD1234'"
fi

# Count distinct tokens in the output: exactly 2 (fresh + stale).
# If a third token appears, something generated an unexpected sentinel.
scen6_distinct_tokens=$(grep -oE 'tp-help-reviewer: (codex|gemini) @[a-zA-Z0-9-]+' "$SCEN6_CAPTURE" | grep -oE '@[a-zA-Z0-9-]+' | sort -u | wc -l)
if [[ "$scen6_distinct_tokens" == "2" ]]; then
  pass_test "scenario 6 token-count pin: exactly 2 distinct tokens in output (fresh + stale)"
else
  fail_test "scenario 6 token-count pin: expected 2 distinct tokens, got $scen6_distinct_tokens"
fi

# A parser that pairs open/close by token can still locate the real
# block: fresh-open + fresh-close for each reviewer. Verify that:
#   - One fresh-open and one fresh-close exist for codex (at column 0)
#   - Same for gemini
# The stale sentinels are inline (not at column 0), so they don't
# interfere with a column-aware parser.
if grep -qE "^<!-- tp-help-reviewer: codex @${scen6_fresh_token} -->" "$SCEN6_CAPTURE" \
   && grep -qE "^<!-- /tp-help-reviewer: codex @${scen6_fresh_token} -->" "$SCEN6_CAPTURE" \
   && grep -qE "^<!-- tp-help-reviewer: gemini @${scen6_fresh_token} -->" "$SCEN6_CAPTURE" \
   && grep -qE "^<!-- /tp-help-reviewer: gemini @${scen6_fresh_token} -->" "$SCEN6_CAPTURE"; then
  pass_test "scenario 6 pairing pin: fresh-token sentinels form well-paired blocks at column 0"
else
  fail_test "scenario 6 pairing pin: fresh-token block pairing broken"
fi

# ── Cross-cutting negative pin: no sentinel leakage into ANY tracked file ─
#
# This is the load-bearing assertion for the entire dual-source /tp-help
# rollout. Attribution sentinels MUST appear in the captured consumer
# outputs (positive pins above) but MUST NOT appear anywhere in the
# tracked source tree outside plan docs, skill docs (which document the
# format), and the validate-tp-help-consumers.sh itself (which lives
# inside the stub harness and must reference the sentinel strings to
# assert on them).
#
# Allowed locations (grep-exclude): plans/, .claude/skills/tp-help/,
# .claude/skills/dual-tpr/scripts/validate-tp-help-consumers.sh itself,
# and the stub binaries / fixtures under .claude/skills/dual-tpr/fixtures/.
say ""
say "--- Cross-cutting: no sentinel leakage into tracked source outside documented locations ---"

leaks=$(git grep -l -F "$TP_HELP_SENTINEL_PREFIX" -- \
  ':(exclude)plans' \
  ':(exclude).claude/skills/tp-help' \
  ':(exclude).claude/commands/tp-help.md' \
  ':(exclude).claude/skills/dual-tpr/scripts/validate-tp-help-consumers.sh' \
  ':(exclude).claude/skills/dual-tpr/scripts/tp-help-sentinels.sh' \
  ':(exclude).claude/skills/dual-tpr/scripts/transport-tests.sh' \
  ':(exclude).claude/skills/dual-tpr/fixtures/stub-bin-tp-help' \
  ':(exclude).claude/skills/dual-tpr/fixtures/stub-bin-tp-help-spoof' \
  ':(exclude).claude/skills/dual-tpr/fixtures/raw-mode' \
  2>/dev/null || true)

if [[ -z "$leaks" ]]; then
  pass_test "cross-cutting negative pin: no tp-help-reviewer: sentinel in tracked source outside documented locations"
else
  fail_test "cross-cutting negative pin: sentinel leak detected in tracked source" \
    "files with leakage: $leaks"
fi

# Also check stub markers don't leak anywhere in tracked tree except the
# stub binaries and this harness file. The markers are in-memory-only
# scratch content; if they appear in a tracked file, something wrote stub
# output to disk outside the scratch dir.
stub_leaks=$(git grep -l -E 'STUB_(CODEX|GEMINI)_RESPONSE_MARKER' -- \
  ':(exclude).claude/skills/dual-tpr/fixtures/stub-bin-tp-help' \
  ':(exclude).claude/skills/dual-tpr/scripts/validate-tp-help-consumers.sh' \
  ':(exclude)plans/dual-tpr-gemini/section-07-tp-help.md' \
  2>/dev/null || true)

if [[ -z "$stub_leaks" ]]; then
  pass_test "cross-cutting negative pin: no STUB_*_RESPONSE_MARKER leak in tracked source"
else
  fail_test "cross-cutting negative pin: stub marker leak detected" \
    "files with leakage: $stub_leaks"
fi

# ── Cleanup ──────────────────────────────────────────────────────────
if (( ! KEEP_RUNS )); then
  for path in "${RUNS_TO_CLEAN[@]}"; do
    rm -rf "$path"
  done
fi

# ── Summary ──────────────────────────────────────────────────────────
say ""
say "=== summary ==="
say "PASS: $PASS"
say "FAIL: $FAIL"
if (( FAIL > 0 )); then
  say ""
  say "Failed scenarios:"
  for t in "${FAILED_TESTS[@]}"; do
    say "  - $t"
  done
  exit 1
fi

say ""
say "All stub scenarios passed. Note: §07.3 also requires REAL integration"
say "runs of /impl-hygiene-review, /review-plan, and /create-plan with"
say "ORI_TPR_REVIEWERS=both — those are separate manual steps (see §04's"
say "validate-dual-tpr.sh precedent for the same 'stubs cover transport,"
say "real runs cover consumer prose processing' split)."
exit 0
