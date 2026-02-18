---
plan: "dpr_type-checker-perf_02182026"
title: "Design Pattern Review: Type Checker Performance"
status: draft
---

# Design Pattern Review: Type Checker Performance

## Ori Today

Ori's type checker follows a multi-pass architecture: registration (Pass 0a-0e), signature collection (Pass 1), then body checking (Passes 2-5). The core is well-designed: `Idx(u32)` handles give O(1) type equality, `Pool` uses parallel arrays (items, flags, hashes, extra, var_states) for cache-friendly access, and `UnifyEngine` implements link-based union-find with path compression for O(alpha(n)) unification. Pre-interned primitives at fixed indices (0-11) enable zero-cost primitive type operations. TypeFlags provide O(1) type classification, and `TypeEnv` uses `Rc`-based parent sharing for O(1) scope creation. These fundamentals are sound.

Current benchmarks show reasonable per-function throughput: ~2.5 us/fn at 10 functions scaling to ~1.7 us/fn at 500 functions (inferred), with Salsa cache hits at 8.7 us. The type checker is approximately 10x slower than the parser for equivalent input (317 us vs ~23 us for 100 functions), which is within expected bounds for HM inference. Registration overhead is amortized well at scale, suggesting the per-module setup cost (Pool::new, register_builtin_types, register_traits, collect_signatures) is modest.

**The critical anomaly is that annotated code is 2x SLOWER than inferred code (140 us vs 71 us for 100 functions).** This is counterintuitive -- annotations should provide shortcuts by avoiding inference. The data suggests the annotation path introduces overhead that exceeds the cost of the work it avoids. Investigation of the code reveals multiple candidates: (1) `resolve_type_with_vars()` performs interner lookups and recursive `ParsedType` traversal for every annotated parameter/return type during signature collection, while inference just calls `pool.fresh_var()`; (2) `check_parsed_type_object_safety()` walks every annotation in both the signature phase AND the body-checking phase (double traversal); (3) the bidirectional `check_expr` path adds `Expected` struct construction, a `resolve()` call on the expected type, and a `check_type()` unification on top of normal inference; (4) `resolve_type_with_vars` calls `checker.interner().lookup(name)` for string comparison on every `ParsedType::Named` even for primitives like `int` and `str`, doing a string match table lookup that `fresh_var()` avoids entirely.

## Prior Art

### Rust -- Arena-Interned Types + Granular Query Caching

Rust's `TyCtxt<'tcx>` owns all type metadata in a central arena, with `Ty<'tcx>` being just a pointer (8 bytes). Type equality is pointer comparison -- O(1), same principle as Ori's `Idx`. The critical performance lesson is **granular query caching**: Rust caches at the per-function level with fingerprint-based invalidation and diagnostic replay. Ori currently rebuilds all registries per `typed(db, file)` call -- there is no caching of `TraitRegistry`, `TypeRegistry`, or the Pool across Salsa invocations. Rust also separates the trait solver from type representation, which avoids coupling trait resolution overhead with basic type checking.

### Zig -- Unified Index Pool + Per-Function Invalidation

Zig's `InternPool` unifies types AND values in a single u32-indexed pool -- the same design philosophy as Ori's `Pool`. The key performance insight is **per-function granularity**: Zig tracks dependencies at the `AnalUnit` level (individual function or type), not per-file. When a function body changes but its signature doesn't, only that function is re-analyzed. Ori's Salsa granularity is per-file (`typed(db, file)`), meaning changing one function re-checks the entire file. Zig also uses thread-local shards with TID-encoded indices for concurrent type checking -- a long-term aspiration for Ori if parallelism becomes needed.

### Gleam -- Two-Phase Hydrator + Link Collapse

Gleam's type checker uses a two-phase approach: `Hydrator` processes type annotations, `ExprTyper` performs inference. This separation is notable because Gleam's annotation path is demonstrably faster than inference (the expected behavior). Gleam's `collapse_links()` performs eager path compression -- same as Ori's `resolve()`. The key difference: Gleam's `Hydrator` converts annotations directly to type variables without going through a separate `ParsedType` intermediate representation for simple cases. Primitives in annotations are resolved to concrete types immediately, without string matching. Ori's three separate type resolution functions (`resolve_parsed_type_simple`, `resolve_type_with_vars`, `resolve_parsed_type`) each independently walk `ParsedType` trees and perform string matching for primitives.

