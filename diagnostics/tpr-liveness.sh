#!/bin/bash
# tpr-liveness.sh — Classify a TPR reviewer sub-agent as alive | quiet | dead.
#
# Problem this exists to solve:
#   /tpr-review's dual-path transport (invariant I14 in
#   .claude/skills/improve-tooling/tpr-review-design.md) tees each reviewer's
#   CLI stdout to $scratch/{reviewer}-stdout.txt line-by-line. When a reviewer
#   sub-agent returns without a <<<TPR-REPORT>>> block, or when the
#   orchestrator is deciding whether to retry, the critical question is:
#     "was the CLI still doing useful work, or is it genuinely hung?"
#   Without a probe, silence looks identical to death — and the orchestrator
#   biases toward abort, killing agents that were mid-investigation and
#   favoring shallow reviewer work over deep analysis.
#
#   This script turns the tee'd stdout file into a three-state verdict the
#   orchestrator can consult BEFORE retry/abort decisions in SKILL.md §9.

set -euo pipefail

# ---- Defaults ----------------------------------------------------------------
GRACE_SECONDS=300   # Under grace = alive; grace..2*grace = quiet; >=2*grace = dead.
                    # A fresh tool_call signal extends the dead threshold to 4*grace
                    # (default 20 min) so slow Bash invocations inside reviewers
                    # (cargo build, ./test-all.sh) don't get classified as dead
                    # while the reviewer is legitimately waiting on them.
TAIL_LINES=20       # How many trailing lines to scan for alive-signal patterns.
OUTPUT=json         # Default output format.

usage() {
    cat <<'EOF'
Usage: tpr-liveness.sh <scratch_dir> <reviewer> [options]

Classify a TPR reviewer sub-agent's liveness from its teed stdout file.

Arguments:
  scratch_dir   Absolute path to the per-round scratch dir (mktemp -d in
                .claude/skills/tpr-review/SKILL.md §8 step 8a).
  reviewer      One of: codex, gemini (case-insensitive).

Options:
  --grace-seconds N   Quiet threshold (default: 300 = 5 min). Verdict is:
                        alive  if mtime-age < N seconds
                        quiet  if N <= mtime-age < 2N seconds
                        dead   if mtime-age >= 2N seconds
                      Strong alive signals (fresh tool_call / thinking tail,
                      TPR-REPORT sentinel) can upgrade quiet -> alive. An
                      in-flight tool_call signal also extends the dead
                      threshold to 4N (default 20 min) so slow Bash
                      invocations inside a reviewer don't look hung.
  --tail-lines N      Trailing lines inspected for alive patterns (default: 20).
  --json              Emit JSON verdict (default). Safe for orchestrator parsing.
  --human             Emit a single human-readable line.
  --help              Show this help.

Exit codes:
  0   alive — reviewer is still emitting; DO NOT retry/abort.
  1   quiet — borderline; consult once more before retrying.
  2   dead  — no emission for >= 2*grace_seconds; retry or abort is warranted.
  3   usage or infrastructure error (missing args, missing stdout file, etc.).

Example:
  tpr-liveness.sh /tmp/tpr-round-ori_lang-abc123 codex --human
  tpr-liveness.sh /tmp/tpr-round-ori_lang-abc123 gemini --grace-seconds 120 --json

See also:
  .claude/skills/tpr-review/SKILL.md §9 — retry/abort decision path.
  .claude/skills/improve-tooling/tpr-review-design.md §2 I19 — invariant.
EOF
}

# ---- Argument parsing --------------------------------------------------------
if [[ $# -eq 0 ]]; then usage; exit 3; fi

SCRATCH_DIR=""
REVIEWER=""
positional_count=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --help|-h) usage; exit 0 ;;
        --json)    OUTPUT=json; shift ;;
        --human)   OUTPUT=human; shift ;;
        --grace-seconds)
            [[ $# -ge 2 ]] || { echo "Error: --grace-seconds requires a value" >&2; exit 3; }
            GRACE_SECONDS="$2"; shift 2 ;;
        --tail-lines)
            [[ $# -ge 2 ]] || { echo "Error: --tail-lines requires a value" >&2; exit 3; }
            TAIL_LINES="$2"; shift 2 ;;
        --*)
            echo "Error: unknown flag: $1" >&2; usage >&2; exit 3 ;;
        *)
            case $positional_count in
                0) SCRATCH_DIR="$1" ;;
                1) REVIEWER="$1" ;;
                *) echo "Error: too many positional arguments" >&2; exit 3 ;;
            esac
            positional_count=$((positional_count + 1))
            shift ;;
    esac
done

if [[ -z "$SCRATCH_DIR" ]] || [[ -z "$REVIEWER" ]]; then
    echo "Error: scratch_dir and reviewer are required" >&2
    usage >&2
    exit 3
fi

