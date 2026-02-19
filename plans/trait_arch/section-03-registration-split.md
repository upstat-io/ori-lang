---
section: "03"
title: Registration Module Split
status: complete
goal: Break the 1761-line registration/mod.rs into focused submodules under 500 lines each
sections:
  - id: "03.1"
    title: Module Inventory
    status: complete
  - id: "03.2"
    title: Split Plan
    status: complete
  - id: "03.3"
    title: Execution
    status: complete
  - id: "03.4"
    title: Completion Checklist
    status: complete
---

# Section 03: Registration Module Split

**Status:** Complete
**Goal:** Split `compiler/ori_types/src/check/registration/mod.rs` (1767 lines) into focused submodules, each under 500 lines, organized by registration pass.

**Previous state:** `registration/mod.rs` was the single most-touched file in the last 40 commits (14 touches). It handled builtin type registration, user type registration, trait/impl registration, derived trait processing, validation, type resolution, and constant registration — all in one file.

**Current state:** 8 focused submodules, each with a single clear responsibility. `mod.rs` is a 52-line orchestrator.

---

## 03.1 Module Inventory

The original plan assumed the file contained separate core trait, operator trait, and special trait registration sections. Inventory revealed the actual structure follows **registration passes** — builtin types → user types → traits → impls → derived → consts — with shared type resolution helpers used across passes.

| Lines | Pass | Content |
|-------|------|---------|
| 1-22 | Imports | Module docs + imports |
| 24-405 | Pass 0a: Builtin Types | Ordering, TraceEntry, Alignment, Sign, FormatType, FormatSpec |
| 407-563 | Pass 0b: User Types | struct/enum/newtype registration + collect_generic_params |
| 565-707 | Type Resolution | resolve_parsed_type_simple, resolve_field_type, convert_visibility |
| 709-1026 | Pass 0c: Traits | register_trait, object safety, method sig building |
| 1028-1425 | Pass 0c: Impls | register_impl, where clauses, Self-substitution |
| 1427-1724 | Pass 0d: Derived | register_derived_impl, validation, build_derived_methods |
| 1726-1767 | Pass 0e: Consts | register_const, infer_const_type |

- [x] Read the full file and map precise line boundaries for each section
- [x] Identify function-level dependencies (which functions call which)
- [x] Identify shared types/imports needed across sections

---

## 03.2 Split Plan

### Actual Structure (implemented)

```
compiler/ori_types/src/check/registration/
├── mod.rs              — Orchestrator + re-exports (52 lines)
├── type_resolution.rs  — Shared type resolution helpers (332 lines)
├── builtin_types.rs    — Ordering, TraceEntry, format types (388 lines)
├── user_types.rs       — Struct/enum/newtype registration (153 lines)
├── traits.rs           — Trait def + object safety (251 lines)
├── impls.rs            — Impl blocks + where clauses (329 lines)
├── derived.rs          — #derive processing + validation (305 lines)
├── consts.rs           — Constant registration (44 lines)
└── tests.rs            — All tests (already separate, 550 lines)
```

### Dependencies Between Submodules

```
mod.rs (orchestrator)
  ├── builtin_types    (self-contained)
  ├── user_types       → type_resolution
  ├── traits           → type_resolution
  ├── impls            → type_resolution
  ├── derived          → type_resolution
  └── consts           (self-contained)

type_resolution       (no sibling deps — only crate:: imports)
```

No circular dependencies. `type_resolution.rs` is the shared leaf; all other modules either are self-contained or depend only on `type_resolution`.

- [x] Design the exact split boundaries
- [x] Verify no circular dependencies exist
- [x] Determine shared types that need to be in `mod.rs` or `helpers.rs`

---

## 03.3 Execution

Split was executed in a single pass (all submodules created simultaneously) since the dependency structure was clear:

1. Created `type_resolution.rs` — shared helpers (collect_generic_params, resolve_parsed_type_simple, resolve_type_with_self, parsed_type_contains_self, convert_visibility)
2. Created `builtin_types.rs` — all register_*_type functions
3. Created `user_types.rs` — register_type_decl + user type registration
4. Created `traits.rs` — register_trait + object safety + method sig building
5. Created `impls.rs` — register_impl + build_impl_method + where constraints
6. Created `derived.rs` — register_derived_impl + validation + build_derived_methods
7. Created `consts.rs` — register_const + infer_const_type
8. Rewrote `mod.rs` — 52-line orchestrator with re-exports
9. Updated imports (`super::ModuleChecker` → `crate::ModuleChecker`)
10. Updated test imports (added explicit `ModuleChecker`, `ParsedType`, etc.)
11. Updated 4 cross-crate consistency tests in oric that scan source file paths

- [x] Phase 1: Extract helpers — `cargo t` passes
- [x] Phase 2a: Extract builtin types — `cargo t` passes
- [x] Phase 2b: Extract user types — `cargo t` passes
- [x] Phase 2c: Extract traits — `cargo t` passes
- [x] Phase 3a: Extract impls — `cargo t` passes
- [x] Phase 3b: Extract derived — `cargo t` passes
- [x] Phase 3c: Extract consts — `cargo t` passes
- [x] Phase 4: `./test-all.sh` passes, all files under 500 lines

---

## 03.4 Completion Checklist

- [x] `registration/mod.rs` is under 200 lines (52 lines — orchestrator + re-exports)
- [x] `registration/type_resolution.rs` is under 500 lines (332 lines)
- [x] `registration/builtin_types.rs` is under 500 lines (388 lines)
- [x] `registration/user_types.rs` is under 500 lines (153 lines)
- [x] `registration/traits.rs` is under 500 lines (251 lines)
- [x] `registration/impls.rs` is under 500 lines (329 lines)
- [x] `registration/derived.rs` is under 500 lines (305 lines)
- [x] `registration/consts.rs` is under 500 lines (44 lines)
- [x] No submodule exceeds 500 lines
- [x] No circular dependencies between submodules
- [x] All existing tests in `registration/tests.rs` pass unchanged
- [x] `./test-all.sh` passes with zero regressions (10,143 passed, 0 failed)
- [x] `./clippy-all.sh` passes

**Exit Criteria:** `registration/mod.rs` is a 52-line slim orchestrator. Each submodule has a single clear responsibility:
- New builtin type → `builtin_types.rs`
- New user type handling → `user_types.rs`
- New trait registration → `traits.rs`
- New impl registration → `impls.rs`
- New derived trait logic → `derived.rs`
- Type resolution change → `type_resolution.rs`
- New constant handling → `consts.rs`

No file hunting required.
