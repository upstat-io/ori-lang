---
plan: "dpr_architecture_02212026"
title: "Design Pattern Review: LLVM Backend Architecture"
status: draft
---

# Design Pattern Review: LLVM Backend Architecture

## Ori Today

Ori's LLVM backend is organized around a clean pipeline: `TypeInfoStore` (lazy `Idx -> TypeInfo` cache backed by `Pool`) feeds `TypeLayoutResolver` (cycle-safe two-phase LLVM struct resolution) into `IrBuilder` (ID-based inkwell wrapper hiding `'ctx` lifetime behind `ValueId`/`BlockId`/`FunctionId` newtypes). `FunctionCompiler` orchestrates two-pass compilation: Phase 1 (`declare_all()`) walks all functions to compute `FunctionAbi` (Direct/Indirect/Sret/Void parameter passing, fastcc vs ccc calling convention) and declare LLVM functions; Phase 2 (`define_all()`) creates `ExprLowerer` per function, binds parameters to `Scope` (persistent `im::HashMap` with O(1) clone), and lowers the canonical IR body. This declare-then-define pattern enables forward references and prevents ABI mismatches during codegen. The `ExprLowerer` exhaustively matches all 40+ `CanExpr` variants, dispatching to focused `lower_*` submodules (literals, operators, control flow, collections, error handling, calls, lambdas). Two codegen tiers coexist: Tier 1 (`ExprLowerer` -> LLVM IR, no RC) and Tier 2 (`CanExpr` -> ARC IR -> `ArcIrEmitter` -> LLVM IR with `ori_rc_inc`/`ori_rc_dec`).

What works well: `TypeInfo`'s exhaustive enum dispatch (inspired by Swift's `TypeInfo` hierarchy) catches new types at compile time -- adding a type means adding one variant, not patching match arms across the codebase. The ABI separation (`ori_types::FunctionSig` = semantic, `FunctionAbi` = physical) follows Rust's `FnAbi` pattern and keeps all calling convention decisions in `codegen/abi/mod.rs`. The trampoline pattern in `lower_iterator_trampolines.rs` elegantly bridges `fastcc` Ori closures to `ccc` runtime callbacks: the `{ fn_ptr, env_ptr }` wrapper struct is allocated once, the trampoline unpacks it and calls through with proper convention, and the same pattern scales to map/filter/fold/for_each. Runtime declarations in `runtime_decl/mod.rs` with corresponding JIT symbol mappings in `evaluator.rs` keep the JIT/AOT boundary explicit. The incremental compilation infrastructure (`aot/incremental/`) with function-level hashing and dependency tracking is forward-looking.

What's strained: `FunctionCompiler::new()` takes 9 parameters including 3 optional borrows (`annotated_sigs`, `arc_classifier`, `debug_context`) -- this will grow as capabilities, async, and more ARC features land. `ExprLowerer::new()` takes 14 parameters and requires `pub(crate)` visibility on all its fields so the `lower_*` submodules can access them. Runtime function declarations are manually synchronized across three locations: `declare_runtime()` (LLVM declarations), `add_runtime_mappings_to_engine()` (JIT symbol table), and `AOT_ONLY_RUNTIME_FUNCTIONS`/`JIT_MAPPED_RUNTIME_FUNCTIONS` (test constants) -- adding a runtime function means updating all four and forgetting one silently breaks either JIT or AOT. Trampoline generation creates a new LLVM function per adapter call site (e.g., `_ori_tramp_map_0`, `_ori_tramp_map_1`) even when the type signature is identical, because deduplication would require a `(elem_type, result_type) -> FunctionId` cache that currently doesn't exist. The `abi_size_inner()` function sums field sizes without alignment padding, which is correct for current built-in types but will misclassify mixed-alignment user structs.

## Prior Art

### Rust (rustc_codegen_llvm) -- Query-Driven Builder with Cached Instances

Rust separates the LLVM builder wrapper (`Builder<'_, 'll, 'tcx>`) from the compilation context (`CodegenCx<'ll, 'tcx>`). The builder is deliberately "dumb" -- it wraps raw LLVM intrinsics (store, load, call) without semantic knowledge, while the query system drives what gets code-generated. `FnAbi` is computed by the middle-end (`rustc_target`) and then extended with LLVM-specific methods via the `FnAbiLlvmExt` trait (`llvm_type()`, `apply_attrs_llfn()`, `apply_attrs_callsite()`). This two-trait pattern means ABI computation happens once in a target-independent way, and LLVM-specific attribute application happens once more in a backend-specific way -- neither pollutes the other. Function declarations are cached in `cx.instances`, preventing duplicate codegen of monomorphizations. Attribute application uses a two-phase approach: ABI-affecting attributes (sret, byval, alignment) are always applied; optimization attributes (noinline, cold) are gated by `-C opt-level`. This prevents optimization attributes from masking ABI bugs during development.

