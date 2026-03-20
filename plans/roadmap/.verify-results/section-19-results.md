# Section 19: Existential Types (impl Trait) -- Verification Results

**Verified**: 2026-03-19
**Section status**: not-started (0/127 items)
**Verdict**: Section is genuinely not started. All items correctly marked `[ ]`.

---

## Spot-Checked Items (7 items)

### 19.1 -- Parser: Parse `impl Trait` in return position
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: Searched for `ImplTrait`, `impl_trait`, `OpaqueTy`, and `existential` across `ori_ir`, `ori_parse`, `ori_types`. No `ImplTrait` type variant exists in `ori_ir/src/parsed_type/mod.rs`. No parser code handles `impl` followed by a trait name in type position. The only `impl Trait` reference in `ori_parse` is in `impl_def/mod.rs` (parsing `impl Type: Trait` blocks, not `impl Trait` return types).
- **Classification**: VERIFIED -- no parser support for `impl Trait` in type position exists.

### 19.1 -- Type checker: Existential type handling
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: No opaque type tracking, no concrete type inference from function body for impl Trait, no multi-return-path unification. The type checker has no awareness of existential types.
- **Classification**: VERIFIED -- genuinely not started.

### 19.2 -- Type inference: Unify return types for impl Trait
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: No return path concrete type tracking for opaque types exists in `ori_types`.
- **Classification**: VERIFIED -- genuinely not started.

### 19.3 -- Associated type constraints (`where Item == int`)
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: The `where` clause parser handles type bounds (`T: Trait`) and const bounds (`N > 0`), but not associated type equality constraints (`where Item == int` on an `impl Trait` return). No `WhereClause::AssocTypeEquality` variant exists.
- **Classification**: VERIFIED -- genuinely not started.

### 19.4 -- Reject impl Trait in argument position
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: Since `impl Trait` is not parsed at all in type position, there is nothing to reject. The parser would treat `impl` as a keyword starting an impl block, not as part of a type expression.
- **Classification**: VERIFIED -- genuinely not started (no implementation means no position restrictions needed yet).

### 19.5 -- impl Trait vs dyn Trait comparison
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: No documentation or test exists. Trait objects (`dyn Trait` equivalent in Ori) exist as `Trait` in type position, but `impl Trait` (static dispatch alternative) is not implemented.
- **Classification**: VERIFIED -- genuinely not started.

### 19.1 -- Test: `tests/spec/types/impl_trait.ori`
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: No file at `tests/spec/types/impl_trait.ori` or `tests/spec/types/impl_trait*.ori`. Glob returned no results.
- **Classification**: VERIFIED -- no tests exist.

---

## Summary

| Classification | Count |
|----------------|-------|
| VERIFIED       | 7     |
| NEEDS TESTS    | 0     |
| WEAK TESTS     | 0     |
| WRONG TEST     | 0     |
| STALE TEST     | 0     |
| REGRESSION     | 0     |
| BUG FOUND      | 0     |

**Conclusion**: All 127 items are genuinely not started. No `impl Trait` support exists at any level:
- **IR**: No `ImplTrait` or `OpaqueType` variant in the type AST
- **Parser**: Cannot parse `impl Trait` in type positions
- **Type checker**: No opaque type inference or concrete type unification
- **Evaluator**: Would need no special handling (sees concrete types), but irrelevant without parser/typeck
- **LLVM**: No monomorphization for opaque return types
- **Tests**: No test files exist

Section status `not-started` is accurate. This is a Tier 7 feature with no implementation work begun.

---

## Cross-Section Notes

Sections 16-19 are all Tier 6-7 features representing future language capabilities. The existing codebase has some infrastructure scaffolding (well-known trait bits for Sendable, pattern stubs for parallel/timeout/spawn/channel, parser support for const generics and fixed-capacity list syntax), but no functional implementations of async, concurrency, or existential types. The roadmap accurately reflects this state.
