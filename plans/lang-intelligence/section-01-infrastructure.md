---
section: "01"
title: "Infrastructure & Canonical Helper"
status: in-progress
reviewed: true
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
    status: complete
  - id: "01.2"
    title: "Fix Existing query_graph.py Issues"
    status: complete
  - id: "01.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "01.N"
    title: "Completion Checklist"
    status: in-progress
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
- The **shell script** owns orchestration and graceful degradation — it sequences the availability checks (directory, venv, Docker, health-check) and returns structured unavailable JSON on any failure. Step 1 (directory check) is pure shell; steps 2-5 invoke Python for the neo4j import probe and health-check semantics.
- **Python** (`query_graph.py`) owns health/status SEMANTICS — detailed graph queries, node counts, index verification, repo list, JSON formatting, and all query logic. The shell's job is to orchestrate the check sequence and wrap failures; everything meaningful happens in Python.

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
4. Neo4j Bolt protocol readiness + auth: `../lang_intelligence/.venv/bin/python ../lang_intelligence/neo4j/query_graph.py --json health-check` — a lightweight command that verifies TCP, Bolt handshake, AND auth using the configured credentials (reads `NEO4J_URI`/`NEO4J_USER`/`NEO4J_PASS` env vars or defaults). Credentials stay in one place (Python), not duplicated inline in the shell script.
5. Full-text index verification: `CALL db.indexes() YIELD name WHERE name = 'issue_text' RETURN count(*) > 0` — commands `search`, `compare`, `fixed`, `pattern` depend on this index from `schema.cypher`

If any check fails, output the unavailable JSON and exit 0. The caller never sees an error.

**Implementation checklist**:
- [x] Create `scripts/intel-query.sh` with the availability check sequence (steps 1-5 above)
- [x] Each check step bounded by `timeout 5` (or Python `socket.settimeout`) — hanging Neo4j must not hang the script
- [x] Parse `--timeout N` flag from caller arguments (default 5s) — use this variable in all bounded check steps; derive query timeout (`QUERY_TIMEOUT = STEP_TIMEOUT * 6`) for proxy and stats calls
- [x] Proxy all arguments to `../lang_intelligence/.venv/bin/python ../lang_intelligence/neo4j/query_graph.py --json [args]`
- [x] Strip `--human` locally — in human mode, omit `--json` when calling `query_graph.py` (not forwarded as a flag)
- [x] Add `status` subcommand that reports: Neo4j version, node/relationship counts, repo list, graph-emptiness check (warn if zero Issue nodes)
- [x] Verify script is idempotent and safe to call from any directory (use `$(dirname "$0")` for relative paths)
- [x] Test: Neo4j running with data → returns JSON with `status:ok` and query results
- [x] Test: Neo4j stopped → returns `{"status":"unavailable","reason":"container not running"}`, exit 0
- [x] Test: lang_intelligence repo missing → returns unavailable JSON, exit 0
- [x] Test: venv missing neo4j package → returns unavailable JSON, exit 0
- [x] Test: Neo4j container running but DB not ready (Bolt handshake fails) → returns unavailable JSON, not hang
- [x] Test: Neo4j reachable but `issue_text` full-text index missing → returns unavailable JSON with reason indicating missing index

- [x] **Subsection close-out (01.1)** — MANDATORY before starting 01.2:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 01.1: no tooling gaps. Script gives clear JSON/text output on every failure path. Availability check sequence is straightforward (5 steps, cheapest first). Timeout handling via shell `timeout` is debuggable. No ori_lang diagnostic/testing tooling exercised.

---

## 01.2 Fix Existing query_graph.py Issues

Real issues found in `~/projects/lang_intelligence/neo4j/query_graph.py` that must be fixed before any integration. Function names referenced below; use `grep -n 'def func_name' neo4j/query_graph.py` for current line numbers.

### 01.2a Error Handling & Robustness

- [x] Unhandled Neo4j exceptions — `get_driver()` is lazy: driver construction succeeds even when Neo4j is down; the real `ServiceUnavailable` exception fires at `session.run()` time inside each command function. Wrapping only `get_driver()` does nothing. Fix: wrap the command dispatch in `main()` in a centralized `try/except (ServiceUnavailable, AuthError, Exception)` that prints an actionable error message and exits non-zero. This catches all command-level failures in one place.
- [x] No `try/finally` on `driver.close()` — every command function (e.g., `cmd_search`, `cmd_compare`) calls `driver.close()` at the end, but if the session query raises an exception, `driver.close()` is skipped and the connection leaks. Use context manager (`with get_driver() as driver:`) or try/finally.
- [x] `_parse_flags` calls `int(args[i+1])` on `--limit` and `--depth` values with no validation — non-numeric input raises an unhandled `ValueError`. Validators now raise `ValueError`, caught by `main()`'s centralized try/except. Also fix positional numeric args in `cmd_related` and `cmd_fix_chain` which do `int(args[1])` — same pattern.
- [x] No connection timeout — `GraphDatabase.driver()` uses default timeout, which can hang indefinitely if the Bolt port is open but the server is wedged. Add `connection_timeout=5` to the driver constructor.

