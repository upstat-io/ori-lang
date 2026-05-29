#!/usr/bin/env bash
#
# check-section-05-compiler-conformance.sh — §05-IM-K compiler-conformance
# cross-walk gate.
#
# Per the decision-predicate proofs
# §05-IM-K: "Mechanically cross-walks every proven DP-N rule against
# the compiler implementation sites
# compiler/ori_arc/src/aims/realize/emit_unified.rs (RC + COW
# emission — DP-1 / DP-7 / DP-4 / DP-5 / DP-9 consumers)
# + decide.rs (COW decision policy — DP-4 / DP-5 / DP-9 predicate consumers)
# + burden_elim.rs (DP-2 / DP-3 consumers per Annex E §AIMS RL-2/RL-3)."
#
# Mechanical cross-walk: for each DP-N rule (9 active + DP-10-REMOVED),
# verify the compiler's realize/ site has the corresponding implementation
# function or symbol that emits/consumes the canonical decision the proof
# asserts.
#
# Exit codes + emitted lines:
# exit 0 + emits `decision_predicates_proven` — ALL 10 entries confirm
# exit 2 + emits `decision_predicate_compiler_spec_divergence` — >=1 divergence
# exit 3 + emits `decision_predicate_infrastructure_failed` — file unreadable
#
# Invocation: bash check-section-05-compiler-conformance.sh
#
# Cwd contract: anchors to aims-proof/ via cd "$(dirname "$0")/.." at
# entry.

set -e
cd "$(dirname "$0")/.."

readonly EMIT_UNIFIED="../compiler/ori_arc/src/aims/realize/emit_unified.rs"
readonly DECIDE="../compiler/ori_arc/src/aims/realize/decide.rs"
readonly BURDEN_ELIM="../compiler/ori_arc/src/aims/realize/burden_elim.rs"
readonly PROOFS_DIR="proofs/05-decisions"

for src in "$EMIT_UNIFIED" "$DECIDE" "$BURDEN_ELIM"; do
    if [[ ! -f "$src" ]]; then
        echo "decision_predicate_infrastructure_failed: compiler realize/ source not found at ${src}"
        exit 3
    fi
done

if [[ ! -d "$PROOFS_DIR" ]]; then
    echo "decision_predicate_infrastructure_failed: proofs directory not found at ${PROOFS_DIR}"
    exit 3
fi

# Conformance entries: each row = "DP-id|target-file|required-token|description"
#
# Target-file is one of {emit_unified, decide, burden_elim} per the §05-IM-K
# consumer-site mapping. Required-token is a literal string that MUST appear
# in the named file (function name, symbol, or canonical-constant reference).
# Absence indicates the compiler has dropped or renamed the rule's
# implementation site.
readonly -a CONFORMANCE_ROWS=(
    "DP-1|emit_unified|emit_rc_unified|DP-1 is_rc_needed: RC emission from state map (Owned ∧ ≠Dead ∧ ¬scalar)"
    "DP-2|burden_elim|is_rc_dec_unnecessary|DP-2 is_rc_dec_unnecessary: burden elimination of supplementary decs (Absent ∨ Dead)"
    "DP-3|burden_elim|is_rc_inc_elidable|DP-3 is_rc_inc_elidable: burden elimination of incs (Once ∧ Linear)"
    "DP-4|decide|Uniqueness::MaybeShared|DP-4 needs_cow_check: COW decision branches on MaybeShared (decide_cow)"
    "DP-5|decide|decide_cow|DP-5 can_mutate_in_place: alias-safety gates StaticUnique branch in decide_cow"
    "DP-6|decide|decide_reuse|DP-6 is_reuse_candidate: reuse decision policy (Owned ∧ ≠Shared ∧ ≠NonReusable)"
    "DP-7|emit_unified|RcInc|DP-7 is_rc_skip_eligible: RC emission elides Owned+Linear+Once+Unique+local pairs"
    "DP-8|decide|locality|DP-8 is_local: decide.rs consults locality for local/escape branching"
    "DP-9|decide|fn decide_cow|DP-9 cow_mode: 5-row classification in decide_cow returning CowMode"
    "DP-10-REMOVED|decide|Uniqueness|DP-10-REMOVED: Uniqueness dimension is SOLE source of RC=1 truth (former DP-10 removed)"
)

divergences=()
checked=0

for row in "${CONFORMANCE_ROWS[@]}"; do
    dp_id="${row%%|*}"
    rest="${row#*|}"
    target_file="${rest%%|*}"
    rest="${rest#*|}"
    required_token="${rest%%|*}"
    description="${rest#*|}"

    proof_path="${PROOFS_DIR}/${dp_id}.proof"
    if [[ ! -f "$proof_path" ]]; then
        divergences+=("${dp_id}: proof_status: divergent (proof file missing: ${proof_path})")
        checked=$((checked + 1))
        continue
    fi

    case "$target_file" in
        emit_unified) target_path="$EMIT_UNIFIED" ;;
        decide) target_path="$DECIDE" ;;
        burden_elim) target_path="$BURDEN_ELIM" ;;
        *)
            echo "decision_predicate_infrastructure_failed: unknown target_file '${target_file}' in conformance row '${dp_id}'"
            exit 3
            ;;
    esac

    if ! grep -qF "$required_token" "$target_path"; then
        divergences+=("${dp_id}: proof_status: divergent (required token '${required_token}' not in ${target_file}.rs — ${description})")
    fi
    checked=$((checked + 1))
done

if [[ ${#divergences[@]} -gt 0 ]]; then
    echo "decision_predicate_compiler_spec_divergence: ${#divergences[@]} divergence(s) across ${checked} DP rules"
    for div in "${divergences[@]}"; do
        echo " ${div}"
    done
    exit 2
fi

echo "decision_predicates_proven: ${checked}/10 DP rules confirmed in emit_unified.rs + decide.rs + burden_elim.rs"
exit 0
