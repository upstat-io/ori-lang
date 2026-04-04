---
section: "05"
title: "Verification"
status: in-progress
reviewed: true
goal: "Full test suite green, dual-execution parity verified, zero regressions"
inspired_by:
  - "Zig comptime/runtime dual execution verification"
  - "Swift SIL ARC test matrix pattern"
depends_on: ["01", "02", "03", "04", "04B"]
third_party_review:
  status: in-progress
  updated: 2026-04-04
sections:
  - id: "05.0"
    title: "Pre-verification checks"
    status: complete
  - id: "05.1"
    title: "Test matrix"
    status: complete
  - id: "05.2"
    title: "Dual-execution parity"
    status: in-progress
  - id: "05.3"
    title: "Regression verification"
    status: complete
  - id: "05.R"
    title: "Third Party Review Findings"
    status: in-progress
  - id: "05.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 05: Verification

**Status:** In Progress — 05.0, 05.1, 05.3 complete. 05.2 has 1 blocked item (BUG-04-031). 05.N awaiting TPR + hygiene review.
**Goal:** `./test-all.sh` green with 0 failures, 0 regressions, and dual-execution parity between interpreter and LLVM for all affected test files.

**Depends on:** Section 04 (bug fixes complete). Also: Section 01.R (stale comment cleanup) must be done before final verification.

---

## 05.0 Pre-verification Checks

- [x] Section 01.R stale comment cleanup completed (4 items): (2026-04-03) All stale comments removed — eh_personality.c, io/mod.rs, jit_recovery.rs docs now current.
- [x] No plan annotations remain in code from Sections 01-04 (2026-04-03) `plan-annotations.sh --count` returns 0 stale.
- [x] `cargo build --release` succeeds (2026-04-03) Release build completes in ~11s.
- [x] Section 04.H hygiene items completed (2026-04-03) All 3 items checked: dead code, banners, file bloat.

---

## 05.1 Test Matrix

Verify each category passes through LLVM in BOTH debug and release builds. For each, run:
- `timeout 30 cargo run -q -p oric --bin ori -- test --backend=llvm <file>`
- `timeout 30 cargo run -q -p oric --bin ori --release -- test --backend=llvm <file>`

- [x] **Panic recovery (catch)** (2026-04-03) Verified via inline tests (`/tmp/test_catch_full.ori`): direct panic caught, closure panic caught, nested catch (inner catches first), short-circuit inside catch, no-panic returns Ok. All 5 scenarios pass in debug AND release. File `tests/spec/patterns/catch.ori` has 7 LCFails from unresolved type variables (BUG-04-030 Root Cause A, not catch-specific).

- [x] **Short-circuit &&/||** (2026-04-03) Core semantics verified via inline tests: `false && panic()` ✓, `true || panic()` ✓, chained `&&` ✓, catch + short-circuit ✓. File `tests/spec/expressions/operators_logical.ori` has 39 LCFails: (1) BUG-04-031 — PHINode error when `&&` RHS has Option method calls, (2) BUG-04-032 — variable mutations in block expressions on evaluated side don't propagate. Both bugs filed in bug tracker.

- [x] **Integer safety** (`tests/spec/types/integer_safety.ori`) (2026-04-03):
  - All 30 tests pass through LLVM (debug + release) ✓
  - Division by zero panics correctly ✓, near-boundary valid ops do NOT panic ✓

- [x] **Bitwise** (`tests/spec/expressions/operators_bitwise.ori`) (2026-04-03):
  - All 43 tests pass through LLVM (debug + release) ✓

- [x] **COW nested collections** (`tests/spec/collections/cow/nested.ori`, `cow/sharing.ori`) (2026-04-03):
  - All 7 + 9 tests pass through LLVM (debug + release) ✓
  - `ORI_CHECK_LEAKS=1` reports 0 leaks on both files ✓

- [x] **Tuple/struct layout** (`tests/spec/types/struct_layout.ori`) (2026-04-03):
  - All 16 tests pass through LLVM (debug + release) ✓
  - No FATAL crash, no type confusion, no FastISel divergence ✓

- [x] **Coalesce** (`tests/spec/test_coalesce_copy.ori`) (2026-04-03):
  - All 17 tests pass through LLVM (debug + release) ✓

- [x] **Infinite range** (`tests/spec/traits/iterator/infinite_range.ori`) (2026-04-03):
  - All 14 tests pass through LLVM (debug + release) ✓

---

## 05.2 Dual-execution parity

- [x] Run `diagnostics/dual-exec-verify.sh tests/spec/types/integer_safety.ori` — ALL VERIFIED 30/30 (2026-04-03)
- [ ] Run `diagnostics/dual-exec-verify.sh tests/spec/expressions/operators_logical.ori` — ZERO VERIFICATIONS: 39 interpreter pass, 0 LLVM pass (all LCFail from BUG-04-031/BUG-04-032). No behavioral mismatches but no comparisons possible. <!-- blocked-by:BUG-04-031 -->
- [x] Run `diagnostics/dual-exec-verify.sh tests/spec/collections/cow/nested.ori` — ALL VERIFIED 7/7 (2026-04-03)
- [x] Run `diagnostics/dual-exec-verify.sh tests/spec/collections/cow/sharing.ori` — ALL VERIFIED 9/9 (2026-04-03)
- [x] Run `diagnostics/dual-exec-verify.sh tests/spec/types/struct_layout.ori` — ALL VERIFIED 16/16 (2026-04-03)
- [x] Run `diagnostics/dual-exec-verify.sh tests/spec/test_coalesce_copy.ori` — ALL VERIFIED 17/17 (2026-04-03)
- [x] Run `diagnostics/dual-exec-verify.sh tests/spec/traits/iterator/infinite_range.ori` — ALL VERIFIED 14/14 (2026-04-03)

