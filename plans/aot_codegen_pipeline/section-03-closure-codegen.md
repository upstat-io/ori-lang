---
section: "03"
title: "Closure Codegen Completion"
status: complete
goal: "Complete PartialApply emission with proper heap allocation, wrapper functions, and environment drop"
sections:
  - id: "03.1"
    title: "Closure environment heap allocation"
    status: complete
  - id: "03.2"
    title: "Wrapper function generation"
    status: complete
  - id: "03.3"
    title: "Environment drop functions"
    status: complete
  - id: "03.4"
    title: "Tests"
    status: complete
---

# Section 03: Closure Codegen Completion

**Status:** Complete
**Goal:** `PartialApply` in ARC IR produces correct, RC-tracked closures in LLVM IR with proper environment drop.

**Context:** Closures have an impedance mismatch: ARC IR's `PartialApply(func, captures)` is a logical closure, but LLVM needs a concrete closure struct `{fn_ptr, env_ptr}`, a wrapper function that unpacks the environment, and a drop function that decrements captured RC values.

**Reference:** Every compiler struggles here — Swift heap-allocates contexts with metadata, Lean boxes closures, Rust monomorphizes most away. The key is clean separation of concerns.

---

## 03.1 Closure Environment Heap Allocation

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`

- [x] Implement `build_closure_env()` (named differently from plan, at lines 913-969):
  - Computes environment struct layout: `{ drop_fn_ptr: ptr, cap_0: T0, cap_1: T1, ... }`
  - Allocates via `ori_rc_alloc(env_size, align=8)` — returns RC-tracked pointer
  - Stores drop function pointer at field 0 via GEP+store
  - Stores each captured value into the environment struct via GEP+store
  - (2026-02-22) Verified: reads lines 913-969 of arc_emitter/mod.rs

- [x] Produce `EmittedValue::Pair { first: wrapper_fn_ptr, second: env_ptr }`
  - (2026-02-22) Verified: `emit_partial_apply()` at lines 847-907 returns pair

- [x] Handle zero-capture case: `env_ptr = null`, no allocation needed
  - (2026-02-22) Verified: line 887-888, returns `const_null_ptr()` for empty captures

---

## 03.2 Wrapper Function Generation

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`

- [x] Implement `generate_closure_wrapper()` (at lines 1063-1193):
  - Generates wrapper function `_ori_partial_{id}(env_ptr, ...user_args)`
  - Rebuilds environment struct type for GEP operations
  - Unpacks captures from environment via GEP+load
  - Forwards user arguments directly
  - Calls original lambda function with full argument set
  - Handles return types: void, sret, direct
  - (2026-02-22) Verified: reads lines 1063-1193

- [x] Wrapper functions use unique counter-based naming (`partial_apply_counter`)
  - (2026-02-22) Verified: line 147 counter, lines 1070-1072 naming

- [x] Handle ABI correctly:
  - Sret: allocates sret alloca and forwards to callee (lines 1130-1136, 1168-1176)
  - Parameters: direct scalars, indirect/reference as pointers (lines 1082-1092)
  - (2026-02-22) Verified: reads ABI handling in wrapper

---

## 03.3 Environment Drop Functions

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`

- [x] Implement `generate_env_drop_fn()` (at lines 975-1048):
  - Generates drop functions: `_ori_partial_{id}_drop`
  - Function signature: `void @_ori_partial_N_drop(ptr %data)`
  - Extracts and RC-decrements each captured variable that needs it
  - Classifies via `classifier.needs_rc(cap_ty)`, skips scalars
  - Calls `ori_rc_dec(data_ptr, drop_fn)` per RC-tracked pointer
  - Frees environment struct via `ori_rc_free(data_ptr, size, align)`
  - Sets `nounwind`, `cold` attributes
  - (2026-02-22) Verified: reads lines 975-1048

- [x] Drop function stored in environment at field 0 (drop_fn_ptr)
  - (2026-02-22) Verified: line 946-950 stores drop fn ptr

- [x] Uses inline drop pattern (consistent with struct drop, `_ori_drop$N`)
  - (2026-02-22) Verified: inline drop via `_ori_partial_{id}_drop`

- [x] Dead `CtorKind::Closure` code path resolved
  - (2026-02-22) Replaced buggy stack-alloca `CtorKind::Closure` arm with `unreachable!()`.
    Closures always use `PartialApply` in ARC IR, never `Construct { ctor: Closure }`.

---

## 03.4 Tests

- [x] Unit test for closure drop function emission:
  - `drop_fn_closure_env_emits_gep_and_rc_dec` — verifies GEP+load+rc_dec pattern
  - (2026-02-22) Verified: runs and passes in `./llvm-test.sh`

- [x] AOT integration tests in `compiler/ori_llvm/tests/aot/arc.rs` (7 tests):
  - `test_arc_lambda_capture_int` — single scalar capture
  - `test_arc_lambda_no_capture` — zero-capture function pointer
  - `test_arc_lambda_capture_multiple` — multiple mixed captures
  - `test_arc_lambda_passed_to_function` — higher-order function with closure
  - `test_arc_lambda_returned_from_function` — closure escaping function scope (NEW)
  - `test_arc_lambda_nested_capture` — inner closure captures outer variable + param (NEW)
  - `test_arc_lambda_capture_bool` — boolean capture (NEW)
  - (2026-02-22) All 7 pass with `ORI_CHECK_LEAKS=1` — zero memory leaks

- [x] Memory leak verification:
  - All AOT tests run with `ORI_CHECK_LEAKS=1` (exit code 2 = leak), all pass
  - (2026-02-22) Verified: `assert_aot_success` automatically enables leak detection

---

## 03.5 Completion Checklist

- [x] `build_closure_env()` allocates RC-tracked environments via `ori_rc_alloc`
- [x] `generate_closure_wrapper()` generates bridge functions with correct ABI
- [x] Wrapper naming uses counter-based unique IDs (no deduplication cache needed — each closure instantiation is unique)
- [x] Environment drop functions generated via `generate_env_drop_fn()`, registered at field 0 of env struct
- [x] Nested closures handled correctly (verified: `test_arc_lambda_nested_capture` passes)
- [x] Zero-capture optimization (null env, no allocation) — verified at line 887-888
- [x] ABI compatibility verified (sret forwarding, indirect params)
- [x] All tests pass, no memory leaks (7 AOT tests + 1 unit test, all with leak detection)
- [x] Dead `CtorKind::Closure` TODO resolved — replaced with `unreachable!()`

**Exit Criteria:** `ori build` compiles programs with lambdas/closures that pass through higher-order functions, are returned from functions, and nest captures — with zero memory leaks and correct captured value lifetimes. Verified 2026-02-22.
