#!/usr/bin/env bash
#
# run-section-08-proofs.sh — §08 realization-rule proofs gate.
#
# Per Annex E §AIMS §8 §08.14
# success_criteria: invokes aims-proof/checker/ across the §08-realization
# corpus ONLY (proofs/08-realization/ RL-1..RL-34 + RL-11a/14a/15a/18a +
# RL-13 removal confirmation + RL-1-RL-2-composition + RL-comp) plus the
# §08-owned RL-31 Lean 4 cross-validation, emits per-RL pass/fail keyed to the
# Per-RL Proof-Status Tracking table, and runs the compiler-conformance
# cross-walk (each proven RL-N -> shipped realize/ emission site). Exits
# non-zero on any RL-N proof-check failure.
#
# Scope guard (per §08.14): this runner verifies §08's OWN proof corpus +
# §08-owned RL-31 Lean 4 cross-validation. It does NOT reach into §11's
# coexistence-handshake critical-proof set (§11 depends on §08 RL-31). The
# full nightly aggregation across the critical-proof cross-validation set
# (§01A + §08 RL-31 + §11) is owned by §15.
#
# Phases:
# (a) cargo build --release -p aims-proof-checker — pre-run build.
# Failure -> realization_rule_infrastructure_failed (exit 3).
# (b) per-proof discharge: every artifact MUST be status: valid. Fail or
# UnimplementedEngineShape -> a fail-branch exit_reason (exit 2).
# RL-31 Lean cross-validation is owned by the dual-discharge gate
# (scripts/dual-discharge.sh) against the hand-authored
# lean/AimsProof/Realization.lean module; the prior placeholder-mirror
# emitter arm is retired.
# (d) compiler-conformance cross-walk: each proven RL-N -> shipped emission
# site; emit per-RL conformance: pass | divergent | target-only. A
# shipped-rule site missing -> realization_rule_compiler_spec_divergence
# (exit 2).
#
# Exit codes + exit_reasons (per this checker's documented exit-code contract):
# exit 0 + realization_rules_proven — all green
# exit 2 + realization_rule_proof_gap_unprovable — default proof fail
# exit 2 + realization_rule_reformulation_required — rule reformulation
# exit 2 + realization_rule_evolution_surfaced — rule evolution
# exit 2 + realization_rule_compiler_spec_divergence — conformance divergent
# exit 3 + realization_rule_infrastructure_failed — pre-test infra
#
# REALIZATION_RULE_FAILURE_CLASSIFICATION env-var (optional, on fail-paths):
# one of {realization_rule_proof_gap_unprovable,
# realization_rule_reformulation_required,
# realization_rule_evolution_surfaced}. Unset -> defaults to
# realization_rule_proof_gap_unprovable. Invalid -> infrastructure_failed.
#
# Cwd contract: anchors to aims-proof/ via cd "$(dirname "$0")/.." at entry.

set -e
cd "$(dirname "$0")/.."

mkdir -p test-results

# --locality-section mode (per §17 SC9): emit a dedicated locality_conformance
# block in test-results/section-17-locality-result.json with per-manifest-rule
# verdict rows {rule_id, conformance, proof_artifact, unblock_condition,
# blocked_by, annotation_targets}. Consumed by §12 / §15 / §16 / §17 flip
# protocol. Sibling-extension shape per §17 SC10.
#
# Probe inputs (read from checker crate after build):
# LATTICE_VARIANT_COUNT (usize) — count of `enum Locality` variants in
# compiler/ori_arc/src/aims/lattice/dimensions.rs
# MAY_ESCAPE_IS_STORED (bool) — whether `ParamContract` in
# compiler/ori_arc/src/aims/contract/mod.rs carries
# `may_escape: <type>` as a stored struct field (vs derived method).
#
# Verdict per manifest rule:
# LATTICE_VARIANT_COUNT == 5 AND MAY_ESCAPE_IS_STORED == false -> `pass`
# LATTICE_VARIANT_COUNT == 4 OR MAY_ESCAPE_IS_STORED == true -> `divergent-pending(blocked-by: locality-representation-unification)`
# Else (manifest-free rule path drift) -> fatal `divergent`
LOCALITY_SECTION_MODE=0
for arg in "$@"; do
    if [[ "$arg" == "--locality-section" ]]; then
        LOCALITY_SECTION_MODE=1
    fi
