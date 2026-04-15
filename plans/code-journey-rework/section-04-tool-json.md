---
section: "04"
title: "Diagnostic Tool JSON Output"
status: not-started
reviewed: false
goal: "Add --json output modes to key diagnostic tools so the orchestrator consumes structured data instead of parsing human-readable text"
success_criteria:
  - "rc-stats.sh --json produces per-function RC balance as JSON array"
  - "codegen-audit.sh --json produces structured findings as JSON"
  - "diagnose-aot.sh --json produces structured 7-section report as JSON"
  - "Existing human-readable output is unchanged (--json is additive)"
  - "Satisfies mission criterion: diagnostic tools produce structured output"
inspired_by:
  - "dual-exec-verify.sh --json (diagnostics/dual-exec-verify.sh:423-454) — already has JSON output"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "rc-stats.sh --json"
    status: not-started
  - id: "04.2"
    title: "codegen-audit.sh --json"
    status: not-started
  - id: "04.3"
    title: "diagnose-aot.sh --json"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Diagnostic Tool JSON Output

**Status:** Not Started
**Goal:** Add `--json` flags to the three key diagnostic tools so the orchestrator can consume structured output instead of fragile text parsing. `dual-exec-verify.sh` already has `--json` — use it as the reference pattern.

**Success Criteria:**
- [ ] `rc-stats.sh --json file.ori` outputs `{"functions": [...], "totals": {...}, "balanced": true|false}`
- [ ] `codegen-audit.sh --json file.ori` outputs `{"errors": N, "warnings": M, "findings": [...]}`
- [ ] `diagnose-aot.sh --json file.ori` outputs `{"sections": [...], "overall": "pass|fail"}`
- [ ] Human-readable mode is unchanged (--json is opt-in, not a breaking change)
- [ ] Satisfies mission criterion for structured diagnostic output

**Context:** The orchestrator (Section 03) needs to parse diagnostic tool output into the JSON results schema (Section 02). Without `--json` modes, the background agent would parse human-readable text — which is fragile (color codes, format changes, AWK table structure). Adding `--json` makes the tool outputs machine-readable and maintainable. This is an `/improve-tooling` improvement that benefits both code journeys and any future automation.

**Reference implementations:**
- **dual-exec-verify.sh** lines 423-454: Existing JSON output pattern. Uses `cat <<EOF` heredoc to emit JSON.

**Depends on:** Nothing — this is Phase 1 foundation, parallel with Section 02.

---

## 04.1 rc-stats.sh --json

**File(s):** `diagnostics/rc-stats.sh`

Add `--json` flag that outputs structured JSON instead of the AWK table.

- [ ] Add `--json` to the argument parser (alongside existing `--color`, `--no-color`, `--optimized`, `--help`)
- [ ] When `--json` is set, replace the AWK table output with JSON:
  ```json
  {
    "file": "file.ori",
    "functions": [
      { "name": "main", "alloc": 2, "inc": 1, "dec": 1, "free": 0, "cow": 0, "balance": 2, "balanced": false }
    ],
    "totals": { "alloc": 5, "inc": 3, "dec": 3, "free": 1, "cow": 0, "balance": 4 },
    "all_balanced": false
  }
  ```
- [ ] The JSON must include all the same data as the table (function name, alloc, inc, dec, free, cow, balance)
- [ ] Functions with zero RC/COW operations should still be omitted (matching current behavior)
- [ ] Exit code semantics unchanged: 0 = all balanced, 1 = imbalanced, 2 = usage error
- [ ] Test: run `rc-stats.sh --json plans/code-journeys/15-fat-nested-collections.ori` and validate JSON output
- [ ] Test: run `rc-stats.sh plans/code-journeys/15-fat-nested-collections.ori` (no --json) and verify human output unchanged

- [ ] **Subsection close-out (04.1)** — MANDATORY before starting 04.2:
  - [ ] All tasks above are `[x]` and --json mode produces valid JSON
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 04.2 codegen-audit.sh --json

**File(s):** `diagnostics/codegen-audit.sh`

Add `--json` flag that captures the structured audit output as JSON.

- [ ] Add `--json` to the argument parser
- [ ] When `--json` is set, parse the `codegen audit:` lines from stderr and emit JSON:
  ```json
  {
    "file": "file.ori",
    "errors": 0,
    "warnings": 0,
    "findings": [
      { "type": "error", "function": "main", "message": "RC balance mismatch", "detail": "+5 unmatched allocs" }
    ]
  }
  ```
- [ ] Parse the summary line (`codegen audit: ...: summary: N error, M warning`) for counts
- [ ] Individual finding lines follow the pattern `codegen audit: <file>:<function>: <type>: <message>`
- [ ] Exit code semantics unchanged: 0 = clean, 1 = findings, 2 = compilation failure
- [ ] Test: run on a file known to have audit warnings

- [ ] **Subsection close-out (04.2)** — MANDATORY before starting 04.3:
  - [ ] All tasks above are `[x]` and --json mode produces valid JSON
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 04.3 diagnose-aot.sh --json

**File(s):** `diagnostics/diagnose-aot.sh`

Add `--json` flag that wraps each of the 7 diagnostic sections as JSON.

- [ ] Add `--json` to the argument parser
- [ ] When `--json` is set, each section's result is captured as a JSON object:
  ```json
  {
    "file": "file.ori",
    "sections": [
      { "index": 1, "title": "Compilation", "status": "pass", "elapsed_seconds": 1.23 },
      { "index": 2, "title": "Execution", "status": "pass", "exit_code": 33, "stdout": "" },
      { "index": 3, "title": "Leak Check", "status": "pass", "live_count": 0 },
      { "index": 4, "title": "RC Stats", "status": "pass", "balanced": true },
      { "index": 5, "title": "LLVM IR", "status": "info", "line_count": 150, "path": "/tmp/..." },
      { "index": 6, "title": "Valgrind", "status": "skip" },
      { "index": 7, "title": "Disassembly", "status": "skip" }
    ],
    "overall": "pass"
  }
  ```
- [ ] Exit code semantics unchanged: 0 = all pass, 1 = failure, 2 = usage error
- [ ] Test: run on J01 (arithmetic) to verify JSON output

- [ ] **Subsection close-out (04.3)** — MANDATORY before marking section complete:
  - [ ] All tasks above are `[x]` and --json mode produces valid JSON
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] `rc-stats.sh --json` produces valid JSON with per-function RC data
- [ ] `codegen-audit.sh --json` produces valid JSON with structured findings
- [ ] `diagnose-aot.sh --json` produces valid JSON with 7-section results
- [ ] Human-readable modes unchanged for all three tools
- [ ] `dual-exec-verify.sh --json` already works (baseline reference)
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `diagnostics/self-test.sh` passes (if it tests these scripts)
- [ ] `/tpr-review` — dual-source review of tool changes
- [ ] `/impl-hygiene-review` — verify no DRIFT (JSON output matches human output data)
- [ ] `/improve-tooling` section-close sweep
