#!/usr/bin/env bash
# roadmap-scan.sh — thin shim delegating to roadmap_scan.py
#
# The real scanner is the Python rewrite at roadmap_scan.py (comprehensive
# cross-plan crawler, rich health signals, blocker DAG, TPR rollups, bug
# tracker integration). This shim exists so the continue-roadmap/SKILL.md
# and create-plan/SKILL.md invocations keep working without change.
#
# The create-plan SKILL.md extracts the REROUTES block via:
#   sed -n '/=== REROUTES ===/,/^$/p'
# which still works because roadmap_scan.py emits that block verbatim at
# the top of the default output.
set -e
exec python3 "$(dirname "$0")/roadmap_scan.py" "$@"
