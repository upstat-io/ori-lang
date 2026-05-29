#!/usr/bin/env bash
#
# run-bootstrap-proofs.sh — §01A bootstrap-proof orchestrator gate.
#
# Per the Lean 4 bootstrap proofs
# Implementation Items + the §01A FAIL-branch closed exit_reason enum.
#
# Three-phase orchestration mirroring scripts/run-coverage-corpus.sh
# (which mirrors §00 IT 123):
#
# (a) cargo build --release -p aims-proof-checker — pre-bootstrap
# build. Failure routes through `bootstrap_infrastructure_failed`
# (exit 3) per the §01A FAIL-branch row 2.
# (b) For each of the 11 bootstrap proofs (3 KERNEL + 8 COVERAGE):
# invoke `./target/release/aims-proof-checker check <proof>
# --json > test-results/bootstrap-<proof-name>.json`. The §01A
# gate is STRICTER than §00's PASS-gate (e): scaffold-time
# `unimplemented_engine_shape` IS NOT acceptable for bootstrap
# PASS — §01A demands every proof discharge GREEN. Therefore
# both Fail AND UnimplementedEngineShape route through
# `checker_smoke_failed` (exit 2) with `failing_proof`,
# `failing_engine`, `reason` fields populated per §01A
# FAIL-branch row 5.
# Lean cross-validation is NOT performed here. Under the dual-prover
# design Lean proofs are hand-authored at aims-proof/lean/AimsProof/*.lean
# and cross-validated per-theorem by the dual-discharge gate
# (scripts/dual-discharge.sh) + statement-parity prelude. This runner is
# purely the §01A Ori-checker bootstrap gate; the prior placeholder-mirror
# emitter arm is retired.
#
# Exit codes match scripts/plan_corpus/exit_reasons.py
# EXIT_REASON_ROUTING:
# 0 = bootstrap_cross_validation_passed (11/11 green in the Ori checker)
# 2 = checker_smoke_failed
# 3 = bootstrap_infrastructure_failed
#
# Cwd contract per §01A: anchors to aims-proof/ via
# cd "$(dirname "$0")/.." at entry; every subsequent path is relative
# to aims-proof/.

set -e

cd "$(dirname "$0")/.."

mkdir -p test-results

RESULT_FILE="test-results/bootstrap-result.json"
BUILD_LOG="test-results/bootstrap-build.log"
INFRA_LOG="test-results/bootstrap-infrastructure-failure.log"

# JSON-string escape (handles backslash and double-quote; sufficient
# for the ASCII diagnostics this script emits).
json_escape() {
  printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g'
}

