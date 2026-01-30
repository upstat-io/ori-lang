# Phase 21: LLVM Backend

**Status:** 🔶 Partial — JIT working, AOT pending

## Current Test Results (2026-01-28)

| Test Suite | Passed | Failed | Skipped | Total |
|------------|--------|--------|---------|-------|
| All Ori tests | 734 | 0 | 19 | 753 |
| Rust unit tests | 204 | 0 | 0 | 204 |

---

## 21.1 LLVM Setup & Infrastructure

- [x] **Setup**: LLVM development environment
  - [x] Docker container with LLVM 17 and development headers
  - [x] Add `inkwell` crate to `compiler/ori_llvm/Cargo.toml`
  - [x] Verify LLVM bindings compile and link correctly
  - [x] Create `compiler/ori_llvm/src/` module structure

- [x] **Implement**: LLVM context and module initialization
  - [x] **Rust Tests**: `context.rs` — SimpleCx, CodegenCx, TypeCache
  - [x] Create LLVM context, module, and builder abstractions

- [x] **Implement**: Basic target configuration
  - [x] Support native target detection (JIT)
  - [ ] Configure data layout and target features (AOT)

---

## 21.2 LLVM IR Generation

- [x] **Implement**: Type lowering to LLVM types
  - [x] **Rust Tests**: `types.rs`, `context.rs` — type mapping
  - [x] Map Ori primitives (int → i64, float → f64, bool → i1, char → i32, byte → i8)
  - [x] Map strings to `{ i64 len, ptr data }` struct
  - [x] Map Option/Result to `{ i8 tag, i64 payload }` tagged unions
  - [x] Map lists to `{ i64 len, i64 cap, ptr data }` struct
  - [x] Handle function types

- [x] **Implement**: Expression codegen
  - [x] **Rust Tests**: `tests/arithmetic_tests.rs`, `tests/operator_tests.rs`
  - [x] Literals (int, float, bool, string, char, byte)
  - [x] Binary ops (add, sub, mul, div, mod, comparisons, logical)
  - [x] Unary ops (neg, not)
  - [x] Function calls and method dispatch
  - [x] Field access and indexing

- [x] **Implement**: Function codegen
  - [x] **Rust Tests**: `tests/function_tests.rs`, `tests/function_call_tests.rs`
  - [x] Function signatures and calling conventions
  - [x] Local variables (in HashMap, not alloca — optimization pending)
  - [x] Control flow (if/else, loops, match)
  - [x] Return statements

- [x] **Implement**: Control flow graphs
  - [x] **Rust Tests**: `tests/control_flow_tests.rs`, `tests/advanced_control_flow_tests.rs`
  - [x] Basic block creation and linking
  - [x] Phi nodes for SSA form
  - [x] Branch and conditional instructions
  - [x] Break/continue with loop context

---

## 21.3 Optimization Passes

- [ ] **Implement**: Optimization pipeline
  - [ ] Configure standard optimization levels (O0, O1, O2, O3)
  - [ ] Enable inlining, dead code elimination, constant folding

- [ ] **Implement**: Debug info generation
  - [ ] Source location tracking
  - [ ] Variable debug info
  - [ ] Type debug info

---

## 21.4 Native Code Output

- [ ] **Implement**: Object file generation
  - [ ] Emit .o files for linking
  - [ ] Support ELF (Linux), Mach-O (macOS), COFF (Windows)

- [ ] **Implement**: Executable linking
  - [ ] Link with system linker (ld, lld, or link.exe)
  - [ ] Handle runtime library linking
  - [ ] Support static and dynamic linking

- [x] **Implement**: JIT compilation
  - [x] **Rust Tests**: `tests/evaluator_tests.rs`, `module.rs`
  - [x] MCJIT integration via inkwell
  - [x] Runtime function mapping for JIT execution
  - [x] Used for test execution

---

## 21.5 Runtime

- [x] **Implement**: Runtime support functions
  - [x] **Rust Tests**: `tests/runtime_tests.rs`
  - [x] `ori_print`, `ori_print_int`, `ori_print_float`, `ori_print_bool`
  - [x] `ori_panic`, `ori_panic_cstr`
  - [x] `ori_assert`, `ori_assert_eq_int`, `ori_assert_eq_bool`, `ori_assert_eq_str`
  - [x] `ori_str_concat`, `ori_str_eq`, `ori_str_ne`
  - [x] `ori_str_from_int`, `ori_str_from_bool`, `ori_str_from_float`
  - [x] `ori_list_new`, `ori_list_free`, `ori_list_len`
  - [x] `ori_compare_int`, `ori_min_int`, `ori_max_int`

- [ ] **Implement**: Memory management (ARC)
  - [ ] Reference counting implementation
  - [ ] Atomic refcount operations (fetch-add, fetch-sub)
  - [ ] Weak references for breaking cycles

## 21.6 Memory Model Edge Cases

