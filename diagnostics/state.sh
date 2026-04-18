#!/bin/bash
# state.sh — Global state indicator for the Ori compiler repo.
#
# Problem this exists to solve:
#   Each fresh Claude session was rediscovering "is the tree in a known-failing
#   state?" from scratch — running ./test-all.sh (~2-3 min), parsing 843
#   failures, grepping file names, cross-referencing the Known Failing Tests
#   table in whichever plan owned the remediation. That discovery cost was
#   paid per-session because the information, despite existing in plan
#   docs, was not session-queryable.
#
#   This script caches that state in .claude/state/known-state.json and
#   exposes it as subcommands. Skills consult `state.sh show --json` on
#   invocation instead of rerunning the test suite.
#
#   Source of truth: the plan-documented "Known Failing Tests" sections
#   remain the SSOT for intent. This cache is an index over that intent,
#   keyed by the commit SHA it was computed at. Consumers that detect
#   SHA mismatch or a dirty working tree treat the cache as stale and
#   fall back to actual runs — fail-safe toward "unknown", never toward
#   "clean".

set -euo pipefail

# ---- Paths -------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
STATE_FILE="$ROOT_DIR/.claude/state/known-state.json"
STATE_DIR="$(dirname "$STATE_FILE")"

# ---- Defaults ----------------------------------------------------------------
OUTPUT=human    # show default; machine consumers pass --json
SUBCMD=""
REFRESH_MODE=""

usage() {
    cat <<'EOF'
Usage: state.sh <subcommand> [options]

Global state indicator for the Ori compiler repo. Caches test-suite status,
clippy status, and repo-hygiene status in .claude/state/known-state.json so
skills don't re-run expensive discovery every session.

Subcommands:
  show                  Pretty-print current cached state (default).
  show --json           Emit JSON verbatim (for skill consumption).
  check                 Verify cache freshness.
                          exit 0 = fresh (SHA matches HEAD, tree clean)
                          exit 1 = stale (SHA matches HEAD but tree dirty)
                          exit 2 = obsolete (SHA != HEAD — commit happened)
                          exit 3 = missing (state file absent)
  known-failing         List known-failing test files, one per line.
                        --json outputs as JSON array. Useful for skills that
                        want to diff current test output against the cache.
  refresh               Update the cache.
                        --sha-only          Update head_sha + updated_at only
                                            (fast; no test rerun). Use this
                                            from commit-push post-commit.
                        --full              Run ./test-all.sh + ./clippy-all.sh,
                                            rewrite all sections. Slow (~3 min).
                        --hygiene-only      Run diagnostics/repo-hygiene.sh
                                            --check and update hygiene block.
                        --by <name>         Record who/what triggered the
                                            refresh. Defaults to "manual".
                                            Values: commit-push, manual,
                                            full-check, section-close.

Options (all subcommands):
  --json                Machine-readable output.
  --human               Human-readable output (default for `show`).
  --help, -h            Show this help.

Examples:
  state.sh show
  state.sh show --json | jq '.test_suite.status'
  state.sh check && echo "cache fresh" || echo "stale"
  state.sh refresh --sha-only --by commit-push
  state.sh refresh --full --by section-close
  state.sh known-failing --json | jq '.[]' | wc -l

See also:
  .claude/skills/improve-tooling/script-state-design.md — design log
  .claude/state/known-state.json — the cache file (schema v1)
EOF
}

# ---- Helpers -----------------------------------------------------------------
die() { echo "Error: $*" >&2; exit 3; }

require_jq() {
    command -v jq >/dev/null 2>&1 || die "jq is required for this subcommand. Install via apt/brew."
}

require_state_file() {
    [[ -f "$STATE_FILE" ]] || die "state file not found at $STATE_FILE. Run: diagnostics/state.sh refresh --full"
}

current_head_sha() {
    git -C "$ROOT_DIR" rev-parse --short HEAD 2>/dev/null || echo "unknown"
}

