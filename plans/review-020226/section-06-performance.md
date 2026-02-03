---
section: "06"
title: Performance Optimization
status: in-progress
priority: high
goal: Eliminate O(n²) patterns, use FxHashMap in hot paths, reduce allocations
files:
  - compiler/oric/src/commands/build.rs
  - compiler/ori_types/src/core.rs
  - compiler/ori_typeck/src/registry/mod.rs
  - compiler/ori_eval/src/module_registration.rs
  - compiler/ori_llvm/src/**/*.rs
---

# Section 06: Performance Optimization

**Status:** 🔄 In Progress (06.1 ✅, 06.2 ✅, 06.3 partial, 06.4 ✅, 06.6 ✅, 06.7 ✅, 06.8 ✅)
**Priority:** HIGH — O(n²) patterns cause compilation slowdown on large projects
**Goal:** Fix algorithmic complexity issues and optimize hash map usage

---

## 06.1 Fix O(n²) Patterns

### build.rs: Linear Scan in Loop ✅

Location: `compiler/oric/src/commands/build.rs:836`

- [x] **FIXED**: Build `FxHashMap<&Path, &CompiledModuleInfo>` index once per module compilation
  - Changed from O(n*m) to O(n + m) where n = imports, m = compiled modules
  - Index is built once, then O(1) lookups for each import

### core.rs: ModuleNamespace Linear Scan

Location: `compiler/ori_types/src/core.rs:143`

- [ ] **Problem**: `items.iter().find()` for namespace field lookup
  ```rust
  // Before:
  Type::ModuleNamespace { items } => {
      items.iter().find(|(n, _)| *n == name).map(|(_, ty)| ty)
  }

  // After: Change ModuleNamespace to use HashMap
  Type::ModuleNamespace { items: FxHashMap<Name, Type> } => {
      items.get(&name)
  }
  ```

- [ ] Update all ModuleNamespace construction sites
- [ ] Update pattern matching on ModuleNamespace

### registry/mod.rs: Variant Lookup Fallback

Location: `compiler/ori_typeck/src/registry/mod.rs:387`

- [ ] **Problem**: Linear scan fallback after O(1) lookup fails
- [ ] Build variant index for built-in types as well

---

## 06.2 Fix Arc Cloning in Hot Path ✅

**COMPLETED**: Changed `collect_impl_methods`, `collect_extend_methods`, and `collect_def_impl_methods` to accept `&Arc<FxHashMap<Name, Value>>` instead of cloning internally.

### Changes Made

- [x] Updated `MethodCollectionConfig.captures` to `Arc<FxHashMap<Name, Value>>`
- [x] Changed all `collect_*` functions to accept `&Arc<FxHashMap<Name, Value>>`
- [x] Removed `Arc::new(captures.clone())` from inside each function
- [x] Updated `module_loading.rs` to wrap captures in Arc once before calling all three functions
- [x] Updated `playground-wasm/src/lib.rs` to wrap captures in Arc once
- [x] Updated all tests to use `Arc::new(FxHashMap::default())`
- [x] All 1,693 Ori spec tests pass

---

## 06.3 Eliminate Clone in Loop (Partial ✅)

Location: `compiler/oric/src/commands/build.rs:850-856`

- [x] **Pre-allocation**: Added `imported_functions.reserve(module_info.public_functions.len())` to avoid Vec reallocations
- [ ] **String/Vec cloning**: Clones remain for `mangled_name` and `param_types`. Changing to borrowing would require significant API changes for low-impact AOT code path. Deferred as low priority.

---

## 06.4 Replace HashMap with FxHashMap in ori_llvm ✅

**COMPLETED**: The ori_llvm crate has been migrated from `std::collections::HashMap` to `rustc_hash::FxHashMap` for faster hash operations with small keys.

### Changes Made

