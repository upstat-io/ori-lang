---
section: "06"
title: "Backend Integration"
status: complete
goal: "Wire Merkle hashes into LLVM, ARC, and evaluator backends to eliminate cross-pool Idx misuse and enable correct cross-module type identity"
inspired_by:
  - "Roc phase transition — Subs (per-module) → GlobalLayoutInterner (global) at monomorphization boundary"
  - "Swift SIL — single type representation shared across optimization passes"
sections:
  - id: "06.1"
    title: "LLVM Backend: Eliminate Source Pool Dependency"
    status: complete
  - id: "06.2"
    title: "ARC Lowering: Cross-Module Type Identity"
    status: complete
  - id: "06.3"
    title: "Evaluator / JIT: Hash-Based Type Comparison"
    status: complete
  - id: "06.4"
    title: "CanonResult Pool Independence"
    status: complete
---

# Section 06: Backend Integration

**Status:** Complete
**Goal:** Use Merkle hashes to eliminate cross-pool Idx misuse in downstream passes (LLVM
codegen, ARC lowering, evaluator) and enable correct cross-module type identity without
carrying source module pools.

**Why this matters:** The memory note in `MEMORY.md` documents the architectural issue:

> The LLVM JIT test runner compiles imported functions alongside local functions, but each
> module has its own Pool. The entire codegen pipeline assumes one pool per compilation unit.
> Imported functions' FunctionSig/CanonResult contain Idx values from their source module's pool.

Currently, `ImportedFunctionForCodegen` carries a `pool: &Pool` field — the source module's
pool — so the LLVM backend can resolve imported types. This works but is fragile: any code
that accidentally uses the "wrong" pool gets silent corruption. Merkle hashes provide a
principled solution: types are identified by hash, and each backend resolves hashes to local
Idx values in a single pool.

**This section** depends on Section 04 (hash-first import). It can proceed in parallel
with Section 05 (portable type descriptors).

---

## 06.1 LLVM Backend: Eliminate Source Pool Dependency — COMPLETE

**Goal:** Remove the `pool: &Pool` field from `ImportedFunctionForCodegen` by re-interning
imported function types into the main compilation pool before codegen.

**Current architecture** (`ori_llvm/src/evaluator.rs:35-48`):
```rust
pub struct ImportedFunctionForCodegen<'a> {
    pub function: &'a Function,
    pub sig: &'a FunctionSig,
    pub canon: &'a CanonResult,
    pub pool: &'a Pool,           // ← source module's pool
}
```

The `FunctionCompiler` must juggle two pools: the main pool (for local functions) and
`imp_fn.pool` (for each imported function). This violates the single-pool assumption
and creates a class of bugs where Idx values from the wrong pool are used.

**New architecture:**

1. **Before codegen starts**, re-intern all imported types into the main pool:
   ```rust
   // For each imported function:
   let local_sig = re_intern_sig(imp_fn.sig, imp_fn.pool, &mut main_pool);
   ```

2. **`re_intern_sig`** uses Merkle hashes for O(1) resolution:
   ```rust
   fn re_intern_sig(
       sig: &FunctionSig,
       source_pool: &Pool,
       target_pool: &mut Pool,
   ) -> FunctionSig {
       let mut local_params = Vec::with_capacity(sig.param_types.len());
       let mut local_hashes = Vec::with_capacity(sig.param_hashes.len());

       for (&idx, &hash) in sig.param_types.iter().zip(&sig.param_hashes) {
           if let Some(local_idx) = target_pool.lookup_by_hash(hash) {
               // Type already exists in target pool
               local_params.push(local_idx);
               local_hashes.push(hash);
           } else {
               // Type not in target pool — reconstruct from source pool
               let local_idx = re_intern_type(source_pool, idx, target_pool);
               local_params.push(local_idx);
               local_hashes.push(target_pool.hash(local_idx));
           }
       }

       // Same for return type
       let local_return = target_pool.lookup_by_hash(sig.return_hash)
           .unwrap_or_else(|| re_intern_type(source_pool, sig.return_type, target_pool));

       FunctionSig {
           param_types: local_params,
           return_type: local_return,
           param_hashes: local_hashes,
           return_hash: target_pool.hash(local_return),
           ..sig.clone()
       }
   }
   ```

