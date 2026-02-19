---
section: "03"
title: Registration Module Split
status: not-started
goal: Break the 1761-line registration/mod.rs into focused submodules under 500 lines each
sections:
  - id: "03.1"
    title: Module Inventory
    status: not-started
  - id: "03.2"
    title: Split Plan
    status: not-started
  - id: "03.3"
    title: Execution
    status: not-started
  - id: "03.4"
    title: Completion Checklist
    status: not-started
---

# Section 03: Registration Module Split

**Status:** Not Started
**Goal:** Split `compiler/ori_types/src/check/registration/mod.rs` (1761 lines) into focused submodules, each under 500 lines, organized by logical function: trait definition registration, derived impl construction, operator trait registration, and validation.

**Current state:** `registration/mod.rs` is the single most-touched file in the last 40 commits (14 touches). It handles:
- Core trait registration (Eq, Clone, Hashable, Printable, Debug, Default, Comparable)
- Operator trait registration (Add, Sub, Mul, Div, FloorDiv, Rem, Neg, BitAnd, BitOr, BitXor, BitNot, Shl, Shr, Not)
- Built-in trait satisfaction for primitives and compound types
- Derived impl construction (`build_derived_methods()`)
- Derived impl validation (supertrait checks, sum type restrictions)
- Format-related type registration (FormatSpec, Alignment, Sign, FormatType)
- Special trait registration (Len, Into, Formattable, Traceable)

Every new trait feature goes into this one file, which is why it's been touched 14 times.

---

## 03.1 Module Inventory

Current logical sections within `registration/mod.rs` (approximate line ranges):

| Lines | Function | Topic |
|-------|----------|-------|
| 1-50 | Module docs + imports | Infrastructure |
| 51-200 | `register_traits()` | Orchestrator — calls all sub-registrations |
| 201-400 | Core trait definitions | Eq, Clone, Hashable, Printable, Debug, Default, Comparable |
| 401-600 | Operator trait definitions | Add through Not (14 traits) |
| 601-800 | Special trait definitions | Len, Into, Formattable, Traceable + FormatSpec types |
| 801-1000 | `register_derived_impls()` | Processes `#[derive(...)]` for user types |
| 1001-1200 | `build_derived_methods()` | Constructs method signatures for derived traits |
| 1201-1500 | Validation logic | Supertrait checks, field constraints, sum type restrictions |
| 1501-1761 | Helper functions | Signature construction, method registration helpers |

- [ ] Read the full file and map precise line boundaries for each section
- [ ] Identify function-level dependencies (which functions call which)
- [ ] Identify shared types/imports needed across sections

---

## 03.2 Split Plan

### Target Structure

```
compiler/ori_types/src/check/registration/
├── mod.rs               — Public API: register_traits(), re-exports (~150 lines)
├── core_traits.rs       — Eq, Clone, Hashable, Printable, Debug, Default, Comparable (~200 lines)
├── operator_traits.rs   — Add through Not, 14 operator traits (~200 lines)
├── special_traits.rs    — Len, Into, Formattable, Traceable + FormatSpec types (~200 lines)
├── derived.rs           — register_derived_impls(), build_derived_methods() (~300 lines)
├── validation.rs        — Supertrait checks, field constraints, sum type checks (~200 lines)
├── helpers.rs           — Shared signature construction, method registration (~150 lines)
└── tests.rs             — All tests (already separate)
```

### Module Responsibilities

**`mod.rs`** — Orchestrator only. Contains `register_traits()` which calls into submodules in order. Re-exports public types. No trait definitions.

**`core_traits.rs`** — Registration of the 7 core derivable traits. Each trait gets: trait definition with method signature, any associated types. Organized by trait, not by function.

**`operator_traits.rs`** — Registration of all operator traits (Add, Sub, Mul, Div, FloorDiv, Rem, Neg, BitAnd, BitOr, BitXor, BitNot, Shl, Shr, Not). These share a common pattern: binary `(Self, Self) -> Self` or unary `(Self) -> Self`.

**`special_traits.rs`** — Non-derivable traits with special behavior: Len, Into, Formattable, Traceable. Also FormatSpec, Alignment, Sign, FormatType enum registrations.

