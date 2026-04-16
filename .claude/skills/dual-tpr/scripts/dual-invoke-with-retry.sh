#!/usr/bin/env bash
# dual-invoke-with-retry.sh — DEPRECATED backward-compatibility shim.
#
# HISTORY
#   This script used to implement ROUND-LEVEL retry with selective re-launch
#   on attempt 2+. That design coupled each retry attempt's wall time to
#   the slowest reviewer in the round — a fast-failing gemini had to wait
#   15-20 min for codex before it could retry, so 5 retries burned 45+ min
#   before circuit-breaker accumulated enough failures to trip.
#
#   On 2026-04-16 the retry model moved to PER-REVIEWER supervisors
#   (`supervisor.sh`). Each reviewer now retries on its own clock, fires
#   the circuit-breaker the moment it gives up, and does not block the
#   partner. The outer dual-invoke.sh is now a thin orchestrator that
#   launches two supervisors as siblings and waits for both. All retry
#   logic moved into supervisor.sh + lib-retry.sh.
#
# THIS SHIM
#   Historical callers that still `exec` dual-invoke-with-retry.sh get
#   redirected to dual-invoke.sh with arguments passed through verbatim.
#   The CLI surface is compatible: same --run/--skill/--codex-prompt/
#   --gemini-prompt/--schema flags.
#
#   First-party callers (one-round.sh, /tp-help, /tpr-review) have been
#   migrated to dual-invoke.sh directly. New consumers MUST call
#   dual-invoke.sh or one-round.sh — do not add new references to this
#   shim.
#
#   This file is retained for one release cycle so any external callers
#   don't break during the transition. Remove in a follow-up commit once
#   grep confirms no surviving references outside this directory.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# --schema was required by the old retry wrapper; dual-invoke.sh requires it
# only when --mode envelope (the default). Pass all args through verbatim.
exec "$SCRIPT_DIR/dual-invoke.sh" "$@"
