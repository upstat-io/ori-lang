#!/usr/bin/env bash
#
# run-section-09-proofs.sh — §09 verification-layer proofs gate.
#
# Per Annex E §AIMS §9 §09.1
# success_criteria: invokes aims-proof/checker/ across the §09-verification
# corpus ONLY (proofs/09-verification/ VF-1..VF-8 + VF-comp), emits per-VF
# pass/fail, and runs the compiler-conformance cross-walk (each proven VF-N ->
# its shipped verifier site in compiler/ori_arc/src/{verify,aims/verify}/).
# Exits non-zero on any VF-N proof-check failure.
#
# Scope guard: this runner verifies §09's OWN proof corpus. VF-5/VF-6/VF-7/VF-8
# are TARGET-only mandates the verification stack asserts as a whole (no single
# shipped verifier function) -> conformance: target-only. The full nightly
# aggregation across the critical-proof cross-validation set is owned by §15.
#
# Phases:
# (a) cargo build --release -p aims-proof-checker — pre-run build.
# Failure -> verification_layer_infrastructure_failed (exit 3).
# (b) per-proof discharge: every artifact MUST be status: valid. Fail or
# UnimplementedEngineShape -> a fail-branch exit_reason (exit 2).
# (c) compiler-conformance cross-walk: each proven VF-N -> shipped verifier
# site; emit per-VF conformance: pass | divergent | target-only. A
# shipped-layer site missing -> verification_layer_compiler_spec_divergence
# (exit 2).
#
# Exit codes + exit_reasons (per scripts/plan_corpus/exit_reasons.py):
# exit 0 + verification_layers_proven — all green
# exit 2 + verification_layer_proof_gap_unprovable — default proof fail
# exit 2 + verification_layer_reformulation_required — layer reformulation
# exit 2 + verification_layer_evolution_surfaced — layer evolution
# exit 2 + verification_layer_compiler_spec_divergence — conformance divergent
# exit 3 + verification_layer_infrastructure_failed — pre-test infra
#
# VERIFICATION_LAYER_FAILURE_CLASSIFICATION env-var (optional, on fail-paths):
# one of {verification_layer_proof_gap_unprovable,
# verification_layer_reformulation_required,
# verification_layer_evolution_surfaced}. Unset -> defaults to
# verification_layer_proof_gap_unprovable. Invalid -> infrastructure_failed.
#
# Cwd contract: anchors to aims-proof/ via cd "$(dirname "$0")/.." at entry.

set -e
cd "$(dirname "$0")/.."

mkdir -p test-results

readonly VALID_FAIL_CLASSIFICATIONS=(
    "verification_layer_proof_gap_unprovable"
    "verification_layer_reformulation_required"
    "verification_layer_evolution_surfaced"
)

# §09-verification corpus: VF-1..VF-8 active + VF-comp composition.
readonly PROOF_FILES=(
    "VF-1" "VF-2" "VF-3" "VF-4"
    "VF-5" "VF-6" "VF-7" "VF-8"
    "VF-comp"
)

# Compiler-conformance cross-walk: each proven VF-N -> shipped verifier site
# (relative to ../). `SHIPPED` rules check the site exists;
# `TARGET` rules are whole-stack mandates with no single shipped verifier
# function -> conformance: target-only, NOT divergent. Format:
# "VF-N|class|path" where class ∈ {SHIPPED, TARGET}.
#
# VF-1 -> structural verify check_function (verify/mod.rs)
# VF-2 -> check_function_with_contract / AbsentParamHasUses (verify/mod.rs)
# VF-3 -> oracle verify_coherence (aims/verify/oracle.rs)
# VF-4 -> fip verify_fip_contract (aims/verify/fip.rs)
# VF-5/6/7/8/comp -> TARGET-only whole-stack mandates
readonly CONFORMANCE_MAP=(
    "VF-1|SHIPPED|compiler/ori_arc/src/verify/mod.rs|check_function"
    "VF-2|SHIPPED|compiler/ori_arc/src/verify/mod.rs|check_function_with_contract"
    "VF-3|SHIPPED|compiler/ori_arc/src/aims/verify/oracle.rs|verify_coherence"
    "VF-4|SHIPPED|compiler/ori_arc/src/aims/verify/fip.rs|verify_fip_contract"
    "VF-5|TARGET|end-to-end-mandate|-"
    "VF-6|TARGET|whole-stack-agreement|-"
    "VF-7|TARGET|active-rewrite-three-tier|-"
    "VF-8|TARGET|universal-coverage|-"
    "VF-comp|TARGET|layered-stack-composition|-"
)

