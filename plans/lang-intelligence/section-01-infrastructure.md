---
section: "01"
title: "Infrastructure & Canonical Helper"
status: not-started
reviewed: false
goal: "Create the single canonical helper script that all intelligence integrations call. Handles availability check, Neo4j health probe, venv activation, query execution, and structured output. Fix real bugs in query_graph.py before any integration."
success_criteria:
  - "scripts/intel-query.sh exists and is executable"
  - "Default output is JSON (for machine consumption by skills); --human flag switches to human-readable display"
  - "When Neo4j is unreachable: returns exit 0 with JSON {\"status\":\"unavailable\",\"reason\":\"...\"}"
  - "When Neo4j is reachable: returns exit 0 with JSON {\"status\":\"ok\",\"data\":...}"
  - "Availability check validates Bolt protocol readiness (not just TCP port), auth, and full-text index presence"
  - "Each availability check step has a bounded timeout (5s max per step)"
  - "All other integration points (sections 02-04) call this script, never open-code availability logic"
  - "query_graph.py label-graph command is implemented (not a TODO stub)"
  - "query_graph.py has --json flag for machine-readable output"
  - "query_graph.py wraps get_driver() in try/except with actionable error messages"
depends_on: []
sections:
  - id: "01.1"
    title: "Canonical Helper Script"
    status: not-started
  - id: "01.2"
    title: "Fix Existing query_graph.py Issues"
    status: not-started
third_party_review:
  status: none
  updated: null
---

# 01 Infrastructure & Canonical Helper

## 01.0 Goal

One script, `scripts/intel-query.sh`, is the SSOT for all intelligence DB access from ori_lang. Every rule, skill, command, and hook that needs intelligence calls this script. No integration point ever checks Neo4j availability, activates the venv, or runs a Cypher query directly — that's the helper's job.

This follows the SSOT principle from `.claude/rules/impl-hygiene.md`: "every behavioral decision has exactly ONE file that defines it."

**Output contract (canonical — all references in this plan MUST match):**
- Default format: **JSON** (for machine consumption by skills and rules)
- `--human` flag: human-readable display format (tables, indentation)
- On success: `{"status":"ok","command":"<cmd>","data":...}` to stdout, exit 0
- On unavailable: `{"status":"unavailable","reason":"<reason>"}` to stdout, exit 0
- Diagnostic/debug messages: stderr only, never stdout

The shell script handles ONLY bootstrap: locate sibling repo, resolve venv, invoke Python, degrade gracefully. All graph semantics, health checks, JSON formatting, and query logic live in Python (`query_graph.py`).

## 01.1 Canonical Helper Script

**File**: `scripts/intel-query.sh`

**Contract**:
```
Usage: scripts/intel-query.sh <command> [args...]
       scripts/intel-query.sh search "pattern matching exhaustiveness"
       scripts/intel-query.sh compare "type inference"
       scripts/intel-query.sh cypher "MATCH (i:Issue) RETURN count(i)"
       scripts/intel-query.sh status
       scripts/intel-query.sh search "type inference" --human

Exit codes:
  0 — always (graceful degradation)

Output (default JSON):
  On success: {"status":"ok","command":"...","data":...} to stdout
  On unavailable: {"status":"unavailable","reason":"..."} to stdout
  Diagnostic messages to stderr only

Flags:
  --human   Switch to human-readable display format
  --timeout N  Override default 5s per-step timeout
```

**Availability check sequence** (all must pass, checked in order — cheapest first, each with 5s timeout):
1. `../lang_intelligence/` directory exists (filesystem check, instant)
2. `../lang_intelligence/.venv/bin/python -c "import neo4j"` succeeds (venv has the package — must pass before we can use the driver)
3. `docker inspect lang-intelligence --format '{{.State.Running}}'` returns `true` (container running)
4. Neo4j Bolt protocol readiness + auth: `../lang_intelligence/.venv/bin/python -c "from neo4j import GraphDatabase; d = GraphDatabase.driver('bolt://localhost:7687', auth=('neo4j','intelligence'), connection_timeout=5); d.verify_connectivity(); d.close()"` — verifies TCP, Bolt handshake, AND auth in one step (not just `nc -z` which only checks TCP)
5. Full-text index verification: `CALL db.indexes() YIELD name WHERE name = 'issue_text' RETURN count(*) > 0` — commands `search`, `compare`, `fixed`, `pattern` depend on this index from `schema.cypher`

If any check fails, output the unavailable JSON and exit 0. The caller never sees an error.

