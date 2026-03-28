# Section 11: FFI — Verification Results

**Verified**: 2026-03-28
**Status in roadmap**: not-started
**Actual status**: PARTIAL — significant parser/IR/formatter infrastructure exists; type checker has repr validation; codegen has `declare_extern_function` infra. No eval or end-to-end FFI calling.

## Summary

The FFI section is marked `not-started` but has substantial hidden implementation:
- **Extern block parsing**: COMPLETE (parser, IR, formatter, incremental copier, tests)
- **C variadic parsing**: COMPLETE (parser handles `...` in extern blocks)
- **`#repr` attribute**: COMPLETE at parse+IR+typeck+repr-opt levels
- **`unsafe` keyword**: Lexed as token; no parser/typeck/eval handling
- **FFI capability**: Not implemented
- **Codegen for user extern blocks**: Not implemented (only runtime function declarations exist)
- **Spec files**: Both `spec/25-conditional-compilation.md` and `spec/26-ffi.md` EXIST

---

## 11.1 Extern Block Syntax (94 items total section, this subsection)

### Spec
- [done] `spec/26-ffi.md` EXISTS with extern block syntax, calling conventions, linkage semantics
  - File: `docs/ori_lang/v2026/spec/26-ffi.md`

### Lexer
- [done] `extern` keyword token — `compiler/ori_lexer/src/keywords/mod.rs:110`
- [done] String literals for ABI ("c", "js") — standard string token, validated in parser

### Parser
- [done] `parse_extern_block()` — `compiler/ori_parse/src/grammar/item/extern_def.rs:22`
- [done] `ExternBlock` AST node — `compiler/ori_ir/src/ast/items/extern_def.rs:64`
- [done] `ExternItem` variants — `compiler/ori_ir/src/ast/items/extern_def.rs:37`
- [done] `from "lib"` library specification — parser line 74, contextual keyword check
- [done] `as "name"` name mapping — parser line 148

### Type checker
- [todo] Ensure types are FFI-safe — no FFI-safety validation in `ori_types`
- [todo] Check for `uses FFI` in callers — no FFI capability tracking

### Codegen
- [partial] `declare_extern_function` exists in `ori_llvm/src/codegen/ir_builder/calls.rs:256` — used for runtime functions only
- [todo] No codegen path from user `ExternBlock` AST to LLVM `declare`
- [todo] No calling convention handling for user extern blocks
- [todo] No external symbol linking from user code

### Formatter
- [done] `format_extern_block` — `compiler/ori_fmt/src/declarations/extern_def.rs`

### Incremental
- [done] Incremental copier handles extern blocks — `compiler/ori_parse/src/incremental/copier.rs:1514`

### Tests
- [done] Parser tests: 18 tests in `compiler/oric/tests/phases/parse/extern_def.rs`
  - Basic extern c/js, empty block, from clause, as alias, C variadic, pub/private, multiple items, multiple blocks, mixed with functions, error cases
- [todo] No spec tests (`tests/spec/ffi/extern_blocks.ori` does not exist)
- [todo] No LLVM tests, no AOT tests

---

## 11.2 C ABI Types

- [todo] `CPtr` type — not in type system (no `TypeId` variant, no pool entry)
- [todo] C type aliases (`c_int`, `c_long`, etc.) — not registered in type checker
  - NOTE: `c_int` appears in test strings and runtime Rust code, but NOT as Ori-level types
- [todo] Size/alignment handling for C types
- [todo] Platform-dependent sizes
- [todo] FFI type validation
- [todo] No spec tests

---

## 11.3 #repr Attribute

- [done] IR `ReprAttrKind` enum — `compiler/ori_ir/src/ast/items/types.rs:22` with C, Packed, Transparent, Aligned(u64), CAligned(u64)
- [done] Parser `#repr("c")`, `#repr("packed")`, `#repr("transparent")`, `#repr("aligned", N)` — `compiler/ori_parse/src/grammar/attr/repr.rs`
- [done] Combined syntax `#repr("c", "aligned", 16)` — parser supports in-paren combinations
- [done] Type checker validation and merging — `compiler/ori_types/src/check/registration/user_types.rs:186` (`validate_and_merge_repr_attrs`)
  - Validates: transparent requires single field, aligned must be power of two, rejects packed+aligned, etc.
- [done] `ori_repr` crate `ReprAttribute` enum — `compiler/ori_repr/src/plan/repr_attr.rs`
- [done] Type checker tests for repr validation — `compiler/ori_types/src/check/registration/tests.rs` (transparent, duplicate C, duplicate aligned, C+aligned)
- [todo] LLVM codegen for repr (packed struct type, transparent, aligned) — not directly wired for user types
- [todo] No spec tests (`tests/spec/ffi/repr.ori` does not exist)
- [todo] No compile-fail tests for invalid repr combinations

---

## 11.4 Unsafe Expressions

- [done] `unsafe` keyword lexed — `compiler/ori_lexer/src/keywords/mod.rs:112`
- [todo] Parser does NOT parse `unsafe { ... }` blocks — no `parse_unsafe_block` function
- [todo] No `in_unsafe` flag in type checker
- [todo] No evaluator handling
- [todo] No codegen
- [todo] No tests

---

## 11.5 FFI Capability

- [todo] `FFI` capability not defined in capability system
- [todo] Not tracked in function signatures
- [todo] Not enforced by type checker
- [todo] No tests

---

## 11.6 Callbacks (Native)

- [todo] Function pointer type in FFI context not implemented
- [todo] No trampoline generation
- [todo] No tests

---

## 11.7 Build System Integration

- [todo] `ori.toml` native section not implemented
- [todo] No link directive generation
- [todo] No pkg-config integration
- [todo] No tests

---

## 11.8 compile_error Built-in

- [todo] `compile_error` not recognized as a built-in function
  - Not in `ori_types/src/infer/expr/identifiers.rs`
  - Not in `ori_eval/src/function_val.rs`
  - Not in `library/std/prelude.ori`
- [todo] No spec tests

---

## 11.9 WASM Target (Section 2)

- [todo] No WASM-specific codegen for user extern "js" blocks
- [todo] No JS glue generation
- NOTE: WASM cross-compilation infrastructure exists in `ori_llvm` (linker, config) but not for user FFI

---

## 11.10 JsValue and Async (Section 3-4)

- [todo] JsValue type not in type system
- [todo] JsPromise type not in type system
- [todo] No implicit resolution
- [todo] No tests

---

## 11.11 Deep FFI

- [todo] All Deep FFI features (error protocols, ownership, marshalling, testability, const-generic safety) are not started
- [todo] No `FfiError` type
- [todo] No `out` parameter modifier
- [todo] No ownership annotations

---

## Correction Needed

The roadmap status should be changed from `not-started` to `partial`. Key completed items:
1. Extern block parsing (parser + IR + formatter + tests) — subsection 11.1 parser portion
2. C variadic parsing in extern blocks — subsection 11.1/12.4
3. `#repr` attribute (parse + IR + typeck validation + repr-opt) — subsection 11.3
4. `unsafe` keyword lexing — subsection 11.4 lexer portion
5. Spec files exist for both FFI and conditional compilation
6. `declare_extern_function` infrastructure in LLVM backend

Estimated: ~25% of section items have hidden implementation.
