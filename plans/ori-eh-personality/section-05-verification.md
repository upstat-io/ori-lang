---
section: "05"
title: "Verification"
status: complete
goal: "Prove end-to-end correctness for personality selection, exception raise/catch lifecycle, and platform-scoped behavior without regressions"
depends_on: ["01", "02", "03", "04"]
sections:
  - id: "05.1"
    title: "Build and Test Suite"
    status: complete
  - id: "05.2"
    title: "Symbol and Source Audit"
    status: complete
  - id: "05.3"
    title: "IR and Behavior Verification"
    status: complete
  - id: "05.4"
    title: "Memory and Exception Lifecycle"
    status: complete
  - id: "05.5"
    title: "Completion Checklist"
    status: complete
---

# Section 05: Verification

**Context:** This plan changes both compiler-side EH symbol wiring and runtime unwind behavior. Verification must confirm:

1. Itanium codegen selects `@ori_eh_personality` everywhere required.
2. Itanium AOT panic path raises Ori exceptions via `_Unwind_RaiseException`.
3. Catch cleanup frees exception objects.
4. MSVC compatibility assumptions remain explicit and non-regressing.
5. No regressions in existing journey/spec behavior.

---

## 05.1 Build and Test Suite

- [x] `cargo build -p ori_rt` (2026-03-03)
- [x] `cargo build -p ori_llvm` (2026-03-03)
- [x] `cargo build` (2026-03-03)
- [x] `cargo build --release` (2026-03-03)
- [x] `./test-all.sh` — 10,779 passed, 0 failed (2026-03-03)
- [x] `./clippy-all.sh` (2026-03-03)
- [x] `./fmt-all.sh` (2026-03-03)

Targeted tests:
- [x] `cargo test -p ori_llvm codegen::eh_model::tests` (2026-03-03) — personality_name_itanium, jit_symbol_mappings pass
- [x] `cargo test -p ori_llvm codegen::runtime_decl::tests::jit_symbol_mappings_match_jit_allowed` (2026-03-03)
- [x] `cargo test -p ori_rt` — 329/329 pass, includes forced-unwind test (2026-03-03)

---

## 05.2 Symbol and Source Audit

### Personality Symbol Migration (Compiler)

- [x] Verify no stale personality symbol in compiler/runtime source: (2026-03-03)
  ```bash
  rg -n "rust_eh_personality" compiler/ori_llvm/src compiler/ori_rt/src
  ```
  Result: 0 matches.

- [x] Verify expected symbol presence: (2026-03-03)
  ```bash
  rg -n "ori_eh_personality" compiler/ori_llvm/src compiler/ori_rt/src
  ```
  Expected hits confirmed: runtime declarations, EH model, JIT mapping, verifier fixture, runtime FFI bridge, test harness.

### Panic Path Migration (Runtime)

- [x] Audit panic-any usage in `io.rs`: (2026-03-03)
  ```bash
  rg -n "panic_any" compiler/ori_rt/src/io.rs
  ```
  Result: only in MSVC-gated `aot_raise_exception` compatibility path (line 297).

- [x] Verify panic entrypoint ABI did not regress: (2026-03-03)
  Both `ori_panic` and `ori_panic_cstr` remain `extern "C-unwind"`.

### Runtime Symbol Presence

- [x] `nm target/debug/libori_rt.a | rg "ori_eh_personality|ori_raise_exception"` (2026-03-03)
  Result: both `T ori_eh_personality` and `T ori_raise_exception` present.

---

## 05.3 IR and Behavior Verification

### LLVM IR Personality Audit

- [x] Dump IR for a known invoke-bearing program: (2026-03-03)
  ```
  personality ptr @ori_eh_personality
  ```
  Confirmed in `_ori_main` for catch-bearing programs.

- [x] Binary symbol check: (2026-03-03)
  `ori_eh_personality` present. `rust_eh_personality` also present (from Rust std internals) — expected and acceptable.

### Journey Regression

- [x] Re-run code journeys with EH/ARC sensitivity: (2026-03-03)
  - Journey 3 — pass
  - Journey 5 — pass
  - Journey 9 — pass
  - Journey 10 — pass
  - Journey 11 — pass
  - Journey 12 — pass

- [x] Confirm eval and AOT results match expected outputs for each. (2026-03-03)

---

## 05.4 Memory and Exception Lifecycle

- [x] Run Valgrind suite: 7/7 pass, 0 errors (2026-03-03)

- [x] Run targeted catch lifecycle check: (2026-03-03)
  Valgrind on catch program: 0 errors, 0 definitely-lost blocks. Exception object (`OriException`) properly freed via `_Unwind_DeleteException` → `ori_exception_cleanup` → `free()`.

- [x] Run leak-check mode for catch flow: (2026-03-03)
  Catch program runs correctly with both success and failure paths.

- [x] Forced-unwind checks: (2026-03-03)
  - `linux x86_64`: both forced-unwind tests pass (catch-all skipped, cleanup entered)
  - Other targets: test file skipped cleanly via cfg.

- [x] Verify `ori_try_call` scope remained SEH-oriented: (2026-03-03)
  All references in `arc_emitter/` are scoped to `EhModel::Seh`.

---

## 05.5 Completion Checklist

### Compiler Integration
- [x] Itanium EH model resolves to `ori_eh_personality` (2026-03-03)
- [x] RT declarations + JIT mappings + verifier fixtures are in sync (2026-03-03)
- [x] Zero `rust_eh_personality` in `compiler/ori_llvm/src` and `compiler/ori_rt/src` (2026-03-03)

### Runtime Behavior
- [x] Itanium AOT panic path raises via `_Unwind_RaiseException` (2026-03-03)
- [x] Panic entrypoints remain `extern "C-unwind"` (2026-03-03)
- [x] Caught exception objects are freed via `_Unwind_DeleteException` (2026-03-03)
- [x] Any remaining `panic_any` use is explicitly MSVC compatibility-gated (2026-03-03)

### Regression and Safety
- [x] `./test-all.sh`, `./clippy-all.sh`, `./fmt-all.sh` clean (2026-03-03)
- [x] Journey regression checks pass (J3, J5, J9, J10, J11, J12) (2026-03-03)
- [x] Valgrind checks show no new errors/leaks (2026-03-03)
- [x] Forced-unwind behavior validated on supported targets (2026-03-03)

### Additional Bugs Found and Fixed During Verification
- [x] **Personality function ttype_index misinterpretation** (2026-03-03): `ttype_index == 0` was incorrectly treated as catch-all. Per the Itanium ABI, `ttype_index > 0` is a catch handler (NULL type table entry = catch-all), and `ttype_index == 0` is cleanup only. Fixed in `eh_personality.c`.
- [x] **`extern "C"` instead of `extern "C-unwind"` for `ori_raise_exception` FFI** (2026-03-03): Rust's `extern "C"` boundary inserts an abort guard that prevents stack unwinding. Changed to `extern "C-unwind"` to allow `_Unwind_RaiseException` to walk through the calling frame.
- [x] **Assembly test LSDA encoding mismatch** (2026-03-03): Test harness used `ttype_index = 0` (wrong) for catch-all. Updated both x86_64 and aarch64 assembly to use proper LLVM encoding: `ttype_index = 1` pointing to a NULL type table entry.

**Exit Criteria:** The Itanium EH path is fully Ori-owned from IR symbol selection through raise/catch lifecycle management, MSVC compatibility boundaries are explicit and non-regressing, and the full verification matrix passes.
