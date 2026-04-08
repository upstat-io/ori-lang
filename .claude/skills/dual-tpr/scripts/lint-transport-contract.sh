#!/usr/bin/env bash
# lint-transport-contract.sh — static linter that detects dead arguments in
# transport scripts under `.claude/skills/dual-tpr/scripts/`.
#
# An argument is "dead" if its target variable is assigned by the arg parser
# but never referenced elsewhere in the script. Dead args are a silent API
# contract drift: callers still pass them (and may believe they do something),
# but the script treats them as inert. This was the exact class of bug that
# hid in `dual-invoke.sh`'s `--schema` arg for two full sections of the
# dual-tpr-gemini plan — BUG-08-003 removed the `--output-schema` passthrough
# to codex, leaving `$SCHEMA` declared, parsed, required-checked, and otherwise
# unused. The dead code was discovered during the §07 pre-implementation
# review, which surfaced the schema-optional fix AND this linter as companion
# tooling.
#
# Usage:
#   lint-transport-contract.sh           # report mode — print findings, always exit 0
#   lint-transport-contract.sh --check   # enforce mode — exit 1 if any UNANNOTATED dead arg
#   lint-transport-contract.sh --help    # show this block
#
# Coverage:
#   Recognizes the `case "$1" in --xxx) VAR="$2"; shift 2 ;; ...` arg-parse
#   pattern used by `dual-invoke.sh`. Scripts that use different arg-parse
#   patterns (e.g., `dual-invoke-with-retry.sh`'s for-loop) are NOT scanned —
#   the linter prints a "no case-pattern args found" line and moves on.
#   Extend this when new arg-parse patterns appear.
#
# Known-dead annotation:
#   An arg can be intentionally preserved (e.g., for caller-signature backward
#   compatibility) by adding a trailing comment on the SAME line as the
#   `--xxx)` case:
#       --schema)         SCHEMA="$2"; shift 2 ;;  # lint-transport-contract: known-dead BUG-08-003
#   The linter reports these as KNOWN-DEAD and does NOT fail --check mode on
#   them. Unannotated dead args always fail --check.
#
# Exit codes:
#   0 = all args live or known-dead (always, unless --check is set)
#   1 = --check mode and at least one unannotated dead arg
#   2 = usage error

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

MODE="report"
case "${1:-}" in
  "")              MODE="report" ;;
  --check)         MODE="check" ;;
  -h|--help)       sed -n '2,38p' "$0"; exit 0 ;;
  *)               echo "unknown arg: $1" >&2; exit 2 ;;
esac

declare -i FAILURES=0

lint_script() {
  local script="$1"
  echo "=== $(basename "$script") ==="

  # Detect whether this script uses the recognized case-pattern. If it has
  # zero `--xxx) VAR=` matches, the linter doesn't know how to parse it.
  if ! grep -qE '^[[:space:]]*--[a-z-]+\)[[:space:]]+[A-Z_]+=' "$script"; then
    echo "  (no case-pattern args found — scanner does not cover this script)"
    return 0
  fi

  local line name var known_dead refs
  while IFS= read -r line; do
    # Match `  --xxx) VAR="$2"; shift 2 ;;` with optional trailing comment
    if [[ "$line" =~ ^[[:space:]]*--([a-z-]+)\)[[:space:]]+([A-Z_]+)= ]]; then
      name="${BASH_REMATCH[1]}"
      var="${BASH_REMATCH[2]}"
      known_dead=0
      if [[ "$line" == *"lint-transport-contract: known-dead"* ]]; then
        known_dead=1
      fi

      # Count variable references in CODE (comments stripped). Matches `$VAR`
      # and `${VAR...}` forms. Uses BRE-compatible word boundary via character
      # class exclusion. The sed pre-filter strips `#`-to-end-of-line to
      # prevent prose mentions of `$VAR` inside comments from counting as
      # live references — e.g., a comment like "$SCHEMA is preserved for
      # backward compat" would otherwise hide a truly dead variable. The
      # sed filter is naive: it does not understand `#` inside string
      # literals. For the bash scripts in this directory that is safe
      # because none of them put `#` inside double-quoted strings in a
      # position that would create a false negative.
      refs=$(sed 's/#.*$//' "$script" \
             | grep -cE "\\\$${var}([^A-Za-z0-9_]|$)|\\\$\{${var}" \
             || true)

      if (( refs == 0 )); then
        if (( known_dead )); then
          printf "  KNOWN-DEAD: --%-10s var \$%-10s — annotated\n" "$name" "$var"
        else
          printf "  DEAD:       --%-10s var \$%-10s — UNANNOTATED\n" "$name" "$var"
          echo "              Add trailing comment: '# lint-transport-contract: known-dead <reason>'"
          echo "              on the --${name}) line, or remove the dead arg entirely."
          FAILURES=$((FAILURES + 1))
        fi
      else
        printf "  LIVE:       --%-10s var \$%-10s — %d reference(s)\n" "$name" "$var" "$refs"
      fi
    fi
  done < "$script"
}

# Scan each transport script. New scripts should be added here as they land.
lint_script "$SCRIPT_DIR/dual-invoke.sh"
lint_script "$SCRIPT_DIR/dual-invoke-with-retry.sh"

echo ""
if (( FAILURES > 0 )); then
  echo "FAIL: $FAILURES unannotated dead argument(s) detected"
  if [[ "$MODE" == "check" ]]; then
    exit 1
  fi
  # report mode: surface the problem but do not fail
  exit 0
fi
echo "OK: all scanned transport script arguments are either live or annotated known-dead"
exit 0