is_tree_dirty() {
    # Exclude the state file itself from the dirty-tree check. state.sh
    # refresh writes to it, which would otherwise always mark the tree
    # dirty post-refresh even when everything else is clean. This is
    # load-bearing for /commit-push Step 8: after the post-push refresh,
    # the state file is the ONLY uncommitted file; consumers must still
    # see a FRESH verdict from state.sh check. See
    # .claude/skills/improve-tooling/script-state-design.md §6 (closed
    # 2026-04-18) for the surfacing incident.
    local dirty
    dirty=$(git -C "$ROOT_DIR" status --porcelain 2>/dev/null \
        | grep -v '^.. \.claude/state/known-state\.json$' || true)
    [[ -n "$dirty" ]]
}

iso_now() {
    date -u +"%Y-%m-%dT%H:%M:%SZ"
}

# Atomic JSON write: write to .tmp, then rename.
write_state() {
    local content="$1"
    local tmp="$STATE_FILE.tmp.$$"
    printf '%s\n' "$content" > "$tmp"
    mv "$tmp" "$STATE_FILE"
}

# ---- Subcommand: show --------------------------------------------------------
cmd_show() {
    require_state_file
    if [[ "$OUTPUT" == "json" ]]; then
        cat "$STATE_FILE"
        return 0
    fi

    # Human-readable summary. Pure bash, no jq requirement for read-only show.
    require_jq
    local head_sha current_sha tree_dirty
    head_sha=$(jq -r '.head_sha' "$STATE_FILE")
    current_sha=$(current_head_sha)
    tree_dirty="no"
    if is_tree_dirty; then tree_dirty="yes"; fi

    echo "=== Ori compiler state (cache @ .claude/state/known-state.json) ==="
    echo
    echo "Cache SHA:         $head_sha"
    echo "Current HEAD SHA:  $current_sha"
    echo "Tree dirty:        $tree_dirty"
    if [[ "$head_sha" == "$current_sha" && "$tree_dirty" == "no" ]]; then
        echo "Freshness:         FRESH"
    elif [[ "$head_sha" == "$current_sha" ]]; then
        echo "Freshness:         STALE (tree has uncommitted changes)"
    else
        echo "Freshness:         OBSOLETE (SHA mismatch — commit happened since last refresh)"
    fi
    echo "Updated at:        $(jq -r '.updated_at' "$STATE_FILE")"
    echo "Updated by:        $(jq -r '.updated_by' "$STATE_FILE")"
    echo

    echo "--- Test suite ---"
    echo "Status:            $(jq -r '.test_suite.status' "$STATE_FILE")"
    local passed failed skipped
    passed=$(jq -r '.test_suite.totals.passed' "$STATE_FILE")
    failed=$(jq -r '.test_suite.totals.failed' "$STATE_FILE")
    skipped=$(jq -r '.test_suite.totals.skipped' "$STATE_FILE")
    echo "Totals:            passed=$passed  failed=$failed  skipped=$skipped"
    echo "Last run SHA:      $(jq -r '.test_suite.last_run_sha' "$STATE_FILE")"
    echo "Last run at:       $(jq -r '.test_suite.last_run_at' "$STATE_FILE")"
    local kf_count
    kf_count=$(jq -r '.test_suite.known_failing_count // (.test_suite.known_failing_files | length)' "$STATE_FILE")
    echo "Known-failing:     $kf_count files"
    local failure_class
    failure_class=$(jq -r '.test_suite.failure_class // "(none)"' "$STATE_FILE")
    echo "Failure class:     $failure_class"
    echo
    echo "Remediation:"
    jq -r '.test_suite.remediation[]? | "  - \(.plan) §\(.subsection) — \(.class)"' "$STATE_FILE"
    echo

    echo "--- Clippy ---"
    echo "Status:            $(jq -r '.clippy.status' "$STATE_FILE")"
    echo "Last run SHA:      $(jq -r '.clippy.last_run_sha' "$STATE_FILE")"
    echo

    echo "--- Repo hygiene ---"
    echo "Status:            $(jq -r '.hygiene.status' "$STATE_FILE")"
    echo "Notes:             $(jq -r '.hygiene.notes // "(none)"' "$STATE_FILE")"
}