- [x] Added `rustc-hash = "2.1"` to ori_llvm dependencies
- [x] Replaced all HashMap/HashSet with FxHashMap/FxHashSet throughout the crate
- [x] Updated ~27 source files including:
  - [x] `compile_ctx.rs` — locals map
  - [x] `context.rs` — type caches, instances, tests
  - [x] `functions/body.rs` — per-function compilation
  - [x] `functions/calls.rs`, `expressions.rs`, `lambdas.rs`, `helpers.rs`, `sequences.rs`
  - [x] `module.rs` — tests() return type
  - [x] `aot/incremental/deps.rs` — dependency graph tracking
  - [x] `aot/incremental/hash.rs` — file metadata cache
  - [x] `aot/incremental/parallel.rs` — dependents map
  - [x] `aot/debug.rs` — primitives cache
  - [x] All collection modules and test files
- [x] All 198 ori_llvm tests pass

---

## 06.5 Fix Repeated HashMap Construction (Low Priority)

Location: `compiler/ori_eval/src/module_registration.rs:112-114`

**Analysis**: The trait_map is built once per `collect_impl_methods` call, which is once per module load. The original description ("called per impl") was inaccurate. Since each module is loaded only once during normal execution, this has minimal impact.

- [ ] **Optional**: Could pre-build trait_map in `MethodCollectionConfig` for cleaner code, but performance impact is negligible
  ```rust
  // Current: trait_map built once per module (acceptable)
  let mut trait_map: FxHashMap<Name, &ori_ir::TraitDef> = FxHashMap::default();
  for trait_def in &module.traits {
      trait_map.insert(trait_def.name, trait_def);
  }
  ```

---

## 06.6 Replace HashMap in ori_patterns ✅

**COMPLETED**: The ori_patterns crate has been migrated from `std::collections::HashMap` to `rustc_hash::FxHashMap`.

### Changes Made

- [x] Added `rustc-hash = "2.1"` to ori_patterns dependencies
- [x] Updated core types:
  - [x] `user_methods.rs` - UserMethod.captures, UserMethodRegistry.methods, derived_methods
  - [x] `composite.rs` - StructValue, FunctionValue captures, MemoizedFunctionValue cache
  - [x] `lib.rs` - ScopedBinding.prop_types
- [x] Updated all dependent crates for type consistency:
  - [x] `ori_eval` - Environment.capture(), module_registration, interpreter
  - [x] `ori_typeck` - pattern.rs prop_types
  - [x] `oric` - import.rs shared_captures, test files
- [x] All 6,368 tests pass

---

## 06.7 Bound Checking Allocations ✅

Location: `compiler/ori_typeck/src/checker/bound_checking.rs:294,299`

- [x] **Changed to FxHashMap**: Replaced `std::collections::HashMap` with `rustc_hash::FxHashMap` for faster hashing with small keys (`u32`, `Name`)
- [ ] Further optimization (caching/combining maps) deferred - already using fast hasher

---

## 06.8 Add #[inline] to Hot Functions ✅

### ori_llvm Hot Path Accessors

- [x] `CompileCtx::new()` - Added `#[inline]`
- [x] `CompileCtx::without_loop()` - Added `#[inline]`
- [x] `CompileCtx::with_loop_ctx()` - Added `#[inline]`
- [x] `CompileCtx::reborrow()` - Added `#[inline]`

### Verify ori_ir Coverage

- [x] ori_ir has good `#[inline]` coverage (158 instances verified)

---

## 06.9 Verification

- [ ] Profile before/after if benchmarks available
- [ ] `./clippy-all` passes
- [ ] `./test-all` passes
- [ ] Large file compilation times improved

---

## 06.N Completion Checklist

- [x] O(n²) pattern in build.rs fixed
- [ ] ModuleNamespace uses HashMap (requires Salsa compatibility analysis)
- [x] Arc cloning eliminated in module_registration
- [x] FxHashMap used in ori_llvm hot paths
- [x] FxHashMap used in ori_patterns (and dependent crates)
- [x] FxHashMap used in ori_typeck/bound_checking.rs
- [ ] Repeated HashMap construction eliminated (low priority - see 06.5)
- [x] `#[inline]` on hot accessors (CompileCtx methods)
- [x] `./test-all` passes (6,368 tests, 0 failures)

**Exit Criteria:** No O(n²) patterns in hot paths; FxHashMap used consistently in performance-critical code
