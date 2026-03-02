---
section: "01"
title: "C Personality Function"
status: not-started
goal: "ori_eh_personality exists in libori_rt.a and correctly handles cleanup + catch-all landing pads"
inspired_by:
  - "GCC gcc_personality_v0.c (~/projects/reference_repos/lang_repos/zig/lib/libunwind/src/gcc_personality_v0.c)"
  - "Rust personality (~/projects/reference_repos/lang_repos/rust/library/std/src/sys/personality/gcc.rs)"
  - "Rust DWARF EH parser (~/projects/reference_repos/lang_repos/rust/library/std/src/sys/personality/dwarf/eh.rs)"
depends_on: []
sections:
  - id: "01.1"
    title: "LSDA Parser and Personality Implementation"
    status: not-started
  - id: "01.2"
    title: "Build System Integration"
    status: not-started
  - id: "01.3"
    title: "JIT Symbol Bridge"
    status: not-started
  - id: "01.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: C Personality Function

**Status:** Not Started
**Goal:** `ori_eh_personality` is compiled into `libori_rt.a` (staticlib) and `libori_rt.rlib` (for JIT), correctly handles both `cleanup` and `catch ptr null` (catch-all) landing pads per the Itanium EH ABI, and is accessible from both AOT and JIT execution paths.

**Context:** Code Journey 3 identified that Ori's LLVM codegen emits `personality ptr @rust_eh_personality` on every function containing `invoke`/`landingpad`. This symbol comes from Rust's standard library panic infrastructure, making every AOT binary depend on Rust's runtime. Ori needs its own personality function to be a standalone language.

The personality function is called by the platform unwinder (`libunwind`) during exception handling. It reads the Language-Specific Data Area (LSDA) — metadata compiled into the `.eh_frame` section by LLVM — and tells the unwinder what to do at each stack frame: run cleanup code, catch the exception, or continue unwinding.

**Reference implementations:**
- **GCC** `gcc_personality_v0.c` (259 lines): Minimal C-only personality. Handles cleanup only (no catch). Simplest reference for LSDA parsing.
- **Rust** `library/std/src/sys/personality/gcc.rs` (341 lines): Full search + cleanup phases. Handles catch-all via `ttype_index == 0`. Closest to what Ori needs.
- **Rust** `library/std/src/sys/personality/dwarf/eh.rs` (272 lines): DWARF LSDA parsing utilities (ULEB128, SLEB128, encoded pointer reader). Reusable patterns.

---

## 01.1 LSDA Parser and Personality Implementation

**File(s):** `compiler/ori_rt/src/eh_personality.c` (NEW)

The personality function must implement the Itanium EH ABI (`_Unwind_Personality_Fn` signature) and parse the DWARF LSDA to handle Ori's two landing pad types.

### Background: How LLVM Exception Handling Works

When Ori's codegen emits `invoke` + `landingpad`, LLVM generates:

1. **Call-site table** in `.eh_frame` / `.gcc_except_table`: maps instruction ranges to landing pads
2. **Action table**: what each landing pad does (cleanup vs. catch, indexed by `ttype_index`)
3. **Type info table**: exception type metadata (unused for catch-all)

During unwinding, the platform unwinder calls our personality function **twice per frame**:
- **Phase 1 (Search)**: "Is there a handler here?" — return `_URC_HANDLER_FOUND` or `_URC_CONTINUE_UNWIND`
- **Phase 2 (Cleanup)**: "Run cleanup and/or install handler" — set registers, return `_URC_INSTALL_CONTEXT`

### Ori's Landing Pad Types

**Type 1: Cleanup** (`landingpad { ptr, i32 } cleanup`)
- Generated for ARC cleanup (decrement RC on unwind)
- Followed by `resume` (re-raise exception)
- Action: `cs_action_entry == 0` (no action record, pure cleanup)
- Personality response: Phase 1 → `_URC_CONTINUE_UNWIND`; Phase 2 → set LP and `_URC_INSTALL_CONTEXT`

**Type 2: Catch-all** (`landingpad { ptr, i32 } catch ptr null`)
- Generated for `catch()` pattern and top-level exception handling
- Catches any exception regardless of type
- Action: `ttype_index == 0` in the action table (catch-all in Itanium ABI)
- Personality response: Phase 1 → `_URC_HANDLER_FOUND`; Phase 2 → set LP and `_URC_INSTALL_CONTEXT`