### TypeScript -- Flag-Based Classification + Lazy Resolution

TypeScript uses `TypeFlags` (27+ bitflags) for O(1) type classification -- Ori already has this pattern. The critical performance technique is **lazy property resolution**: TypeScript types have `resolvedReturnType?`, `resolvedProperties?` etc. that are computed on first access rather than eagerly during construction. TypeScript also employs multiple specialized caches: `subtypeReductionCache`, `contextualIsRelatedCache`, etc. The lesson for Ori is that a single `intern_map` for deduplication is necessary but not sufficient -- hot lookup patterns (like "does this trait have a method X?") deserve their own single-purpose cache.

## Proposed Optimizations

### Investigation: Annotation Slowdown

The 2x slowdown of annotations over inference has multiple root causes, all stemming from the annotation path doing MORE work than the inference path, not less:

**Hypothesis 1: Double type resolution in signature phase.** For annotated functions, `infer_function_signature_with_arena()` (signatures/mod.rs:128-263) calls `resolve_type_with_vars()` for every parameter and return type. Each call:
- Walks the `ParsedType` AST recursively
- Calls `checker.interner().lookup(name)` for string comparison (hash map lookup + string compare) on every `Named` node
- Calls `check_parsed_type_object_safety()` which walks the SAME tree again
- For primitives like `int`, goes through `ParsedType::Named -> interner.lookup -> match "int" -> Idx::INT`, when the parser already knows it's `ParsedType::Primitive(TypeId::INT)` and that maps to `Idx::from_raw(0)` in one operation

For unannotated functions, the same path just calls `checker.pool_mut().fresh_var()` -- a single counter increment.

**Files to profile:**
- `compiler/ori_types/src/check/signatures/mod.rs:178-200` (resolve loop for params + return)
- `compiler/ori_types/src/check/object_safety.rs:36-105` (redundant tree walk)
- `compiler/ori_types/src/check/signatures/mod.rs:359-515` (`resolve_type_with_vars` -- the full recursive resolution)

**Hypothesis 2: Bidirectional checking overhead in body pass.** When a function has annotations, `check_function()` (bodies/mod.rs:93-143) creates an `Expected` struct and calls `check_expr()` instead of `infer_expr()`. The `check_expr` path:
- Constructs `Expected { ty, origin: ExpectedOrigin::Context { span, kind } }` -- allocates variant data
- Calls `engine.resolve(expected.ty)` to follow links on the expected type
- After inference, calls `engine.check_type(inferred, &expected, span)` which does another `unify()` + potential error construction

For unannotated functions, `infer_expr()` is called directly. The body type naturally flows through without any extra unification step.

**Files to profile:**
- `compiler/ori_types/src/check/bodies/mod.rs:122-132` (Expected construction + check_expr call)
- `compiler/ori_types/src/infer/expr/mod.rs:262-347` (check_expr overhead vs infer_expr)
- `compiler/ori_types/src/infer/mod.rs:615-629` (check_type path with error construction)

**Hypothesis 3: Per-parameter object safety walks.** `check_parsed_type_object_safety` walks the annotation tree for every parameter and return type. For a function with 5 annotated parameters, this is 6 tree walks (5 params + 1 return). The object safety check does a `BTreeMap` lookup (`traits_by_name`) + `Vec` scan (`object_safety_violations`) per named type. For simple primitives this is wasted work -- they can never be trait objects.

**Hypothesis 4: String interner overhead.** `resolve_type_with_vars` calls `checker.interner().lookup(name)` (a hash map get + dereference) for every `ParsedType::Named` node, then matches the resulting `&str` against a table of 11+ primitive names. This happens in the signature phase (Pass 1) and again in the inference phase (`resolve_parsed_type`). Fresh variables avoid all of this.

### Quick Wins (Implement First)

