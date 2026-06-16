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
# STATE_FILE / BASELINES_FILE accept env overrides (default to canonical paths)
# so the baseline subcommand is testable in isolation against a temp dir.
STATE_FILE="${ORI_STATE_FILE:-$ROOT_DIR/.claude/state/known-state.json}"
STATE_DIR="$(dirname "$STATE_FILE")"
BASELINES_FILE="${ORI_BASELINES_FILE:-$STATE_DIR/baselines.json}"
# Append-only test-all metrics ledger (data file; written by the test-all
# ledger ingest, read here for display). Env override for isolated testing.
LEDGER_FILE="${ORI_LEDGER_FILE:-$ROOT_DIR/build/test-all-ledger.json}"
# Known aims-burden AOT floor (one test-id per line). baseline_compare excludes
# these from newly_failing/newly_fixed so a known floor cell never reads as a
# section-introduced regression (BUG-07-263). Env override for isolated testing;
# a missing file degrades to empty-floor (legacy) behavior.
FLOOR_FILE="${ORI_BASELINE_FLOOR_FILE:-$SCRIPT_DIR/baseline_failing_ids.txt}"

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
  check --stale         Extended freshness check. Warns when test_suite is
                        unpopulated, lags HEAD by >=10 commits, last refresh
                        was >=24 hours ago, or compiler source files changed
                        since last refresh.
                          exit 0 = fresh  |  exit 4 = stale (test_suite needs refresh)
  known-failing         List known-failing test files, one per line.
                        --json outputs as JSON array. Useful for skills that
                        want to diff current test output against the cache.
  dispositions          List disposition-tagged tests (#[ignore], #skip), one
                        per line as: <file>:<line>\t<kind>\t<tracking_bug>\t<reason>
                        --json outputs as a JSON array of entry objects.
                        --untracked-only filters to entries with tracking_bug=null.
  ledger                Latest test-all metrics-ledger run: per-metric delta
                        (raw current-minus-previous vs the prior run).
                        --json outputs the latest run's delta block (or null).
  baseline <action>     Pinned pre-work snapshots keyed by bug-id or section,
                        stored in baselines.json. Capture up front (before work);
                        compare at close to separate pre-existing failures from
                        regressions the work introduced.
                          capture --key K [--by W] [--force] [--refresh]
                                              Snapshot current test/clippy/
                                              disposition state under key K.
                                              create-if-absent (first capture
                                              wins); --force overwrites;
                                              --refresh runs `refresh --full`
                                              first for a fresh measurement.
                          show --key K        Print baseline K. exit 4 if absent.
                          compare --key K     Diff current state vs baseline K.
                                              exit 0 = no regressions; exit 5 =
                                              regressions (newly-failing tests,
                                              new untracked dispositions, or more
                                              clippy errors); exit 4 = no baseline.
                                              capture + compare exit 6 when the
                                              test_suite cache is degraded (status
                                              not clean/known-failing or null
                                              totals) — refresh first.
                          list                List captured baselines.
                          clear --key K       Remove baseline K.
  refresh               Update the cache.
                        --sha-only          Update head_sha + updated_at only
                                            (fast; no test rerun). Use this
                                            from commit-push post-commit.
                        --full              Run ./test-all.sh + ./clippy-all.sh
                                            + disposition scan. Slow (~3 min).
                        --hygiene-only      Run diagnostics/repo-hygiene.sh
                                            --check and update hygiene block.
                        --dispositions-only Scan compiler/tests/tools/library for
                                            #[ignore]/#skip annotations and update
                                            the test_dispositions block. Sub-second.
                        --from-summary=PATH Ingest a pre-existing test-all.sh JSON
                                            summary into state. Skips the test
                                            run; just parses + writes test_suite +
                                            test_dispositions. Used by test-all.sh's
                                            tail (producer-writes-cache pattern) so
                                            direct ./test-all.sh runs keep state
                                            current without going through --full.
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
  state.sh refresh --dispositions-only --by test-all
  state.sh known-failing --json | jq '.[]' | wc -l
  state.sh dispositions --untracked-only       # pull human-readable drift list
  state.sh dispositions --json | jq '.[] | select(.tracking_bug == null)'

See also:
  .claude/skills/improve-tooling/script-state-design.md — design log
  .claude/state/known-state.json — the cache file (schema v2)
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

    # v3 fields
    local schema_ver failures_status
    schema_ver=$(jq -r '.schema_version // 2' "$STATE_FILE")
    if [[ "$schema_ver" -ge 3 ]]; then
        failures_status=$(jq -r '.test_suite.failures_status // "unavailable"' "$STATE_FILE")
        local failure_count attributed_count
        failure_count=$(jq -r '.test_suite.failures | length' "$STATE_FILE")
        attributed_count=$(jq -r '[.test_suite.failures[]? | select(.attributed_bug != null)] | length' "$STATE_FILE")
        echo "Failures status:   $failures_status  ($failure_count total, $attributed_count attributed)"
        local per_suite_count
        per_suite_count=$(jq -r '.test_suite.per_suite | keys | length' "$STATE_FILE" 2>/dev/null || echo 0)
        if [[ "$per_suite_count" -gt 0 ]]; then
            echo
            printf "  %-30s %6s %6s %6s\n" "Suite" "Pass" "Fail" "Skip"
            printf "  %-30s %6s %6s %6s\n" "------------------------------" "------" "------" "------"
            jq -r '.test_suite.per_suite | to_entries[] | "  \(.value.display_name // .key)\t\(.value.passed // 0)\t\(.value.failed // 0)\t\(.value.skipped // 0)"' "$STATE_FILE" 2>/dev/null \
                | while IFS=$'\t' read -r name p f s; do
                    printf "  %-30s %6s %6s %6s\n" "$name" "$p" "$f" "$s"
                done
        fi
    fi
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
    echo

    echo "--- Test dispositions (#[ignore], #skip) ---"
    local disp_status disp_total disp_untracked disp_ignore disp_skip
    disp_status=$(jq -r '.test_dispositions.status // "unknown"' "$STATE_FILE")
    disp_total=$(jq -r '.test_dispositions.totals.total // 0' "$STATE_FILE")
    disp_untracked=$(jq -r '.test_dispositions.totals.untracked // 0' "$STATE_FILE")
    disp_ignore=$(jq -r '.test_dispositions.totals.ignore // 0' "$STATE_FILE")
    disp_skip=$(jq -r '.test_dispositions.totals.skip // 0' "$STATE_FILE")
    echo "Status:            $disp_status"
    echo "Totals:            total=$disp_total  ignore=$disp_ignore  skip=$disp_skip  untracked=$disp_untracked"
    if [[ "$disp_untracked" != "0" && "$disp_untracked" != "null" ]]; then
        echo "DRIFT:             $disp_untracked annotation(s) lack BUG-XX-NNN tracking. Run:"
        echo "                     state.sh dispositions --untracked-only"
    fi

    if [[ -f "$LEDGER_FILE" ]]; then
        local ledger_runs
        ledger_runs=$(jq -r '.runs | length' "$LEDGER_FILE" 2>/dev/null || echo 0)
        if [[ "$ledger_runs" != "0" && "$ledger_runs" != "null" ]]; then
            echo ""
            echo "=== Metrics Ledger (test-all) ==="
            cmd_ledger
        fi
    fi
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

    # --stale mode: extended freshness check for test-suite staleness
    if [[ "$CHECK_STALE" == "1" ]]; then
        local stale_reasons=()
        local exit_code=0

        # Check 1: test_suite never populated
        local ts_status
        ts_status=$(jq -r '.test_suite.status // "unknown"' "$STATE_FILE")
        if [[ "$ts_status" == "unknown" ]]; then
            stale_reasons+=("test_suite never populated (status: unknown)")
            exit_code=4
        fi

        # Check 2: SHA mismatch
        if [[ "$head_sha" != "$current_sha" ]]; then
            local commits_behind
            commits_behind=$(git -C "$ROOT_DIR" rev-list --count "$head_sha..$current_sha" 2>/dev/null || echo "?")
            if [[ "$commits_behind" -ge 10 ]] || [[ "$commits_behind" == "?" ]]; then
                stale_reasons+=("HEAD $commits_behind commits ahead of cache SHA $head_sha")
                exit_code=4
            fi
        fi

        # Check 3: hours since last refresh
        local last_run_at now_epoch last_epoch hours_since
        last_run_at=$(jq -r '.test_suite.last_run_at // ""' "$STATE_FILE")
        if [[ -n "$last_run_at" && "$last_run_at" != "null" ]]; then
            now_epoch=$(date -u +%s)
            last_epoch=$(date -u -d "$last_run_at" +%s 2>/dev/null || echo 0)
            hours_since=$(( (now_epoch - last_epoch) / 3600 ))
            if [[ "$hours_since" -ge 24 ]]; then
                stale_reasons+=("last refresh was $hours_since hours ago (>= 24)")
                exit_code=4
            fi
        fi

        # Check 4: source files modified since last refresh
        local last_run_sha
        last_run_sha=$(jq -r '.test_suite.last_run_sha // ""' "$STATE_FILE")
        if [[ -n "$last_run_sha" && "$last_run_sha" != "null" && "$last_run_sha" != "" ]]; then
            local changed_files
            changed_files=$(git -C "$ROOT_DIR" diff --name-only "$last_run_sha..HEAD" -- \
                compiler/ library/ tests/ diagnostics/state.sh 2>/dev/null || true)
            if [[ -n "$changed_files" ]]; then
                local changed_count
                changed_count=$(echo "$changed_files" | grep -c .)
                stale_reasons+=("$changed_count compiler/library/test file(s) modified since last refresh at $last_run_sha")
                exit_code=4
            fi
        fi

        if [[ "$OUTPUT" == "json" ]]; then
            local reasons_json
            reasons_json=$(printf '%s\n' "${stale_reasons[@]}" | jq -R -s 'split("\n") | map(select(length > 0))')
            printf '{"status":"%s","reasons":%s}\n' "$([[ $exit_code -eq 0 ]] && echo "fresh" || echo "stale")" "$reasons_json"
        else
            if [[ $exit_code -eq 0 ]]; then
                echo "STALE CHECK: FRESH — test_suite is up to date"
            else
                echo "STALE CHECK: STALE — ${#stale_reasons[@]} reason(s):"
                for reason in "${stale_reasons[@]}"; do
                    echo "  - $reason"
                done
                echo "  Run: diagnostics/state.sh refresh --full --by section-close"
            fi
        fi
        exit $exit_code
    fi

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

# ---- Subcommand: dispositions ------------------------------------------------
cmd_dispositions() {
    require_state_file
    require_jq
    local filter='.test_dispositions.entries // []'
    if [[ "$DISPOSITIONS_UNTRACKED_ONLY" == "1" ]]; then
        filter="${filter} | map(select(.tracking_bug == null))"
    fi
    if [[ "$OUTPUT" == "json" ]]; then
        jq "$filter" "$STATE_FILE"
    else
        jq -r "$filter | .[] | \"\(.file):\(.line)\t\(.kind)\t\(.tracking_bug // \"<UNTRACKED>\")\t\(.reason)\"" "$STATE_FILE"
    fi
}

# ---- Subcommand: ledger ------------------------------------------------------
# Emit the latest test-all metrics-ledger run's delta block (raw current-minus-
# previous vs the prior run). Reads the ledger data file written by the
# metrics-ledger runtime; graceful-degrade when absent (no runs yet).
cmd_ledger() {
    if [[ ! -f "$LEDGER_FILE" ]]; then
        if [[ "$OUTPUT" == "json" ]]; then
            echo 'null'
        else
            echo "Metrics ledger:    (no ledger yet — run the metrics-ledger ingest)"
        fi
        return 0
    fi
    require_jq
    local latest='.runs[-1] | {run_id, vs_run_id, overall, metrics, deltas}'
    if [[ "$OUTPUT" == "json" ]]; then
        jq "$latest" "$LEDGER_FILE"
    else
        local empty
        empty=$(jq -r '.runs | length' "$LEDGER_FILE" 2>/dev/null || echo 0)
        if [[ "$empty" == "0" ]]; then
            echo "Metrics ledger:    (empty — no runs recorded)"
            return 0
        fi
        jq -r '.runs[-1] | "Latest run:        \(.run_id) (vs \(.vs_run_id // "none"))",
            "Delta:             passed:\(.deltas.passed) failed:\(.deltas.failed) skipped:\(.deltas.skipped) lcfail:\(.deltas.lcfail) aot_leaks:\(.deltas.aot_leaks)"' "$LEDGER_FILE"
    fi
}

# ---- Skeleton seed -----------------------------------------------------------
# Write a minimal schema-v3 state file with every content block marked
# status: "unknown" and the given head_sha / updated_at / updated_by. Every
# refresh mode layers its real values on top — the sha-only path leaves
# test_suite / clippy / hygiene at "unknown" (honest: the cache really
# doesn't know yet), hygiene-only overwrites the hygiene block, --full
# overwrites test_suite + clippy. Consumers already fail-safe on
# status != "clean", so the seeded file reads as "nothing trusted yet"
# until a --full or explicit mode populates fields.
seed_skeleton_state() {
    local sha="$1" at="$2" by="$3"
    mkdir -p "$STATE_DIR"
    write_state "$(cat <<EOF
{
  "schema_version": 3,
  "head_sha": "$sha",
  "updated_at": "$at",
  "updated_by": "$by",
  "notes": "Seeded by state.sh first-run bootstrap. Run refresh --full to populate test_suite + clippy + test_dispositions with real values.",
  "test_suite": {
    "status": "unknown",
    "last_run_sha": "",
    "last_run_at": "",
    "last_run_kind": "",
    "totals": { "passed": 0, "failed": 0, "skipped": 0 },
    "known_failing_files": [],
    "known_failing_count": 0,
    "failure_class": "",
    "remediation": [],
    "failures": [],
    "failures_status": "unavailable",
    "per_suite": {}
  },
  "clippy": {
    "status": "unknown",
    "last_run_sha": "",
    "last_run_at": ""
  },
  "hygiene": {
    "status": "unknown",
    "last_run_sha": "",
    "last_run_at": "",
    "notes": ""
  },
  "test_dispositions": {
    "status": "unknown",
    "scanned_at_sha": "",
    "scanned_at": "",
    "totals": { "ignore": 0, "skip": 0, "total": 0, "untracked": 0 },
    "entries": []
  }
}
EOF
)"
}