done

if [[ "$LOCALITY_SECTION_MODE" -eq 1 ]]; then
    # Delegate to the dedicated locality-conformance runner. Sibling-extension
    # entry point; the runner consumes the checker crate's probe consts +
    # the shipped-conformance-manifest + JSON Schema.
    exec bash scripts/run-locality-conformance.sh
fi

readonly VALID_FAIL_CLASSIFICATIONS=(
    "realization_rule_proof_gap_unprovable"
    "realization_rule_reformulation_required"
    "realization_rule_evolution_surfaced"
)

# §08-realization corpus: RL-1..RL-34 active + sub-rules + RL-13 removal +
# composition. RL-13 carries the terminal `n/a — removed` proof_status but its
# removal-confirmation proof still discharges valid.
readonly PROOF_FILES=(
    "RL-1" "RL-2" "RL-3" "RL-4" "RL-5"
    "RL-6" "RL-7" "RL-8" "RL-9" "RL-10"
    "RL-11" "RL-11a" "RL-12" "RL-13"
    "RL-14" "RL-14a" "RL-15" "RL-15a" "RL-16"
    "RL-17" "RL-18" "RL-18a"
    "RL-19" "RL-20" "RL-21"
    "RL-22" "RL-23" "RL-24" "RL-25" "RL-26"
    "RL-27" "RL-28"
    "RL-29" "RL-30" "RL-31"
    "RL-32" "RL-33" "RL-34"
    "RL-1-RL-2-composition" "RL-comp"
)

# Compiler-conformance cross-walk: each proven RL-N -> shipped emission site
# (relative to ../). `SHIPPED` rules check the site exists;
# `TARGET` rules are target-system per Annex E §AIMS frontmatter (no shipped
# site yet -> conformance: target-only, NOT divergent). Format:
# "RL-N|class|path" where class ∈ {SHIPPED, TARGET}.
readonly CONFORMANCE_MAP=(
    "RL-1|SHIPPED|compiler/ori_arc/src/aims/emit_rc/forward_walk.rs"
    "RL-2|SHIPPED|compiler/ori_arc/src/aims/realize/walk_dec.rs"
    "RL-3|SHIPPED|compiler/ori_arc/src/aims/realize/decide.rs"
    "RL-4|SHIPPED|compiler/ori_arc/src/aims/emit_rc/edge_cleanup.rs"
    "RL-5|SHIPPED|compiler/ori_arc/src/aims/emit_rc/dead_cleanup/mod.rs"
    "RL-6|SHIPPED|compiler/ori_arc/src/aims/emit_rc/cow.rs"
    "RL-7|SHIPPED|compiler/ori_arc/src/aims/emit_rc/cow.rs"
    "RL-8|SHIPPED|compiler/ori_arc/src/aims/emit_rc/cow.rs"
    "RL-9|SHIPPED|compiler/ori_arc/src/aims/realize/decide.rs"
    "RL-10|SHIPPED|compiler/ori_arc/src/aims/emit_rc/cow.rs"
    "RL-11|SHIPPED|compiler/ori_arc/src/aims/emit_reuse/detect.rs"
    "RL-11a|SHIPPED|compiler/ori_arc/src/aims/emit_reuse/dynamic.rs"
    "RL-12|SHIPPED|compiler/ori_arc/src/aims/emit_reuse/planner.rs"
    "RL-13|TARGET|removed"
    "RL-14|SHIPPED|compiler/ori_arc/src/aims/realize/project_escape.rs"
    "RL-14a|TARGET|repr-stub"
    "RL-15|TARGET|repr-stub"
    "RL-15a|SHIPPED|compiler/ori_arc/src/aims/realize/project_escape.rs"
    "RL-16|SHIPPED|compiler/ori_arc/src/aims/realize/decide.rs"
    "RL-17|TARGET|repr-stub"
    "RL-18|TARGET|repr-stub"
    "RL-18a|SHIPPED|compiler/ori_arc/src/aims/realize/decide.rs"
    "RL-19|TARGET|repr-stub"
    "RL-20|TARGET|repr-stub"
    "RL-21|TARGET|repr-stub"
    "RL-22|TARGET|post-pipeline"
    "RL-23|TARGET|post-pipeline"
    "RL-24|TARGET|post-pipeline"
    "RL-25|TARGET|post-pipeline"
    "RL-26|TARGET|post-pipeline"
    "RL-27|SHIPPED|compiler/ori_arc/src/aims/emit_rc/arg_ownership/mod.rs"
    "RL-28|SHIPPED|compiler/ori_arc/src/aims/emit_rc/arg_ownership/mod.rs"
    "RL-29|TARGET|llvm-fact-export"
    "RL-30|TARGET|llvm-fact-export"
    "RL-31|TARGET|llvm-fact-export"
    "RL-32|SHIPPED|compiler/ori_arc/src/borrow/mod.rs"
    "RL-33|SHIPPED|compiler/ori_arc/src/borrow/mod.rs"
    "RL-34|SHIPPED|compiler/ori_arc/src/borrow/mod.rs"
)