1. **Skip object safety check for primitives and simple types** (Inspired by: TypeScript flag gating)
   - In `check_parsed_type_object_safety()`, add an early return for `ParsedType::Primitive`, `ParsedType::List`, `ParsedType::Map`, `ParsedType::Tuple`, and `ParsedType::Function` -- these can never be non-object-safe trait objects.
   - Currently the function only exits early for leaf types at the bottom of the match. Moving compound types to early-return avoids recursion for common cases.
   - **Estimated impact**: 5-10% on annotated code (eliminates most object safety walks)
   - **Effort**: 30 minutes

2. **Cache `interner().lookup()` results in `resolve_type_with_vars`** (Inspired by: Zig intern pool -- resolve once, use everywhere)
   - The string match for primitive names (`"int"`, `"float"`, `"bool"`, etc.) happens on every call. Instead, pre-compute a `FxHashMap<Name, Idx>` of primitive name->Idx mappings once during `ModuleChecker::new()` and look up by `Name` (u32 comparison) instead of string.
   - This eliminates the hash map get + string comparison for every `ParsedType::Named` node.
   ```rust
   // In ModuleChecker::new():
   let mut primitive_names: FxHashMap<Name, Idx> = FxHashMap::default();
   for (s, idx) in [("int", Idx::INT), ("float", Idx::FLOAT), ("bool", Idx::BOOL),
                     ("str", Idx::STR), ("char", Idx::CHAR), ("byte", Idx::BYTE),
                     ("void", Idx::UNIT), ("()", Idx::UNIT), ("never", Idx::NEVER),
                     ("Never", Idx::NEVER), ("Duration", Idx::DURATION),
                     ("duration", Idx::DURATION), ("Size", Idx::SIZE),
                     ("size", Idx::SIZE), ("Ordering", Idx::ORDERING),
                     ("ordering", Idx::ORDERING)] {
       primitive_names.insert(interner.intern(s), idx);
   }
   ```
   Then in all three resolve functions, check `primitive_names.get(&name)` before calling `interner().lookup()`.
   - **Estimated impact**: 15-20% on annotated code (eliminates string comparison on hot path)
   - **Effort**: 1-2 hours

3. **Fuse object safety check with type resolution** (Inspired by: Gleam single-pass Hydrator)
   - Currently `check_parsed_type_object_safety()` and `resolve_type_with_vars()` BOTH walk the same `ParsedType` tree. Fuse them into a single walk: check object safety inline during resolution.
   ```rust
   fn resolve_type_with_vars_and_safety(
       checker: &mut ModuleChecker<'_>,
       parsed: &ParsedType,
       type_param_vars: &FxHashMap<Name, Idx>,
       arena: &ExprArena,
       span: Span,
   ) -> Idx {
       match parsed {
           ParsedType::Named { name, type_args } => {
               // Object safety check inline (only for Named, which is the only case that matters)
               if !type_args.is_empty() {
                   // ... well-known check + resolution (existing code)
               } else {
                   // Check object safety for bare named types
                   check_name_object_safety(checker, *name, span);
                   // ... rest of resolution
               }
           }
           // Other arms: just resolve, no object safety possible
           _ => { /* existing resolution code */ }
       }
   }
   ```
   - **Estimated impact**: 10-15% on annotated code (eliminates double tree walk)
   - **Effort**: 2-3 hours

4. **Avoid `Expected` allocation for simple bidirectional checking** (Inspired by: TypeScript lazy resolution)
   - In `check_function()` (bodies/mod.rs), the `Expected` struct is always constructed even when the return type is a simple primitive. For monomorphic return types (no `HAS_VAR` flag), skip `check_expr` and just `infer_expr` + `unify` directly:
   ```rust
   let body_ty = if !pool.flags(sig.return_type).contains(TypeFlags::HAS_VAR) {
       // Concrete return type: infer body, unify directly (skip Expected overhead)
       let inferred = infer_expr(&mut engine, arena, func.body);
       let _ = engine.unify_types(inferred, sig.return_type);
       inferred
   } else {
       // Polymorphic: use full bidirectional checking
       let expected = Expected { ty: sig.return_type, origin: ... };
       check_expr(&mut engine, arena, func.body, &expected, body_span)
   };
   ```
   - **Estimated impact**: 5-10% on annotated code with concrete return types
   - **Effort**: 1 hour

