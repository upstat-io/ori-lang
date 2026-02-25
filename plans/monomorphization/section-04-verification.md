---
section: "04"
title: "Verification"
status: not-started
goal: "Verify monomorphized generic functions produce identical results to the interpreter"
sections:
  - id: "04.1"
    title: "Existing Test Un-ignore"
    status: not-started
  - id: "04.2"
    title: "New AOT Tests"
    status: not-started
  - id: "04.3"
    title: "Dual-Execution Verification"
    status: not-started
  - id: "04.4"
    title: "Full Test Suite"
    status: not-started
---

# Section 04: Verification

**Goal:** Monomorphized generic functions produce identical results to the interpreter across all test scenarios.

---

## 04.1 Existing Test Un-ignore

**File:** `compiler/ori_llvm/tests/aot/spec.rs`

Un-ignore the existing generic AOT tests that were skipped because monomorphization wasn't implemented.

- [ ] Un-ignore `test_aot_generic_identity` — `@identity<T>(x: T) -> T = x` called with `int`
- [ ] Un-ignore `test_aot_generic_pair` — `@make_pair<A, B>(a: A, b: B) -> (A, B)` called with `int`, `str`
- [ ] Both tests pass

---

## 04.2 New AOT Tests

**File:** `compiler/ori_llvm/tests/aot/spec.rs` (or new `generics.rs` test file)

Add targeted tests for monomorphization edge cases.

- [ ] Generic with 3+ type params
- [ ] Generic returning a tuple
- [ ] Generic calling a non-generic function
- [ ] Same generic called with different concrete types (two distinct specializations)
- [ ] Generic with container type arg (e.g., `identity([1, 2, 3])` → `identity$m$Lint`)
- [ ] `str()` prelude function (the most impactful unblocked call site)
- [ ] `assert_eq` from `std.testing` (the most frequent generic call — 2,472+ sites)

---

## 04.3 Dual-Execution Verification

Verify interpreter and AOT produce identical output for generic function calls.

- [ ] Run `scripts/dual-exec-verify.sh` — no new mismatches
- [ ] Spot-check: `ori run` vs `ori build && ./output` for generic identity, pair, container functions

---

## 04.4 Full Test Suite

- [ ] `./test-all.sh` — all existing tests still pass
- [ ] `./llvm-test.sh` — LLVM tests pass
- [ ] `cargo blr` — release build (FastISel bug guard)
- [ ] `./scripts/valgrind-aot.sh` — no memory errors in monomorphized functions
- [ ] `./clippy-all.sh` — no new warnings
