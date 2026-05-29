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
# (c) For each proof passing (b) with status `valid`: emit a Lean 4
# mirror via `emit-lean4`; invoke `lean --run` on the mirror.
# Phase (c) is dormant at scaffold-time (no proof reaches it
# because the stub engines return UnimplementedShape and the
# §01A gate treats that as FAIL). The wiring exists so the
# phase-(c) FAIL-branch routes are authored against the eventual
# PASS-gate path:
# - emit-lean4 unimplemented/failed → `bootstrap_infrastructure_failed`
# (exit 3) with phase: "emit_lean4"
# - lean toolchain absent (`command -v lean` fails) →
# `bootstrap_infrastructure_failed` (exit 3) with phase:
# "lean_toolchain_missing"
# - `lean --run` non-zero exit (binary present) →
# `cross_validation_divergence` (exit 2) with
# divergence_class default "lean4_emitter_bug"
# - cross_validation_divergence_count >= 5 →
# `bootstrap_divergence_cap_reached` (exit 2)
#
# Exit codes match scripts/plan_corpus/exit_reasons.py
# EXIT_REASON_ROUTING:
# 0 = bootstrap_cross_validation_passed (11/11 green in both)
# 2 = checker_smoke_failed / cross_validation_divergence /
# bootstrap_divergence_cap_reached
# 3 = bootstrap_infrastructure_failed
#
# Cwd contract per §01A: anchors to aims-proof/ via
# cd "$(dirname "$0")/.." at entry; every subsequent path is relative
# to aims-proof/.

set -e

cd "$(dirname "$0")/.."

mkdir -p test-results proofs/01A-bootstrap/lean4-emitted

RESULT_FILE="test-results/bootstrap-result.json"
DIVERGENCE_FILE="test-results/divergence-report.json"
BUILD_LOG="test-results/bootstrap-build.log"
INFRA_LOG="test-results/bootstrap-infrastructure-failure.log"

# Iteration-counter scaffold for cross_validation_divergence cap
# enforcement per §01A Implementation Items "Cross-validation
# divergence iteration counter mechanism".
DIVERGENCE_CAP=5
PRIOR_COUNT=0
if [ -f "${RESULT_FILE}" ]; then
  # Best-effort parse of `cross_validation_divergence_count` field
  # using grep/sed (jq optional). Default 0 when field absent.
  EXTRACTED=$(grep -o '"cross_validation_divergence_count"[[:space:]]*:[[:space:]]*[0-9]\+' "${RESULT_FILE}" 2>/dev/null | grep -o '[0-9]\+' | head -1 || true)
  if [ -n "${EXTRACTED}" ]; then
    PRIOR_COUNT="${EXTRACTED}"
  fi
fi

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
  body="${body}, \"cross_validation_divergence_count\": ${PRIOR_COUNT}"
  while [ $# -ge 2 ]; do
    local k="$1"; shift
    local v="$1"; shift
    body="${body}, \"$(json_escape "${k}")\": \"$(json_escape "${v}")\""
  done
  echo "{${body}}" > "${RESULT_FILE}"
}

emit_divergence() {
  # Args: proof_file ori_verdict lean4_verdict divergence_class
  local proof_file="$1"
  local ori_verdict="$2"
  local lean4_verdict="$3"
  local divergence_class="$4"
  printf '{"proof_file": "%s", "ori_verdict": "%s", "lean4_verdict": "%s", "divergence_class": "%s"}\n' \
    "$(json_escape "${proof_file}")" \
    "$(json_escape "${ori_verdict}")" \
    "$(json_escape "${lean4_verdict}")" \
    "$(json_escape "${divergence_class}")" \
    > "${DIVERGENCE_FILE}"
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
PROOFS_FOR_PHASE_C=()

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
      PROOFS_FOR_PHASE_C+=("${entry}")
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

# Phase (c) — Lean 4 cross-validation on proofs that passed phase (b).
# At scaffold-time this loop is empty because phase (b) terminates the
# script on the first UnimplementedShape; the wiring below is authored
# against the eventual PASS-gate path per §01A Implementation Items.
for entry in "${PROOFS_FOR_PHASE_C[@]}"; do
  PROOF_FILE="proofs/01A-bootstrap/${entry}.proof"
  PROOF_NAME=$(basename "${entry}")
  LEAN_FILE="proofs/01A-bootstrap/lean4-emitted/${PROOF_NAME}.lean"

  if ! ./target/release/aims-proof-checker emit-lean4 "${PROOF_FILE}" > "${LEAN_FILE}" 2>>"${INFRA_LOG}"; then
    emit_result "fail" "bootstrap_infrastructure_failed" \
      "phase" "emit_lean4" \
      "failing_proof" "${PROOF_NAME}" \
      "reason" "emit-lean4 exit non-zero; see ${INFRA_LOG}"
    exit 3
  fi

  if ! command -v lean >/dev/null 2>&1; then
    emit_result "fail" "bootstrap_infrastructure_failed" \
      "phase" "lean_toolchain_missing" \
      "failing_proof" "${PROOF_NAME}" \
      "reason" "lean binary not on PATH; install Lean 4 toolchain per the Lean 4 cross-validation strategy"
    exit 3
  fi

  if ! lean --run "${LEAN_FILE}" >>"${INFRA_LOG}" 2>&1; then
    NEW_COUNT=$((PRIOR_COUNT + 1))
    PRIOR_COUNT="${NEW_COUNT}"
    if [ "${NEW_COUNT}" -ge "${DIVERGENCE_CAP}" ]; then
      emit_result "fail" "bootstrap_divergence_cap_reached" \
        "failing_proof" "${PROOF_NAME}" \
        "iterations" "${NEW_COUNT}" \
        "cap_reached" "true" \
        "reason" "cross_validation_divergence count >= bootstrap_divergence_reformulation_cap (${DIVERGENCE_CAP})"
      emit_divergence "${PROOF_FILE}" "valid" "rejected" "lean4_emitter_bug"
      exit 2
    fi
    emit_result "fail" "cross_validation_divergence" \
      "failing_proof" "${PROOF_NAME}" \
      "divergence_class" "lean4_emitter_bug" \
      "reason" "lean --run rejected the emitted mirror; see ${INFRA_LOG}"
    emit_divergence "${PROOF_FILE}" "valid" "rejected" "lean4_emitter_bug"
    exit 2
  fi
done

# Green path — every proof discharged GREEN in both Ori's checker AND
# Lean 4 cross-validation. Per §01A "Cross-validation divergence
# iteration counter mechanism" Implementation Item: the counter resets
# to 0 on PASS so the next §01A re-dispatch cycle starts fresh.
PRIOR_COUNT=0
emit_result "pass" "bootstrap_cross_validation_passed" \
  "kernel_passed" "${KERNEL_PASSED}" \
  "coverage_passed" "${COVERAGE_PASSED}"
echo "bootstrap PASS (kernel ${KERNEL_PASSED}/3, coverage ${COVERAGE_PASSED}/8)"
exit 0
