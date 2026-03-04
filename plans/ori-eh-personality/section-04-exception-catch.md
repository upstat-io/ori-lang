---
section: "04"
title: "Native Exception Catch"
status: complete
goal: "catch(expr:) frees caught Ori exceptions on Itanium paths, and ori_try_call remains a clearly scoped SEH compatibility mechanism"
depends_on: ["01", "03"]
sections:
  - id: "04.1"
    title: "Fix ori_catch_cleanup"
    status: complete
  - id: "04.2"
    title: "Lock Down ori_try_call Scope"
    status: complete
  - id: "04.3"
    title: "Completion Checklist"
    status: complete
---

# Section 04: Native Exception Catch

**Context:** `ori_catch_cleanup` is currently a no-op and intentionally leaks because Rust panic exception cleanup could abort when called outside `catch_unwind`. After Section 03 introduces Ori-owned exception objects and cleanup callback, this leak should be removed.

`ori_try_call` currently exists for Windows MSVC SEH compatibility and must be explicitly documented as such, not treated as a cross-platform Itanium catch mechanism.

---

## 04.1 Fix ori_catch_cleanup

**File(s):** `compiler/ori_rt/src/io.rs` (MODIFY)

- [x] Declare `_Unwind_DeleteException` in FFI block: (2026-03-03) — cfg-gated to non-MSVC
  ```rust
  extern "C" {
      fn _Unwind_DeleteException(exception: *mut u8);
  }
  ```

- [x] Replace no-op implementation: (2026-03-03) — calls `_Unwind_DeleteException` on Itanium, no-op on MSVC
  ```rust
  #[no_mangle]
  pub extern "C" fn ori_catch_cleanup(exc_ptr: *mut u8) {
      if exc_ptr.is_null() {
          return;
      }
      unsafe { _Unwind_DeleteException(exc_ptr) };
  }
  ```

- [x] Update doc comments for `ori_catch_cleanup` and `ori_catch_recover` to remove stale "known leak" language. (2026-03-03)

Expected behavior:
- Itanium catch landing pad calls `ori_catch_cleanup(exc_ptr)`.
- `_Unwind_DeleteException` invokes `ori_exception_cleanup` (from Section 03 C runtime code).
- Caught exception object memory is released.

---

## 04.2 Lock Down ori_try_call Scope

**File(s):**
- `compiler/ori_rt/src/io.rs`
- `compiler/ori_llvm/src/codegen/arc_emitter/terminators.rs`
- `compiler/ori_llvm/src/codegen/arc_emitter/catch_thunk.rs`
- `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`

- [x] Confirm and document current emission behavior: `ori_try_call` is used only under `EhModel::Seh` paths. (2026-03-03) — verified: `terminators.rs:146` gates on `eh_model == EhModel::Seh`

- [x] Update comments/docstrings to explicitly state: (2026-03-03)
  - Itanium platforms use LLVM `landingpad catch ptr null` + `ori_eh_personality`.
  - `ori_try_call` is SEH compatibility logic for Windows MSVC.

- [x] Ensure runtime declaration comments for `ori_try_call` remain accurate relative to current EH model behavior. (2026-03-03) — updated in runtime_functions.rs

- [x] Ensure Section 03 target gating is consistent with this scope (MSVC fallback may still rely on `catch_unwind` semantics until dedicated SEH migration). (2026-03-03) — consistent: both use `#[cfg(all(target_os = "windows", target_env = "msvc"))]`

Optional cleanup (only if low risk in this plan):
- Remove dead Itanium assumptions from `ori_try_call` docs/comments if present.

---

## 04.3 Completion Checklist

- [x] `ori_catch_cleanup` calls `_Unwind_DeleteException` and is no longer a no-op (2026-03-03)
- [x] leak rationale comments removed from `io.rs` (2026-03-03)
- [x] `ori_try_call` role documented as SEH/MSVC compatibility path (2026-03-03) — in io.rs, runtime_functions.rs
- [x] `rg -n "ori_try_call" compiler/ori_llvm/src/codegen/arc_emitter` confirms usage remains in SEH-specific code paths (2026-03-03) — all references scoped to SEH
- [x] `cargo build -p ori_rt` succeeds (2026-03-03)
- [x] `cargo test -p ori_rt` succeeds (2026-03-03) — 329/329 pass
- [x] catch-related spec tests pass (2026-03-03) — 4143/4143 pass in tests/spec/control_flow/

**Exit Criteria:** caught exception objects are freed on Itanium catch paths (`_Unwind_DeleteException`), and `ori_try_call` is explicitly constrained/documented as SEH compatibility behavior, with no ambiguity about cross-platform catch semantics.
