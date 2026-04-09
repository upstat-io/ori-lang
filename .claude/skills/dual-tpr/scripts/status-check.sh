#!/usr/bin/env bash
# status-check.sh — real-time status snapshot for an in-flight dual-tpr
# review round. Reads append-only artifacts (round.log, JSONL streams,
# done-signal files) and extracts the LATEST reviewer activity — what
# commands each reviewer is running, what files it's reading, what
# it's thinking — so an operator can see live progress without polling
# the atomic-write envelope files.
#
# Usage:
#   .claude/skills/dual-tpr/scripts/status-check.sh <RUN-dir> [--events N]
#
# Options:
#   --events N   Show the last N events per reviewer (default: 5)
#
# Safe to call freely during a long-running round. Reads ONLY:
#   - $RUN/round.log           (append-only orchestration log)
#   - $RUN/codex.jsonl         (append-only codex event stream)
#   - $RUN/gemini.jsonl        (append-only gemini event stream)
#   - $RUN/{reviewer}.exit     (written atomically at subshell exit)
#   - $RUN/{reviewer}.walltime
#   - $RUN/{reviewer}.skipped  (written by dual-invoke.sh when a reviewer
#                              is filtered out via ORI_TPR_REVIEWERS; §07.3
#                              TPR fix — distinguishes "skipped" from
#                              "still running" so a filtered-out reviewer
#                              no longer shows as running forever)
#   - $RUN/{reviewer}.envelope.json  (existence check only, NEVER read mid-stream)
#   - $RUN/{reviewer}.parse-error
#
# Designed for 5-minute polling during the `wait for completion
# notification` phase of /tpr-review. See tpr-review/SKILL.md §Polling
# Protocol for the full rule: poll append-only streams for situational
# awareness, but NEVER poll envelope.json (atomic-write race hazard).

set -uo pipefail

RUN="${1:-}"
EVENT_COUNT=5

# Parse optional flags after RUN
shift 2>/dev/null || true
while [[ $# -gt 0 ]]; do
  case "$1" in
    --events)
      if [[ -z "${2:-}" ]]; then
        printf 'error: --events requires an argument\n' >&2
        exit 2
      fi
      # Validate: must be a positive integer. Reject empty, non-numeric,
      # zero, and negative values in bash BEFORE passing to Python, so
      # operators get a clean usage error instead of a Python traceback.
      # (TPR-04-004-codex regression.)
      if ! [[ "$2" =~ ^[0-9]+$ ]] || [[ "$2" == "0" ]]; then
        printf 'error: --events must be a positive integer (got: %s)\n' "$2" >&2
        exit 2
      fi
      EVENT_COUNT="$2"
      shift 2
      ;;
    --help|-h)
      sed -n '2,30p' "$0"
      exit 0
      ;;
    *)
      printf 'unknown arg: %s (use --help)\n' "$1" >&2
      exit 2
      ;;
  esac
done

if [[ -z "$RUN" ]]; then
  printf 'usage: status-check.sh <RUN-dir> [--events N]\n' >&2
  exit 2
fi
if [[ ! -d "$RUN" ]]; then
  printf 'error: not a directory: %s\n' "$RUN" >&2
  exit 2
fi

# ── Color setup (matches validate-dual-tpr.sh convention) ───────────
GREEN=$'\033[0;32m'
YELLOW=$'\033[0;33m'
RED=$'\033[0;31m'
DIM=$'\033[0;2m'
BOLD=$'\033[1m'
RESET=$'\033[0m'
if [[ -n "${NO_COLOR:-}" ]] || [[ ! -t 1 ]]; then
  GREEN='' YELLOW='' RED='' DIM='' BOLD='' RESET=''
fi

printf '%s=== dual-tpr status @ %s ===%s\n' "$BOLD" "$(date '+%Y-%m-%d %H:%M:%S %Z')" "$RESET"
printf '%srun:%s %s\n' "$DIM" "$RESET" "$RUN"
printf '\n'

