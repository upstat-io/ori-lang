---
section: "05"
title: "Verification"
status: not-started
goal: "All 17 code journeys score 10.0/10, test-all.sh green, Valgrind clean"
depends_on: ["01", "02", "03", "04"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Re-run All 17 Code Journeys"
    status: not-started
  - id: "05.2"
    title: "Behavioral Equivalence"
    status: not-started
  - id: "05.3"
    title: "Safety Verification"
    status: not-started
  - id: "05.4"
    title: "Regression Suite"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "05.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Verification

**Status:** Not Started
**Goal:** Prove the entire system works: all 17 code journeys at 10.0/10, all tests green, all Valgrind checks clean.

**Depends on:** Sections 01-04 (all fixes landed and test matrix passing).

---

## 05.1 Re-run All 17 Code Journeys

- [ ] Run `/code-journey rerun existing scenarios` to re-execute all 17 journeys
- [ ] J1-J13: Verify all remain at 10.0/10 (no regressions from the fixes)
- [ ] J14: Verify score improves from 9.4 to 10.0 (control_flow: 8/10 from redundant `br` + ir_quality: 8/10 from duplicate ptrtoint -- both fixed by Section 03)
- [ ] J15: Verify score improves from 6.2 to 10.0 (double-free fixed by Section 01, landing pad double-drop fixed by Section 01.3)
- [ ] J16: Verify score improves from 9.4 to 10.0 (other_findings: 7/10 from HIGH-1 aggregate pattern + attributes_safety: 9/10 from LOW-2 invoke-to-nounwind -- fixed by Section 03)
- [ ] J17: Verify score improves from 3.0 to 10.0 (closure capture crash fixed by Section 02)
- [ ] All 17 journeys score 10.0/10
- [ ] Update `plans/code-journeys/overview.md` with new results
- [ ] Update individual journey results files (`plans/code-journeys/1[4-7]-*-results.md`) with new IR, scores, and finding status changes

---

## 05.2 Behavioral Equivalence

- [ ] Run `diagnostics/dual-exec-verify.sh` on ALL spec tests -- 0 mismatches between eval and AOT
- [ ] Run `diagnostics/dual-exec-verify.sh` on ALL fat matrix test programs -- 0 mismatches
- [ ] Run `diagnostics/dual-exec-verify.sh` on ALL code journey .ori files -- 0 mismatches

---

## 05.3 Safety Verification

- [ ] Run `diagnostics/valgrind-aot.sh` on all 17 journey .ori files -- "0 errors from 0 contexts" for each
- [ ] Run `diagnostics/valgrind-aot.sh tests/valgrind/fat_matrix/` -- "0 errors" for every fat matrix test
- [ ] Run `ORI_CHECK_LEAKS=1` on all 17 journey AOT binaries -- no leak reports
- [ ] Run `ORI_TRACE_RC=1` on J15 journey (the former double-free) -- verify balanced RC operations

---

## 05.4 Regression Suite

- [ ] `timeout 150 ./test-all.sh` green (all existing tests pass) -- debug build
- [ ] `timeout 150 cargo b --release && timeout 150 ./test-all.sh` green -- release build
- [ ] `timeout 150 ./clippy-all.sh` green (no new warnings)
- [ ] `timeout 150 ./fmt-all.sh` passes (code formatted)
- [ ] `timeout 150 cargo test -p ori_llvm fat_matrix` -- all matrix tests pass
- [ ] No new `#[ignore]` tests introduced
- [ ] No new `#[allow(clippy)]` without justification
- [ ] No new files over 500 lines (per coding guidelines)

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [ ] All 17 code journeys score 10.0/10
- [ ] Overall journey average: 10.0/10
- [ ] `dual-exec-verify.sh` reports 0 mismatches on all test suites
- [ ] Valgrind clean on all journeys and fat matrix tests
- [ ] `ORI_CHECK_LEAKS=1` clean on all journey binaries
- [ ] `./test-all.sh` green (debug)
- [ ] `./test-all.sh` green (release)
- [ ] `./clippy-all.sh` green
- [ ] `./fmt-all.sh` green
- [ ] `plans/code-journeys/overview.md` updated with final scores
- [ ] Individual journey results files (`14-*`, `15-*`, `16-*`, `17-*`) updated with new IR and scores
- [ ] `plans/fat-pointer-hardening/section-04-test-matrix.md` coverage matrix fully populated (no `---` cells)
- [ ] Bug entries in journey results files (C15-1, C15-2, C17) status changed from OPEN to FIXED

**Exit Criteria:** `/code-journey --summary` shows all 17 journeys at 10.0/10 AND `./test-all.sh` passes with 0 failures in both debug and release AND `valgrind-aot.sh` reports 0 errors across all test programs.