**`derived.rs`** — `register_derived_impls()` and `build_derived_methods()`. This is the code that processes `#[derive(Eq, Clone, ...)]` attributes and constructs method signatures using `DerivedTrait::shape()` (from Section 01).

**`validation.rs`** — All validation logic for derives: supertrait requirements, field trait satisfaction, sum type restrictions. Error construction for E2028, E2029, E2032, E2033.

**`helpers.rs`** — Shared utilities: signature construction helpers, method registration wrappers, trait definition builders.

### Dependencies Between Submodules

```
mod.rs (orchestrator)
  ├── core_traits.rs     (no deps on siblings)
  ├── operator_traits.rs (no deps on siblings)
  ├── special_traits.rs  (no deps on siblings)
  ├── derived.rs         (uses helpers.rs)
  └── validation.rs      (uses helpers.rs)

helpers.rs (shared utilities, no deps on siblings)
```

No circular dependencies. Each submodule imports from `crate::` (Pool, Idx, etc.) but not from siblings.

- [ ] Design the exact split boundaries
- [ ] Verify no circular dependencies exist
- [ ] Determine shared types that need to be in `mod.rs` or `helpers.rs`

---

## 03.3 Execution

### Step-by-Step

**Phase 1: Extract helpers first (lowest risk)**

1. Create `registration/helpers.rs`
2. Move shared utility functions (signature builders, method registration helpers)
3. Add `mod helpers;` and `use helpers::*;` to `mod.rs`
4. `cargo t -p ori_types` — passes
5. `./test-all.sh` — passes

**Phase 2: Extract trait groups (medium risk)**

6. Create `registration/core_traits.rs`
7. Move core trait registration functions
8. `cargo t -p ori_types` — passes

9. Create `registration/operator_traits.rs`
10. Move operator trait registration functions
11. `cargo t -p ori_types` — passes

12. Create `registration/special_traits.rs`
13. Move special trait registration functions
14. `cargo t -p ori_types` — passes

**Phase 3: Extract derived + validation (higher risk — most interconnected)**

15. Create `registration/validation.rs`
16. Move validation functions
17. `cargo t -p ori_types` — passes

18. Create `registration/derived.rs`
19. Move `register_derived_impls()` and `build_derived_methods()`
20. `cargo t -p ori_types` — passes

**Phase 4: Verify**

21. `./test-all.sh` — passes
22. Verify each file is under 500 lines
23. Verify `mod.rs` is under 200 lines (orchestrator only)

### Visibility Strategy

All submodule functions are `pub(super)` or `pub(crate)` depending on whether they're used outside the registration module:

- Functions called from `mod.rs` orchestrator: `pub(super)`
- Functions called from other `check/` modules: `pub(crate)`
- Helper utilities: `pub(super)`

- [ ] Phase 1: Extract helpers — `cargo t` passes
- [ ] Phase 2a: Extract core traits — `cargo t` passes
- [ ] Phase 2b: Extract operator traits — `cargo t` passes
- [ ] Phase 2c: Extract special traits — `cargo t` passes
- [ ] Phase 3a: Extract validation — `cargo t` passes
- [ ] Phase 3b: Extract derived — `cargo t` passes
- [ ] Phase 4: `./test-all.sh` passes, all files under 500 lines

---

## 03.4 Completion Checklist

- [ ] `registration/mod.rs` is under 200 lines (orchestrator + re-exports)
- [ ] `registration/core_traits.rs` is under 300 lines
- [ ] `registration/operator_traits.rs` is under 300 lines
- [ ] `registration/special_traits.rs` is under 300 lines
- [ ] `registration/derived.rs` is under 400 lines
- [ ] `registration/validation.rs` is under 300 lines
- [ ] `registration/helpers.rs` is under 200 lines
- [ ] No submodule exceeds 500 lines
- [ ] No circular dependencies between submodules
- [ ] All existing tests in `registration/tests.rs` pass unchanged
- [ ] `./test-all.sh` passes with zero regressions
- [ ] `./clippy-all.sh` passes

**Exit Criteria:** `registration/mod.rs` is a slim orchestrator. Each submodule has a single clear responsibility. Adding a new core trait goes in `core_traits.rs`. Adding a new operator trait goes in `operator_traits.rs`. Adding a new derived trait's validation goes in `validation.rs`. No file hunting required.
