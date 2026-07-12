#!/usr/bin/env bash
# AOT identity-gate library: snapshot-staging manifest helpers + the post-leg
# trust verdict. Sourced by test-all.sh (the gate) and by the gate's shell test
# so the verdict is unit-testable with controlled inputs. Pure functions: no
# side effects, no global-state mutation.

# Identity fingerprint (dev:inode:mtime:size) of a single path. GNU stat first,
# BSD/macOS fallback, "absent" when the path is missing.
# KEEP IN SYNC with artifact_identity_of() in
# compiler/ori_llvm/tests/aot/util/binary.rs.
aot_gate_command() {
    if [ -n "${AOT_GATE_COMMAND_TIMEOUT_SECS:-}" ]; then
        timeout --signal=TERM --kill-after=2 "${AOT_GATE_COMMAND_TIMEOUT_SECS}s" "$@"
    else
        "$@"
    fi
}

aot_artifact_identity_of_path() {
    aot_gate_command stat -c '%d:%i:%Y:%s' "$1" 2>/dev/null \
        || aot_gate_command stat -f '%d:%i:%m:%z' "$1" 2>/dev/null \
        || echo "absent"
}

# The per-PID hardlink stage dir recorded in the manifest header (empty when
# the manifest is absent / has no stage-dir line). Captures the entire path so
# a stage dir containing spaces survives.
aot_manifest_stage_dir() {
    [ -f "$1" ] || return 0
    aot_gate_command awk '$1 == "stage-dir" { sub(/^stage-dir[ \t]+/, ""); print; exit }' "$1"
}

# Post-leg trust verdict for the AOT leg (SNAPSHOT-INTEGRITY). Echoes exactly
# one of:
#   valid               -- every staged artifact still matches its manifest-
#                          recorded identity; the snapshot stayed intact
#   invalid:<reason>    -- a staged artifact is missing or drifted mid-leg
#   fallback            -- no usable stage manifest; caller does the legacy
#                          live compare
# Re-stats each staged file at <stage-dir>/<name> and compares to its manifest
# field-4 identity. Mid-run target/ churn (test-all's own concurrent rebuilds)
# is harmless by design and never invalidates: the leg ran the inode-pinned
# snapshot, not target/.
# Args: <manifest_path> [baseline]  (baseline is unused on the integrity path;
# accepted for signature parity with the fallback caller.)
aot_snapshot_verdict() {
    local manifest="$1" stage_dir name recorded actual seen=0
    [ -f "$manifest" ] || { echo "fallback"; return 0; }
    stage_dir=$(aot_manifest_stage_dir "$manifest")
    [ -n "$stage_dir" ] || { echo "fallback"; return 0; }
    while read -r name recorded; do
        seen=1
        actual=$(aot_artifact_identity_of_path "$stage_dir/$name")
        if [ "$actual" = "absent" ]; then
            echo "invalid:staged-file-missing:$name"
            return 0
        fi
        if [ "$actual" != "$recorded" ]; then
            echo "invalid:staged-file-drifted:$name"
            return 0
        fi
    done < <(aot_gate_command awk '$1 == "artifact" { print $2, $4 }' "$manifest")
    if [ "$seen" = "0" ]; then
        echo "fallback"
        return 0
    fi
    echo "valid"
}
