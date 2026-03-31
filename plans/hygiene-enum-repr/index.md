# Hygiene Enum Repr Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Hygiene Fixes
**File:** `section-01-hygiene-fixes.md` | **Status:** Not Started

```
EnumTag, EnumRepr, EnumTag::None, EnumTag::Niche, EnumTag::Explicit
is_niche, is_tagless, needs_tag_field, payload_gep_index
get_enum_repr, ReprPlan, canonical_enum, compute_tagless_enum_layout
is_non_void_field, Unit, Never, filter, predicate
niche_variant_idx, resolve_enum_niche, resolve_enum_tagless
layout_resolver.rs, type_repr.rs, enum_repr.rs, niche.rs
enum_layout.rs, extract, file split, 500-line limit
debug_assert, validation, bounds check, invariant
SmallVec, unnecessary collect, dead branch, identical branch
tracing::warn, fallback, silent, error handling
doc comment, spec citation, plan annotation, comment mismatch
test_canonical_single_variant_enum_is_tagless, AOT repr test
ori_repr, ori_llvm, codegen, type_info
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Hygiene Fixes | `section-01-hygiene-fixes.md` |
