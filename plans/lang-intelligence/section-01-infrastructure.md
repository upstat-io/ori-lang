---
section: "01"
title: "Infrastructure & Canonical Helper"
status: in-progress
reviewed: false
goal: "Create the single canonical helper script that all intelligence integrations call. Handles availability check, Neo4j health probe, venv activation, query execution, and structured output. Fix real bugs in query_graph.py before any integration."
success_criteria:
  - "scripts/intel-query.sh exists and is executable"
  - "Default output is JSON (for machine consumption by skills); --human flag switches to human-readable display"
  - "When Neo4j is unreachable: returns exit 0 with JSON {\"status\":\"unavailable\",\"reason\":\"...\"}"
  - "When Neo4j is reachable: returns exit 0 with JSON {\"status\":\"ok\",\"data\":...} (no command field)"
  - "Availability check validates Bolt protocol readiness (not just TCP port), auth, and full-text index presence"
  - "Each availability check step has a bounded timeout (5s max per step)"
  - "All other integration points (sections 02-04) call this script, never open-code availability logic"
  - "query_graph.py label-graph command is implemented (not a TODO stub)"
  - "query_graph.py has --json flag for machine-readable output"
  - "query_graph.py wraps command dispatch in centralized try/except with actionable error messages (not just get_driver() — driver is lazy)"
depends_on: []
sections:
  - id: "01.1"
    title: "Canonical Helper Script"
    status: in-progress
  - id: "01.2"
    title: "Fix Existing query_graph.py Issues"
    status: in-progress
  - id: "01.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
third_party_review:
  status: resolved
  updated: 2026-04-12
---

# 01 Infrastructure & Canonical Helper

## 01.0 Goal

One script, `scripts/intel-query.sh`, is the SSOT for all intelligence DB access from ori_lang. Every rule, skill, command, and hook that needs intelligence calls this script. No integration point ever checks Neo4j availability, activates the venv, or runs a Cypher query directly — that's the helper's job.

This follows the SSOT principle from `.claude/rules/impl-hygiene.md`: "every behavioral decision has exactly ONE file that defines it."

**Output contract (canonical — all references in this plan MUST match):**
- Default format: **JSON** (for machine consumption by skills and rules)
- `--human` flag: **raw text** output for direct human consumption — NOT JSON. Skills NEVER pass `--human`; it is for interactive use only.
- On success: `{"status":"ok","data":...}` to stdout, exit 0
- On unavailable: `{"status":"unavailable","reason":"<reason>"}` to stdout, exit 0
- Diagnostic/debug messages: stderr only, never stdout

**Ownership split (important distinction)**:
- The **shell script** owns the availability CHECK — fast bootstrap probes that answer "is Neo4j reachable?" (filesystem check, venv check, container check, Bolt handshake). These run before invoking Python so the shell can return the unavailable JSON without ever starting Python.
- **Python** (`query_graph.py`) owns health/status SEMANTICS — detailed graph queries, node counts, index verification, repo list, JSON formatting, and all query logic. The shell's job is to get Python running; everything meaningful happens in Python.

These are distinct concerns. The shell answers "can I reach Neo4j?" The Python answers "what is the graph state?"

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
  On success: {"status":"ok","data":...} to stdout
  On unavailable: {"status":"unavailable","reason":"..."} to stdout
  Diagnostic messages to stderr only

  --human: raw text output (NOT JSON) — human consumption only; skills never pass this flag

Flags:
  --human   Switch to human-readable display format
  --timeout N  Override default 5s per-step timeout