### 01.2b Missing Functionality

- [x] Implement `health-check` command in `query_graph.py` — a lightweight probe that verifies TCP + Bolt handshake + auth using the configured credentials (env vars or defaults). Returns JSON `{"status":"ok"}` on success, `{"status":"error","reason":"..."}` on failure. Exit 0 always. This is called by `intel-query.sh` step 4 to validate connectivity without running a full query. Also verify that the full-text `issue_text` index exists (needed by `search`, `compare`, `fixed`, `pattern`).
- [x] `cmd_label_graph` was a TODO stub. Implement the label co-occurrence graph query or remove it from the usage docstring — a silently no-op command is a trap.
- [x] No `--json` output mode — all commands print human-readable text to stdout. Add `--json` flag that makes every command output structured JSON instead. This is required by `intel-query.sh` which needs machine-parseable results.
- [x] No graph-emptiness detection in `cmd_stats` — if the graph has zero Issue nodes (e.g., after a fresh `schema.cypher` without any data import), `stats` prints empty tables with no warning. Add a "graph is empty — run the fetch pipeline first" message.
- [x] Refactor `cmd_compare` and `cmd_pattern` to use `_parse_flags` for argument parsing — both used to do `" ".join(args)` which would include `--json` in the search terms. Using `_parse_flags` to strip global flags before joining ensures `--json` (and future flags) are intercepted.

### 01.2c Hardcoded Configuration

- [x] Credentials hardcoded: `bolt://localhost:7687`, `neo4j`, `intelligence` were inline constants. Read from environment variables (`NEO4J_URI`, `NEO4J_USER`, `NEO4J_PASS`) with the current values as defaults. This allows different environments (CI, remote, Docker Compose with different ports) without editing source.

- [x] Validate Python fixes: test `query_graph.py --limit foo` exits cleanly with error message (not traceback), test `query_graph.py related rust notanumber` exits cleanly, test `query_graph.py --json stats` returns valid JSON

- [x] **Subsection close-out (01.2)** — MANDATORY before starting 01.N:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 01.2: no tooling gaps. Work was entirely Python changes in an external project (`~/projects/lang_intelligence/`). Ori compiler tooling (diagnostics, test harness, scripts) was not exercised. No improvements needed.

---

## 01.R Third Party Review Findings

- [x] `[TPR-01-001-codex][high]` `section-01-infrastructure.md:108` — get_driver() is lazy; wrapping it won't catch Neo4j failures.
  Resolved: Fixed on 2026-04-12. Rewrote 01.2a to target command dispatch, not get_driver().
- [x] `[TPR-01-002-codex][high]` `section-01-infrastructure.md:46` — LEAK between shell bootstrap and Python health logic.
  Resolved: Fixed on 2026-04-12. Added ownership split paragraph; shell does availability CHECK, Python does health SEMANTICS. Step 4 now calls `query_graph.py health-check` instead of inline probe.
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
- [x] `[TPR-01-001-codex][medium]` (iter3) `index.md` — index.md has no per-section status table; only keyword clusters.
  Resolved: Fixed on 2026-04-12. Added Quick Reference table to index.md before keyword clusters section.
- [x] `[TPR-01-002-codex][low]` (iter3) `00-overview.md:5` — overview frontmatter status still `not-started` with section 01 in-progress.
  Resolved: Fixed on 2026-04-12. Updated overview frontmatter status to `in-progress` and Quick Reference table row 01 to `in-progress`.
- [x] `[TPR-01-003-codex][low]` (iter3) `00-overview.md:102` — Implementation Sequence says "everything else depends on section 01" but sections 05-09 have `depends_on: []`.
  Resolved: Fixed on 2026-04-12. Changed "everything else depends on this" to "sections 02-04 depend on this".
- [x] `[TPR-01-001-gemini][low]` (iter3) `section-01-infrastructure.md:112` — close-out `/improve-tooling` bullet lacks conventional-commit type guidance.
  Resolved: Fixed on 2026-04-12. Expanded both 01.1 and 01.2 close-out `/improve-tooling` bullets with conventional-commit type list and explicit `tools(...)` rejection note.
- [x] `[TPR-01-002-gemini][low]` (iter3) `section-01-infrastructure.md:202` — 01.N section-close sweep bullet is bare; no guidance on what to verify or how to commit.
  Resolved: Fixed on 2026-04-12. Expanded 01.N `/improve-tooling` section-close sweep item with cross-subsection pattern guidance and conventional-commit type requirements.
