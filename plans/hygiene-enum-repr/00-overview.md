---
plan: "hygiene-enum-repr"
title: "Enum Repr Hygiene: Exhaustive Implementation Plan"
status: not-started
references:
  - "plans/repr-opt/section-07-enum-repr.md"
---

# Enum Repr Hygiene: Exhaustive Implementation Plan

## Mission

Clean up 24 hygiene findings in `ori_repr` and `ori_llvm` enum representation code introduced by the recent `EnumTag` niche/tagless support (Section 07.2 of repr-opt). The findings span scattered knowledge (no canonical enum-repr query), duplicated dispatch (Unit/Never filtering repeated 3x), missing validation (`debug_assert!`s), a file over the 500-line limit, missing tests, dead branches, and documentation gaps. This plan consolidates all findings into one pass, executed in priority order: LEAKs first, then GAPs, DRIFT/BLOAT, minor findings, and documentation.

## Architecture

```
ori_repr                                    ori_llvm
┌───────────────────────┐     ┌──────────────────────────────────────────┐
│ enum_repr.rs           │     │ codegen/type_info/                       │
│  EnumTag (+ new preds) │     │  layout_resolver.rs (553→split)          │
│  EnumRepr              │     │  enum_layout.rs (NEW — extracted methods)│
│  VariantRepr           │     │  repr_lowering.rs                        │
│  min_tag_width()       │     │  mod.rs                                  │
├────────────────────────┤     ├──────────────────────────────────────────┤
│ canonical/type_repr.rs │     │ codegen/arc_emitter/                     │
│  canonical_enum()      │     │  tag_access/mod.rs                       │
│  + debug_assert!       │     │  (16 consumers — PLANNED in repr-opt)    │
├────────────────────────┤     └──────────────────────────────────────────┘
│ plan.rs                │
│  get_repr() → existing │
│  get_enum_repr() → NEW │
│  (canonical query)     │
├────────────────────────┤
│ layout/niche.rs        │
│  find_enum_niches()    │
│  optimize_option_repr()│
└────────────────────────┘
```

## Design Principles

1. **Single Source of Truth (SSOT)**: `EnumTag` semantics are currently scattered across `canonical_enum()`, `find_enum_niches()`, `tag_access/mod.rs`, and `layout_resolver.rs`. This plan introduces `ReprPlan::get_enum_repr()` as the canonical query and predicate methods on `EnumTag` so consumers ask the type instead of pattern-matching ad hoc.

2. **Extract before grow**: `layout_resolver.rs` is at 553 lines, over the 500-line limit. The four `resolve_enum*()` methods are the natural extraction boundary. Extracting them to `enum_layout.rs` also eliminates duplicated Unit/Never filtering by centralizing it in a helper.

## Section Dependency Graph

```
Section 01 (all findings)
  └── single section, executed in priority groups:
      Group 1: LEAK fixes (findings 1-6)
      Group 2: GAP fixes (findings 7-10)
      Group 3: DRIFT + BLOAT (findings 11-12)
      Group 4: EXPOSURE + WASTE + TYPE (findings 13-18)
      Group 5: Documentation (findings 19-24)
      Group 6: Cleanup (test-all, clippy-all, plan deletion)
```

This is a single-section plan. All 24 findings are in one section because they touch overlapping files and should land as one cohesive commit.

## Implementation Sequence

```
Phase 1 — LEAK fixes (findings 1-6)
  └─ 01.1: Add EnumTag predicate methods
  └─ 01.1: Add ReprPlan::get_enum_repr()
  └─ 01.1: Extract compute_tagless_enum_layout()
  └─ 01.1: Extract is_non_void_field() predicate helper
  └─ 01.1: Use niche_variant_idx directly (stop re-deriving)
  └─ 01.1: Add debug_assert! for EnumTag::None construction
  └─ 01.1: Narrow resolve_enum_tagless/niche params

Phase 2 — GAP fixes (findings 7-10)
  └─ 01.2: Mark finding 7 as PLANNED (repr-opt §07.2)
  └─ 01.2: Add test_canonical_single_variant_enum_is_tagless
  └─ 01.2: Add AOT integration test in ori_llvm/tests/aot/repr.rs
  └─ 01.2: Add tracing::warn! for repr_plan None fallback on enums

Phase 3 — DRIFT + BLOAT (findings 11-12)
  └─ 01.3: Merge identical dead branches
  └─ 01.3: Extract enum methods to type_info/enum_layout.rs

Phase 4 — Minor findings (findings 13-18)
  └─ 01.4: Add bounds check debug_assert for niche_variant_idx
  └─ 01.4: Add invariant comment for variant ordering
  └─ 01.4: Add doc comment for EnumTag construction scope
  └─ 01.4: Add capacity hint to Vec collect (Vec::with_capacity(2))
  └─ 01.4: Add doc comment for VariantRepr::is_pointer scope
  └─ 01.4: Move u32→usize cast to point of use

Phase 5 — Documentation (findings 19-24)
  └─ 01.5: Add spec citation placeholders
  └─ 01.5: Clarify ambiguous comment at line 140
  └─ 01.5: Note that unit tests are covered by finding 9
  └─ 01.5: Note §07.2 annotations acceptable during active plan
  └─ 01.5: Add min_tag_width/canonical_enum interaction doc
  └─ 01.5: Note type_repr.rs at 229 lines — monitor for growth

Phase 6 — Cleanup
  └─ 01.6: Run test-all.sh, clippy-all.sh
  └─ 01.6: Delete plans/hygiene-enum-repr/ directory
```

**Why this order:**
- LEAKs create drift risk if left unfixed. Fix them first.
- GAP tests validate the LEAK fixes (test what we just built).
- DRIFT/BLOAT cleanup depends on the extracted helpers from Phase 1.
- Minor findings and docs are independent polish.

## Metrics (Current State)

| File | Lines | Issue |
|------|-------|-------|
| `compiler/ori_llvm/src/codegen/type_info/layout_resolver.rs` | 553 | Over 500-line limit |
| `compiler/ori_repr/src/canonical/type_repr.rs` | 229 | Monitor for growth |
| `compiler/ori_repr/src/enum_repr.rs` | 106 | No `impl EnumTag` block |
| `compiler/ori_repr/src/layout/niche.rs` | 383 | OK |

## Estimated Effort

| Section | Est. Lines Changed | Complexity | Depends On |
|---------|-------------------|------------|------------|
| 01 Hygiene Fixes | ~200 new, ~150 moved | Low-Medium | — |
|   01.1 LEAK fixes | ~120 | Medium | — |
|   01.2 GAP fixes | ~60 | Low | 01.1 |
|   01.3 DRIFT + BLOAT | ~50 moved | Low | 01.1 |
|   01.4 Minor fixes | ~30 | Low | 01.3 |
|   01.5 Documentation | ~20 | Low | — |
|   01.6 Cleanup | 0 | Low | all |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Hygiene Fixes | `section-01-hygiene-fixes.md` | Not Started |
