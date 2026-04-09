---
plan: "enum-layout-ssot"
title: "Enum Layout SSOT & Take-Project Hardening: Exhaustive Implementation Plan"
status: not-started
reviewed: false
supersedes: []
references:
  - "plans/repr-opt/section-07-enum-repr.md (resolves 10 TPR findings)"
  - "plans/repr-opt/00-overview.md (parent plan architecture)"
  - ".claude/rules/impl-hygiene.md (SSOT, No Side Logic, canonical homes)"
  - ".claude/rules/arc.md (AIMS pipeline rules)"
---

# Enum Layout SSOT & Take-Project Hardening: Exhaustive Implementation Plan

## Mission

Centralize ALL enum layout knowledge into a single source of truth in `ori_repr`, migrate every codegen consumer to use that SSOT instead of hardcoded layout assumptions, harden the AIMS take-project ownership model against lineage gaps and iterator-only scope limitations, and replace weak source-text-only niche tests with emitter-driven IR verification — resolving 10 confirmed dual-source TPR findings from the repr-opt §07 architectural review and making the enum representation architecture sound for all current AND future layout optimizations (niche filling, payload compression, §08-§12 work).

**Why this matters:** The current codebase has enum layout knowledge scattered across 88 consumer sites in 13+ files. The i64-slot packing rule alone is duplicated in 8+ locations. Derive enum bodies (Eq, Comparable, Hashable) assume `{tag, payload}` struct layout and **actively miscompile** tagged-pointer enums that qualify TODAY (TAGGED_PTR_CODEGEN_READY is true). The AIMS take-project system has an unenforced invariant (`in_class ⊆ var_to_lineage.keys()`) that can produce memory leaks for predecessor block variables. These aren't future risks — they are latent bugs in the current compiler.

**Relationship to repr-opt:** This plan is a **prerequisite reroute** blocking repr-opt §07.4+ (Payload Compression) and §08-§12. Without centralized layout knowledge, every future layout optimization (natural-alignment packing, escape analysis, ARC header compression) would have to update 88+ scattered sites — guaranteed drift. This plan pays down the debt BEFORE more optimization work compounds it.

## Mission Success Criteria

- [ ] Zero `div_ceil(8) * 8` (or equivalent i64-slot packing) computations outside `ori_repr/src/layout/` — verified by `rg 'div_ceil\(8\)' compiler/ --glob '!**/ori_repr/**' --glob '!**/tests*'` returning zero matches
- [ ] Zero hardcoded enum field indices (`extract_value(..., 0/1, ...)` for enum tag/payload) outside `tag_access/` — verified by grep audit
- [ ] Derive enum bodies (enum_eq.rs, enum_comparable.rs, enum_hashable.rs) correctly handle ALL `EnumTag` variants: `Explicit`, `Niche`, `TaggedPtr`, `None` — verified by AOT tests with each encoding
- [ ] `is_take_project` predicate driven by `MachineRepr` ownership classification, not tag whitelist — automatically covers any future `UnmanagedPtr` unique-owned type
- [ ] `debug_assert!` enforces `in_class ⊆ var_to_lineage.keys()` — no orphaned variables possible
- [ ] Emitter-driven IR tests for niche helpers replace source-text-only tests — verified by `cargo test -p ori_llvm option_result_helpers` exercising actual LLVM IR emission
- [ ] `./test-all.sh` green — no regressions from any migration
- [ ] All section success criteria met

## Architecture

```
                  ┌─────────────────────────────────────┐
                  │   ori_repr (SSOT for enum layout)   │
                  │                                      │
                  │  EnumLayoutInfo:                     │
                  │    tag_encoding: EnumTag              │
                  │    tag_gep_index: u32                 │
                  │    payload_gep_index: u32             │
                  │    payload_field_offsets: Vec<u32>    │
                  │    abi_size: u64                      │
                  │    slot_count(field_bytes) -> u64     │
                  │                                      │
                  │  Canonical home for ALL layout facts  │
                  │  Phase C decision (post-narrowing)    │
                  └──────────────┬──────────────────────┘
                                 │
              ┌──────────────────┼──────────────────┐
              ▼                  ▼                   ▼
   ┌──────────────────┐ ┌──────────────┐ ┌──────────────────┐
   │  ori_llvm         │ │  ori_arc      │ │  ori_llvm         │
   │  TagAccess        │ │  take_project │ │  derive_codegen   │
   │  (LLVM adapter)   │ │  (ownership)  │ │  (enum bodies)    │
   │                    │ │              │ │                    │
   │  Reads layout     │ │  MachineRepr │ │  Uses TagAccess    │
   │  from ori_repr    │ │  -driven     │ │  for ALL encodings │
   │  Emits LLVM IR    │ │  classifier  │ │  Explicit/Niche/   │
   │  via tag helpers   │ │              │ │  TaggedPtr/None    │
   └──────────────────┘ └──────────────┘ └──────────────────┘
              ▲                                      ▲
              │                                      │
   ┌──────────────────────────────────────────────────────┐
   │  Consumers (builtins, monadic helpers, etc.)          │
   │  ALL access enum layout ONLY through TagAccess        │
   │  ZERO hardcoded field indices                         │
   └──────────────────────────────────────────────────────┘
```

