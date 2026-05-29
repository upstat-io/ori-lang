#!/usr/bin/env bash
#
# run-coverage-corpus.sh — §00 PASS-gate clause (e) orchestrator
# gate.
#
# Per the design-validation gate
# IT 126 + FAIL-branch closed exit_reason enum.
#
# Iterates the 10 categories from coverage-manifest.json; for each:
# (1) invokes ./target/release/aims-proof-checker check
# proofs/00-coverage-corpus/<category>-shape.proof --json
# (2) diffs the actual output against
# proofs/00-coverage-corpus/<category>-shape.expected
#
# Exit 0 only when ALL 10 corpus shapes:
# (a) parse without error
# (b) dispatch through the declared engine set
# (c) result matches expected (either "valid" or
# "unimplemented_engine_shape" — both are acceptable at §00
# per IT 126; engines are stubs, parser-can-parse +
# dispatcher-can-route is the gate).
#
# Exit codes match scripts/plan_corpus/exit_reasons.py
# EXIT_REASON_ROUTING:
# 0 = all 10 shapes pass
# 2 = checker_smoke_failed (one or more shapes failed) — populated
# with failing_corpus_entry: <category>-shape per the §00
# FAIL-branch table extension
# 3 = checker_build_failed (cargo build failed; pre-corpus phase)
#
# Cwd contract: script anchors to aims-proof/ via
# cd "$(dirname "$0")/.." at entry.

set -e

cd "$(dirname "$0")/.."

mkdir -p test-results

# Build the checker binary first. Pre-corpus build failure routes
# through checker_build_failed per §00 FAIL-branch table.
if ! cargo build --release -p aims-proof-checker > test-results/build.log 2>&1; then
  echo '{"status": "fail", "exit_reason": "checker_build_failed", "reason": "cargo build failed; see test-results/build.log"}' > test-results/coverage-result.json
  exit 3
fi

# 10 categories per coverage-manifest.json (the proof-checker design
# Engine-per-Category-Inventory). The list is hardcoded here to
# match the manifest order; drift between this list and
# coverage-manifest.json is a STRUCTURE:operational-rule-leak Major
# per routing.md §7 — reviewer cure: regenerate this list from the
# manifest or amend the manifest.
CATEGORIES=("L" "CN" "TF" "DP" "IC" "IA" "PL" "RL" "VF" "CH")
FAILED=()

# Reset corpus stderr/diff logs for the current run.
: > test-results/corpus-stderr.log
: > test-results/corpus-diff.log

for cat in "${CATEGORIES[@]}"; do
  PROOF_FILE="proofs/00-coverage-corpus/${cat}-shape.proof"
  EXPECTED_FILE="proofs/00-coverage-corpus/${cat}-shape.expected"
  RESULT_FILE="/tmp/corpus-${cat}.json"

  # Phase 1 — invoke the binary on the corpus shape.
  if ! ./target/release/aims-proof-checker check "${PROOF_FILE}" --json > "${RESULT_FILE}" 2>>test-results/corpus-stderr.log; then
    FAILED+=("${cat}-shape: binary exit non-zero")
    continue
  fi

  # Phase 2 — diff against expected output.
  if ! diff -u "${RESULT_FILE}" "${EXPECTED_FILE}" >> test-results/corpus-diff.log 2>&1; then
    FAILED+=("${cat}-shape: expected/actual diff")
    continue
  fi
done

# Aggregate results. Any failure routes through checker_smoke_failed
# with failing_corpus_entry populated per the §00 FAIL-branch table
# extension.
if [ ${#FAILED[@]} -ne 0 ]; then
  # Build a JSON list of failing entries. Quoted, comma-separated.
  FAILED_JSON=""
  for entry in "${FAILED[@]}"; do
    if [ -n "${FAILED_JSON}" ]; then
      FAILED_JSON="${FAILED_JSON}, "
    fi
    FAILED_JSON="${FAILED_JSON}\"${entry}\""
  done
  echo "{\"status\": \"fail\", \"exit_reason\": \"checker_smoke_failed\", \"failing_corpus_entries\": [${FAILED_JSON}], \"reason\": \"coverage corpus mismatches; see test-results/corpus-stderr.log + test-results/corpus-diff.log\"}" > test-results/coverage-result.json
  exit 2
fi

# Green path — all 10 shapes pass.
echo '{"status": "pass", "shapes_passed": 10}' > test-results/coverage-result.json
echo "coverage-corpus PASS (10/10 shapes)"
exit 0
