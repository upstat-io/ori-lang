---
section: "11"
title: LLVM Optimization Pass Configuration
status: not-started
goal: Configurable optimization pass pipeline with sensible defaults for debug and release builds, ARC-safe LLVM attributes, and module verification on all paths
sections:
  - id: "11.1"
    title: Existing Infrastructure (Preserve)
    status: not-started
  - id: "11.2"
    title: V2 Changes
    status: not-started
  - id: "11.3"
    title: ARC Safety & LLVM Attributes
    status: not-started
  - id: "11.4"
    title: Per-Function Attributes
    status: not-started
  - id: "11.5"
    title: LTO Pipeline
    status: not-started
  - id: "11.6"
    title: ModuleEmitter Integration
    status: not-started
---

# Section 11: LLVM Optimization Pass Configuration

**Status:** Not Started
**Goal:** Configurable LLVM optimization pass pipeline with profile presets, ARC-safe runtime function attributes, module verification on all codegen paths, and integration with the V2 `ModuleEmitter`.

**Reference compilers:**
- **Rust** `compiler/rustc_codegen_ssa/src/back/write.rs` -- `ModuleConfig` with pass selection, verification, and LTO pipeline staging
- **Zig** `src/codegen/llvm.zig` -- `opt_level` mapping, profile presets (Debug, ReleaseSafe, ReleaseFast, ReleaseSmall)
- **Swift** `lib/IRGen/IRGenModule.cpp` -- ARC runtime function attributes; all ARC optimization at SIL level, LLVM sees opaque calls

**Current state:** `ori_llvm/src/aot/passes.rs` (676 lines) is well-structured and provides the core pass pipeline. V2 preserves this infrastructure and integrates it with the new pipeline architecture.

---

## 11.1 Existing Infrastructure (Preserve)

The `aot/passes.rs` module already provides a solid foundation. V2 preserves all of the following:

**LLVM New Pass Manager via `LLVMRunPasses`:** Pipeline strings (`default<O0>` through `default<Oz>`) passed to `LLVMRunPasses` from `llvm-sys`. This is the correct modern approach (NPM default since LLVM 14).

**`OptimizationConfig` builder pattern:** Granular overrides for loop vectorization, SLP vectorization, loop unrolling, loop interleaving, function merging, inliner threshold, verify-each, and debug logging. All options use `Option<bool>` with level-based defaults.

**`PassBuilderOptionsGuard` RAII cleanup:** Wraps `LLVMPassBuilderOptionsRef` with `Drop` to ensure proper disposal. This pattern is correct and should not change.

**`OptimizationLevel` enum:** All six levels: O0, O1, O2, O3, Os, Oz. Each has a `pipeline_string()` method and query methods for level-specific defaults.

**LTO support (Thin/Full):** `LtoMode` enum with correct pipeline strings: `thinlto-pre-link<OX>` / `thinlto<OX>` for Thin, `lto-pre-link<OX>` / `lto<OX>` for Full. The `pipeline_string()` method on `OptimizationConfig` correctly dispatches to the right pipeline based on LTO mode and phase.

**Custom pipeline support:** `run_custom_pipeline()` accepts arbitrary LLVM pipeline strings for advanced use cases.

**`OptimizationError` enum:** Three variants: `PassBuilderOptionsCreationFailed`, `PassesFailed`, `InvalidPipeline`. Proper `Display` and `Error` impls.

- [ ] Verify all existing `passes.rs` functionality works after V2 module restructuring
- [ ] Ensure `PassBuilderOptionsGuard` Drop impl is preserved in new location

---

## 11.2 V2 Changes

### Profile Presets

