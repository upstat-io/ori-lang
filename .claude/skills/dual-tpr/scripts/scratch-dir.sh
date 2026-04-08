#!/usr/bin/env bash
# scratch-dir.sh — create a per-run scratch directory for a dual-TPR round
#
# Usage: RUN=$(.claude/skills/dual-tpr/scripts/scratch-dir.sh)
# Returns: absolute path to the new scratch directory on stdout
# Cleanup: rm -rf "$RUN" on success; leave for postmortem on failure
#
# The directory is created via mktemp -d under $TMPDIR (typically /tmp)
# with the template ori-tpr-XXXXXXXX. See Section 01's envelope-format.md
# for the canonical file naming inside the scratch directory.

set -euo pipefail
mktemp -d -t "ori-tpr-XXXXXXXX"
