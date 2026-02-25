---
section: "04"
title: "Hash-First Import Resolution"
status: not_started
goal: "At import boundaries, resolve types by Merkle hash lookup (O(1)) instead of AST re-walking (O(depth)), falling back to AST only for types not yet in the local pool"
inspired_by:
  - "Git fetch — only transfer objects you don't already have"
  - "DNS caching — check local cache before upstream query"
  - "CPU cache hierarchy — L1 hit (hash lookup) vs L3 miss (AST walk)"
sections:
  - id: "04.1"
    title: "Hash-Lookup Import Path"
    status: not_started
  - id: "04.2"
    title: "Prelude Cache Warming"
    status: not_started
  - id: "04.3"
    title: "AST Fallback Path"
    status: not_started
  - id: "04.4"
    title: "Import Resolution Benchmark"
    status: not_started
---

# Section 04: Hash-First Import Resolution

**Status:** Not Started
**Goal:** Replace the AST-walking import path with a hash-first lookup that resolves types
in O(1) when they already exist in the local pool, falling back to AST walking only for
genuinely novel types.

**Why this matters:** This is the primary performance win. The current import path calls
`resolve_and_check_type_with_vars()` for every parameter and return type of every imported
function — recursively walking the AST type tree. For a function with signature
`(Map<str, List<Option<T>>>, (T) -> U) -> Map<str, List<U>>`, that's ~10 recursive calls,
each doing hash lookups, name resolution, and pool interning.

With hash-first resolution, the same import is ~6 hash lookups in `intern_map` — one per
unique type node in the signature. For types that already exist (which is the common case
after prelude loading), each lookup is a single `FxHashMap::get()` call.

**This section** depends on Section 03 (hash-forwarded signatures). It is the primary
consumer of Merkle hashes.

---

## 04.1 Hash-Lookup Import Path — NOT STARTED

**Goal:** Add a fast path to `register_imported_function()` that resolves types by
Merkle hash lookup before falling back to AST re-walking.

**Current flow** (`check/mod.rs:420-438`):
```
register_imported_function(func, foreign_arena)
  → infer_function_signature_from(checker, func, foreign_arena)
    → infer_function_signature_with_arena(checker, func, foreign_arena)
      → for each param: resolve_and_check_type_with_vars(...)  // AST walk
      → resolve return type
    → build FunctionSig with local Idx values
  → pool.function(&sig.param_types, sig.return_type)
  → pool.scheme(...) if generic
  → import_env.bind(...)
  → signatures.insert(...)
```

**New flow — hash-first with AST fallback:**
```
register_imported_function(func, foreign_arena, imported_sig)
  // imported_sig: the FunctionSig from module A's TypeCheckResult
  // imported_sig.param_hashes / return_hash are Merkle hashes

  → try_resolve_by_hash(imported_sig, &mut pool)
    → for each param_hash in imported_sig.param_hashes:
        if let Some(idx) = pool.lookup_by_hash(param_hash):
          local_params.push(idx)  // O(1) hit
        else:
          return None  // at least one type not in local pool
    → same for return_hash
    → if all resolved: build FunctionSig with local Idx values
    → return Some(local_sig)

  → if None: fall back to current AST path
    → infer_function_signature_from(checker, func, foreign_arena)
```

**New Pool method:**
```rust
impl Pool {
    /// Look up a type by its Merkle hash.
    ///
    /// Returns `Some(idx)` if a type with this hash exists in the pool,
    /// `None` otherwise. O(1) via `intern_map`.
    #[inline]
    pub fn lookup_by_hash(&self, merkle_hash: u64) -> Option<Idx> {
        self.intern_map.get(&merkle_hash).copied()
    }
}
```