### Key Design Decisions

1. **`ori_repr` is the SSOT for enum layout** — NOT the registry. The registry stores static behavioral metadata (methods, operators). Layout is a Phase C computed decision depending on narrowed field types, machine properties, and global state. Both Codex and Gemini confirmed this independently.

2. **`TagAccess` becomes a thin LLVM adapter** — reads layout facts from `ori_repr`'s `EnumLayoutInfo`, emits LLVM IR. It does NOT compute layout; it only translates layout queries into LLVM builder calls.

3. **Derive enum bodies must handle ALL `EnumTag` variants** — this is an active miscompile fix, not future-proofing. `TAGGED_PTR_CODEGEN_READY` is true today. User-defined enums can qualify.

4. **Take-project lineage reconciliation via separate pass** — NOT backward edges in `build_alias_graph()` (which would contaminate singleton lineages via phi→arg backward paths). A dedicated reconciliation pass in `compute_lineage()` assigns default lineage to orphaned in_class vars.

5. **`is_take_project` driven by `MachineRepr::UnmanagedPtr` ownership** — any `UnmanagedPtr` that represents unique ownership automatically qualifies, not just iterators. Future types (Box<T>, channels) are covered without predicate changes.

6. **Niche stub implementation is OUT OF SCOPE** — deferred to repr-opt §07.2's NICHE_CODEGEN_READY gate flip. This plan fixes the weak TESTS (finding 9) but does not implement the stubs (finding 6). Confirmed by dual-source consensus.

## Section Dependency Graph

```
§01 EnumLayoutInfo API
  │
  ├──→ §02 TagAccess Full Migration + Derive Fix (depends §01)
  │
  └──→ §03 Take-Project Hardening (independent of §01/§02)
  
§02 + §03 ──→ §04 Verification & Testing (depends §02, §03)
```

- **§01** must come before **§02** — TagAccess must consume `EnumLayoutInfo` from the SSOT, not compute layout itself
- **§03** is independent of §01/§02 — take-project hardening doesn't touch enum layout
- **§04** depends on both §02 and §03 — verification covers all changes

## Implementation Sequence

```
Phase 1 — Foundation
  └─ §01: Create EnumLayoutInfo API in ori_repr
     Gate: all current layout queries expressible through the new API

Phase 2 — Migration (§02 and §03 can partially overlap)
  ├─ §02: Migrate ALL 88 consumer sites to use TagAccess + EnumLayoutInfo
  │   Sub-phases: 02.1 derive fix (URGENT), 02.2 builtins, 02.3 packing
  └─ §03: Harden take-project (lineage reconciliation + is_take_project generalization)
  Gate: zero hardcoded layout access, debug_assert enforced

Phase 3 — Verification
  └─ §04: Emitter-driven niche tests, codegen gate audit, integration testing
  Gate: all mission success criteria met, TPR clean, hygiene clean
```

## Cross-Section Interactions

- **§01 + §02**: §01 defines `EnumLayoutInfo`; §02 migrates consumers to query it via TagAccess. If §01's API doesn't answer a consumer's question, §02 discovers this and §01 is extended.
- **§01 + repr-opt §07.4**: Once `EnumLayoutInfo` owns the packing rule, §07.4's payload compression only needs to update ONE function instead of 8+.
- **§02 + repr-opt §07.2**: After derive enum bodies handle all `EnumTag` variants, the NICHE_CODEGEN_READY gate flip becomes safer (fewer consumers to audit).
- **§03 + AIMS pipeline**: The lineage reconciliation pass runs in `take_project::analyze()` — same phase, no pipeline ordering change.

## Estimated Effort

| Section | Est. Lines | Files | Complexity |
|---------|-----------|-------|-----------|
| 01 EnumLayoutInfo API | ~200 | 3-4 | Medium |
| 02 TagAccess Full Migration | ~800 | 13+ | Medium-High |
| 03 Take-Project Hardening | ~300 | 4-5 | High |
| 04 Verification & Testing | ~400 | 5-6 | Medium |
| **Total** | **~1,700** | **~20** | |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | EnumLayoutInfo API | `section-01-enum-layout-api.md` | Not Started |
| 02 | TagAccess Full Migration + Derive Fix | `section-02-tagaccess-migration.md` | Not Started |
| 03 | Take-Project Hardening | `section-03-take-project-hardening.md` | Not Started |
| 04 | Verification & Testing | `section-04-verification.md` | Not Started |

## Known Bugs (from research)

1. **Active miscompile**: Derive enum bodies (Eq, Comparable, Hashable) produce wrong code for tagged-pointer enums. Eq returns false on extraction failure; Comparable returns Equal; Hashable panics. User-defined enums can qualify today. → §02.1
2. **Potential memory leak**: Take-project predecessor args in_class without lineage → no RC decrement. → §03
3. **Weak tests**: Niche-helper tests use include_str! source-text matching, not IR emission. → §04
