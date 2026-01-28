# Ori LLVM Backend Architecture

**Status:** ✅ Reorganization Complete (2026-01-27)

Based on analysis of Rust's `rustc_codegen_llvm`, the `ori_llvm` crate follows industry-standard patterns.

## Architecture Overview

### 1. Context Hierarchy

Rust-style hierarchical context pattern:

```rust
// Simple context - just LLVM types (context.rs)
pub struct SimpleCx<'ll> {
    pub llcx: &'ll Context,
    pub llmod: Module<'ll>,
    pub ptr_type: PointerType<'ll>,
    pub isize_ty: IntType<'ll>,
}

// Full context - adds Ori-specific state (context.rs)
pub struct CodegenCx<'ll, 'tcx> {
    pub scx: SimpleCx<'ll>,
    pub interner: &'tcx StringInterner,
    pub instances: RefCell<HashMap<Name, FunctionValue<'ll>>>,
    pub tests: RefCell<HashMap<Name, FunctionValue<'ll>>>,
    pub type_cache: RefCell<TypeCache<'ll>>,
}
```

### 2. Separate Builder Type

Builder separates instruction building from context:

```rust
// builder.rs
pub struct Builder<'a, 'll, 'tcx> {
    llbuilder: &'a inkwell::builder::Builder<'ll>,
    cx: &'a CodegenCx<'ll, 'tcx>,
}
```

**Benefits:**
- Builder has a clear scope (single basic block)
- Methods on builder (add, sub, call, etc.) don't pollute context
- Auto-cleanup via Drop

### 3. Two-Phase Codegen

```rust
// module.rs - ModuleCompiler
// Phase 1: Declare all symbols
let func = cx.declare_fn(name, &param_types, return_type);

// Phase 2: Define function bodies
let builder = Builder::build(&cx, entry_bb);
builder.compile_function_body(...);
```

**Enables:**
- Forward references (A calls B before B is defined)
- Consistent ABI before bodies exist

### 4. Type Caching

Two-level cache in `TypeCache`:

```rust
pub struct TypeCache<'ll> {
    scalars: HashMap<TypeId, BasicTypeEnum<'ll>>,
    complex: HashMap<TypeId, BasicTypeEnum<'ll>>,
}
```

### 5. Trait-Based Abstraction

For future backend extensibility:

```rust
// traits.rs
pub trait BackendTypes {
    type Value;
    type Type;
    type Function;
    type BasicBlock;
}

pub trait CodegenMethods<'tcx>: BackendTypes {
    fn default_value(&self, type_id: TypeId) -> Self::Value;
    fn llvm_type(&self, type_id: TypeId) -> Self::Type;
    // ...
}

pub trait BuilderMethods<'a>: BackendTypes {
    fn build(cx: &'a Self::CodegenCx, bb: Self::BasicBlock) -> Self;
    fn ret(&self, val: Self::Value);
    fn add(&self, a: Self::Value, b: Self::Value) -> Self::Value;
    // ...
}
```

## Directory Structure

