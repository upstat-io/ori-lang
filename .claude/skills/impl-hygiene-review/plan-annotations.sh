#!/usr/bin/env bash
# plan-annotations.sh — compat wrapper around plan-annotations.py
#
# The real implementation is in plan-annotations.py (Python 3.10+). This
# wrapper exists so existing `.sh` callers keep working. All flags pass
# through unchanged. See `plan-annotations.sh --help` (or
# `plan-annotations.py --help`) for the full flag list.
#
# Why Python: classifying annotations requires reading every plan's
# markdown content to extract `- [ ]` / `- [x]` checkbox items and
# building an ID → status map. That's trivial in Python and painful
# in bash. The legacy pure-bash implementation tried to fake it with
# filename-based section-number filtering and silently hid nearly every
# annotation as "active scaffolding."

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$SCRIPT_DIR/plan-annotations.py" "$@"
