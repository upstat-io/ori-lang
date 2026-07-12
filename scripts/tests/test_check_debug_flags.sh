#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

mkdir -p "$WORK/diagnostics" "$WORK/compiler/oric/src/debug_flags" "$WORK/compiler/ori_rt/src"
cp "$ROOT/diagnostics/check-debug-flags.sh" "$WORK/diagnostics/"

cat > "$WORK/compiler/oric/src/debug_flags/mod.rs" <<'EOF'
    ORI_USED
    ORI_TRACE_RC
    ORI_RT_DEBUG
    ORI_CHECK_LEAKS
EOF
cat > "$WORK/compiler/use.rs" <<'EOF'
const USED: &str = "ORI_USED";
fn sysroot() { let _ = std::env::var_os("ORI_SYSROOT"); }
EOF
cat > "$WORK/compiler/ori_rt/src/lib.rs" <<'EOF'
const TRACE: &str = "ORI_TRACE_RC";
const DEBUG: &str = "ORI_RT_DEBUG";
const LEAKS: &str = "ORI_CHECK_LEAKS";
EOF

positive_output="$($WORK/diagnostics/check-debug-flags.sh --no-color)"
case "$positive_output" in
    *"All checks passed."*) ;;
    *)
        printf 'FAIL: non-diagnostic transport variable was classified as a debug flag\n%s\n' "$positive_output"
        exit 1
        ;;
esac

cat >> "$WORK/compiler/use.rs" <<'EOF'
fn unknown() { let _ = std::env::var("ORI_UNREGISTERED_FLAG"); }
EOF
set +e
negative_output="$($WORK/diagnostics/check-debug-flags.sh --no-color 2>&1)"
negative_rc=$?
set -e
if [ "$negative_rc" -ne 1 ]; then
    printf 'FAIL: unregistered debug flag exited %s instead of 1\n%s\n' "$negative_rc" "$negative_output"
    exit 1
fi
case "$negative_output" in
    *"ORPHAN: ORI_UNREGISTERED_FLAG"*) ;;
    *)
        printf 'FAIL: unregistered debug flag was not diagnosed\n%s\n' "$negative_output"
        exit 1
        ;;
esac

echo "PASS: debug-flag classification accepts non-diagnostic variables and rejects unknown flags"
