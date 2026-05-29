#!/usr/bin/env bash
#
# run-smoke-test.sh — §00 PASS-gate clause (d) orchestrator gate.
#
# Per the design-validation gate
# IT 123 + FAIL-branch closed exit_reason enum.
#
# Pipeline (3 phases, distinct FAIL-branch routing per phase):
# (a) cargo build --release -p aims-proof-checker — checker binary
# build; pre-JSON-emission failure routes through
# checker_build_failed (exit 3).
# (b) ./target/release/aims-proof-checker check
# proofs/00-smoke-test/cn-1-bidirectional.proof --json
# — binary invocation on CN-1 smoke proof; non-zero exit
# routes through checker_smoke_failed (exit 2).
# (c) diff /tmp/smoke-result.json
# proofs/00-smoke-test/cn-1-bidirectional.expected
# — verdict comparison; mismatch routes through
# checker_smoke_failed (exit 2).
#
# Exit codes match scripts/plan_corpus/exit_reasons.py
# EXIT_REASON_ROUTING:
# 0 = smoke_passes_in_ori_checker (PASS)
# 2 = checker_smoke_failed (engine-test or diff failure)
# 3 = checker_build_failed (cargo build / infrastructure failure)
#
# Cwd contract per IT 123: script anchors to aims-proof/ via
# cd "$(dirname "$0")/.." at entry; every subsequent path is
# relative to aims-proof/, NOT the repo-root cwd of the caller.

set -e

cd "$(dirname "$0")/.."

mkdir -p test-results

# Phase (a) — build the checker binary in release mode.
# Pre-JSON-emission infrastructure failure routes through
# checker_build_failed per §00 FAIL-branch table.
if ! cargo build --release -p aims-proof-checker > test-results/build.log 2>&1; then
  echo '{"status": "fail", "exit_reason": "checker_build_failed", "reason": "cargo build failed; see test-results/build.log"}' > test-results/smoke-result.json
  exit 3
fi

# Phase (b) — invoke the binary on the CN-1 smoke proof.
# Binary failure (panic before JSON emission, invalid JSON, missing
# binary) routes through checker_smoke_failed per §00 FAIL-branch
# table (binary built per phase (a); failure is in invocation, not
# build).
if ! ./target/release/aims-proof-checker check proofs/00-smoke-test/cn-1-bidirectional.proof --json > /tmp/smoke-result.json 2>test-results/smoke-stderr.log; then
  echo '{"status": "fail", "exit_reason": "checker_smoke_failed", "reason": "binary exit non-zero; see test-results/smoke-stderr.log"}' > test-results/smoke-result.json
  exit 2
fi

# Phase (c) — diff against expected output.
# Mismatch routes through checker_smoke_failed per §00 FAIL-branch
# table.
if ! diff -u /tmp/smoke-result.json proofs/00-smoke-test/cn-1-bidirectional.expected > test-results/smoke-diff.log 2>&1; then
  echo '{"status": "fail", "exit_reason": "checker_smoke_failed", "reason": "expected/actual diff; see test-results/smoke-diff.log"}' > test-results/smoke-result.json
  exit 2
fi

# Green path — copy actual output for downstream consumers + log.
cp /tmp/smoke-result.json test-results/smoke-result.json
echo "smoke-test PASS"
exit 0
