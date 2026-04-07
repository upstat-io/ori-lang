#!/usr/bin/env bash
# Cargo Cache Doctor
#
# Detects cargo cache pollution in target/ — specifically root-owned
# files (usually .fingerprint/ entries) that cargo cannot update under
# the current user.
#
# Default mode: detect and report. The script REFUSES to run destructive
# cleanup unless given an explicit flag. The tool's primary value is
# visibility, not unsupervised sudo.
#
# Usage: ./scripts/cache-doctor.sh [OPTIONS]
#
# Options:
#   -h, --help          Show this help
#       --print-cleanup Print the sudo commands to fix detected pollution
#       --clean         Run cleanup (prompts for sudo password)
#       --no-color      Disable color output
#
# Exit codes:
#   0  No pollution detected OR --clean succeeded
#   1  Pollution detected (report-only mode) or cleanup left residue
#   2  Cleanup failed
#   3  Invalid arguments
#
# Background:
#   Accidentally running `sudo cargo build` (e.g., inside a container
#   without CAP_SETUID, or during CI debugging with `sudo ./test-all.sh`)
#   writes .fingerprint/ entries owned by root. Subsequent non-root
#   cargo invocations cannot update these fingerprints, producing
#   erratic behavior: silent build staleness, cryptic "failed to write"
#   errors mid-build, and unexpected full rebuilds.
#
#   Surfaced during section-07 /improve-tooling retrospective — see
#   plans/repr-opt/section-07-enum-repr.md §07.RZ "Tooling gaps surfaced
#   during iteration 2".

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
TARGET_DIR="$ROOT_DIR/target"

# Color output (auto-disabled on non-TTY or --no-color)
if [[ -t 1 ]]; then
    RED=$'\033[31m'
    YELLOW=$'\033[33m'
    GREEN=$'\033[32m'
    BOLD=$'\033[1m'
    DIM=$'\033[2m'
    RESET=$'\033[0m'
else
    RED=''; YELLOW=''; GREEN=''; BOLD=''; DIM=''; RESET=''
fi

usage() {
    cat <<EOF
Usage: ${0##*/} [OPTIONS]

Detect cargo cache pollution in target/ (root-owned files that cargo
cannot update under the current user).

OPTIONS:
  -h, --help          Show this help
      --print-cleanup Print the sudo commands to fix detected pollution
      --clean         Run cleanup (prompts for sudo password)
      --no-color      Disable color output

EXAMPLES:
  ${0##*/}                    # detect and report
  ${0##*/} --print-cleanup    # print copy-pastable sudo commands
  ${0##*/} --clean            # run cleanup (prompts for sudo password)

EXIT CODES:
  0  No pollution detected OR --clean succeeded
  1  Pollution detected (report-only mode) or cleanup left residue
  2  Cleanup failed
  3  Invalid arguments

BACKGROUND:
  Accidentally running 'sudo cargo build' writes .fingerprint/ entries
  owned by root. Subsequent non-root cargo invocations cannot update
  these fingerprints, producing erratic behavior:
    - Silent build staleness ("tests pass, but old code ran")
    - Cryptic "failed to write" errors mid-build
    - cargo falling back to full rebuilds even when no source changed

  Surfaced during section-07 /improve-tooling retrospective — see
  plans/repr-opt/section-07-enum-repr.md §07.RZ "Tooling gaps surfaced
  during iteration 2".
EOF
}

MODE="detect"
for arg in "$@"; do
    case "$arg" in
        -h|--help) usage; exit 0 ;;
        --print-cleanup) MODE="print-cleanup" ;;
        --clean) MODE="clean" ;;
        --no-color)
            RED=''; YELLOW=''; GREEN=''; BOLD=''; DIM=''; RESET='' ;;
        *)
            printf 'Unknown option: %s\n' "$arg" >&2
            usage >&2
            exit 3
            ;;
    esac
done

if [[ ! -d "$TARGET_DIR" ]]; then
    printf '%sNo target/ directory at %s — nothing to check.%s\n' "$DIM" "$TARGET_DIR" "$RESET"
    exit 0
fi

printf '%s=== cargo cache-doctor ===%s\n' "$BOLD" "$RESET"
printf 'Scanning %s for root-owned files...\n\n' "$TARGET_DIR"

# Collect root-owned paths into an array (newline-safe via mapfile)
mapfile -t ROOT_FILES < <(find "$TARGET_DIR" -uid 0 2>/dev/null || true)

if [[ "${#ROOT_FILES[@]}" -eq 0 ]]; then
    printf '%s[OK]%s No root-owned files in target/ — cache is clean.\n' "$GREEN" "$RESET"
    exit 0
fi

TOTAL="${#ROOT_FILES[@]}"
printf '%s[WARN]%s Detected %s%d%s root-owned paths in target/:\n\n' \
    "$YELLOW" "$RESET" "$BOLD" "$TOTAL" "$RESET"

