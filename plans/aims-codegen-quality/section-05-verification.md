---
section: "05"
title: "Verification"
status: not-started
goal: "All 13 journeys ≥ 9.8/10, simple journeys (J1, J4, J6, J8, J11) = 10.0/10, merge-ready"
depends_on: ["01", "02", "03", "04"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Re-run All Journeys"
    status: not-started
  - id: "05.2"
    title: "Score Validation"
    status: not-started
  - id: "05.3"
    title: "Full Test Suite"
    status: not-started
  - id: "05.4"
    title: "Rollback Plan"
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
**Goal:** Re-run all 13 code journeys, confirm all scores ≥ 9.8/10, simple journeys at 10.0/10, and full test suite passes. This is the merge gate.

**Context:** After all fixes in Sections 01-04, this section confirms the results. Any journey below 9.8 triggers a loop back to the relevant section.

**Depends on:** All of Sections 01-04.

---

## 05.1 Re-run All Journeys

- [ ] Run `/code-journey` on all 13 existing `.ori` files in `plans/code-journeys/`
  ```bash
  # Or use the batch rescore script:
  bash .claude/skills/code-journey/rescore-all.sh
  ```
- [ ] All 13 produce correct output on both eval and AOT
- [ ] No CRITICAL or HIGH findings in any journey
- [ ] Compare scores to baseline (2026-03-15 AIMS re-run)
- [ ] **Leak checks on all heap-allocating journeys** (not just J5):
  ```bash
  for j in 05-closures 09-strings 10-lists 13-iterators; do
    echo "=== $j ==="
    ORI_CHECK_LEAKS=1 /tmp/journey_${j%%-*}/binary
  done
  ```

---

## 05.2 Score Validation

**Target scores:**

| Journey | Current | Target | Category Targets |
|---------|---------|--------|-----------------|
| J1 | 9.8 | **10.0** | Attr 10, all others 10 |
| J2 | 9.2 | **9.8+** | Attr 9+, CF 10 |
| J3 | 8.9 | **9.8+** | Attr 8+, CF 9+, IR 10 |
| J4 | 9.7 | **10.0** | Attr 9+ |
| J5 | 8.5 | **9.8+** | Other 10 (no regressions), Attr 8+ |
| J6 | 9.7 | **10.0** | Attr 9+ |
| J7 | 9.2 | **9.8+** | CF 9+, IR 9+, Attr 9+ |
| J8 | 9.8 | **10.0** | Attr 10 |
| J9 | 8.8 | **9.8+** | Attr 8+, CF 9+, IR 9+ |
| J10 | 8.7 | **9.8+** | Attr 8+, CF 9+, Other 10 |
| J11 | 9.7 | **10.0** | Attr 9+ |
| J12 | 9.2 | **9.8+** | CF 9+ |
| J13 | 9.4 | **9.8+** | Attr 8+ |

- [ ] All 13 journeys score ≥ 9.8/10
- [ ] J1, J4, J6, J8, J11 score 10.0/10
- [ ] Overall average ≥ 9.8/10
- [ ] No journey has any category below 8/10

---

## 05.3 Full Test Suite

- [ ] `timeout 150 ./test-all.sh` passes (zero failures)
- [ ] `./clippy-all.sh` passes (zero warnings)
- [ ] `./fmt-all.sh` passes (no formatting changes)
- [ ] `diagnostics/dual-exec-verify.sh` passes (eval == AOT for all tests)
- [ ] `cargo b --release && timeout 150 ./test-all.sh` — release build also passes (FastISel differences between debug/release can cause hidden bugs; see llvm.md "MANDATORY: Test with Release Binary")

---

## 05.4 Rollback Plan

If any journey fails to reach 9.8 after all sections are complete:

1. **Identify which category is below target**: re-run `extract-metrics.py` to get per-category scores
2. **Check if the issue is in scoring vs codegen**: some scoring deductions may be debatable (e.g., a `br` in an entry block that serves as a phi-node landing pad is architecturally necessary, not waste)
3. **Adjust scoring weights if justified**: update `.claude/skills/code-journey/score.py` with comments explaining why a deduction is waived
4. **If codegen is genuinely suboptimal**: loop back to the relevant section (01-04) and add a new sub-task
5. **If 9.8 is unreachable for complex journeys (J9, J10, J13)**: accept 9.5+ with documented justification in the overview. The goal is "no regressions from old ARC" + "all systematic issues fixed."

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [ ] All 13 journeys PASS on both eval and AOT
- [ ] All 13 journeys score ≥ 9.8/10
- [ ] Simple journeys (J1, J4, J6, J8, J11) score 10.0/10
- [ ] Overall average ≥ 9.8/10
- [ ] No CRITICAL or HIGH findings in any journey
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] `cargo b --release && ./test-all.sh` green (release build)
- [ ] Zero memory leaks confirmed by `ORI_CHECK_LEAKS=1` on J5, J9, J10, J13
- [ ] `diagnostics/valgrind-aot.sh` clean on J5, J10, J13 (optional but recommended — catches double-free and use-after-free that `ORI_CHECK_LEAKS` misses)
- [ ] `plans/code-journeys/overview.md` updated with final scores
- [ ] All 13 `plans/code-journeys/*-results.md` files updated with new IR dumps and scores
- [ ] Branch `experiment/aims` is merge-ready

**Exit Criteria:** All 13 journeys at ≥ 9.8/10. Simple journeys at 10.0. `./test-all.sh` and `./clippy-all.sh` green. Zero leaks. Overview updated. **MERGE APPROVED.**