**Implementation checklist**:
- [ ] Create `scripts/intel-query.sh` with the availability check sequence (steps 1-5 above)
- [ ] Each check step bounded by `timeout 5` (or Python `socket.settimeout`) — hanging Neo4j must not hang the script
- [ ] Proxy all arguments to `../lang_intelligence/.venv/bin/python ../lang_intelligence/neo4j/query_graph.py --json [args]`
- [ ] Pass `--human` through when specified by caller, otherwise default to `--json`
- [ ] Add `status` subcommand that reports: Neo4j version, node/relationship counts, repo list, graph-emptiness check (warn if zero Issue nodes)
- [ ] Verify script is idempotent and safe to call from any directory (use `$(dirname "$0")` for relative paths)
- [ ] Test: Neo4j running with data → returns JSON with `status:ok` and query results
- [ ] Test: Neo4j stopped → returns `{"status":"unavailable","reason":"container not running"}`, exit 0
- [ ] Test: lang_intelligence repo missing → returns unavailable JSON, exit 0
- [ ] Test: venv missing neo4j package → returns unavailable JSON, exit 0
- [ ] Test: Neo4j container running but DB not ready (Bolt handshake fails) → returns unavailable JSON, not hang

### Subsection 01.1 close-out

**`/improve-tooling` retrospective**: After implementing this subsection, look back at the debugging/testing experience. Did `scripts/intel-query.sh` give clear output on failure? Was the availability check sequence easy to debug? Any flags or output formats that would have helped? If so, improve the script before moving on.

---

## 01.2 Fix Existing query_graph.py Issues

Real issues found in `~/projects/lang_intelligence/neo4j/query_graph.py` that must be fixed before any integration. All line references are from the current file.

### 01.2a Error Handling & Robustness

- [ ] `get_driver()` (line 35-36) has no try/except — if Neo4j is unreachable, the raw `ServiceUnavailable` exception propagates to the caller as an unhandled traceback. Wrap in try/except with a clear error message and non-zero exit.
- [ ] No `try/finally` on `driver.close()` — every command function (e.g., `cmd_search` line 83-102, `cmd_compare` line 105-143) calls `driver.close()` at the end, but if the session query raises an exception, `driver.close()` is skipped and the connection leaks. Use context manager (`with get_driver() as driver:`) or try/finally.
- [ ] `_parse_flags` (line 53, 55) calls `int(args[i+1])` on `--limit` and `--depth` values with no validation — non-numeric input like `--limit foo` raises an unhandled `ValueError` traceback. Add `try/except ValueError` with a clear error message.
- [ ] No connection timeout — `GraphDatabase.driver()` (line 36) uses default timeout, which can hang indefinitely if the Bolt port is open but the server is wedged. Add `connection_timeout=5` to the driver constructor.

### 01.2b Missing Functionality

- [ ] `label-graph` command (line 444) is a TODO stub: `lambda a: print("TODO: label co-occurrence")`. Implement the label co-occurrence graph query or remove it from the usage docstring — a silently no-op command is a trap.
- [ ] No `--json` output mode — all commands print human-readable text to stdout. Add `--json` flag that makes every command output structured JSON instead. This is required by `intel-query.sh` which needs machine-parseable results.
- [ ] No graph-emptiness detection in `cmd_stats` — if the graph has zero Issue nodes (e.g., after a fresh `schema.cypher` without any data import), `stats` prints empty tables with no warning. Add a "graph is empty — run the fetch pipeline first" message.

### 01.2c Hardcoded Configuration

- [ ] Credentials hardcoded (lines 30-32): `bolt://localhost:7687`, `neo4j`, `intelligence` are inline constants. Read from environment variables (`NEO4J_URI`, `NEO4J_USER`, `NEO4J_PASS`) with the current values as defaults. This allows different environments (CI, remote, Docker Compose with different ports) without editing source.

### Subsection 01.2 close-out

**`/improve-tooling` retrospective**: Did fixing these issues surface any other problems in the query tool? Any commands that silently fail? Any missing error handling paths?

---

## 01.R Third Party Review Findings

- None.

## 01.C Completion Checklist

- [ ] `scripts/intel-query.sh` exists, executable, passes all 5 test scenarios (running, stopped, missing repo, missing venv, Bolt-not-ready)
- [ ] `query_graph.py` issues fixed: driver error handling, driver.close() leak, _parse_flags validation, connection timeout
- [ ] `query_graph.py` label-graph command implemented (not a stub)
- [ ] `query_graph.py` --json output mode works for all commands
- [ ] `query_graph.py` credentials read from env vars with current values as defaults
- [ ] `scripts/intel-query.sh status` returns live graph stats including emptiness check
- [ ] Output contract is JSON-by-default everywhere — no command returns unstructured text to stdout in default mode
- [ ] No test regressions: `timeout 150 ./test-all.sh`
- [ ] `/tpr-review` clean
- [ ] `/impl-hygiene-review` clean
- [ ] `/improve-tooling` section-close sweep
