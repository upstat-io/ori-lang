---
section: "05"
title: "Verification"
status: not-started
reviewed: false
goal: "Full test suite green, dual-execution parity verified, zero regressions"
inspired_by:
  - "Zig comptime/runtime dual execution verification"
  - "Swift SIL ARC test matrix pattern"
depends_on: ["01", "02", "03", "04", "04B"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.0"
    title: "Pre-verification checks"
    status: not-started
  - id: "05.1"
    title: "Test matrix"
    status: not-started
  - id: "05.2"
    title: "Dual-execution parity"
    status: not-started
  - id: "05.3"
    title: "Regression verification"
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
**Goal:** `./test-all.sh` green with 0 failures, 0 regressions, and dual-execution parity between interpreter and LLVM for all affected test files.

**Depends on:** Section 04 (bug fixes complete). Also: Section 01.R (stale comment cleanup) must be done before final verification.

---

## 05.0 Pre-verification Checks

- [ ] Section 01.R stale comment cleanup completed (4 items):
  - `compiler/ori_rt/src/eh_personality.c` line 418: remove step 3 (longjmp recovery)
  - `compiler/ori_rt/src/io/mod.rs` line 198-199: update `ori_assert` doc (no longjmp)
  - `compiler/ori_rt/src/io/mod.rs` line 7: update module doc (landingpads are primary)
  - `compiler/ori_rt/src/io/jit_recovery.rs` line 5: update module doc (legacy fallback)
- [ ] No plan annotations remain in code from Sections 01-04 (run `.claude/skills/impl-hygiene-review/plan-annotations.sh`)
- [ ] `cargo build --release` succeeds (release binary needed for dual-exec and verification)
- [ ] Section 04.H hygiene items completed (banners, dead code, file bloat -- tracked in `section-04-exposed-bugs.md` 04.H)

---

## 05.1 Test Matrix

Verify each category passes through LLVM in BOTH debug and release builds. For each, run:
- `timeout 30 cargo run -q -p oric --bin ori -- test --backend=llvm <file>`
- `timeout 30 cargo run -q -p oric --bin ori --release -- test --backend=llvm <file>`

- [ ] **Panic recovery (catch)** (test files: `tests/spec/expressions/catch/` if exists, else verify via inline tests):
  - `catch(expr: panic(msg:))` -- direct panic caught
  - `catch(expr: f())` where f is closure that panics -- indirect panic caught
  - `assert_panics(f: () -> overflow)` -- checked arithmetic in closure
  - Nested catch: `catch(expr: catch(expr: panic()))` -- inner catches first
  - Catch with RC cleanup: panicking expression with heap values on stack

- [ ] **Short-circuit &&/||** (`tests/spec/expressions/operators_logical.ori`):
  - `false && panic()` -- does NOT panic
  - `true || panic()` -- does NOT panic
  - `true && panic()` -- DOES panic
  - `false || panic()` -- DOES panic
  - Chained: `a && b && c` -- evaluates left-to-right with short-circuit
  - Inside catch: `catch(expr: false && panic())` returns Ok

- [ ] **Integer safety** (`tests/spec/types/integer_safety.ori`):
  - All 30 tests pass through LLVM (debug + release)
  - Division by zero panics correctly: `1/0`, `1%0`, `0/0`, `MIN/-1`
  - Near-boundary valid operations do NOT panic: `MAX/MAX`, `MAX%2`, `-7%3`

- [ ] **Bitwise** (`tests/spec/expressions/operators_bitwise.ori` -- not a Section 04 bug fix target; verify no regressions):
  - All shift tests pass through LLVM

- [ ] **COW nested collections** (`tests/spec/collections/cow/nested.ori`, `cow/sharing.ori`):
  - All tests pass through LLVM (debug + release)
  - `ORI_CHECK_LEAKS=1` reports 0 leaks on both files

- [ ] **Tuple/struct layout** (`tests/spec/types/struct_layout.ori`):
  - All tests pass through LLVM (no FATAL crash, no type confusion)
  - For-yield with padded tuples, structs, and enum payloads produces correct values
  - Debug AND release produce identical results (no FastISel divergence)

- [ ] **Coalesce** (`tests/spec/test_coalesce_copy.ori`):
  - All 17 tests pass through LLVM (debug + release)
  - `ORI_CHECK_LEAKS=1` reports 0 leaks

- [ ] **Infinite range** (`tests/spec/traits/iterator/infinite_range.ori`):
  - All 14 tests pass through LLVM (debug + release)
  - Negative step iteration produces `[0, -1, -2, -3, -4]`

---

## 05.2 Dual-execution parity

- [ ] Run `diagnostics/dual-exec-verify.sh tests/spec/types/integer_safety.ori` -- 0 mismatches
- [ ] Run `diagnostics/dual-exec-verify.sh tests/spec/expressions/operators_logical.ori` -- 0 mismatches
- [ ] Run `diagnostics/dual-exec-verify.sh tests/spec/collections/cow/nested.ori` -- 0 mismatches
- [ ] Run `diagnostics/dual-exec-verify.sh tests/spec/collections/cow/sharing.ori` -- 0 mismatches
- [ ] Run `diagnostics/dual-exec-verify.sh tests/spec/types/struct_layout.ori` -- 0 mismatches
- [ ] Run `diagnostics/dual-exec-verify.sh tests/spec/test_coalesce_copy.ori` -- 0 mismatches
- [ ] Run `diagnostics/dual-exec-verify.sh tests/spec/traits/iterator/infinite_range.ori` -- 0 mismatches

---

## 05.3 Regression verification

- [ ] `timeout 150 ./test-all.sh` — full suite green
- [ ] Rust unit tests: 7379+ passed, 0 failed
- [ ] Runtime tests: 367 passed, 0 failed
- [ ] LLVM unit tests: 501+ passed, 0 failed
- [ ] AOT integration: 2093+ passed, 0 failed
- [ ] Ori spec (interpreter): 4392+ passed, 0 failed
- [ ] Ori spec (LLVM): 3500+ passed, 0 failed, no CRASHED (04B reduces LCFails from 2639 baseline by >2000)
- [ ] `cargo build --release` succeeds
- [ ] `./clippy-all.sh` passes

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [ ] Section 01.R stale comment cleanup completed (4 items in `eh_personality.c`, `io/mod.rs`, `jit_recovery.rs`)
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh` returns 0 annotations
- [ ] Hygiene: all 04.H items completed (dead code, banners, file bloat)
- [ ] Test matrix covers all 8 categories (catch, short-circuit, integer, bitwise, COW, layout, coalesce, range)
- [ ] Dual-execution parity verified for all 7 affected test files
- [ ] `timeout 150 ./test-all.sh` green -- 0 failures, no CRASHED
- [ ] LLVM spec tests: verify count increased from baseline captured in Section 04
- [ ] All previously-failing tests from Section 04 now pass
- [ ] Debug AND release builds pass
- [ ] `./clippy-all.sh` green
- [ ] Bug tracker updated: new bugs filed for any remaining issues
- [ ] `/tpr-review` passed -- independent Codex review clean
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** `./test-all.sh` green with 0 failures. `./clippy-all.sh` green. All previously-failing LLVM tests from Section 04 produce identical output in interpreter and LLVM (verified by dual-exec-verify.sh). Note: verify exact test count numbers at the start of this section -- the numbers in 05.3 are estimates that may have changed since plan creation.
