
## 21.1 LLVM Setup & Infrastructure

- [ ] **Setup**: LLVM development environment
  - [ ] Install LLVM 17+ and development headers
  - [ ] Add `inkwell` or `llvm-sys` crate to `compiler/oric/Cargo.toml`
  - [ ] Verify LLVM bindings compile and link correctly
  - [ ] Create `oric/src/codegen/llvm/mod.rs` module structure

- [ ] **Implement**: LLVM context and module initialization
  - [ ] **Rust Tests**: `oric/src/codegen/llvm/context.rs` — context management
  - [ ] Create LLVM context, module, and builder abstractions

- [ ] **Implement**: Basic target configuration
  - [ ] **Rust Tests**: `oric/src/codegen/llvm/target.rs` — target triple setup
  - [ ] Support native target detection
  - [ ] Configure data layout and target features

---

## 21.2 LLVM IR Generation

- [ ] **Implement**: Type lowering to LLVM types
  - [ ] **Rust Tests**: `oric/src/codegen/llvm/types.rs` — type mapping
  - [ ] Map Ori primitives (int → i64, float → f64, bool → i1, etc.)
  - [ ] Map structs to LLVM struct types
  - [ ] Map sum types to tagged unions
  - [ ] Handle function types and closures

- [ ] **Implement**: Expression codegen
  - [ ] **Rust Tests**: `oric/src/codegen/llvm/expr.rs` — expression lowering
  - [ ] **Ori Tests**: `tests/spec/codegen/llvm_expressions.ori`
  - [ ] Literals, binary ops, unary ops
  - [ ] Function calls and method dispatch
  - [ ] Field access and indexing

- [ ] **Implement**: Function codegen
  - [ ] **Rust Tests**: `oric/src/codegen/llvm/function.rs` — function lowering
  - [ ] **Ori Tests**: `tests/spec/codegen/llvm_functions.ori`
  - [ ] Function signatures and calling conventions
  - [ ] Local variables (alloca)
  - [ ] Control flow (if/else, loops, match)
  - [ ] Return statements

- [ ] **Implement**: Control flow graphs
  - [ ] **Rust Tests**: `oric/src/codegen/llvm/cfg.rs` — CFG construction
  - [ ] Basic block creation and linking
  - [ ] Phi nodes for SSA form
  - [ ] Branch and conditional instructions

---

## 21.3 Optimization Passes

- [ ] **Implement**: Optimization pipeline
  - [ ] **Rust Tests**: `oric/src/codegen/llvm/opt.rs` — optimization passes
  - [ ] Configure standard optimization levels (O0, O1, O2, O3)
  - [ ] Enable inlining, dead code elimination, constant folding
  - [ ] Add Ori-specific optimizations if needed

- [ ] **Implement**: Debug info generation
  - [ ] **Rust Tests**: `oric/src/codegen/llvm/debug.rs` — DWARF debug info
  - [ ] Source location tracking
  - [ ] Variable debug info
  - [ ] Type debug info

---

## 21.4 Native Code Output

- [ ] **Implement**: Object file generation
  - [ ] **Rust Tests**: `oric/src/codegen/llvm/object.rs` — object emission
  - [ ] **Ori Tests**: `tests/spec/codegen/llvm_native.ori`
  - [ ] Emit .o files for linking
  - [ ] Support ELF (Linux), Mach-O (macOS), COFF (Windows)

- [ ] **Implement**: Executable linking
  - [ ] **Rust Tests**: `oric/src/codegen/llvm/link.rs` — linker integration
  - [ ] Link with system linker (ld, lld, or link.exe)
  - [ ] Handle runtime library linking
  - [ ] Support static and dynamic linking

- [ ] **Implement**: JIT compilation (optional)
  - [ ] **Rust Tests**: `oric/src/codegen/llvm/jit.rs` — JIT engine
  - [ ] MCJIT or ORC JIT integration
  - [ ] Useful for REPL and testing

---

## 21.5 Runtime

- [ ] **Implement**: Memory management (ARC)
  - [ ] **Rust Tests**: `oric/src/runtime/memory.rs` — ARC memory management
  - [ ] **Ori Tests**: `tests/spec/runtime/memory.ori`
  - [ ] Reference counting implementation
  - [ ] Weak references for breaking cycles

- [ ] **Implement**: Runtime support functions
  - [ ] **Rust Tests**: `oric/src/runtime/support.rs` — runtime helpers
  - [ ] Panic handling
  - [ ] String operations
  - [ ] Collection operations

- [ ] **Implement**: FFI runtime support
  - [ ] **Rust Tests**: `oric/src/runtime/ffi.rs` — FFI runtime
  - [ ] **Ori Tests**: `tests/spec/runtime/ffi.ori`
  - [ ] C ABI compatibility
  - [ ] Callback support

---

## 21.6 Phase Completion Checklist

- [ ] All items above have all three checkboxes marked `[x]`
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage
- [ ] Run full test suite: `cargo test && ori test tests/spec/`

**Exit Criteria**: Native binaries run correctly via LLVM backend
