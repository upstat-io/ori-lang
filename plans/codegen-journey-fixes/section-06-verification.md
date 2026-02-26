---
section: "06"
title: "Verification"
status: not-started
goal: "All 9 findings verified fixed, zero regressions, journey summary updated"
inspired_by: []
depends_on: ["01", "02", "03", "04", "05"]
sections:
  - id: "06.1"
    title: "Test Matrix"
    status: not-started
  - id: "06.2"
    title: "Code Journey Re-verification"
    status: not-started
  - id: "06.3"
    title: "Summary Update"
    status: not-started
  - id: "06.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Verification

**Status:** Not Started
**Goal:** All 9 open findings from the code journey summary are verified fixed. All journey programs produce identical eval/AOT results. The summary document is updated to reflect resolved status. Zero regressions across all test suites.

**Context:** The code journey summary tracks 10 findings (#1 already fixed, #2–#10 open). This section verifies that sections 01–05 have resolved all 9 open findings and updates the tracking document.

**Depends on:** All previous sections (01–05).

---

## 06.1 Test Matrix

Re-run all 7 journey programs through both eval and AOT paths. Verify findings are resolved.

- [ ] **Finding #2** (Nounwind indirect calls — §01.1):
  - Compile Journey 4 program → `_ori_apply` uses `invoke` for closure call
  - No UB: panicking closure correctly unwinds through invoke

- [ ] **Finding #3** (Nounwind monomorphized callees — §01.2):
  - Compile Journey 3 program → `_ori_main` uses `call` for `identity<int>`
  - Landing pad count reduced (no unnecessary landing pads for nounwind generics)

- [ ] **Finding #4** (Eager runtime declarations — §02.1):
  - Compile Journey 1 program → count `declare` statements
  - Only used runtime functions declared (≤5 for simple arithmetic)

- [ ] **Finding #5** (Dead unreachable blocks — §02.2):
  - Compile Journey 2 program → zero `unreachable` blocks
  - Compile Journey 4 program → zero `unreachable` blocks

- [ ] **Finding #6** (Non-capturing lambda trampolines — §03.1):
  - Compile Journey 4 program → no `_ori_partial` for non-capturing lambdas
  - Capturing lambdas still have trampolines

- [ ] **Finding #7** (Redundant match branches — §02.3):
  - Compile Journey 6 program → no single-instruction `br` blocks in match
  - Complex match arms still have separate blocks

- [ ] **Finding #8** (Opaque struct names — §04):
  - Compile Journey 5 program → `%ori.Point` in IR (not `%ori.3`)

- [ ] **Finding #9** (Trampoline missing nounwind — §03.2):
  - Compile Journey 4 program → nounwind trampolines have attribute
  - May-unwind trampolines do NOT have attribute

- [ ] **Finding #10** (cargo run strips LLVM — §05):
  - Run `cargo run -- run tests/spec/basics/let.ori` → binary still has LLVM
  - Run `ori build tests/spec/basics/let.ori` after → succeeds without `cargo bl`

---

## 06.2 Code Journey Re-verification

Re-run all 7 code journeys to verify eval/AOT behavioral equivalence.

- [ ] Journey 1 (bare arithmetic): eval=33, AOT=33
- [ ] Journey 2 (function calls): eval=11, AOT=11
- [ ] Journey 3 (generics): eval=17, AOT=17
- [ ] Journey 4 (closures): eval=16, AOT=16
- [ ] Journey 5 (structs): eval=7, AOT=7
- [ ] Journey 6 (pattern matching): eval=3, AOT=3
- [ ] Journey 7 (sum types): eval=37, AOT=37

- [ ] Run `./scripts/dual-exec-verify.sh` on all journey programs — 0 mismatches

---

## 06.3 Summary Update

Update `plans/code-journeys/summary.md` to reflect resolved findings.

- [ ] Mark all 9 findings as FIXED with dates and section references
- [ ] Update the "Recommended Fix Priority" table — all items resolved
- [ ] Update the "Deduplicated Findings" section — all items have FIXED status
- [ ] Add a "Resolution" section documenting what was done
- [ ] Update the "What Works Well" section if new positive observations emerge

---

## 06.4 Completion Checklist

- [ ] All 9 findings (#2–#10) verified fixed with specific evidence
- [ ] All 7 journey programs produce identical eval/AOT results
- [ ] `./scripts/dual-exec-verify.sh` passes on all journey programs
- [ ] `./test-all.sh` green
- [ ] `./llvm-test.sh` green
- [ ] `./clippy-all.sh` green
- [ ] `plans/code-journeys/summary.md` updated — 0 open findings
- [ ] No new warnings or regressions introduced

**Exit Criteria:** The code journey summary shows 10/10 findings resolved (1 previously fixed + 9 fixed by this plan). All journey programs produce correct, identical results on both eval and AOT paths. `dual-exec-verify.sh` reports 0 mismatches. All test suites (`test-all.sh`, `llvm-test.sh`, `clippy-all.sh`) pass with zero regressions.
