---
section: "02"
title: "Codegen Integration"
status: not-started
goal: "All LLVM IR emitted by Ori references @ori_eh_personality with zero mentions of rust_eh_personality"
depends_on: ["01"]
sections:
  - id: "02.1"
    title: "Runtime Declaration Table"
    status: not-started
  - id: "02.2"
    title: "ARC Emitter Personality Attachment"
    status: not-started
  - id: "02.3"
    title: "JIT Symbol Mapping"
    status: not-started
  - id: "02.4"
    title: "Remove rust_eh_personality References"
    status: not-started
  - id: "02.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Codegen Integration

**Status:** Not Started
**Goal:** Every reference to `rust_eh_personality` in the LLVM codegen and JIT infrastructure is replaced with `ori_eh_personality`. The emitted LLVM IR contains `personality ptr @ori_eh_personality` on functions with `invoke`/`landingpad`. JIT execution resolves the symbol to the C function in `ori_rt`.

**Context:** The symbol `rust_eh_personality` appears in 4 locations across `ori_llvm`:
1. Runtime declaration table (`runtime_decl/mod.rs:702`) — declares the symbol to LLVM
2. ARC emitter personality attachment (`arc_emitter/mod.rs:377`) — attaches to functions
3. JIT mapped functions list (`evaluator.rs:595`) — registers for JIT resolution
4. JIT address mapping (`evaluator.rs:729`) — provides runtime address

All 4 must be updated atomically. Missing any one creates a mismatch: LLVM IR references a symbol that doesn't exist, or JIT can't resolve it.

**Depends on:** Section 01 (`ori_eh_personality` must exist before we reference it).

---

## 02.1 Runtime Declaration Table

**File(s):** `compiler/ori_llvm/src/codegen/runtime_decl/mod.rs`

The `RT_FUNCTIONS` table is the single source of truth for all runtime function declarations. Every runtime function used in LLVM IR must be declared here.

- [ ] Rename the entry at line ~700-706:
  ```rust
  // Before:
  RtFn {
      name: "rust_eh_personality",
      params: &[Ty::I32],
      ret: Some(Ty::I32),
      attrs: &[Attr::Nounwind],
  },

  // After:
  RtFn {
      name: "ori_eh_personality",
      params: &[Ty::I32],
      ret: Some(Ty::I32),
      attrs: &[Attr::Nounwind],
  },
  ```

  The signature `(i32) -> i32` is a placeholder — LLVM never calls the personality through this declaration. It only needs the symbol to exist so it can set it as the personality on functions. The actual calling convention is dictated by the Itanium EH ABI and handled by the platform unwinder.

  The `Nounwind` attribute is correct: the personality function itself must not throw (it's called during exception handling — throwing from the personality is undefined behavior).

---

## 02.2 ARC Emitter Personality Attachment

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`

The ARC emitter attaches the personality function to LLVM functions that contain `invoke`/`landingpad`.

- [ ] Update the `runtime_fn` call at line ~377:
  ```rust
  // Before:
  let pid = self.builder.runtime_fn("rust_eh_personality");

  // After:
  let pid = self.builder.runtime_fn("ori_eh_personality");
  ```

  This is the code path that generates `personality ptr @ori_eh_personality` in the LLVM IR for every function with unwind blocks. The `runtime_fn()` method returns a cached `FunctionId` referencing the declaration from the `RT_FUNCTIONS` table (Section 02.1).

---

## 02.3 JIT Symbol Mapping

**File(s):** `compiler/ori_llvm/src/evaluator.rs`

The JIT execution engine needs explicit symbol mappings for functions not in the dynamic symbol table. The personality function is one of these.

- [ ] Update `JIT_MAPPED_RUNTIME_FUNCTIONS` array (line ~595):
  ```rust
  // Before:
  "rust_eh_personality",

  // After:
  "ori_eh_personality",
  ```

- [ ] Update the mapping in `add_runtime_mappings_to_engine()` (line ~729):
  ```rust
  // Before:
  ("rust_eh_personality", rust_eh_personality_addr()),

  // After:
  ("ori_eh_personality", runtime::ori_eh_personality_addr()),
  ```

  Note the change from `rust_eh_personality_addr()` (local helper) to `runtime::ori_eh_personality_addr()` (from `ori_rt` via Section 01.3). The address now points to the C implementation instead of Rust's personality.

---

## 02.4 Remove rust_eh_personality References

**File(s):** `compiler/ori_llvm/src/evaluator.rs`

- [ ] Delete the `rust_eh_personality_addr()` helper function (lines ~750-761):
  ```rust
  // DELETE THIS ENTIRE FUNCTION:
  /// Get the address of `rust_eh_personality` for JIT symbol mapping.
  ///
  /// This function is defined in the Rust standard library and handles
  /// DWARF-based exception handling (Itanium ABI). It's present in the
  /// host binary but not exported in the dynamic symbol table, so the
  /// LLVM MCJIT can't resolve it via `dlsym`. We provide it explicitly.
  fn rust_eh_personality_addr() -> usize {
      extern "C" {
          fn rust_eh_personality();
      }
      rust_eh_personality as *const () as usize
  }
  ```

  This function is replaced by `ori_rt::ori_eh_personality_addr()` which points to the C implementation.

- [ ] Verify with `grep -r "rust_eh_personality" compiler/` returns zero results.

- [ ] Update the comment on the JIT mapping (line ~726-728) to reference Ori's personality:
  ```rust
  // Exception handling personality function — required by any function
  // containing `invoke`/`landingpad`. Implemented in C (ori_rt/src/eh_personality.c).
  // Not in the dynamic symbol table, so MCJIT needs explicit mapping.
  ```

---

## 02.5 Completion Checklist

- [ ] `grep -r "rust_eh_personality" compiler/` returns 0 results
- [ ] `grep -r "ori_eh_personality" compiler/ori_llvm/` returns results in:
  - `codegen/runtime_decl/mod.rs` (declaration)
  - `codegen/arc_emitter/mod.rs` (personality attachment)
  - `evaluator.rs` (JIT mapping, 2 locations)
- [ ] `ORI_DEBUG_LLVM=1 ori check tests/spec/functions/recursion.ori 2>&1 | grep personality` shows `@ori_eh_personality`
- [ ] No compilation errors in `ori_llvm`
- [ ] `cargo cll` (LLVM clippy) clean

**Exit Criteria:** `ORI_DEBUG_LLVM=1 ori build <any-program-with-invoke>` emits IR containing `personality ptr @ori_eh_personality` on all functions with landing pads. `grep -r "rust_eh_personality" compiler/` returns zero matches. Both JIT and AOT paths resolve the symbol correctly.
