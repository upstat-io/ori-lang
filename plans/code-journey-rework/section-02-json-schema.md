---
section: "02"
title: "JSON Results Schema"
status: not-started
reviewed: false
goal: "Define the canonical JSON schema for code journey results that serves human reading, website rendering, and regression detection"
success_criteria:
  - "JSON schema documented in .claude/skills/code-journey/results-schema.json (JSON Schema format)"
  - "Schema includes provenance, execution, diagnostics, pipeline, findings, and verdict blocks"
  - "Finding vocabulary uses domain-specific kinds (memory_leak, rc_imbalance, etc.), not impl-hygiene kinds"
  - "Finding IDs use deterministic fingerprints, not sequential numbers"
  - "Schema supports both fast mode (tools only) and deep mode (tools + AI) — same shape, different completeness"
  - "Satisfies mission criterion: JSON schema documented, versioned, stable for website"
inspired_by:
  - "dual-exec-verify.sh --json output format (diagnostics/dual-exec-verify.sh:423-454)"
  - "Codex consultation: provenance block, artifact manifest, deterministic fingerprints"
  - "Gemini consultation: instruction ratio as first-class metric, trace file references"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Design the Schema"
    status: not-started
  - id: "02.2"
    title: "Write results-schema.json"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: JSON Results Schema

**Status:** Not Started
**Goal:** Define the canonical JSON results schema — the sole source of truth for code journey output. This schema replaces the deleted SCHEMA.md and is consumed by: (1) the orchestrator (Section 03) that produces it, (2) the markdown generator (Section 06), (3) the website (separate plan), and (4) regression detection tooling.

**Success Criteria:**
- [ ] Schema file exists at `.claude/skills/code-journey/results-schema.json`
- [ ] Schema validates against JSON Schema draft-07 or later
- [ ] All required blocks present: provenance, journey, execution, diagnostics, pipeline, findings, verdict
- [ ] Domain-specific finding vocabulary defined and documented
- [ ] Fast mode and deep mode produce the same schema shape
- [ ] Satisfies mission criteria for JSON schema stability

**Context:** Two independent consultations (tp-help) converged on: JSON as sole canonical artifact, trace files referenced by path not embedded, domain-specific finding vocabulary (NOT impl-hygiene LEAK/DRIFT which mean different things), deterministic fingerprints for finding IDs, and provenance metadata for reproducibility.

**Reference implementations:**
- **dual-exec-verify.sh** `diagnostics/dual-exec-verify.sh:423-454`: The only existing tool with JSON output — use as reference for structure conventions.

**Depends on:** Nothing — this is Phase 1 foundation.

---

## 02.1 Design the Schema

**File(s):** `.claude/skills/code-journey/results-schema.json`

Design the complete JSON structure based on consensus from two tp-help consultations:

