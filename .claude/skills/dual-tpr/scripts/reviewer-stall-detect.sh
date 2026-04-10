#!/bin/bash
# reviewer-stall-detect.sh — Intelligent stall detection for reviewer processes
#
# Compares two snapshots of process activity to classify a reviewer as:
#   ACTIVE    — JSONL output growing (reviewer producing results)
#   RECEIVING — JSONL stable but process reading data (API streaming, file I/O)
#   COMPUTING — JSONL stable, no I/O, but CPU active (processing/thinking)
#   IDLE      — no output, no I/O, no CPU activity (likely hung)
#   DEAD      — process doesn't exist
#
# Uses three independent signals from /proc:
#   1. JSONL file size — reviewer output growth
#   2. /proc/PID/io rchar — total bytes read (network + files)
#   3. /proc/PID/stat utime+stime — CPU ticks consumed
#
# Usage:
#   reviewer-stall-detect.sh --pid <pid> --jsonl <path> [--interval <sec>]
#   reviewer-stall-detect.sh --snapshot --pid <pid> --jsonl <path> --out <file>
#   reviewer-stall-detect.sh --compare --previous <file> --pid <pid> --jsonl <path>
#
# Modes:
#   Default (no --snapshot/--compare): takes two samples separated by --interval
#     seconds and classifies. Simple but blocking.
#
#   --snapshot + --compare: two-phase non-blocking mode for integration into
#     loops. --snapshot writes current state to a file; --compare reads the
#     previous snapshot and compares against current state.
#
# Exit codes:
#   0 = ACTIVE or RECEIVING or COMPUTING (not stalled)
#   1 = IDLE (all signals frozen — likely stalled)
#   2 = DEAD (process doesn't exist)
#   3 = usage error
#
# Output (stdout): one line — the classification (ACTIVE/RECEIVING/COMPUTING/IDLE/DEAD)
# Machine-readable details on stderr when --verbose.

set -euo pipefail

# Defaults
PID=""
JSONL=""
INTERVAL=30
MODE="inline"  # inline | snapshot | compare
SNAPSHOT_OUT=""
PREVIOUS=""
VERBOSE=0

usage() {
  sed -n '2,/^$/{ s/^# \?//; p }' "$0"
  exit 3
}

while [[ $# -gt 0 ]]; do
  case $1 in
    --pid) PID="$2"; shift 2 ;;
    --jsonl) JSONL="$2"; shift 2 ;;
    --interval) INTERVAL="$2"; shift 2 ;;
    --snapshot) MODE="snapshot"; shift ;;
    --compare) MODE="compare"; shift ;;
    --out) SNAPSHOT_OUT="$2"; shift 2 ;;
    --previous) PREVIOUS="$2"; shift 2 ;;
    --verbose) VERBOSE=1; shift ;;
    -h|--help) usage ;;
    *) echo "Error: unknown option: $1" >&2; exit 3 ;;
  esac
done

[[ -z "$PID" ]] && { echo "Error: --pid required" >&2; exit 3; }
[[ -z "$JSONL" ]] && { echo "Error: --jsonl required" >&2; exit 3; }

# --- Signal collection ---

collect_signals() {
  local pid="$1" jsonl="$2"

  # Check process exists
  if ! kill -0 "$pid" 2>/dev/null; then
    echo "DEAD 0 0 0"
    return
  fi

  # 1. JSONL file size
  local jsonl_bytes=0
  if [[ -f "$jsonl" ]]; then
    jsonl_bytes=$(stat -c %s "$jsonl" 2>/dev/null || echo 0)
  fi

  # 2. Process I/O: rchar from /proc/PID/io
  local rchar=0
  if [[ -r "/proc/$pid/io" ]]; then
    rchar=$(awk '/^rchar:/ { print $2 }' "/proc/$pid/io" 2>/dev/null || echo 0)
  fi

  # 3. CPU ticks: utime + stime from /proc/PID/stat
  # Fields 14 (utime) and 15 (stime) in /proc/PID/stat
  local cpu_ticks=0
  if [[ -r "/proc/$pid/stat" ]]; then
    cpu_ticks=$(awk '{ print $14 + $15 }' "/proc/$pid/stat" 2>/dev/null || echo 0)
  fi

  # Also collect from all children (the node worker subprocess)
  local child_rchar=0 child_cpu=0
  for child_pid in $(ps --ppid "$pid" -o pid= 2>/dev/null); do
    child_pid=$(echo "$child_pid" | tr -d ' ')
    if [[ -r "/proc/$child_pid/io" ]]; then
      local cr
      cr=$(awk '/^rchar:/ { print $2 }' "/proc/$child_pid/io" 2>/dev/null || echo 0)
      child_rchar=$((child_rchar + cr))
    fi
    if [[ -r "/proc/$child_pid/stat" ]]; then
      local ct
      ct=$(awk '{ print $14 + $15 }' "/proc/$child_pid/stat" 2>/dev/null || echo 0)
      child_cpu=$((child_cpu + ct))
    fi
  done

  # Aggregate: parent + children
  local total_rchar=$((rchar + child_rchar))
  local total_cpu=$((cpu_ticks + child_cpu))

  echo "ALIVE $jsonl_bytes $total_rchar $total_cpu"
}

