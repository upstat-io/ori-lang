---
section: "03"
title: "Ori Exception Raise"
status: complete
goal: "On Itanium AOT paths, ori_panic/ori_panic_cstr raise Ori exceptions via _Unwind_RaiseException instead of std::panic::panic_any"
depends_on: ["01"]
sections:
  - id: "03.1"
    title: "Exception Object & Raise Function"
    status: complete
  - id: "03.2"
    title: "Runtime Panic Integration"
    status: complete
  - id: "03.3"
    title: "Target Gating and Compatibility"
    status: complete
  - id: "03.4"
    title: "Completion Checklist"
    status: complete
---

# Section 03: Ori Exception Raise

**Context:** After Section 02, codegen can reference `@ori_eh_personality`, but runtime panic emission still uses `std::panic::panic_any` in `ori_panic` and `ori_panic_cstr`. That means Rust panic runtime semantics still drive AOT unwind behavior.

This section moves Itanium AOT panic raising to Ori-owned exception objects via `_Unwind_RaiseException`.

Critical correctness constraints:
- `ori_panic`/`ori_panic_cstr` are exported FFI entrypoints that may unwind, so their ABI must remain `extern "C-unwind"`.
- `ori_raise_exception` does **not** need a JIT mapping entry unless LLVM IR directly calls it.
- Windows MSVC compatibility path (`ori_try_call` + `catch_unwind`) must remain explicitly handled.

---

## 03.1 Exception Object & Raise Function

**File(s):** `compiler/ori_rt/src/eh_personality.c` (MODIFY)

- [x] Define Ori exception class and object header: (2026-03-03)
  ```c
  #define ORI_EXCEPTION_CLASS 0x4f524900504e4300ULL

  typedef struct {
      struct _Unwind_Exception header;
  } OriException;
  ```

- [x] Implement cleanup callback for caught exceptions: (2026-03-03)
  ```c
  static void ori_exception_cleanup(
      _Unwind_Reason_Code reason,
      struct _Unwind_Exception *exc)
  {
      (void)reason;
      free(exc);
  }
  ```

- [x] Implement `_Unwind_RaiseException` bridge: (2026-03-03)
  ```c
  __attribute__((noreturn))
  void ori_raise_exception(void) {
      OriException *exc = (OriException *)malloc(sizeof(OriException));
      if (!exc) {
          fprintf(stderr, "ori: fatal — out of memory during panic\n");
          abort();
      }
      memset(exc, 0, sizeof(*exc));
      exc->header.exception_class = ORI_EXCEPTION_CLASS;
      exc->header.exception_cleanup = ori_exception_cleanup;

      _Unwind_Reason_Code result = _Unwind_RaiseException(&exc->header);

      free(exc);
      fprintf(stderr,
          "ori: fatal — _Unwind_RaiseException returned (code %d)\n",
          (int)result);
      abort();
  }
  ```

- [x] Ensure required includes are present: `<stdlib.h>`, `<stdio.h>`, `<string.h>`, `<unwind.h>`. (2026-03-03)

- [x] Keep function externally visible (no `static`). (2026-03-03) — `nm` confirms `T ori_raise_exception` (global text symbol)

---

## 03.2 Runtime Panic Integration

**File(s):** `compiler/ori_rt/src/io.rs`, `compiler/ori_rt/src/lib.rs` (MODIFY)

- [x] Add FFI declaration in `io.rs`: (2026-03-03) — `#[cfg(not(all(target_os = "windows", target_env = "msvc")))]`
  ```rust
  extern "C" {
      fn ori_raise_exception() -> !;
  }
  ```

- [x] Update AOT Itanium panic path in both functions (`ori_panic`, `ori_panic_cstr`) to call: (2026-03-03) — via `aot_raise_exception()` helper with cfg-gated implementations
  ```rust
  unsafe { ori_raise_exception() }
  ```
  after panic message TLS storage and JIT `longjmp` fast path handling.

- [x] Keep ABI as `extern "C-unwind"` for `ori_panic` and `ori_panic_cstr`. (2026-03-03)

- [x] Remove stale comments that describe the default AOT path as `panic_any`. (2026-03-03) — updated assert doc, panic dispatch doc

- [x] Update `lib.rs` comments around `OriPanic`/`ori_run_main` so they no longer claim unconditional `panic_any`-based AOT behavior. (2026-03-03) — OriPanic documented as MSVC-only, ori_run_main documented as not in LLVM call chain

---

## 03.3 Target Gating and Compatibility

**File(s):** `compiler/ori_rt/src/io.rs`, `compiler/ori_rt/src/lib.rs` (MODIFY)

- [x] Gate behavior by target model: (2026-03-03) — `aot_raise_exception()` with `#[cfg]` dispatch
  - Itanium-oriented targets: use `ori_raise_exception`.
  - Windows MSVC compatibility path: keep `panic_any` behavior until a dedicated SEH migration replaces `ori_try_call`/`catch_unwind` coupling.

- [x] Keep `OriPanic` only as long as needed for MSVC compatibility path; mark/document it accordingly. (2026-03-03) — doc comment updated to state MSVC-only scope

- [x] Do **not** add `ori_raise_exception` to `runtime_mappings.rs` unless LLVM IR starts calling that symbol directly. (2026-03-03) — confirmed not added, no JIT mapping needed

Rationale:
- JIT mapping table exists for symbols called by JIT-generated LLVM IR.
- `ori_raise_exception` is an internal call from runtime Rust code (`ori_panic*`), so normal static linking resolves it without MCJIT symbol registration.

---

## 03.4 Completion Checklist

- [x] `ori_raise_exception` exists in `eh_personality.c` and is `__attribute__((noreturn))` (2026-03-03)
- [x] `nm target/debug/libori_rt.a | grep ori_raise_exception` shows the symbol (2026-03-03) — `T ori_raise_exception`
- [x] `ori_panic` and `ori_panic_cstr` remain `extern "C-unwind"` (2026-03-03)
- [x] Itanium AOT panic path no longer uses `panic_any` (2026-03-03) — uses `ori_raise_exception()` via `aot_raise_exception()`
- [x] `rg -n "panic_any" compiler/ori_rt/src/io.rs` returns only MSVC-guarded compatibility matches (2026-03-03) — 1 match at line 279 inside `#[cfg(all(target_os = "windows", target_env = "msvc"))]`
- [x] `cargo build -p ori_rt` succeeds (2026-03-03)
- [x] `cargo test -p ori_rt` succeeds (2026-03-03) — 329/329 pass

**Exit Criteria:** Itanium AOT panic raising is Ori-owned (`_Unwind_RaiseException` via `ori_raise_exception`), ABI correctness is preserved (`extern "C-unwind"` on panic entrypoints), and any remaining `panic_any` usage is explicitly MSVC compatibility-scoped and documented.
