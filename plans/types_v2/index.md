# Types 2.0 Index

> **STATUS:** ✅ **PLAN COMPLETE** — All sections implemented, migrated, and verified (2026-02-05)
>
> | Metric | Value |
> |--------|-------|
> | Production code | ~23,700 lines |
> | Tests passing | 8,490 (0 failures) |
> | Clippy | Clean |
> | Old system remnants | Zero |

---

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file: `section-{ID}-*.md`

---

## Keyword Clusters by Section

### Section 01: Unified Pool Architecture
**File:** `section-01-pool.md` | **Status:** ✅ Complete

```
Pool, TypePool, unified storage
Idx, index, u32 index, 32-bit
Tag, tag-driven, discriminant
Item, compact storage, 5-byte
intern, interning, deduplication
hash, stable hash, FxHashMap
Zig InternPool, Roc Subs
```

---

### Section 02: Pre-Computed Metadata (TypeFlags)
**File:** `section-02-metadata.md` | **Status:** ✅ Complete (deferred: `effects()` helper)

> **Note:** "Metadata" here means `TypeFlags` (type properties), NOT Parser V2's `ModuleExtra` (formatting trivia).

```
TypeFlags, flags, bitflags
pre-computed, cached, O(1)
HAS_VAR, HAS_ERROR, IS_RESOLVED
WithCachedTypeInfo, Rust pattern
traversal skip, optimization gate
category flags, presence flags
```

---

### Section 03: Unification Engine
**File:** `section-03-unification.md` | **Status:** ✅ Complete

```
unify, unification, union-find
VarState, Link, Unbound, Rigid
resolve, path compression
occurs check, infinite type
Gleam unification, link-based
substitution, substitute
```

---

### Section 04: Rank-Based Generalization
**File:** `section-04-ranks.md` | **Status:** ✅ Complete

```
Rank, rank system, scope depth
generalize, generalization
let-polymorphism, quantify
instantiate, instantiation
TypeScheme, forall, quantified
Elm ranks, Roc ranks
```

---

### Section 05: Context-Aware Errors
**File:** `section-05-errors.md` | **Status:** ✅ Complete

```
Expected, ExpectedOrigin
ContextKind, context tracking
TypeProblem, diff, diffing
suggestions, hints, fixes
Elm errors, error messages
IntFloat, MissingField, FieldTypo
```

---

### Section 06: Type Inference Engine
**File:** `section-06-inference.md` | **Status:** ✅ Complete (deferred: qualified idents, closure capture)

```
InferEngine, infer, inference
infer_expr, type inference
bidirectional, check, Expected
expression typing, bottom-up
HM, Hindley-Milner
match, pattern, for, loop
```

---

### Section 07: Registries
**File:** `section-07-registries.md` | **Status:** ✅ Complete (deferred: auto-deref, method ambiguity, caching)

```
TypeRegistry, TraitRegistry
struct, enum, variant
trait, impl, method lookup
MethodRegistry, builtin methods
coherence, orphan rules
```

---

### Section 08: Salsa Integration
**File:** `section-08-salsa.md` | **Status:** ✅ Complete

```
Salsa, incremental, query
Clone, Eq, Hash, Debug
deterministic, pure
typed_v2 query, TypeCheckResultV2
oric integration, typeck_v2
import registration, cross-arena
Pool sharing, per-module
error rendering bridge
```

---

### Section 08b: Module-Level Type Checker
**File:** `section-08b-module-checker.md` | **Status:** ✅ Complete

> **Cross-Reference:** `plans/roadmap/section-03-traits.md` — Trait features in current `ori_typeck`

```
ModuleChecker, check_module
registration passes, types, traits
function signatures, body checking
statement inference, let binding
scope management, capabilities
```

---

### Section 09: Migration
**File:** `section-09-migration.md` | **Status:** ✅ Complete (all 10 phases done)

```
DELETE, remove, replace
ori_types, ori_typeck
ori_eval, ori_patterns
ori_llvm, oric
dependent crates, imports
V2 pipeline, production, TypeCheckResultV2
error rendering, message, code, span
```

---

## Quick Reference