emit_result() {
  # Args: status exit_reason key1 val1 key2 val2 ...
  local status="$1"; shift
  local exit_reason="$1"; shift
  local body="\"status\": \"$(json_escape "${status}")\", \"exit_reason\": \"$(json_escape "${exit_reason}")\""
  while [ $# -ge 2 ]; do
    local k="$1"; shift
    local v="$1"; shift
    body="${body}, \"$(json_escape "${k}")\": \"$(json_escape "${v}")\""
  done
  echo "{${body}}" > "${RESULT_FILE}"
}

# Phase (a) — build.
if ! cargo build --release -p aims-proof-checker > "${BUILD_LOG}" 2>&1; then
  cp "${BUILD_LOG}" "${INFRA_LOG}" || true
  emit_result "fail" "bootstrap_infrastructure_failed" \
    "phase" "cargo_build" \
    "reason" "cargo build failed; see ${BUILD_LOG}"
  exit 3
fi

# Bootstrap proof inventory — ordered KERNEL then COVERAGE per §01A
# Implementation Items.
BOOTSTRAP_PROOFS=(
  "kernel/parser-soundness"
  "kernel/dispatch-monotonicity"
  "kernel/engine-composition-acyclicity"
  "engines/case_analysis"
  "engines/refinement"
  "engines/rc_counting"
  "engines/lattice"
  "engines/monotonicity"
  "engines/fixpoint"
  "engines/structural_induction"
  "engines/interprocedural_summary"
)

KERNEL_PASSED=0
COVERAGE_PASSED=0

# Phase (b) — per-proof Ori check.
for entry in "${BOOTSTRAP_PROOFS[@]}"; do
  PROOF_FILE="proofs/01A-bootstrap/${entry}.proof"
  PROOF_NAME=$(basename "${entry}")
  RESULT_JSON="test-results/bootstrap-${PROOF_NAME}.json"

  if ! ./target/release/aims-proof-checker check "${PROOF_FILE}" --json > "${RESULT_JSON}" 2>>"${INFRA_LOG}"; then
    # Non-zero exit before JSON emission ⇒ infrastructure-class
    # failure (binary panic, parser bug, missing file). Distinct from
    # engine fail per §01A FAIL-branch row 2 vs row 5.
    emit_result "fail" "bootstrap_infrastructure_failed" \
      "phase" "binary_invocation" \
      "failing_proof" "${PROOF_NAME}" \
      "reason" "binary exit non-zero before JSON emission; see ${INFRA_LOG}"
    exit 3
  fi

  # Parse status field via grep (jq optional; keeps script
  # dependency-free per the proof-checker design Kernel Verification Methodology).
  STATUS=$(grep -o '"status"[[:space:]]*:[[:space:]]*"[^"]*"' "${RESULT_JSON}" | sed -e 's/.*: *"//' -e 's/"$//' || true)
  REASON=$(grep -o '"reason"[[:space:]]*:[[:space:]]*"[^"]*"' "${RESULT_JSON}" | sed -e 's/.*: *"//' -e 's/"$//' || true)
  FAILING_ENGINE=$(grep -o '"failing_engine"[[:space:]]*:[[:space:]]*"[^"]*"' "${RESULT_JSON}" | sed -e 's/.*: *"//' -e 's/"$//' || true)
  if [ -z "${FAILING_ENGINE}" ]; then
    # The §00 JSON contract suppresses `failing_engine` on
    # UnimplementedShape per checker.rs::to_json. Derive a semantic
    # fallback from the proof path: COVERAGE proofs map 1:1 to engine
    # names (engines/<engine_name>.proof); KERNEL proofs target the
    # whole engine inventory and report "<all engines>" so the
    # downstream consumer can route per §01A FAIL-branch row 5.
    case "${entry}" in
      engines/*)
        FAILING_ENGINE="${PROOF_NAME}"
        ;;
      kernel/*)
        FAILING_ENGINE="<all engines>"
        ;;
      *)
        FAILING_ENGINE="(none)"
        ;;
    esac
  fi
  [ -z "${REASON}" ] && REASON="(no reason emitted)"

  case "${STATUS}" in
    valid)
      if [[ "${entry}" == kernel/* ]]; then
        KERNEL_PASSED=$((KERNEL_PASSED + 1))
      else
        COVERAGE_PASSED=$((COVERAGE_PASSED + 1))
      fi
      ;;
    unimplemented_engine_shape|fail)
      # §01A stricter gate: scaffold-time UnimplementedEngineShape is
      # NOT acceptable for bootstrap PASS. Both shapes route through
      # checker_smoke_failed per §01A FAIL-branch row 5.
      emit_result "fail" "checker_smoke_failed" \
        "failing_proof" "${PROOF_NAME}" \
        "failing_engine" "${FAILING_ENGINE}" \
        "reason" "${REASON}"
      exit 2
      ;;
    *)
      emit_result "fail" "bootstrap_infrastructure_failed" \
        "phase" "json_parse" \
        "failing_proof" "${PROOF_NAME}" \
        "reason" "unexpected status field value: ${STATUS}"
      exit 3
      ;;
  esac
done

# Green path — every bootstrap proof discharged GREEN in the Ori
# checker. Lean cross-validation is owned by the dual-discharge gate
# (scripts/dual-discharge.sh); this runner no longer emits Lean mirrors.
emit_result "pass" "bootstrap_cross_validation_passed" \
  "kernel_passed" "${KERNEL_PASSED}" \
  "coverage_passed" "${COVERAGE_PASSED}"
echo "bootstrap PASS (kernel ${KERNEL_PASSED}/3, coverage ${COVERAGE_PASSED}/8)"
exit 0
