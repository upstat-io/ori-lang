---
section: "01"
title: "Runtime Panic Path"
status: complete
reviewed: true
goal: "ori_panic/ori_panic_cstr always use _Unwind_RaiseException; JIT test wrappers catch panics via landingpad"
inspired_by:
  - "Swift SIL panic �� _Unwind_RaiseException (lib/SILOptimizer/ARC/)"
  - "Lean 4 lean_panic_fn → Itanium unwind (src/runtime/object.cpp)"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Remove longjmp from ori_panic"
    status: complete
  - id: "01.2"
    title: "AOT main wrapper uses invoke/landingpad"
    status: complete
  - id: "01.3"
    title: "Widen jit_recovery visibility"
    status: complete
  - id: "01.R"
    title: "Stale Comment Cleanup"
    status: complete
  - id: "01.N"
    title: "Completion Checklist"
    status: complete
---

# Section 01: Runtime Panic Path

**Status:** Complete
**Goal:** `ori_panic` and `ori_panic_cstr` always dispatch panics through `_Unwind_RaiseException` (Itanium) or `throw OriPanicException` (MSVC). No longjmp. JIT test wrappers catch panics via LLVM `invoke`/`landingpad`.

**Implementation summary:** All items in this section have been implemented. The longjmp path was removed from `ori_panic`/`ori_panic_cstr` — both now go directly to `aot_raise_exception()`. The AOT `main()` wrapper in `entry_point.rs` uses `invoke`/`landingpad` on Itanium and `ori_try_call` (C++ SEH) on MSVC. The JIT test runner uses two-layer LLVM test wrappers with catch-all landingpads. The `jit_recovery` module is `pub(crate)`.

---

## 01.1 Remove longjmp from ori_panic — COMPLETE

**File(s):** `compiler/ori_rt/src/io/mod.rs`

Verified: `ori_panic` (lines 85-107) goes directly to `panic_state::store_panic` → `panic_state::call_panic_trampoline` → `aot_raise_exception`. No longjmp. Same for `ori_panic_cstr` (lines 113-130).

- [x] longjmp path removed from `ori_panic`
- [x] longjmp path removed from `ori_panic_cstr`
- [x] Dispatch order: store panic state → call user handler → raise C exception

---

## 01.2 AOT main wrapper uses invoke/landingpad — COMPLETE

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/entry_point.rs`

The LLVM-generated `main()` wrapper uses `invoke`/`landingpad` catch-all on Itanium (via `emit_main_call_with_invoke`) and `ori_try_call` SEH thunk on MSVC (via `emit_main_call_with_seh_try`). The Rust-side `ori_run_main` Itanium path (lib.rs:430-443) still uses `catch_unwind` as a safety net but is rarely called — the LLVM-generated wrapper is the actual entry point for AOT binaries.

- [x] AOT main wrapper uses `invoke`/`landingpad` on Itanium
- [x] AOT main wrapper uses `ori_try_call` on MSVC

**Note:** `ori_run_main` Itanium path still uses `catch_unwind`. This is low priority — the LLVM-generated `main()` wrapper handles unwinding directly. Creating an Itanium `ori_try_call` would require either a separate C++ file (currently `eh_personality.c` is compiled as C11, not C++) or using the raw `_Unwind_ForcedUnwind` API.

---

## 01.3 Widen jit_recovery visibility — COMPLETE

**File(s):** `compiler/ori_rt/src/io/mod.rs`, `compiler/ori_rt/src/io/jit_recovery.rs`

Verified: `mod jit_recovery` is `pub(crate)` (io/mod.rs:11). Items `longjmp`, `JIT_RECOVERY_BUF`, `is_jit_mode` are all `pub(crate)` (jit_recovery.rs:54, 81, 105).

- [x] `jit_recovery` module is `pub(crate)`
- [x] All needed items have `pub(crate)` visibility

---

## 01.R Stale Comment Cleanup — NOT STARTED

**Prerequisite:** None (can be done any time before Section 05 verification).

The following stale comments reference the removed longjmp panic path and must be cleaned up:

- [x] `compiler/ori_rt/src/eh_personality.c` line 418: `ori_raise_exception` comment says "3. Attempting JIT longjmp recovery (if in JIT mode)" — this step no longer exists since `ori_panic` goes directly to `aot_raise_exception`. Remove step 3.
- [x] `compiler/ori_rt/src/io/mod.rs` line 198-199: `ori_assert` doc says "longjmp to test runner" — longjmp is no longer used for panic dispatch. Update to "unwind via Ori exception in both JIT and AOT paths".
- [x] `compiler/ori_rt/src/io/mod.rs` line 7: module doc says "`setjmp`/`longjmp`-based error recovery for test runners" — the primary JIT recovery mechanism is now LLVM landingpads. Update to reflect that `jit_recovery` module provides legacy setjmp/longjmp support (still used by `jit_run_protected` but not the primary path).
- [x] `compiler/ori_rt/src/io/jit_recovery.rs` line 5: module doc says "JIT mode: Thread-local state that redirects panics to `longjmp`" — panics no longer redirect to longjmp. Update to reflect that JIT mode / longjmp is a legacy fallback, not the primary recovery mechanism.

---

## 01.N Completion Checklist

- [x] `ori_panic` has no `longjmp` path — only `aot_raise_exception`
- [x] `ori_panic_cstr` has no `longjmp` path — only `aot_raise_exception`
- [x] AOT main wrapper uses `invoke`/`landingpad` on Itanium
- [x] `jit_recovery` module is `pub(crate)` with necessary items visible

**Exit Criteria:** Met. `ori_panic` always raises via `_Unwind_RaiseException`. AOT panic recovery works via LLVM landingpads.