# ── Round log tail — authoritative progress record ──────────────────
if [[ -s "$RUN/round.log" ]]; then
  printf '%sround.log (last 5 lines):%s\n' "$BOLD" "$RESET"
  tail -5 "$RUN/round.log" | sed 's/^/  /'
else
  printf '%sround.log:%s (empty or missing)\n' "$DIM" "$RESET"
fi
printf '\n'

# ── Reviewer activity — real streamed events ────────────────────────
# Uses python3 because the JSONL parsing is too gnarly in pure bash and
# every dual-tpr host already has python3 (required by parse-codex.py /
# parse-gemini.py). A pure-bash version would mis-handle escaped quotes,
# multi-line content fields, and unicode in the reviewer messages.
EVENT_COUNT="$EVENT_COUNT" RUN="$RUN" GREEN="$GREEN" YELLOW="$YELLOW" \
RED="$RED" DIM="$DIM" BOLD="$BOLD" RESET="$RESET" python3 - <<'PY'
import json, os, sys

RUN = os.environ["RUN"]
N = int(os.environ.get("EVENT_COUNT", "5"))

# Color passthrough from shell. Empty when NO_COLOR / not a tty.
GREEN = os.environ.get("GREEN", "")
YELLOW = os.environ.get("YELLOW", "")
RED = os.environ.get("RED", "")
DIM = os.environ.get("DIM", "")
BOLD = os.environ.get("BOLD", "")
RESET = os.environ.get("RESET", "")

def truncate(s, n=180):
    """Collapse whitespace and cap length for terminal output."""
    if not isinstance(s, str):
        s = str(s)
    s = " ".join(s.split())
    return s if len(s) <= n else s[:n-1] + "…"

def load_jsonl(path):
    """Read a JSONL file and return the list of parsed events.

    Tolerates malformed lines (codex occasionally emits non-JSON noise
    on stderr that gets mixed into the stream if redirection fails).
    """
    events = []
    if not os.path.exists(path):
        return events
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                events.append(json.loads(line))
            except json.JSONDecodeError:
                continue
    return events

def fmt_codex_event(e):
    """Render one codex event as a readable status line.

    Codex event types we care about (from codex exec --json output):
      - item.completed with item.type in {reasoning, command_execution,
        agent_message, todo_list}
      - turn.started / turn.completed / turn.failed
      - error (top-level error event)
    """
    t = e.get("type", "?")
    if t == "item.completed":
        item = e.get("item", {}) or {}
        it = item.get("type", "?")
        if it == "reasoning":
            return f"[reasoning] {truncate(item.get('text',''))}"
        if it == "command_execution":
            cmd = item.get("command") or item.get("cmd") or ""
            return f"[cmd] {truncate(cmd, 150)}"
        if it == "agent_message":
            return f"[agent_message] {truncate(item.get('text',''))}"
        if it == "todo_list":
            items = item.get("items", []) or []
            pending = sum(1 for x in items if not x.get("completed"))
            total = len(items)
            return f"[todo_list] {pending}/{total} pending"
        return f"[item.{it}]"
    if t == "item.started":
        item = e.get("item", {}) or {}
        it = item.get("type", "?")
        return f"[item.started {it}]"
    if t == "error":
        return f"{RED}[error]{RESET} {truncate(e.get('message',''), 150)}"
    if t == "turn.started":
        return "[turn.started]"
    if t == "turn.completed":
        return "[turn.completed]"
    if t == "turn.failed":
        return f"{RED}[turn.failed]{RESET}"
    return f"[{t}]"