**Modified `register_imported_function()`:**
```rust
pub fn register_imported_function(
    &mut self,
    func: &ori_ir::Function,
    foreign_arena: &ExprArena,
    imported_sig: Option<&FunctionSig>,  // NEW: from module A's TypeCheckResult
) {
    // Fast path: resolve by Merkle hash
    if let Some(ext_sig) = imported_sig {
        if let Some(local_sig) = self.try_resolve_sig_by_hash(ext_sig) {
            let fn_type = self.pool.function(&local_sig.param_types, local_sig.return_type);
            let bound_type = if local_sig.scheme_var_ids.is_empty() {
                fn_type
            } else {
                self.pool.scheme(&local_sig.scheme_var_ids, fn_type)
            };
            self.import_env.bind(local_sig.name, bound_type);
            self.signatures.insert(local_sig.name, local_sig);
            return;
        }
    }

    // Slow path: AST re-walking (existing code, unchanged)
    let (sig, var_ids) = signatures::infer_function_signature_from(self, func, foreign_arena);
    let fn_type = self.pool.function(&sig.param_types, sig.return_type);
    let bound_type = if var_ids.is_empty() {
        fn_type
    } else {
        self.pool.scheme(&var_ids, fn_type)
    };
    self.import_env.bind(sig.name, bound_type);
    self.signatures.insert(sig.name, sig);
}
```

**`try_resolve_sig_by_hash()` implementation:**
```rust
fn try_resolve_sig_by_hash(&self, ext_sig: &FunctionSig) -> Option<FunctionSig> {
    // Try to resolve every param type by hash
    let mut local_param_types = Vec::with_capacity(ext_sig.param_types.len());
    for &hash in &ext_sig.param_hashes {
        local_param_types.push(self.pool.lookup_by_hash(hash)?);
    }

    // Try to resolve return type by hash
    let local_return_type = self.pool.lookup_by_hash(ext_sig.return_hash)?;

    // All types resolved — build local FunctionSig
    Some(FunctionSig {
        name: ext_sig.name,
        type_params: ext_sig.type_params.clone(),
        const_params: ext_sig.const_params.clone(),
        param_names: ext_sig.param_names.clone(),
        param_types: local_param_types,
        return_type: local_return_type,
        capabilities: ext_sig.capabilities.clone(),
        param_hashes: ext_sig.param_hashes.clone(),
        return_hash: ext_sig.return_hash,
        // ... clone remaining fields from ext_sig ...
    })
}
```

**Important consideration for generic functions:**

Generic functions have type variables in their signatures. A function `fn foo<T>(x: T) -> T`
has `param_types = [Var(id)]` and `return_type = Var(id)`. The Merkle hash of `Var(id)` includes
the var_states index, which is pool-local.

**For generic functions, the hash-first path must handle type variables specially:**
- Type variable hashes will NOT match across pools (different var_states indices)
- Option A: Skip hash-first for generic functions (fall back to AST)
- Option B: Hash type variables by their scheme position (de Bruijn index)
- Option C: Resolve non-variable types by hash, create fresh variables locally

**Recommended: Option A initially.** Generic function import already creates fresh type
variables in the local pool. The cost of AST walking for generic functions is low (the
variable creation is the dominant cost, not the type resolution). Non-generic functions
(which are the majority of imports) get the full O(1) benefit.

Future optimization (Option C) can be added in a later pass if profiling shows generic
function imports are a bottleneck.

**Caller changes** (`oric/src/typeck.rs:170-250`, `register_resolved_imports()`):
```rust
// Current:
checker.register_imported_function(func, &imported_parsed.arena);

// New: pass imported module's FunctionSig if available
let imported_sig = imported_typed.as_ref()
    .and_then(|tc| tc.functions.iter().find(|s| s.name == func_ref.original_name));
checker.register_imported_function(func, &imported_parsed.arena, imported_sig);
```

This requires `register_resolved_imports()` to have access to the imported module's
`TypeCheckResult`. Currently it receives `ResolvedImports` which contains `ParseOutput`
(AST only). Need to also pass the `TypeCheckResult` for each module, obtained via the
Salsa `typed()` query.

**Exit Criteria:**
- [ ] `Pool::lookup_by_hash()` method added
- [ ] `try_resolve_sig_by_hash()` implemented
- [ ] `register_imported_function()` accepts optional imported FunctionSig
- [ ] Non-generic imports resolved by hash (O(1) per type)
- [ ] Generic imports fall back to AST path (correct behavior preserved)
- [ ] `register_resolved_imports()` passes TypeCheckResult for hash resolution
- [ ] All existing import tests pass unchanged

