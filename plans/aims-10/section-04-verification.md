---
section: "04"
title: "Verification"
status: not-started
goal: "All 13 code journeys score 10.0/10 — merge gate for experiment/aims branch"
depends_on: ["01", "02", "03"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Re-run All Journeys"
    status: not-started
  - id: "04.2"
    title: "Score Validation"
    status: not-started
  - id: "04.3"
    title: "Leak Verification"
    status: not-started
  - id: "04.4"
    title: "Full Test Suite"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Verification

**Status:** Not Started
**Goal:** All 13 code journeys score 10.0/10. Full test suite passes. Zero memory leaks. Branch is merge-ready.

**Context:** After Sections 01-03 fix all systematic codegen issues, this section verifies the results. Any journey below 10.0 triggers a loop back to the relevant section.

**Depends on:** Sections 01, 02, 03 (all must be complete).

---

## 04.1 Re-run All Journeys

- [ ] Re-run all 13 code journeys using the `/code-journey` skill (each journey individually for full scoring)
- [ ] All 13 produce correct output on both eval and AOT backends
- [ ] No CRITICAL or HIGH findings in any journey
- [ ] Compare scores to 2026-03-16 baseline:
  | Journey | Baseline | Target |
  |---------|----------|--------|
  | J1 | 9.8 | 10.0 |
  | J2 | 9.2 | 10.0 |
  | J3 | 9.2 | 10.0 |
  | J4 | 9.7 | 10.0 |
  | J5 | 9.2 | 10.0 |
  | J6 | 9.8 | 10.0 |
  | J7 | 9.2 | 10.0 |
  | J8 | 9.9 | 10.0 |
  | J9 | 8.8 | 10.0 |
  | J10 | 8.8 | 10.0 |
  | J11 | 9.8 | 10.0 |
  | J12 | 9.3 | 10.0 |
  | J13 | 9.4 | 10.0 |

---

## 04.2 Score Validation

- [ ] All 13 journeys score 10.0/10
- [ ] All 7 scoring categories at 10/10 for every journey:
  - Instruction Efficiency: 10/10 (all functions OPTIMAL, 1.0x ratio)
  - ARC Correctness: 10/10 (zero violations — already achieved)
  - Attributes & Safety: 10/10 (100% compliance)
  - Control Flow: 10/10 (zero defects)
  - IR Quality: 10/10 (zero unjustified instructions)
  - Binary Quality: 10/10 (correct output — already achieved)
  - Other Findings: 10/10 (no uncategorized findings)
- [ ] Overall average = 10.0/10
- [ ] If any journey scores below 10.0:
  1. Identify which category is below 10
  2. Run `.claude/skills/code-journey/extract-metrics.py` to get the exact gap
  3. Go back to the relevant section (01/02/03) and fix it
  4. Re-run the failing journey
  5. Repeat until 10.0

---

## 04.3 Leak Verification

- [ ] Build all heap-allocating journey binaries:
  ```bash
  for j in 05-closures 09-strings 10-lists 13-iterators; do
    ./target/debug/ori build plans/code-journeys/$j.ori -o /tmp/${j}_binary
  done
  ```
- [ ] Run with leak checking:
  ```bash
  for j in 05-closures 09-strings 10-lists 13-iterators; do
    echo "=== $j ==="
    ORI_CHECK_LEAKS=1 /tmp/${j}_binary; echo "exit: $?"
  done
  ```
- [ ] Zero leaks reported on all four heap-allocating journeys
- [ ] Run valgrind on heap-allocating journeys (including J09-strings):
  ```bash
  for j in 05-closures 09-strings 10-lists 13-iterators; do
    echo "=== $j ==="
    timeout 30 valgrind --leak-check=full --error-exitcode=1 /tmp/${j}_binary 2>&1 | tail -20
  done
  ```
- [ ] Zero valgrind errors (no leaks, no use-after-free, no invalid reads/writes)

---

## 04.4 Full Test Suite

- [ ] `timeout 150 ./test-all.sh` — zero failures
- [ ] `./clippy-all.sh` — zero warnings
- [ ] `./fmt-all.sh` — no formatting changes
- [ ] `timeout 300 diagnostics/dual-exec-verify.sh` — eval == AOT for all spec tests
- [ ] `cargo b --release && timeout 150 ./test-all.sh` — release build also passes
- [ ] `timeout 150 cargo test -p ori_llvm` — LLVM crate tests pass (includes AOT tests)

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] All 13 journeys score 10.0/10
- [ ] All 7 scoring categories at 10/10 for every journey
- [ ] Overall average = 10.0/10
- [ ] Zero memory leaks (ORI_CHECK_LEAKS=1)
- [ ] Zero valgrind errors on heap-allocating journeys (including J09-strings)
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] `./fmt-all.sh` — no formatting changes
- [ ] `cargo b --release && timeout 150 ./test-all.sh` green
- [ ] `timeout 150 cargo test -p ori_llvm` green
- [ ] `timeout 300 diagnostics/dual-exec-verify.sh` passes
- [ ] `plans/code-journeys/overview.md` updated with final 10.0 scores
- [ ] All 13 `plans/code-journeys/*-results.md` files updated
- [ ] All scoring tool changes (`.claude/skills/code-journey/{attribute,control_flow,instruction}_metrics.py`) committed
- [ ] Branch `experiment/aims` is merge-ready

**Exit Criteria:** Every journey at 10.0/10. Every test green. Zero leaks. Zero valgrind errors. `overview.md` shows all 10s. **MERGE APPROVED.**