- [x] `[TPR-01-003-gemini][medium]` (iter3) `section-01-infrastructure.md:97` — `--timeout N` flag described in contract but not in 01.1 implementation checklist.
  Resolved: Fixed on 2026-04-12. Added `--timeout N` parsing task to 01.1 implementation checklist after the bounded-check-step item.
- [x] `[TPR-01-004-gemini][medium]` (iter3) `section-01-infrastructure.md:139` — No test validation item for the Python fixes in 01.2.
  Resolved: Fixed on 2026-04-12. Added validation item before 01.2 close-out block covering --limit, positional numeric, and --json tests.
- [x] `[TPR-01-005-gemini][rejected]` (iter3) `section-01-infrastructure.md` — Claimed `depends_on: []` on section-01 frontmatter was incorrect (should list itself as a dependency).
  Resolved: Rejected on 2026-04-12. `depends_on: []` on section 01 is correct — it has no dependencies. A section cannot depend on itself.
- [x] `[TPR-01-001-codex][high]` (close-out) `scripts/intel-query.sh:37` — GAP: `--timeout` without value causes infinite loop (`shift 2` fails, while loop spins).
  Resolved: Fixed on 2026-04-12. Added numeric validation and graceful fallback to default for missing/non-numeric `--timeout` values.
- [x] `[TPR-01-002-codex][high]` (close-out) `scripts/intel-query.sh:49` — LEAK: `unavailable()` hand-rolls JSON escaping via sed (misses newlines/control chars → malformed JSON).
  Resolved: Fixed on 2026-04-12. Replaced sed escaping with `python3 -c json.dump(...)` for proper JSON serialization.
- [x] `[TPR-01-003-codex][medium]` (close-out) `query_graph.py:83` — GAP: `_parse_flags` silently absorbs dangling flags (`--limit` with no value → becomes search term).
  Resolved: Fixed on 2026-04-12. Added missing-value checks that raise ValueError, caught by main()'s try/except.
- [x] `[TPR-01-004-codex][medium]` (close-out) `section-01-infrastructure.md:98` — DRIFT: Plan claims `--timeout N` is passed to `query_graph.py` via argument; implementation only uses it for shell probes.
  Resolved: Fixed on 2026-04-12. Updated plan claim to match reality (shell-side bounded probes + derived QUERY_TIMEOUT for proxy calls).
- [x] `[TPR-01-001-gemini][high]` (close-out) `scripts/intel-query.sh:105` — Proxy execution swallows Python's structured error JSON, returns generic "query failed".
  Resolved: Fixed on 2026-04-12. Proxy now extracts `reason` from Python's JSON error output when available.
- [x] `[TPR-01-002-gemini][medium]` (close-out) `query_graph.py:44` — Argument validation (`_parse_int`, command validators) bypasses `json_mode` — prints to stderr + `sys.exit(1)`.
  Resolved: Fixed on 2026-04-12. Changed all validators to raise ValueError; caught by main()'s try/except → routed through `_handle_error(msg, json_mode)`.
- [x] `[TPR-01-003-gemini][medium]` (close-out) `scripts/intel-query.sh:31` — Unsafe manual JSON construction (near-agreement with TPR-01-002-codex close-out).
  Resolved: Fixed on 2026-04-12. Same fix as TPR-01-002-codex close-out (python3 json.dump).
- [x] `[TPR-01-001-codex][medium]` (close-out-2) `query_graph.py:702` — GAP: `query_graph.py --json` (no command) crashes with IndexError; `--json` stripped before length check.
  Resolved: Fixed on 2026-04-12. Moved `--json` stripping before the `len(argv) < 2` check; returns JSON error in json_mode.
- [x] `[TPR-01-002-codex][low]` (close-out-2) `section-01-infrastructure.md:119` — DRIFT: Stale line references in 01.2 checklist (shifted after fixes).
  Resolved: Fixed on 2026-04-12. Removed fragile line numbers; use function names as stable identifiers with grep instructions.
- [x] `[TPR-01-001-gemini][informational]` (close-out-2) `scripts/intel-query.sh:1` — All 7 fixes verified successfully; no architectural violations found. Proof-of-work from thorough re-review pass.
- [x] `[TPR-01-001-codex][medium]` (close-out-3) `scripts/intel-query.sh:41` — GAP: `--timeout 0` disables GNU timeout, breaking bounded-call guarantee.
  Resolved: Fixed on 2026-04-12. Added minimum-of-1 guard: `$(( $2 > 0 ? $2 : 1 ))` so `--timeout 0` is clamped to 1s.
- [x] `[TPR-01-002-codex][low]` (close-out-3) `section-01-infrastructure.md:237` — DRIFT: test-all pass count stale (17120 vs 17140).
  Resolved: Fixed on 2026-04-12. Updated count to 17140 (current HEAD).