---

## 04.2 Prelude Cache Warming — NOT STARTED

**Goal:** Ensure that prelude types (the most commonly imported types) are always present
in every module's pool, so hash-first lookups always hit for prelude types.

**Current behavior:** The prelude is imported via `register_imported_function()` for each
prelude function. Each call walks the AST of the prelude function's signature, which
creates the necessary types in the local pool. So `int`, `str`, `List<int>`, etc. are
already present after prelude import.

**After hash-first:** Prelude functions are the FIRST imports processed. On the first import,
the local pool is fresh (only primitives 0-11). Common types like `List<str>`, `Option<int>`,
`Map<str, int>` don't exist yet → hash lookup misses → falls back to AST.

But after the first few prelude functions are imported, common types accumulate in the pool.
Subsequent imports of functions with the same types (which is very common — most prelude
functions use `int`, `str`, `List`, `Option`) hit the hash cache.

**Optimization: Pre-warm with common types.**

After primitives are interned (during `Pool::new()`), eagerly intern the most common
compound types:

```rust
impl Pool {
    /// Pre-intern commonly used compound types for hash-first import performance.
    ///
    /// Called during pool construction. These types appear in nearly every
    /// module's imports (via prelude). Pre-interning ensures hash lookups
    /// hit immediately, avoiding AST fallback for the most common types.
    fn intern_common_types(&mut self) {
        // Common containers of primitives
        for &prim in &[Idx::INT, Idx::FLOAT, Idx::BOOL, Idx::STR, Idx::CHAR, Idx::BYTE] {
            self.list(prim);
            self.option(prim);
        }

        // Common function types
        self.function(&[], Idx::UNIT);     // () -> void
        self.function(&[], Idx::INT);      // () -> int
        self.function(&[], Idx::STR);      // () -> str
        self.function(&[], Idx::BOOL);     // () -> bool
        self.function1(Idx::INT, Idx::INT);    // (int) -> int
        self.function1(Idx::STR, Idx::STR);    // (str) -> str
        self.function1(Idx::INT, Idx::BOOL);   // (int) -> bool

        // Common pairs
        self.pair(Idx::INT, Idx::INT);
        self.pair(Idx::STR, Idx::INT);

        // Common maps
        self.map(Idx::STR, Idx::INT);
        self.map(Idx::STR, Idx::STR);
    }
}
```

**Trade-off:** Pre-interning ~30-40 types adds ~200 bytes to each pool and takes ~1μs.
This is negligible. The benefit is that the most common import types always hit the
hash cache on first access.

**Alternative: Lazy warming via prelude import.** Instead of pre-interning, let the
prelude import path (which runs first for every module) warm the cache naturally. The
first module to import the prelude pays the AST cost; subsequent modules benefit from
hash hits. With Salsa memoization, the prelude's TypeCheckResult is cached, so hash-forwarded
import is available for all modules after the first.

**Recommended:** Start with lazy warming (no code change needed). Add pre-interning
only if benchmarks show first-module prelude import is a bottleneck.

**Exit Criteria:**
- [ ] Prelude import path verified to warm common types
- [ ] Decision documented: lazy warming vs pre-interning (based on benchmarks)
- [ ] If pre-interning chosen: `intern_common_types()` added to `Pool::new()`
- [ ] Hash hit rate measured for typical module with prelude imports

---

## 04.3 AST Fallback Path — NOT STARTED

**Goal:** Ensure the AST fallback path (existing `infer_function_signature_from()`)
remains correct and well-tested as the primary path for types not yet in the local pool.

