#!/usr/bin/env bash
# scripts/intel-query.sh — Canonical helper for language intelligence queries.
#
# All intelligence integrations call this script, never open-code availability logic.
# Graceful degradation: always exits 0, reports unavailability via structured output.
#
# Usage:
#   scripts/intel-query.sh search "pattern matching exhaustiveness"
#   scripts/intel-query.sh compare "type inference" --repo rust,go
#   scripts/intel-query.sh status
#   scripts/intel-query.sh search "borrow checker" --human
#   scripts/intel-query.sh --timeout 10 search "ARC optimization"
#
# Output (default JSON):
#   On success:     {"status":"ok","data":...}
#   On unavailable: {"status":"unavailable","reason":"..."}
#   --human:        raw text output (NOT JSON); diagnostic messages to stderr
#
# Exit codes: 0 — always (graceful degradation)

# Resolve paths relative to script location (safe to call from any directory)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
INTEL_DIR="$PROJECT_ROOT/../lang_intelligence"
VENV_PYTHON="$INTEL_DIR/.venv/bin/python"
QUERY_SCRIPT="$INTEL_DIR/neo4j/query_graph.py"

# Defaults
STEP_TIMEOUT=5
JSON_FLAG="--json"

# Parse our flags, collect the rest for query_graph.py
PASS_ARGS=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --human)   JSON_FLAG=""; shift ;;
        --timeout)
            if [[ $# -lt 2 ]]; then
                # --timeout at end with no value — use default
                shift
            elif [[ "$2" =~ ^[0-9]+$ ]]; then
                # 10# forces decimal — prevents bash octal trap on leading-zero inputs (e.g. 08)
                # Minimum 1s — timeout 0 disables GNU timeout entirely
                STEP_TIMEOUT=$(( 10#$2 > 0 ? 10#$2 : 1 ))
                shift 2
            else
                # Next arg isn't numeric — treat as implicit default
                shift
            fi
            ;;
        *)         PASS_ARGS+=("$1"); shift ;;
    esac
done

# Derive query timeout from step timeout (queries need more time than probes)
QUERY_TIMEOUT=$((STEP_TIMEOUT * 6))

is_json() { [[ -n "$JSON_FLAG" ]]; }

# Output unavailable response and exit 0
unavailable() {
    local reason="$1"
    if is_json; then
        timeout "$STEP_TIMEOUT" python3 -c \
            'import sys,json; json.dump({"status":"unavailable","reason":sys.argv[1]}, sys.stdout)' \
            "$reason" 2>/dev/null \
            || printf '{"status":"unavailable","reason":"%s"}' "$reason"
        printf '\n'
    else
        printf 'Intelligence unavailable: %s\n' "$reason" >&2
    fi
    exit 0
}

# --- Availability Check Sequence (cheapest first, each bounded) ---

# Step 1: lang_intelligence directory exists
[[ -d "$INTEL_DIR" ]] || unavailable "lang_intelligence directory not found"

# Step 2: venv has neo4j package
timeout "$STEP_TIMEOUT" "$VENV_PYTHON" -c "import neo4j" 2>/dev/null \
    || unavailable "Python venv missing or neo4j package not installed"

# Step 3: Docker container running
CONTAINER_STATE=$(timeout "$STEP_TIMEOUT" docker inspect lang-intelligence \
    --format '{{.State.Running}}' 2>/dev/null || echo "false")
[[ "$CONTAINER_STATE" == "true" ]] \
    || unavailable "Neo4j container 'lang-intelligence' not running"

# Step 4+5: Bolt + auth + full-text index (health-check verifies all three)
HEALTH_JSON=$(timeout "$STEP_TIMEOUT" "$VENV_PYTHON" "$QUERY_SCRIPT" \
    --json health-check 2>/dev/null) \
    || unavailable "health-check timed out or crashed"

HEALTH_STATUS=$(timeout "$STEP_TIMEOUT" python3 -c \
    "import sys,json; print(json.load(sys.stdin).get('status','error'))" \
    <<< "$HEALTH_JSON" 2>/dev/null) \
    || unavailable "failed to parse health-check response"

if [[ "$HEALTH_STATUS" != "ok" ]]; then
    REASON=$(timeout "$STEP_TIMEOUT" python3 -c \
        "import sys,json; print(json.load(sys.stdin).get('reason','unknown'))" \
        <<< "$HEALTH_JSON" 2>/dev/null || echo "unknown")
    unavailable "$REASON"
fi

HAS_INDEX=$(timeout "$STEP_TIMEOUT" python3 -c \
    "import sys,json; print(json.load(sys.stdin).get('has_fulltext_index',False))" \
    <<< "$HEALTH_JSON" 2>/dev/null || echo "False")
if [[ "$HAS_INDEX" != "True" ]]; then
    unavailable "full-text index 'issue_text' not found — run schema.cypher first"
fi

# --- All checks passed ---

# Handle 'status' subcommand
if [[ ${#PASS_ARGS[@]} -gt 0 && "${PASS_ARGS[0]}" == "status" ]]; then
    STATS_JSON=$(timeout "$QUERY_TIMEOUT" "$VENV_PYTHON" "$QUERY_SCRIPT" --json stats 2>/dev/null) \
        || unavailable "stats query failed"

    VERSION_JSON=$(timeout "$STEP_TIMEOUT" "$VENV_PYTHON" "$QUERY_SCRIPT" --json cypher \
        "CALL dbms.components() YIELD name, versions RETURN name, versions[0] as version" \
        2>/dev/null || echo "{}")

    # Per-repo code-symbol counts. `stats` reports issues+PRs counts only (from
    # the GitHub corpus). Repos without an indexed issue tracker (e.g. Ori) show
    # total=0 there, which is indistinguishable from "not indexed" — historically
    # the #1 cause of users concluding "Ori symbols aren't indexed" and bypassing
    # the graph. This query restores per-repo visibility of code-symbol counts.
    SYMBOLS_JSON=$(timeout "$STEP_TIMEOUT" "$VENV_PYTHON" "$QUERY_SCRIPT" --json cypher \
        "MATCH (r:Repo) RETURN r.name AS repo, count{(s:Symbol)-[:IN_REPO]->(r)} AS symbols" \
        2>/dev/null || echo "{}")

    if is_json; then
        COMBINED=$(_STATS="$STATS_JSON" _VERSION="$VERSION_JSON" _SYMBOLS="$SYMBOLS_JSON" \
            timeout "$STEP_TIMEOUT" python3 << 'PYEOF'
import json, os, sys
stats = json.loads(os.environ["_STATS"])
try:
    vdata = json.loads(os.environ.get("_VERSION", "{}"))
    version = vdata.get("rows", [{}])[0].get("version", "unknown")
except Exception:
    version = "unknown"
try:
    sdata = json.loads(os.environ.get("_SYMBOLS", "{}"))
    symbols_by_repo = {row["repo"]: row["symbols"] for row in sdata.get("rows", [])}
except Exception:
    symbols_by_repo = {}
repos = stats.get("repos", [])
for entry in repos:
    entry["symbols"] = symbols_by_repo.get(entry.get("name"), 0)
empty = "warning" in stats
result = {
    "neo4j_version": version,
    "repos": repos,
    "nodes": stats.get("nodes", []),
    "relationships": stats.get("relationships", []),
    "empty": empty,
}
if empty:
    result["warning"] = stats["warning"]
json.dump(result, sys.stdout)
PYEOF
        ) || unavailable "failed to assemble status response"
        printf '{"status":"ok","data":%s}\n' "$COMBINED"
    else
        VERSION=$(timeout "$STEP_TIMEOUT" python3 -c "
import json, sys
try:
    d = json.loads(sys.argv[1])
    print(d.get('rows',[{}])[0].get('version','unknown'))
except Exception:
    print('unknown')
" "$VERSION_JSON" 2>/dev/null || echo "unknown")
        echo "=== Language Intelligence Status ==="
        echo "Neo4j version: $VERSION"
        echo ""
        timeout "$QUERY_TIMEOUT" "$VENV_PYTHON" "$QUERY_SCRIPT" stats
        STATS_RC=$?
        if [[ $STATS_RC -ne 0 ]]; then
            printf 'Intelligence unavailable: stats query failed or timed out\n' >&2
        fi
        # Render per-repo code-symbol counts (not in `stats` output)
        echo ""
        echo "Code symbols per repo:"
        _SYMBOLS="$SYMBOLS_JSON" timeout "$STEP_TIMEOUT" python3 << 'PYEOF' 2>/dev/null || echo "  (symbol counts unavailable)"
import json, os, sys
try:
    sdata = json.loads(os.environ.get("_SYMBOLS", "{}"))
    rows = sorted(sdata.get("rows", []), key=lambda r: r.get("symbols", 0), reverse=True)
    if not rows:
        print("  (no repos indexed)")
    else:
        for row in rows:
            name = row.get("repo", "?")
            n = row.get("symbols", 0)
            print(f"  {name:<12} {n:>8,} symbols")
except Exception as exc:
    print(f"  (symbol counts unavailable: {exc})")
PYEOF
    fi
    exit 0
fi

# Show help if no command given
if [[ ${#PASS_ARGS[@]} -eq 0 ]]; then
    if is_json; then
        printf '{"status":"ok","data":{"help":"Usage: intel-query.sh <command> [args...]. Run with --human for full help."}}\n'
    else
        timeout "$STEP_TIMEOUT" "$VENV_PYTHON" "$QUERY_SCRIPT" 2>&1
        RC=$?
        # rc=1 is expected (query_graph.py exits 1 for help); rc=124 = timeout, others = crash
        if [[ $RC -eq 124 ]]; then
            printf 'Intelligence unavailable: help timed out\n' >&2
        elif [[ $RC -gt 1 ]]; then
            printf 'Intelligence unavailable: help command failed (exit %d)\n' "$RC" >&2
        fi
    fi
    exit 0
fi

# Proxy to query_graph.py
if is_json; then
    RESULT=$(timeout "$QUERY_TIMEOUT" "$VENV_PYTHON" "$QUERY_SCRIPT" --json "${PASS_ARGS[@]}" 2>/dev/null)
    RC=$?
    if [[ $RC -ne 0 || -z "$RESULT" ]]; then
        # Try to extract reason from Python's JSON error output
        REASON="query failed"
        if [[ -n "$RESULT" ]]; then
            REASON=$(timeout "$STEP_TIMEOUT" python3 -c \
                'import sys,json; print(json.loads(sys.stdin.read()).get("reason","query failed"))' \
                <<< "$RESULT" 2>/dev/null) || REASON="query failed"
        fi
        unavailable "$REASON"
    fi
    printf '{"status":"ok","data":%s}\n' "$RESULT"
else
    timeout "$QUERY_TIMEOUT" "$VENV_PYTHON" "$QUERY_SCRIPT" "${PASS_ARGS[@]}"
    if [[ $? -ne 0 ]]; then
        printf 'Intelligence unavailable: query failed or timed out\n' >&2
    fi
fi
exit 0
