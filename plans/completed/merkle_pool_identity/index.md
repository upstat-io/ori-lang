---
reroute: true
name: "Merkle Pool"
full_name: "Merkle Pool Identity"
status: resolved
---

# Merkle Pool Identity — Plan Index

> **Maintenance Notice:** Update this index when adding/modifying sections.
> **Related:** `plans/type_strategy_registry/` (orthogonal — method/operator dispatch, not type identity)

## Overview

Per-module Pools are architecturally correct for Salsa's per-query memoization. But the current
hash function (`compute_hash`) hashes raw `Idx` values — which are pool-local sequential integers.
The same type in two different pools gets different hashes, making cross-module type identity
impossible without AST re-walking.

**Merkle hashing** replaces raw-Idx hashing with recursive child-hash hashing. Like Git's
content-addressed object model, each type's hash depends only on its *structure*, not its position
in any pool. `List<MyStruct>` gets the same hash whether `MyStruct` is at `Idx(30)` or `Idx(50)`.

This gives per-module pools the performance characteristics of a global pool:
- **O(1) cross-module type comparison** (compare `u64` hashes)
- **O(1) import resolution** (hash lookup in `intern_map`, not AST walk)
- **Zero re-interning for known types** (primitives + prelude types always hit)

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Merkle Hash Foundation
**File:** `section-01-merkle-hash-foundation.md` | **Status:** Complete

```
compute_hash, merkle_hash, FxHasher, content-addressed
Tag, tag.rs, data, extra, Item, child reference, structural data
intern, intern_complex, intern_map, hashes, pool/mod.rs
List, Option, Map, Result, Function, Tuple, Struct, Enum
Named, Applied, Alias, Var, BoundVar, RigidVar, Scheme
Projection, ModuleNs, Infer, SelfType, Borrowed
simple container, two-child, complex, type variable
child Idx, recursive hash, bottom-up, stable, universal
Tag::has_child_in_data, classify_extra_layout
FxHash, 64-bit, collision probability, birthday paradox
```

---

### Section 02: Hash Stability Testing
**File:** `section-02-hash-stability-testing.md` | **Status:** Complete

```
cross-pool, stability, identical hash, different pool
same type, different Idx, same hash
Pool::new, fresh pool, intern sequence
primitive stability, container stability, complex stability
generic types, type variables, schemes
collision detection, hash distribution, birthday bound
debug_assert, hash mismatch, diagnostic
format_merkle_hash, hash visualization
pool_hash_eq, structural_eq
```

---

### Section 03: Hash-Forwarded Signatures
**File:** `section-03-hash-forwarded-signatures.md` | **Status:** Complete

```
FunctionSig, param_hashes, return_hash, u64
TypeCheckResult, TypedModule, Salsa, memoized
Clone, Eq, PartialEq, Hash, Debug, derive
output/mod.rs, cross-module, transport
scheme_var_ids, type_params, const_params
register_imported_function, infer_function_signature_from
import_env, signatures, ModuleChecker
PoolCache, typed_pool, typed, query/mod.rs
```

---

### Section 04: Hash-First Import Resolution
**File:** `section-04-hash-first-import-resolution.md` | **Status:** Complete

```
register_imported_function, resolve_and_check_type_with_vars
hash lookup, intern_map, O(1), fallback, AST walk
foreign_arena, ExprArena, ParsedType
cache warming, prelude, common types
lazy population, amortized, first miss
import boundary, register_resolved_imports
typeck.rs, check/mod.rs, signatures/mod.rs
resolve_parsed_type, type_resolution.rs
```

---

### Section 05: Portable Type Descriptors
**File:** `section-05-portable-type-descriptors.md` | **Status:** Complete

```
TypeDescriptor, portable, pool-independent, self-contained
topological sort, leaves first, bottom-up reconstruction
zero-AST, no foreign arena, no ParsedType
FunctionSig, TypeCheckResult, embedded
Primitive, Container, TwoChild, Complex, Named
hash reference, child hash, Merkle tree
monomorphization, generic instantiation, explosion
reconstruct_from_descriptor, ensure_type_exists
```

---

### Section 06: Backend Integration
**File:** `section-06-backend-integration.md` | **Status:** Complete

```
ori_llvm, ImportedFunctionForCodegen, pool field
FunctionCompiler, declare_all, define_all_cached
ARC, borrow inference, ori_arc, cross-module
evaluator, ori_eval, JIT, test runner
two-pool problem, source pool, local pool
canon, CanonResult, canonical IR
ABI computation, param_names, indirect param
hash-based identity, eliminate pool dependency
```

---

### Section 07: Benchmarks & Exit Criteria
**File:** `section-07-benchmarks-exit-criteria.md` | **Status:** Complete

```
benchmark, import boundary, throughput
cross-module comparison, O(1), structural comparison
memory, duplication, pool size
perf-baseline.sh, criterion, cargo bench
regression, test-all.sh, llvm-test.sh
exit criteria, hash stability, no collisions
generics, effects, capabilities, depth, width
monomorphization, instantiation count
scalability, 100 modules, 1000 types
```

---

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Merkle Hash Foundation | `section-01-merkle-hash-foundation.md` | Complete |
| 02 | Hash Stability Testing | `section-02-hash-stability-testing.md` | Complete |
| 03 | Hash-Forwarded Signatures | `section-03-hash-forwarded-signatures.md` | Complete |
| 04 | Hash-First Import Resolution | `section-04-hash-first-import-resolution.md` | Complete |
| 05 | Portable Type Descriptors | `section-05-portable-type-descriptors.md` | Complete |
| 06 | Backend Integration | `section-06-backend-integration.md` | Complete |
| 07 | Benchmarks & Exit Criteria | `section-07-benchmarks-exit-criteria.md` | Complete |

---

## Dependency Graph

```
Section 01 (Merkle Hash)
    ↓
Section 02 (Stability Tests)  ←── must pass before any downstream work
    ↓
Section 03 (Hash-Forwarded Sigs)
    ↓
Section 04 (Hash-First Import)  ←── primary performance win
    ↓                    ↓
Section 05 (Descriptors) Section 06 (Backends)
         ↘             ↙
    Section 07 (Benchmarks & Exit)
```

Sections 05 and 06 can proceed in parallel after Section 04.
Section 05 is optional — Section 04's fallback-to-AST path is sufficient
for correctness and good performance. Section 05 is for maximum throughput
in large codebases with heavy monomorphization.