# Normalize reviewer name.
REVIEWER="$(printf '%s' "$REVIEWER" | tr '[:upper:]' '[:lower:]')"
case "$REVIEWER" in
    codex|gemini) ;;
    *) echo "Error: reviewer must be codex or gemini (got: $REVIEWER)" >&2; exit 3 ;;
esac

# Validate numeric args.
case "$GRACE_SECONDS" in
    ''|*[!0-9]*) echo "Error: --grace-seconds must be a non-negative integer" >&2; exit 3 ;;
esac
case "$TAIL_LINES" in
    ''|*[!0-9]*) echo "Error: --tail-lines must be a non-negative integer" >&2; exit 3 ;;
esac

# ---- Locate the stdout file --------------------------------------------------
STDOUT_PATH="$SCRATCH_DIR/${REVIEWER}-stdout.txt"

if [[ ! -d "$SCRATCH_DIR" ]]; then
    emit_missing() {
        case "$OUTPUT" in
            human) echo "dead (scratch_dir does not exist: $SCRATCH_DIR)" ;;
            json)  printf '{"reviewer":"%s","scratch_dir":"%s","exists":false,"verdict":"dead","verdict_reason":"scratch_dir does not exist"}\n' \
                          "$REVIEWER" "$SCRATCH_DIR" ;;
        esac
    }
    emit_missing
    exit 2
fi

if [[ ! -f "$STDOUT_PATH" ]]; then
    # stdout file never created — the CLI never got to Step 2 of tp_agent_prompt.md
    # (or the sub-agent crashed before tee). That's genuinely dead, not quiet.
    case "$OUTPUT" in
        human) echo "dead (stdout file missing: $STDOUT_PATH)" ;;
        json)  printf '{"reviewer":"%s","scratch_dir":"%s","stdout_path":"%s","exists":false,"verdict":"dead","verdict_reason":"stdout file missing — CLI never tee''d"}\n' \
                      "$REVIEWER" "$SCRATCH_DIR" "$STDOUT_PATH" ;;
    esac
    exit 2
fi

# ---- Stat the file (portable: GNU stat first, BSD fallback) ------------------
file_mtime_epoch() {
    if stat -c %Y "$1" >/dev/null 2>&1; then
        stat -c %Y "$1"
    else
        stat -f %m "$1"
    fi
}

file_bytes() {
    if stat -c %s "$1" >/dev/null 2>&1; then
        stat -c %s "$1"
    else
        stat -f %z "$1"
    fi
}

NOW_EPOCH="$(date +%s)"
MTIME_EPOCH="$(file_mtime_epoch "$STDOUT_PATH")"
BYTES="$(file_bytes "$STDOUT_PATH")"
MTIME_AGE=$((NOW_EPOCH - MTIME_EPOCH))
[[ $MTIME_AGE -lt 0 ]] && MTIME_AGE=0   # clock skew guard

# ---- Scan trailing lines for alive signals -----------------------------------
# Signals that the CLI is still doing productive work, even if mtime is old:
#   - <<<TPR-REPORT sentinel present: reviewer is wrapping up, not dead.
#   - Tool-call markers: codex JSON stream, gemini stream-json, Bash/Read hits.
#   - Thinking-token markers: "delta", "text":, "reasoning", etc.

TAIL_CONTENT=""
if [[ $BYTES -gt 0 ]]; then
    TAIL_CONTENT="$(tail -n "$TAIL_LINES" "$STDOUT_PATH" 2>/dev/null || true)"
fi

LAST_LINE=""
if [[ -n "$TAIL_CONTENT" ]]; then
    LAST_LINE="$(printf '%s\n' "$TAIL_CONTENT" | awk 'NF{line=$0} END{print line}')"
fi

TAIL_SIGNAL="none"
SENTINEL_PRESENT=false

if printf '%s' "$TAIL_CONTENT" | grep -q '<<<TPR-REPORT'; then
    TAIL_SIGNAL="final_report_marker"
    SENTINEL_PRESENT=true
elif printf '%s' "$TAIL_CONTENT" | grep -Eq '"tool_use"|"functionCall"|"tool_call"|"shell_call"|→ (Bash|Read|Grep|Edit|Write|Glob)'; then
    TAIL_SIGNAL="tool_call"
elif printf '%s' "$TAIL_CONTENT" | grep -Eq '"delta"|"text":|"reasoning"|"thought"|"content":|"message"'; then
    TAIL_SIGNAL="thinking"
elif printf '%s' "$TAIL_CONTENT" | grep -Eq '"type":"(assistant|user|tool_result)"'; then
    TAIL_SIGNAL="stream_event"
fi

# ---- Compute verdict ---------------------------------------------------------
# Time-based:
#   age < grace        -> alive
#   grace <= age < 2g  -> quiet
#   age >= 2g          -> dead
#
# Override rules:
#   - SENTINEL_PRESENT: always alive (reviewer is writing the final report).
#   - Empty file + age < grace: alive (CLI may be cold-starting).
#   - Empty file + age >= grace: dead (CLI never produced output).

