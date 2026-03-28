# Section 19: Existential Types (impl Trait) -- Verification Results

**Verified**: 2026-03-28
**Methodology**: Searched codebase for `ImplTrait`, `OpaqueTy`, `opaque_type`, `impl Trait` in return position, existential type IR representations. Checked parser, type checker, evaluator, LLVM codegen, tests.
**Sections verified**: 19.1-19.5 + Completion Checklist
**Total items**: 26

## Summary

| Subsection | Items | Done | Partial | Not Started | Notes |
|-----------|-------|------|---------|-------------|-------|
| 19.1 Return Position impl Trait | 4 | 0 | 0 | 4 | No IR, parser, or typeck support |
| 19.2 Type Inference | 4 | 0 | 0 | 4 | No implementation |
| 19.3 Associated Type Constraints | 3 | 0 | 0 | 3 | No implementation |
| 19.4 Limitations and Errors | 4 | 0 | 0 | 4 | No implementation |
| 19.5 impl Trait vs dyn Trait | 3 | 0 | 0 | 3 | No implementation |
| Completion Checklist | 8 | 0 | 0 | 8 | N/A |

**Hidden implementations found**: 0

## Detailed Findings

### 19.1 Return Position impl Trait

All 4 items are [not-started].

- No `Type::ImplTrait` variant exists in `ori_ir`. Searched `ImplTrait`, `OpaqueTy`, `opaque_type` across all Rust source -- zero matches.
- No parser support for `impl Trait` in return type position. `ori_parse/src` has no impl-trait parsing code.
- No type checker support. `ori_types` has no opaque type inference or existential type handling.
- No test files in `tests/spec/types/impl_trait*` or `tests/compile-fail/types/impl_trait*`.

The only references to "impl Trait" in the Rust source are:
- `ori_parse/src/grammar/item/impl_def/mod.rs` -- parsing `impl Type: Trait` blocks (trait implementations, not existential types)
- `ori_ir/src/ast/items/traits.rs` -- trait definition IR
- `ori_types/src/registry/traits/mod.rs` -- trait registry
- `ori_fmt/tests/property_tests.rs` and `ori_fmt/src/declarations/def_impls.rs` -- formatting `impl` blocks
- `ori_types/src/check/well_known/trait_set.rs` -- well-known trait sets

None of these relate to existential return types (`-> impl Trait`).

### 19.2 Type Inference

All 4 items are [not-started]. No concrete type inference from function bodies, no return type unification for opaque types.

### 19.3 Associated Type Constraints

All 3 items are [not-started]. No `where Assoc == Type` constraint syntax for existential types.

### 19.4 Limitations and Errors

All 4 items are [not-started]. No position restrictions implemented (since the feature doesn't exist at all).

### 19.5 impl Trait vs dyn Trait

All 3 items are [not-started]. No documentation, comparison, or test infrastructure.

### Completion Checklist

All 8 items are [not-started].

## Accuracy Assessment

The section's `not-started` status is **accurate**. There is zero implementation of existential types (`impl Trait` in return position) anywhere in the codebase. No IR representation, no parsing, no type checking, no tests. This is a clean not-started section.

**Recommended status**: not-started