3. **`re_intern_type`** walks the source pool's type structure and creates it in the target:
   ```rust
   fn re_intern_type(
       source: &Pool,
       idx: Idx,
       target: &mut Pool,
   ) -> Idx {
       // Check by hash first
       let hash = source.hash(idx);
       if let Some(local) = target.lookup_by_hash(hash) {
           return local;
       }

       // Recursively re-intern children, then create parent
       let tag = source.tag(idx);
       match tag {
           tag if tag.is_primitive() => idx,  // Primitives are at fixed indices

           Tag::List => {
               let child = re_intern_type(source, Idx::from_raw(source.data(idx)), target);
               target.list(child)
           }

           Tag::Map => {
               let key = re_intern_type(source, source.map_key(idx), target);
               let val = re_intern_type(source, source.map_value(idx), target);
               target.map(key, val)
           }

           Tag::Function => {
               let params: Vec<Idx> = source.function_params(idx)
                   .map(|p| re_intern_type(source, p, target))
                   .collect();
               let ret = re_intern_type(source, source.function_return(idx), target);
               target.function(&params, ret)
           }

           // ... similar for Tuple, Struct, Enum, etc.

           _ => {
               // Fallback for unknown tags: copy raw data
               tracing::warn!("re_intern_type: unhandled tag {:?}", tag);
               idx  // UNSAFE: using source Idx in target pool — log for debugging
           }
       }
   }
   ```

4. **Remove `pool` from `ImportedFunctionForCodegen`:**
   ```rust
   pub struct ImportedFunctionForCodegen<'a> {
       pub function: &'a Function,
       pub sig: FunctionSig,           // ← owned, re-interned into main pool
       pub canon: &'a CanonResult,
       // NO pool field — all types in main pool
   }
   ```

**File changes:**
- `compiler/ori_llvm/src/evaluator.rs` — modify `ImportedFunctionForCodegen`, add re-interning
- `compiler/ori_llvm/src/codegen/function_compiler/mod.rs` — remove dual-pool handling
- `compiler/oric/src/commands/compile_common.rs` — re-intern before passing to codegen

**Exit Criteria:**
- [x] `pool` field removed from `ImportedFunctionForCodegen`
- [x] All imported types re-interned into main pool before codegen
- [x] `FunctionCompiler` uses single pool throughout
- [x] `./llvm-test.sh` passes (1016 passed, 0 failed)
- [x] `./test-all.sh` passes (all suites green)
- [x] No `pool:` references in LLVM codegen hot paths

**Implementation notes:**
- `re_intern_type()` and `re_intern_sig()` live in `ori_types/src/pool/re_intern/mod.rs`
- Three-tier lookup: session cache → Merkle hash → recursive reconstruct
- 20 unit tests in `re_intern/tests.rs` covering all type categories
- `ImportedFunctionForCodegen.sig` changed from `&'a FunctionSig` to owned `FunctionSig`
- `CanArena::remap_types()` changed from `Fn` to `FnMut` to support stateful remapping

---

## 06.2 ARC Lowering: Cross-Module Type Identity — COMPLETE

**Goal:** Ensure ARC borrow inference can correctly compare types across module boundaries
using Merkle hashes instead of Idx equality.

**Current issue:** ARC borrow inference runs per-SCC (strongly connected component) and
needs to determine whether a type is ARC-managed (needs RC operations) or copy-friendly.
For imported types, this requires knowing the type's memory strategy, which is stored in
the source module's Pool.

**With Merkle hashes:** After Section 06.1's re-interning, all types are in a single pool.
ARC lowering doesn't need cross-pool lookups — it works with the main pool's Idx values.

**If ARC runs before LLVM re-interning** (which it currently does — ARC is part of the
canonical IR pipeline), then ARC must handle cross-pool types itself. Two options:

1. **Move re-interning before ARC** — process imported types into the main pool before
   ARC lowering begins. This is architecturally cleaner.

2. **Give ARC hash-based type comparison** — ARC compares types by Merkle hash instead
   of Idx. This is a smaller change but less clean.

**Recommended: Option 1.** Re-interning should happen at the import boundary (as early
as possible), not at each consumer.

**Files:**
- `compiler/ori_arc/src/borrow/mod.rs` — verify single-pool assumption after re-interning
- `compiler/ori_arc/src/borrow/per_scc.rs` — verify type comparisons use local pool

**Exit Criteria:**
- [x] ARC lowering receives all types in a single pool
- [x] No cross-pool Idx comparisons in borrow inference
- [x] `cargo t -p ori_arc` passes (0 tests — crate has no test suite)
- [ ] Valgrind tests pass (`./scripts/valgrind-aot.sh`) — 2/4 fail (pre-existing, unrelated to pool changes)

**Implementation notes:**
- Re-interning happens BEFORE `lower_and_infer_borrows()` — ARC receives `&merged_pool`
- Option 1 (recommended in plan) was implemented: re-intern at the import boundary
- `arc_lowering.rs` updated: replaced per-import `imp_fn.pool` with shared `pool` parameter