**When the fallback fires:**
1. First module to import a type not in the prelude (user-defined struct, etc.)
2. Generic function imports (type variables don't match by hash)
3. Functions with effect types (`uses Http`) that aren't pre-warmed

**Important:** The fallback path must populate hashes in the resulting FunctionSig
(via `populate_hashes()`) so that SUBSEQUENT importers of the same module can use
hash-first resolution.

**Flow diagram:**
```
Module A exports foo(x: MyStruct) -> List<MyStruct>

Module B imports foo:
  1. Try hash-first: lookup hash(MyStruct) → MISS
  2. Fall back to AST: walk foo's signature
     → creates MyStruct in B's pool (with Merkle hash)
     → creates List<MyStruct> in B's pool (with Merkle hash)
  3. FunctionSig stored with param_hashes, return_hash

Module C imports foo:
  1. Try hash-first: lookup hash(MyStruct) → depends:
     - If C also imported MyStruct directly → HIT
     - If C hasn't seen MyStruct → MISS, fall back to AST
  2. After resolution, C's pool has MyStruct → future lookups HIT
```

**Test:**
```rust
#[test]
fn import_fallback_populates_hashes() {
    // Module A: defines foo(x: int) -> List<int>
    // Module B: imports foo via AST fallback
    // Verify: B's FunctionSig has correct param_hashes and return_hash

    let mut checker = ModuleChecker::new(/* ... */);
    checker.register_imported_function(func, foreign_arena, None);

    let sig = checker.signatures.get(&foo_name).unwrap();
    assert!(!sig.param_hashes.is_empty());
    assert_eq!(sig.param_hashes.len(), sig.param_types.len());
    assert_ne!(sig.return_hash, 0);
}
```

**Exit Criteria:**
- [ ] AST fallback always populates hash fields in resulting FunctionSig
- [ ] Fallback path unchanged for generic functions
- [ ] Test verifying hash population after fallback
- [ ] No regression in import correctness

---

## 04.4 Import Resolution Benchmark — NOT STARTED

**Goal:** Measure the performance improvement from hash-first import resolution.

**Benchmark design:**

```rust
// In compiler/oric/benches/ or as a dedicated test

fn bench_import_resolution() {
    // Setup: Module A with 50 exported functions of varying complexity
    // Module B imports all 50 functions

    // Baseline: AST-walking import (current code)
    let t_ast = measure(|| {
        for func in &module_a.functions {
            checker.register_imported_function(func, &arena, None);
        }
    });

    // Optimized: Hash-first import
    let t_hash = measure(|| {
        for (func, sig) in module_a.functions.iter().zip(&typed_result.functions) {
            checker.register_imported_function(func, &arena, Some(sig));
        }
    });

    // Report: t_hash / t_ast (expect 0.1–0.3x for non-generic functions)
}
```

**Metrics to capture:**
- Import time per function (ns)
- Hash hit rate (% of types resolved by hash vs AST fallback)
- Total import time for 50 functions
- Breakdown by function complexity (0 params, 1 param, 3+ params, generic)

**Expected results:**
- Non-generic functions with primitive types: ~5-10x faster (hash lookup vs AST walk)
- Non-generic functions with user types (first import): no improvement (cache miss)
- Non-generic functions with user types (subsequent): ~3-5x faster
- Generic functions: no improvement (AST fallback)
- Overall for typical module: ~2-5x faster import resolution

**Exit Criteria:**
- [ ] Benchmark implemented and runnable via `cargo bench`
- [ ] Hash hit rate ≥ 80% for typical non-generic imports (after prelude)
- [ ] ≥ 2x overall import resolution speedup measured
- [ ] Results documented in this section with numbers

---

## Section 04 Completion Checklist

- [ ] `Pool::lookup_by_hash()` method added (04.1)
- [ ] Hash-first import path implemented in `register_imported_function()` (04.1)
- [ ] `try_resolve_sig_by_hash()` handles non-generic functions (04.1)
- [ ] Generic functions correctly fall back to AST (04.1)
- [ ] `register_resolved_imports()` passes TypeCheckResult for hash resolution (04.1)
- [ ] Prelude cache warming strategy decided and implemented (04.2)
- [ ] AST fallback populates hash fields (04.3)
- [ ] Import resolution benchmark showing ≥ 2x speedup (04.4)
- [ ] All existing tests pass unchanged
- [ ] `./test-all.sh` passes
