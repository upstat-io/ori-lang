#!/usr/bin/env bash
# tpr-failure-summary.sh — summarize failure patterns across recent TPR runs.
#
# Scans /tmp/ori-tpr-* run directories for gemini and codex failure patterns,
# producing a compact report showing success rates, error categories, repair
# stats, and API error messages.
#
# Usage:
#   diagnostics/tpr-failure-summary.sh [OPTIONS]
#
# Options:
#   --reviewer REVIEWER   Filter to codex|gemini|both (default: both)
#   --failures            Show only failed runs (non-zero exit or empty envelope)
#   --verbose             Show per-run details for failures
#   --json                Output as JSON (for downstream tooling)
#   --help                Show this help message
#
# Examples:
#   diagnostics/tpr-failure-summary.sh                # full summary
#   diagnostics/tpr-failure-summary.sh --failures     # only failures
#   diagnostics/tpr-failure-summary.sh --reviewer gemini --verbose  # gemini detail

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Color definitions
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'

REVIEWER_FILTER="both"
FAILURES_ONLY=false
VERBOSE=false
JSON_OUTPUT=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --reviewer) REVIEWER_FILTER="$2"; shift 2 ;;
    --failures) FAILURES_ONLY=true; shift ;;
    --verbose) VERBOSE=true; shift ;;
    --json) JSON_OUTPUT=true; shift ;;
    --help|-h)
      sed -n '2,/^$/{ s/^# //; s/^#$//; p }' "$0"
      exit 0
      ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

RUN_DIRS=(/tmp/ori-tpr-*/)
if [[ ${#RUN_DIRS[@]} -eq 0 || ! -d "${RUN_DIRS[0]}" ]]; then
  echo "No TPR run directories found in /tmp/ori-tpr-*" >&2
  exit 1
fi

# Collect stats
total=0; ok=0; exit1=0; exit143=0
capacity=0; repaired=0; rescued=0
declare -A api_errors

extract_api_error() {
  local jsonl="$1"
  [[ -f "$jsonl" ]] || return 0
  python3 -c "
import json, sys
with open('$jsonl') as f:
    for line in f:
        line = line.strip()
        if not line: continue
        try:
            obj = json.loads(line)
            if obj.get('type') == 'result' and obj.get('status') == 'error':
                err = obj.get('error', {})
                print(err.get('message', ''), end='')
                sys.exit(0)
        except: pass
" 2>/dev/null || true
}

# Process each reviewer
process_reviewer() {
  local reviewer="$1"
  local total_r=0 ok_r=0 failed_r=0 capacity_r=0 repaired_r=0 rescued_r=0 stalled_r=0
  local failure_details=""

  for dir in "${RUN_DIRS[@]}"; do
    [[ -f "$dir/${reviewer}.exit" ]] || continue
    total_r=$((total_r + 1))

    local ec
    ec=$(cat "$dir/${reviewer}.exit" 2>/dev/null)

    local has_envelope=false
    [[ -s "$dir/${reviewer}.envelope.json" ]] && has_envelope=true

    local parse_err=""
    [[ -f "$dir/${reviewer}.parse-error" ]] && parse_err=$(head -1 "$dir/${reviewer}.parse-error" 2>/dev/null)

    if [[ "$ec" == "0" ]] && $has_envelope; then
      ok_r=$((ok_r + 1))

      # Check for repairs/rescues
      if grep -q "^REPAIR:" "$dir/${reviewer}.parse-error" 2>/dev/null; then
        repaired_r=$((repaired_r + 1))
      fi
      if grep -q "^RESCUED:" "$dir/${reviewer}.parse-error" 2>/dev/null; then
        rescued_r=$((rescued_r + 1))
      fi

      if $FAILURES_ONLY; then continue; fi
    else
      failed_r=$((failed_r + 1))

      # Extract API error if present
      local api_err
      api_err=$(extract_api_error "$dir/${reviewer}.jsonl")

      local fail_reason="unknown"
      if [[ "$ec" == "143" ]]; then
        fail_reason="watchdog_killed"
        stalled_r=$((stalled_r + 1))
      elif [[ -n "$api_err" && "$api_err" == *"No capacity"* ]]; then
        fail_reason="api_capacity"
        capacity_r=$((capacity_r + 1))
      elif [[ -n "$api_err" ]]; then
        fail_reason="api_error: $api_err"
      elif [[ "$ec" == "0" ]] && ! $has_envelope; then
        fail_reason="parse_failed (exit=0, no envelope): $parse_err"
      else
        fail_reason="exit=$ec"
      fi

      if $VERBOSE; then
        local name
        name=$(basename "$dir")
        local wall=""
        [[ -f "$dir/${reviewer}.walltime" ]] && wall=$(cat "$dir/${reviewer}.walltime" 2>/dev/null)
        failure_details+="  ${name}: ${fail_reason} (wall=${wall:-?}s)\n"
      fi
    fi
  done

  if $JSON_OUTPUT; then
    printf '{"reviewer":"%s","total":%d,"ok":%d,"failed":%d,"capacity":%d,"stalled":%d,"repaired":%d,"rescued":%d,"success_rate":"%.1f%%"}' \
      "$reviewer" "$total_r" "$ok_r" "$failed_r" "$capacity_r" "$stalled_r" "$repaired_r" "$rescued_r" \
      "$(echo "scale=1; $ok_r * 100 / ($total_r + 0.001)" | bc 2>/dev/null || echo "0")"
  else
    local success_pct
    if [[ $total_r -gt 0 ]]; then
      success_pct=$(echo "scale=1; $ok_r * 100 / $total_r" | bc 2>/dev/null || echo "?")
    else
      success_pct="N/A"
    fi

    echo -e "${BOLD}${reviewer^^}${NC} — ${total_r} runs, ${GREEN}${ok_r} ok${NC}, ${RED}${failed_r} failed${NC} (${success_pct}% success)"
    if [[ $failed_r -gt 0 ]]; then
      [[ $capacity_r -gt 0 ]] && echo -e "  ${YELLOW}API capacity errors: ${capacity_r}${NC}"
      [[ $stalled_r -gt 0 ]] && echo -e "  ${RED}Watchdog kills (stalled): ${stalled_r}${NC}"
      local other=$((failed_r - capacity_r - stalled_r))
      [[ $other -gt 0 ]] && echo -e "  Other failures: ${other}"
    fi
    if [[ $repaired_r -gt 0 || $rescued_r -gt 0 ]]; then
      echo -e "  ${CYAN}Envelope repairs: ${repaired_r}${NC}, ${CYAN}Rescues: ${rescued_r}${NC}"
    fi
    if $VERBOSE && [[ -n "$failure_details" ]]; then
      echo -e "  ${BOLD}Failure details:${NC}"
      echo -e "$failure_details"
    fi
  fi
}

if ! $JSON_OUTPUT; then
  echo -e "${BOLD}TPR Failure Summary${NC} — $(date '+%Y-%m-%d %H:%M')"
  echo -e "Scanning ${#RUN_DIRS[@]} run directories in /tmp/ori-tpr-*"
  echo ""
fi

if [[ "$REVIEWER_FILTER" == "both" ]]; then
  process_reviewer "gemini"
  $JSON_OUTPUT || echo ""
  process_reviewer "codex"
elif [[ "$REVIEWER_FILTER" == "gemini" || "$REVIEWER_FILTER" == "codex" ]]; then
  process_reviewer "$REVIEWER_FILTER"
else
  echo "invalid --reviewer: $REVIEWER_FILTER (must be codex|gemini|both)" >&2
  exit 2
fi
