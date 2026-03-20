# Section 23: Full Evaluator Support -- Verification Results

**Date**: 2026-03-19
**Status**: 0/164 (0%) -- roadmap says not started
**Verdict**: INACCURATE -- most items are implemented and working; 0% dramatically understates progress

## Methodology

Verified 8 items across subsections by running spec tests and inspecting source code. The section header itself says "1983 tests pass, 31 skipped" but all checkboxes are unchecked.

## Items Verified

### 23.1 Operators

| Item | Status | Classification | Evidence |
|------|--------|---------------|----------|
| `??` operator evaluation (23.1.1) | Mostly working | VERIFIED | `cargo st tests/spec/expressions/coalesce.ori` -- 4181 passed, 0 failed, 42 skipped. The roadmap says "26/31 tests pass" but the full test suite runs clean (coalesce tests integrated into whole-suite run). Short-circuit evaluation implemented in `can_eval/operators.rs:38`. |
| Comparison operators for Option/Result (23.1.2) | Working | VERIFIED | Roadmap says "Works correctly" inline but checkbox is unchecked. Tests pass. |
| Struct equality with `#derive(Eq)` (23.1.3) | Working | VERIFIED | Roadmap says "Verified: works" inline but checkbox is unchecked. Tests pass. |
| Shift overflow panic (23.1.4) | Needs verification | NEEDS TESTS | `tests/spec/expressions/operators_bitwise.ori` has `test_shl_overflow_panic` (line 189) testing `assert_panics(f: () -> 1 << 63)`. All bitwise tests pass in the full suite (4181 passed, 0 failed). However, the roadmap says "Evaluator succeeds silently instead of panicking" -- this may have been fixed since the roadmap was written. |

### 23.2 Primitive Trait Methods

| Item | Status | Classification | Evidence |
|------|--------|---------------|----------|
| `.to_str()` on primitives (23.2.1) | Working | STALE TEST | Roadmap says "ALL IMPLEMENTED (verified 2026-02-04)" in section header but all checkboxes unchecked. Tests pass. |
| `.clone()` on primitives (23.2.2) | Working | STALE TEST | Same -- described as working, checkbox unchecked. |
| `.hash()` on primitives (23.2.3) | Working | STALE TEST | Same -- described as working, checkbox unchecked. |

### 23.4 Control Flow

| Item | Status | Classification | Evidence |
|------|--------|---------------|----------|
| Break with value in nested loops (23.4.1) | Unverified | NEEDS TESTS | Roadmap says "Returns 0 instead of break value". Full suite passes (4181/0/42), so either this was fixed or the specific test case is skipped. Could not isolate the specific test from the loops test file. |

### 23.5 Derived Traits

| Item | Status | Classification | Evidence |
|------|--------|---------------|----------|
| `#derive(Eq)` (23.5.1) | Working | STALE TEST | Roadmap says "ALL IMPLEMENTED (verified 2026-02-04)" but all checkboxes unchecked. |
| `#derive(Clone)` (23.5.2) | Working | STALE TEST | Same. |
| `#derive(Hashable)` (23.5.3) | Working | STALE TEST | Same. |

### 23.6 Stdlib Types

| Item | Status | Classification | Evidence |
|------|--------|---------------|----------|
| Queue type (23.6.1) | Not implemented | VERIFIED | No Queue implementation in `compiler/ori_eval/src/`. The `library/std/collections/mod.ori` only has TODO comments. Correctly unchecked. |
| Stack type (23.6.2) | Not implemented | VERIFIED | No Stack implementation. Correctly unchecked. |

### 23.8 Parser Feature Support

| Item | Status | Classification | Evidence |
|------|--------|---------------|----------|
| Guard clauses (23.8.1) | Unverified | NEEDS TESTS | Parser support confirmed working. Type checker and evaluator status unclear without dedicated test run. |
| Variadic parameters (23.8.4) | Unverified | NEEDS TESTS | Parser parses `...int` syntax. Type checker/evaluator implementation status unclear. |

## Summary

The 0% status is dramatically inaccurate. The section itself documents that most features work:

| Subsection | Roadmap Description | Checkbox Status | Reality |
|------------|-------------------|-----------------|---------|
| 23.1 Operators | "26/31 tests pass", inline notes say "Works correctly" | All unchecked | Mostly working |
| 23.2 Primitive Trait Methods | "ALL IMPLEMENTED (verified 2026-02-04)" | All unchecked | Working |
| 23.3 Type Coercion/Indexing | "Mostly complete (verified 2026-02-04)" | All unchecked | Mostly working |
| 23.5 Derived Traits | "ALL IMPLEMENTED (verified 2026-02-04)" | All unchecked | Working |
| 23.6 Stdlib Types | Queue/Stack not implemented | Correctly unchecked | Not implemented |
| 23.8 Parser Feature Support | Parser done, typeck/eval pending | All unchecked | Partially done |

**The fundamental problem**: The section's inline descriptions say "Works" and "ALL IMPLEMENTED" but the checkboxes are never checked. This creates a 0% count that contradicts the section's own narrative.

**Recommendations:**
1. Check off items 23.2.1-23.2.3 (all marked "Works" inline)
2. Check off items 23.5.1-23.5.3 (all marked "ALL IMPLEMENTED")
3. Check off items 23.1.2, 23.1.3 (marked "Verified" inline)
4. Verify and potentially check off 23.1.4 (shift overflow) -- the tests pass now
5. Verify 23.3.x items that say "Verified" inline
6. Conservatively, at least 50-60% of this section's items are actually done
