---
reroute: true
name: "LLVM Isolation"
full_name: "LLVM Worker Subprocess Isolation"
status: active
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
json_protocol.rs, JsonTestOutcome, JsonFileSummary, JsonTestResult
```

---

### Section 02: Subprocess Orchestrator
**File:** `section-02-orchestrator.md` | **Status:** Not Started

```
subprocess, worker, isolation, process boundary
Command::new, current_exe, spawn, wait, try_wait
exit code, signal, SIGSEGV, SIGABRT, crash detection
llvm_backend.rs, llvm_worker.rs, run_file_llvm, orchestrator
WaitError, detect_crash, crash_summary, extract_framed_json
worker pool, bounded concurrency, parallel, WorkerPool
timeout, hang detection, kill, process tree
ORI_LLVM_CRASHED, weakened gate, gate reversion, test-all.sh
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
ORI_LLVM_CRASHED, weakened gate reversion verified
```

---

## Quick Reference

| ID | Title | File | Items | Tests |
|----|-------|------|-------|-------|
| 01 | JSON Output Protocol | `section-01-json-protocol.md` | 3 subsections | 11 unit + 5 integration |
| 02 | Subprocess Orchestrator | `section-02-orchestrator.md` | 4 subsections | 18 unit + 3 integration |
| 03 | Verification | `section-03-verification.md` | 4 subsections | Verification tasks (no new code) |
