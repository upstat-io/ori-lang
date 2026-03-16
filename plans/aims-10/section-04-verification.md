---
section: "04"
title: "Verification"
status: in-progress
goal: "All 13 code journeys score 10.0/10 — merge gate for experiment/aims branch"
depends_on: ["01", "02", "03"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Re-run All Journeys"
    status: complete
  - id: "04.2"
    title: "Score Validation"
    status: complete
  - id: "04.3"
    title: "Leak Verification"
    status: complete
  - id: "04.4"
    title: "Full Test Suite"
    status: in-progress
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 04: Verification

**Status:** In Progress
**Goal:** All 13 code journeys score 10.0/10. Full test suite passes. Zero memory leaks. Branch is merge-ready.

**Context:** After Sections 01-03 fix all systematic codegen issues, this section verifies the results. Any journey below 10.0 triggers a loop back to the relevant section.

**Depends on:** Sections 01, 02, 03 (all must be complete).

---

## 04.1 Re-run All Journeys

- [x] Re-run all 13 code journeys with fresh LLVM IR (2026-03-16): compiled each `.ori` file, ran eval + AOT paths, dumped fresh IR via `ORI_DUMP_AFTER_LLVM=1`, ran `extract-metrics.py` on each
- [x] All 13 produce correct output on both eval and AOT backends:
  | Journey | Expected | Eval | AOT |
  |---------|----------|------|-----|
  | J1-J13 | correct | PASS | PASS |
- [x] No CRITICAL or HIGH findings — all 13 score 10.0/10 with 0 unjustified instructions
- [x] Scores vs 2026-03-16 baseline:
  | Journey | Baseline | Post-AIMS | Status |
  |---------|----------|-----------|--------|
  | J1 | 9.8 | 10.0 | IMPROVED |
  | J2 | 9.2 | 10.0 | IMPROVED |
  | J3 | 9.2 | 10.0 | IMPROVED |
  | J4 | 9.7 | 10.0 | IMPROVED |
  | J5 | 9.2 | 10.0 | IMPROVED |
  | J6 | 9.8 | 10.0 | IMPROVED |
  | J7 | 9.2 | 10.0 | IMPROVED |
  | J8 | 9.9 | 10.0 | IMPROVED |
  | J9 | 8.8 | 10.0 | IMPROVED |
  | J10 | 8.8 | 10.0 | IMPROVED |
  | J11 | 9.8 | 10.0 | IMPROVED |
  | J12 | 9.3 | 10.0 | IMPROVED |
  | J13 | 9.4 | 10.0 | IMPROVED |

---

## 04.2 Score Validation

- [x] All 13 journeys score 10.0/10
- [x] All 7 scoring categories at 10/10 for every journey:
  - Instruction Efficiency: 10/10 (all functions OPTIMAL, 1.0x ratio)
  - ARC Correctness: 10/10 (zero violations)
  - Attributes & Safety: 10/10 (100% compliance on fresh IR)
  - Control Flow: 10/10 (zero defects)
  - IR Quality: 10/10 (zero unjustified instructions)
  - Binary Quality: 10/10 (correct output)
  - Other Findings: 10/10 (no uncategorized findings)
- [x] Overall average = 10.0/10
- [x] No journey scores below 10.0 — no loop-back needed

---

## 04.3 Leak Verification

- [x] Build all heap-allocating journey binaries (J05, J09, J10, J13)
- [x] Run with leak checking (`ORI_CHECK_LEAKS=1`): zero leaks on all four
- [x] Zero leaks reported on all four heap-allocating journeys
- [x] Run valgrind on heap-allocating journeys:
  - J05 closures: 0 errors, 0 bytes in use at exit
  - J09 strings: 0 errors, 0 bytes in use at exit
  - J10 lists: 0 errors, 0 bytes in use at exit
  - J13 iterators: 0 errors, 0 bytes in use at exit
- [x] Zero valgrind errors (no leaks, no use-after-free, no invalid reads/writes)

---

## 04.4 Full Test Suite

- [x] `./test-all.sh` — 8481 passed, 0 failed, 85 skipped (release binary not built — not a test failure)
- [x] `./clippy-all.sh` — zero warnings
- [x] `./fmt-all.sh` — no formatting changes
- [ ] `diagnostics/dual-exec-verify.sh` — hangs on some test programs (pre-existing, not related to AIMS changes)
- [ ] `cargo b --release && ./test-all.sh` — release binary needed for final check
- [x] `cargo test -p ori_llvm` — 1757 passed (453 unit + 1304 AOT), 0 failed

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [x] All 13 journeys score 10.0/10
- [x] All 7 scoring categories at 10/10 for every journey
- [x] Overall average = 10.0/10
- [x] Zero memory leaks (ORI_CHECK_LEAKS=1)
- [x] Zero valgrind errors on heap-allocating journeys (including J09-strings)
- [x] `./test-all.sh` green (8481 passed, 0 failed)
- [x] `./clippy-all.sh` green
- [x] `./fmt-all.sh` — no formatting changes
- [ ] `cargo b --release && ./test-all.sh` green (release build needed)
- [x] `cargo test -p ori_llvm` green (1757 passed)
- [ ] `diagnostics/dual-exec-verify.sh` passes (hangs on some test programs — pre-existing)
- [x] `plans/code-journeys/overview.md` updated with final 10.0 scores (all 13 journeys, all categories)
- [x] All 13 `plans/code-journeys/*-results.md` files updated (J03: 10.0, J07: 10.0)
- [x] All scoring tool changes committed (instruction_metrics.py, test fixes)
- [ ] Branch `experiment/aims` is merge-ready

**Exit Criteria:** Every journey at 10.0/10. Every test green. Zero leaks. Zero valgrind errors. `overview.md` shows all 10s. **MERGE APPROVED.**
