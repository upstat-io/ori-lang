#!/usr/bin/env bash
# Detect and optionally clean untracked temp/scratch files from the worktree.
#
# Designed to run as a subsection close-out step in /continue-roadmap and
# /tpr-review workflows. Catches debug dumps, one-off test scripts, editor
# backups, and other detritus that accumulates during development.
#
# Usage:
#   diagnostics/repo-hygiene.sh              # List detected temp files (default)
#   diagnostics/repo-hygiene.sh --check      # Exit 1 if temp files found (CI gate)
#   diagnostics/repo-hygiene.sh --clean      # Remove detected temp files
#   diagnostics/repo-hygiene.sh --gitignore  # Show patterns to add to .gitignore
#   diagnostics/repo-hygiene.sh --help       # Show this help

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── Defaults ──────────────────────────────────────────────────────────────

MODE="list"       # list | check | clean | gitignore
NO_COLOR="${NO_COLOR:-}"
COLOR=true

# ── Parse args ────────────────────────────────────────────────────────────

usage() {
    cat <<'EOF'
Usage: diagnostics/repo-hygiene.sh [OPTIONS]

Detect and optionally clean untracked temp/scratch files from the worktree.

Modes:
  (default)     List detected temp files with categories
  --check       Exit 0 if clean, exit 1 if temp files found (CI/skill gate)
  --clean       Remove detected temp files (prints what it removes)
  --gitignore   Suggest .gitignore patterns for detected files

Options:
  --no-color    Disable colored output
  --color       Force colored output
  --help        Show this help

Categories detected:
  DUMP      Debug/IR dump files (*.txt, *.ll, *.bc at repo root)
  SCRATCH   One-off test/scratch scripts (test_*.py, scratch.*, temp.* at root)
  BACKUP    Editor backup files (*.orig, *.bak, *.rej, *~ anywhere)
  ARTIFACT  Build artifacts outside target/ (*.o, *.a, *.so, *.dylib at root)
  STALE     Stale temp files that .gitignore should catch but didn't
  TPR_STALE Stale TPR run dirs in /tmp/ori-tpr-* older than 72 hours

Exit codes:
  0   Clean (no temp files) or successful --clean
  1   Temp files detected (--check mode)
  2   Usage error
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --check)    MODE="check"; shift ;;
        --clean)    MODE="clean"; shift ;;
        --gitignore) MODE="gitignore"; shift ;;
        --no-color) COLOR=false; shift ;;
        --color)    COLOR=true; NO_COLOR=""; shift ;;
        --help|-h)  usage; exit 0 ;;
        *)
            echo "Error: unknown option '$1'" >&2
            echo "Run with --help for usage." >&2
            exit 2
            ;;
    esac
done

if [[ -n "$NO_COLOR" ]]; then
    COLOR=false
fi

# ── Color helpers ─────────────────────────────────────────────────────────

if $COLOR; then
    RED='\033[0;31m'
    YELLOW='\033[0;33m'
    GREEN='\033[0;32m'
    CYAN='\033[0;36m'
    DIM='\033[0;90m'
    BOLD='\033[1m'
    RESET='\033[0m'
else
    RED='' YELLOW='' GREEN='' CYAN='' DIM='' BOLD='' RESET=''
fi

# ── Detection ─────────────────────────────────────────────────────────────

cd "$ROOT_DIR"

# Collect untracked files (excluding directories git reports with trailing /)
mapfile -t UNTRACKED < <(git ls-files --others --exclude-standard 2>/dev/null || true)

# Also check for ignored files that might be hanging around in unexpected places
# (not needed — .gitignore already handles those)

declare -a DUMP_FILES=()
declare -a SCRATCH_FILES=()
declare -a BACKUP_FILES=()
declare -a ARTIFACT_FILES=()
declare -a STALE_FILES=()