V2 renames the convenience constructors to follow Zig-style profile naming (matching the plan's convention). Five presets, each mapping to a fixed `OptimizationLevel`:

| Profile | LLVM Level | Constructor | CLI `--opt=` | Description |
|---------|-----------|-------------|-------------|-------------|
| Debug | O0 | `OptimizationConfig::debug()` | `0` | No optimization. Fastest compilation, best for debugging. |
| Release | O2 | `OptimizationConfig::release()` | `2` | Standard optimization. Production default. |
| ReleaseFast | O3 | `OptimizationConfig::release_fast()` | `3` | Aggressive optimization. Maximum performance. |
| ReleaseSmall | Os | `OptimizationConfig::release_small()` | `s` | Size optimization. Prefers smaller code over faster code. |
| ReleaseMinSize | Oz | `OptimizationConfig::release_min_size()` | `z` | Aggressive size optimization. Smallest possible code. |

The `--release` flag maps to the Release profile (O2). Users can override with `--opt=3` for ReleaseFast or `--opt=s` for ReleaseSmall. O1 remains available via `--opt=1` but has no named profile.

**Note:** `--emit=llvm-ir` already exists in the current AOT pipeline (`ObjectEmitter::emit_llvm_ir`). V2 does not re-add this.

### Module Verification Before Optimization

**Existing bug:** The JIT path (`evaluator.rs`) calls `module.verify()` before JIT compilation. The AOT path (`commands/build.rs`, `commands/run.rs`) skips verification and calls `run_optimization_passes` directly. Invalid IR in the AOT path causes LLVM to segfault during optimization or object emission instead of producing a diagnostic.

**V2 fix:** Module verification runs unconditionally before optimization on all codegen paths. This is not opt-in, not gated behind debug builds. Verification is cheap relative to optimization and object emission. Invalid IR is a compiler bug that must be caught early.

```rust
/// Run the optimization pipeline with mandatory pre-verification.
///
/// Pipeline: verify_module → run_optimization_passes → emit
/// Verification is unconditional — catching invalid IR early prevents
/// LLVM segfaults during optimization or object emission.
pub fn optimize_module(
    module: &Module<'_>,
    target_machine: &TargetMachine,
    config: &OptimizationConfig,
) -> Result<(), CodegenError> {
    // Step 1: Verify module (unconditional)
    if let Err(msg) = module.verify() {
        return Err(CodegenError::VerificationFailed {
            message: msg.to_string(),
        });
    }

    // Step 2: Run optimization passes
    run_optimization_passes(module, target_machine, config)?;

    Ok(())
}
```

- [ ] Rename convenience constructors: `aggressive()` -> `release_fast()`, `size()` -> `release_small()`, `min_size()` -> `release_min_size()`
- [ ] Add `optimize_module()` wrapper that verifies before optimizing
- [ ] Replace all direct `run_optimization_passes` calls in `commands/build.rs` and `commands/run.rs` with `optimize_module`
- [ ] Verify that `module.verify()` catches malformed IR generated by the codegen

---

## 11.3 ARC Safety & LLVM Attributes (CRITICAL)

**Design decision:** No custom LLVM passes for ARC. All ARC optimization happens in `ori_arc` (borrow inference, RC insertion, RC elimination, constructor reuse). LLVM sees only opaque runtime calls with correct attributes. This is Swift's strategy: all ARC optimization at the SIL level, LLVM never reasons about reference counting semantics.

**Why this matters:** Without correct attributes, standard LLVM optimization passes can break ARC semantics:
- **Dead Store Elimination (DSE)** may eliminate RC increment stores if functions are marked `readonly`
- **Loop-Invariant Code Motion (LICM)** may hoist RC operations out of loops if functions lack `memory` constraints
- **Global Value Numbering (GVN)** may merge distinct RC operations if functions appear pure

### RC Runtime Function Attributes

These attributes are set during function declaration (in `declare_runtime_functions` or its V2 equivalent), not during the optimization pass pipeline. The pass pipeline operates on the whole module after declarations are complete.

```rust
// ori_rc_inc(ptr): Increment reference count.
// NOT readonly/readnone — modifies the refcount word at ptr-8.
// argmemonly — only touches memory reachable from the argument pointer.
// nounwind — RC operations never throw.
fn declare_ori_rc_inc(cx: &CodegenCx) {
    let func = cx.declare_extern_fn("ori_rc_inc", &[ptr_ty.into()], None);
    func.add_attribute(AttributeLoc::Function, nounwind_attr);
    func.add_attribute(AttributeLoc::Function, memory_argmem_attr);
    // NO readonly, NO readnone — writes to refcount
}

// ori_rc_dec(ptr, drop_fn): Decrement reference count, call drop_fn if zero.
// NOT readonly — may decrement refcount AND free memory.
// argmemonly — only touches memory reachable from arguments.
// nounwind — drop functions must not unwind (panic = abort).
fn declare_ori_rc_dec(cx: &CodegenCx) {
    let func = cx.declare_extern_fn(
        "ori_rc_dec",
        &[ptr_ty.into(), ptr_ty.into()],
        None,
    );
    func.add_attribute(AttributeLoc::Function, nounwind_attr);
    func.add_attribute(AttributeLoc::Function, memory_argmem_attr);
    // NO readonly — writes refcount, may free memory
}

// ori_rc_alloc(size, align): Allocate new RC object.
// noalias on return — fresh allocation, no existing pointers alias it.
// nounwind — allocation failure = abort (no unwinding).
fn declare_ori_rc_alloc(cx: &CodegenCx) {
    let func = cx.declare_extern_fn(
        "ori_rc_alloc",
        &[i64_ty.into(), i64_ty.into()],
        Some(ptr_ty),
    );
    func.add_attribute(AttributeLoc::Function, nounwind_attr);
    func.add_attribute(AttributeLoc::Return, noalias_attr);
}

// ori_rc_free(ptr, size, align): Free RC object.
// nounwind — deallocation never throws.
fn declare_ori_rc_free(cx: &CodegenCx) {
    let func = cx.declare_extern_fn(
        "ori_rc_free",
        &[ptr_ty.into(), i64_ty.into(), i64_ty.into()],
        None,
    );
    func.add_attribute(AttributeLoc::Function, nounwind_attr);
}
```

### Attribute Rationale

| Attribute | Applied To | Why |
|-----------|-----------|-----|
| `nounwind` | All RC functions | Enables LLVM to optimize exception handling paths around RC calls. Drop functions are `nounwind` because panic during drop = abort. |
| `memory(argmem: readwrite)` | `ori_rc_inc`, `ori_rc_dec` | Prevents LICM from moving RC ops past unrelated memory operations. The function only reads/writes memory reachable from its pointer arguments (the refcount at `ptr-8`). |
| NOT `readonly`/`readnone` | `ori_rc_inc`, `ori_rc_dec` | Prevents DSE from eliminating RC stores. These functions modify the refcount word. |
| `noalias` (return) | `ori_rc_alloc` | Enables alias analysis. A fresh allocation does not alias any existing pointer. |

### Specialized Drop Functions

Per-type drop functions (e.g., `_ori_drop$List_Str`, `_ori_drop$MyStruct`) are generated by codegen (Section 07). They receive the same attributes:

```rust
// Specialized drop: nounwind + argmemonly.
// noinline — prevent inlining RC cleanup into hot paths.
fn declare_drop_function(cx: &CodegenCx, name: &str) {
    let func = cx.declare_extern_fn(name, &[ptr_ty.into()], None);
    func.add_attribute(AttributeLoc::Function, nounwind_attr);
    func.add_attribute(AttributeLoc::Function, memory_argmem_attr);
    func.add_attribute(AttributeLoc::Function, noinline_attr);
}
```

- [ ] Add `nounwind` + `memory(argmem: readwrite)` to `ori_rc_inc` and `ori_rc_dec` declarations
- [ ] Add `nounwind` + `noalias` (return) to `ori_rc_alloc` declaration
- [ ] Add `nounwind` to `ori_rc_free` declaration
- [ ] Add `nounwind` + `memory(argmem: readwrite)` + `noinline` to all specialized drop functions
- [ ] Verify via `--emit=llvm-ir` that attributes appear correctly on RC runtime declarations
- [ ] Test that LLVM O2/O3 does not reorder or eliminate RC operations in generated IR

---

## 11.4 Per-Function Attributes

Per-function attributes are set during function declaration (in `declare.rs` / `context.rs`), not during the optimization pass pipeline. The pass pipeline operates on the whole module. This distinction matters: attributes are per-function metadata, passes are per-module transformations.

### Calling Convention Attributes

Set at declaration time based on function origin (Section 04):

| Attribute | Applied To | Purpose |
|-----------|-----------|---------|
| `fastcc` | All internal Ori functions | Enables aggressive calling convention optimizations (register passing, tail calls) |
| `ccc` (C calling convention) | `@main`, `@panic`, FFI, runtime functions | Required for C ABI compatibility |

### Optimization Hint Attributes

Set at declaration time based on function characteristics:

| Attribute | Applied To | Purpose |
|-----------|-----------|---------|
| `noinline` | Specialized drop functions | Prevent inlining RC cleanup into hot paths. Drop functions are called on the cold path (refcount reaches zero). |
| `cold` | Panic/error path functions (`ori_panic`, `ori_panic_cstr`) | Hint to LLVM that these paths are unlikely. Moves panic code out of hot code layout. |
| `nounwind` | All RC runtime functions, drop functions | No unwinding through RC operations. |
| `noalias` (param 0) | `sret` return parameters | Caller-allocated return slot does not alias other memory. |
| `sret(T)` (param 0) | Functions returning large structs | Hidden struct return parameter (Section 04). |

### Future Attributes

These are not implemented in V2 but are planned for later phases:

- `alwaysinline` for small helper functions (e.g., trivial wrappers, field accessors)
- Function-level optimization hints from Ori annotations (e.g., `@inline`, `@cold`)
- Profile-guided optimization (PGO) metadata from runtime profiling data

- [ ] Add `cold` attribute to `ori_panic` and `ori_panic_cstr` declarations
- [ ] Add `fastcc` to all internal function declarations (V2 Section 04 integration)
- [ ] Verify `sret` + `noalias` attributes are preserved during V2 restructuring
- [ ] Document the declaration-time vs pass-time distinction in code comments

---

## 11.5 LTO Pipeline

### Existing LTO Modes

The current `LtoMode` enum supports three modes:

| Mode | Pipeline Strings | Use Case |
|------|-----------------|----------|
| Off | `default<OX>` | Default. Single-module compilation. |
| Thin | `thinlto-pre-link<OX>` + `thinlto<OX>` | Parallel, scalable. Recommended when LTO is desired. |
| Full | `lto-pre-link<OX>` + `lto<OX>` | Maximum optimization. Slower, more memory. Final release builds. |

### Pipeline Ordering for Multi-File Compilation

```
Per-module phase:     compile_to_llvm → verify → pre-link pipeline → emit bitcode
                                                   ↓
LTO phase:            merge bitcode → LTO pipeline → emit object
```

In the LTO phase, all per-module bitcode files are merged, then the full LTO pipeline runs. For Thin LTO, LLVM handles the parallelism internally.

### Existing Gap: `is_lto_phase` Flag

The `OptimizationConfig` struct has an `is_lto_phase: bool` field with an `as_lto_phase()` builder method. However, this flag is never set to `true` in any call site. The multi-file compilation path in `commands/build.rs` calls `run_optimization_passes` for each module individually but never performs the merge-and-LTO step.

**V2 must wire this up:** When `--lto=thin` or `--lto=full` is specified:
1. Per-module: use `prelink_pipeline_string()` (not `pipeline_string()`)
2. Emit bitcode (not object files) for each module
3. Merge bitcode
4. Run LTO pipeline with `is_lto_phase: true`
5. Emit final object from merged module

- [ ] Wire up `is_lto_phase` in the multi-file build path
- [ ] Per-module phase: use pre-link pipeline + emit bitcode when LTO is enabled
- [ ] LTO phase: merge bitcode + run LTO pipeline + emit object
- [ ] Test Thin LTO with a simple multi-module program
- [ ] Test Full LTO with a simple multi-module program

---

## 11.6 ModuleEmitter Integration

In V2, the pass pipeline integrates with the `ModuleEmitter` architecture. The full codegen pipeline for a single module is:

```
compile_to_llvm → verify_module → optimize → emit_object
```

`ModuleEmitter` owns the `optimize_module` call. The optimization pipeline is not a separate concern that callers must remember to invoke — it is a step within the `ModuleEmitter` pipeline.

```rust
/// ModuleEmitter orchestrates the full codegen pipeline for a single module.
///
/// Pipeline: lower → verify → optimize → emit
/// Each step is a method on ModuleEmitter. The caller does not need to
/// invoke optimization separately.
impl<'ll> ModuleEmitter<'ll> {
    /// Run the full pipeline: lower → verify → optimize → emit.
    pub fn emit(
        &self,
        config: &OptimizationConfig,
        output: &EmitOutput,
    ) -> Result<(), CodegenError> {
        // 1. Lower Ori IR to LLVM IR (already done by this point)
        // 2. Verify module (unconditional)
        self.verify()?;
        // 3. Run optimization passes
        self.optimize(config)?;
        // 4. Emit to requested format
        self.emit_to(output)?;
        Ok(())
    }
}
```

The `OptPipeline` type (wrapping `run_optimization_passes` and related configuration) lives within `ori_llvm::emit` as a module-level concern. It is not exposed as a top-level API that callers invoke independently.

**Relationship to existing code:** The current `run_optimization_passes` free function in `aot/passes.rs` becomes an internal implementation detail called by `ModuleEmitter::optimize`. External callers use `ModuleEmitter::emit` which handles the full pipeline.

- [ ] Implement `ModuleEmitter::verify` (calls `module.verify()`)
- [ ] Implement `ModuleEmitter::optimize` (calls `run_optimization_passes` with config)
- [ ] Implement `ModuleEmitter::emit` orchestrating the full pipeline
- [ ] Update `commands/build.rs` to use `ModuleEmitter` instead of manual pipeline steps
- [ ] Update `commands/run.rs` to use `ModuleEmitter` instead of manual pipeline steps
- [ ] Ensure `run_optimization_passes` is no longer public API (internal to `ModuleEmitter`)

---

## Completion Checklist

- [ ] All existing `passes.rs` functionality preserved and tested
- [ ] Profile presets renamed: Debug, Release, ReleaseFast, ReleaseSmall, ReleaseMinSize
- [ ] Module verification runs unconditionally before optimization on all paths (JIT and AOT)
- [ ] All RC runtime functions have correct LLVM attributes (nounwind, memory, noalias)
- [ ] Specialized drop functions have nounwind + argmemonly + noinline
- [ ] Per-function attributes (fastcc, cold, noinline, sret) set at declaration time
- [ ] `is_lto_phase` flag wired up for multi-file LTO builds
- [ ] `ModuleEmitter` owns the verify-optimize-emit pipeline
- [ ] `--emit=llvm-ir` works with new pipeline (already exists, verify preserved)
- [ ] Integration test: compile a program with RC types at O2, verify attributes in IR output

**Exit Criteria:** Debug builds compile fast (O0, minimal passes). Release builds produce well-optimized code with correct ARC semantics. No LLVM optimization pass can break reference counting because all RC functions carry defensive attributes. Module verification catches codegen bugs before they reach LLVM's optimizer.