**Proposal**: `proposals/approved/memory-model-edge-cases-proposal.md`

Custom destructors, destruction ordering, and panic behavior.

- [ ] **Implement**: Drop trait codegen
  - [ ] **Rust Tests**: `tests/drop_tests.rs`
  - [ ] Detect types implementing Drop trait
  - [ ] Generate destructor calls when refcount reaches zero
  - [ ] Destructor called before memory reclamation

- [ ] **Implement**: Destruction ordering
  - [ ] **Rust Tests**: `tests/destruction_order_tests.rs`
  - [ ] Reverse declaration order for local bindings
  - [ ] Reverse declaration order for struct fields
  - [ ] Back-to-front for list elements
  - [ ] Right-to-left for tuple elements

- [ ] **Implement**: Panic during destruction
  - [ ] **Rust Tests**: `tests/destructor_panic_tests.rs`
  - [ ] Single panic in destructor: propagate normally
  - [ ] Other destructors still run after panic
  - [ ] Double panic (destructor panics during unwind): abort

- [ ] **Implement**: Async destructor restriction
  - [ ] Compile error if Drop.drop declares `uses Async`
  - [ ] Error code and message for async destructor attempt

- [ ] **Implement**: FFI runtime support
  - [ ] C ABI compatibility
  - [ ] Callback support

---

## 21.6 Architecture (Completed Reorganization)

The LLVM backend follows Rust's `rustc_codegen_llvm` patterns:

### Context Hierarchy

```rust
// Simple context - just LLVM types
pub struct SimpleCx<'ll> {
    pub llcx: &'ll Context,
    pub llmod: Module<'ll>,
    pub ptr_type: PointerType<'ll>,
    pub isize_ty: IntType<'ll>,
}

// Full context - adds Ori-specific state
pub struct CodegenCx<'ll, 'tcx> {
    pub scx: SimpleCx<'ll>,
    pub interner: &'tcx StringInterner,
    pub instances: RefCell<HashMap<Name, FunctionValue<'ll>>>,
    pub type_cache: RefCell<TypeCache<'ll>>,
}
```

### Builder Pattern

```rust
pub struct Builder<'a, 'll, 'tcx> {
    llbuilder: &'a inkwell::builder::Builder<'ll>,
    cx: &'a CodegenCx<'ll, 'tcx>,
}
```

### Directory Structure

```
ori_llvm/src/
├── lib.rs              # Crate root, re-exports
├── context.rs          # SimpleCx, CodegenCx, TypeCache
├── builder.rs          # Builder + expression compilation
├── types.rs            # Type mapping helpers
├── declare.rs          # Function declaration
├── traits.rs           # BackendTypes, BuilderMethods traits
├── module.rs           # ModuleCompiler (two-phase codegen)
├── runtime.rs          # Runtime FFI functions
├── evaluator.rs        # JIT evaluator (OwnedLLVMEvaluator)
├── operators.rs        # Binary/unary operator codegen
├── control_flow.rs     # if/else, loops, break/continue
├── matching.rs         # Pattern matching codegen
├── collections/        # Collection codegen (tuples, structs, lists)
│   ├── mod.rs
│   ├── tuples.rs
│   ├── structs.rs
│   ├── lists.rs
│   └── option_result.rs
├── functions/          # Function codegen
│   ├── mod.rs
│   ├── body.rs         # Function body compilation
│   ├── calls.rs        # Function call codegen
│   ├── lambdas.rs      # Lambda/closure codegen
│   ├── builtins.rs     # Built-in function codegen
│   ├── sequences.rs    # FunctionSeq (run, try, match)
│   └── expressions.rs  # FunctionExp (recurse, print, panic)
└── tests/              # Unit tests (204 tests)
    ├── mod.rs
    ├── arithmetic_tests.rs
    ├── collection_tests.rs
    ├── control_flow_tests.rs
    ├── function_tests.rs
    ├── matching_tests.rs
    └── ...
```

---

## 21.7 Phase Completion Checklist

- [x] JIT compilation working
- [x] All Ori tests pass (734/734, 19 skipped)
- [x] All Rust unit tests pass (204/204)
- [x] Architecture follows Rust patterns
- [ ] AOT compilation (object files)
- [ ] Optimization passes
- [ ] Debug info generation
- [ ] 80+% test coverage (currently ~68%)

**Exit Criteria**: Native binaries run correctly via LLVM backend

---

## Running Tests

```bash
# Build Docker container (first time only)
./docker/llvm/build.sh

# Run all Ori tests via LLVM
./docker/llvm/run.sh ori test

# Run spec tests only
./docker/llvm/run.sh ori test tests/spec

# Run Rust unit tests
./docker/llvm/run.sh cargo test -p ori_llvm --lib

# Run with debug output
ORI_DEBUG_LLVM=1 ./docker/llvm/run.sh ori test tests/spec/types/primitives.ori
```
