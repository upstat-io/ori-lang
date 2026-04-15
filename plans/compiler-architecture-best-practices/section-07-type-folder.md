---
section: "07"
title: "TypeFolder Trait"
status: not-started
reviewed: false
goal: "Extract the shared type-transformation recursion skeleton from 4+ substitution implementations into a TypeFolder trait, eliminating algorithmic duplication (impl-hygiene LEAK)"
success_criteria:
  - "A TypeFolder trait exists in ori_types with fold_var, fold_named, fold_applied, etc. methods"
  - "substitute_in_pool() is reimplemented atop TypeFolder (same behavior, shared recursion)"
  - "UnifyEngine::substitute() is reimplemented atop TypeFolder"
  - "All 4+ substitution implementations use the shared skeleton — no parallel tag-dispatch matches"
  - "All existing tests pass unchanged (substitution behavior identical)"
  - "impl-hygiene.md §Aspirational Patterns updated to mark TypeFolder as 'implemented'"
inspired_by:
  - "Rust rustc_type_ir TypeFolder<TyCtxt> (compiler/rustc_type_ir/src/fold.rs)"
  - "Rust TypeSuperFoldable (super_fold_with handles recursion, consumers override interesting cases)"
  - "Koka src/Type/Operations.hs (type folding via Higher-order Abstract Syntax)"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "07.1"
    title: "Design TypeFolder Trait"
    status: not-started
  - id: "07.2"
    title: "Implement Default Recursion"
    status: not-started
  - id: "07.3"
    title: "Migrate Substitution Implementations"
    status: not-started
  - id: "07.4"
    title: "Cleanup & Documentation"
    status: not-started
  - id: "07.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "07.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 07: TypeFolder Trait

**Status:** Not Started
**Goal:** Extract the shared type-transformation recursion skeleton from 4+ substitution implementations into a `TypeFolder` trait. This is the aspirational pattern already documented in `impl-hygiene.md` §Aspirational Patterns — "A `TypeFolder` trait where consumers implement only the interesting cases and `super_fold_with()` handles recursion."

**Success Criteria:**
- [ ] `TypeFolder` trait exists — satisfies mission criterion "TypeFolder eliminates 4+ duplications"
- [ ] All substitution impls use the shared skeleton — satisfies mission criterion "new transformations use fold_*"
- [ ] All tests pass unchanged — behavior is identical, only the code structure changes
- [ ] `impl-hygiene.md` updated to mark the aspirational pattern as implemented

**Context:** Research found 4+ substitution implementations with identical control-flow skeletons:
1. `pool/substitute/mod.rs` (360 lines): `substitute_in_pool()` — standalone on `&mut Pool` for monomorphization
2. `unify/substitute.rs` (270 lines): `UnifyEngine::substitute()` — scheme instantiation
3. `infer/expr/control_flow.rs:473,555`: `substitute_type_params()` / `substitute_type_params_with_map()` — multi-clause function type params
4. `infer/expr/structs/mod.rs:312`: `substitute_named_types()` — struct context
5. `pool/substitute/mod.rs`: `extract_var_from_types()` — parallel-walk extraction (same skeleton)

All share: HAS_VAR flag fast-path → Tag match → recurse on children per tag layout → handle Var/BoundVar specially. The skeleton is ~50 lines of tag dispatch; only the leaf cases differ.

**Reference implementations:**
- **Rust** `rustc_type_ir/src/fold.rs`: `TypeFolder<I: Interner>` trait with `fold_ty()`, `fold_region()`, `fold_const()`. Default implementations call `super_fold_with()` which handles recursion. Consumers implement only the cases they care about.
- **Rust** `TypeSuperFoldable` trait: the "walk the children" implementation that the default `fold_*` methods delegate to.

**Depends on:** Section 01 (policy language). Independent of Sections 02-06.

---

## 07.1 Design TypeFolder Trait

**File(s):** `compiler/ori_types/src/pool/` (new file `fold.rs` or `fold/mod.rs`)

Design the `TypeFolder` trait interface based on the shared skeleton extracted from the 4+ implementations.

- [ ] Analyze the shared skeleton across all 4+ implementations:
  - Map the tag-dispatch arms that are identical vs. the leaf cases that differ
  - Identify the minimal set of `fold_*` methods consumers need to override
  - Identify what context the folder needs (pool access, substitution map, depth counter)

- [ ] Design the trait:
  ```rust
  /// Trait for recursive type-to-type transformations.
  /// Consumers implement only the interesting cases;
  /// `super_fold()` handles structural recursion.
  pub trait TypeFolder {
      /// The context type providing pool access and transformation state.
      type Context;
      
      /// Fold a type variable. Default: return unchanged.
      fn fold_var(&mut self, ctx: &mut Self::Context, idx: Idx, var_id: u32) -> Idx { idx }
      
      /// Fold a bound variable. Default: return unchanged.
      fn fold_bound_var(&mut self, ctx: &mut Self::Context, idx: Idx, var_id: u32) -> Idx { idx }
      
      /// Fold a named type reference. Default: recurse via super_fold.
      fn fold_named(&mut self, ctx: &mut Self::Context, idx: Idx) -> Idx { ... }
      
      // ... other interesting-case methods
  }
  
  /// Structural recursion: walk children of `idx`, calling `folder.fold_*()` on each.
  /// This is the shared skeleton that all 4+ implementations currently duplicate.
  pub fn super_fold<F: TypeFolder>(folder: &mut F, ctx: &mut F::Context, pool: &mut Pool, idx: Idx) -> Idx {
      // HAS_VAR fast-path (from TypeFlags)
      // Tag dispatch → recurse on children
      // This is the ~50 lines of shared skeleton
  }
  ```

