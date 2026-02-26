---
section: "06"
title: "Verification"
status: complete
goal: "All 9 findings verified fixed, zero regressions, journey summary updated"
inspired_by: []
depends_on: ["01", "02", "03", "04", "05"]
sections:
  - id: "06.1"
    title: "Test Matrix"
    status: complete
  - id: "06.2"
    title: "Code Journey Re-verification"
    status: complete
  - id: "06.3"
    title: "Summary Update"
    status: complete
  - id: "06.4"
    title: "Completion Checklist"
    status: complete
---

# Section 06: Verification

**Status:** Complete
**Goal:** All 9 open findings from the code journey summary are verified fixed. All journey programs produce identical eval/AOT results. The summary document is updated to reflect resolved status. Zero regressions across all test suites.

**Context:** The code journey summary tracks 10 findings (#1 already fixed, #2–#10 open). This section verifies that sections 01–05 have resolved all 9 open findings and updates the tracking document.

**Depends on:** All previous sections (01–05).

---

## 06.1 Test Matrix

Re-run all 7 journey programs through both eval and AOT paths. Verify findings are resolved.

- [x] **Finding #2** (Nounwind indirect calls — §01.1): (2026-02-26)
  - `_ori_apply` is NOT marked `nounwind` (correct — indirect call)
  - `_ori_main` uses `invoke fastcc i64 @_ori_apply(...)` with landing pad
  - No UB: panicking closure correctly unwinds through invoke

- [x] **Finding #3** (Nounwind monomorphized callees — §01.2): (2026-02-26)
  - `_ori_main` uses `call fastcc i64 @"_ori_identity$24m$24int"(i64 7)` (not invoke)
  - `_ori_identity$m$int` and `_ori_first$m$int_int` both have `nounwind` attribute
  - Zero landing pads in Journey 3 IR

- [x] **Finding #4** (Eager runtime declarations — §02.1): (2026-02-26)
  - Journey 1 program produces 0 `declare` statements (even better than ≤5)
  - Only `_ori_main` and `main` functions in IR

- [x] **Finding #5** (Dead unreachable blocks — §02.2): (2026-02-26)
  - Journey 2 IR: zero `unreachable` instructions
  - Journey 4 IR: zero `unreachable` instructions (landing pad uses `resume`, not `unreachable`)

- [x] **Finding #6** (Non-capturing lambda trampolines — §03.1): (2026-02-26)
  - Journey 4 IR: no `_ori_partial` functions
  - Lambda passed as `{ ptr @_ori___lambda_0, ptr null }` — direct function pointer, no trampoline

- [x] **Finding #7** (Redundant match branches — §02.3): (2026-02-26)
  - Simple match test: `match n { 0->10, 1->20, 2->30, _->40 }` emits `icmp eq` + `select` chain — no switch, no br blocks
  - Journey 6 retains phi/branch structure as expected (has guard — not select-eligible)

- [x] **Finding #8** (Opaque struct names — §04): (2026-02-26)
  - Journey 5 IR shows `%ori.Point = type { i64, i64 }` (not `%ori.3`)

- [x] **Finding #9** (Trampoline missing nounwind — §03.2): (2026-02-26)
  - Journey 4: `_ori___lambda_0` has `nounwind` attribute (#0)
  - Non-capturing fast path eliminates trampoline entirely; attribute propagation verified in §03 tests

- [x] **Finding #10** (cargo run strips LLVM — §05): (2026-02-26)
  - `cargo run -- run plans/code-journeys/journey1.ori` → exit=33 (LLVM present)
  - `cargo run -- build plans/code-journeys/journey1.ori -o /tmp/j1_test` → succeeds, exit=33

---

## 06.2 Code Journey Re-verification

Re-run all 7 code journeys to verify eval/AOT behavioral equivalence.

- [x] Journey 1 (bare arithmetic): eval=33, AOT=33 (2026-02-26)
- [x] Journey 2 (function calls): eval=11, AOT=11 (2026-02-26)
- [x] Journey 3 (generics): eval=17, AOT=17 (2026-02-26)
- [x] Journey 4 (closures): eval=16, AOT=16 (2026-02-26)
- [x] Journey 5 (structs): eval=7, AOT=7 (2026-02-26)
- [x] Journey 6 (pattern matching): eval=3, AOT=3 (2026-02-26)
- [x] Journey 7 (sum types): eval=37, AOT=37 (2026-02-26)

- [x] Run `./scripts/dual-exec-verify.sh` on all journey programs — 0 mismatches (2026-02-26)

---

## 06.3 Summary Update

Update `plans/code-journeys/summary.md` to reflect resolved findings.

- [x] Mark all 9 findings as FIXED with dates and section references (2026-02-26)
- [x] Update the "Recommended Fix Priority" table — all items resolved (2026-02-26)
- [x] Update the "Deduplicated Findings" section — all items have FIXED status (2026-02-26)
- [x] Add a "Resolution" section documenting what was done (2026-02-26)
- [x] Update the "What Works Well" section if new positive observations emerge (2026-02-26)

---

## 06.4 Completion Checklist

- [x] All 9 findings (#2–#10) verified fixed with specific evidence (2026-02-26)
- [x] All 7 journey programs produce identical eval/AOT results (2026-02-26)
- [x] `./scripts/dual-exec-verify.sh` passes on all journey programs (2026-02-26)
- [x] `./test-all.sh` green — 10,574 passed, 0 failed (2026-02-26)
- [x] `./llvm-test.sh` green — 1,016 passed (2026-02-26)
- [x] `./clippy-all.sh` green (2026-02-26)
- [x] `plans/code-journeys/summary.md` updated — 0 open findings (2026-02-26)
- [x] No new warnings or regressions introduced (2026-02-26)

**Exit Criteria:** The code journey summary shows 10/10 findings resolved (1 previously fixed + 9 fixed by this plan). All journey programs produce correct, identical results on both eval and AOT paths. `dual-exec-verify.sh` reports 0 mismatches. All test suites (`test-all.sh`, `llvm-test.sh`, `clippy-all.sh`) pass with zero regressions.
