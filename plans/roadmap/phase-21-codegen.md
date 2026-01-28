# Phase 21: Code Generation

**Goal**: Compile to native code

> **DESIGN**: Implementation-specific (no spec)

---

## 21.1 C Backend

- [ ] **Implement**: Expression compilation
  - [ ] **Rust Tests**: `oric/src/codegen/c/expr.rs` — C expression codegen
  - [ ] **Ori Tests**: `tests/spec/codegen/c_expressions.ori`

- [ ] **Implement**: Function compilation
  - [ ] **Rust Tests**: `oric/src/codegen/c/function.rs` — C function codegen
  - [ ] **Ori Tests**: `tests/spec/codegen/c_functions.ori`

- [ ] **Implement**: Type mapping to C
  - [ ] **Rust Tests**: `oric/src/codegen/c/types.rs` — C type mapping
  - [ ] **Ori Tests**: `tests/spec/codegen/c_types.ori`

---

## 21.2 LLVM Backend (Alternative)

- [ ] **Implement**: LLVM IR generation
  - [ ] **Rust Tests**: `oric/src/codegen/llvm/ir.rs` — LLVM IR generation
  - [ ] **Ori Tests**: `tests/spec/codegen/llvm_ir.ori`

- [ ] **Implement**: Optimization passes
  - [ ] **Rust Tests**: `oric/src/codegen/llvm/opt.rs` — optimization passes
  - [ ] **Ori Tests**: `tests/spec/codegen/llvm_opt.ori`

- [ ] **Implement**: Native code output
  - [ ] **Rust Tests**: `oric/src/codegen/llvm/native.rs` — native output
  - [ ] **Ori Tests**: `tests/spec/codegen/llvm_native.ori`

---

## 21.3 Runtime

- [ ] **Implement**: Memory management
  - [ ] **Rust Tests**: `oric/src/runtime/memory.rs` — memory management
  - [ ] **Ori Tests**: `tests/spec/runtime/memory.ori`

- [ ] **Implement**: Garbage collection or ownership
  - [ ] **Rust Tests**: `oric/src/runtime/gc.rs` — garbage collection
  - [ ] **Ori Tests**: `tests/spec/runtime/gc.ori`

- [ ] **Implement**: FFI support
  - [ ] **Rust Tests**: `oric/src/runtime/ffi.rs` — FFI runtime
  - [ ] **Ori Tests**: `tests/spec/runtime/ffi.ori`

---

## 21.4 Phase Completion Checklist

- [ ] All items above have all three checkboxes marked `[x]`
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage
- [ ] Run full test suite: `./test-all`

**Exit Criteria**: Native binaries run correctly
