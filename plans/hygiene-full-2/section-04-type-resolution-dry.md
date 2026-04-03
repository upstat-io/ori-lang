---
section: "04"
title: "Type Resolution DRY"
status: not-started
reviewed: false
goal: "Reduce 5 ParsedType resolution functions to a shared TypeResolver pattern — eliminate the most severe algorithmic duplication in the type checker"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Design TypeResolver Trait"
    status: not-started
  - id: "04.2"
    title: "Unify Well-Known Type Tables"
    status: not-started
  - id: "04.3"
    title: "Add Unit/Never TypeDefs to Registry"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Type Resolution DRY

**Status:** Not Started
**Goal:** The 5 ParsedType resolution functions (`resolve_parsed_type_simple`, `resolve_type_with_params`, `resolve_type_with_self_inner`, `resolve_and_check_type_with_vars`, `resolve_parsed_type`) share an identical match-on-variant structure. Extract the shared tree walk into a generic resolver parameterized by how to handle type variables, Self, and unknown names.

**Context:** This is the most severe algorithmic duplication in the type checker. All 5 functions walk the same `ParsedType` enum (Primitive, List, Map, Tuple, Function, Named, etc.) with identical recursive structure, differing only in: (a) how type parameters are resolved (fresh vars, lookup, error), (b) how Self is handled (fresh var, lookup, error), (c) whether to validate bounds. Additionally, dual string/Name well-known type tables create 2 more duplication sites.

---

## 04.1 Design TypeResolver Trait

**File(s):** `compiler/ori_types/src/check/registration/type_resolution.rs`, `compiler/ori_types/src/check/signatures/mod.rs`, `compiler/ori_types/src/infer/expr/type_resolution.rs`

<!-- reviewed: feasibility fix — the 5 resolution functions operate on two different context types (ModuleChecker vs InferEngine). A shared trait needs to abstract pool_mut(), interner(), resolve_well_known_generic_cached(), and resolve_registration_primitive(). The InferEngine version also handles variants the ModuleChecker ones don't (FixedList, Option, Result, Set, AssociatedType, ExistentialType, ConstGeneric). The trait abstraction is more complex than a simple config struct. -->

- [ ] Design a `TypeResolveContext` trait that abstracts:
  - `pool_mut() -> &mut Pool` — access to type pool (both ModuleChecker and InferEngine have this)
  - `interner() -> &StringInterner` — name interning
  - `resolve_well_known_generic(name: Name, args: &[Idx]) -> Option<Idx>` — well-known type lookup
  - `resolve_registration_primitive(name: Name) -> Option<Idx>` — primitive name resolution
- [ ] Design a `ResolveConfig` struct that parameterizes:
  - `resolve_type_param(name: Name) -> Idx` — how to handle type parameters
  - `resolve_self() -> Idx` — how to handle Self type
  - `resolve_unknown_name(name: Name) -> Idx` — fresh var vs Idx::ERROR
  - `check_bounds: bool` — whether to validate trait bounds
- [ ] Implement `resolve_parsed_type_with<C: TypeResolveContext>()` as the single canonical tree walk
- [ ] Handle the variant coverage gap: the InferEngine version handles FixedList, Option, Result, Set, AssociatedType, ExistentialType, ConstGeneric — these MUST be supported in the canonical function (likely as optional handlers in the config)
- [ ] Rewrite all 5 existing functions as thin wrappers that construct the appropriate config and call the canonical function
- [ ] Verify: the canonical function handles ALL `ParsedType` variants (Primitive, List, FixedList, Map, Tuple, Function, Named, Option, Result, Set, TraitBounds, SelfType, AssociatedType, ExistentialType, ConstGeneric, etc.)
- [ ] Verify: all existing type checker tests pass unchanged

---

## 04.2 Unify Well-Known Type Tables

**File(s):** `compiler/ori_types/src/check/well_known/mod.rs`

Two pairs of functions encode the same tables in both string and Name forms: `resolve_well_known_generic()` (string) / `WellKnownNames::resolve_generic()` (Name), and `is_concrete_named_type()` (string) / `WellKnownNames::is_concrete()` (Name).

- [ ] Make the string-based versions derive from the Name-based versions (or vice versa)
- [ ] If WellKnownNames is not always available (e.g., in isolated tests), provide a `from_str()` lookup that maps through interning rather than maintaining a parallel table
- [ ] Verify: adding a new well-known type requires updating exactly ONE location

---

## 04.3 Add Unit/Never TypeDefs to Registry

**File(s):** `compiler/ori_registry/src/defs/`, `compiler/ori_types/src/check/well_known/trait_set.rs`, `compiler/ori_types/src/infer/expr/registry_bridge/mod.rs`

Unit and Never have no TypeDef in ori_registry, forcing hardcoded trait satisfaction fallbacks in ori_types at 3+ locations.

- [ ] Add `TypeDef` for Unit in `ori_registry` with traits: Eq, Comparable, Hashable, Clone, Default, Debug, Printable
- [ ] Add `TypeDef` for Never in `ori_registry` with appropriate traits
- [ ] Remove hardcoded Unit trait satisfaction from `trait_set.rs:229-236`
- [ ] Remove hardcoded Unit/Never fallbacks from `registry_bridge/mod.rs:65-75`
- [ ] Verify: `registry_satisfies_trait()` correctly resolves Unit and Never traits via registry query

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] Single canonical `resolve_parsed_type_with()` function
- [ ] 5 existing resolution functions are thin wrappers
- [ ] Well-known type tables have single source (not dual string/Name)
- [ ] Unit and Never have TypeDefs in ori_registry
- [ ] No hardcoded trait satisfaction for Unit/Never in ori_types
- [ ] `timeout 150 cargo test -p ori_types` passes
- [ ] `timeout 150 cargo test -p ori_registry` passes
- [ ] `timeout 150 ./test-all.sh` passes
- [ ] `./clippy-all.sh` clean
- [ ] `/tpr-review` covering Section 04
- [ ] `/impl-hygiene-review last commit`
