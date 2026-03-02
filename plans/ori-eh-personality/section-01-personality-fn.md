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
    title: "Forced-Unwind Test Harness"
    status: not-started
  - id: "01.5"
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

**Forced unwind (`_UA_FORCE_UNWIND`)**: When the unwinder is performing a forced unwind (e.g. `pthread_cancel`, `longjmp`-based cleanup), the personality must NOT install catch-all handler contexts. Catch-all landing pads jump into handler code that *absorbs* the exception (no `resume`), so installing them would swallow the forced unwind and hang the thread. During `_UA_FORCE_UNWIND`:
- **Cleanup pads**: install normally (they end with `resume`, which continues unwinding)
- **Catch-all pads**: skip entirely — return `_URC_CONTINUE_UNWIND` in both phases; do NOT set the landing pad IP

This means: Phase 1 → never return `_URC_HANDLER_FOUND`; Phase 2 → only install cleanup pads. Rust's personality does this same check.

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
- **Forced unwind:** If `actions & _UA_FORCE_UNWIND`, skip entirely — `_URC_CONTINUE_UNWIND` in both phases (catch pads don't `resume`, so installing them would swallow the unwind)

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
      // 0. Version gate: if (version != 1) return _URC_FATAL_PHASE1_ERROR
      // 1. Get LSDA pointer from context; if NULL return _URC_CONTINUE_UNWIND
      // 2. Parse LSDA header
      // 3. Get current IP via _Unwind_GetIP(context) - 1 (return addr → call site)
      //    find matching call-site entry
      // 4. Classify action (cleanup vs catch-all)
      // 5. Phase 1 (search):
      //    - catch-all AND NOT _UA_FORCE_UNWIND → HANDLER_FOUND
      //    - catch-all AND _UA_FORCE_UNWIND → CONTINUE_UNWIND (skip — no resume)
      //    - cleanup only → CONTINUE_UNWIND
      // 6. Phase 2 (cleanup):
      //    - cleanup pad: set GR[0]=exception, GR[1]=selector, IP=landing_pad
      //      → INSTALL_CONTEXT (pad will run ARC cleanup then `resume`)
      //    - catch-all pad (normal unwind only, never forced):
      //      set GR[0]=exception, GR[1]=selector, IP=landing_pad
      //      → INSTALL_CONTEXT (pad will jump to handler code)
      //    - catch-all pad AND _UA_FORCE_UNWIND → CONTINUE_UNWIND (never install)
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

## 01.4 Forced-Unwind Test Harness

**File(s):** `compiler/ori_rt/src/test_forced_unwind.c` (NEW), `compiler/ori_rt/src/test_frames_x86_64.S` (NEW), `compiler/ori_rt/src/test_frames_aarch64.S` (NEW), `compiler/ori_rt/build.rs` (MODIFY), `compiler/ori_rt/tests/forced_unwind.rs` (NEW)

A C + assembly test that verifies `ori_eh_personality` correctly handles `_UA_FORCE_UNWIND` — catch-all pads are skipped, cleanup pads still run, and the unwind completes without hanging.

**Target scope: `x86_64-unknown-linux-*` and `aarch64-unknown-linux-*`.** Per-architecture assembly stubs use native registers and calling conventions, but share the same DWARF CFI directives (`.cfi_personality`, `.cfi_lsda`) and ELF LSDA format (`.section .gcc_except_table`). The C harness and Rust test file are architecture-independent. `build.rs` selects the correct `.S` file based on `CARGO_CFG_TARGET_ARCH`. Non-Linux targets (macOS Mach-O) are deferred — they use different section names and would need a third variant.

**Why C + assembly, not Ori:** Forced unwind is triggered by `_Unwind_ForcedUnwind`, a C API not exposed to Ori. Attaching `ori_eh_personality` to specific test frames requires `.cfi_personality` directives in assembly — the standard DWARF CFI mechanism supported by all ELF assemblers. This avoids depending on recent compiler extensions (`__attribute__((personality(...)))` requires GCC 14+/Clang 18+).

**Architecture differences:**

| | x86-64 | aarch64 |
|---|--------|---------|
| First arg register | `%rdi` | `x0` |
| Return value register | `%eax`/`%rax` | `w0`/`x0` |
| Frame pointer | `%rbp` | `x29` |
| Link register | (on stack via `call`) | `x30` (aka `lr`) |
| Indirect call | `callq *%rdi` | `blr x0` |
| PC-relative global | `symbol(%rip)` | `adrp` + `str` |
| Exception object in LP | `%rax` (from `_Unwind_SetGR` → data reg 0) | `x0` (data reg 0) |
| Resume call | `callq _Unwind_Resume@PLT` | `bl _Unwind_Resume` |

The LSDA format (`.gcc_except_table` section) is identical on both architectures — it's DWARF, not architecture-specific.

- [ ] Create `compiler/ori_rt/src/test_frames_x86_64.S`:

  x86-64 assembly stubs with `ori_eh_personality` and hand-written LSDA entries.

  ```asm
  # x86-64 System V ABI — test frames for ori_eh_personality
  .text

  # ---------------------------------------------------------------
  # frame_with_catch_all(trigger: fn ptr in %rdi)
  # LSDA has catch-all entry (ttype_index == 0).
  # Forced unwind: personality must NOT install this handler.
  # ---------------------------------------------------------------
  .globl frame_with_catch_all
  .type frame_with_catch_all, @function
  frame_with_catch_all:
      .cfi_startproc
      .cfi_personality 0, ori_eh_personality
      .cfi_lsda 0, .LLSDA_catch
      pushq   %rbp
      .cfi_def_cfa_offset 16
      .cfi_offset %rbp, -16
      movq    %rsp, %rbp
      .cfi_def_cfa_register %rbp
  .Lcatch_call_start:
      callq   *%rdi                       # call trigger(void)
  .Lcatch_call_end:
      xorl    %eax, %eax                  # return 0 (no exception)
      popq    %rbp
      .cfi_def_cfa %rsp, 8
      ret
  .Lcatch_landing_pad:                     # landing pad — should NOT be reached
      movl    $1, catch_handler_entered(%rip)
      xorl    %eax, %eax
      popq    %rbp
      ret
      .cfi_endproc
      .size frame_with_catch_all, .-frame_with_catch_all

  # LSDA for catch-all
  .section .gcc_except_table,"a",@progbits
  .LLSDA_catch:
      .byte   0xFF                        # @LPStart encoding: omit
      .byte   0xFF                        # @TType encoding: omit
      .byte   0x01                        # call-site encoding: uleb128
      .uleb128 .Lcatch_cs_end - .Lcatch_cs_start
  .Lcatch_cs_start:
      .uleb128 .Lcatch_call_start - frame_with_catch_all
      .uleb128 .Lcatch_call_end - .Lcatch_call_start
      .uleb128 .Lcatch_landing_pad - frame_with_catch_all
      .uleb128 1                          # action: index 1
  .Lcatch_cs_end:
      .sleb128 0                          # ttype_index = 0 → catch-all
      .sleb128 0                          # next action = 0 → end
  .text

  # ---------------------------------------------------------------
  # frame_with_cleanup(trigger: fn ptr in %rdi)
  # LSDA has cleanup-only entry (action == 0).
  # Forced unwind: personality MUST install this handler.
  # ---------------------------------------------------------------
  .globl frame_with_cleanup
  .type frame_with_cleanup, @function
  frame_with_cleanup:
      .cfi_startproc
      .cfi_personality 0, ori_eh_personality
      .cfi_lsda 0, .LLSDA_cleanup
      pushq   %rbp
      .cfi_def_cfa_offset 16
      .cfi_offset %rbp, -16
      movq    %rsp, %rbp
      .cfi_def_cfa_register %rbp
  .Lclean_call_start:
      callq   *%rdi
  .Lclean_call_end:
      xorl    %eax, %eax
      popq    %rbp
      .cfi_def_cfa %rsp, 8
      ret
  .Lclean_landing_pad:                     # cleanup pad — SHOULD be reached
      movl    $1, cleanup_handler_entered(%rip)
      movq    %rax, %rdi                  # exception object (data reg 0 = %rax on x86-64)
      callq   _Unwind_Resume@PLT
      .cfi_endproc
      .size frame_with_cleanup, .-frame_with_cleanup

  .section .gcc_except_table,"a",@progbits
  .LLSDA_cleanup:
      .byte   0xFF
      .byte   0xFF
      .byte   0x01
      .uleb128 .Lclean_cs_end - .Lclean_cs_start
  .Lclean_cs_start:
      .uleb128 .Lclean_call_start - frame_with_cleanup
      .uleb128 .Lclean_call_end - .Lclean_call_start
      .uleb128 .Lclean_landing_pad - frame_with_cleanup
      .uleb128 0                          # action: 0 → cleanup only
  .Lclean_cs_end:
  .text

  # Shared state
  .bss
  .globl catch_handler_entered
  catch_handler_entered: .long 0
  .globl cleanup_handler_entered
  cleanup_handler_entered: .long 0
  ```

- [ ] Create `compiler/ori_rt/src/test_frames_aarch64.S`:

  aarch64 (ARM64) AAPCS64 assembly stubs — same LSDA layout, different registers.

  ```asm
  # aarch64 AAPCS64 — test frames for ori_eh_personality
  .text

  # ---------------------------------------------------------------
  # frame_with_catch_all(trigger: fn ptr in x0)
  # ---------------------------------------------------------------
  .globl frame_with_catch_all
  .type frame_with_catch_all, @function
  frame_with_catch_all:
      .cfi_startproc
      .cfi_personality 0, ori_eh_personality
      .cfi_lsda 0, .LLSDA_catch
      stp     x29, x30, [sp, #-16]!       // save fp + lr
      .cfi_def_cfa_offset 16
      .cfi_offset x29, -16
      .cfi_offset x30, -8
      mov     x29, sp
      .cfi_def_cfa_register x29
      mov     x9, x0                      // save trigger ptr (x0 is callee-clobbered)
  .Lcatch_call_start:
      blr     x9                          // call trigger(void)
  .Lcatch_call_end:
      mov     w0, #0                      // return 0
      ldp     x29, x30, [sp], #16
      .cfi_def_cfa sp, 0
      ret
  .Lcatch_landing_pad:
      adrp    x9, catch_handler_entered
      mov     w10, #1
      str     w10, [x9, :lo12:catch_handler_entered]
      mov     w0, #0
      ldp     x29, x30, [sp], #16
      ret
      .cfi_endproc
      .size frame_with_catch_all, .-frame_with_catch_all

  # LSDA — identical wire format to x86-64 (DWARF, not arch-specific)
  .section .gcc_except_table,"a",@progbits
  .LLSDA_catch:
      .byte   0xFF
      .byte   0xFF
      .byte   0x01
      .uleb128 .Lcatch_cs_end - .Lcatch_cs_start
  .Lcatch_cs_start:
      .uleb128 .Lcatch_call_start - frame_with_catch_all
      .uleb128 .Lcatch_call_end - .Lcatch_call_start
      .uleb128 .Lcatch_landing_pad - frame_with_catch_all
      .uleb128 1
  .Lcatch_cs_end:
      .sleb128 0
      .sleb128 0
  .text

  # ---------------------------------------------------------------
  # frame_with_cleanup(trigger: fn ptr in x0)
  # ---------------------------------------------------------------
  .globl frame_with_cleanup
  .type frame_with_cleanup, @function
  frame_with_cleanup:
      .cfi_startproc
      .cfi_personality 0, ori_eh_personality
      .cfi_lsda 0, .LLSDA_cleanup
      stp     x29, x30, [sp, #-16]!
      .cfi_def_cfa_offset 16
      .cfi_offset x29, -16
      .cfi_offset x30, -8
      mov     x29, sp
      .cfi_def_cfa_register x29
      mov     x9, x0
  .Lclean_call_start:
      blr     x9
  .Lclean_call_end:
      mov     w0, #0
      ldp     x29, x30, [sp], #16
      .cfi_def_cfa sp, 0
      ret
  .Lclean_landing_pad:
      adrp    x9, cleanup_handler_entered
      mov     w10, #1
      str     w10, [x9, :lo12:cleanup_handler_entered]
      // x0 already holds exception object (data reg 0 = x0 on aarch64)
      bl      _Unwind_Resume
      .cfi_endproc
      .size frame_with_cleanup, .-frame_with_cleanup

  .section .gcc_except_table,"a",@progbits
  .LLSDA_cleanup:
      .byte   0xFF
      .byte   0xFF
      .byte   0x01
      .uleb128 .Lclean_cs_end - .Lclean_cs_start
  .Lclean_cs_start:
      .uleb128 .Lclean_call_start - frame_with_cleanup
      .uleb128 .Lclean_call_end - .Lclean_call_start
      .uleb128 .Lclean_landing_pad - frame_with_cleanup
      .uleb128 0
  .Lclean_cs_end:
  .text

  # Shared state
  .bss
  .globl catch_handler_entered
  catch_handler_entered: .long 0
  .globl cleanup_handler_entered
  cleanup_handler_entered: .long 0
  ```

  **Key aarch64 differences from x86-64:**
  - First arg in `x0`, saved to `x9` before `blr` (indirect call clobbers `x0`)
  - Frame: `stp x29, x30, [sp, #-16]!` (push fp+lr pair) / `ldp x29, x30, [sp], #16` (pop)
  - PC-relative global access: `adrp x9, symbol` + `str w10, [x9, :lo12:symbol]` (two-instruction sequence vs x86-64's single `symbol(%rip)`)
  - Exception object arrives in `x0` (data register 0 on aarch64) — no move needed before `bl _Unwind_Resume`
  - `bl` (branch-with-link) instead of `callq @PLT` — aarch64 ELF uses veneers for PLT, not explicit `@PLT` suffix

  **Design notes (both architectures):**
  - Each frame takes a `void (*trigger)(void)` and calls it. The LSDA maps that call range to a landing pad.
  - `frame_with_catch_all` has a catch-all action (`ttype_index == 0`). During forced unwind, the personality must **not** install this pad.
  - `frame_with_cleanup` has a cleanup action (`action == 0`). During forced unwind, the personality **must** install this pad. The pad calls `_Unwind_Resume` to continue unwinding.
  - Global flags (`catch_handler_entered`, `cleanup_handler_entered`) are read by the C harness to verify behavior.
  - The LSDA sections (`.gcc_except_table`) are byte-identical between architectures — DWARF encoding is architecture-independent.

- [ ] Create `compiler/ori_rt/src/test_forced_unwind.c` (~50 lines):

  C harness that sets up the exception object, triggers forced unwind, and checks results.

  ```c
  #include <unwind.h>
  #include <stdint.h>
  #include <string.h>

  // Defined in test_frames_{x86_64,aarch64}.S
  extern int catch_handler_entered;
  extern int cleanup_handler_entered;
  extern int frame_with_catch_all(void (*trigger)(void));
  extern int frame_with_cleanup(void (*trigger)(void));

  // Stop function: allows unwinding through all frames
  static _Unwind_Reason_Code force_unwind_stop(
      int version, _Unwind_Action actions, uint64_t exception_class,
      struct _Unwind_Exception *exc, struct _Unwind_Context *ctx,
      void *stop_parameter)
  {
      if (actions & _UA_END_OF_STACK) return _URC_NORMAL_STOP;
      return _URC_NO_REASON;
  }

  static struct _Unwind_Exception test_exc;

  // Exception cleanup callback (required by _Unwind_Exception contract)
  static void exc_cleanup(_Unwind_Reason_Code reason,
                          struct _Unwind_Exception *exc)
  {
      (void)reason; (void)exc;
  }

  static void trigger_forced_unwind(void) {
      memset(&test_exc, 0, sizeof(test_exc));
      test_exc.exception_cleanup = exc_cleanup;
      _Unwind_ForcedUnwind(&test_exc, force_unwind_stop, NULL);
  }

  // Returns 0 on success: catch-all handler was NOT entered
  int test_forced_unwind_skips_catch(void) {
      catch_handler_entered = 0;
      frame_with_catch_all(trigger_forced_unwind);
      return catch_handler_entered;  // 0 = pass, 1 = fail
  }

  // Returns 0 on success: cleanup handler WAS entered
  int test_forced_unwind_runs_cleanup(void) {
      cleanup_handler_entered = 0;
      frame_with_cleanup(trigger_forced_unwind);
      return cleanup_handler_entered ? 0 : 1;  // 0 = pass, 1 = fail
  }
  ```

  **Key details:**
  - `test_exc` is zero-initialized with `memset`, then `exception_cleanup` is set to a no-op callback. The `_Unwind_Exception` contract requires this field — without it, the unwinder may call through a null pointer when the exception is "caught" or the unwind completes.
  - `test_forced_unwind_skips_catch` returns 0 (pass) if the catch-all landing pad was never entered.
  - `test_forced_unwind_runs_cleanup` returns 0 (pass) if the cleanup landing pad was entered.

- [ ] Update `build.rs` to compile test files for supported architectures:
  ```rust
  fn main() {
      // --- Personality function (always built, all targets) ---
      let mut build = cc::Build::new();
      build
          .file("src/eh_personality.c")
          .flag("-std=c11")
          .warnings(true)
          .extra_warnings(true);
      build.flag_if_supported("-fno-exceptions");
      build.compile("ori_eh");

      // --- Forced-unwind test harness (Linux x86-64 + aarch64) ---
      // Per-arch assembly stubs; C harness is arch-independent.
      // Always compiled (not #[cfg(test)] — that gates build-script-self-tests,
      // not `cargo test -p ori_rt`). Linker dead-strips unreferenced symbols.
      let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
      let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

      if target_os == "linux" {
          let asm_file = match target_arch.as_str() {
              "x86_64" => Some("src/test_frames_x86_64.S"),
              "aarch64" => Some("src/test_frames_aarch64.S"),
              _ => None,
          };
          if let Some(asm) = asm_file {
              cc::Build::new()
                  .file("src/test_forced_unwind.c")
                  .file(asm)
                  .flag("-std=c11")
                  .warnings(true)
                  .compile("ori_eh_test");
          }
      }
  }
  ```

  **Target gating:** `build.rs` reads `CARGO_CFG_TARGET_ARCH` and `CARGO_CFG_TARGET_OS` (set by Cargo for the *target* platform, not the host). On Linux, it selects the architecture-specific assembly file. On non-Linux or unsupported architectures, the test harness is skipped entirely. `#[cfg(test)]` in `build.rs` would not work because it gates build-script-self-tests, not `cargo test -p ori_rt`.

- [ ] Create `compiler/ori_rt/tests/forced_unwind.rs`:
  ```rust
  //! Tests that ori_eh_personality handles _UA_FORCE_UNWIND correctly.
  //!
  //! Linux only (x86-64 + aarch64) — the test frames use per-arch assembly
  //! stubs with ELF-specific directives. Skipped on other targets via #[cfg].
  //!
  //! These tests call C/assembly test frames (compiled from test_forced_unwind.c
  //! and test_frames_{x86_64,aarch64}.S) that use ori_eh_personality with
  //! hand-written LSDA entries. _Unwind_ForcedUnwind is triggered through frames
  //! with catch-all and cleanup landing pads to verify the personality does not
  //! install catch handlers during forced unwind, while still running cleanup pads.

  #![cfg(all(
      target_os = "linux",
      any(target_arch = "x86_64", target_arch = "aarch64")
  ))]

  extern "C" {
      fn test_forced_unwind_skips_catch() -> i32;
      fn test_forced_unwind_runs_cleanup() -> i32;
  }

  /// Single test function to avoid data races on shared C globals
  /// (`catch_handler_entered`, `cleanup_handler_entered`).
  ///
  /// Rust's default test runner executes #[test] functions in parallel.
  /// The C test harness uses unsynchronized global flags written by
  /// assembly landing pads, so concurrent execution would race.
  /// A single test function serializes both checks naturally.
  #[test]
  fn forced_unwind_personality_behavior() {
      // Catch-all pads must NOT be installed during forced unwind
      let result = unsafe { test_forced_unwind_skips_catch() };
      assert_eq!(result, 0, "catch-all handler should not run during forced unwind");

      // Cleanup pads MUST still run during forced unwind
      let result = unsafe { test_forced_unwind_runs_cleanup() };
      assert_eq!(result, 0, "cleanup pads should still run during forced unwind");
  }
  ```

  The `#![cfg(...)]` at the crate level ensures the entire test file is skipped on non-matching targets. This matches the `build.rs` gate — both accept `(x86_64 | aarch64) + linux`. On skipped targets, `cargo test -p ori_rt` reports 0 tests from this file (not a failure).

  **ARM testing:** Spin up a preemptible GCP `t2a-standard-2` (aarch64) instance, run `cargo test -p ori_rt`, then destroy it. This validates the aarch64 assembly stubs on real hardware.

---

## 01.5 Completion Checklist

- [ ] `eh_personality.c` exists in `compiler/ori_rt/src/` and compiles without warnings
- [ ] `build.rs` uses `cc` crate to compile the C file
- [ ] `nm target/debug/libori_rt.a | grep ori_eh_personality` returns the symbol (T = text section)
- [ ] `ori_eh_personality_addr()` is exported from `ori_rt` and callable from `ori_llvm`
- [ ] `cargo build -p ori_rt` succeeds (both rlib and staticlib)
- [ ] No new Clippy warnings in `ori_rt`
- [ ] `cargo test -p ori_rt` passes forced-unwind tests on x86-64 Linux (catch skipped, cleanup ran)
- [ ] `cargo test -p ori_rt` passes forced-unwind tests on aarch64 Linux (native execution on GCP preemptible instance — cross-compilation alone is insufficient, tests must run)
- [ ] Unsupported targets (non-Linux or unsupported arch): forced-unwind tests cleanly skipped (0 tests, not failure)

**Exit Criteria:** `ori_eh_personality` symbol is present in both `libori_rt.a` and the rlib, and its address is obtainable via `ori_rt::ori_eh_personality_addr()`. The function implements correct LSDA parsing for cleanup and catch-all landing pads per the Itanium EH ABI. On x86-64 and aarch64 Linux: forced-unwind tests verify catch-all pads are skipped and cleanup pads still run under `_UA_FORCE_UNWIND`. On unsupported targets (non-Linux or unsupported arch): forced-unwind tests are skipped cleanly. Verified via `nm`, `cargo build -p ori_rt`, and `cargo test -p ori_rt`.
