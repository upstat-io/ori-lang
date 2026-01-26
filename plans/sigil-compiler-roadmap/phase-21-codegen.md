# Phase 21: Code Generation

**Goal**: Compile to native code

> **DESIGN**: Implementation-specific (no spec)

---

## 21.1 C Backend

- [ ] **Implement**: Expression compilation
  - [ ] **Rust Tests**: `sigilc/src/codegen/c/expr.rs` — C expression codegen
  - [ ] **Sigil Tests**: `tests/spec/codegen/c_expressions.si`

- [ ] **Implement**: Function compilation
  - [ ] **Rust Tests**: `sigilc/src/codegen/c/function.rs` — C function codegen
  - [ ] **Sigil Tests**: `tests/spec/codegen/c_functions.si`

- [ ] **Implement**: Type mapping to C
  - [ ] **Rust Tests**: `sigilc/src/codegen/c/types.rs` — C type mapping
  - [ ] **Sigil Tests**: `tests/spec/codegen/c_types.si`

---

## 21.2 LLVM Backend (Alternative)

- [ ] **Implement**: LLVM IR generation
  - [ ] **Rust Tests**: `sigilc/src/codegen/llvm/ir.rs` — LLVM IR generation
  - [ ] **Sigil Tests**: `tests/spec/codegen/llvm_ir.si`

- [ ] **Implement**: Optimization passes
  - [ ] **Rust Tests**: `sigilc/src/codegen/llvm/opt.rs` — optimization passes
  - [ ] **Sigil Tests**: `tests/spec/codegen/llvm_opt.si`

- [ ] **Implement**: Native code output
  - [ ] **Rust Tests**: `sigilc/src/codegen/llvm/native.rs` — native output
  - [ ] **Sigil Tests**: `tests/spec/codegen/llvm_native.si`

---

## 21.3 Runtime

- [ ] **Implement**: Memory management
  - [ ] **Rust Tests**: `sigilc/src/runtime/memory.rs` — memory management
  - [ ] **Sigil Tests**: `tests/spec/runtime/memory.si`

- [ ] **Implement**: Garbage collection or ownership
  - [ ] **Rust Tests**: `sigilc/src/runtime/gc.rs` — garbage collection
  - [ ] **Sigil Tests**: `tests/spec/runtime/gc.si`

- [ ] **Implement**: FFI support
  - [ ] **Rust Tests**: `sigilc/src/runtime/ffi.rs` — FFI runtime
  - [ ] **Sigil Tests**: `tests/spec/runtime/ffi.si`

---

## 21.4 Phase Completion Checklist

- [ ] All items above have all three checkboxes marked `[x]`
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage
- [ ] Run full test suite: `cargo test && sigil test tests/spec/`

**Exit Criteria**: Native binaries run correctly
