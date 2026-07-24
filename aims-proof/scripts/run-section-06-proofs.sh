#!/usr/bin/env bash
#
# run-section-06-proofs.sh — §06 interprocedural-contract proofs gate.
#
# Per Annex E §AIMS §7
# §06.1 + §06.2 + §06.3 success_criteria: IC-1 (SCC topological), IC-2
# (ParamContract init), IC-3 (ParamContract join), IC-4 (ReturnContract
# join), IC-5 (EffectSummary derivation), IC-6 (FipContract POST-REALIZATION),
# IC-7 (convergence bound), IC-8a (address-taken CONSERVATIVE init),
# IC-8-REMOVED (removal soundness) each have a machine-checked proof
# artifact in proofs/06-interprocedural/<artifact>.proof.
#
# Two-phase orchestration (no compiler-conformance phase; §06 conformance
# script is separate scope):
#
# (a) cargo build --release -p aims-proof-checker — pre-run build.
# Failure routes through `interprocedural_contract_infrastructure_failed`
# (exit 3).
# (b) For each of the 9 §06 proof artifacts: invoke
# `./target/release/aims-proof-checker check
# proofs/06-interprocedural/<artifact-file>.proof --json >
# test-results/section-06-<artifact>.json`. Every proof MUST
# discharge GREEN (status: valid). Fail or UnimplementedEngineShape
# routes through a fail-branch exit_reason (exit 2).
#
# Exit codes + exit_reasons:
# exit 0 + interprocedural_contracts_proven — 9/9 green
# exit 2 + interprocedural_contract_proof_gap_unprovable — default fail
# exit 2 + interprocedural_contract_rule_evolution_surfaced — rule evolution
# exit 2 + interprocedural_contract_compiler_spec_divergence — IC vs impl
# exit 3 + interprocedural_contract_infrastructure_failed — pre-test infra
#
# INTERPROCEDURAL_CONTRACT_FAILURE_CLASSIFICATION env-var (optional, on
# fail-paths): one of
# {interprocedural_contract_proof_gap_unprovable,
# interprocedural_contract_rule_evolution_surfaced,
# interprocedural_contract_compiler_spec_divergence}.
# Unset / empty -> defaults to interprocedural_contract_proof_gap_unprovable.
# Invalid values route through interprocedural_contract_infrastructure_failed
# (exit 3).
#
# Proof artifact roster (§06.1 + §06.2 + §06.3 — 9 total):
# IC-1 -> IC-1-scc-topological.proof
# IC-2 -> IC-2-param-init.proof
# IC-3 -> IC-3-param-join.proof
# IC-4 -> IC-4-return-contract.proof
# IC-5 -> IC-5-effect-summary.proof
# IC-6 -> IC-6-fipcontract.proof
# IC-7 -> IC-7-convergence.proof
# IC-8a -> IC-8a-address-taken.proof
# IC-8-REMOVED -> IC-8-removal-soundness.proof
#
# Cwd contract: anchors to aims-proof/ via cd "$(dirname "$0")/.." at entry.

set -e
cd "$(dirname "$0")/.."

mkdir -p test-results

readonly VALID_FAIL_CLASSIFICATIONS=(
    "interprocedural_contract_proof_gap_unprovable"
    "interprocedural_contract_rule_evolution_surfaced"
    "interprocedural_contract_compiler_spec_divergence"
)

readonly PROOF_ARTIFACTS=(
    "IC-1"
    "IC-2"
    "IC-3"
    "IC-4"
    "IC-5"
    "IC-6"
    "IC-7"
    "IC-8a"
    "IC-8-REMOVED"
)

readonly PROOF_FILES=(
    "IC-1-scc-topological"
    "IC-2-param-init"
    "IC-3-param-join"
    "IC-4-return-contract"
    "IC-5-effect-summary"
    "IC-6-fipcontract"
    "IC-7-convergence"
    "IC-8a-address-taken"
    "IC-8-removal-soundness"
)

fail_classification="${INTERPROCEDURAL_CONTRACT_FAILURE_CLASSIFICATION:-interprocedural_contract_proof_gap_unprovable}"
classification_valid=0
for valid in "${VALID_FAIL_CLASSIFICATIONS[@]}"; do
    if [[ "$fail_classification" == "$valid" ]]; then
        classification_valid=1
        break
    fi
done
if [[ "$classification_valid" -ne 1 ]]; then
    cat > test-results/section-06-result.json <<EOF
{"status": "fail", "exit_reason": "interprocedural_contract_infrastructure_failed", "phase": "preflight", "reason": "INTERPROCEDURAL_CONTRACT_FAILURE_CLASSIFICATION='${fail_classification}' not in {${VALID_FAIL_CLASSIFICATIONS[*]}}"}
EOF
    exit 3
fi

# Phase (a) — build.
if ! cargo build --release -p aims-proof-checker > test-results/section-06-build.log 2>&1; then
    cat > test-results/section-06-result.json <<EOF
{"status": "fail", "exit_reason": "interprocedural_contract_infrastructure_failed", "phase": "build", "reason": "cargo build --release -p aims-proof-checker failed; see test-results/section-06-build.log"}
EOF
    exit 3
fi

# Phase (b) — per-proof discharge.
proofs_passed=0
proofs_failed=()

for i in "${!PROOF_ARTIFACTS[@]}"; do
    artifact="${PROOF_ARTIFACTS[$i]}"
    proof_file="${PROOF_FILES[$i]}"
    proof_path="proofs/06-interprocedural/${proof_file}.proof"
    out_path="test-results/section-06-${artifact}.json"

    if [[ ! -f "$proof_path" ]]; then
        cat > test-results/section-06-result.json <<EOF
{"status": "fail", "exit_reason": "interprocedural_contract_infrastructure_failed", "phase": "discharge", "failing_proof": "${artifact}", "reason": "proof file missing: ${proof_path}"}
EOF
        exit 3
    fi

    ./target/release/aims-proof-checker check "$proof_path" --json > "$out_path" 2>&1 || true
    proof_status=$(python3 -c "import sys,json; print(json.loads(open('${out_path}').read()).get('status','PARSE_ERR'))" 2>/dev/null || echo "PARSE_ERR")

    if [[ "$proof_status" == "valid" ]]; then
        proofs_passed=$((proofs_passed + 1))
    else
        proofs_failed+=("${artifact}:${proof_status}")
    fi
done

if [[ ${#proofs_failed[@]} -gt 0 ]]; then
    failing_list=$(IFS=,; echo "${proofs_failed[*]}")
    cat > test-results/section-06-result.json <<EOF
{"status": "fail", "exit_reason": "${fail_classification}", "proofs_passed": ${proofs_passed}, "proofs_failed": "${failing_list}"}
EOF
    exit 2
fi

cat > test-results/section-06-result.json <<EOF
{"status": "pass", "exit_reason": "interprocedural_contracts_proven", "proofs_passed": ${proofs_passed}}
EOF
exit 0
