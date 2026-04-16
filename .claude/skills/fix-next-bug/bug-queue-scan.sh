#!/usr/bin/env bash
# bug-queue-scan.sh — thin shim delegating to bug_queue_scan.py.
#
# The real scanner is the Python script (bug_queue_scan.py). This shim
# exists so callers that prefer a shell entry point stay stable.
set -e
exec python3 "$(dirname "$0")/bug_queue_scan.py" "$@"
