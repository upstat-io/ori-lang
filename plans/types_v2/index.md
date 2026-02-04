# Types 2.0 Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

Quick-reference keyword index for finding Types 2.0 implementation sections.

---

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file: `section-{ID}-*.md`

---

## Keyword Clusters by Section

### Section 01: Unified Pool Architecture
**File:** `section-01-pool.md` | **Status:** Not Started

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

### Section 02: Pre-Computed Metadata
**File:** `section-02-metadata.md` | **Status:** Not Started

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
**File:** `section-03-unification.md` | **Status:** Not Started

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
**File:** `section-04-ranks.md` | **Status:** Not Started

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
**File:** `section-05-errors.md` | **Status:** Not Started

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
**File:** `section-06-inference.md` | **Status:** Not Started

```
InferEngine, infer, inference
infer_expr, type inference
bidirectional, check, Expected
expression typing, bottom-up
HM, Hindley-Milner
```

---

### Section 07: Registries
**File:** `section-07-registries.md` | **Status:** Not Started

```
TypeRegistry, TraitRegistry
struct, enum, variant
trait, impl, method lookup
MethodRegistry, builtin methods
coherence, orphan rules
```

---

### Section 08: Salsa Integration
**File:** `section-08-salsa.md` | **Status:** Not Started

```
Salsa, incremental, query
Clone, Eq, Hash, Debug
deterministic, pure
type_check query, TypedModule
oric integration
```

---

### Section 09: Migration
**File:** `section-09-migration.md` | **Status:** Not Started

```
DELETE, remove, replace
ori_types, ori_typeck
ori_eval, ori_patterns
ori_llvm, oric
dependent crates, imports
```

---

## Quick Reference

| ID | Title | File | Priority | Status |
|----|-------|------|----------|--------|
| 01 | Unified Pool Architecture | `section-01-pool.md` | P0 | Not Started |
| 02 | Pre-Computed Metadata | `section-02-metadata.md` | P0 | Not Started |
| 03 | Unification Engine | `section-03-unification.md` | P0 | Not Started |
| 04 | Rank-Based Generalization | `section-04-ranks.md` | P1 | Not Started |
| 05 | Context-Aware Errors | `section-05-errors.md` | P1 | Not Started |
| 06 | Type Inference Engine | `section-06-inference.md` | P1 | Not Started |
| 07 | Registries | `section-07-registries.md` | P2 | Not Started |
| 08 | Salsa Integration | `section-08-salsa.md` | P2 | Not Started |
| 09 | Migration | `section-09-migration.md` | P3 | Not Started |

---

## Cross-References

| Related Plan | Relevance |
|--------------|-----------|
| `plans/roadmap/section-02-types.md` | Type system roadmap |
| `plans/parser_v2/` | AST representation consumed by type checker |
| `plans/ori_lsp/` | LSP depends on type checking |

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
