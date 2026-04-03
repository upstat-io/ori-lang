---
plan: "imported-generic-mono"
title: "Imported Generic Monomorphization: Fix LLVM Test Runner"
status: in-progress
references:
  - "plans/bug-tracker/section-04-codegen-llvm.md"
---

# Imported Generic Monomorphization: Fix LLVM Test Runner

## Mission

Fix BUG-04-011: the LLVM JIT test runner silently skips monomorphized instances of imported generic functions (e.g., `assert_eq<int>` from `std.testing`), causing "unresolved function" errors at JIT execution. This blocks dual-execution parity for 282+ spec tests that use `std.testing` imports.

## Architecture

```
Test file: use std.testing { assert_eq }
           assert_eq(actual: 42, expected: 42)
                     │
   ┌─────────────────┼──────────────────────────────────┐
   │  TYPE CHECKER    │                                  │
   │  Records MonoInstance:                              │
   │    fn_name: assert_eq                               │
   │    generic_args: [Type(int)]                        │
   │    body_type_map: {X → int}  (test pool Idx keys)  │
   └──────────────────┬─────────────────────────────────┘
                      │
   ┌──────────────────┼─────────────────────────────────┐
   │  LLVM BACKEND (llvm_backend.rs)                    │
   │                                                     │
   │  Stage 4: Re-intern imported canon + sigs           │
   │    per_module_caches: source_idx → merged_idx       │
   │                                                     │
   │  Stage 5: Sig Collection                            │
   │    [BUG-A] Generic sigs SKIPPED (line 199)          │
   │    → mono instances can't find sig in either        │
   │      lower_and_infer_borrows or compile_all         │
   │                                                     │
   │  Stage 6: body_type_map keys from test pool (X)     │
   │    [BUG-B] but re-interned canon has merged vars (Y)│
   │    → X≠Y → substitution fails                      │
   └──────────────────┬─────────────────────────────────┘
                      │
   ┌──────────────────┼─────────────────────────────────┐
   │  FIX: Build imported MonoFunctions separately       │
   │                                                     │
   │  1. Re-intern imported generic sigs (new)           │
   │  2. Build fresh body_type_map from                  │
   │     per_module_cache values + scheme_var_ids         │
   │  3. Lower with imported canon in arc_lowering.rs    │
   │  4. Merge into mono_functions in compile.rs         │
   └────────────────────────────────────────────────────┘
```

## Design Principles

1. **Separate imported mono path** — Don't modify `collect_mono_functions` (works correctly for locals). Build imported mono functions separately in `llvm_backend.rs` where all module data (per_module_caches, re_interned_canons, imported pools) is available. This is the same principle used for imported non-generic functions: they have their own collection loop.

2. **Scope body_type_map to per_module_cache** — Avoid var_id collisions between test file and imported module by iterating ONLY per_module_cache values when building body_type_map. `substitute_in_pool` is keyed by var_id (u32), and re-interning preserves var_ids (`target.intern(Var, source.data(idx))`). Scoping to cached values prevents contamination.

3. **Key by local_name, not original_name** — `MonoInstance.fn_name` is the call-site identifier (the local/aliased name), because the type checker registers imported sigs under `local_name` (via `register_imported_function_as`). The `imported_generic_sigs` map must be keyed by `func_ref.local_name` to match MonoInstance lookups. The sig itself is found in the source module by `original_name`.

4. **Merge before codegen** — Both `lower_and_infer_borrows` (ARC lowering) and `compile_all_functions` (LLVM codegen) need the imported mono functions. They flow through the same declaration → preparation → emission pipeline as local mono functions.

## Section Dependency Graph

```
Section 01: JIT Imported Generic Mono  (standalone — no dependencies)
```

Single section — focused bug fix.

## Implementation Sequence

```
Phase 1 - Implementation
  └─ 01.1: Make mangle_mono_name public (monomorphize/mod.rs)
  └─ 01.2: Collect imported generic sigs + build ImportedMonoFunctions (llvm_backend.rs)
  └─ 01.3: Lower imported mono functions with correct canons + borrow inference (arc_lowering.rs)
  └─ 01.4: Accept imported mono functions in codegen (compile.rs + llvm_backend.rs)
  Gate: `timeout 30 cargo run -- test --backend=llvm /tmp/test.ori` with assert_eq passes

Phase 2 - Verification
  └─ 01.5: Cross-type matrix, negative pin, leak checks, LCFail reduction
  Gate: Full test suite passes, LCFail count decreases, zero leaks on matrix test
```

## Metrics (Current State)

| Crate | Production LOC | Test LOC | Total |
|-------|---------------|----------|-------|
| `oric` (test runner) | ~380 (llvm_backend) + ~300 (arc_lowering) | ~0 | ~680 |
| `ori_llvm` (monomorphize) | ~290 | ~100 | ~390 |
| `ori_llvm` (evaluator/compile) | ~400 | ~0 | ~400 |
| `ori_types` (re_intern) | ~260 | ~100 | ~360 |
| `ori_types` (substitute) | ~200 | ~170 | ~370 |
| **Total relevant** | **~1530** | **~370** | **~1900** |

## Estimated Effort

| Section | Est. Lines | Complexity | Depends On |
|---------|-----------|------------|------------|
| 01 JIT Imported Generic Mono | ~120 new | Medium | — |
|   ↳ 01.1 Public mangle_mono_name | ~1 | Low | — |
|   ↳ 01.2 Collect imported generic sigs | ~50 | Medium | 01.1 |
|   ↳ 01.3 Lower imported mono functions | ~25 | Medium | 01.2 |
|   ↳ 01.4 Codegen integration | ~15 | Low | 01.3 |
|   ↳ 01.5 Verification | ~65 test | Medium | 01.4 |
| **Total new** | **~120 + ~65 test** | | |

## Known Bugs (Pre-existing)

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| BUG-04-011 (JIT) | 3-layer pool identity mismatch | Section 01 | Not Started |

### AOT Path — Same Bug, Different Architecture (Not Included)

The AOT path (`codegen_pipeline.rs:112`) has the same underlying bug: `collect_mono_functions` only receives local `function_sigs`, so imported generic functions are silently skipped.

**Why it cannot be included in this plan:** The JIT path already has cross-module infrastructure (per_module_caches, imported_pools, re_interned_canons, imported_for_codegen) built for imported non-generic functions. This plan leverages that existing infrastructure. The AOT path (`compile_to_llvm` / `compile_to_llvm_with_imports`) has fundamentally different architecture:
- No `per_module_caches` — no type re-interning between modules
- No `re_interned_canons` — no cross-module canon remapping
- No `imported_pools` — no per-module pool management
- `import_sigs` is just `&[(Name, FunctionSig)]` — symbol declarations only, no function bodies

The AOT path compiles each module independently and links via symbol resolution. Fixing imported generics in AOT requires building the cross-module type re-interning infrastructure from scratch — a fundamentally different scope from "use the existing JIT infrastructure for mono functions."

**Concrete blocker**: AOT imported generic mono is blocked on building per-module pool + canon + cache infrastructure in `compile_to_llvm_with_imports`. This is architectural work, not a parameter-threading exercise. File as a separate bug via `/add-bug` if not already tracked in the bug tracker.

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | JIT Imported Generic Mono | `section-01-jit-imported-mono.md` | Not Started |