# Per-profile counts
for profile in debug release; do
    count=0
    for path in "${ROOT_FILES[@]}"; do
        [[ "$path" == "$TARGET_DIR/$profile/"* ]] && count=$((count + 1))
    done
    if [[ "$count" -gt 0 ]]; then
        label=$([[ "$count" -eq 1 ]] && echo file || echo files)
        printf '  %s%-8s%s %d %s\n' "$BOLD" "$profile" "$RESET" "$count" "$label"
    fi
done
echo

# Show a sample of paths
printf '%sSample paths (first 10):%s\n' "$DIM" "$RESET"
for i in "${!ROOT_FILES[@]}"; do
    [[ "$i" -ge 10 ]] && break
    printf '  %s\n' "${ROOT_FILES[$i]}"
done
if [[ "$TOTAL" -gt 10 ]]; then
    printf '  %s... and %d more%s\n' "$DIM" "$((TOTAL - 10))" "$RESET"
fi
echo

printf '%sWhy this matters:%s\n' "$BOLD" "$RESET"
cat <<'EOF'
  Cargo cannot update root-owned .fingerprint/ entries under your user,
  so incremental builds become unpredictable. Symptoms include silent
  staleness, cryptic "failed to write" errors, and unexpected full
  rebuilds.
EOF
echo

# Partition paths into fingerprint directories and "other" (unknown).
# Only fingerprint dirs are considered safe for --clean.
FINGERPRINT_DIRS=()
OTHER_PATHS=()
for path in "${ROOT_FILES[@]}"; do
    if [[ "$path" =~ ^$TARGET_DIR/(debug|release)/\.fingerprint/[^/]+ ]]; then
        dir="${BASH_REMATCH[0]}"
        # Deduplicate by appending only if not already in the array.
        # O(n²) is fine — target/ fingerprints are at most a few hundred dirs.
        already_present=0
        for existing in "${FINGERPRINT_DIRS[@]:-}"; do
            [[ "$existing" == "$dir" ]] && already_present=1 && break
        done
        [[ "$already_present" -eq 0 ]] && FINGERPRINT_DIRS+=("$dir")
    else
        OTHER_PATHS+=("$path")
    fi
done

case "$MODE" in
    detect)
        printf '%sTo see the cleanup commands:%s  %s./scripts/%s --print-cleanup%s\n' \
            "$BOLD" "$RESET" "$DIM" "${0##*/}" "$RESET"
        printf '%sTo run cleanup now:%s          %s./scripts/%s --clean%s\n' \
            "$BOLD" "$RESET" "$DIM" "${0##*/}" "$RESET"
        exit 1
        ;;
    print-cleanup)
        printf '%sCleanup commands (copy-paste into your shell):%s\n\n' "$BOLD" "$RESET"
        if [[ "${#FINGERPRINT_DIRS[@]}" -gt 0 ]]; then
            printf '  # Remove %d root-owned fingerprint directories:\n' \
                "${#FINGERPRINT_DIRS[@]}"
            for dir in "${FINGERPRINT_DIRS[@]}"; do
                printf '  sudo rm -rf %q\n' "$dir"
            done
            echo
        fi
        if [[ "${#OTHER_PATHS[@]}" -gt 0 ]]; then
            printf '  # Other root-owned paths (review manually before removing):\n'
            for path in "${OTHER_PATHS[@]}"; do
                printf '  #   %s\n' "$path"
            done
            echo
        fi
        printf '  # After cleanup, run `cargo b` to regenerate fingerprints cleanly.\n'
        exit 1
        ;;
    clean)
        if [[ "${#FINGERPRINT_DIRS[@]}" -eq 0 ]]; then
            printf '%s[ERR]%s No .fingerprint/ directories detected — refusing to\n' \
                "$RED" "$RESET"
            printf '      touch unknown paths. Run --print-cleanup to review manually.\n'
            exit 2
        fi
        printf '%sRunning cleanup (will prompt for sudo password)...%s\n\n' \
            "$YELLOW" "$RESET"
        printf 'Removing %d fingerprint directories.\n' "${#FINGERPRINT_DIRS[@]}"
        if sudo rm -rf "${FINGERPRINT_DIRS[@]}"; then
            printf '%s[OK]%s Fingerprint directories removed.\n' "$GREEN" "$RESET"
            # Re-check
            post_count=$(find "$TARGET_DIR" -uid 0 2>/dev/null | wc -l)
            if [[ "$post_count" -gt 0 ]]; then
                printf '%s[WARN]%s %d root-owned paths remain (non-.fingerprint).\n' \
                    "$YELLOW" "$RESET" "$post_count"
                printf '       Run --print-cleanup for details.\n'
                exit 1
            fi
            printf 'Run `cargo b` to regenerate fingerprints cleanly.\n'
            exit 0
        else
            printf '%s[ERR]%s Cleanup failed (sudo rm exited non-zero).\n' \
                "$RED" "$RESET"
            exit 2
        fi
        ;;
esac
