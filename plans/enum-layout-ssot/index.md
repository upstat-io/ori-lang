---
reroute: true
name: "Enum SSOT"
full_name: "Enum Layout SSOT & Take-Project Hardening"
status: active
reviewed: false
order: 3
---

# Enum Layout SSOT & Take-Project Hardening Index

> **Maintenance Notice:** Update this index when adding/modifying sections.
> **Resolves:** 10 dual-source TPR findings from repr-opt §07 architectural review (2026-04-09)

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: EnumLayoutInfo API
**File:** `section-01-enum-layout-api.md` | **Status:** Not Started

```
EnumLayoutInfo, enum_layout_info, layout_info_for_enum
tag_encoding, EnumTag, Explicit, Niche, TaggedPtr, None
tag_gep_index, payload_gep_index, payload_field_offsets
abi_size, slot_count, i64-slot packing, div_ceil(8)
ori_repr, layout/mod.rs, enum_repr.rs, canonical/type_repr.rs
compute_enum_payload_layout, compute_explicit_tag_layout
MachineRepr::Enum, EnumRepr, VariantRepr
SSOT, canonical home, single source of truth
Phase C layout decision, post-narrowing
```

---

### Section 02: TagAccess Full Migration + Derive Fix
**File:** `section-02-tagaccess-migration.md` | **Status:** Not Started

```
TagAccess, TagEncoding, tag_access/mod.rs
extract_value, struct_gep, insert_value, hardcoded index
field 0, field 1, tag at index 0, payload at index 1
derive enum bodies, enum_eq.rs, enum_comparable.rs, enum_hashable.rs
option_result.rs, option_result_helpers.rs
result_monadic.rs, option_result_monadic.rs
compound_type_impls/option.rs, compound_type_impls/result.rs
list_builtins/helpers.rs, map_builtins.rs, debug_helpers.rs
build_option_struct, build_result_struct
resolve_type_for_option, resolve_type_for_result
TAGGED_PTR_CODEGEN_READY, active miscompile
88 consumer sites, 13 files, migration
```

---

### Section 03: Take-Project Hardening
**File:** `section-03-take-project-hardening.md` | **Status:** Not Started

```
take_project, TakeMoveFacts, in_class, var_to_lineage
lineage reconciliation, orphaned vars, predecessor args
compute_membership, compute_lineage, build_alias_graph
union-find, bidirectional, forward-only Jump
is_take_project, borrowed_defs.rs, Tag::Iterator
MachineRepr::UnmanagedPtr, unique-owned, ownership classification
debug_assert, invariant enforcement
dead_cleanup.rs, edge_cleanup.rs, bypass_safe_entry
membership vs lineage, phi contamination
```

---

### Section 04: Verification & Testing
**File:** `section-04-verification.md` | **Status:** Not Started

```
emitter-driven tests, IR emission verification
option_result_helpers/tests.rs, include_str!, source-text
emit_option_niche, emit_result_niche, synthetic emitter
codegen gate audit, TagAccess bypass detection
AOT tests, dual-exec parity, leak check
debug_assert, invariant verification
NICHE_CODEGEN_READY, TAGGED_PTR_CODEGEN_READY
test-all.sh, clippy-all.sh, valgrind
TPR findings verification, regression guard
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | EnumLayoutInfo API | `section-01-enum-layout-api.md` |
| 02 | TagAccess Full Migration + Derive Fix | `section-02-tagaccess-migration.md` |
| 03 | Take-Project Hardening | `section-03-take-project-hardening.md` |
| 04 | Verification & Testing | `section-04-verification.md` |
