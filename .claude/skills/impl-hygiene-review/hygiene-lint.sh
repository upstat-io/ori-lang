#!/usr/bin/env bash
# hygiene-lint.sh — wrapper around hygiene-lint.py
#
# Static analysis for Ori compiler hygiene rules. Runs all deterministic,
# pattern-based checks (file length, function length, test naming, banners,
# commented code, etc.) that don't require AI judgment.
#
# See `hygiene-lint.sh --help` for the full flag list.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$SCRIPT_DIR/hygiene-lint.py" "$@"