fail_classification="${REALIZATION_RULE_FAILURE_CLASSIFICATION:-realization_rule_proof_gap_unprovable}"
classification_valid=0
for valid in "${VALID_FAIL_CLASSIFICATIONS[@]}"; do
    if [[ "$fail_classification" == "$valid" ]]; then
        classification_valid=1
        break
    fi
done
if [[ "$classification_valid" -ne 1 ]]; then
    cat > test-results/section-08-result.json <<EOF
{"status": "fail", "exit_reason": "realization_rule_infrastructure_failed", "phase": "preflight", "reason": "REALIZATION_RULE_FAILURE_CLASSIFICATION='${fail_classification}' not in {${VALID_FAIL_CLASSIFICATIONS[*]}}"}
EOF
    exit 3
fi

# Phase (a) — build.
if ! cargo build --release -p aims-proof-checker > test-results/section-08-build.log 2>&1; then
    cat > test-results/section-08-result.json <<EOF
{"status": "fail", "exit_reason": "realization_rule_infrastructure_failed", "phase": "build", "reason": "cargo build --release -p aims-proof-checker failed; see test-results/section-08-build.log"}
EOF
    exit 3
fi

readonly CHECKER="./target/release/aims-proof-checker"

# Phase (b) — per-proof discharge.
proofs_passed=0
proofs_failed=()

for proof in "${PROOF_FILES[@]}"; do
    proof_path="proofs/08-realization/${proof}.proof"
    out_path="test-results/section-08-${proof}.json"

    if [[ ! -f "$proof_path" ]]; then
        cat > test-results/section-08-result.json <<EOF
{"status": "fail", "exit_reason": "realization_rule_infrastructure_failed", "phase": "discharge", "failing_proof": "${proof}", "reason": "proof file missing: ${proof_path}"}
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
    cat > test-results/section-08-result.json <<EOF
{"status": "fail", "exit_reason": "${fail_classification}", "phase": "discharge", "proofs_passed": ${proofs_passed}, "proofs_failed": "${failing_list}"}
EOF
    exit 2
fi

# Phase (d) — compiler-conformance cross-walk.
conformance_divergent=()
conformance_pass=0
conformance_target_only=0
for entry in "${CONFORMANCE_MAP[@]}"; do
    IFS='|' read -r rl_n cls path <<< "$entry"
    if [[ "$cls" == "TARGET" ]]; then
        conformance_target_only=$((conformance_target_only + 1))
        continue
    fi
    # SHIPPED: the proven rule must have a present emission site.
    if [[ -f "../${path}" ]]; then
        conformance_pass=$((conformance_pass + 1))
    else
        conformance_divergent+=("${rl_n}:${path}")
    fi
done

if [[ ${#conformance_divergent[@]} -gt 0 ]]; then
    divergent_list=$(IFS=,; echo "${conformance_divergent[*]}")
    cat > test-results/section-08-result.json <<EOF
{"status": "fail", "exit_reason": "realization_rule_compiler_spec_divergence", "phase": "conformance_cross_walk", "proofs_passed": ${proofs_passed}, "conformance_divergent": "${divergent_list}"}
EOF
    exit 2
fi

cat > test-results/section-08-result.json <<EOF
{"status": "pass", "exit_reason": "realization_rules_proven", "proofs_passed": ${proofs_passed}, "conformance_pass": ${conformance_pass}, "conformance_target_only": ${conformance_target_only}}
EOF
exit 0
