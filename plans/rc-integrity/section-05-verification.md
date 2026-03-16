---
section: "05"
title: "Verification & Merge Gate"
status: not-started
goal: "Zero leaks, zero regressions, all 16 journeys 10/10, all matrix tests green — branch merge-ready"
depends_on: ["01", "02", "03", "04"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Full Test Suite"
    status: not-started
  - id: "05.2"
    title: "Leak Verification"
    status: not-started
  - id: "05.3"
    title: "Journey Score Verification"
    status: not-started
  - id: "05.4"
    title: "Release Build"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "05.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Verification & Merge Gate

**Status:** Not Started
**Goal:** Comprehensive verification that all fixes, tests, and journeys are complete. Zero leaks, zero regressions, all 16 journeys score 10/10, all matrix tests pass. Branch is merge-ready.

**Depends on:** All of Sections 01-04.

---

## 05.1 Full Test Suite

- [ ] `timeout 150 ./test-all.sh` — all tests pass, 0 failures
- [ ] `./clippy-all.sh` — zero warnings
- [ ] `./fmt-all.sh` — no formatting changes
- [ ] `cargo test -p ori_llvm --test aot` — all AOT tests pass with zero leaks
- [ ] `diagnostics/dual-exec-verify.sh` — all spec tests verified (no behavioral mismatches)
- [ ] Verify no new `#[ignore]` attributes were added to suppress failures (search: `grep -r '#\[ignore\]' compiler/ori_llvm/tests/`)
- [ ] Verify no new `#skip` attributes were added to spec tests to mask leaks

---

## 05.2 Leak Verification

- [ ] **Positive control:** Create a deliberately leaking program, verify `ORI_CHECK_LEAKS=1` detects it (exit code 2). This ensures the detection infrastructure was not accidentally disabled by Section 02 fixes.
- [ ] All 16 code journey binaries run with `ORI_CHECK_LEAKS=1` — zero leaks
- [ ] Valgrind on heap-allocating journeys (J5, J9, J10, J13, J14, J15, J16) — zero errors, zero bytes at exit
- [ ] All 66+ matrix tests pass with `ORI_CHECK_LEAKS=1` — zero leaks
- [ ] `ORI_TRACE_RC=1` on a representative sample shows balanced alloc/free

---

## 05.3 Journey Score Verification

- [ ] All 13 original journeys score 10/10 (no regression from AIMS baseline)
- [ ] All 3 new journeys (J14-J16) score 10/10 (if below 10/10, the blocking issue must be fixed before merge)
- [ ] `plans/code-journeys/overview.md` updated with J14-J16 entries
- [ ] All 16 `*-results.md` files current

---

## 05.4 Release Build

- [ ] `cargo b && timeout 150 ./test-all.sh` — debug build, all tests pass (baseline)
- [ ] `cargo b --release` — release build succeeds with no new warnings
- [ ] `cargo b --release && timeout 150 ./test-all.sh` — release build, all tests pass (release LLVM optimizations can change drop ordering vs debug FastISel)
- [ ] Run all 16 journey `.ori` files with `ori build --release` + execute — correct results
- [ ] Run all 16 journey release binaries with `ORI_CHECK_LEAKS=1` — zero leaks
- [ ] Valgrind on release binary for at least 3 heap-allocating journeys (J14, J15, J16)

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [ ] `./test-all.sh` green (12,900+ tests, 0 failures)
- [ ] `./clippy-all.sh` green
- [ ] `cargo test -p ori_llvm --test aot` — 1383+ tests (1317 existing + 66 matrix/guard), 0 failures, 0 leaks
- [ ] All 16 journeys score 10/10
- [ ] All 16 journeys leak-free (ORI_CHECK_LEAKS + valgrind)
- [ ] `cargo b --release && ./test-all.sh` green
- [ ] `diagnostics/dual-exec-verify.sh` passes (with per-test timeouts)
- [ ] No new `#[ignore]` or `#skip` attributes added to suppress failures
- [ ] Branch `experiment/aims` is merge-ready

**Exit Criteria:** Zero leaks in any AOT test or journey. Zero regressions in any existing test. All 16 journeys at 10/10. Matrix tests guard against future regressions across the full cross-product of value types, operations, and contexts. The branch is ready to merge with confidence.
