#!/usr/bin/env bash
#
# run-section-02-proofs.sh — §02 lattice-algebra proofs orchestrator gate.
#
# Per Annex E §AIMS §3
# success_criteria row 1: "Each of L-1..L-10 has a machine-checked
# proof artifact in proofs/02-lattice/L-N.{ext}".
#
# Two-phase orchestration:
#
# (a) cargo build --release -p aims-proof-checker — pre-run build.
# Failure routes through `lattice_infrastructure_failed` (exit 3).
# (b) For each of the 10 proofs L-1..L-10: invoke
# `./target/release/aims-proof-checker check proofs/02-lattice/L-N.proof
# --json > test-results/section-02-L-N.json`.
# Per §02 the gate is STRICTER than §00's PASS-gate (e): every
# proof MUST discharge GREEN (status: valid). Fail or
# UnimplementedEngineShape routes through a fail-branch exit_reason
# (exit 2). Default is `lattice_proof_gap_unprovable`; callers
# driving the §02 design-laboratory reformulation cycle MAY set
# LATTICE_FAILURE_CLASSIFICATION to override per success_criteria
# 3 / 4 / 5 / 7 (00-overview.md DP-3 cases).
#
# Exit codes + exit_reasons (per this checker's documented exit-code
# contract — the 5 semantic names below plus proof_search_cap_reached):
# exit 0 + lattice_axioms_proven — 10/10 green
# exit 2 + lattice_proof_gap_unprovable — default fail
# exit 2 + lattice_dimension_reformulation_required — DP-3 case 3
# exit 2 + lattice_dimension_evolution_surfaced — DP-3 case 2/4
# exit 2 + proof_search_cap_reached — DP-3 Cure 3 cap
# exit 3 + lattice_infrastructure_failed — pre-test infra
#
# LATTICE_FAILURE_CLASSIFICATION env-var (optional, on fail-paths only):
# one of {lattice_proof_gap_unprovable,
# lattice_dimension_reformulation_required,
# lattice_dimension_evolution_surfaced, proof_search_cap_reached}.
# Unset / empty → defaults to lattice_proof_gap_unprovable. Invalid
# values route through lattice_infrastructure_failed (exit 3) — runner
# rejects bad classifications rather than silently coercing.
#
# Cwd contract: anchors to aims-proof/ via cd "$(dirname "$0")/.." at
# entry.

set -e
cd "$(dirname "$0")/.."

mkdir -p test-results

readonly VALID_FAIL_CLASSIFICATIONS=(
    "lattice_proof_gap_unprovable"
    "lattice_dimension_reformulation_required"
    "lattice_dimension_evolution_surfaced"
    "proof_search_cap_reached"
)

# Validate LATTICE_FAILURE_CLASSIFICATION up front so misuse fails fast
# at infrastructure level rather than masquerading as a proof outcome.
fail_classification="${LATTICE_FAILURE_CLASSIFICATION:-lattice_proof_gap_unprovable}"
classification_valid=0
for valid in "${VALID_FAIL_CLASSIFICATIONS[@]}"; do
    if [[ "$fail_classification" == "$valid" ]]; then
        classification_valid=1
        break
    fi
done
if [[ "$classification_valid" -ne 1 ]]; then
    cat > test-results/section-02-result.json <<EOF
{"status": "fail", "exit_reason": "lattice_infrastructure_failed", "phase": "preflight", "reason": "LATTICE_FAILURE_CLASSIFICATION='${fail_classification}' not in {${VALID_FAIL_CLASSIFICATIONS[*]}}"}
EOF
    exit 3
fi

# Phase (a) — build.
if ! cargo build --release -p aims-proof-checker > test-results/section-02-build.log 2>&1; then
    cat > test-results/section-02-result.json <<EOF
{"status": "fail", "exit_reason": "lattice_infrastructure_failed", "phase": "build", "reason": "cargo build --release -p aims-proof-checker failed; see test-results/section-02-build.log"}
EOF
    exit 3
fi

# Phase (b) — per-proof discharge.
proofs_passed=0
proofs_failed=()

for n in 1 2 3 4 5 6 7 8 9 10; do
    proof_path="proofs/02-lattice/L-${n}.proof"
    out_path="test-results/section-02-L-${n}.json"

    if [[ ! -f "$proof_path" ]]; then
        cat > test-results/section-02-result.json <<EOF
{"status": "fail", "exit_reason": "lattice_infrastructure_failed", "phase": "discharge", "failing_proof": "L-${n}", "reason": "proof file missing: ${proof_path}"}
EOF
        exit 3
    fi

    ./target/release/aims-proof-checker check "$proof_path" --json > "$out_path" 2>&1 || true
    proof_status=$(python3 -c "import sys,json; print(json.loads(open('${out_path}').read()).get('status','PARSE_ERR'))" 2>/dev/null || echo "PARSE_ERR")

    if [[ "$proof_status" == "valid" ]]; then
        proofs_passed=$((proofs_passed + 1))
    else
        proofs_failed+=("L-${n}:${proof_status}")
    fi
done

if [[ ${#proofs_failed[@]} -gt 0 ]]; then
    failing_list=$(IFS=,; echo "${proofs_failed[*]}")
    cat > test-results/section-02-result.json <<EOF
{"status": "fail", "exit_reason": "${fail_classification}", "proofs_passed": ${proofs_passed}, "proofs_failed": "${failing_list}"}
EOF
    exit 2
fi

cat > test-results/section-02-result.json <<EOF
{"status": "pass", "exit_reason": "lattice_axioms_proven", "proofs_passed": ${proofs_passed}}
EOF
exit 0
