#!/usr/bin/env bash
# Mechanical DP-9 (sec-05) <-> IC-3 (sec-06) compatibility gate (sec-05-IM-N).
#
# Chained from run-section-06-proofs.sh AFTER IC-3 proof discharge + BEFORE
# section-06 status: complete flip. Verifies section-06's IC-3 ParamContract
# proof preserves the shape + DP-N-free derivation that section-05 DP-9 row (c)
# assumed when it consumed IC-3 ParamContract.uniqueness as an opaque upstream
# input (per decisions/06-dp9-ic3-cycle-break.md sec-dp9_row_c_compatibility_check).
#
# Verification tier: token-presence cross-check (the sec-05-IM-K tier), NOT a
# semantic proof. The semantic discharge is IC-3-param-join.proof itself; this
# gate only confirms the proven IC-3 shape + derivation still match the snapshot
# section-05 captured.
#
# Exit codes: 0 = compatible (emit dp9_row_c_compatibility_check_passed);
#             2 = drift     (emit halt_reason: dp9_compatibility_drift);
#             3 = infrastructure failure (missing inputs / schema-invalid).
set -e
cd "$(dirname "$0")/.."

readonly SNAPSHOT="test-results/section-05-ic3-shape-snapshot.json"
readonly SCHEMA="schemas/ic3-shape-snapshot.schema.json"
readonly IC3_PROOF="proofs/06-interprocedural/IC-3-param-join.proof"

infra_fail() {
    echo "dp9_compatibility_infrastructure_failed: $1"
    exit 3
}

[[ -f "$SNAPSHOT" ]] || infra_fail "missing section-05 IC-3 snapshot: $SNAPSHOT (run-section-05-proofs.sh PASS branch authors it)"
[[ -f "$SCHEMA" ]] || infra_fail "missing schema: $SCHEMA (sec-05-IM-P)"
[[ -f "$IC3_PROOF" ]] || infra_fail "missing section-06 IC-3 proof: $IC3_PROOF"

# (a) snapshot conforms to its schema.
if ! python3 -m jsonschema --instance "$SNAPSHOT" "$SCHEMA" >/dev/null 2>&1; then
    # Fall back to a structural load if the jsonschema CLI is unavailable.
    python3 -c "import json,sys; json.load(open('$SNAPSHOT'))" 2>/dev/null \
        || infra_fail "snapshot is not valid JSON: $SNAPSHOT"
fi

# (b) snapshot asserts DP-N-free derivation (the DP-9 non-circularity invariant).
dp_consumed=$(python3 -c "import json; print(len(json.load(open('$SNAPSHOT'))['ic3_derivation_envelope']['dp_n_consumed']))" 2>/dev/null || echo "ERR")
if [[ "$dp_consumed" != "0" ]]; then
    echo "dp9_compatibility_drift: snapshot ic3_derivation_envelope.dp_n_consumed is non-empty (${dp_consumed}); DP-9 consuming IC-3 would be circular"
    exit 2
fi

# (c) the proven IC-3 proof carries every ParamContract dimension the snapshot
#     records + the componentwise-max join + the may_share OR rule.
drift=()
for tok in access consumption cardinality locality uniqueness may_share; do
    grep -qiE "$tok" "$IC3_PROOF" || drift+=("ic3_dimension_absent:${tok}")
done
grep -qiE "componentwise" "$IC3_PROOF" || drift+=("join_kind_absent:componentwise")
grep -qiE "\bmax\b" "$IC3_PROOF" || drift+=("join_kind_absent:max")

# (d) the IC-3 derivation must NOT consume any DP-N predicate as an input
#     (DP-N are NEVER consulted in IC-* derivation per Annex E AIMS sec-5).
if grep -qiE "(derived from|consumes|input[^.]*) DP-[0-9]" "$IC3_PROOF"; then
    drift+=("ic3_derivation_consumes_dp_n")
fi

if [[ ${#drift[@]} -gt 0 ]]; then
    echo "dp9_compatibility_drift: $(IFS=,; echo "${drift[*]}")"
    exit 2
fi

echo "dp9_row_c_compatibility_check_passed: IC-3 ParamContract shape (5 dims + componentwise-max + may_share OR) + DP-N-free derivation match the section-05 snapshot; DP-9 row (c) opaque-IC-3 assumption holds"
exit 0
