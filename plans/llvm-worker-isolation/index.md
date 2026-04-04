---
reroute: true
name: "LLVM Isolation"
full_name: "LLVM Worker Subprocess Isolation"
status: queued
order: 1
---

# LLVM Worker Subprocess Isolation Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: JSON Output Protocol
**File:** `section-01-json-protocol.md` | **Status:** Not Started

```
--json, json output, structured output, wire protocol
serde, Serialize, Deserialize, serde_json
TestOutcome, TestResult, FileSummary, TestSummary
BackendCrash, LlvmCompileFail, test outcome
result/mod.rs, commands/test.rs, print_test_summary
sentinel framing, ORI_JSON_BEGIN, ORI_JSON_END, stdout pollution
```

---

### Section 02: Subprocess Orchestrator
**File:** `section-02-orchestrator.md` | **Status:** Not Started

```
subprocess, worker, isolation, process boundary
Command::new, current_exe, spawn, wait, try_wait
exit code, signal, SIGSEGV, SIGABRT, crash detection
llvm_backend.rs, run_file_llvm, orchestrator
worker pool, bounded concurrency, parallel, rayon
timeout, hang detection, kill, process tree
```

---

### Section 03: Verification
**File:** `section-03-verification.md` | **Status:** Not Started

```
test matrix, integration test, regression
test-all.sh, exit code, crash detection
performance, overhead, wall clock, subprocess spawn
dual execution, interpreter, LLVM backend
BackendCrash, LlvmCompileFail, gate integrity
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | JSON Output Protocol | `section-01-json-protocol.md` |
| 02 | Subprocess Orchestrator | `section-02-orchestrator.md` |
| 03 | Verification | `section-03-verification.md` |