# ---- Disposition scan --------------------------------------------------------
# Scans compiler/, tests/, tools/, library/ for #[ignore] and #skip annotations.
# Emits TSV: <file-relative-to-ROOT_DIR>\t<line>\t<kind>\t<reason>
# Reason text is the captured string inside the annotation (empty if none).
# Tracking-bug extraction (BUG-XX-NNN regex) happens in jq during JSON build —
# this keeps the bash side simple and the contract uniform across consumers.
scan_dispositions_tsv() {
    # Rust #[ignore] — matches:
    #   #[ignore]
    #   #[ignore = "reason text"]
    #   #[ignore(note = "...")]   (rare; reason field captured if present)
    if [[ -d "$ROOT_DIR/compiler" || -d "$ROOT_DIR/tests" || -d "$ROOT_DIR/tools" ]]; then
        grep -rEn '^[[:space:]]*#\[ignore([[:space:]]*=[[:space:]]*"[^"]*")?[[:space:]]*\]' \
            "$ROOT_DIR/compiler" "$ROOT_DIR/tests" "$ROOT_DIR/tools" \
            --include='*.rs' 2>/dev/null \
        | while IFS= read -r match; do
            local file line content reason=""
            file="${match%%:*}"
            local rest="${match#*:}"
            line="${rest%%:*}"
            content="${rest#*:}"
            if [[ "$content" =~ \#\[ignore[[:space:]]*=[[:space:]]*\"([^\"]*)\" ]]; then
                reason="${BASH_REMATCH[1]}"
            fi
            # Strip ROOT_DIR prefix for stable relative paths
            local rel="${file#$ROOT_DIR/}"
            printf '%s\t%s\t%s\t%s\n' "$rel" "$line" '#[ignore]' "$reason"
        done
    fi

    # Ori #skip("reason") — Ori test annotation
    if [[ -d "$ROOT_DIR/tests" || -d "$ROOT_DIR/library" ]]; then
        grep -rEn '#skip\("[^"]*"\)' \
            "$ROOT_DIR/tests" "$ROOT_DIR/library" \
            --include='*.ori' 2>/dev/null \
        | while IFS= read -r match; do
            local file line content reason=""
            file="${match%%:*}"
            local rest="${match#*:}"
            line="${rest%%:*}"
            content="${rest#*:}"
            if [[ "$content" =~ \#skip\(\"([^\"]*)\"\) ]]; then
                reason="${BASH_REMATCH[1]}"
            fi
            local rel="${file#$ROOT_DIR/}"
            printf '%s\t%s\t%s\t%s\n' "$rel" "$line" '#skip' "$reason"
        done
    fi
}

# Build JSON test_dispositions block from the TSV. Stdout = JSON object.
build_dispositions_block() {
    local sha="$1" at="$2"
    local tsv
    tsv=$(scan_dispositions_tsv)

    # jq: split TSV into rows, parse each into entry object, extract BUG-ID
    # from reason via capture, compute totals, set status.
    printf '%s' "$tsv" | jq -R -s --arg sha "$sha" --arg at "$at" '
        def entries:
            split("\n")
            | map(select(length > 0))
            | map(split("\t") | {
                file: .[0],
                line: (.[1] | tonumber),
                kind: .[2],
                reason: .[3],
                tracking_bug: (
                    if (.[3] | test("BUG-[A-Z0-9]+-[0-9]+"))
                    then (.[3] | capture("(?<id>BUG-[A-Z0-9]+-[0-9]+)") | .id)
                    else null
                    end
                )
            });
        entries as $es
        | ($es | length) as $total
        | ($es | map(select(.kind == "#[ignore]")) | length) as $ignore
        | ($es | map(select(.kind == "#skip")) | length) as $skip
        | ($es | map(select(.tracking_bug == null)) | length) as $untracked
        | {
            status: (if $untracked == 0 then "clean" else "drift" end),
            scanned_at_sha: $sha,
            scanned_at: $at,
            totals: { ignore: $ignore, skip: $skip, total: $total, untracked: $untracked },
            entries: $es
        }
    '
}

# ---- Subcommand: refresh -----------------------------------------------------
cmd_refresh() {
    require_jq

    # Single-flight: serialize concurrent refreshes so the jq-read -> write_state
    # read-modify-write in each mode below cannot lost-update (one mode's write
    # clobbering a sibling mode's just-written block). Every refresh funnels
    # through this subcommand, so one lock serializes all writers. flock-unavailable
    # degrades to per-write-atomic last-writer-wins (write_state's rename stays
    # atomic); known-state.json is a self-healing cache, so a dropped block is
    # rebuilt on the next refresh. Lock fd is held until process exit (one-shot
    # subcommand), covering the whole read-modify-write.
    mkdir -p "$STATE_DIR"
    if command -v flock >/dev/null 2>&1; then
        exec {STATE_LOCK_FD}>"$STATE_FILE.lock"
        flock "$STATE_LOCK_FD"
    fi

    local current_sha updated_at updated_by_val
    current_sha=$(current_head_sha)
    updated_at=$(iso_now)
    updated_by_val="${UPDATED_BY:-manual}"

    # First-run bootstrap: every mode seeds a skeleton with status:unknown so
    # the normal per-mode jq update below finds a valid file. Invariant S1
    # (design log §2) was amended 2026-04-20 to permit this on the grounds
    # that fail-safe semantics come from per-block status fields, not from
    # the file's existence. See script-state-design.md §6 entry.
    local seeded=0
    if [[ ! -f "$STATE_FILE" ]]; then
        seed_skeleton_state "$current_sha" "$updated_at" "$updated_by_val (auto-seed)"
        seeded=1
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
            local seeded_bool=false
            local seed_tag=""
            if [[ $seeded -eq 1 ]]; then
                seeded_bool=true
                seed_tag=" (seeded; run refresh --full to populate test_suite + clippy)"
            fi
            if [[ "$OUTPUT" == "json" ]]; then
                printf '{"status":"refreshed","mode":"sha-only","head_sha":"%s","seeded":%s}\n' "$current_sha" "$seeded_bool"
            else
                echo "state refreshed (sha-only): head_sha=$current_sha updated_by=$updated_by_val$seed_tag"
            fi
            ;;
        dispositions-only)
            local disp_block tmp
            disp_block=$(build_dispositions_block "$current_sha" "$updated_at")
            tmp=$(jq --arg sha "$current_sha" \
                    --arg at "$updated_at" \
                    --arg by "$updated_by_val" \
                    --argjson disp "$disp_block" \
                    '.schema_version = 2
                     | .head_sha = $sha | .updated_at = $at | .updated_by = $by
                     | .test_dispositions = $disp' \
                    "$STATE_FILE")
            write_state "$tmp"
            local untracked total
            untracked=$(printf '%s' "$disp_block" | jq -r '.totals.untracked')
            total=$(printf '%s' "$disp_block" | jq -r '.totals.total')
            if [[ "$OUTPUT" == "json" ]]; then
                printf '{"status":"refreshed","mode":"dispositions-only","total":%s,"untracked":%s}\n' "$total" "$untracked"
            else
                echo "dispositions block refreshed: total=$total untracked=$untracked"
            fi
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
            if [[ "$OUTPUT" == "json" ]]; then
                printf '{"status":"refreshed","mode":"hygiene-only","hygiene_status":"%s"}\n' "$hygiene_status"
            else
                echo "hygiene block refreshed: status=$hygiene_status"
            fi
            ;;
        full)
            # Composite-harness timeouts. CLAUDE.md §MANDATORY TEST TIMEOUTS pins
            # individual test commands at 150s; that rule was authored for leaf
            # invocations (cargo t, cargo st), NOT for ./test-all.sh + ./clippy-all.sh
            # which orchestrate dozens of leaf commands. Documented runtime is
            # ~3 minutes; cap at 600s/300s gives headroom for slow CI hosts
            # without masking genuine hangs.
            local test_timeout=600
            local clippy_timeout=300
            echo "Running ./test-all.sh + ./clippy-all.sh (typically ~3 minutes; cap=${test_timeout}s+${clippy_timeout}s)..." >&2
            local test_log clippy_log test_status clippy_status summary_json
            test_log="$ROOT_DIR/build/state-refresh-test.log"
            clippy_log="$ROOT_DIR/build/state-refresh-clippy.log"
            summary_json="$ROOT_DIR/build/state-refresh-summary.json"
            mkdir -p "$ROOT_DIR/build"

            local test_exit=0
            timeout "$test_timeout" "$ROOT_DIR/test-all.sh" --json-summary="$summary_json" > "$test_log" 2>&1 || test_exit=$?
            if [[ "$test_exit" -eq 0 ]]; then
                test_status="clean"
            elif [[ "$test_exit" -eq 124 ]]; then
                test_status="timeout"
                echo "WARN: ./test-all.sh exceeded ${test_timeout}s cap; test_suite block will be marked timeout" >&2
            else
                test_status="known-failing"
            fi
            local clippy_exit=0
            timeout "$clippy_timeout" "$ROOT_DIR/clippy-all.sh" > "$clippy_log" 2>&1 || clippy_exit=$?
            if [[ "$clippy_exit" -eq 0 ]]; then
                clippy_status="clean"
            elif [[ "$clippy_exit" -eq 124 ]]; then
                clippy_status="timeout"
                echo "WARN: ./clippy-all.sh exceeded ${clippy_timeout}s cap; clippy block will be marked timeout" >&2
            else
                clippy_status="warnings"
            fi

            local passed failed skipped failures_json per_suite_json failures_status
            passed=0; failed=0; skipped=0
            failures_json='[]'
            per_suite_json='{}'
            failures_status="unavailable"

            if [[ -f "$summary_json" ]]; then
                # Validate the summary file parses cleanly before consuming it.
                # If test-all.sh got SIGKILL'd mid-write (timeout) or wrote
                # malformed JSON, downstream jq passes silently produce empty
                # values, leaving test_suite.last_run_sha unchanged so callers
                # cannot distinguish "not yet refreshed" from "refresh failed".
                if ! jq empty "$summary_json" >/dev/null 2>&1; then
                    test_status="parse-error"
                    echo "WARN: $summary_json failed to parse (likely truncated by timeout or malformed by test-all.sh); test_suite block will be marked parse-error" >&2
                else
                    passed=$(jq -r '.totals.passed // 0' "$summary_json")
                    failed=$(jq -r '.totals.failed // 0' "$summary_json")
                    skipped=$(jq -r '.totals.skipped // 0' "$summary_json")
                    failures_json=$(jq -r '.failures // []' "$summary_json")
                    per_suite_json=$(jq -r '.per_suite // {}' "$summary_json")
                    if jq -e '.failures' "$summary_json" >/dev/null 2>&1; then
                        failures_status="complete"
                    else
                        failures_status="partial"
                    fi
                fi
            else
                test_status="missing-summary"
                echo "WARN: $summary_json absent after test-all.sh run; test_suite block will be marked missing-summary" >&2
            fi

            # Attribution: cross-reference failures with dispositions (tier 1).
            # For each failure, check if the test path matches a disposition entry.
            local disp_block
            disp_block=$(build_dispositions_block "$current_sha" "$updated_at")
            local attributed_failures_json
            attributed_failures_json=$(printf '%s' "$failures_json" | jq --argjson disp "$disp_block" '
                def disposition_match($test_id):
                    ($disp.entries // []) as $entries
                    | ([$entries[] | select(.file != null and ($test_id | contains(.file))) | .tracking_bug] | first // null);
                map(
                    if .attributed_bug == null then
                        (.attributed_bug = disposition_match(.test_id))
                        | if .attributed_bug != null then
                            .attribution_source = "disposition"
                            | .attribution_confidence = "high"
                          else . end
                    else . end
                )
            ' 2>/dev/null || printf '%s' "$failures_json")

            disp_block=$(build_dispositions_block "$current_sha" "$updated_at")
            # Failure-mode statuses MUST NOT bump test_suite.last_run_sha.
            # Status callers (status-report Step 0.5, /commit-push Step 8) verify
            # `test_suite.last_run_sha == HEAD_SHA` to confirm a successful refresh;
            # bumping the SHA on parse-error/missing-summary/timeout would mask the
            # failure as success. Status field still updates so observers see WHY.
            local test_suite_update
            case "$test_status" in
                clean|known-failing)
                    test_suite_update='.test_suite.status = $tstatus
                                     | .test_suite.last_run_sha = $sha
                                     | .test_suite.last_run_at = $at
                                     | .test_suite.last_run_kind = "test-all.sh"
                                     | .test_suite.totals.passed = $passed
                                     | .test_suite.totals.failed = $failed
                                     | .test_suite.totals.skipped = $skipped
                                     | .test_suite.failures = $afailures
                                     | .test_suite.failures_status = $fstatus
                                     | .test_suite.per_suite = $per_suite'
                    ;;
                *)
                    # parse-error | missing-summary | timeout — record status +
                    # attempt timestamp; preserve prior last_run_sha + counts.
                    test_suite_update='.test_suite.status = $tstatus
                                     | .test_suite.last_attempt_at = $at
                                     | .test_suite.last_attempt_outcome = $tstatus'
                    ;;
            esac
            local tmp
            tmp=$(jq --arg sha "$current_sha" \
                    --arg at "$updated_at" \
                    --arg by "$updated_by_val" \
                    --arg tstatus "$test_status" \
                    --argjson passed "$passed" \
                    --argjson failed "$failed" \
                    --argjson skipped "$skipped" \
                    --arg cstatus "$clippy_status" \
                    --argjson disp "$disp_block" \
                    --argjson afailures "$attributed_failures_json" \
                    --argjson per_suite "$per_suite_json" \
                    --arg fstatus "$failures_status" \
                    ".schema_version = 3
                     | .head_sha = \$sha | .updated_at = \$at | .updated_by = \$by
                     | $test_suite_update
                     | .clippy.status = \$cstatus
                     | .clippy.last_run_sha = \$sha
                     | .clippy.last_run_at = \$at
                     | .test_dispositions = \$disp" \
                    "$STATE_FILE")
            write_state "$tmp"
            local d_total d_untracked
            d_total=$(printf '%s' "$disp_block" | jq -r '.totals.total')
            d_untracked=$(printf '%s' "$disp_block" | jq -r '.totals.untracked')
            local attributed_count
            attributed_count=$(printf '%s' "$attributed_failures_json" | jq -r '[.[] | select(.attributed_bug != null)] | length' 2>/dev/null || echo 0)
            local total_failures_count
            total_failures_count=$(printf '%s' "$attributed_failures_json" | jq -r 'length' 2>/dev/null || echo 0)
            if [[ "$OUTPUT" == "json" ]]; then
                printf '{"status":"refreshed","mode":"full","test_status":"%s","clippy_status":"%s","passed":%s,"failed":%s,"failures_status":"%s","failures_total":%s,"failures_attributed":%s,"dispositions_total":%s,"dispositions_untracked":%s}\n' "$test_status" "$clippy_status" "$passed" "$failed" "$failures_status" "$total_failures_count" "$attributed_count" "$d_total" "$d_untracked"
            else
                echo "full refresh complete: tests=$test_status clippy=$clippy_status totals=$passed/$failed/$skipped failures=$total_failures_count/$attributed_count-attributed dispositions=$d_total/$d_untracked-untracked"
            fi
            ;;
        from-summary)
            # Ingest mode: caller (typically test-all.sh's tail) already ran the
            # tests and produced a summary JSON. Parse it, write test_suite +
            # dispositions, leave clippy block alone (no clippy data here).
            #
            # Producer-writes-cache pattern: the entity with first-hand data
            # populates state, instead of state.sh re-orchestrating the run.
            # See script-state-design.md §6 2026-05-06 ingest-mode entry.
            [[ -n "$FROM_SUMMARY_PATH" ]] || die "--from-summary requires a path"
            local summary_json="$FROM_SUMMARY_PATH"
            local test_status passed failed skipped failures_json per_suite_json failures_status
            passed=0; failed=0; skipped=0
            failures_json='[]'
            per_suite_json='{}'
            failures_status="unavailable"

            if [[ ! -f "$summary_json" ]]; then
                test_status="missing-summary"
                echo "WARN: $summary_json absent; test_suite block will be marked missing-summary" >&2
            elif ! jq empty "$summary_json" >/dev/null 2>&1; then
                test_status="parse-error"
                echo "WARN: $summary_json failed to parse; test_suite block will be marked parse-error" >&2
            else
                passed=$(jq -r '.totals.passed // 0' "$summary_json")
                failed=$(jq -r '.totals.failed // 0' "$summary_json")
                skipped=$(jq -r '.totals.skipped // 0' "$summary_json")
                failures_json=$(jq -r '.failures // []' "$summary_json")
                per_suite_json=$(jq -r '.per_suite // {}' "$summary_json")
                if jq -e '.failures' "$summary_json" >/dev/null 2>&1; then
                    failures_status="complete"
                else
                    failures_status="partial"
                fi
                # totals.failed only counts PARSED test failures; a suite that
                # build-failed or crashed before printing results contributes 0
                # there but carries status "errored"/"failed" in per_suite. Key
                # the verdict off both so an errored suite is never recorded
                # "clean". The known LLVM-backend crash ("crashed"/"build_failed")
                # stays exempt.
                local broken_suites
                broken_suites=$(jq -r '[.per_suite[]? | select(.status == "errored" or .status == "failed")] | length' "$summary_json" 2>/dev/null || echo 0)
                if [[ "$failed" -gt 0 || "${broken_suites:-0}" -gt 0 ]]; then
                    test_status="known-failing"
                else
                    test_status="clean"
                fi
            fi

            # Disposition scan + attribution (same flow as --full).
            local disp_block
            disp_block=$(build_dispositions_block "$current_sha" "$updated_at")
            local attributed_failures_json
            attributed_failures_json=$(printf '%s' "$failures_json" | jq --argjson disp "$disp_block" '
                def disposition_match($test_id):
                    ($disp.entries // []) as $entries
                    | ([$entries[] | select(.file != null and ($test_id | contains(.file))) | .tracking_bug] | first // null);
                map(
                    if .attributed_bug == null then
                        (.attributed_bug = disposition_match(.test_id))
                        | if .attributed_bug != null then
                            .attribution_source = "disposition"
                            | .attribution_confidence = "high"
                          else . end
                    else . end
                )
            ' 2>/dev/null || printf '%s' "$failures_json")

            # Branched writeback: success bumps last_run_sha; failure modes
            # preserve prior SHA so callers gating on SHA equality see the
            # failure correctly. Mirrors the --full writeback contract.
            local test_suite_update
            case "$test_status" in
                clean|known-failing)
                    test_suite_update='.test_suite.status = $tstatus
                                     | .test_suite.last_run_sha = $sha
                                     | .test_suite.last_run_at = $at
                                     | .test_suite.last_run_kind = "test-all.sh"
                                     | .test_suite.totals.passed = $passed
                                     | .test_suite.totals.failed = $failed
                                     | .test_suite.totals.skipped = $skipped
                                     | .test_suite.failures = $afailures
                                     | .test_suite.failures_status = $fstatus
                                     | .test_suite.per_suite = $per_suite'
                    ;;
                *)
                    test_suite_update='.test_suite.status = $tstatus
                                     | .test_suite.last_attempt_at = $at
                                     | .test_suite.last_attempt_outcome = $tstatus'
                    ;;
            esac
            local tmp
            tmp=$(jq --arg sha "$current_sha" \
                    --arg at "$updated_at" \
                    --arg by "$updated_by_val" \
                    --arg tstatus "$test_status" \
                    --argjson passed "$passed" \
                    --argjson failed "$failed" \
                    --argjson skipped "$skipped" \
                    --argjson disp "$disp_block" \
                    --argjson afailures "$attributed_failures_json" \
                    --argjson per_suite "$per_suite_json" \
                    --arg fstatus "$failures_status" \
                    ".schema_version = 3
                     | .head_sha = \$sha | .updated_at = \$at | .updated_by = \$by
                     | $test_suite_update
                     | .test_dispositions = \$disp" \
                    "$STATE_FILE")
            write_state "$tmp"
            local d_total d_untracked attributed_count total_failures_count
            d_total=$(printf '%s' "$disp_block" | jq -r '.totals.total')
            d_untracked=$(printf '%s' "$disp_block" | jq -r '.totals.untracked')
            attributed_count=$(printf '%s' "$attributed_failures_json" | jq -r '[.[] | select(.attributed_bug != null)] | length' 2>/dev/null || echo 0)
            total_failures_count=$(printf '%s' "$attributed_failures_json" | jq -r 'length' 2>/dev/null || echo 0)
            if [[ "$OUTPUT" == "json" ]]; then
                printf '{"status":"refreshed","mode":"from-summary","test_status":"%s","passed":%s,"failed":%s,"failures_status":"%s","failures_total":%s,"failures_attributed":%s,"dispositions_total":%s,"dispositions_untracked":%s}\n' "$test_status" "$passed" "$failed" "$failures_status" "$total_failures_count" "$attributed_count" "$d_total" "$d_untracked"
            else
                echo "from-summary ingest complete: tests=$test_status totals=$passed/$failed/$skipped failures=$total_failures_count/$attributed_count-attributed dispositions=$d_total/$d_untracked-untracked"
            fi
            ;;
        *)
            die "unknown refresh mode: $REFRESH_MODE. Use --sha-only, --hygiene-only, --full, --dispositions-only, or --from-summary."
            ;;
    esac
}

# ---- Subcommand: baseline ----------------------------------------------------
# Pinned pre-work snapshots of test/clippy/disposition state, keyed by a bug id
# or plan-section key, stored in baselines.json. Captured up front (before work)
# so close-out can diff current-vs-baseline and distinguish pre-existing failures
# from regressions the work introduced.

ensure_baselines_file() {
    if [[ ! -f "$BASELINES_FILE" ]]; then
        mkdir -p "$(dirname "$BASELINES_FILE")"
        printf '%s\n' '{"schema_version":1,"baselines":{}}' > "$BASELINES_FILE"
    fi
}

# Atomic JSON write for the baselines store: write to .tmp, then rename.
write_baselines() {
    local content="$1"
    local tmp="$BASELINES_FILE.tmp.$$"
    printf '%s\n' "$content" > "$tmp"
    mv "$tmp" "$BASELINES_FILE"
}

# Refuse baseline work on a degraded test_suite cache: status outside
# {clean, known-failing} or null totals means the cache holds no real
# measurement — `// 0` coercion would fabricate a clean snapshot and the
# later compare would flag every pre-tracked failure as REGRESSION.
# Exit 6 = degraded-state refusal (0 ok, 4 absent, 5 regression).
require_measured_test_suite() {
    local status passed
    status=$(jq -r '.test_suite.status // "unknown"' "$STATE_FILE")
    passed=$(jq -r '.test_suite.totals.passed' "$STATE_FILE")
    if [[ "$status" != "clean" && "$status" != "known-failing" ]] \
        || [[ "$passed" == "null" ]]; then
        echo "error: test_suite cache is degraded (status=$status, passed=$passed) — no real measurement to baseline. Run: state.sh refresh --full (or baseline capture --refresh)" >&2
        exit 6
    fi
}

# Normalized snapshot object extracted from known-state.json.
baseline_snapshot_from_state() {
    require_state_file
    jq '{
      test_suite: {
        totals: {
          passed: (.test_suite.totals.passed // 0),
          failed: (.test_suite.totals.failed // 0),
          skipped: (.test_suite.totals.skipped // 0)
        },
        failures: ([.test_suite.failures[]?.test_id] | map(select(. != null)) | unique),
        known_failing_count: (.test_suite.known_failing_count // 0),
        status: (.test_suite.status // "unknown")
      },
      clippy: {
        status: (.clippy.status // "unknown"),
        warnings: (.clippy.warnings // 0),
        errors: (.clippy.errors // 0)
      },
      test_dispositions: {
        total: (.test_dispositions.totals.total // 0),
        untracked: (.test_dispositions.totals.untracked // 0)
      }
    }' "$STATE_FILE"
}

baseline_capture() {
    [[ -n "$BASELINE_KEY" ]] || die "baseline capture requires --key"
    require_state_file
    ensure_baselines_file
    local exists
    exists=$(jq --arg k "$BASELINE_KEY" '.baselines | has($k)' "$BASELINES_FILE")
    if [[ "$exists" == "true" && "$BASELINE_FORCE" != "1" ]]; then
        # create-if-absent: the FIRST (pre-work) capture wins; re-handoff never
        # clobbers it. --force overwrites deliberately.
        if [[ "$OUTPUT" == "json" ]]; then
            jq -n --arg k "$BASELINE_KEY" \
                '{action:"no-op",key:$k,reason:"baseline exists (use --force to overwrite)"}'
        else
            echo "baseline '$BASELINE_KEY' already exists; preserved (use --force to re-capture)"
        fi
        return 0
    fi
    if [[ "$BASELINE_REFRESH" == "1" ]]; then
        REFRESH_MODE=full
        cmd_refresh >/dev/null 2>&1 || true
    elif [[ "$CHECK_STALE" == "0" ]]; then
        # Best-effort freshness warning (non-fatal): a stale cache means the
        # baseline reflects an old measurement, not true work-start state.
        local cache_sha head_sha
        cache_sha=$(jq -r '.head_sha // ""' "$STATE_FILE")
        head_sha=$(current_head_sha)
        if [[ -n "$cache_sha" && "$cache_sha" != "$head_sha" ]]; then
            echo "warning: known-state cache SHA ($cache_sha) != HEAD ($head_sha); baseline may be stale (use --refresh for a fresh measurement)" >&2
        fi
    fi
    require_measured_test_suite
    local snapshot captured_sha captured_at by entry merged
    snapshot=$(baseline_snapshot_from_state)
    captured_sha=$(jq -r '.test_suite.last_run_sha // .head_sha // "unknown"' "$STATE_FILE")
    captured_at=$(iso_now)
    by="${UPDATED_BY:-manual}"
    entry=$(jq -n \
        --arg k "$BASELINE_KEY" --arg at "$captured_at" --arg sha "$captured_sha" --arg by "$by" \
        --argjson snap "$snapshot" \
        '{key:$k, captured_at:$at, captured_at_sha:$sha, captured_by:$by} + $snap')
    merged=$(jq --arg k "$BASELINE_KEY" --argjson e "$entry" '.baselines[$k] = $e' "$BASELINES_FILE")
    write_baselines "$merged"
    if [[ "$OUTPUT" == "json" ]]; then
        echo "$entry" | jq '{action:"captured"} + .'
    else
        echo "baseline '$BASELINE_KEY' captured @ $captured_sha (by $by)"
        echo "$snapshot" | jq -r '"  tests: failed=\(.test_suite.totals.failed) known_failing=\(.test_suite.known_failing_count)  dispositions: untracked=\(.test_dispositions.untracked)  clippy: errors=\(.clippy.errors)"'
    fi
}

baseline_show() {
    [[ -n "$BASELINE_KEY" ]] || die "baseline show requires --key"
    ensure_baselines_file
    local entry
    entry=$(jq -c --arg k "$BASELINE_KEY" '.baselines[$k] // empty' "$BASELINES_FILE")
    if [[ -z "$entry" ]]; then
        if [[ "$OUTPUT" == "json" ]]; then
            jq -n --arg k "$BASELINE_KEY" '{action:"absent",key:$k}'
        else
            echo "no baseline for key '$BASELINE_KEY'"
        fi
        exit 4
    fi
    if [[ "$OUTPUT" == "json" ]]; then
        echo "$entry" | jq .
    else
        echo "$entry" | jq -r '"baseline \(.key) @ \(.captured_at_sha) (\(.captured_at), by \(.captured_by))\n  tests: failed=\(.test_suite.totals.failed) known_failing=\(.test_suite.known_failing_count)\n  dispositions: untracked=\(.test_dispositions.untracked)\n  clippy: errors=\(.clippy.errors) warnings=\(.clippy.warnings)"'
    fi
}

baseline_compare() {
    [[ -n "$BASELINE_KEY" ]] || die "baseline compare requires --key"
    require_state_file
    require_measured_test_suite
    ensure_baselines_file
    local base
    base=$(jq -c --arg k "$BASELINE_KEY" '.baselines[$k] // empty' "$BASELINES_FILE")
    if [[ -z "$base" ]]; then
        if [[ "$OUTPUT" == "json" ]]; then
            jq -n --arg k "$BASELINE_KEY" '{action:"absent",key:$k,gate_pass:false,exit_code:4}'
        else
            echo "no baseline for key '$BASELINE_KEY' — cannot compare (capture one at work-start)"
        fi
        exit 4
    fi
    local cur cur_sha report exit_code
    cur=$(baseline_snapshot_from_state)
    cur_sha=$(jq -r '.test_suite.last_run_sha // .head_sha // "unknown"' "$STATE_FILE")
    # Known aims-burden AOT floor (one test-id per line; blank lines dropped).
    # A missing floor file degrades to empty-floor (legacy) behavior.
    local floor
    if [[ -f "$FLOOR_FILE" ]]; then
        floor=$(jq -Rn '[inputs | select(length > 0)]' "$FLOOR_FILE")
    else
        floor='[]'
    fi
    report=$(jq -n --argjson base "$base" --argjson cur "$cur" --argjson floor "$floor" --arg cur_sha "$cur_sha" '
        ((($cur.test_suite.failures - $base.test_suite.failures)) - $floor) as $newfail |
        ((($base.test_suite.failures - $cur.test_suite.failures)) - $floor) as $newfix |
        (($cur.test_dispositions.untracked - $base.test_dispositions.untracked)) as $undelta |
        (($cur.clippy.errors - $base.clippy.errors)) as $clipdelta |
        (($newfail | length) > 0 or $undelta > 0 or $clipdelta > 0) as $reg |
        {
          key: $base.key,
          baseline_sha: $base.captured_at_sha,
          current_sha: $cur_sha,
          newly_failing: $newfail,
          newly_fixed: $newfix,
          totals_delta: {
            passed: ($cur.test_suite.totals.passed - $base.test_suite.totals.passed),
            failed: ($cur.test_suite.totals.failed - $base.test_suite.totals.failed),
            skipped: ($cur.test_suite.totals.skipped - $base.test_suite.totals.skipped)
          },
          dispositions: { untracked_delta: $undelta },
          clippy: { errors_delta: $clipdelta },
          regression: $reg,
          gate_pass: ($reg | not),
          exit_code: (if $reg then 5 else 0 end)
        }')
    exit_code=$(echo "$report" | jq -r '.exit_code')
    if [[ "$OUTPUT" == "json" ]]; then
        echo "$report" | jq .
    else
        echo "$report" | jq -r '"baseline compare \(.key): baseline=\(.baseline_sha) current=\(.current_sha)\n  newly_failing: \(.newly_failing | length)  newly_fixed: \(.newly_fixed | length)  untracked_delta: \(.dispositions.untracked_delta)  clippy_errors_delta: \(.clippy.errors_delta)"'
        if [[ "$(echo "$report" | jq -r '.regression')" == "true" ]]; then
            echo "  REGRESSION vs baseline — newly_failing: $(echo "$report" | jq -r '.newly_failing | join(", ")')"
        else
            echo "  no regressions vs baseline"
        fi
    fi
    exit "$exit_code"
}

baseline_list() {
    ensure_baselines_file
    if [[ "$OUTPUT" == "json" ]]; then
        jq '.baselines | to_entries | map({key:.key, captured_at_sha:.value.captured_at_sha, captured_at:.value.captured_at, captured_by:.value.captured_by})' "$BASELINES_FILE"
    else
        jq -r '.baselines | to_entries[] | "\(.key)\t\(.value.captured_at_sha)\t\(.value.captured_at)\t\(.value.captured_by)"' "$BASELINES_FILE"
    fi
}

baseline_clear() {
    [[ -n "$BASELINE_KEY" ]] || die "baseline clear requires --key"
    ensure_baselines_file
    local exists
    exists=$(jq --arg k "$BASELINE_KEY" '.baselines | has($k)' "$BASELINES_FILE")
    if [[ "$exists" != "true" ]]; then
        echo "no baseline for key '$BASELINE_KEY'"
        exit 4
    fi
    local merged
    merged=$(jq --arg k "$BASELINE_KEY" 'del(.baselines[$k])' "$BASELINES_FILE")
    write_baselines "$merged"
    echo "baseline '$BASELINE_KEY' cleared"
}

cmd_baseline() {
    require_jq
    [[ -n "$BASELINE_ACTION" ]] || die "baseline requires an action: capture|show|compare|list|clear"
    case "$BASELINE_ACTION" in
        capture)  baseline_capture ;;
        show)     baseline_show ;;
        compare)  baseline_compare ;;
        list)     baseline_list ;;
        clear)    baseline_clear ;;
        *) die "unknown baseline action: $BASELINE_ACTION (use capture|show|compare|list|clear)" ;;
    esac
}

# ---- Argument parsing --------------------------------------------------------
if [[ $# -eq 0 ]]; then usage; exit 3; fi

UPDATED_BY=""
DISPOSITIONS_UNTRACKED_ONLY=0
CHECK_STALE=0
BASELINE_KEY=""
BASELINE_ACTION=""
BASELINE_FORCE=0
BASELINE_REFRESH=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --help|-h) usage; exit 0 ;;
        --json) OUTPUT=json; shift ;;
        --human) OUTPUT=human; shift ;;
        --key)
            [[ $# -ge 2 ]] || die "--key requires a value"
            BASELINE_KEY="$2"; shift 2 ;;
        --force) BASELINE_FORCE=1; shift ;;
        --refresh) BASELINE_REFRESH=1; shift ;;
        baseline)
            [[ -z "$SUBCMD" ]] || die "multiple subcommands: $SUBCMD and $1"
            SUBCMD="baseline"; shift
            # `baseline <action>` — consume the sub-action positional together.
            if [[ $# -ge 1 && "$1" != --* ]]; then
                BASELINE_ACTION="$1"; shift
            fi
            ;;
        --sha-only) REFRESH_MODE=sha-only; shift ;;
        --full) REFRESH_MODE=full; shift ;;
        --hygiene-only) REFRESH_MODE=hygiene-only; shift ;;
        --dispositions-only) REFRESH_MODE=dispositions-only; shift ;;
        --from-summary=*)
            REFRESH_MODE=from-summary
            FROM_SUMMARY_PATH="${1#--from-summary=}"
            shift ;;
        --from-summary)
            [[ $# -ge 2 ]] || die "--from-summary requires a path argument"
            REFRESH_MODE=from-summary
            FROM_SUMMARY_PATH="$2"
            shift 2 ;;
        --untracked-only) DISPOSITIONS_UNTRACKED_ONLY=1; shift ;;
        --stale) CHECK_STALE=1; shift ;;
        --by)
            [[ $# -ge 2 ]] || die "--by requires a value"
            UPDATED_BY="$2"; shift 2 ;;
        --*) die "unknown flag: $1" ;;
        show|check|refresh|known-failing|dispositions|ledger)
            [[ -z "$SUBCMD" ]] || die "multiple subcommands: $SUBCMD and $1"
            SUBCMD="$1"; shift ;;
        *)
            if [[ "$SUBCMD" == "baseline" && -z "$BASELINE_ACTION" ]]; then
                BASELINE_ACTION="$1"; shift
            else
                die "unknown argument: $1"
            fi
            ;;
    esac
done

case "$SUBCMD" in
    show) cmd_show ;;
    check) cmd_check ;;
    refresh) cmd_refresh ;;
    known-failing) cmd_known_failing ;;
    dispositions) cmd_dispositions ;;
    ledger) cmd_ledger ;;
    baseline) cmd_baseline ;;
    "") usage; exit 3 ;;
    *) die "unknown subcommand: $SUBCMD" ;;
esac