for f in "${UNTRACKED[@]}"; do
    [[ -z "$f" ]] && continue

    # Get just the filename and the top-level directory
    basename_f="$(basename "$f")"
    dirname_f="$(dirname "$f")"

    # ── DUMP: debug/IR dump files at repo root ──
    if [[ "$dirname_f" == "." ]]; then
        case "$basename_f" in
            *.txt|*.ll|*.bc|*.dump|*.ir)
                DUMP_FILES+=("$f")
                continue
                ;;
        esac
    fi
    # DUMP: llvm_dump*, ir_dump*, *_dump.* anywhere
    case "$basename_f" in
        llvm_dump*|ir_dump*|*_dump.txt|*_dump.ll|*_dump.ir|*.llvm-ir)
            DUMP_FILES+=("$f")
            continue
            ;;
    esac

    # ── SCRATCH: one-off scripts at repo root ──
    if [[ "$dirname_f" == "." ]]; then
        case "$basename_f" in
            test_*.py|test_*.sh|test_*.rs|test_*.ori)
                SCRATCH_FILES+=("$f")
                continue
                ;;
            scratch.*|scratch_*|temp.*|temp_*|tmp.*|tmp_*|hack.*|hack_*|wip.*|wip_*)
                SCRATCH_FILES+=("$f")
                continue
                ;;
            *.py)
                # Any loose Python script at repo root is suspicious
                # (legitimate Python lives in scripts/ or diagnostics/)
                SCRATCH_FILES+=("$f")
                continue
                ;;
        esac
    fi

    # ── BACKUP: editor/merge artifacts anywhere ──
    case "$basename_f" in
        *.orig|*.bak|*.rej|*.conflict|*.BACKUP.*|*.BASE.*|*.LOCAL.*|*.REMOTE.*)
            BACKUP_FILES+=("$f")
            continue
            ;;
    esac

    # ── ARTIFACT: build artifacts outside target/ ──
    if [[ "$dirname_f" == "." ]]; then
        case "$basename_f" in
            *.o|*.a|*.so|*.dylib|*.dll|*.exe|*.lib)
                ARTIFACT_FILES+=("$f")
                continue
                ;;
        esac
    fi

    # ── STALE: patterns that SHOULD be in .gitignore but slipped through ──
    case "$basename_f" in
        core|core.[0-9]*|hs_err_pid*|*.core)
            STALE_FILES+=("$f")
            continue
            ;;
    esac
done

# ── TPR_STALE: /tmp/ori-tpr-* directories older than 72 hours ──
declare -a TPR_STALE_DIRS=()
TPR_STALE_BYTES=0
if compgen -G "/tmp/ori-tpr-*/" > /dev/null 2>&1; then
    NOW=$(date +%s)
    TPR_MAX_AGE=259200  # 72 hours in seconds
    for tpr_dir in /tmp/ori-tpr-*/; do
        [[ -d "$tpr_dir" ]] || continue
        dir_mtime=$(stat -c '%Y' "$tpr_dir" 2>/dev/null || echo "$NOW")
        age=$(( NOW - dir_mtime ))
        if [[ $age -ge $TPR_MAX_AGE ]]; then
            TPR_STALE_DIRS+=("$tpr_dir")
            dir_bytes=$(du -sb "$tpr_dir" 2>/dev/null | awk '{print $1}')
            TPR_STALE_BYTES=$(( TPR_STALE_BYTES + dir_bytes ))
        fi
    done
fi