# ---- Subcommand: check -------------------------------------------------------
cmd_check() {
    if [[ ! -f "$STATE_FILE" ]]; then
        [[ "$OUTPUT" == "json" ]] && echo '{"status":"missing"}'
        [[ "$OUTPUT" == "human" ]] && echo "state file missing"
        exit 3
    fi
    require_jq
    local head_sha current_sha
    head_sha=$(jq -r '.head_sha' "$STATE_FILE")
    current_sha=$(current_head_sha)
    local dirty="no"
    is_tree_dirty && dirty="yes"

    if [[ "$head_sha" != "$current_sha" ]]; then
        [[ "$OUTPUT" == "json" ]] && printf '{"status":"obsolete","cache_sha":"%s","head_sha":"%s"}\n' "$head_sha" "$current_sha"
        [[ "$OUTPUT" == "human" ]] && echo "OBSOLETE: cache SHA ($head_sha) != HEAD SHA ($current_sha). Run: state.sh refresh --sha-only"
        exit 2
    fi
    if [[ "$dirty" == "yes" ]]; then
        [[ "$OUTPUT" == "json" ]] && printf '{"status":"stale","reason":"dirty_tree"}\n'
        [[ "$OUTPUT" == "human" ]] && echo "STALE: tree has uncommitted changes. Cache may not reflect current state."
        exit 1
    fi
    [[ "$OUTPUT" == "json" ]] && printf '{"status":"fresh","head_sha":"%s"}\n' "$head_sha"
    [[ "$OUTPUT" == "human" ]] && echo "FRESH: cache matches HEAD ($head_sha), tree clean."
    exit 0
}

# ---- Subcommand: known-failing -----------------------------------------------
cmd_known_failing() {
    require_state_file
    require_jq
    if [[ "$OUTPUT" == "json" ]]; then
        jq '.test_suite.known_failing_files' "$STATE_FILE"
    else
        jq -r '.test_suite.known_failing_files[]' "$STATE_FILE"
    fi
}