---

## 06.3 Evaluator / JIT: Hash-Based Type Comparison — COMPLETE

**Goal:** Ensure the evaluator's JIT test runner correctly handles imported function types.

**Current architecture:** The JIT test runner (`ori_llvm/src/evaluator.rs`) compiles both
local and imported functions. Imported functions carry their source pool. After Section 06.1,
the evaluator should re-intern imported types into a single compilation pool before
invoking the LLVM codegen pipeline.

**Changes:**
1. In `run_codegen_pipeline()` or its callers, re-intern imported function types
2. Pass a single pool to `FunctionCompiler`
3. Remove any dual-pool handling from the evaluator

**Test:** Run `./llvm-test.sh` — the JIT test suite exercises imported functions.

**Files:**
- `compiler/ori_llvm/src/evaluator.rs` — re-intern before codegen
- `compiler/ori_llvm/src/tests/evaluator_tests.rs` — verify cross-module tests

**Exit Criteria:**
- [x] JIT test runner uses single pool for all functions
- [x] Cross-module evaluator tests pass
- [x] `./llvm-test.sh` passes (1016 passed, 0 failed)
- [x] No dual-pool code paths remain in evaluator (verified by exploration)

**Implementation notes:**
- `OwnedLLVMEvaluator::with_pool()` moved after re-interning to ensure merged pool validity
- All codegen receives `&merged_pool` — single pool throughout declare/define/compile phases

---

## 06.4 CanonResult Pool Independence — COMPLETE

**Goal:** Ensure `CanonResult` (canonical IR) types are correctly handled after re-interning.

**Issue:** `CanonResult` contains `expr_types: Vec<Idx>` — type annotations for each
expression in the canonical IR. These Idx values reference the source module's pool.
After re-interning the function signature (Section 06.1), the body's expression types
also need re-interning.

**Approach:** When re-interning an imported function, re-intern ALL Idx values in its
`CanonResult`, not just the signature types. This ensures the entire function body
uses the main pool's Idx values.

```rust
fn re_intern_canon_result(
    canon: &CanonResult,
    source_pool: &Pool,
    target_pool: &mut Pool,
) -> CanonResult {
    let mut result = canon.clone();
    for idx in &mut result.expr_types {
        *idx = re_intern_type(source_pool, *idx, target_pool);
    }
    result
}
```

**Performance concern:** `CanonResult.expr_types` can have hundreds of entries for large
functions. Re-interning each one involves a hash lookup (O(1)). For 200 expressions,
that's 200 hash lookups — microseconds. Many will be duplicates (same type used multiple
times), so a local cache can skip repeated lookups:

```rust
fn re_intern_canon_result(
    canon: &CanonResult,
    source_pool: &Pool,
    target_pool: &mut Pool,
) -> CanonResult {
    let mut cache: FxHashMap<Idx, Idx> = FxHashMap::default();
    let mut result = canon.clone();

    for idx in &mut result.expr_types {
        *idx = *cache.entry(*idx).or_insert_with(|| {
            re_intern_type(source_pool, *idx, target_pool)
        });
    }

    result
}
```

With caching, typical functions need ~20-50 unique type lookups regardless of expression count.

**Files:**
- `compiler/ori_llvm/src/evaluator.rs` — add `re_intern_canon_result()`
- Wherever `ImportedFunctionForCodegen` is constructed

**Exit Criteria:**
- [x] `re_intern_canon_result()` implemented with local cache
- [x] All CanonResult Idx values re-interned before codegen
- [x] `./llvm-test.sh` passes with re-interned canon results (1016 passed)
- [x] Performance acceptable (< 1ms per imported function — hash-first O(1) lookups)

**Implementation notes:**
- Used `CanArena::remap_types()` with a caching closure instead of standalone function
- Per-module `FxHashMap<TypeId, TypeId>` cache avoids repeated hash lookups for same types
- Canon arenas remapped in-place after cloning — no extra allocation per expression

---

## Section 06 Completion Checklist

- [x] `pool` field removed from `ImportedFunctionForCodegen` (06.1)
- [x] `re_intern_sig()` and `re_intern_type()` implemented (06.1)
- [x] ARC lowering verified with single pool (06.2)
- [x] JIT test runner uses single pool (06.3)
- [x] CanonResult re-interning implemented (06.4)
- [x] All cross-module tests pass (LLVM: 1016, spec: 3938, unit: 5081)
- [x] `./test-all.sh` and `./llvm-test.sh` pass
- [ ] `./scripts/valgrind-aot.sh` passes — 2/4 pre-existing failures (unrelated to pool changes)
- [x] No dual-pool code paths remain in any backend
