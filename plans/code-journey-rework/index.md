---
reroute: true
name: "Journey Rework"
full_name: "Code Journey Rework: From Broken Scoring to Real Bug-Finding"
status: active
order: 11
---

# Code Journey Rework Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Tear Down Broken Pipeline
**File:** `section-01-teardown.md` | **Status:** Not Started

```
Python pipeline, arc_metrics.py, rc_state.py, score.py, instruction_metrics.py,
effect_summaries.py, extract-metrics.py, SCHEMA.md, scoring, deterministic scoring,
code-journey SKILL.md, rescore-all.sh, control_flow_metrics.py, attribute_metrics.py,
binary_metrics.py, ir_parser.py, ir_parser_internal.py, ir_utils.py,
extract_ir_from_results.py, golden tests, test_arc_metrics.py
```

---

### Section 02: JSON Results Schema
**File:** `section-02-json-schema.md` | **Status:** Not Started

```
JSON schema, results format, schema_version, journey metadata, execution results,
diagnostics block, pipeline block, findings array, verdict, provenance,
artifact manifest, sha256, deterministic fingerprint, finding vocabulary,
memory_leak, rc_imbalance, double_free, abi_mismatch, nounwind_missed,
parity_mismatch, domain-specific finding kinds, fast mode, deep mode
```

---

### Section 03: Orchestrator Skill
**File:** `section-03-orchestrator.md` | **Status:** Not Started

```
SKILL.md, orchestrator, main agent, background agent, phase capture,
diagnostic tool composition, rc-stats.sh, codegen-audit.sh, ORI_CHECK_LEAKS,
dual-exec-verify.sh, diagnose-aot.sh, ir-dump.sh, arc-dump.sh,
fast mode, deep mode, AI IR analysis, anti-pattern checklist,
temp directory, /tmp/journey_N, run list, --add, --infinity, --summary,
background Task agent, context conservation
```

---

### Section 04: Diagnostic Tool JSON Output
**File:** `section-04-tool-json.md` | **Status:** Not Started

```
rc-stats.sh --json, codegen-audit.sh --json, diagnose-aot.sh --json,
structured output, machine-readable, JSON mode, AWK table refactor,
codegen audit line format, report.rs, ORI_AUDIT_CODEGEN,
dual-exec-verify.sh --json, existing JSON format
```

---

### Section 05: Journey Corpus Expansion
**File:** `section-05-corpus-expansion.md` | **Status:** Not Started

```
J21-J30, leak-prone patterns, break-in-loop with fat values,
nested closures capturing heap, partial iterator consumption,
question-mark exit with heap types, COW mutations with early exit,
unwind cleanup, struct field reassignment, iterator chains,
fat_matrix coverage, valgrind test patterns, smoke baseline
```

---

### Section 06: Bug Integration & Markdown Generation
**File:** `section-06-integration.md` | **Status:** Not Started

```
/add-bug integration, auto-filing, bug tracker, subsystem mapping,
severity mapping, markdown generation, JSON-to-markdown, overview.md,
batch runner, run-journeys.sh, regression detection, CI integration,
journey gallery, recurring issues, resolved issues
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Tear Down Broken Pipeline | `section-01-teardown.md` |
| 02 | JSON Results Schema | `section-02-json-schema.md` |
| 03 | Orchestrator Skill | `section-03-orchestrator.md` |
| 04 | Diagnostic Tool JSON Output | `section-04-tool-json.md` |
| 05 | Journey Corpus Expansion | `section-05-corpus-expansion.md` |
| 06 | Bug Integration & Markdown Generation | `section-06-integration.md` |