### Swift (IRGen) -- Three-Tier Function Representation with Thunk Architecture

Swift maintains three function representations: thin pointer (bare function pointer), thick pointer (`{ fn_ptr, context_ptr }` pair with ref-counted context), and Objective-C block. Transitions between representations go through `IRGenThunk` (GenThunk.cpp), which collects parameters from one convention, forwards to another, and handles witness table/generic metadata passing. The key insight is that Swift's parameter ordering is fixed by convention -- Return -> Block context -> Formal args -> Generics -> Thick context -> Error out-param -> Witness metadata -- so adapting between thin and thick functions requires only appending/stripping the context parameter, not reshuffling everything. Capture analysis determines whether a closure needs heap allocation (escaping captures) or can stay on the stack (non-escaping). The `CallEmission` class in `Callee.h` encapsulates the entire process of adapting caller convention to callee convention, including self/witness table injection for protocol dispatch. This is directly relevant to Ori's trampoline pattern: instead of generating per-call-site trampolines, a thunk architecture generates per-signature thunks and caches them.

### Zig -- Multi-Backend IR Abstraction with Staged Job Pipeline

Zig uses a three-phase IR pipeline (AST -> ZIR -> AIR -> backend MIR) where each backend (`codegen/llvm.zig`, `codegen/x86_64/CodeGen.zig`) independently consumes AIR. The compilation state is passed as a large config struct with no global mutables; a Mutex protects only the work queue for parallel codegen. Per-arch ABI lookup tables (`x86_64/abi.zig`) are pure data -- no IR construction, no builder state -- making them trivially testable and reusable across backends. The `InstMap` type uses a linear array indexed by `Zir.Inst.Index - start` to avoid hashmap overhead in the hot instruction-lowering path. Zig's multi-backend design is more than Ori needs today, but two patterns transfer directly: (1) the config struct pattern for passing compilation state, replacing the 9+ parameter constructors; (2) the staged job pipeline for incremental codegen, where each function is an independent work item.

## Proposed Best-of-Breed Design

### Core Idea

The proposal strengthens Ori's existing architecture at three pressure points -- context threading, runtime declaration sync, and trampoline deduplication -- without replacing the patterns that already work (two-pass declare/define, `TypeInfo` dispatch, `ExprLowerer` submodule split, ABI separation). From Rust, we take the **extension trait pattern** for ABI application and a **cached function instance map** for trampoline deduplication. From Swift, we take the **config struct pattern** for codegen context threading and the **per-signature thunk cache** for iterator trampolines. From Zig, we take the **declarative runtime table** pattern to eliminate the manual 4-way sync between runtime declarations, JIT mappings, and test constants. The combined design keeps Ori's single-backend simplicity while solving the scaling problems that will compound as capabilities, async, and more iterator adapters land.

### Key Design Choices