5. **VarState: make `Unbound` and `Link` Copy-friendly** (Inspired by: Zig compact representation)
   - Currently `VarState` is `Clone`-derived. Line 276 of `unify/mod.rs` clones it in the hot `unify_var_with` path. The `Unbound` variant holds `Option<Name>` (8 bytes + discriminant) and `Link` holds `Idx` (4 bytes). Both fit in 16 bytes. Derive `Copy` for VarState or restructure to avoid the clone:
   ```rust
   // Before (clone in hot path):
   let state = self.pool.var_state(var_id).clone();

   // After (destructure to extract needed data, no clone):
   let (is_unbound, rank, name) = match self.pool.var_state(var_id) {
       VarState::Unbound { rank, name, .. } => (true, *rank, *name),
       VarState::Link { target } => {
           let target = *target;
           return self.unify_with_context(target, other, context);
       }
       // ... other arms
   };
   ```
   - **Estimated impact**: 3-5% across all code (hot path in unification)
   - **Effort**: 30 minutes

### Medium-Term Optimizations

1. **Unify the three type resolution functions** (Inspired by: Gleam Hydrator -- single resolution path)
   - `resolve_parsed_type_simple` (registration), `resolve_type_with_vars` (signatures), and `resolve_parsed_type` (inference) duplicate the same `ParsedType -> Idx` logic with minor variations. Create a single `ResolveContext` trait with methods for the varying behavior:
   ```rust
   trait ResolveContext {
       fn lookup_type_param(&self, name: Name) -> Option<Idx>;
       fn lookup_self_type(&self) -> Option<Idx>;
       fn on_infer(&mut self) -> Idx;  // ERROR for registration, fresh_var for inference
       fn pool_mut(&mut self) -> &mut Pool;
       fn interner(&self) -> &StringInterner;
       fn primitive_name_cache(&self) -> &FxHashMap<Name, Idx>;
   }

   fn resolve_parsed_type<C: ResolveContext>(
       ctx: &mut C, arena: &ExprArena, parsed: &ParsedType
   ) -> Idx { /* single implementation */ }
   ```
   - **Estimated impact**: Cleaner code, eliminates 3-way drift, enables shared optimizations
   - **Effort**: 4-6 hours