```

**Availability check sequence** (all must pass, checked in order — cheapest first, each with 5s timeout):
1. `../lang_intelligence/` directory exists (filesystem check, instant)
2. `../lang_intelligence/.venv/bin/python -c "import neo4j"` succeeds (venv has the package — must pass before we can use the driver)
3. `docker inspect lang-intelligence --format '{{.State.Running}}'` returns `true` (container running)
4. Neo4j Bolt protocol readiness + auth: `../lang_intelligence/.venv/bin/python ../lang_intelligence/neo4j/query_graph.py --health-check` — a lightweight command that verifies TCP, Bolt handshake, AND auth using the configured credentials (reads `NEO4J_URI`/`NEO4J_USER`/`NEO4J_PASS` env vars or defaults). Credentials stay in one place (Python), not duplicated inline in the shell script.
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
- [ ] Test: Neo4j reachable but `issue_text` full-text index missing → returns unavailable JSON with reason indicating missing index

- [ ] **Subsection close-out (01.1)** — MANDATORY before starting 01.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect on the debugging/testing experience for 01.1: did `scripts/intel-query.sh` give clear output on failure? Was the availability check sequence easy to debug? Any flags or output formats that would have helped? Implement improvements NOW via separate `/commit-push`.

---

## 01.2 Fix Existing query_graph.py Issues

Real issues found in `~/projects/lang_intelligence/neo4j/query_graph.py` that must be fixed before any integration. All line references are from the current file.

### 01.2a Error Handling & Robustness

- [ ] Unhandled Neo4j exceptions — `get_driver()` (line 35-36) is lazy: driver construction succeeds even when Neo4j is down; the real `ServiceUnavailable` exception fires at `session.run()` time inside each command function. Wrapping only `get_driver()` does nothing. Fix: wrap the command dispatch in `main()` (the routing block that calls `cmd_*`) in a centralized `try/except (ServiceUnavailable, AuthError, Exception)` that prints an actionable error message and exits non-zero. This catches all command-level failures in one place.
- [ ] No `try/finally` on `driver.close()` — every command function (e.g., `cmd_search` line 83-102, `cmd_compare` line 105-143) calls `driver.close()` at the end, but if the session query raises an exception, `driver.close()` is skipped and the connection leaks. Use context manager (`with get_driver() as driver:`) or try/finally.
- [ ] `_parse_flags` (line 53, 55) calls `int(args[i+1])` on `--limit` and `--depth` values with no validation — non-numeric input like `--limit foo` raises an unhandled `ValueError` traceback. Add `try/except ValueError` with a clear error message. Also fix positional numeric args in `cmd_related` (line 151) and `cmd_fix_chain` (line 202) which do `int(args[1])` without any validation — same crash on non-numeric input. Validate all positional numeric args in those commands with the same try/except pattern.
- [ ] No connection timeout — `GraphDatabase.driver()` (line 36) uses default timeout, which can hang indefinitely if the Bolt port is open but the server is wedged. Add `connection_timeout=5` to the driver constructor.

### 01.2b Missing Functionality

- [ ] Implement `--health-check` command in `query_graph.py` — a lightweight probe that verifies TCP + Bolt handshake + auth using the configured credentials (env vars or defaults). Returns JSON `{"status":"ok"}` on success, `{"status":"error","reason":"..."}` on failure. Exit 0 always. This is called by `intel-query.sh` step 4 to validate connectivity without running a full query. Also verify that the full-text `issue_text` index exists (needed by `search`, `compare`, `fixed`, `pattern`).
- [ ] `label-graph` command (line 444) is a TODO stub: `lambda a: print("TODO: label co-occurrence")`. Implement the label co-occurrence graph query or remove it from the usage docstring — a silently no-op command is a trap.
- [ ] No `--json` output mode — all commands print human-readable text to stdout. Add `--json` flag that makes every command output structured JSON instead. This is required by `intel-query.sh` which needs machine-parseable results.
- [ ] No graph-emptiness detection in `cmd_stats` — if the graph has zero Issue nodes (e.g., after a fresh `schema.cypher` without any data import), `stats` prints empty tables with no warning. Add a "graph is empty — run the fetch pipeline first" message.
- [ ] Refactor `cmd_compare` (line 106) and `cmd_pattern` (line 256) to use `_parse_flags` for argument parsing — both currently do `" ".join(args)` which would include `--json` in the search terms if that flag appears in `args`. Using `_parse_flags` to strip global flags before joining ensures `--json` (and future flags) are intercepted rather than appended to the Cypher query string.

### 01.2c Hardcoded Configuration

- [ ] Credentials hardcoded (lines 30-32): `bolt://localhost:7687`, `neo4j`, `intelligence` are inline constants. Read from environment variables (`NEO4J_URI`, `NEO4J_USER`, `NEO4J_PASS`) with the current values as defaults. This allows different environments (CI, remote, Docker Compose with different ports) without editing source.

- [ ] **Subsection close-out (01.2)** — MANDATORY before starting 01.N:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect on the debugging/testing experience for 01.2: did fixing these issues surface any other problems in the query tool? Any commands that silently fail? Any missing error handling paths? Implement improvements NOW via separate `/commit-push`.

---

## 01.R Third Party Review Findings

- [x] `[TPR-01-001-codex][high]` `section-01-infrastructure.md:108` — get_driver() is lazy; wrapping it won't catch Neo4j failures.
  Resolved: Fixed on 2026-04-12. Rewrote 01.2a to target command dispatch, not get_driver().
- [x] `[TPR-01-002-codex][high]` `section-01-infrastructure.md:46` — LEAK between shell bootstrap and Python health logic.
  Resolved: Fixed on 2026-04-12. Added ownership split paragraph; shell does availability CHECK, Python does health SEMANTICS. Step 4 now calls `query_graph.py --health-check` instead of inline probe.
- [x] `[TPR-01-003-codex][medium]` `section-01-infrastructure.md:42` — Success JSON contract DRIFT (command field inconsistent).
  Resolved: Fixed on 2026-04-12. Removed `command` field from canonical contract; unified across all locations.
- [x] `[TPR-01-004-codex][medium]` `section-01-infrastructure.md:110` — Missing numeric validation in cmd_related/cmd_fix_chain.
  Resolved: Fixed on 2026-04-12. Expanded _parse_flags item to cover positional args.
- [x] `[TPR-01-005-codex][low]` `00-overview.md:150` — Overview Known Issues lists only 5 of 8 items.
  Resolved: Fixed on 2026-04-12. Added 3 missing issues to overview.