### Implementation

- [ ] Create `compiler/ori_rt/src/eh_personality.c` with the following components:

  **ULEB128 decoder** (~15 lines):
  ```c
  static uintptr_t read_uleb128(const uint8_t **p) {
      uintptr_t result = 0;
      int shift = 0;
      uint8_t byte;
      do {
          byte = **p; (*p)++;
          result |= (uintptr_t)(byte & 0x7F) << shift;
          shift += 7;
      } while (byte & 0x80);
      return result;
  }
  ```

  **SLEB128 decoder** (~20 lines):
  ```c
  static intptr_t read_sleb128(const uint8_t **p) {
      intptr_t result = 0;
      int shift = 0;
      uint8_t byte;
      do {
          byte = **p; (*p)++;
          result |= (intptr_t)(byte & 0x7F) << shift;
          shift += 7;
      } while (byte & 0x80);
      if ((shift < (int)(sizeof(intptr_t) * 8)) && (byte & 0x40))
          result |= -(((intptr_t)1) << shift);
      return result;
  }
  ```

  **Encoded pointer reader** (~40 lines):
  Handle DWARF pointer encodings that LLVM may use in the LSDA:
  - `DW_EH_PE_absptr` (0x00) — absolute pointer
  - `DW_EH_PE_uleb128` (0x01) — ULEB128
  - `DW_EH_PE_udata2/4/8` (0x02/0x03/0x04) — fixed-width unsigned
  - `DW_EH_PE_pcrel` (0x10) — PC-relative
  - `DW_EH_PE_omit` (0xFF) — no value present

  **LSDA header parser** (~25 lines):
  Parse the header at the start of the LSDA:
  ```
  byte: lp_start_encoding (usually DW_EH_PE_omit)
  byte: ttype_encoding (for type info table, may be omit)
  uleb128: ttype_base_offset (if ttype_encoding != omit)
  byte: call_site_encoding
  uleb128: call_site_table_length
  ```

  **Call-site table walker** (~30 lines):
  Linear scan through call-site entries. **Critical:** use `ip - 1` (not raw IP) for matching.
  The unwinder provides the *return address* (instruction after the call), but the call-site
  table maps the *calling instruction* range. Subtracting 1 maps back into the caller's range.
  Every production personality does this (GCC, Rust, libcxxabi).
  ```
  // IMPORTANT: adjust IP before matching
  uintptr_t ip = _Unwind_GetIP(context) - 1;

  for each entry:
      start  = read_encoded(call_site_encoding)  // range start (relative to function)
      length = read_encoded(call_site_encoding)  // range length
      lpad   = read_encoded(call_site_encoding)  // landing pad offset (0 = no LP)
      action = read_uleb128()                     // action table index (0 = cleanup only)

      if ip is in [func_start+start, func_start+start+length):
          this is our entry
  ```

  **Action classifier** (~15 lines):
  ```c
  // action == 0 → cleanup only (no action record)
  // action > 0  → read action table at (action_table + action - 1)
  //   ttype_index from SLEB128:
  //     == 0 → catch-all
  //     >  0 → catch specific type (not needed for Ori MVP)
  //     <  0 → exception spec filter (not needed for Ori MVP)
  ```

  **Main personality function** (~30 lines):
  ```c
  _Unwind_Reason_Code ori_eh_personality(
      int version,
      _Unwind_Action actions,
      uint64_t exception_class,
      struct _Unwind_Exception *exception_object,
      struct _Unwind_Context *context)
  {
      // 1. Get LSDA pointer from context
      // 2. Parse LSDA header
      // 3. Get current IP via _Unwind_GetIP(context) - 1 (return addr → call site)
      //    find matching call-site entry
      // 4. Classify action (cleanup vs catch-all)
      // 5. Phase 1 (search): return HANDLER_FOUND for catch-all, CONTINUE for cleanup
      // 6. Phase 2 (cleanup): set GR[0]=exception, GR[1]=selector, IP=landing_pad
      //    return INSTALL_CONTEXT
  }
  ```

  **Register setup** (inside phase 2):
  ```c
  _Unwind_SetGR(context, __builtin_eh_return_data_regno(0),
                (uintptr_t)exception_object);
  _Unwind_SetGR(context, __builtin_eh_return_data_regno(1),
                (uintptr_t)selector);
  _Unwind_SetIP(context, landing_pad);
  return _URC_INSTALL_CONTEXT;
  ```

