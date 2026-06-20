#!/usr/bin/env bash
# Unit tests for the AOT snapshot-integrity gate verdict (scripts/aot_gate_lib.sh).
# The gate must verify the staged snapshot stayed intact (immune to mid-run
# target/ churn), NOT compare the staged identity to a pre-parallel baseline.
# Run: bash compiler_repo/scripts/tests/test_aot_snapshot_gate.sh
set -u
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../aot_gate_lib.sh
source "$HERE/../aot_gate_lib.sh"

pass=0
fail=0
check() { # <name> <expected> <actual>
    if [ "$2" = "$3" ]; then
        echo "ok   - $1"
        pass=$((pass + 1))
    else
        echo "FAIL - $1: expected [$2] got [$3]"
        fail=$((fail + 1))
    fi
}

make_manifest() { # <manifest_path> <stage_dir> <ori_id> <rt_id>
    cat >"$1" <<EOF
schema 1
pid 12345
profile debug
stage-dir $2
source-dir /nonexistent/target/debug
artifact ori hardlink $3
artifact libori_rt.a hardlink $4
EOF
}

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# --- Test 7 (bug-repro pin): intact snapshot + diverged baseline -> VALID ---
# The staged files match their manifest-recorded identity; only the live
# pre-parallel baseline diverged (test-all's own concurrent rebuild). The leg
# ran a consistent pinned snapshot, so it is VALID. FAILS on the staged-vs-
# baseline gate (returns invalid), PASSES on snapshot-integrity.
STAGE7="$WORK/stage7"
mkdir -p "$STAGE7"
echo "ori-binary-bytes" >"$STAGE7/ori"
echo "rt-archive-bytes" >"$STAGE7/libori_rt.a"
ORI7=$(aot_artifact_identity_of_path "$STAGE7/ori")
RT7=$(aot_artifact_identity_of_path "$STAGE7/libori_rt.a")
make_manifest "$WORK/m7" "$STAGE7" "$ORI7" "$RT7"
BASELINE_DIVERGED=$'9999:9999:9999:9999\n8888:8888:8888:8888'
check "intact_snapshot_with_diverged_baseline_stays_valid" \
    "valid" "$(aot_snapshot_verdict "$WORK/m7" "$BASELINE_DIVERGED")"

# --- Test 8 (negative pin): drifted staged file -> INVALID ---
# baseline == the manifest identity (so a staged-vs-baseline gate would call it
# valid), but the staged file's on-disk identity drifted from its manifest
# record -> snapshot-integrity says INVALID.
STAGE8="$WORK/stage8"
mkdir -p "$STAGE8"
echo "ori-binary-bytes" >"$STAGE8/ori"
echo "rt-archive-bytes" >"$STAGE8/libori_rt.a"
ORI8=$(aot_artifact_identity_of_path "$STAGE8/ori")
RT8=$(aot_artifact_identity_of_path "$STAGE8/libori_rt.a")
make_manifest "$WORK/m8" "$STAGE8" "$ORI8" "$RT8"
sleep 1
echo "tampered-after-staging" >"$STAGE8/ori"
BASELINE8=$(printf '%s\n%s' "$ORI8" "$RT8")
check "drifted_staged_file_is_invalid" \
    "invalid" "$(aot_snapshot_verdict "$WORK/m8" "$BASELINE8" | cut -d: -f1)"

# --- Test 9 (fallback): manifest absent -> fallback ---
check "missing_manifest_falls_back_to_live_compare" \
    "fallback" "$(aot_snapshot_verdict "$WORK/no-such-manifest" "whatever")"

echo ""
echo "passed: $pass  failed: $fail"
[ "$fail" -eq 0 ]