- [ ] Define the top-level structure:
  ```json
  {
    "schema_version": "2.0",
    "provenance": {
      "timestamp": "ISO8601",
      "commit": "sha",
      "dirty": false,
      "compiler_version": "v2026.x.x",
      "analysis_mode": "fast|deep",
      "tool_versions": { "rc_stats": "1.0", "codegen_audit": "1.0" }
    },
    "journey": {
      "number": 1,
      "slug": "arithmetic",
      "theme": "I am arithmetic",
      "difficulty": "simple|moderate|complex",
      "features": ["arithmetic", "function_calls"],
      "source_file": "plans/code-journeys/01-arithmetic.ori",
      "source_sha256": "abc123..."
    },
    "execution": {
      "eval": { "exit_code": 33, "stdout": "", "stderr": "", "status": "pass|fail|crash|timeout" },
      "aot": { "exit_code": 33, "stdout": "", "stderr": "", "status": "pass|fail|crash|compile_fail|timeout" },
      "expected": 33,
      "parity": true,
      "stdout_match": true,
      "exit_code_match": true
    },
    "diagnostics": {
      "leak_check": {
        "status": "pass|fail|skip",
        "live_count": 0,
        "detail": "optional text from ORI_CHECK_LEAKS stderr"
      },
      "rc_stats": {
        "status": "pass|warn|fail|skip",
        "functions": [
          { "name": "main", "alloc": 2, "inc": 1, "dec": 1, "free": 0, "cow": 0, "balance": 2, "balanced": false }
        ],
        "totals": { "alloc": 5, "inc": 3, "dec": 3, "free": 1, "cow": 0, "balance": 4 }
      },
      "codegen_audit": {
        "status": "pass|warn|fail|skip",
        "errors": 0,
        "warnings": 0,
        "findings": [
          { "type": "error|warning", "message": "...", "function": "...", "detail": "..." }
        ]
      },
      "dual_exec": {
        "status": "pass|fail|skip",
        "verified": true,
        "mismatches": 0
      }
    },
    "pipeline": {
      "lexer": { "token_count": null, "trace_path": "lexer.txt", "trace_bytes": 1234 },
      "parser": { "node_count": null, "trace_path": "parser.txt", "trace_bytes": 5678 },
      "typeck": { "trace_path": "typeck.txt", "trace_bytes": 9012 },
      "arc_ir": { "trace_path": "arc_ir.txt", "trace_bytes": 3456 },
      "llvm_ir": {
        "line_count": 150,
        "function_count": 3,
        "ir_path": "llvm_ir.txt",
        "ir_bytes": 12345,
        "ir_sha256": "def456..."
      },
      "binary": { "size": 12345, "text_size": 4567, "rodata_size": 890 }
    },
    "findings": [
      {
        "id": "sha256_fingerprint_first_12_chars",
        "severity": "critical|high|medium|low|note",
        "category": "memory_leak|rc_imbalance|double_free|abi_mismatch|nounwind_missed|parity_mismatch|large_aggregate_load|redundant_option_wrap|landing_pad_overgen|dead_load|type_leak|codegen_anomaly",
        "source": "tool:rc_stats|tool:codegen_audit|tool:leak_check|tool:dual_exec|ai:ir_analysis",
        "location": "function_name:approximate_ir_line",
        "title": "Short description",
        "evidence": "Exact IR excerpt or tool output proving the finding",
        "filed_as": "BUG-XX-NNN|null"
      }
    ],
    "verdict": {
      "status": "clean|findings|blocked",
      "finding_count": 0,
      "critical_count": 0,
      "high_count": 0,
      "tool_gate": "pass|fail",
      "ai_gate": "pass|fail|skip"
    }
  }
  ```

- [ ] **Finding vocabulary** — define the complete set of domain-specific categories:
  - `memory_leak` — RC live count > 0, heap objects not freed
  - `rc_imbalance` — per-function inc/dec mismatch (potential leak or over-release)
  - `double_free` — dec without matching inc on same value
  - `abi_mismatch` — calling convention or arg count violation
  - `nounwind_missed` — function should have nounwind but doesn't
  - `parity_mismatch` — eval and AOT produce different results
  - `large_aggregate_load` — aggregate > 16 bytes loaded (FastISel bug)
  - `redundant_option_wrap` — Option struct materialized then immediately unwrapped
  - `landing_pad_overgen` — invoke+landingpad on nounwind function
  - `dead_load` — value loaded but never used
  - `type_leak` — unresolved type variable at codegen
  - `codegen_anomaly` — AI-detected anti-pattern not covered by other categories

- [ ] **Finding ID fingerprint** — define the deterministic hash:
  - Input: `category + location + title` (content-addressable)
  - Algorithm: SHA-256 of the concatenation, truncated to 12 hex chars
  - Purpose: stable across runs for regression diffing; same finding always gets same ID

- [ ] **Analysis mode distinction**:
  - `fast` mode: diagnostics block populated, findings from tools only, ai_gate = "skip"
  - `deep` mode: diagnostics + AI analysis, findings from both tools and AI

- [ ] **Subsection close-out (02.1)** — MANDATORY before starting 02.2:
  - [ ] All tasks above are `[x]` and schema design reviewed
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 02.2 Write results-schema.json

**File(s):** `.claude/skills/code-journey/results-schema.json`

- [ ] Write the JSON Schema file in JSON Schema draft-07 format
- [ ] Validate the schema against a sample journey result (manually construct a sample)
- [ ] Add a `"$comment"` field documenting that this replaces the deleted SCHEMA.md
- [ ] Add `"$comment"` fields in the findings section documenting the vocabulary

- [ ] **Subsection close-out (02.2)** — MANDATORY before marking section complete:
  - [ ] All tasks above are `[x]` and schema file validates
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [ ] `results-schema.json` exists and validates as JSON Schema
- [ ] Sample journey result validates against the schema
- [ ] Finding vocabulary documented with definitions
- [ ] Fast/deep mode distinction clear in schema
- [ ] Provenance block includes all fields needed for reproducibility
- [ ] `/tpr-review` — dual-source review of schema design
- [ ] `/impl-hygiene-review` — verify SSOT (this is the ONLY schema), no DRIFT
- [ ] `/improve-tooling` section-close sweep