1. **`CodegenConfig` struct to replace parameter explosion** (Zig: config struct, Rust: `CodegenCx`). `FunctionCompiler::new()` currently takes 9 parameters and `ExprLowerer::new()` takes 14. Group the shared context into a single struct that threads through the pipeline:

   ```rust
   /// Shared codegen context — created once per module compilation.
   /// All fields are immutable borrows; the struct is cheaply clonable.
   pub struct CodegenConfig<'a, 'scx, 'ctx, 'tcx> {
       pub type_info: &'a TypeInfoStore<'tcx>,
       pub type_resolver: &'a TypeLayoutResolver<'a, 'scx, 'ctx>,
       pub interner: &'a StringInterner,
       pub pool: &'tcx Pool,
       pub module_path: &'a str,
       pub debug_context: Option<&'a DebugContext<'ctx>>,
       // ARC fields — None for Tier 1, Some for Tier 2
       pub annotated_sigs: Option<&'a FxHashMap<Name, AnnotatedSig>>,
       pub arc_classifier: Option<&'a ArcClassifier<'tcx>>,
   }
   ```

   `FunctionCompiler::new()` becomes `new(builder, config)`. `ExprLowerer::new()` becomes `new(builder, config, scope, canon, func_id, fn_maps, lambda_counter)` -- still several parameters, but the shared context is bundled. When capabilities or async metadata need to flow through codegen, they go into `CodegenConfig` without changing any constructor signature.