```
ori_llvm/src/
├── lib.rs              # Crate root, re-exports
├── context.rs          # SimpleCx, CodegenCx, TypeCache
├── builder.rs          # Builder + expression compilation
├── types.rs            # Type mapping helpers
├── declare.rs          # Function declaration
├── traits.rs           # BackendTypes, BuilderMethods, CodegenMethods
├── module.rs           # ModuleCompiler (two-phase codegen)
├── runtime.rs          # Runtime FFI functions
├── evaluator.rs        # JIT evaluator (LLVMEvaluator, OwnedLLVMEvaluator)
├── operators.rs        # Binary/unary operator codegen
├── control_flow.rs     # if/else, loops, break/continue
├── matching.rs         # Pattern matching codegen
├── collections/        # Collection codegen
│   ├── mod.rs
│   ├── tuples.rs       # Tuple construction and access
│   ├── structs.rs      # Struct construction and access
│   ├── lists.rs        # List operations
│   └── option_result.rs # Option/Result construction
├── functions/          # Function codegen
│   ├── mod.rs
│   ├── body.rs         # Function body compilation
│   ├── calls.rs        # Function call codegen
│   ├── lambdas.rs      # Lambda/closure codegen
│   ├── builtins.rs     # Built-in function codegen (len, is_some, etc.)
│   ├── sequences.rs    # FunctionSeq (run, try, match)
│   └── expressions.rs  # FunctionExp (recurse, print, panic)
└── tests/              # Unit tests (204 tests)
    ├── mod.rs
    ├── helper.rs       # Test utilities
    ├── arithmetic_tests.rs
    ├── collection_tests.rs
    ├── control_flow_tests.rs
    ├── advanced_control_flow_tests.rs
    ├── function_tests.rs
    ├── function_call_tests.rs
    ├── function_exp_tests.rs
    ├── function_seq_tests.rs
    ├── matching_tests.rs
    ├── more_control_flow_tests.rs
    ├── operator_tests.rs
    ├── runtime_tests.rs
    ├── string_tests.rs
    ├── type_conversion_tests.rs
    ├── evaluator_tests.rs
    └── builtins_tests.rs
```

## Type Mappings

| Ori Type | LLVM Type |
|----------|-----------|
| `int` | `i64` |
| `float` | `f64` |
| `bool` | `i1` |
| `char` | `i32` (Unicode codepoint) |
| `byte` | `i8` |
| `str` | `{ i64 len, ptr data }` |
| `Option<T>` | `{ i8 tag, T payload }` |
| `Result<T, E>` | `{ i8 tag, T payload }` |
| `[T]` (list) | `{ i64 len, i64 cap, ptr data }` |
| `(A, B)` (tuple) | `{ A, B }` |
| `void` | `void` |
| Unknown/TypeVar | `i64` (fallback) |

## Runtime Functions

All runtime functions are declared in `runtime.rs` and mapped during JIT:

| Function | Purpose |
|----------|---------|
| `ori_print` | Print string |
| `ori_print_int` | Print integer |
| `ori_print_float` | Print float |
| `ori_print_bool` | Print boolean |
| `ori_panic` | Panic with string |
| `ori_panic_cstr` | Panic with C string |
| `ori_assert` | Assert condition |
| `ori_assert_eq_int` | Assert int equality |
| `ori_assert_eq_bool` | Assert bool equality |
| `ori_assert_eq_str` | Assert string equality |
| `ori_str_concat` | Concatenate strings |
| `ori_str_eq` | String equality |
| `ori_str_ne` | String inequality |
| `ori_str_from_int` | Convert int to string |
| `ori_str_from_bool` | Convert bool to string |
| `ori_str_from_float` | Convert float to string |
| `ori_list_new` | Create new list |
| `ori_list_free` | Free list |
| `ori_list_len` | Get list length |
| `ori_compare_int` | Compare integers |
| `ori_min_int` | Minimum of integers |
| `ori_max_int` | Maximum of integers |

## Test Results (2026-01-27)

| Test Suite | Passed | Failed | Skipped |
|------------|--------|--------|---------|
| All Ori tests | 711 | 0 | 34 |
| Spec tests | 416 | 0 | 5 |
| Rust unit tests | 204 | 0 | 0 |

## Running Tests

```bash
# Build Docker container (required for LLVM)
./docker/llvm/build.sh

# Run all Ori tests via LLVM
./docker/llvm/run.sh ori test

# Run Rust unit tests
./docker/llvm/run.sh cargo test -p ori_llvm --lib

# Run with debug IR output
ORI_DEBUG_LLVM=1 ./docker/llvm/run.sh ori test tests/spec/types/primitives.ori
```

## Future Work

- [ ] AOT compilation (object file generation)
- [ ] Optimization passes (O1, O2, O3)
- [ ] Debug info (DWARF)
- [ ] Memory management (ARC)
- [ ] Local variables via alloca (currently HashMap)