# ---- Subcommand: refresh -----------------------------------------------------
cmd_refresh() {
    require_jq
    local current_sha updated_at updated_by_val
    current_sha=$(current_head_sha)
    updated_at=$(iso_now)
    updated_by_val="${UPDATED_BY:-manual}"

    if [[ ! -f "$STATE_FILE" ]]; then
        die "refresh --sha-only requires an existing state file. For first-time setup, use refresh --full."
    fi

    case "$REFRESH_MODE" in
        sha-only|"")
            # Fast path: just update the top-level SHA + timestamp.
            local tmp
            tmp=$(jq --arg sha "$current_sha" \
                    --arg at "$updated_at" \
                    --arg by "$updated_by_val" \
                    '.head_sha = $sha | .updated_at = $at | .updated_by = $by' \
                    "$STATE_FILE")
            write_state "$tmp"
            [[ "$OUTPUT" == "json" ]] && printf '{"status":"refreshed","mode":"sha-only","head_sha":"%s"}\n' "$current_sha"
            [[ "$OUTPUT" == "human" ]] && echo "state refreshed (sha-only): head_sha=$current_sha updated_by=$updated_by_val"
            ;;
        hygiene-only)
            local hygiene_output hygiene_status
            if hygiene_output=$(diagnostics/repo-hygiene.sh --check 2>&1); then
                hygiene_status="clean"
            else
                hygiene_status="noise"
            fi
            # First line of output as a compact note; full output lives in the script run.
            local notes
            notes=$(printf '%s' "$hygiene_output" | head -1 | sed 's/"/\\"/g')
            local tmp
            tmp=$(jq --arg sha "$current_sha" \
                    --arg at "$updated_at" \
                    --arg by "$updated_by_val" \
                    --arg status "$hygiene_status" \
                    --arg notes "$notes" \
                    '.head_sha = $sha | .updated_at = $at | .updated_by = $by
                     | .hygiene.status = $status
                     | .hygiene.last_run_sha = $sha
                     | .hygiene.last_run_at = $at
                     | .hygiene.notes = $notes' \
                    "$STATE_FILE")
            write_state "$tmp"
            [[ "$OUTPUT" == "json" ]] && printf '{"status":"refreshed","mode":"hygiene-only","hygiene_status":"%s"}\n' "$hygiene_status"
            [[ "$OUTPUT" == "human" ]] && echo "hygiene block refreshed: status=$hygiene_status"
            ;;
        full)
            echo "Running ./test-all.sh + ./clippy-all.sh (this takes ~3 minutes)..." >&2
            local test_log clippy_log test_status clippy_status
            test_log="$ROOT_DIR/build/state-refresh-test.log"
            clippy_log="$ROOT_DIR/build/state-refresh-clippy.log"
            mkdir -p "$ROOT_DIR/build"

            if timeout 150 "$ROOT_DIR/test-all.sh" > "$test_log" 2>&1; then
                test_status="clean"
            else
                test_status="known-failing"
            fi
            if timeout 150 "$ROOT_DIR/clippy-all.sh" > "$clippy_log" 2>&1; then
                clippy_status="clean"
            else
                clippy_status="warnings"
            fi

            # Parse test-all.sh SUMMARY totals from the log.
            # Format lives in test-all.sh; look for the TOTAL row.
            local passed failed skipped
            passed=$(awk '/^TOTAL/ {print $2}' "$test_log" | tail -1)
            failed=$(awk '/^TOTAL/ {print $3}' "$test_log" | tail -1)
            skipped=$(awk '/^TOTAL/ {print $4}' "$test_log" | tail -1)
            passed=${passed:-0}; failed=${failed:-0}; skipped=${skipped:-0}

            local tmp
            tmp=$(jq --arg sha "$current_sha" \
                    --arg at "$updated_at" \
                    --arg by "$updated_by_val" \
                    --arg tstatus "$test_status" \
                    --argjson passed "$passed" \
                    --argjson failed "$failed" \
                    --argjson skipped "$skipped" \
                    --arg cstatus "$clippy_status" \
                    '.head_sha = $sha | .updated_at = $at | .updated_by = $by
                     | .test_suite.status = $tstatus
                     | .test_suite.last_run_sha = $sha
                     | .test_suite.last_run_at = $at
                     | .test_suite.last_run_kind = "test-all.sh"
                     | .test_suite.totals.passed = $passed
                     | .test_suite.totals.failed = $failed
                     | .test_suite.totals.skipped = $skipped
                     | .clippy.status = $cstatus
                     | .clippy.last_run_sha = $sha
                     | .clippy.last_run_at = $at' \
                    "$STATE_FILE")
            write_state "$tmp"
            [[ "$OUTPUT" == "json" ]] && printf '{"status":"refreshed","mode":"full","test_status":"%s","clippy_status":"%s","passed":%s,"failed":%s}\n' "$test_status" "$clippy_status" "$passed" "$failed"
            [[ "$OUTPUT" == "human" ]] && echo "full refresh complete: tests=$test_status clippy=$clippy_status totals=$passed/$failed/$skipped"
            echo "Note: known_failing_files list is NOT auto-populated from test-all.sh — it reflects plan intent." >&2
            echo "      If the failing set changed, update plan Known Failing Tests + edit state file accordingly." >&2
            ;;
        *)
            die "unknown refresh mode: $REFRESH_MODE. Use --sha-only, --hygiene-only, or --full."
            ;;
    esac
}

# ---- Argument parsing --------------------------------------------------------
if [[ $# -eq 0 ]]; then usage; exit 3; fi

UPDATED_BY=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --help|-h) usage; exit 0 ;;
        --json) OUTPUT=json; shift ;;
        --human) OUTPUT=human; shift ;;
        --sha-only) REFRESH_MODE=sha-only; shift ;;
        --full) REFRESH_MODE=full; shift ;;
        --hygiene-only) REFRESH_MODE=hygiene-only; shift ;;
        --by)
            [[ $# -ge 2 ]] || die "--by requires a value"
            UPDATED_BY="$2"; shift 2 ;;
        --*) die "unknown flag: $1" ;;
        show|check|refresh|known-failing)
            [[ -z "$SUBCMD" ]] || die "multiple subcommands: $SUBCMD and $1"
            SUBCMD="$1"; shift ;;
        *) die "unknown argument: $1" ;;
    esac
done

case "$SUBCMD" in
    show) cmd_show ;;
    check) cmd_check ;;
    refresh) cmd_refresh ;;
    known-failing) cmd_known_failing ;;
    "") usage; exit 3 ;;
    *) die "unknown subcommand: $SUBCMD" ;;
esac