- [x] `[TPR-01-001-codex][medium]` (close-out-4) `scripts/intel-query.sh:43` — GAP: Bash octal trap in `--timeout` arithmetic — `$(( $2 ))` treats `08`/`09` as invalid octal.
  Resolved: Fixed on 2026-04-12. Added `10#` base-10 prefix: `$(( 10#$2 > 0 ? 10#$2 : 1 ))`.
- [x] `[TPR-01-001-gemini][high]` (close-out-4) `scripts/intel-query.sh:42` — GAP: Same octal trap as TPR-01-001-codex (near-agreement).
  Resolved: Fixed on 2026-04-12. Same fix as TPR-01-001-codex close-out-4.
- [x] `[TPR-01-002-gemini][low]` (close-out-4) `section-01-infrastructure.md:246` — DRIFT: test-all count 17140 vs 17142.
  Resolved: Rejected on 2026-04-12. Count 17140 is correct for committed HEAD; 17142 reflects uncommitted parallel-session tests in working tree.
- [x] `[TPR-01-001-codex][low]` (close-out-5) `section-01-infrastructure.md:90` — DRIFT: `--health-check` spelling throughout plan; actual CLI is positional `health-check`.
  Resolved: Fixed on 2026-04-12. Replaced all `--health-check` with `health-check` (except historical TPR finding titles).
- [x] `[TPR-01-002-codex][low]` (close-out-5) `section-01-infrastructure.md:53` — DRIFT: Ownership paragraph claims shell avoids starting Python; steps 2+4 invoke Python.
  Resolved: Fixed on 2026-04-12. Rewrote ownership paragraph to reflect actual orchestration-with-Python design.
- [x] `[TPR-01-003-codex][low]` (close-out-5) `section-01-infrastructure.md:100` — DRIFT: Checklist says `--human` is "passed through"; actually stripped locally.
  Resolved: Fixed on 2026-04-12. Changed to "Strip `--human` locally — omit `--json` in human mode."
- [x] `[TPR-01-001-gemini][informational]` (close-out-5) `scripts/intel-query.sh:43` — Confirms timeout arithmetic fix is complete and correct for all edge cases.

## 01.N Completion Checklist

- [x] `scripts/intel-query.sh` exists, executable, passes all 6 test scenarios (running, stopped, missing repo, missing venv, Bolt-not-ready, index-missing)
- [x] `query_graph.py` `health-check` command implemented and used by `intel-query.sh` step 4
- [x] `query_graph.py` issues fixed: driver error handling, driver.close() leak, _parse_flags validation, connection timeout
- [x] `query_graph.py` label-graph command implemented (not a stub)
- [x] `query_graph.py` --json output mode works for all commands
- [x] `query_graph.py` credentials read from env vars with current values as defaults
- [x] `scripts/intel-query.sh status` returns live graph stats including emptiness check
- [x] Output contract is JSON-by-default everywhere — no command returns unstructured text to stdout in default mode
- [x] No test regressions: `timeout 150 ./test-all.sh` (17140 passed, 0 failed)
- [ ] `/tpr-review` clean
- [ ] `/impl-hygiene-review` clean
- [ ] `/improve-tooling` **section-close sweep** — MANDATORY after both reviews are clean. Verify every subsection (01.1, 01.2) has an "improvements made" entry or a documented "no gaps" finding from its per-subsection retrospective. Look for cross-subsection patterns: command sequences repeated across subsections, integration failures with unhelpful messages, instrumentation that became obvious only after seeing both subsections. Implement immediately via separate `/commit-push` using valid conventional-commit types (`build`/`test`/`chore`/`ci`/`docs` — NOT `tools(...)`).
- [ ] **Plan annotation cleanup**: Run `plan-annotations.sh --cleanup-only --plan lang-intelligence` — remove any stale annotations
- [ ] **Plan sync**:
  - [ ] Update section 01 frontmatter `status` to `complete`
  - [ ] Update `index.md` section status
  - [ ] Update `00-overview.md` Quick Reference table — Section 01 status
  - [ ] Update `00-overview.md` mission success criteria checkboxes
  - [ ] Verify Section 02's `depends_on: [01]` is still accurate

**Exit Criteria:** `scripts/intel-query.sh search "type inference"` returns valid JSON with `status:ok` when Neo4j is running, and `scripts/intel-query.sh search "type inference"` returns `{"status":"unavailable","reason":"..."}` with exit 0 when Neo4j is stopped. All 6 test scenarios pass. `query_graph.py health-check` returns `{"status":"ok"}` when connected. `query_graph.py --json stats` returns structured JSON. `timeout 150 ./test-all.sh` green.