| ID | Title | File | Priority | Status |
|----|-------|------|----------|--------|
| 01 | Unified Pool Architecture | `section-01-pool.md` | P0 | ✅ Complete |
| 02 | Pre-Computed Metadata | `section-02-metadata.md` | P0 | ✅ Complete (deferred: `effects()` helper, optimization gate tests) |
| 03 | Unification Engine | `section-03-unification.md` | P0 | ✅ Complete |
| 04 | Rank-Based Generalization | `section-04-ranks.md` | P1 | ✅ Complete |
| 05 | Context-Aware Errors | `section-05-errors.md` | P1 | ✅ Complete |
| 06 | Type Inference Engine | `section-06-inference.md` | P1 | ✅ Complete (deferred: qualified idents, closure capture, `recurse` self) |
| 07 | Registries | `section-07-registries.md` | P2 | ✅ Complete (deferred: auto-deref, method ambiguity, caching) |
| 08 | Salsa Integration | `section-08-salsa.md` | P2 | ✅ Complete |
| 08b | Module-Level Type Checker | `section-08b-module-checker.md` | P1 | ✅ Complete ¹ |
| 09 | Migration | `section-09-migration.md` | P3 | ✅ Complete (8,490 tests passing, 0 failures) |

---

## Cross-References

| Related Plan | Relevance |
|--------------|-----------|
| `plans/roadmap/section-03-traits.md` | ¹ Trait features in current `ori_typeck` — Types V2 must re-implement |
| `plans/roadmap/section-02-type-inference.md` | Type inference roadmap (complete in `ori_typeck`) |
| `plans/parser_v2/` | AST representation consumed by type checker (✅ Complete, no changes needed) |
| `plans/parser_v2/section-06-metadata.md` | Parser's `ModuleExtra` — **unrelated** to Types' `TypeFlags` |
| `plans/ori_lsp/` | LSP depends on type checking |

### Parser V2 Independence

Types V2 does **not** require changes to Parser V2. The systems are decoupled:
- Parser produces AST → Type checker consumes AST
- Parser's "metadata" = `ModuleExtra` (formatting trivia)
- Types' "metadata" = `TypeFlags` (type properties)
- Different files, different purposes, no conflicts

### Roadmap Section 03 Relationship

Types V2 is a **parallel rewrite** of the type checker, not an extension:

| Aspect | Current (`ori_typeck`) | Types V2 (`ori_types`) |
|--------|------------------------|------------------------|
| **Location** | `compiler/ori_typeck/` | `compiler/ori_types/src/check/` |
| **Traits** | ✅ Working (Roadmap 3.0-3.6) | ❌ Stubbed (08b.3) |
| **Type Storage** | `TypeInterner` + `TypeId` | `Pool` + `Idx` |
| **Unification** | Basic | Path-compressed union-find |

Roadmap Section 03 items are implemented in `ori_typeck`. Section 08b.3 (Registration Passes)
must **re-implement** trait/impl support using Types V2 infrastructure — it is not blocked
by Roadmap Section 03.

---

## Performance Validation

### When to Benchmark

Types V2 is **not yet integrated** with the main compiler. Benchmarking is deferred until:

1. **Section 08 (Salsa Integration)** — Enables measuring real query overhead
2. **Section 09 (Migration)** — Enables A/B comparison with current type checker

After migration, use `/benchmark` to validate:

```bash
/benchmark short   # Quick sanity check after changes to:
                   # - Pool internals (Section 01)
                   # - Unification hot paths (Section 03)
                   # - Inference engine (Section 06)
```

**Skip benchmarks** for: error formatting (05), registries (07 — cold path).

### Future Baselines

| Metric | Current V1 | Target V2 | Notes |
|--------|------------|-----------|-------|
| Type check | TBD | 2x faster | Pool deduplication |
| Unification | TBD | 1.5x faster | Path compression |
| Memory | TBD | 50% less | Compact representation |

Baselines will be captured during Section 09 migration.

---

## Source Analysis

| Language | Files Analyzed | Key Patterns Adopted |
|----------|---------------|---------------------|
| **Zig** | `InternPool.zig` (13k lines) | Unified pool, tag+data, thread sharding |
| **Roc** | `types/`, `unify/`, `solve/` | SoA layout, ranks, lambda sets |
| **Rust** | `rustc_type_ir/`, `rustc_middle/` | TypeFlags, arena allocation, TypeFolder |
| **Gleam** | `type_/`, `exhaustiveness/` | Link-based unification, variant inference |
| **Elm** | `Type/`, `Reporting/Error/` | Rank system, context-aware errors |
| **TypeScript** | `checker.ts`, `types.ts` | Multi-level caches, relation caches |
| **Go** | `go/types/`, `types2/` | Minimal interface, Object separation |