2. **Cache registration results across Salsa invocations** (Inspired by: Rust query caching)
   - Currently `check_module_impl()` rebuilds `TypeRegistry`, `TraitRegistry`, and `MethodRegistry` from scratch for every file on every Salsa re-check. For a stable prelude (which doesn't change between edits), this is redundant work.
   - Solution: Make registry construction a separate Salsa query, or cache the prelude's registries in the session-scoped `PoolCache`.
   ```rust
   // New Salsa query:
   #[salsa::tracked]
   fn prelude_registries(db: &dyn Db) -> (TypeRegistry, TraitRegistry) {
       // Only recomputes when prelude source changes
       let prelude = parsed(db, prelude_file(db));
       build_registries(&prelude.module, &prelude.arena, interner(db))
   }
   ```
   - **Estimated impact**: 20-40% for files that import the prelude (avoids redundant registration of ~30 trait definitions + ~50 impls)
   - **Effort**: 1-2 days (Salsa plumbing)

3. **Pre-compute super-trait method tables** (Inspired by: Rust trait solver separation)
   - `TraitRegistry::collected_methods()` walks the super-trait DAG on every call, allocating a `VecDeque` + `FxHashSet` for cycle detection. Pre-compute and cache the flattened method table when a trait is registered:
   ```rust
   pub struct TraitEntry {
       // ... existing fields ...
       /// Pre-computed: all methods from this trait + all super-traits (flattened).
       /// Computed lazily on first access, then cached.
       collected_methods_cache: OnceCell<Vec<(Name, Idx, TraitMethodDef)>>,
   }
   ```
   - **Estimated impact**: 5-10% for code using trait hierarchies
   - **Effort**: 3-4 hours

4. **Replace `FxHashMap<ExprIndex, Idx>` with `Vec<Idx>` in InferEngine** (Inspired by: Zig dense arrays)
   - `InferEngine.expr_types` is a `FxHashMap<usize, Idx>`. Since `ExprIndex` is sequential starting from 0, a `Vec<Idx>` with pre-allocated capacity is strictly faster (no hashing, no collision handling, better cache locality). `ModuleChecker` already uses `Vec<Idx>` for `expr_types` -- the `InferEngine` should match.
   ```rust
   // In InferEngine:
   expr_types: Vec<Idx>,  // was FxHashMap<ExprIndex, Idx>

   pub fn store_type(&mut self, expr: ExprIndex, ty: Idx) {
       if expr >= self.expr_types.len() {
           self.expr_types.resize(expr + 1, Idx::ERROR);
       }
       self.expr_types[expr] = ty;
   }
   ```
   - **Estimated impact**: 5-8% across all code (expr_types is written/read for every expression)
   - **Effort**: 1-2 hours

5. **Eliminate capability set cloning in `create_engine()`** (Inspired by: Rust zero-copy patterns)
   - `ModuleChecker::create_engine()` and `create_engine_with_env()` both clone `current_capabilities` and `provided_capabilities` (`FxHashSet<Name>`) on every function body check. Pass by reference instead:
   ```rust
   // Change InferEngine to borrow capability sets:
   current_capabilities: &'pool FxHashSet<Name>,
   provided_capabilities: &'pool FxHashSet<Name>,
   ```
   - **Estimated impact**: 2-3% (eliminates HashSet clone per function)
   - **Effort**: 1 hour

### Long-Term Architecture

Ori's type checker should evolve toward **Rust-style granular caching with Zig-style compact interning**:

1. **Per-function Salsa queries.** Split `typed(db, file)` into `function_signature(db, file, func_name)` and `function_body_typed(db, file, func_name)`. When a function body changes but its signature doesn't, only that function is re-checked. This matches Zig's `AnalUnit` granularity and Rust's per-function query model.

2. **Persistent Pool across incremental checks.** Instead of creating a fresh `Pool::new()` per file check, maintain a session-scoped Pool that grows monotonically. New types are interned into the existing pool; stale types are never freed (they're just u32 indices, so the memory cost is negligible). This avoids re-interning the same primitive, container, and function types on every check cycle.

3. **Lazy trait resolution.** Instead of eagerly registering all trait impls and walking super-trait hierarchies during Pass 0c, resolve trait methods lazily on first access (like TypeScript's lazy property resolution). Store a `OnceCell<ResolvedMethods>` on each impl entry.

4. **Bidirectional type checking as the primary mode.** Currently `check_expr` is a wrapper around `infer_expr` + unify. For the expression-based architecture, true bidirectional checking (propagating expected types DOWN into expressions) would eliminate many unification steps. Block result types, if-else branches, and match arms could receive their expected type directly.

### Concrete Types & Interfaces

```rust
/// Pre-computed primitive name -> Idx cache.
/// Created once in ModuleChecker::new(), shared by all resolution functions.
struct PrimitiveNameCache {
    names: FxHashMap<Name, Idx>,  // "int" -> Idx::INT, etc.
}

impl PrimitiveNameCache {
    fn new(interner: &StringInterner) -> Self {
        let mut names = FxHashMap::with_capacity_and_hasher(20, Default::default());
        for (s, idx) in Self::PRIMITIVES {
            names.insert(interner.intern(s), idx);
        }
        Self { names }
    }

    const PRIMITIVES: &[(&str, Idx)] = &[
        ("int", Idx::INT), ("float", Idx::FLOAT), ("bool", Idx::BOOL),
        ("str", Idx::STR), ("char", Idx::CHAR), ("byte", Idx::BYTE),
        ("void", Idx::UNIT), ("()", Idx::UNIT), ("never", Idx::NEVER),
        ("Never", Idx::NEVER), ("Duration", Idx::DURATION),
        ("duration", Idx::DURATION), ("Size", Idx::SIZE),
        ("size", Idx::SIZE), ("Ordering", Idx::ORDERING),
        ("ordering", Idx::ORDERING),
    ];

    /// O(1) lookup by Name (u32 hash), no string comparison.
    fn resolve(&self, name: Name) -> Option<Idx> {
        self.names.get(&name).copied()
    }
}

/// Unified type resolution context (replaces 3 separate resolve functions).
trait ResolveContext {
    fn lookup_type_param(&self, name: Name) -> Option<Idx>;
    fn lookup_self_type(&self) -> Option<Idx>;
    fn on_infer(&mut self) -> Idx;
    fn pool_mut(&mut self) -> &mut Pool;
    fn primitive_cache(&self) -> &PrimitiveNameCache;
    fn interner(&self) -> &StringInterner;
}

/// Registration context: type params resolve to named types, Infer -> ERROR.
struct RegistrationContext<'a> {
    checker: &'a mut ModuleChecker<'a>,
    type_params: &'a [Name],
    self_type: Option<Idx>,
}

/// Signature context: type params resolve to fresh vars, Infer -> fresh_var.
struct SignatureContext<'a> {
    checker: &'a mut ModuleChecker<'a>,
    type_param_vars: &'a FxHashMap<Name, Idx>,
}

/// Inference context: full inference engine access.
struct InferContext<'a, 'pool> {
    engine: &'a mut InferEngine<'pool>,
}

/// Modified InferEngine: Vec instead of HashMap for expr_types,
/// borrowed capability sets instead of owned.
pub struct InferEngine<'pool> {
    unify: UnifyEngine<'pool>,
    env: TypeEnv,
    expr_types: Vec<Idx>,                          // was FxHashMap<ExprIndex, Idx>
    context_stack: Vec<ContextKind>,
    errors: Vec<TypeCheckError>,
    warnings: Vec<TypeCheckWarning>,
    interner: Option<&'pool StringInterner>,
    trait_registry: Option<&'pool TraitRegistry>,
    signatures: Option<&'pool FxHashMap<Name, FunctionSig>>,
    type_registry: Option<&'pool TypeRegistry>,
    self_type: Option<Idx>,
    impl_self_type: Option<Idx>,
    loop_break_types: Vec<Idx>,
    current_capabilities: &'pool FxHashSet<Name>,   // was owned FxHashSet
    provided_capabilities: &'pool FxHashSet<Name>,   // was owned FxHashSet
    pattern_resolutions: Vec<(PatternKey, PatternResolution)>,
    const_types: Option<&'pool FxHashMap<Name, Idx>>,
}

/// Modified ModuleChecker with primitive name cache.
pub struct ModuleChecker<'a> {
    // ... existing fields ...
    primitive_names: PrimitiveNameCache,  // NEW: pre-computed primitive resolution
}
```

## Benchmark Targets

Based on current baselines:

| Metric | Current | Target | Improvement |
|--------|---------|--------|-------------|
| Annotated 100 functions | 140 us | 60 us | 2.3x faster |
| Inferred 100 functions | 71 us | 55 us | 1.3x faster |
| Annotated vs inferred gap | 2x slower | 1.1x slower (annotations slightly faster) | Gap eliminated |
| Annotated single function | 31 us | 20 us | 1.5x faster |
| Inferred single function | 24 us | 18 us | 1.3x faster |
| 500 functions annotated | 1.31 ms | 700 us | 1.9x faster |
| 500 functions inferred | 874 us | 650 us | 1.3x faster |
| Salsa cache hit | 8.7 us | 8.7 us | (already fast) |

**Primary goal**: Eliminate the annotation slowdown. Annotated code should be AT MOST equal speed to inferred code, ideally 10-20% faster (annotations provide information that avoids inference work).

**Secondary goal**: 30-50% overall improvement through medium-term optimizations (registry caching, Vec expr_types).

## Implementation Roadmap

### Phase 1: Investigation & Quick Wins (1-2 days)
- [ ] **Instrument annotation path**: Add `tracing::trace!` spans around `resolve_type_with_vars`, `check_parsed_type_object_safety`, and `check_expr` in the body checker. Run with `ORI_LOG=ori_types=trace` on 100-function annotated benchmark to measure time distribution.
- [ ] **Implement PrimitiveNameCache**: Pre-compute `Name -> Idx` mapping in `ModuleChecker::new()`. Use in all three resolve functions. (Quick Win #2)
- [ ] **Fuse object safety with type resolution**: Inline the object safety check into `resolve_type_with_vars` and `resolve_parsed_type`. Remove the separate `check_parsed_type_object_safety` calls from signatures/mod.rs. (Quick Win #3)
- [ ] **Avoid VarState clone in unify_var_with**: Destructure to extract fields instead of cloning. (Quick Win #5)
- [ ] **Skip object safety for non-Named types**: Early return in `check_parsed_type_object_safety` for Primitive, List, Map, Tuple, Function. (Quick Win #1)
- [ ] **Benchmark**: Re-run annotation vs inference benchmarks. Target: annotation overhead < 20%.

### Phase 2: Core Optimizations (3-5 days)
- [ ] **Replace InferEngine.expr_types with Vec<Idx>**: Match ModuleChecker's pattern. Pre-allocate based on arena expression count.
- [ ] **Eliminate capability set cloning**: Change InferEngine to borrow capability sets from ModuleChecker.
- [ ] **Simplify bidirectional path for concrete return types**: Skip Expected construction when return type has no HAS_VAR flag. (Quick Win #4)
- [ ] **Unify the three resolve functions**: Create `ResolveContext` trait with single `resolve_parsed_type` implementation.
- [ ] **Pre-compute super-trait method tables**: Cache flattened method tables in `TraitEntry` using `OnceCell`.
- [ ] **Benchmark**: Target overall 40-50% improvement on annotated code, 20-30% on inferred code.

### Phase 3: Architecture (1-2 weeks)
- [ ] **Cache prelude registries**: Make prelude trait/type registration a separate Salsa query or session-cached operation.
- [ ] **Investigate per-function Salsa queries**: Prototype `function_signature(db, file, func_name)` to enable fine-grained incremental re-checking.
- [ ] **Persistent Pool**: Explore session-scoped Pool that survives across `typed()` invocations, avoiding re-interning of stable types.
- [ ] **True bidirectional checking**: Propagate expected types into block results, if-else branches, and match arms directly (not infer-then-check).

## References

### Ori Source Files Studied
- `compiler/ori_types/src/check/signatures/mod.rs` -- signature collection, `resolve_type_with_vars`
- `compiler/ori_types/src/check/bodies/mod.rs` -- function body checking, `check_function`
- `compiler/ori_types/src/check/mod.rs` -- `ModuleChecker`, `create_engine`, capability cloning
- `compiler/ori_types/src/check/api/mod.rs` -- `check_module_impl`, pass orchestration
- `compiler/ori_types/src/check/registration/mod.rs` -- `resolve_parsed_type_simple`, registration passes
- `compiler/ori_types/src/check/object_safety.rs` -- `check_parsed_type_object_safety` (double walk)
- `compiler/ori_types/src/check/well_known.rs` -- `resolve_well_known_generic`
- `compiler/ori_types/src/infer/mod.rs` -- `InferEngine`, `check_type`
- `compiler/ori_types/src/infer/expr/mod.rs` -- `infer_expr`, `check_expr`
- `compiler/ori_types/src/infer/expr/type_resolution.rs` -- `resolve_parsed_type` (third resolver)
- `compiler/ori_types/src/infer/expr/identifiers.rs` -- `infer_ident`, constructor resolution
- `compiler/ori_types/src/infer/expr/blocks.rs` -- `infer_block`, `infer_let` (annotation path)
- `compiler/ori_types/src/infer/expr/structs.rs` -- struct field resolution (TypeEntry cloning)
- `compiler/ori_types/src/infer/env/mod.rs` -- `TypeEnv`, Rc-based scope chain
- `compiler/ori_types/src/unify/mod.rs` -- `UnifyEngine`, VarState clone on line 276
- `compiler/ori_types/src/pool/mod.rs` -- Pool layout, VarState definition
- `compiler/ori_types/src/registry/traits/mod.rs` -- TraitRegistry, BTreeMap for traits_by_name
- `compiler/oric/src/query/mod.rs` -- Salsa query pipeline

### Reference Compiler Files Consulted
- **Rust**: `rustc_middle/src/ty/context.rs` (TyCtxt arena), `rustc_middle/src/ty/mod.rs` (Ty pointer equality)
- **Zig**: `src/InternPool.zig` (unified type+value pool, u32 indices), `src/Sema.zig` (AnalUnit granularity)
- **Gleam**: `compiler-core/src/type_/hydrator.rs` (annotation resolution), `compiler-core/src/type_/expression.rs` (ExprTyper)
- **TypeScript**: `src/compiler/checker.ts` (TypeFlags, lazy resolution, contextual caches)