2. **Declarative runtime function registry** (Zig: compile-time tables, Ori's own registration sync pattern). Replace the manual 4-way sync (`declare_runtime()` + `add_runtime_mappings_to_engine()` + `AOT_ONLY_RUNTIME_FUNCTIONS` + `JIT_MAPPED_RUNTIME_FUNCTIONS`) with a single declarative table:

   ```rust
   /// Single source of truth for all runtime functions.
   struct RuntimeFn {
       name: &'static str,
       params: &'static [ParamKind],       // I64, F64, Bool, Ptr, I8, I32
       ret: Option<RetKind>,               // None = void
       jit_addr: Option<fn() -> usize>,    // None = AOT-only
       attrs: &'static [FnAttr],           // Cold, Nounwind, NoaliasReturn, etc.
   }

   /// The complete runtime function table — adding a function means
   /// adding ONE entry here.
   const RUNTIME_FUNCTIONS: &[RuntimeFn] = &[
       RuntimeFn {
           name: "ori_print",
           params: &[ParamKind::Ptr],
           ret: None,
           jit_addr: Some(|| runtime::ori_print as usize),
           attrs: &[],
       },
       RuntimeFn {
           name: "ori_panic",
           params: &[ParamKind::Ptr],
           ret: None,
           jit_addr: Some(|| runtime::ori_panic as usize),
           attrs: &[FnAttr::Cold],
       },
       // Iterator functions: AOT-only (jit_addr = None)
       RuntimeFn {
           name: "ori_iter_from_list",
           params: &[ParamKind::Ptr, ParamKind::I64, ParamKind::I64],
           ret: Some(RetKind::Ptr),
           jit_addr: None,
           attrs: &[],
       },
       // ...
   ];

   /// Generated from RUNTIME_FUNCTIONS — no manual sync needed.
   pub fn declare_runtime(builder: &mut IrBuilder<'_, '_>) {
       for entry in RUNTIME_FUNCTIONS {
           let param_types = entry.params.iter()
               .map(|p| p.to_llvm_type(builder))
               .collect::<Vec<_>>();
           let ret_type = entry.ret.map(|r| r.to_llvm_type(builder));
           let fn_id = builder.declare_extern_function(
               entry.name, &param_types, ret_type,
           );
           for attr in entry.attrs {
               attr.apply(builder, fn_id);
           }
       }
   }

   /// Generated from RUNTIME_FUNCTIONS — guaranteed in sync.
   pub fn add_runtime_mappings(engine: &ExecutionEngine<'_>, module: &Module<'_>) {
       for entry in RUNTIME_FUNCTIONS {
           if let Some(addr_fn) = entry.jit_addr {
               if let Some(func) = module.get_function(entry.name) {
                   engine.add_global_mapping(&func, (addr_fn)());
               }
           }
       }
   }
   ```

   The test constants (`AOT_ONLY_RUNTIME_FUNCTIONS`, `JIT_MAPPED_RUNTIME_FUNCTIONS`) are derived by filtering `RUNTIME_FUNCTIONS` on `jit_addr.is_none()` / `jit_addr.is_some()`. Adding a new runtime function is exactly one table entry -- impossible to forget a sync point.

3. **Per-signature trampoline cache** (Swift: per-signature thunks, Rust: `cx.instances` caching). Currently `generate_map_trampoline()` creates a new LLVM function every time, even if the same `(T -> U)` signature was already trampolined. Add a cache keyed by `(TrampolineKind, Vec<Idx>)`:

   ```rust
   /// Trampoline cache: (kind, type_args) -> FunctionId.
   /// Shared across all ExprLowerer instances in a module via &RefCell.
   pub struct TrampolineCache {
       cache: FxHashMap<TrampolineKey, FunctionId>,
   }

   #[derive(Clone, Hash, Eq, PartialEq)]
   struct TrampolineKey {
       kind: TrampolineKind,
       type_args: Vec<Idx>,
   }

   #[derive(Clone, Copy, Hash, Eq, PartialEq)]
   enum TrampolineKind {
       Map,      // (T -> U): wrapper, in_ptr, out_ptr
       Filter,   // (T -> bool): wrapper, elem_ptr
       ForEach,  // (T -> void): wrapper, elem_ptr
       Fold,     // (Acc, T -> Acc): wrapper, acc_ptr, elem_ptr, out_ptr
   }
   ```

   When `generate_map_trampoline(closure, int, str)` is called twice, the second call returns the cached `FunctionId`. This mirrors Swift's thunk caching and Rust's `cx.instances` map. The cache is module-scoped (shared via `&RefCell<TrampolineCache>` on `FunctionCompiler`, passed to `ExprLowerer`).

4. **ABI extension trait for LLVM-specific attribute application** (Rust: `FnAbiLlvmExt`). Currently `declare_function_llvm()` mixes ABI computation with LLVM attribute application. Separate these into an extension trait:

   ```rust
   /// Extension trait: LLVM-specific methods on FunctionAbi.
   /// Keeps abi/mod.rs pure (no LLVM dependency) while providing
   /// LLVM-specific operations in codegen context.
   pub(crate) trait FunctionAbiExt {
       /// Build the LLVM parameter type list from ABI descriptors.
       fn llvm_param_types(
           &self,
           resolver: &TypeLayoutResolver<'_, '_, '_>,
           builder: &IrBuilder<'_, '_>,
       ) -> Vec<LLVMTypeId>;

       /// Apply ABI-affecting attributes (sret, noalias, alignment)
       /// to a declared function.
       fn apply_abi_attrs(&self, builder: &mut IrBuilder<'_, '_>, func_id: FunctionId);

       /// Apply optimization attributes (cold, noinline) gated by
       /// optimization level.
       fn apply_opt_attrs(
           &self,
           builder: &mut IrBuilder<'_, '_>,
           func_id: FunctionId,
           opt_level: OptLevel,
       );
   }
   ```

   This keeps `abi/mod.rs` free of LLVM types (pure semantic ABI computation) while providing LLVM-specific extension methods used by `FunctionCompiler`. The split follows Rust's `FnAbi` (in `rustc_target`) + `FnAbiLlvmExt` (in `rustc_codegen_llvm`) pattern exactly.

5. **Alignment-aware ABI size computation** (all three compilers: query LLVM TargetData). Replace the current `abi_size_inner()` sum-of-fields approximation with layout queries that respect alignment:

   ```rust
   /// Compute ABI size using LLVM's layout engine when available.
   /// Falls back to sum-of-fields for types not yet registered.
   pub fn abi_size_with_layout(
       ty: Idx,
       store: &TypeInfoStore<'_>,
       resolver: &TypeLayoutResolver<'_, '_, '_>,
   ) -> u64 {
       // For primitives and built-in compounds with known sizes,
       // use the existing TypeInfo::size() fast path.
       let info = store.get(ty);
       if let Some(size) = info.size() {
           return size;
       }
       // For user-defined types, resolve to LLVM type and query
       // the actual store size including alignment padding.
       let llvm_ty = resolver.resolve(ty);
       TypeLayoutResolver::type_store_size(llvm_ty)
   }
   ```

   `TypeLayoutResolver::type_store_size()` already exists and computes sizes from LLVM struct layout. The change is to use it in `compute_param_passing()` / `compute_return_passing()` when `TypeInfo::size()` returns `None`, instead of the recursive sum approximation. This prevents the misclassification bug noted in the FIXME comment in `abi/mod.rs`.

6. **Dual-mode iterator strategy** (Ori-specific: no reference compiler does this). Ori's JIT uses native `IteratorValue` (tree-walking interpreter), while AOT uses opaque `IterState` handles with trampoline callbacks. This split is currently implicit (iterator runtime functions are in `AOT_ONLY_RUNTIME_FUNCTIONS`). Make it explicit with a strategy trait:

   ```rust
   /// Iterator lowering strategy — selected per compilation mode.
   pub(crate) enum IterStrategy {
       /// JIT: iterators are native IteratorValue from ori_patterns.
       /// No trampoline needed — closures called directly.
       Native,
       /// AOT: iterators are opaque handles from ori_rt.
       /// Closures bridged via trampolines to C ABI.
       Opaque { trampoline_cache: Rc<RefCell<TrampolineCache>> },
   }
   ```

   `ExprLowerer` checks `iter_strategy` when lowering `.map()`, `.filter()`, etc. This makes the JIT/AOT split explicit at the architecture level rather than relying on which runtime functions happen to be mapped. When Ori eventually unifies the iterator representations (e.g., compiling iterator combinators to inline loops like Rust's `Iterator` trait), the strategy enum is the natural extension point.

### What Makes Ori's Approach Unique

Ori's dual execution model (JIT for tests, AOT for binaries) creates an architectural constraint that none of the reference compilers face. Rust, Swift, and Zig each have one codegen path; Ori has two that must produce identical semantics from the same type-checked IR. The `FunctionCompiler` two-pass pattern handles this cleanly because both paths share the same `FunctionAbi` computation and `TypeInfoStore` -- only the body emission differs (Tier 1 `ExprLowerer` vs Tier 2 `ArcIrEmitter`). This is a genuine advantage: the ABI layer is tested by both paths, making calling convention bugs surface faster.

Ori's expression-based semantics (no `return` keyword, block value = last expression) simplify codegen compared to Rust or Swift. Every expression produces a `ValueId` or `None`, and the function return is always `emit_return(result_of_last_expr)`. This eliminates an entire class of control flow complexity (early return, multiple return paths, return-value-in-register vs return-value-on-stack decisions at each return site). Combined with ARC memory management (no GC pauses, no borrow checker constraints on codegen), Ori can generate simpler IR than any of the reference compilers for equivalent programs.

The mandatory-tests constraint means the JIT path runs orders of magnitude more often than the AOT path during development. This inverts the usual priority: most compilers optimize for AOT speed and treat JIT as a debugging aid. For Ori, JIT compilation speed directly impacts developer experience (every `ori check` runs tests). This motivates the trampoline cache and runtime registry improvements -- both reduce per-module JIT overhead that compounds across hundreds of test files.

Capability-based effects will eventually need codegen support (likely as additional hidden parameters or a capability context pointer, similar to Swift's witness table passing). The `CodegenConfig` struct provides a natural home for capability resolution state, and the `FunctionAbi` extension trait provides a place to inject capability parameters into the LLVM calling convention without modifying the semantic ABI.

### Concrete Types & Interfaces

```rust
// ── codegen/config.rs ────────────────────────────────────────────────

/// Shared codegen context. Created once per module, threaded by reference.
pub struct CodegenConfig<'a, 'scx, 'ctx, 'tcx> {
    pub type_info: &'a TypeInfoStore<'tcx>,
    pub type_resolver: &'a TypeLayoutResolver<'a, 'scx, 'ctx>,
    pub interner: &'a StringInterner,
    pub pool: &'tcx Pool,
    pub module_path: &'a str,
    pub debug_context: Option<&'a DebugContext<'ctx>>,
    pub annotated_sigs: Option<&'a FxHashMap<Name, AnnotatedSig>>,
    pub arc_classifier: Option<&'a ArcClassifier<'tcx>>,
}

// ── codegen/runtime_decl/registry.rs ─────────────────────────────────

/// Parameter kind for runtime function declarations.
#[derive(Clone, Copy)]
pub enum ParamKind { I8, I32, I64, F64, Bool, Ptr }

/// Return kind for runtime function declarations.
#[derive(Clone, Copy)]
pub enum RetKind { I8, I32, I64, F64, Bool, Ptr, Str, List, CharResult }

/// Function attribute for runtime declarations.
#[derive(Clone, Copy)]
pub enum FnAttr { Cold, Nounwind, NoaliasReturn, MemArgmemReadwrite }

/// Runtime function descriptor — single source of truth.
pub struct RuntimeFn {
    pub name: &'static str,
    pub params: &'static [ParamKind],
    pub ret: Option<RetKind>,
    pub jit_addr: Option<fn() -> usize>,
    pub attrs: &'static [FnAttr],
}

impl ParamKind {
    pub fn to_llvm_type(self, builder: &IrBuilder<'_, '_>) -> LLVMTypeId {
        match self {
            Self::I8 => builder.i8_type(),
            Self::I32 => builder.i32_type(),
            Self::I64 => builder.i64_type(),
            Self::F64 => builder.f64_type(),
            Self::Bool => builder.bool_type(),
            Self::Ptr => builder.ptr_type(),
        }
    }
}

// ── codegen/trampoline_cache.rs ──────────────────────────────────────

use std::cell::RefCell;
use std::rc::Rc;

/// Cache key: trampoline kind + type arguments.
#[derive(Clone, Hash, Eq, PartialEq)]
struct TrampolineKey {
    kind: TrampolineKind,
    type_args: Vec<Idx>,
}

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
enum TrampolineKind { Map, Filter, ForEach, Fold }

/// Per-module trampoline cache. Shared via Rc<RefCell<>> across
/// all ExprLowerer instances in a FunctionCompiler session.
pub struct TrampolineCache {
    entries: FxHashMap<TrampolineKey, FunctionId>,
}

impl TrampolineCache {
    pub fn new() -> Self {
        Self { entries: FxHashMap::default() }
    }

    /// Look up a cached trampoline, or return None to signal creation.
    pub fn get(&self, kind: TrampolineKind, type_args: &[Idx]) -> Option<FunctionId> {
        let key = TrampolineKey {
            kind,
            type_args: type_args.to_vec(),
        };
        self.entries.get(&key).copied()
    }

    /// Register a newly created trampoline.
    pub fn insert(&mut self, kind: TrampolineKind, type_args: Vec<Idx>, func_id: FunctionId) {
        let key = TrampolineKey { kind, type_args };
        self.entries.insert(key, func_id);
    }
}

// ── codegen/abi/ext.rs ───────────────────────────────────────────────

/// LLVM-specific extension methods for FunctionAbi.
/// Keeps abi/mod.rs free of LLVM types.
pub(crate) trait FunctionAbiExt {
    /// Build LLVM parameter type list including sret pointer.
    fn llvm_param_types(
        &self,
        resolver: &TypeLayoutResolver<'_, '_, '_>,
        builder: &IrBuilder<'_, '_>,
    ) -> Vec<LLVMTypeId>;

    /// Apply sret/noalias/alignment attributes to a declared function.
    fn apply_abi_attrs(
        &self,
        builder: &mut IrBuilder<'_, '_>,
        func_id: FunctionId,
    );
}

impl FunctionAbiExt for FunctionAbi {
    fn llvm_param_types(
        &self,
        resolver: &TypeLayoutResolver<'_, '_, '_>,
        builder: &IrBuilder<'_, '_>,
    ) -> Vec<LLVMTypeId> {
        let mut types = Vec::with_capacity(self.params.len() + 1);
        if matches!(self.return_abi.passing, ReturnPassing::Sret { .. }) {
            types.push(builder.ptr_type());
        }
        for param in &self.params {
            match &param.passing {
                ParamPassing::Direct => {
                    let ty = resolver.resolve(param.ty);
                    types.push(builder.register_type(ty));
                }
                ParamPassing::Indirect { .. } | ParamPassing::Reference => {
                    types.push(builder.ptr_type());
                }
                ParamPassing::Void => {}
            }
        }
        types
    }

    fn apply_abi_attrs(
        &self,
        builder: &mut IrBuilder<'_, '_>,
        func_id: FunctionId,
    ) {
        match self.call_conv {
            CallConv::Fast => builder.set_fastcc(func_id),
            CallConv::C => builder.set_ccc(func_id),
        }
        if let ReturnPassing::Sret { .. } = &self.return_abi.passing {
            let ret_ty = resolver.resolve(self.return_abi.ty);
            let ret_ty_id = builder.register_type(ret_ty);
            builder.add_sret_attribute(func_id, 0, ret_ty_id);
            builder.add_noalias_attribute(func_id, 0);
        }
    }
}
```

## Implementation Roadmap

### Phase 1: Foundation (Non-Breaking Refactors)

- [ ] **Extract `CodegenConfig` struct** -- Create `codegen/config.rs` with the shared context struct. Update `FunctionCompiler::new()` and `ExprLowerer::new()` to accept `&CodegenConfig` instead of individual borrows. This is a pure refactor -- no behavior change, all existing tests pass unchanged.
- [ ] **Build declarative runtime registry** -- Create `codegen/runtime_decl/registry.rs` with the `RuntimeFn` table. Generate `declare_runtime()` and `add_runtime_mappings()` from the table. Derive the test constants from the table. Delete the manual sync code. Add a compile-time assertion that every `RUNTIME_FUNCTIONS` entry with `jit_addr: Some(...)` has a corresponding runtime function, and vice versa.
- [ ] **Extract `FunctionAbiExt` trait** -- Create `codegen/abi/ext.rs` implementing LLVM-specific methods. Refactor `declare_function_llvm()` to use the extension trait. The `abi/mod.rs` file stays pure (no LLVM imports).

### Phase 2: Core Improvements

- [ ] **Implement trampoline cache** -- Create `codegen/trampoline_cache.rs`. Thread `Rc<RefCell<TrampolineCache>>` through `FunctionCompiler` to `ExprLowerer`. Modify `generate_map_trampoline()`, `generate_filter_trampoline()`, `generate_for_each_trampoline()`, and `generate_fold_trampoline()` to check the cache before creating a new LLVM function. Add a test that verifies two `.map(f)` calls with the same type signature reuse the same trampoline function.
- [ ] **Fix alignment-aware ABI size** -- Update `compute_param_passing()` and `compute_return_passing()` to use `TypeLayoutResolver::type_store_size()` for types where `TypeInfo::size()` returns `None`. Add test cases for mixed-alignment user structs (e.g., `struct Mixed { a: byte, b: int, c: byte }` should be Indirect at 24 bytes with padding, not Direct at 10 bytes without).
- [ ] **Add iterator strategy enum** -- Create `IterStrategy` in `codegen/config.rs`. Thread through `ExprLowerer`. For now, both variants produce the same code -- the enum is the extension point. Add `IterStrategy` selection logic to `OwnedLLVMEvaluator` (Native) and the AOT pipeline (Opaque).

### Phase 3: Polish & Future-Proofing

- [ ] **Capability context parameter slot** -- Reserve space in `FunctionAbi` for a future capability context parameter. Define the `CapabilityCtx` type layout (opaque `ptr` initially). When capability codegen lands, the ABI infrastructure is ready without a new refactor cycle.
- [ ] **Incremental function-level codegen** -- Use `FunctionDependencyGraph` (already in `aot/incremental/function_deps/`) to skip re-lowering unchanged function bodies in AOT. Requires function-level ABI caching (the `FunctionAbi` for an unchanged function can be reused without recomputation). The two-pass declare/define pattern enables this naturally: Phase 1 declares all functions (cheap), Phase 2 only defines changed functions.
- [ ] **Cross-phase runtime sync test** -- Add a test in `ori_llvm/tests/` that iterates `RUNTIME_FUNCTIONS`, checks each entry against the actual `ori_rt` symbols (using `dlsym` or compile-time `extern "C"` declarations), and verifies parameter counts match. This catches ABI drift between the codegen declarations and the runtime implementations.

## References

### Rust (rustc_codegen_llvm)
- `compiler/rustc_codegen_llvm/src/callee.rs` -- `get_fn()` cached function lookup
- `compiler/rustc_codegen_llvm/src/abi.rs` -- `FnAbiLlvmExt` trait, `llvm_type()`, `apply_attrs_llfn()`
- `compiler/rustc_codegen_llvm/src/builder.rs` -- `Builder` thin wrapper, store/load/call
- `compiler/rustc_target/src/abi/call/mod.rs` -- `FnAbi` (target-independent)

### Swift (IRGen)
- `lib/IRGen/GenFunc.cpp` -- `FuncSignatureInfo`, context parameter slots
- `lib/IRGen/GenThunk.cpp` -- `IRGenThunk::prepareArguments()`, `emit()` forwarding
- `lib/IRGen/Callee.h` -- `CallEmission` class, thick/thin convention bridging
- `lib/IRGen/GenCall.cpp` -- Parameter ordering discipline, convention adaptation

### Zig
- `src/codegen.zig` -- `generateFunction()` backend dispatch
- `src/codegen/llvm.zig` -- LLVM builder wrapper consuming AIR
- `src/Compilation.zig` -- Config struct pattern, Mutex work queue
- `src/link.zig` -- Staged job pipeline (AstGen -> Sema -> Codegen -> Link)

### Ori (current codebase)
- `compiler/ori_llvm/src/codegen/function_compiler/mod.rs` -- Two-pass declare/define
- `compiler/ori_llvm/src/codegen/abi/mod.rs` -- `FunctionAbi`, `compute_function_abi()`
- `compiler/ori_llvm/src/codegen/type_info/mod.rs` -- `TypeInfo` enum, `TypeInfoStore`, `TypeLayoutResolver`
- `compiler/ori_llvm/src/codegen/expr_lowerer.rs` -- `ExprLowerer` dispatch
- `compiler/ori_llvm/src/codegen/lower_iterator_trampolines.rs` -- Trampoline generation
- `compiler/ori_llvm/src/codegen/runtime_decl/mod.rs` -- Runtime declarations
- `compiler/ori_llvm/src/evaluator.rs` -- JIT pipeline, runtime mappings
- `compiler/ori_llvm/src/codegen/arc_emitter.rs` -- Tier 2 ARC codegen
- `compiler/ori_llvm/src/codegen/scope/mod.rs` -- Persistent scope
- `compiler/ori_llvm/src/context/mod.rs` -- `SimpleCx` LLVM context
- `compiler/ori_rt/src/iterator/mod.rs` -- Runtime iterator state machine
- `compiler/ori_llvm/src/aot/incremental/mod.rs` -- Incremental compilation infrastructure
