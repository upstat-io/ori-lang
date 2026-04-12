---
section: "01"
title: "Infrastructure & Canonical Helper"
status: not-started
reviewed: false
goal: "Create the single canonical helper script that all intelligence integrations call. Handles availability check, Neo4j health probe, venv activation, query execution, and structured output."
success_criteria:
  - "scripts/intel-query.sh exists and is executable"
  - "Returns structured JSON on success, empty JSON on unavailable (exit 0 either way)"
  - "Checks ../lang_intelligence/ exists, Neo4j container running, Bolt port reachable"
  - "Activates venv transparently, handles missing neo4j Python package"
  - "All other integration points (sections 02-04) call this script, never open-code availability logic"
depends_on: []
third_party_review:
  status: none
  updated: null
---

# 01 Infrastructure & Canonical Helper

## 01.0 Goal

One script, `scripts/intel-query.sh`, is the SSOT for all intelligence DB access from ori_lang. Every rule, skill, command, and hook that needs intelligence calls this script. No integration point ever checks Neo4j availability, activates the venv, or runs a Cypher query directly — that's the helper's job.

This follows the SSOT principle from `.claude/rules/impl-hygiene.md`: "every behavioral decision has exactly ONE file that defines it."

## 01.1 Canonical Helper Script

**File**: `scripts/intel-query.sh`

**Contract**:
```
Usage: scripts/intel-query.sh <command> [args...]
       scripts/intel-query.sh search "pattern matching exhaustiveness"
       scripts/intel-query.sh compare "type inference"
       scripts/intel-query.sh cypher "MATCH (i:Issue) RETURN count(i)"
       scripts/intel-query.sh status

Exit codes:
  0 — always (graceful degradation)
  
Output:
  On success: command output to stdout
  On unavailable: '{"status":"unavailable","reason":"..."}' to stdout
  Diagnostic messages to stderr only
```

**Availability check sequence** (all must pass, checked in order):
1. `../lang_intelligence/` directory exists
2. `docker inspect lang-intelligence --format '{{.State.Running}}'` returns `true`
3. `nc -z localhost 7687` succeeds (Bolt port reachable)
4. `../lang_intelligence/.venv/bin/python -c "import neo4j"` succeeds

If any check fails, output the unavailable JSON and exit 0. The caller never sees an error.

**Implementation checklist**:
- [ ] Create `scripts/intel-query.sh` with the availability check sequence
- [ ] Proxy all arguments to `../lang_intelligence/.venv/bin/python ../lang_intelligence/neo4j/query_graph.py`
- [ ] Add `status` subcommand that reports: Neo4j version, node/relationship counts, repo list, last fetch timestamps
- [ ] Add `--json` flag for machine-readable output (for skill integrations that parse results)
- [ ] Add `--timeout 5` default to prevent hanging on unresponsive Neo4j
- [ ] Verify script is idempotent and safe to call from any directory (use `$(dirname "$0")` for relative paths)
- [ ] Test: Neo4j running → returns query results
- [ ] Test: Neo4j stopped → returns unavailable JSON, exit 0
- [ ] Test: lang_intelligence repo missing → returns unavailable JSON, exit 0
- [ ] Test: venv missing neo4j package → returns unavailable JSON, exit 0

### Subsection 01.1 close-out

**`/improve-tooling` retrospective**: After implementing this subsection, look back at the debugging/testing experience. Did `scripts/intel-query.sh` give clear output on failure? Was the availability check sequence easy to debug? Any flags or output formats that would have helped? If so, improve the script before moving on.

---

## 01.2 Fix Existing query_graph.py Bugs

The round-2 TPR found real bugs in the query tool that must be fixed before any integration:

- [ ] Fix `sqlite3.Row.get()` crash — `Row` supports `row["key"]` not `row.get("key")`. Replace all `.get()` calls with dict-style access or convert rows to dicts first.
- [ ] Fix preset args silently ignored — `cmd_ori_preset` receives `_args` but never merges them into `effective_args`. User flags like `--repo rust` are dropped.
- [ ] Fix `INSERT OR REPLACE` on `pull_reviews` — use `INSERT ... ON CONFLICT(issue_id, github_id) DO UPDATE SET ...` instead.
- [ ] Add `--json` output mode to `query_graph.py` for machine-readable results (used by `intel-query.sh`)
- [ ] Test all query commands with actual data (Rust + Koka are already in Neo4j)

### Subsection 01.2 close-out

**`/improve-tooling` retrospective**: Did fixing these bugs surface any other issues in the query tool? Any missing error handling? Any commands that silently fail?

---

## 01.R Third Party Review Findings

- None.

## Completion Checklist

- [ ] `scripts/intel-query.sh` exists, executable, passes all 4 test scenarios
- [ ] `query_graph.py` bugs fixed (Row.get, preset args, INSERT OR REPLACE)
- [ ] `scripts/intel-query.sh status` returns live graph stats
- [ ] No test regressions: `timeout 150 ./test-all.sh`
- [ ] `/tpr-review` clean
- [ ] `/impl-hygiene-review` clean
- [ ] `/improve-tooling` section-close sweep