---

## 05.3 Regression verification

- [x] `timeout 150 ./test-all.sh` — full suite green (2026-04-03): 16,533 passed, 0 failed, 154 skipped, 2656 LCFail
- [x] Rust unit tests: 7379 passed, 0 failed (2026-04-03)
- [x] Runtime tests: 367 passed, 0 failed (2026-04-03)
- [x] LLVM unit tests: 501 passed, 0 failed (2026-04-03)
- [x] AOT integration: 2096 passed, 0 failed (2026-04-03)
- [x] Ori spec (interpreter): 4409 passed, 0 failed (2026-04-03)
- [x] Ori spec (LLVM): 1781 passed, 0 failed, 2656 LCFail (2026-04-03). Note: plan estimated 3500+ passed / >2000 LCFail reduction, but 04B only addressed Root Cause A of 4+ root causes (BUG-04-030). Actual LCFails 2656 (baseline was 2639; +17 from TPR-04B-007 AOT test additions). No CRASHED.
- [x] `cargo build --release` succeeds (2026-04-03)
- [x] `./clippy-all.sh` passes (2026-04-03)

---

## 05.R Third Party Review Findings

- [x] `[TPR-05-001][medium]` [compiler/ori_llvm/src/codegen/function_compiler/lambda_mono/type_predicates.rs](/home/eric/projects/ori_lang/compiler/ori_llvm/src/codegen/function_compiler/lambda_mono/type_predicates.rs) — Missing regression coverage for Tuple/Map/Set branches in lambda mono type predicates.
  Resolved: Fixed on 2026-04-04. Added `test_multi_inst_tuple_lambda` and `test_multi_inst_map_lambda` AOT tests with corresponding `.ori` fixtures. Both tests exercise the Tag::Tuple and Tag::Map branches in `contains_var`, `contains_bound_var`, and `map_types_structural`. Both pass in debug.

- [x] `[TPR-05-002][medium]` [plans/jit-exception-handling/00-overview.md](/home/eric/projects/ori_lang/plans/jit-exception-handling/00-overview.md) — Stale overview contradicting section files.
  Resolved: Fixed on 2026-04-04. Updated 04B status to Complete, 05 to In Progress, dependency graph, Quick Reference table, and Live Test Results with post-fix verification data.

- [ ] `[TPR-05-003][medium]` `type_predicates.rs` — Missing `Tag::Set` AOT regression test for lambda mono type predicates. <!-- blocked-by:BUG-04-030 -->
  Validated on 2026-04-04. The `Tag::Set` branches exist in all four helpers but cannot be AOT-tested: polymorphic lambdas involving `Set<T>` crash in AOT (SIGSEGV, exit -139) due to unresolved monomorphization (BUG-04-030 Root Cause A/B). JIT path works (`cargo run --backend=llvm`), but `assert_aot_success` crashes. Test `test_multi_inst_set_lambda` must be added after BUG-04-030 is fixed.

---

## 05.N Completion Checklist

- [x] Section 01.R stale comment cleanup completed (2026-04-03) All 4 stale comments cleaned up.
- [x] Plan annotation cleanup: 0 stale annotations (2026-04-03)
- [x] Hygiene: all 04.H items completed (2026-04-03)
- [x] Test matrix covers all 8 categories (2026-04-03): 6/8 fully pass (integer, bitwise, COW, layout, coalesce, range). 2/8 have known LCFails: catch (BUG-04-030 Root Cause A), short-circuit (BUG-04-031, BUG-04-032). Core semantics verified via inline tests for catch and short-circuit.
- [x] Dual-execution parity verified for 6/7 files (2026-04-03): 93/93 tests verified. operators_logical.ori blocked by BUG-04-031 (all LCFail).
- [x] `timeout 150 ./test-all.sh` green (2026-04-03): 16,533 passed, 0 failed, no CRASHED.
- [x] LLVM spec tests: 1781 passed (2026-04-03). Baseline from Section 04 was ~1781. LCFails: 2656 (+17 from TPR-04B-007 AOT tests). 04B addressed Root Cause A only of 4+ root causes.
- [x] All previously-failing tests from Section 04 now pass (2026-04-03): integer_safety 30/30, operators_bitwise 43/43, struct_layout 16/16, coalesce 17/17, infinite_range 14/14, cow/nested 7/7, cow/sharing 9/9.
- [x] Debug AND release builds pass (2026-04-03)
- [x] `./clippy-all.sh` green (2026-04-03)
- [x] Bug tracker updated (2026-04-03): BUG-04-031 (PHINode short-circuit + Option methods), BUG-04-032 (short-circuit side-effect propagation) filed.
- [ ] `/tpr-review` passed -- independent Codex review clean
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** `./test-all.sh` green with 0 failures. `./clippy-all.sh` green. All previously-failing LLVM tests from Section 04 produce identical output in interpreter and LLVM (verified by dual-exec-verify.sh). Note: verify exact test count numbers at the start of this section -- the numbers in 05.3 are estimates that may have changed since plan creation.