def fmt_gemini_event(e):
    """Render one gemini event as a readable status line.

    Gemini stream-json event types (from gemini --output-format stream-json):
      - init
      - message (role=user|assistant, content, delta)
      - tool_use (tool_name, parameters)
      - tool_result (tool_id, status, output)
      - result (status=success|failure, terminal marker)
    """
    t = e.get("type", "?")
    if t == "message":
        role = e.get("role", "?")
        content = e.get("content", "")
        delta = " delta" if e.get("delta") else ""
        return f"[msg.{role}{delta}] {truncate(content)}"
    if t == "tool_use":
        tn = e.get("tool_name", "?")
        params = e.get("parameters", {}) or {}
        # Show the command for shell-style tools; fall back to params
        cmd = (params.get("command") if isinstance(params, dict) else None) or truncate(params, 150)
        return f"[tool_use:{tn}] {truncate(cmd, 150)}"
    if t == "tool_result":
        status = e.get("status", "?")
        output = e.get("output", "")
        color = GREEN if status == "success" else RED
        return f"{color}[tool_result:{status}]{RESET} {truncate(output, 120)}"
    if t == "result":
        status = e.get("status", "?")
        color = GREEN if status == "success" else RED
        return f"{color}[result:{status}]{RESET}"
    return f"[{t}]"

def print_reviewer(name, jsonl_path, fmt_fn):
    events = load_jsonl(jsonl_path)
    exit_file = os.path.join(RUN, f"{name}.exit")
    walltime_file = os.path.join(RUN, f"{name}.walltime")
    envelope_file = os.path.join(RUN, f"{name}.envelope.json")
    parse_err_file = os.path.join(RUN, f"{name}.parse-error")
    skipped_file = os.path.join(RUN, f"{name}.skipped")

    # Header with subshell state. Three-state distinction (§07.3 TPR fix):
    #   1. `.skipped` exists  → reviewer was filtered out by ORI_TPR_REVIEWERS
    #   2. `.exit` exists     → reviewer ran to completion (check rc for pass/fail)
    #   3. neither exists     → reviewer still running
    # Without the `.skipped` branch, a filtered-out reviewer would show as
    # "running" forever, which was the original bug codex surfaced.
    if os.path.exists(skipped_file):
        try:
            reason = open(skipped_file).read().strip()
        except Exception:
            reason = "?"
        status = f"{DIM}skipped{RESET} ({reason})"
    elif os.path.exists(exit_file):
        try:
            rc = open(exit_file).read().strip()
            wt = open(walltime_file).read().strip() if os.path.exists(walltime_file) else "?"
        except Exception:
            rc, wt = "?", "?"
        color = GREEN if rc == "0" else RED
        status = f"{color}done{RESET} (rc={rc}, walltime={wt}s)"
    else:
        status = f"{YELLOW}running{RESET}"

    size = os.path.getsize(jsonl_path) if os.path.exists(jsonl_path) else 0
    print(f"{BOLD}{name}:{RESET} {status}  {DIM}({size} bytes, {len(events)} events){RESET}")

    # Parser / envelope state
    if os.path.exists(envelope_file) and os.path.getsize(envelope_file) > 0:
        esize = os.path.getsize(envelope_file)
        print(f"  {GREEN}envelope parsed{RESET} ({esize} bytes)")
    elif os.path.exists(parse_err_file) and os.path.getsize(parse_err_file) > 0:
        with open(parse_err_file) as f:
            first = f.readline().strip()
        print(f"  {RED}parse failed:{RESET} {first}")

    # Last N events
    if events:
        print(f"  {DIM}last {min(N, len(events))} events:{RESET}")
        for e in events[-N:]:
            print(f"    {fmt_fn(e)}")
    else:
        print(f"  {DIM}(no events yet){RESET}")
    print()

print_reviewer("codex", os.path.join(RUN, "codex.jsonl"), fmt_codex_event)
print_reviewer("gemini", os.path.join(RUN, "gemini.jsonl"), fmt_gemini_event)

# Worktree state
wt_err = os.path.join(RUN, "worktree-error")
wt_before = os.path.join(RUN, "worktree-before.txt")
if os.path.exists(wt_err) and os.path.getsize(wt_err) > 0:
    print(f"{BOLD}worktree:{RESET} {RED}dirty{RESET} ({os.path.getsize(wt_err)} bytes in worktree-error)")
elif os.path.exists(wt_before):
    print(f"{BOLD}worktree:{RESET} snapshot recorded")
PY