classify() {
  local prev_status="$1" prev_jsonl="$2" prev_rchar="$3" prev_cpu="$4"
  local curr_status="$5" curr_jsonl="$6" curr_rchar="$7" curr_cpu="$8"

  if [[ "$curr_status" == "DEAD" ]]; then
    echo "DEAD"
    return
  fi

  local jsonl_delta=$((curr_jsonl - prev_jsonl))
  local rchar_delta=$((curr_rchar - prev_rchar))
  local cpu_delta=$((curr_cpu - prev_cpu))

  if [[ $VERBOSE -eq 1 ]]; then
    echo "jsonl_delta=$jsonl_delta rchar_delta=$rchar_delta cpu_delta=$cpu_delta" >&2
    echo "prev: status=$prev_status jsonl=$prev_jsonl rchar=$prev_rchar cpu=$prev_cpu" >&2
    echo "curr: status=$curr_status jsonl=$curr_jsonl rchar=$curr_rchar cpu=$curr_cpu" >&2
  fi

  if [[ $jsonl_delta -gt 0 ]]; then
    echo "ACTIVE"
  elif [[ $rchar_delta -gt 0 ]]; then
    echo "RECEIVING"
  elif [[ $cpu_delta -gt 0 ]]; then
    echo "COMPUTING"
  else
    echo "IDLE"
  fi
}

# --- Mode dispatch ---

case "$MODE" in
  snapshot)
    [[ -z "$SNAPSHOT_OUT" ]] && { echo "Error: --out required with --snapshot" >&2; exit 3; }
    signals=$(collect_signals "$PID" "$JSONL")
    echo "$signals" > "$SNAPSHOT_OUT"
    # Also echo classification-ready timestamp
    echo "$(date +%s)" >> "$SNAPSHOT_OUT"
    if [[ $VERBOSE -eq 1 ]]; then
      echo "Snapshot: $signals ($(date +%H:%M:%S))" >&2
    fi
    exit 0
    ;;

  compare)
    [[ -z "$PREVIOUS" ]] && { echo "Error: --previous required with --compare" >&2; exit 3; }
    [[ ! -f "$PREVIOUS" ]] && { echo "Error: previous snapshot not found: $PREVIOUS" >&2; exit 3; }

    # Read previous snapshot
    prev_line=$(head -1 "$PREVIOUS")
    read -r prev_status prev_jsonl prev_rchar prev_cpu <<< "$prev_line"

    # Collect current signals
    curr_line=$(collect_signals "$PID" "$JSONL")
    read -r curr_status curr_jsonl curr_rchar curr_cpu <<< "$curr_line"

    result=$(classify "$prev_status" "$prev_jsonl" "$prev_rchar" "$prev_cpu" \
                      "$curr_status" "$curr_jsonl" "$curr_rchar" "$curr_cpu")
    echo "$result"

    # Write current as new snapshot for next comparison
    echo "$curr_line" > "$PREVIOUS"
    echo "$(date +%s)" >> "$PREVIOUS"

    case "$result" in
      ACTIVE|RECEIVING|COMPUTING) exit 0 ;;
      IDLE) exit 1 ;;
      DEAD) exit 2 ;;
    esac
    ;;

  inline)
    # Two-sample inline mode
    signals1=$(collect_signals "$PID" "$JSONL")
    read -r s1_status s1_jsonl s1_rchar s1_cpu <<< "$signals1"

    if [[ "$s1_status" == "DEAD" ]]; then
      echo "DEAD"
      exit 2
    fi

    sleep "$INTERVAL"

    signals2=$(collect_signals "$PID" "$JSONL")
    read -r s2_status s2_jsonl s2_rchar s2_cpu <<< "$signals2"

    result=$(classify "$s1_status" "$s1_jsonl" "$s1_rchar" "$s1_cpu" \
                      "$s2_status" "$s2_jsonl" "$s2_rchar" "$s2_cpu")
    echo "$result"

    case "$result" in
      ACTIVE|RECEIVING|COMPUTING) exit 0 ;;
      IDLE) exit 1 ;;
      DEAD) exit 2 ;;
    esac
    ;;
esac
