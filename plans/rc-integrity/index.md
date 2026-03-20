---
reroute: true
name: "RC Integrity"
full_name: "RC Integrity: Leak-Free Codegen & Matrix Regression Guard"
status: active
order: 1
---

# RC Integrity Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Tooling — Leak Detection Infrastructure
**File:** `section-01-tooling.md` | **Status:** Complete

```
ORI_CHECK_LEAKS, ori_check_leaks, leak detection, main wrapper
test-all.sh, assert_aot_success, compile_and_run_capture
AOT leak check, runtime leak report, RC_LIVE_COUNT
exit code 2, leak attribution, alloc_registry
```

---

### Section 02: Fix All Pre-Existing Leaks
**File:** `section-02-leak-fixes.md` | **Status:** Complete

```
RcDec, FatValue, RcPointer, Aggregate
struct drop, slice cleanup, string concat loop
is_consuming_primop, emit_last_use_decs, edge_cleanup
ori_str_concat, ori_rc_dec, ori_buffer_rc_dec
slice leak, struct with heap field, list equality
```

---

### Section 03: Code Journeys — Expanded Coverage
**File:** `section-03-journeys.md` | **Status:** Complete

```
code journey, string builder, loop accumulation
heap reassignment, COW loop, FatValue loop
journey 14, journey 15, journey 16
.claude/skills/code-journey/extract-metrics.py
dual-exec-verify.sh, behavioral equivalence
```

---

### Section 04: Matrix Testing — Regression Guard
**File:** `section-04-matrix-testing.md` | **Status:** In Progress

```
matrix test, regression guard, leak matrix
valgrind, ORI_CHECK_LEAKS, ORI_TRACE_RC
AOT leak test, string pattern, list pattern, map pattern
struct pattern, slice pattern, closure capture pattern
while loop, match arm, nested pattern, loop pattern
cross-product, combinatorial, narrowing bands
journey guard, rc_matrix.rs, journey_guard.rs
```

---

### Section 05: Verification & Merge Gate
**File:** `section-05-verification.md` | **Status:** In Progress

```
10/10, code journey scores, merge gate
test-all.sh, clippy, fmt, release build
zero leaks, zero valgrind errors, zero regressions
dual-exec-verify, behavioral equivalence
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Tooling — Leak Detection Infrastructure | `section-01-tooling.md` |
| 02 | Fix All Pre-Existing Leaks | `section-02-leak-fixes.md` |
| 03 | Code Journeys — Expanded Coverage | `section-03-journeys.md` |
| 04 | Matrix Testing — Regression Guard | `section-04-matrix-testing.md` |
| 05 | Verification & Merge Gate | `section-05-verification.md` |
