---
section: "06"
title: "Verification"
status: not-started
reviewed: false
goal: "All 17 code journeys score 10.0/10 with zero regressions"
depends_on: ["01", "02", "03", "04", "05"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "06.1"
    title: "Re-run all journeys"
    status: not-started
  - id: "06.2"
    title: "Score validation"
    status: not-started
  - id: "06.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "06.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Verification

**Status:** Not Started
**Goal:** All 17 code journeys achieve 10.0/10 codegen quality scores. Zero regressions from the fixes in Sections 01-05. Overview updated with final scores.

**Context:** After implementing the 5 codegen polish fixes, re-run all 17 journeys to verify the improvements and confirm no regressions were introduced.

**Depends on:** Sections 01, 02, 03, 04, 05 (all fixes must land first).

---

## 06.1 Re-run all journeys

- [ ] Re-run all 17 code journeys with the current compiler (use `/code-journey` skill with `keep same scenarios`)
- [ ] Verify all 17 journeys return PASS on both eval and AOT paths
- [ ] Verify J07, J15, J16, J17 no longer have the specific findings:
  - J07: no unused `extractvalue` for inclusive field
  - J15: no option struct wrapping overhead, nounwind on main wrapper
  - J16: no dead aggregate loads, no sret identity copy, nounwind on `check_multi`
  - J17: no dead loads in lambda body, nounwind on `@_ori_main`
- [ ] Verify with release binary: `cargo b --release && timeout 150 ./test-all.sh` — release FastISel behavior may differ
- [ ] Run Valgrind on affected journeys: `timeout 150 diagnostics/valgrind-aot.sh plans/code-journeys/07-loops.ori plans/code-journeys/15-fat-nested-collections.ori plans/code-journeys/16-fat-ownership-transfer.ori plans/code-journeys/17-fat-closure-capture.ori`

---

## 06.2 Score validation

- [ ] All 17 journeys score 10.0/10 (or justify any remaining deviation)
- [ ] Previously-10.0 journeys (J01-J06, J08-J14) remain at 10.0
- [ ] Overview at `plans/code-journeys/overview.md` updated with final scores
- [ ] Score trend shows improvement from 9.90 to 10.0 overall average

---

## 06.R Third Party Review Findings

- None.

---

## 06.N Completion Checklist

- [ ] `timeout 150 ./test-all.sh` green (debug)
- [ ] `timeout 150 cargo b --release && timeout 150 ./test-all.sh` green (release)
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `timeout 150 ./fmt-all.sh` green (no formatting regressions)
- [ ] All 17 journey results files updated with 2026-03-XX date
- [ ] `plans/code-journeys/overview.md` reflects final scores
- [ ] No CRITICAL or HIGH findings in any journey
- [ ] Overall average score = 10.0/10
- [ ] Valgrind clean on affected journeys (J07, J15, J16, J17)

**Exit Criteria:** `grep '^score:' plans/code-journeys/*-results.md | grep -v '10.0'` returns empty (all scores are 10.0).
