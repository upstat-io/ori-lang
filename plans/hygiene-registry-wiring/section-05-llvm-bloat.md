---
section: "05"
title: "LLVM Bloat Reduction (ori_llvm)"
status: done
goal: "Reduce function sizes and narrow crate-level clippy allows"
depends_on: []
sections:
  - id: "05.1"
    title: "Split compile_module_with_tests"
    status: done
  - id: "05.2"
    title: "generate_js_wrapper (no split needed)"
    status: done
  - id: "05.3"
    title: "Split emit_field_operation"
    status: done
  - id: "05.4"
    title: "Narrow clippy::too_many_lines allow"
    status: done
  - id: "05.5"
    title: "Completion Checklist"
    status: done
---

# Section 05: LLVM Bloat Reduction (ori_llvm)

**Status:** Done
**Goal:** Reduce oversized functions to under 100 lines (target <50) and narrow the crate-level `#![allow(clippy::too_many_lines)]` to per-function `#[expect]` annotations.

**Context:** Two functions exceed the 100-line target (`compile_module_with_tests` at 185 lines, `emit_field_operation` at 108 lines). `generate_js_wrapper` was originally reported at 142 lines but is actually 65 lines (no split needed). Additionally, `ori_llvm/src/lib.rs` has a crate-level `#![allow(clippy::too_many_lines)]` which suppresses the lint globally, hiding future violations. The lint should only be allowed on specific dispatch-table functions that genuinely need it.

---

## 05.1 Split compile_module_with_tests

**File(s):** `compiler/ori_llvm/src/evaluator/compile.rs:63` (185 lines)

- [x] Extracted `compile_all_functions()` helper (~110 lines) — type infrastructure + two-pass compilation + test wrappers
- [x] Extracted `finalize_jit()` helper — IR verification + JIT engine creation
- [x] `compile_module_with_tests` is now ~50 lines (calls helpers + passes arguments)
- [x] Named impl lifetime `'tcx` to resolve invariant `SimpleCx` borrow across extracted helper

---

## 05.2 generate_js_wrapper — no split needed, leak fix only

**File(s):** `compiler/ori_llvm/src/aot/wasm/mod.rs:335` (65 lines — under limit)

- [x] No action needed — function is already under 100 lines
- [x] String param cleanup fix done in Section 01.2

---

## 05.3 Split emit_field_operation

**File(s):** `compiler/ori_llvm/src/codegen/derive_codegen/field_ops.rs:21` (108 → 87 lines)

- [x] Extracted `emit_user_type_field_op()` helper — handles Struct/Enum arms (both share same match arm)
- [x] `emit_field_operation` is now 87 lines (under 100)

---

## 05.4 Narrow clippy::too_many_lines allow

**File(s):** `compiler/ori_llvm/src/lib.rs`

- [x] Removed crate-level `#![allow(clippy::too_many_lines)]` from `lib.rs`
- [x] Added `#[expect(clippy::too_many_lines, reason = "...")]` to 15 genuine dispatch/emission functions
- [x] Added `#![allow(clippy::too_many_lines)]` to 3 test modules (test setup is inherently verbose)
- [x] Fixed `double_must_use` warnings on `ori_registry` iterator-returning functions
- [x] `./clippy-all.sh` green

---

## 05.5 Completion Checklist

- [x] `compile_module_with_tests` is under 100 lines (50 lines)
- [x] `emit_field_operation` is under 100 lines (87 lines)
- [x] No crate-level `#![allow(clippy::too_many_lines)]` in `ori_llvm/src/lib.rs`
- [x] Per-function `#[expect]` on genuine dispatch tables only
- [x] `cargo test -p ori_llvm` passes
- [x] `./clippy-all.sh` green