- [ ] Include `<unwind.h>` for Itanium EH ABI types (`_Unwind_Context`, `_Unwind_Exception`, `_Unwind_Action`, etc.)

- [ ] Mark function with `__attribute__((used))` to prevent dead-code elimination by the C compiler, and ensure it's exported from the static library.

- [ ] Add a header comment explaining: this is Ori's exception handling personality function, it handles cleanup and catch-all only, and references the Itanium EH ABI spec.

---

## 01.2 Build System Integration

**File(s):** `compiler/ori_rt/build.rs` (NEW), `compiler/ori_rt/Cargo.toml` (MODIFY)

The C file must be compiled and linked into `libori_rt.a`. The standard way to do this in Rust is via the `cc` crate in a build script.

- [ ] Add `cc` as a build dependency in `compiler/ori_rt/Cargo.toml`:
  ```toml
  [build-dependencies]
  cc = "1"
  ```

- [ ] Create `compiler/ori_rt/build.rs`:
  ```rust
  fn main() {
      let mut build = cc::Build::new();
      build
          .file("src/eh_personality.c")
          .flag("-std=c11")
          .warnings(true)
          .extra_warnings(true);

      // These flags are standard on GCC/Clang but may not exist on all
      // toolchains. flag_if_supported avoids hard failures on exotic compilers.
      build.flag_if_supported("-fno-exceptions");

      build.compile("ori_eh"); // produces libori_eh.a, merged into final lib
  }
  ```

  Notes:
  - `-fno-rtti` is omitted — it's a C++ flag, not applicable to C code.
  - `-fno-exceptions` uses `flag_if_supported` for cross-toolchain portability.

  The `cc` crate compiles the C file into a static archive and tells Cargo to link it. When Cargo builds `libori_rt.a` (staticlib), the C object gets bundled in.

- [ ] Verify that both build outputs contain the symbol:
  - `libori_rt.a` (staticlib for AOT): `nm target/debug/libori_rt.a | grep ori_eh_personality`
  - `libori_rt.rlib` (for JIT): symbol available via Cargo linking

---

## 01.3 JIT Symbol Bridge

**File(s):** `compiler/ori_rt/src/lib.rs` (MODIFY)

The JIT execution engine needs to find `ori_eh_personality` at runtime. Since the C function is compiled into the same library, we need a Rust-side way to get its address.

- [ ] Add an `extern "C"` declaration and address-getter in `ori_rt/src/lib.rs`:
  ```rust
  extern "C" {
      /// Ori's Itanium EH ABI personality function (implemented in eh_personality.c).
      /// Required by any LLVM function containing `invoke`/`landingpad`.
      fn ori_eh_personality();
  }

  /// Get the address of `ori_eh_personality` for JIT symbol mapping.
  ///
  /// The personality function is implemented in C (`src/eh_personality.c`) and
  /// compiled into this library. This function provides its address so the
  /// LLVM MCJIT engine can resolve the symbol.
  #[must_use]
  pub fn ori_eh_personality_addr() -> usize {
      ori_eh_personality as *const () as usize
  }
  ```

  This follows the exact same pattern as the existing `rust_eh_personality_addr()` in `evaluator/runtime_mappings.rs`, but moves the address resolution to `ori_rt` where the symbol lives.

- [ ] Verify the function is accessible from `ori_llvm` via `runtime::ori_eh_personality_addr()`.

---

## 01.4 Completion Checklist

- [ ] `eh_personality.c` exists in `compiler/ori_rt/src/` and compiles without warnings
- [ ] `build.rs` uses `cc` crate to compile the C file
- [ ] `nm target/debug/libori_rt.a | grep ori_eh_personality` returns the symbol (T = text section)
- [ ] `ori_eh_personality_addr()` is exported from `ori_rt` and callable from `ori_llvm`
- [ ] `cargo build -p ori_rt` succeeds (both rlib and staticlib)
- [ ] No new Clippy warnings in `ori_rt`

**Exit Criteria:** `ori_eh_personality` symbol is present in both `libori_rt.a` and the rlib, and its address is obtainable via `ori_rt::ori_eh_personality_addr()`. The function implements correct LSDA parsing for cleanup and catch-all landing pads per the Itanium EH ABI. Verified via `nm` and `cargo build -p ori_rt`.
