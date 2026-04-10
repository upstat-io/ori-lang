#!/bin/bash
# ORI_BIN wrapper that forces a deterministic mismatch between interpreter
# and AOT for self-testing dual-exec-debug.sh's mismatch diagnostics path.
#
# Contract:
#   - Default: pass-through to real ori (all commands, flags, env modes)
#   - Override: `ori run <file>` prints "INTERP" to stdout (forced divergence)
#   - Override: `ori build <file> -o <out>` creates a binary that prints "AOT"
#
# This ensures dual-exec-debug.sh sees different stdout from interpreter vs AOT,
# triggering the auto-diagnostics block. All helper invocations (arc-dump.sh's
# ORI_DUMP_AFTER_ARC=1 mode, ir-dump.sh's --emit=llvm-ir, codegen-audit.sh's
# build, _common.sh's build /dev/null probe) pass through to the real binary.

set -uo pipefail

# Resolve the real ori binary — use REAL_ORI if set, otherwise find it
if [[ -n "${REAL_ORI:-}" ]]; then
    REAL="$REAL_ORI"
else
    # Find the real ori from standard locations (skip ourselves)
    SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
    SELF="$(realpath "$0")"
    for candidate in \
        "$SCRIPT_DIR/../../target/debug/ori" \
        "$HOME/.local/bin/ori" \
        "$(command -v ori 2>/dev/null || true)"; do
        if [[ -n "$candidate" && -x "$candidate" ]]; then
            resolved="$(realpath "$candidate" 2>/dev/null || echo "$candidate")"
            if [[ "$resolved" != "$SELF" ]]; then
                REAL="$resolved"
                break
            fi
        fi
    done
fi

if [[ -z "${REAL:-}" ]]; then
    echo "Error: mismatch-wrapper.sh could not find real ori binary" >&2
    exit 2
fi

# Check if this is an overridden command
case "${1:-}" in
    run)
        # Override: print "INTERP" instead of running the real interpreter
        echo "INTERP"
        exit 0
        ;;
    build)
        # Only override plain `build <file> -o <out>` — NOT env-driven modes
        # like ORI_DUMP_AFTER_ARC=1 or ORI_DUMP_AFTER_LLVM=1 or ORI_AUDIT_CODEGEN=1,
        # which are used by diagnostic helper scripts (arc-dump.sh, ir-dump.sh,
        # codegen-audit.sh) and need the real compiler to produce IR/audit data.
        if [[ -z "${ORI_DUMP_AFTER_ARC:-}" && -z "${ORI_DUMP_AFTER_LLVM:-}" && \
              -z "${ORI_AUDIT_CODEGEN:-}" && -z "${ORI_DEBUG_LLVM:-}" && \
              -z "${ORI_VERIFY_ARC:-}" ]]; then
            # No env-driven diagnostic mode — check for plain build override
            has_output=0
            output_path=""
            args=("$@")
            for ((i=0; i<${#args[@]}; i++)); do
                if [[ "${args[$i]}" == "-o" && $((i+1)) -lt ${#args[@]} ]]; then
                    has_output=1
                    output_path="${args[$((i+1))]}"
                fi
            done

            if [[ $has_output -eq 1 && -n "$output_path" ]]; then
                # Override: create a binary that prints "AOT" via a shell wrapper
                cat > "$output_path" <<'WRAPPER'
#!/bin/bash
echo "AOT"
WRAPPER
                chmod +x "$output_path"
                exit 0
            fi
        fi
        # Fall through to real ori for env-driven or non -o builds
        ;;
esac

# Default: pass through to real ori
exec "$REAL" "$@"