- [ ] Validate the design against all 4+ implementations — every implementation must be expressible as a `TypeFolder` impl with only leaf-case overrides

- [ ] **Subsection close-out (07.1)** — MANDATORY before starting 07.2:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 07.2 Implement Default Recursion

**File(s):** `compiler/ori_types/src/pool/fold.rs` (or `fold/mod.rs`)

Implement the `super_fold()` function — the shared recursion skeleton.

- [ ] Write failing tests: create a test `TypeFolder` impl that counts folded types, fold a complex type tree, assert the count matches the expected traversal

- [ ] Implement `super_fold()`:
  - Extract the tag-dispatch logic from `substitute_in_pool()` (the most complete implementation)
  - Handle all tag categories: primitives (return unchanged), single-child containers (recurse on child), two-child containers, complex types (Function, Tuple, Struct, Enum), Named/Applied, variables (delegate to `fold_var`/`fold_bound_var`), schemes
  - Gate with `TypeFlags::HAS_VAR` fast-path (or make the gate configurable if some folders need to walk non-variable types)
  - Pass depth counter for the Section 03 substitution depth limit

- [ ] Add tests for edge cases: empty types, deeply nested types, circular references (should hit occurs check), types with no variables (fast-path)

- [ ] **Subsection close-out (07.2)** — MANDATORY before starting 07.3:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 07.3 Migrate Substitution Implementations

**File(s):** `compiler/ori_types/src/pool/substitute/mod.rs`, `compiler/ori_types/src/unify/substitute.rs`, `compiler/ori_types/src/infer/expr/control_flow.rs`, `compiler/ori_types/src/infer/expr/structs/mod.rs`

Rewrite each substitution implementation as a `TypeFolder` impl. Each migration must preserve exact behavior.

- [ ] Migrate `substitute_in_pool()` (pool/substitute/mod.rs):
  - Create `struct PoolSubstFolder { var_subst: &FxHashMap<u32, Idx> }`
  - Implement `TypeFolder` with `fold_var()` that does the var_id lookup
  - Replace the 360-line implementation with the folder call
  - Run all existing tests — behavior must be identical

- [ ] Migrate `UnifyEngine::substitute()` (unify/substitute.rs):
  - Create `struct UnifySubstFolder { ... }` with link-following logic in `fold_var()`
  - Replace the 270-line implementation
  - Run all existing tests

- [ ] Migrate `substitute_type_params()` and `substitute_type_params_with_map()` (infer/expr/control_flow.rs):
  - These are simpler — may become direct calls to the pool substitution folder with a different var map

- [ ] Migrate `substitute_named_types()` (infer/expr/structs/mod.rs):
  - Similar simplification

- [ ] Migrate `extract_var_from_types()` — this is a parallel-walk extraction, not a substitution. It may need a `TypeVisitor` (read-only fold) rather than a `TypeFolder` (mutating fold). If the skeleton is different enough, keep it separate but document why.

- [ ] Verify: `timeout 150 ./test-all.sh` — all tests pass, no behavioral changes

- [ ] **Subsection close-out (07.3)** — MANDATORY before starting 07.4:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 07.4 Cleanup & Documentation

**File(s):** `.claude/rules/impl-hygiene.md`, `.claude/rules/types.md`

- [ ] Update `impl-hygiene.md` §Aspirational Patterns → TypeFolder: change from "aspirational" to "implemented". Note the adoption path that was followed.

- [ ] Add `TypeFolder` to `types.md` §Key Files or as a new section — document the trait, its methods, and how to add new type transformations

- [ ] Delete dead code — the old substitution implementations that were replaced by folder impls. Verify no callers remain.

- [ ] **Subsection close-out (07.4)** — MANDATORY before completing section:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 07.R Third Party Review Findings

- None.

---

## 07.N Completion Checklist

- [ ] All subsections (07.1-07.4) complete
- [ ] `TypeFolder` trait exists with `super_fold()` recursion
- [ ] All 4+ substitution implementations migrated to the trait
- [ ] Net code reduction (expect ~400-500 lines removed from duplicated skeletons)
- [ ] All existing tests pass unchanged (behavior identical)
- [ ] Debug AND release builds pass
- [ ] `timeout 150 ./test-all.sh` passes
- [ ] **`/tpr-review`** — independent dual-source review clean
- [ ] **`/impl-hygiene-review`** — implementation hygiene clean
- [ ] **`/improve-tooling`** — section-close sweep
- [ ] **`/sync-claude`** — section-close doc sync