- [x] `[TPR-01-001-gemini][medium]` `section-01-infrastructure.md:108` — Incorrect line numbers for query_graph.py references.
  Resolved: Rejected after verification on 2026-04-12. Line numbers in plan are correct: get_driver IS at 35-36, _parse_flags int() IS at 53, label-graph IS at 444. Gemini confabulated incorrect alternatives.
- [x] `[TPR-01-002-gemini][high]` `section-01-infrastructure.md:116` — cmd_compare and cmd_pattern bypass _parse_flags; --json flag would be included in search terms.
  Resolved: Fixed on 2026-04-12. Added new 01.2b item requiring refactor of cmd_compare/cmd_pattern to use _parse_flags.
- [x] `[TPR-01-003-gemini][low]` `00-overview.md:157` — label-graph line number wrong (claims 332).
  Resolved: Rejected after verification on 2026-04-12. Label-graph is at line 444, not 332. Gemini confabulated.
- [x] `[TPR-01-004-gemini][medium]` `section-01-infrastructure.md:41` — --human flag behavior ambiguous.
  Resolved: Fixed on 2026-04-12. Clarified: --human returns raw text (NOT JSON); added to contract and success criteria.
- [x] `[TPR-01-005-gemini][medium]` `00-overview.md:150` — Overview missing 3 issues (near-agreement with TPR-01-005-codex).
  Resolved: Fixed on 2026-04-12. Same fix as TPR-01-005-codex.
- [x] `[TPR-01-006-gemini][medium]` `section-01-infrastructure.md:19` — 01.R and 01.N missing from frontmatter sections.
  Resolved: Fixed on 2026-04-12. Added both to sections array.
- [x] `[TPR-01-007-gemini][high]` `section-01-infrastructure.md:96` — Close-out blocks not in canonical checklist format.
  Resolved: Fixed on 2026-04-12. Replaced prose with canonical close-out checklist per plan-schema.md.
- [x] `[TPR-01-008-gemini][high]` `section-01-infrastructure.md:133` — Missing plan-sync and annotation-cleanup in completion checklist.
  Resolved: Fixed on 2026-04-12. Added both items to 01.N.
- [x] `[TPR-01-001-codex][high]` (iter2) `section-01-infrastructure.md:90` — --health-check not in 01.2 tasks.
  Resolved: Fixed on 2026-04-12. Added --health-check implementation task to 01.2b.
- [x] `[TPR-01-002-codex][medium]` (iter2) `section-01-infrastructure.md:29` — 01.C should be 01.N per schema.
  Resolved: Fixed on 2026-04-12. Renamed all 01.C references to 01.N.
- [x] `[TPR-01-003-codex][medium]` (iter2) `section-01-infrastructure.md:91` — Missing index-absent test scenario.
  Resolved: Fixed on 2026-04-12. Added 6th test scenario and updated completion checklist count.
- [x] `[TPR-01-001-gemini][medium]` (iter2) `section-01-infrastructure.md:188` — Missing index.md sync in plan-sync.
  Resolved: Fixed on 2026-04-12. Added index.md update item.
- [x] `[TPR-01-002-gemini][medium]` (iter2) `00-overview.md:150` — cmd_compare/_parse_flags not in overview.
  Resolved: Fixed on 2026-04-12. Added to Known Issues.
- [x] `[TPR-01-003-gemini][high]` (iter2) `section-01-infrastructure.md:126` — --health-check not in 01.2 tasks.
  Resolved: Fixed on 2026-04-12. Same fix as iter2 TPR-01-001-codex (near-agreement).
- [x] `[TPR-01-004-gemini][medium]` (iter2) `section-01-infrastructure.md:189` — Missing Exit Criteria block.
  Resolved: Fixed on 2026-04-12. Added Exit Criteria paragraph.

## 01.N Completion Checklist

- [ ] `scripts/intel-query.sh` exists, executable, passes all 6 test scenarios (running, stopped, missing repo, missing venv, Bolt-not-ready, index-missing)
- [ ] `query_graph.py` `--health-check` command implemented and used by `intel-query.sh` step 4
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
- [ ] **Plan annotation cleanup**: Run `plan-annotations.sh --cleanup-only --plan lang-intelligence` — remove any stale annotations
- [ ] **Plan sync**:
  - [ ] Update section 01 frontmatter `status` to `complete`
  - [ ] Update `index.md` section status
  - [ ] Update `00-overview.md` Quick Reference table — Section 01 status
  - [ ] Update `00-overview.md` mission success criteria checkboxes
  - [ ] Verify Section 02's `depends_on: [01]` is still accurate

**Exit Criteria:** `scripts/intel-query.sh search "type inference"` returns valid JSON with `status:ok` when Neo4j is running, and `scripts/intel-query.sh search "type inference"` returns `{"status":"unavailable","reason":"..."}` with exit 0 when Neo4j is stopped. All 6 test scenarios pass. `query_graph.py --health-check` returns `{"status":"ok"}` when connected. `query_graph.py --json stats` returns structured JSON. `timeout 150 ./test-all.sh` green.