DOUBLE_GRACE=$((GRACE_SECONDS * 2))
QUAD_GRACE=$((GRACE_SECONDS * 4))
VERDICT=""
VERDICT_REASON=""

if $SENTINEL_PRESENT; then
    VERDICT="alive"
    VERDICT_REASON="TPR-REPORT sentinel present in tail — reviewer is emitting the final report"
elif [[ $BYTES -eq 0 ]]; then
    if [[ $MTIME_AGE -lt $GRACE_SECONDS ]]; then
        VERDICT="alive"
        VERDICT_REASON="empty stdout but file is fresh (${MTIME_AGE}s) — CLI likely cold-starting"
    else
        VERDICT="dead"
        VERDICT_REASON="empty stdout and no write in ${MTIME_AGE}s — CLI never emitted"
    fi
elif [[ $MTIME_AGE -lt $GRACE_SECONDS ]]; then
    VERDICT="alive"
    VERDICT_REASON="stdout grew ${MTIME_AGE}s ago (tail_signal=${TAIL_SIGNAL})"
elif [[ $MTIME_AGE -lt $DOUBLE_GRACE ]]; then
    # Within [grace, 2*grace): default to quiet, but upgrade to alive if the
    # last productive emission looks like in-progress work. Principled fix for
    # the "kill deep-investigating agent" bias — a reviewer that JUST emitted
    # a tool_call but hasn't produced stdout since is almost certainly waiting
    # on that tool, not hung.
    if [[ "$TAIL_SIGNAL" == "tool_call" ]] || [[ "$TAIL_SIGNAL" == "thinking" ]]; then
        VERDICT="alive"
        VERDICT_REASON="mtime ${MTIME_AGE}s (past grace) but tail_signal=${TAIL_SIGNAL} — deep work in flight"
    else
        VERDICT="quiet"
        VERDICT_REASON="mtime ${MTIME_AGE}s in [grace, 2*grace), tail_signal=${TAIL_SIGNAL}"
    fi
elif [[ "$TAIL_SIGNAL" == "tool_call" ]] && [[ $MTIME_AGE -lt $QUAD_GRACE ]]; then
    # Extended window: an in-flight tool_call is strong evidence the reviewer
    # is blocked on a slow external process (cargo build, ./test-all.sh,
    # grep over a large corpus) rather than hung. Hold the verdict at quiet
    # through 4*grace (20 min at default) before giving up. The 45-min hook
    # ceiling (block-banned-commands.sh) still caps the absolute worst case.
    VERDICT="quiet"
    VERDICT_REASON="mtime ${MTIME_AGE}s in [2*grace, 4*grace), tail_signal=tool_call — slow tool invocation suspected"
else
    VERDICT="dead"
    VERDICT_REASON="mtime ${MTIME_AGE}s >= 2*grace (${DOUBLE_GRACE}s), tail_signal=${TAIL_SIGNAL}"
fi

# ---- Emit --------------------------------------------------------------------
json_escape() {
    # Escape backslash, double-quote, newline, tab, carriage return for JSON string content.
    printf '%s' "$1" | awk '
        BEGIN { RS="\0" }
        {
            gsub(/\\/, "\\\\")
            gsub(/"/, "\\\"")
            gsub(/\r/, "\\r")
            gsub(/\t/, "\\t")
            gsub(/\n/, "\\n")
            printf "%s", $0
        }'
}

case "$OUTPUT" in
    human)
        printf '%s (reviewer=%s, bytes=%s, mtime_age=%ss, tail_signal=%s) — %s\n' \
               "$VERDICT" "$REVIEWER" "$BYTES" "$MTIME_AGE" "$TAIL_SIGNAL" "$VERDICT_REASON"
        ;;
    json)
        LAST_LINE_ESCAPED="$(json_escape "$LAST_LINE")"
        VERDICT_REASON_ESCAPED="$(json_escape "$VERDICT_REASON")"
        printf '{"reviewer":"%s","scratch_dir":"%s","stdout_path":"%s","exists":true,"bytes":%s,"mtime_age_seconds":%s,"grace_seconds":%s,"tail_signal":"%s","last_line":"%s","verdict":"%s","verdict_reason":"%s"}\n' \
               "$REVIEWER" "$SCRATCH_DIR" "$STDOUT_PATH" "$BYTES" "$MTIME_AGE" "$GRACE_SECONDS" "$TAIL_SIGNAL" "$LAST_LINE_ESCAPED" "$VERDICT" "$VERDICT_REASON_ESCAPED"
        ;;
esac

case "$VERDICT" in
    alive) exit 0 ;;
    quiet) exit 1 ;;
    dead)  exit 2 ;;
esac
