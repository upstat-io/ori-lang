---
reroute: true
name: "Arch Best Practices"
full_name: "Compiler Architecture Best Practices"
status: active
order: 6
---

# Compiler Architecture Best Practices Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Foundation & Policy Rules
**File:** `section-01-foundation.md` | **Status:** Not Started

```
compile-time performance, perf gate, measurement evidence, benchmark
crash regression, ICE, tests/crashes, minimized crash
phase documentation, phase graph, job queue, Zig Compilation.zig
policy rules, enforcement, impl-hygiene.md, tests.md
```

---

### Section 02: AST/IR Immutability Contract
**File:** `section-02-immutability.md` | **Status:** Not Started

```
immutability, &ExprArena, &Pool, phase boundary
mutation, &mut, arena, in-place modification
TypeScript AST, Lean 4 IR, Rust TyCtxt interned
parse.md, impl-hygiene.md, canon.md
```

---

### Section 03: Type Solver Budget Infrastructure
**File:** `section-03-solver-budgets.md` | **Status:** Not Started

```
solver fuel, budget, depth limit, recursion limit
unification, substitute, instantiation, type inference
cycle detection, occurs check, nontermination, overflow
TypeScript instantiationCount, instantiationDepth
E2042, E2043, solver overflow, trait resolution limit
ori_types, InferEngine, UnifyEngine, Pool
```

---

### Section 04: Diagnostic Ordering & Suppression
**File:** `section-04-diagnostic-ordering.md` | **Status:** Not Started

```
diagnostic ordering, deterministic, source position, stable sort
dedup, deduplication, error_code, span, follow-on suppression
TyError, child span, cascading errors, poison type
DiagnosticQueue, emitter, terminal, JSON, SARIF
Gleam sort warnings, Rust DiagnosticBuilder, child-span suppression
ori_diagnostic, queue/mod.rs
```

---

### Section 05: Incremental Edit-Sequence Testing
**File:** `section-05-incremental-testing.md` | **Status:** Not Started

```
incremental, Salsa, cache invalidation, revision
edit sequence, multi-step, revision test, @revisions
rustc_clean, rustc_dirty, TypeScript incremental baseline
Zig revision files, hello.0.zig, hello.1.zig
ori_test_harness, revision/mod.rs, query/tests.rs
tests.md, salsa::Setter, CompilerDb
```

---

### Section 06: Cross-Target Codegen Verification
**File:** `section-06-cross-target.md` | **Status:** Not Started

```
cross-target, non-host, ABI, aarch64, arm64, x86_64
FileCheck, codegen_checks, --target, triple
sret, calling convention, c_char, signedness
Go codegen/README, Rust cross-compile test suite
ori_llvm, cross.rs, target_features.rs, codegen-rules.md
BUG-04-045, cross-compilation
```

---

### Section 07: TypeFolder Trait
**File:** `section-07-type-folder.md` | **Status:** Not Started

```
TypeFolder, type folding, substitution, transformation
pool/substitute, unify/substitute, substitute_in_pool
algorithmic duplication, LEAK, shared recursion skeleton
fold_var, fold_named, super_fold_with, Visitor
Rust TypeFolder<TyCtxt>, rustc_type_ir
ori_types, pool, unify, infer
```

---

### Section 08: Packed Symbol Representation
**File:** `section-08-packed-symbol.md` | **Status:** Not Started

```
Symbol, ModuleId, packed representation, O(1) equality
Name, u32, u64, module provenance, cross-module resolution
Roc Symbol = (ModuleId, IdentId), niche optimization
ori_ir, name/mod.rs, StringInterner
TypeRegistry, TraitRegistry, check/imports.rs
```

---

### Section 09: Layout Caching via Salsa Query
**File:** `section-09-layout-query.md` | **Status:** Not Started

```
layout_of, Salsa query, layout caching, memoization
ReprPlan, TypeLayoutResolver, RefCell<FxHashMap>
repr_size, repr_align, compute_field_layout
Rust TyCtxt::layout_of, layout computation
ori_repr, ori_llvm, layout_resolver.rs
imperative pass, demand-driven, query-based
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Foundation & Policy Rules | `section-01-foundation.md` |
| 02 | AST/IR Immutability Contract | `section-02-immutability.md` |
| 03 | Type Solver Budget Infrastructure | `section-03-solver-budgets.md` |
| 04 | Diagnostic Ordering & Suppression | `section-04-diagnostic-ordering.md` |
| 05 | Incremental Edit-Sequence Testing | `section-05-incremental-testing.md` |
| 06 | Cross-Target Codegen Verification | `section-06-cross-target.md` |
| 07 | TypeFolder Trait | `section-07-type-folder.md` |
| 08 | Packed Symbol Representation | `section-08-packed-symbol.md` |
| 09 | Layout Caching via Salsa Query | `section-09-layout-query.md` |