fail_classification="${VERIFICATION_LAYER_FAILURE_CLASSIFICATION:-verification_layer_proof_gap_unprovable}"
classification_valid=0
for valid in "${VALID_FAIL_CLASSIFICATIONS[@]}"; do
    if [[ "$fail_classification" == "$valid" ]]; then
        classification_valid=1
        break
    fi
done
if [[ "$classification_valid" -ne 1 ]]; then
    cat > test-results/section-09-result.json <<EOF
{"status": "fail", "exit_reason": "verification_layer_infrastructure_failed", "phase": "preflight", "reason": "VERIFICATION_LAYER_FAILURE_CLASSIFICATION='${fail_classification}' not in {${VALID_FAIL_CLASSIFICATIONS[*]}}"}
EOF
    exit 3
fi

# Phase (a) — build.
if ! cargo build --release -p aims-proof-checker > test-results/section-09-build.log 2>&1; then
    cat > test-results/section-09-result.json <<EOF
{"status": "fail", "exit_reason": "verification_layer_infrastructure_failed", "phase": "build", "reason": "cargo build --release -p aims-proof-checker failed; see test-results/section-09-build.log"}
EOF
    exit 3
fi

readonly CHECKER="./target/release/aims-proof-checker"

# Phase (b) — per-proof discharge.
proofs_passed=0
proofs_failed=()

for proof in "${PROOF_FILES[@]}"; do
    proof_path="proofs/09-verification/${proof}.proof"
    out_path="test-results/section-09-${proof}.json"

    if [[ ! -f "$proof_path" ]]; then
        cat > test-results/section-09-result.json <<EOF
{"status": "fail", "exit_reason": "verification_layer_infrastructure_failed", "phase": "discharge", "failing_proof": "${proof}", "reason": "proof file missing: ${proof_path}"}
EOF
        exit 3
    fi

    "$CHECKER" check "$proof_path" --json > "$out_path" 2>&1 || true
    proof_status=$(python3 -c "import sys,json; print(json.loads(open('${out_path}').read()).get('status','PARSE_ERR'))" 2>/dev/null || echo "PARSE_ERR")

    if [[ "$proof_status" == "valid" ]]; then
        proofs_passed=$((proofs_passed + 1))
    else
        proofs_failed+=("${proof}:${proof_status}")
    fi
done

if [[ ${#proofs_failed[@]} -gt 0 ]]; then
    failing_list=$(IFS=,; echo "${proofs_failed[*]}")
    cat > test-results/section-09-result.json <<EOF
{"status": "fail", "exit_reason": "${fail_classification}", "phase": "discharge", "proofs_passed": ${proofs_passed}, "proofs_failed": "${failing_list}"}
EOF
    exit 2
fi

# Phase (c) — compiler-conformance cross-walk.
conformance_divergent=()
conformance_pass=0
conformance_target_only=0
for entry in "${CONFORMANCE_MAP[@]}"; do
    IFS='|' read -r vf_n cls path sym <<< "$entry"
    if [[ "$cls" == "TARGET" ]]; then
        conformance_target_only=$((conformance_target_only + 1))
        continue
    fi
    # SHIPPED: the proven layer must have a present verifier site WITH the named
    # verifier FUNCTION DEFINED (anchored `pub fn <sym>(`). File-existence alone
    # passes even if the function were deleted/renamed; a bare symbol-reference
    # grep would false-pass a deleted-but-still-referenced fn (call site / comment
    # / use). Definition-presence is the real proven-VF↔shipped-verifier
    # conformance the success_criterion promises to mechanically detect.
    if [[ -f "../${path}" ]] && grep -qE "pub fn ${sym}\(" "../${path}"; then
        conformance_pass=$((conformance_pass + 1))
    else
        conformance_divergent+=("${vf_n}:${path}:${sym}")
    fi
done

if [[ ${#conformance_divergent[@]} -gt 0 ]]; then
    divergent_list=$(IFS=,; echo "${conformance_divergent[*]}")
    cat > test-results/section-09-result.json <<EOF
{"status": "fail", "exit_reason": "verification_layer_compiler_spec_divergence", "phase": "conformance_cross_walk", "proofs_passed": ${proofs_passed}, "conformance_divergent": "${divergent_list}"}
EOF
    exit 2
fi

cat > test-results/section-09-result.json <<EOF
{"status": "pass", "exit_reason": "verification_layers_proven", "proofs_passed": ${proofs_passed}, "conformance_pass": ${conformance_pass}, "conformance_target_only": ${conformance_target_only}}
EOF
exit 0