TOTAL=$(( ${#DUMP_FILES[@]} + ${#SCRATCH_FILES[@]} + ${#BACKUP_FILES[@]} + ${#ARTIFACT_FILES[@]} + ${#STALE_FILES[@]} + ${#TPR_STALE_DIRS[@]} ))

# ── Output ────────────────────────────────────────────────────────────────

print_category() {
    local label="$1"
    local color="$2"
    shift 2
    local files=("$@")

    if [[ ${#files[@]} -eq 0 ]]; then
        return
    fi

    printf "\n${color}${BOLD}%s${RESET} ${DIM}(%d file%s)${RESET}\n" \
        "$label" "${#files[@]}" "$( [[ ${#files[@]} -ne 1 ]] && echo 's' || echo '' )"
    for f in "${files[@]}"; do
        printf "  %s\n" "$f"
    done
}

case "$MODE" in
    list)
        if [[ $TOTAL -eq 0 ]]; then
            printf "${GREEN}Worktree clean${RESET} — no temp/scratch files detected.\n"
            exit 0
        fi

        printf "${BOLD}Detected %d temp/scratch file%s:${RESET}\n" \
            "$TOTAL" "$( [[ $TOTAL -ne 1 ]] && echo 's' || echo '' )"

        print_category "DUMP — debug/IR dump files"     "$YELLOW" "${DUMP_FILES[@]}"
        print_category "SCRATCH — one-off test scripts"  "$RED"    "${SCRATCH_FILES[@]}"
        print_category "BACKUP — editor backup files"    "$CYAN"   "${BACKUP_FILES[@]}"
        print_category "ARTIFACT — build artifacts"      "$RED"    "${ARTIFACT_FILES[@]}"
        print_category "STALE — should be in .gitignore" "$YELLOW" "${STALE_FILES[@]}"

        if [[ ${#TPR_STALE_DIRS[@]} -gt 0 ]]; then
            local mb=$(( TPR_STALE_BYTES / 1048576 ))
            printf "\n${YELLOW}${BOLD}TPR_STALE — /tmp/ori-tpr-* dirs >72h old${RESET} ${DIM}(%d dir%s, ~%d MB)${RESET}\n" \
                "${#TPR_STALE_DIRS[@]}" "$( [[ ${#TPR_STALE_DIRS[@]} -ne 1 ]] && echo 's' || echo '' )" "$mb"
            for d in "${TPR_STALE_DIRS[@]}"; do
                local age_h=$(( (NOW - $(stat -c '%Y' "$d" 2>/dev/null || echo "$NOW")) / 3600 ))
                printf "  %s ${DIM}(%dh old)${RESET}\n" "$d" "$age_h"
            done
        fi

        printf "\n${DIM}Run with --clean to remove, or --check for CI gating.${RESET}\n"
        ;;

    check)
        if [[ $TOTAL -eq 0 ]]; then
            printf "${GREEN}repo-hygiene: clean${RESET}\n"
            exit 0
        fi

        printf "${RED}repo-hygiene: %d temp/scratch file%s detected${RESET}\n" \
            "$TOTAL" "$( [[ $TOTAL -ne 1 ]] && echo 's' || echo '' )"

        # Brief listing for CI output
        for f in "${DUMP_FILES[@]}" "${SCRATCH_FILES[@]}" "${BACKUP_FILES[@]}" "${ARTIFACT_FILES[@]}" "${STALE_FILES[@]}"; do
            printf "  %s\n" "$f"
        done
        for d in "${TPR_STALE_DIRS[@]}"; do
            printf "  %s (stale TPR run)\n" "$d"
        done

        printf "\nRun ${BOLD}diagnostics/repo-hygiene.sh --clean${RESET} to remove.\n"
        exit 1
        ;;

    clean)
        if [[ $TOTAL -eq 0 ]]; then
            printf "${GREEN}Worktree clean${RESET} — nothing to remove.\n"
            exit 0
        fi

        printf "${BOLD}Removing %d temp/scratch file%s:${RESET}\n" \
            "$TOTAL" "$( [[ $TOTAL -ne 1 ]] && echo 's' || echo '' )"

        ALL_FILES=("${DUMP_FILES[@]}" "${SCRATCH_FILES[@]}" "${BACKUP_FILES[@]}" "${ARTIFACT_FILES[@]}" "${STALE_FILES[@]}")
        for f in "${ALL_FILES[@]}"; do
            printf "  ${RED}rm${RESET} %s\n" "$f"
            rm -f "$ROOT_DIR/$f"
        done

        # Clean stale TPR run directories (these are in /tmp, not the worktree)
        for d in "${TPR_STALE_DIRS[@]}"; do
            age_h=$(( (NOW - $(stat -c '%Y' "$d" 2>/dev/null || echo "$NOW")) / 3600 ))
            printf "  ${RED}rm -rf${RESET} %s ${DIM}(%dh old)${RESET}\n" "$d" "$age_h"
            rm -rf "$d"
        done

        printf "\n${GREEN}Done.${RESET} Removed %d item%s.\n" \
            "$TOTAL" "$( [[ $TOTAL -ne 1 ]] && echo 's' || echo '' )"
        ;;

    gitignore)
        if [[ $TOTAL -eq 0 ]]; then
            printf "${GREEN}No new .gitignore patterns needed.${RESET}\n"
            exit 0
        fi

        printf "# Suggested additions to .gitignore:\n"
        if [[ ${#DUMP_FILES[@]} -gt 0 ]]; then
            printf "\n# Debug/IR dumps\n"
            printf "*.ll\n*.bc\n*.dump\n*.ir\n*.llvm-ir\n"
        fi
        if [[ ${#SCRATCH_FILES[@]} -gt 0 ]]; then
            printf "\n# One-off scratch files at repo root\n"
            printf "/test_*.py\n/test_*.sh\n/scratch.*\n/scratch_*\n/temp.*\n/temp_*\n/tmp.*\n/tmp_*\n/hack.*\n/wip.*\n"
        fi
        if [[ ${#ARTIFACT_FILES[@]} -gt 0 ]]; then
            printf "\n# Stray build artifacts\n"
            printf "/*.o\n/*.a\n/*.so\n/*.dylib\n"
        fi
        if [[ ${#STALE_FILES[@]} -gt 0 ]]; then
            printf "\n# Core dumps\n"
            printf "core\ncore.*\n"
        fi
        ;;
esac
