---
section: "04"
title: "Verification"
status: complete
goal: "Verify monomorphized generic functions produce identical results to the interpreter"
sections:
  - id: "04.1"
    title: "Existing Test Un-ignore"
    status: complete
  - id: "04.2"
    title: "New AOT Tests"
    status: complete
  - id: "04.3"
    title: "Dual-Execution Verification"
    status: complete
  - id: "04.4"
    title: "Full Test Suite"
    status: complete
---

# Section 04: Verification

**Goal:** Monomorphized generic functions produce identical results to the interpreter across all test scenarios.

---

## 04.1 Existing Test Un-ignore

**File:** `compiler/ori_llvm/tests/aot/spec.rs`

Un-ignore the existing generic AOT tests that were skipped because monomorphization wasn't implemented.

- [x] Un-ignore `test_aot_generic_identity` — `@identity<T>(x: T) -> T = x` called with `int`
- [x] Un-ignore `test_aot_generic_pair` — `@make_pair<A, B>(a: A, b: B) -> (A, B)` called with `int`, `str`
- [x] Both tests pass

---

## 04.2 New AOT Tests

**File:** `compiler/ori_llvm/tests/aot/spec.rs`

Added targeted tests for monomorphization edge cases.

- [x] Generic with 3 type params (`test_aot_generic_three_type_params`)
- [x] Generic calling a non-generic function (`test_aot_generic_calling_non_generic`)
- [x] Same generic called with different concrete types — two distinct specializations (`test_aot_generic_two_specializations`)
- [ ] Generic with container type arg (e.g., `identity([1, 2, 3])`) — deferred: requires List ARC codegen
- [ ] `str()` prelude function — deferred: requires prelude wiring in AOT path
- [ ] `assert_eq` from `std.testing` — deferred: requires import resolution in AOT path

---

## 04.3 Dual-Execution Verification

Verified interpreter and AOT produce identical output for generic function calls.

- [x] Release binary (`cargo blr`) compiles and runs generic identity/pair programs with exit code 0
- [x] Debug AOT test harness produces identical results to interpreter for all 5 generic test cases

---

## 04.4 Full Test Suite

- [x] `./test-all.sh` — 10,040 tests pass, 0 failures
- [x] `./llvm-test.sh` — 962 LLVM tests pass
- [x] `cargo blr` — release build succeeds (FastISel bug guard)
- [ ] `./scripts/valgrind-aot.sh` — requires debug binary with LLVM feature; release binary validated end-to-end
- [x] `./clippy-all.sh` — no warnings
