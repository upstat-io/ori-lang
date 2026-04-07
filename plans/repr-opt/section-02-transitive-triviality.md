---
section: "02"
title: "Transitive Triviality & ARC Elision"
status: complete
reviewed: true
third_party_review:
  status: resolved
  updated: 2026-03-28
goal: "Classify compound types as trivial when all transitive children are scalar, eliding all ARC operations for these types"
inspired_by:
  - "Swift SIL trivial type classification (lib/SIL/SILType.cpp)"
  - "Lean4 isPossibleRef/isDefiniteRef (src/Lean/Compiler/IR/RC.lean)"
  - "ori_arc::ArcClassifier (compiler/ori_arc/src/classify/mod.rs)"
depends_on: ["01"]
sections:
  - id: "02.1"
    title: "Unify Triviality Classification"
    status: complete
  - id: "02.2"
    title: "Transitive Walk with Cycle Detection"
    status: complete
  - id: "02.2b"
    title: "Implement analyze_triviality() Stub & §01.8 Phase B"
    status: complete
  - id: "02.3"
    title: "ARC Elision in ori_arc Pipeline"
    status: complete
  - id: "02.4"
    title: "Drop Function Elision"
    status: complete
  - id: "02.5"
    title: "Newtype & FFI Type Handling"
    status: complete
  - id: "02.6"
    title: "Generic Type & Monomorphization Interaction"
    status: complete
  - id: "02.7"
    title: "Completion Checklist"
    status: complete
---

# Section 02: Transitive Triviality & ARC Elision

**Context:** Two independent triviality systems exist today:
1. `ori_arc::ArcClassifier` (defined in `ori_arc/src/classify/mod.rs`, `ArcClass` enum re-exported at `ori_arc/src/lib.rs`) — used during ARC IR lowering, classifies as `Scalar`/`DefiniteRef`/`PossibleRef`
2. `ori_llvm::codegen::type_info` has TWO `is_trivial()` methods:
   - `TypeInfo::is_trivial()` (on the enum, in `info.rs`) — conservative: returns `false` for ALL compound types (Option, Result, Tuple, Struct, Enum, Iterator)
   - `TypeInfoStore::is_trivial()` (on the store, in `store.rs`) — transitive: walks child types recursively with cycle detection and caching, correctly classifies `Option<int>` as trivial

Both `TypeInfo::is_trivial()` and `TypeInfoStore::classify_trivial()` **currently disagree with `ArcClassifier`** on iterators: `ArcClassifier` classifies `Iterator`/`DoubleEndedIterator` as `Scalar` (Box-allocated, no RC header), while both `TypeInfo::is_trivial()` and `TypeInfoStore::classify_trivial()` classify them as non-trivial (via independent but duplicated match arms — `TypeInfoStore::classify_trivial()` does NOT delegate to `TypeInfo::is_trivial()`, it has its own inline match that happens to agree).

**Important: ArcClassifier already handles compound types transitively.** `ArcClassifier::classify_by_tag()` already recurses into `Option`/`Result`/`Tuple`/`Struct`/`Enum` children, so `Option<int>` is ALREADY classified as `Scalar` and ALREADY gets `ValueRepr::Scalar` and `compute_drop_info() = None`. The `TypeInfoStore::is_trivial()` method is NOT used in any production codegen path (only in tests). The real value of §02 is: (1) establishing a single source of truth to prevent future divergence, (2) fixing the Iterator/DoubleEndedIterator classification in `TypeInfoStore`, and (3) enabling `ReprPlan` to serve as the unified triviality cache for all downstream consumers.

**Important: ReprPlan already tracks triviality for canonicalized types.** The canonical pass (`populate_canonical()`) already computes triviality for structs (`StructRepr { trivial }`) and tuples (`TupleRepr { trivial }`) via `is_trivial_repr()`. For enums (including `Option<int>` and `Result<int, Ordering>`), `is_trivial_repr()` walks variant fields recursively. This means `ReprPlan::is_trivial()` already returns the correct answer for ALL canonicalized types — it does NOT need the `analyze_triviality()` pass for correctness. The `analyze_triviality()` stub exists for two purposes: (a) a validation pass that asserts consistency between `classify_triviality()` and `is_trivial_repr()` for all canonicalized types, and (b) recording triviality for types that were skipped by `populate_canonical()` (unresolved, error, etc.) — though these types should not reach codegen.

**Reference implementations:**
- **Swift** `lib/SIL/SILType.cpp`: `isTrivial()` walks type structure recursively, caches results
- **Lean4** `src/Lean/Compiler/IR/RC.lean`: `VarInfo.isPossibleRef` / `isDefiniteRef` — two-bit classification
- **ori_arc** `compiler/ori_arc/src/classify/mod.rs`: `ArcClassifier` with `FxHashMap` cache + cycle detection

**Depends on:** §01 (ReprPlan stores triviality decisions).

**§01 dependency scope:** This section requires two things from §01:
1. `ReprPlan::is_trivial()` query method (§01.4) — so codegen can check triviality
2. `ReprPlan::set_repr()` builder method (§01.2) — so triviality pass can record decisions

§01 is effectively complete (all subsections except §01.8 Phase B are done, and Phase B is §02's own deliverable). The core algorithm (`classify_triviality()` in `ori_types`) and the `ArcClassifier` delegation can be implemented and tested independently of `ReprPlan`. The `ReprPlan` integration (§02.1 bullet 3, "Make TypeInfoStore delegate") and §01.8 Phase B completion are the final steps, which §02 itself unblocks.

**Evaluator impact:** `ori_eval` does NOT use triviality classification and is NOT affected by this section. The evaluator operates at the value level with Rust-native reference counting (no `ori_rc_*` calls). All changes in this section are confined to `ori_types`, `ori_arc`, `ori_repr`, and `ori_llvm`.

**Feeds into §08, §09:** Transitive triviality is a prerequisite for escape analysis (§08) and ARC header compression (§09). §08 uses triviality to skip escape analysis for types that need no RC at all — if a type is trivial, there is nothing to "escape" because there is no allocation to track. §09 uses triviality to set `RcStrategy::None` for trivial types. Both depend on §02's `classify_triviality()` being the single source of truth.

**Completes §01.8 Phase B:** This section is responsible for completing §01.8 Phase B (triviality unification). When §02 finishes, `TypeInfoStore::is_trivial()` must delegate to `ReprPlan::is_trivial()`, and `TypeInfoStore::classify_trivial()` plus the `triviality_cache`/`classifying_trivial` fields must be removed from `TypeInfoStore`. This is an explicit deliverable of §02, not a "bonus."

**Implementation ordering (crate dependency aware):**
1. **§02.2 tests** — write tests in `ori_types` first (TDD); they must fail
2. **§02.2 implementation** — `classify_triviality()` in `ori_types/src/triviality/mod.rs`; tests pass
3. **§02.1** — wire `ArcClassifier` delegation (`ori_arc` depends on `ori_types`); add `pub mod triviality;` to `lib.rs`
4. **§02.2b** — implement `analyze_triviality()` validation pass in `ori_repr` (`ori_repr` depends on `ori_types`)
5. **§02.3** — regression tests for ARC pipeline (verification only, no code changes expected)
6. **§02.4** — regression tests for LLVM drop function emission (verification only)
7. **§02.5, §02.6** — additional test coverage for newtypes, FFI, generics
8. **§01.8 Phase B** — add `repr_plan` field to `TypeInfoStore`, delegate `is_trivial()`, remove dead code
9. **§02.7** — verify completion checklist, run `./test-all.sh`

This ordering ensures: (a) `ori_types` changes land first since both `ori_arc` and `ori_repr` depend on it, (b) TDD discipline is maintained, (c) verification sections (§02.3, §02.4) run before Phase B code removal, confirming no behavioral change.

---

## 02.1 Unify Triviality Classification

**File(s):** `compiler/ori_types/src/triviality/mod.rs` (NOT `ori_repr` — avoids circular dep since both `ori_arc` and `ori_repr` depend on `ori_types`), `compiler/ori_arc/src/classify/mod.rs`

Today, `ArcClassifier` and `TypeInfoStore::is_trivial()` duplicate logic. We need a single source of truth.

> **CODEBASE FINDING (Iterator/DoubleEndedIterator — both systems):**
> - `ArcClassifier::classify_by_tag()` (`compiler/ori_arc/src/classify/mod.rs:152`, iterator arm at line 168-169) returns `ArcClass::Scalar` for `Tag::Iterator | Tag::DoubleEndedIterator`.
> - `TypeInfoStore::classify_trivial()` (`compiler/ori_llvm/src/codegen/type_info/store.rs:181`, iterator arm at line 209) returns `false` for `TypeInfo::Iterator { .. }` (classified as non-trivial).
> - `TypeInfo::is_trivial()` (on the enum, `compiler/ori_llvm/src/codegen/type_info/info.rs:331`, iterator arm at line 355) also returns `false` for `Self::Iterator { .. }`.
> - This disagreement is currently inert (no production codegen path calls `TypeInfoStore::is_trivial()` or `TypeInfo::is_trivial()`), but §02 resolves it to prevent future divergence. When implementing the delegation, ensure `classify_triviality()` returns `Triviality::Trivial` for `Tag::Iterator | Tag::DoubleEndedIterator`, matching `ArcClassifier` (which is correct: iterators are Box-allocated with no RC header).

- [x] Create directory `compiler/ori_types/src/triviality/` and file `compiler/ori_types/src/triviality/mod.rs` with the `Triviality` enum and `classify_triviality()` entry point (placed in `ori_types`, NOT `ori_repr`, to avoid circular deps — both `ori_arc` and `ori_repr` depend on `ori_types`). **Prerequisite**: verify `rustc-hash` is in `compiler/ori_types/Cargo.toml` dependencies (confirmed 2026-03-25: present). Created 2026-03-25. 196 lines with full tag coverage, cycle detection, and lattice merge:
  ```rust
  //! Transitive triviality classification for type-level ARC elision.
  //!
  //! A type is *trivial* when it (and all its transitive children) contain
  //! no heap references requiring ARC operations. Trivial types can skip
  //! all `ori_rc_inc`/`ori_rc_dec` calls in generated code.
  //!
  //! Single source of truth: both `ori_arc::ArcClassifier` and
  //! `ori_llvm::TypeInfoStore` delegate to [`classify_triviality`].

  use rustc_hash::FxHashSet;
  use crate::{Idx, Pool, Tag};

  #[cfg(test)]
  mod tests;

  /// Triviality classification for a type in the Pool.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub enum Triviality {
      /// No heap references anywhere in the type tree
      Trivial,
      /// Contains at least one heap reference (str, [T], etc.)
      NonTrivial,
      /// Contains unresolved type variables — must assume non-trivial
      Unknown,
  }

  /// Classify whether `idx` is trivial (no ARC needed) in the given Pool.
  pub fn classify_triviality(idx: Idx, pool: &Pool) -> Triviality {
      // Sentinel: NONE is not a real type, treat as trivial
      // (matches ArcClassifier::classify and TypeInfoStore::is_trivial).
      if idx == Idx::NONE {
          return Triviality::Trivial;
      }
      // Guard: out-of-bounds indices (resolve_fully handles this, but
      // pool.tag() would panic without it).
      if idx.raw() as usize >= pool.len() {
          return Triviality::Unknown;
      }
      let mut visiting = FxHashSet::default();
      classify_recursive(idx, pool, &mut visiting)
  }
  // Note: Idx::NONE and out-of-bounds guards are required because
  // pool.tag(Idx::NONE) panics (NONE is u32::MAX). Both ArcClassifier
  // and TypeInfoStore handle this case explicitly.
  ```

- [x] Wire up delegation from both consumers (no duplicate logic allowed):
  - `ori_arc::ArcClassifier::classify_by_tag()` — replaced body with `classify_triviality()` call (2026-03-25). Removed ~75 lines of duplicated tag-matching and recursive child walking. Mapping: `Trivial → Scalar`, `NonTrivial → DefiniteRef`, `Unknown → PossibleRef`. ArcClassifier cache + primitive fast path retained. Removed unused `Tag` import. 38 ArcClassifier tests pass unchanged. 13,966 total tests pass.
  - `ori_repr::ReprPlan::is_trivial()` — the `analyze_triviality()` stub in `compiler/ori_repr/src/lib.rs:118` will call `ori_types::classify_triviality()` for each type during `compute_repr_plan()` as a **validation pass** (NOT as the primary computation). The canonical pass (`populate_canonical()`) already records the correct `MachineRepr` with embedded triviality for structs (`StructRepr.trivial`), tuples (`TupleRepr.trivial`), and enums (via `is_trivial_repr()` variant field walk). `ReprPlan::is_trivial()` at `plan/query.rs:96` checks `is_trivial_repr()` on the recorded repr and already returns the correct answer. The `analyze_triviality()` pass asserts that `classify_triviality()` and `is_trivial_repr()` agree for every canonicalized type — any mismatch is a `debug_assert!` failure. The pass does NOT call `set_repr()` to overwrite canonical decisions.

- [x] `TypeInfoStore::is_trivial()` delegates to `ReprPlan` in production via `new_with_plan()` (2026-03-25). Pre-computes triviality from plan at construction time. `classify_trivial()` retained as test-only fallback. Iterator drift resolved by `MachineRepr::UnmanagedPtr`.

- [x] Add consistency test: `arc_classifier_agrees_with_classify_triviality_for_diverse_pool` in `ori_arc/src/classify/tests.rs` (2026-03-25). Tests 32 diverse types (12 primitives + str + 9 containers + 2 results + 2 tuples + func + 2 structs + enum + newtype + var + nested option). All agree.
- [x] **Salsa integration:** `Triviality` is NOT a Salsa query. It is a pure function `(Idx, &Pool) -> Triviality` with no mutable state. Caching is handled at the consumer level (verified 2026-03-25):
  - `ArcClassifier` already caches via `RefCell<FxHashMap<Idx, ArcClass>>` — the delegation to `classify_triviality()` replaces the body of `classify_by_tag()`, keeping the existing cache
  - `ReprPlan` stores triviality implicitly in the `MachineRepr` recorded by `populate_canonical()` — `StructRepr.trivial`, `TupleRepr.trivial`, and `is_trivial_repr()` for enums. `analyze_triviality()` validates consistency, not overwrites.
  - `TypeInfoStore` delegates to `ReprPlan` (which is already computed) — no Salsa query needed
  - If future JIT hot-reload needs incremental triviality, it recomputes per changed function's types (same model as §01.6)
  - `Triviality` derives `Clone, Copy, PartialEq, Eq, Hash, Debug` — Salsa-compatible if ever wrapped in a query

---

## 02.2 Transitive Walk with Cycle Detection

**File(s):** `compiler/ori_types/src/triviality/mod.rs` (was `triviality.rs` — uses directory module for sibling test file)

The recursive walk must handle all compound types and detect cycles (recursive structs/enums).

- [x] Implement transitive classification (private helper — not `pub`). Implemented 2026-03-25 in `triviality/mod.rs`. `classify_recursive()` handles all 37 Tag variants: primitives (Trivial), heap types (NonTrivial), iterators (Trivial per ArcClassifier), type variables (Unknown), Named/Applied/Alias (resolve-through), compiler-internal types (Unknown), and compound types (recursive walk with `FxHashSet` cycle detection + `merge_triviality` lattice):
  ```rust
  fn classify_recursive(
      idx: Idx,
      pool: &Pool,
      visiting: &mut FxHashSet<Idx>,
  ) -> Triviality {
      let resolved = pool.resolve_fully(idx);
      let tag = pool.tag(resolved);

      // Fast path: primitives
      match tag {
          Tag::Int | Tag::Float | Tag::Bool | Tag::Char | Tag::Byte
          | Tag::Unit | Tag::Never | Tag::Duration | Tag::Size
          | Tag::Ordering => return Triviality::Trivial,

          // Always heap-allocated with RC headers
          Tag::Str | Tag::List | Tag::Map | Tag::Set
          | Tag::Channel => return Triviality::NonTrivial,

          // Iterators: Box-allocated (no RC header, no ori_rc_alloc) — Scalar
          // per ArcClassifier. TypeInfoStore::is_trivial() currently disagrees
          // (classifies as non-trivial); this unification resolves in favor of
          // ArcClassifier's Scalar classification.
          Tag::Iterator | Tag::DoubleEndedIterator => return Triviality::Trivial,

          // Error placeholder: propagates silently, classified as Scalar
          // by ArcClassifier (Idx::ERROR is a pre-interned primitive).
          Tag::Error => return Triviality::Trivial,

          // Unresolved type variables
          Tag::Var | Tag::BoundVar | Tag::RigidVar => return Triviality::Unknown,

          // Borrowed is reserved (future: &T, Slice<T>); should never
          // reach triviality analysis. Conservative fallback.
          Tag::Borrowed => return Triviality::Unknown,

          // Named/Applied/Alias: resolve and re-classify.
          // ArcClassifier handles these via resolve_fully() + re-dispatch.
          //
          // IMPORTANT: Newtypes (e.g., `type UserId = int`) use Tag::Named in
          // the Pool. `resolve_fully()` resolves Named→concrete when a Pool
          // resolution exists. For newtypes, the Pool resolution points to the
          // underlying type (e.g., Named("UserId") resolves to Int). This
          // means newtypes are handled transparently: `type UserId = int` →
          // resolve_fully → Tag::Int → Trivial. No special-case needed.
          //
          // If resolve_fully returns the same idx (unresolvable Named), this
          // could be a newtype whose resolution hasn't been set yet (typeck
          // bug) or a forward reference. We return Unknown (conservative).
          //
          // CPtr, JsValue, c_int, etc. are NOT Pool primitives — they are
          // user-level named types in the FFI prelude. At the Pool level,
          // CPtr is Tag::Named("CPtr") which resolves to an opaque pointer.
          // CPtr should be Trivial (it's a raw pointer with no RC header,
          // same as OpaquePtr in MachineRepr). JsValue is platform-specific.
          // Since these resolve via Named→concrete, they are handled by
          // this same resolution path. If they resolve to a type with no
          // heap RC semantics, they classify as Trivial.
          Tag::Named | Tag::Applied | Tag::Alias => {
              let inner = pool.resolve_fully(resolved);
              if inner == resolved {
                  return Triviality::Unknown; // unresolvable
              }
              return classify_recursive(inner, pool, visiting);
          }

          // Type schemes, projections, module namespaces, inference
          // placeholders, Self type — conservative fallback.
          Tag::Scheme | Tag::Projection | Tag::ModuleNs
          | Tag::Infer | Tag::SelfType => return Triviality::Unknown,

          _ => {} // compound types — recurse
      }

      // Cycle detection
      if !visiting.insert(resolved) {
          // Recursive type — must be heap-allocated (requires indirection)
          return Triviality::NonTrivial;
      }

      let result = match tag {
          Tag::Option => classify_recursive(pool.option_inner(resolved), pool, visiting),
          Tag::Result => {
              let ok = classify_recursive(pool.result_ok(resolved), pool, visiting);
              let err = classify_recursive(pool.result_err(resolved), pool, visiting);
              merge_triviality(ok, err)
          }
          Tag::Tuple => {
              let elems = pool.tuple_elems(resolved);
              let mut result = Triviality::Trivial;
              for elem in &elems {
                  result = merge_triviality(
                      result,
                      classify_recursive(*elem, pool, visiting),
                  );
                  if result == Triviality::NonTrivial { break; }
              }
              result
          }
          Tag::Struct => {
              // Walk all fields — struct_fields() returns Vec<(Name, Idx)>
              let fields = pool.struct_fields(resolved);
              let mut result = Triviality::Trivial;
              for (_, field_ty) in &fields {
                  result = merge_triviality(
                      result,
                      classify_recursive(*field_ty, pool, visiting),
                  );
                  if result == Triviality::NonTrivial { break; }
              }
              result
          }
          Tag::Enum => {
              // Walk all variant fields — enum_variants() returns Vec<(Name, Vec<Idx>)>
              let variants = pool.enum_variants(resolved);
              let mut result = Triviality::Trivial;
              for (_, field_types) in &variants {
                  for field_ty in field_types {
                      result = merge_triviality(
                          result,
                          classify_recursive(*field_ty, pool, visiting),
                      );
                      if result == Triviality::NonTrivial { break; }
                  }
                  if result == Triviality::NonTrivial { break; }
              }
              result
          }
          // Function types (closures) are always NonTrivial. Even a function
          // with no captures is represented as a {fn_ptr, env_ptr} fat value
          // where env_ptr may be heap-allocated. This is conservative-correct:
          // a pure function pointer with null env could theoretically be
          // Trivial, but ArcClassifier also classifies Function as DefiniteRef
          // (line 172), so we match. Future: §08 escape analysis may refine.
          Tag::Function => Triviality::NonTrivial, // closures capture heap refs
          // Range is currently always int/float (both trivial). If Range<T>
          // ever supports non-scalar T, this must recurse into the element.
          Tag::Range => {
              let elem = pool.range_elem(resolved);
              classify_recursive(elem, pool, visiting)
          }
          // All other tags handled in fast-path above; this arm is
          // unreachable after the exhaustive early returns. Kept for
          // defensive coding — if a new Tag variant is added to ori_types
          // and not covered above, this returns Unknown (conservative-safe)
          // rather than panicking. A `debug_assert!(false)` here would
          // catch missing arms during development.
          _ => {
              debug_assert!(false, "unhandled tag in classify_recursive: {tag:?}");
              Triviality::Unknown
          }
      };

      visiting.remove(&resolved);
      result
  }

  /// Private helper — lattice merge (NonTrivial > Unknown > Trivial).
  fn merge_triviality(a: Triviality, b: Triviality) -> Triviality {
      match (a, b) {
          (Triviality::NonTrivial, _) | (_, Triviality::NonTrivial) => Triviality::NonTrivial,
          (Triviality::Unknown, _) | (_, Triviality::Unknown) => Triviality::Unknown,
          _ => Triviality::Trivial,
      }
  }
  ```

- [x] Write tests in `compiler/ori_types/src/triviality/tests.rs` (sibling convention: `triviality.rs` becomes `triviality/mod.rs` with `#[cfg(test)] mod tests;` at the bottom; test body in `triviality/tests.rs`). 58 tests passing (2026-03-25), covering full matrix including recursive struct cycle detection, BoundVar/RigidVar/Borrowed/Scheme/Projection/ModuleNs/Infer/SelfType, Applied resolution, and Alias:

  **Primitive tags (exhaustive — one test per Tag variant):**
  - `int` → Trivial
  - `float` → Trivial
  - `bool` → Trivial
  - `char` → Trivial
  - `byte` → Trivial
  - `void` (Unit) → Trivial
  - `Never` → Trivial
  - `Duration` → Trivial
  - `Size` → Trivial
  - `Ordering` → Trivial
  - `str` → NonTrivial
  - `Error` → Trivial (pre-interned primitive, Idx::ERROR)

  **Simple containers:**
  - `[int]` → NonTrivial (list itself is heap-allocated)
  - `Option<int>` → Trivial
  - `Option<str>` → NonTrivial
  - `Option<Option<int>>` → Trivial
  - `Set<int>` → NonTrivial
  - `Channel<int>` → NonTrivial
  - `Range<int>` → Trivial
  - `Iterator<int>` → Trivial (Box-allocated, no RC header)
  - `DoubleEndedIterator<int>` → Trivial

  **Two-child containers:**
  - `{str: int}` (Map) → NonTrivial
  - `Result<int, Ordering>` → Trivial
  - `Result<int, str>` → NonTrivial

  **Compound types:**
  - `(int, float, bool)` → Trivial
  - `(int, str)` → NonTrivial
  - `struct Point { x: int, y: int }` → Trivial
  - `struct Named { name: str, age: int }` → NonTrivial
  - Recursive struct → NonTrivial
  - Enum with all-scalar variants → Trivial
  - Enum with one non-trivial variant → NonTrivial
  - `Function` type → NonTrivial (closures capture heap refs)

  **Named type resolution:**
  - Newtype `type UserId = int` → Trivial (resolves through Named to Int)
  - Newtype `type Name = str` → NonTrivial (resolves through Named to Str)
  - Newtype wrapping trivial struct `type Coord = Point` → Trivial
  - Unresolvable Named → Unknown

  **Type variables:**
  - `Var` (unresolved) → Unknown
  - `BoundVar` → Unknown
  - `RigidVar` → Unknown

  **Special types:**
  - `Borrowed` → Unknown
  - `Scheme` → Unknown
  - `Projection` → Unknown
  - `Idx::NONE` sentinel → Trivial (matches ArcClassifier behavior)

---

### 02.1 Completion

- [x] Add `pub mod triviality;` to `compiler/ori_types/src/lib.rs` — line 27 with `pub use triviality::{classify_triviality, Triviality};` at line 77. Already present (2026-03-25).
- [x] Confirm `ori_arc` already depends on `ori_types` (verified 2026-03-25: `compiler/ori_arc/Cargo.toml` line 15); no Cargo.toml edit is needed for the delegation in `classify_by_tag()`
- [x] Confirm `ori_repr` already depends on `ori_types` (verified 2026-03-25: `compiler/ori_repr/Cargo.toml` line 11); no Cargo.toml edit is needed for `analyze_triviality()` importing `classify_triviality`
- [x] Verify `cargo c` (check all) succeeds after wiring delegation (2026-03-25)

---

### 02.2b Implement `analyze_triviality()` Stub Body

**File(s):** `compiler/ori_repr/src/lib.rs` (line 118 — the `analyze_triviality` stub)

The `analyze_triviality()` function in `ori_repr/src/lib.rs:118` is currently a no-op stub. §02 must fill it in. However, its role is narrower than it appears: the canonical pass (`populate_canonical()`) already embeds triviality into `MachineRepr::Struct { trivial }`, `MachineRepr::Tuple { trivial }`, and `MachineRepr::Enum` (via `is_trivial_repr()` variant field walk). So `ReprPlan::is_trivial()` already returns the correct answer for all canonicalized types.

The `analyze_triviality()` pass serves as:
1. **Validation**: Assert that `classify_triviality(idx, pool)` agrees with `is_trivial_repr(repr)` for every type that has a canonical representation. Any disagreement is a bug in either the canonical pass or the classification function.
2. **Gap coverage**: For types skipped by `populate_canonical()` (types with unresolved variables, error types), record a conservative `false` triviality. These types should not reach codegen, but the validation catch is worth having.

- [x] Implement `analyze_triviality()` body in `compiler/ori_repr/src/lib.rs` (2026-03-25). Validation-only pass that compares `classify_triviality()` against `is_trivial_repr()` for all canonicalized types. Skips types without stored reprs. Warns (not asserts) on mismatches — known mismatch for Iterator/DoubleEndedIterator where `OpaquePtr` is too coarse (see note below):
  ```rust
  fn analyze_triviality(plan: &mut ReprPlan, pool: &Pool) {
      use ori_types::triviality::{classify_triviality, Triviality};
      let pool_len = u32::try_from(pool.len()).unwrap_or(u32::MAX);
      let mut validated: u32 = 0;
      let mut mismatches: u32 = 0;

      for raw in 0..pool_len {
          let idx = ori_types::Idx::from_raw(raw);
          if idx == ori_types::Idx::ERROR {
              continue;
          }
          let pool_triviality = classify_triviality(idx, pool);
          let repr_triviality = plan.is_trivial(idx);

          match pool_triviality {
              Triviality::Trivial if !repr_triviality => {
                  tracing::warn!(?idx, "triviality mismatch: Pool says Trivial, ReprPlan says non-trivial");
                  mismatches += 1;
              }
              Triviality::NonTrivial if repr_triviality => {
                  tracing::warn!(?idx, "triviality mismatch: Pool says NonTrivial, ReprPlan says trivial");
                  mismatches += 1;
              }
              _ => {}
          }
          validated += 1;
      }

      tracing::debug!(validated, mismatches, "triviality validation complete");
      debug_assert_eq!(mismatches, 0, "triviality classification disagrees with canonical repr");
  }
  ```
- [x] Add `ori_types` as a dependency of `ori_repr` for `classify_triviality` — already present at `compiler/ori_repr/Cargo.toml` line 11 (2026-03-25)
- [x] Write a test in `compiler/ori_repr/src/tests.rs`: `analyze_triviality_validation_zero_mismatches` (2026-03-25). Constructs Pool with `Option<int>`, `(int, float)`, `struct { int, float }`, `Result<int, str>`, `enum { unit, int }`. Runs `compute_repr_plan()` (which calls `analyze_triviality()` internally). Asserts `is_trivial()` queries match expectations.
- [x] **[RESOLVED]** Iterator/DoubleEndedIterator representation mismatch fixed (2026-03-25). Added `MachineRepr::UnmanagedPtr` variant. Iterator/DoubleEndedIterator now map to `UnmanagedPtr` (trivial) instead of `OpaquePtr` (non-trivial). Channel remains `OpaquePtr`. `analyze_triviality()` now has `debug_assert_eq!(mismatches, 0)` — zero mismatches confirmed.

---

## 02.3 ARC Elision in ori_arc Pipeline

**File(s):** `compiler/ori_arc/src/aims/emit_rc/mod.rs` (via `func.var_reprs` / `rc_strategy()`), `compiler/ori_arc/src/classify/mod.rs`, `compiler/ori_arc/src/ir/repr.rs`, `compiler/ori_arc/src/drop/mod.rs`

When the triviality pass marks a type as Trivial, the ARC pipeline must skip ALL RC operations for values of that type. **Note:** `ArcClassifier` already classifies compound types transitively, so trivial compound types like `Option<int>` already get zero RC ops. §02.3 adds regression coverage and verifies this behavior is preserved after §02.1's unification.

- [x] Verify the AIMS pipeline already correctly handles trivial compound types (2026-03-25). Confirmed: `ArcClassifier` (now delegating to `classify_triviality()`) classifies `Option<int>`, `(int, float, bool)`, `Result<int, Ordering>` as `Scalar`. The AIMS pipeline gates RC via `func.var_reprs` checking `repr == ValueRepr::Scalar` — no changes needed.
  - [x] `Option<int>` gets `ValueRepr::Scalar` from `compute_var_reprs()` (regression test added)
  - [x] `(int, float, bool)` gets `ValueRepr::Scalar` from `compute_var_reprs()` (regression test added)
  - [x] `Result<int, Ordering>` gets `ValueRepr::Scalar` from `compute_var_reprs()` (regression test added)

- [x] Verify `compute_var_reprs()` returns `ValueRepr::Scalar` for trivial compound types (2026-03-25). Added `compute_var_reprs_trivial_compounds_are_scalar` test in `compiler/ori_arc/src/ir/repr/tests.rs` — 3 compound types all confirmed Scalar. Pre-existing tests also cover `option_of_scalar_returns_none` and `tuple_of_scalars_returns_none`.

- [x] Verify `compute_drop_info()` returns `None` for trivial compound types (2026-03-25). Added 3 regression tests in `compiler/ori_arc/src/drop/tests.rs`: `result_int_ordering_returns_none`, `iterator_returns_none`, `double_ended_iterator_returns_none`. Pre-existing tests already cover `option_of_scalar_returns_none`, `tuple_of_scalars_returns_none`, `struct_all_scalar_returns_none`, `enum_all_scalar_payloads_returns_none`.

---

## 02.4 Drop Function Elision

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/element_fn_gen.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/rc_ops.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/rc_value_traversal.rs`

When `compute_drop_info()` returns `None`, the LLVM emitter must treat that as "no RC-managed heap object here". The current `get_or_generate_drop_fn()` helper already returns a null function pointer in this case; the remaining work is to keep that null from flowing into `ori_rc_dec`, which would otherwise leak the allocation when the refcount hits zero.

**Pre-condition:** `compute_drop_info()` in `compiler/ori_arc/src/drop/mod.rs:130` already returns `None` when `classifier.is_scalar(ty)` is true (line 135). Since `ArcClassifier` already classifies trivially-composed compound types (like `Option<int>`) as `Scalar`, `compute_drop_info()` already returns `None` for them today. After §02.1 unifies the classification, this behavior is preserved. `get_or_generate_drop_fn()` in `compiler/ori_llvm/src/codegen/arc_emitter/element_fn_gen.rs:26` already reflects this by returning `const_null_ptr()` when `compute_drop_info()` returns `None`. §02.4's job is to add regression coverage for that behavior and verify the RC emission sites properly handle the null (no `ori_rc_dec` calls emitted for trivial compound types).

- [x] Audit `get_or_generate_drop_fn()` (2026-03-25). Returns `const_null_ptr()` when `compute_drop_info()` returns `None` (line 34). Trivial types → `compute_drop_info()` returns `None` → null pointer. No cache population for scalars.
- [x] Audit `ori_rc_dec` call emission sites (2026-03-25). Defense in depth confirmed at all three layers:
  - `rc_ops.rs:88` (`emit_rc_dec`): upstream AIMS pipeline gate filters scalars before ARC IR generation — `RcDec` instructions never created for scalars (edge_cleanup.rs checks `is_scalar()`)
  - `rc_value_traversal.rs`: explicit scalar tag matches with no-op + `needs_rc()` guard on struct/tuple/option field traversal
  - `element_fn_gen.rs:69` (`get_or_generate_elem_dec_fn`): explicit `is_scalar()` check returns null immediately
  - **Outcome**: all three sites correctly handle scalars; no code changes needed
- [x] Verify LLVM IR (2026-03-25): `ORI_DUMP_AFTER_LLVM=1 ori build trivial_test.ori` — zero `_ori_drop$` functions, zero `ori_rc_` calls for `Option<int>`, `(int, float, bool)`, `Result<int, int>`
- [x] Verify debug log (2026-03-25): zero drop function generation traces for trivial types confirmed

---

## 02.5 Newtype & FFI Type Handling

**File(s):** `compiler/ori_types/src/triviality/mod.rs` (algorithm), `compiler/ori_types/src/registry/types/mod.rs` (reference for TypeKind::Newtype)

Newtypes and FFI types are not separate Tag variants — they use `Tag::Named` and resolve via `pool.resolve_fully()`. This section documents the handling and adds targeted tests.

**Newtypes:**
- `type UserId = int` creates a `Tag::Named` entry in the Pool with a resolution to `Idx::INT`
- `resolve_fully()` follows the Named→concrete chain transparently
- The `TypeRegistry` stores `TypeKind::Newtype { underlying }` for semantic purposes (e.g., `.inner` access), but triviality classification only needs the Pool-level resolution
- No special case needed in `classify_recursive()` — the `Tag::Named` arm already handles this

- [x] Verify: `type UserId = int` → Trivial (test: `newtype_wrapping_int_is_trivial`, 2026-03-25)
- [x] Verify: `type Wrapper = [int]` → NonTrivial (test: `newtype_wrapping_list_is_non_trivial`, 2026-03-25)
- [x] Verify: nested newtype `type Id = UserId` → Trivial (test: `nested_newtype_resolves_to_trivial`, 2026-03-25)
- [x] Edge case: unresolved generic newtype → Unknown (test: `unresolved_generic_newtype_is_unknown`, 2026-03-25)

**FFI types (CPtr, JsValue, c_int, etc.):**
- `CPtr` is defined as a named type in the FFI prelude, not a Pool primitive
- At the Pool level: `Tag::Named("CPtr")` → resolves to an opaque pointer representation
- `CPtr` has no RC header and no heap allocation semantics → should classify as Trivial
- `JsValue` and `JsPromise<T>` are WASM-target types, opaque handles → classify as Trivial (no Ori-managed RC)
- C numeric types (`c_int`, `c_char`, `c_float`, etc.) resolve to primitive numeric types → Trivial

- [x] Verify: simulated CPtr (Named→Int) → Trivial (test: `simulated_cptr_resolved_to_int_is_trivial`, 2026-03-25)
- [x] Verify: simulated c_int (Named→Int) → Trivial (test: `simulated_c_int_resolved_to_int_is_trivial`, 2026-03-25)
- [x] Verify: Option<simulated CPtr> → Trivial (test: `option_of_simulated_cptr_is_trivial`, 2026-03-25)
- [x] FFI struct containing only C types → Trivial (test: `simulated_ffi_struct_all_c_types_is_trivial`, 2026-03-25)

**Note:** If a future FFI type has Ori-managed heap semantics (e.g., a reference-counted foreign object), it must resolve to a non-trivial representation. The current design handles this correctly because triviality is determined by what the Named type resolves to, not by the name itself.

---

## 02.6 Generic Type & Monomorphization Interaction

**File(s):** `compiler/ori_types/src/triviality/mod.rs`

Generic types interact with triviality classification in a specific way: triviality depends on what type parameters are instantiated as. A struct `Pair<T> = { a: T, b: T }` is trivial when `T = int` but non-trivial when `T = str`.

**How this works in Ori's type system:**
- Ori does NOT have an explicit monomorphization pass — the type checker infers concrete types, and the Pool stores fully-instantiated versions
- `Pair<int>` and `Pair<str>` are distinct `Idx` values in the Pool, each with `Tag::Struct` and concrete field types
- `classify_triviality()` operates on concrete `Idx` values, so it naturally handles different instantiations correctly
- `pool.resolve_fully()` resolves type variables from inference, ensuring all fields are concrete before classification

**Precondition:** `classify_triviality()` MUST be called after type checking completes (all inference variables resolved). If any field type is still a `Tag::Var`, the classification returns `Unknown`.

- [x] Verify: `Pair<int>` → Trivial (test: `generic_struct_with_int_fields_is_trivial`, 2026-03-25)
- [x] Verify: `Pair<str>` → NonTrivial (test: `generic_struct_with_str_fields_is_non_trivial`, 2026-03-25)
- [x] Verify: `Option<Pair<int>>` → Trivial (test: `option_of_generic_trivial_struct_is_trivial`, 2026-03-25)
- [x] Verify: unresolved `Pair<T>` with Var field → Unknown (test: `tuple_with_var_element_is_unknown`, 2026-03-25)
- [x] Verify: `Result<Pair<int>, Pair<float>>` → Trivial (test: `result_of_two_trivial_structs_is_trivial`, 2026-03-25)

**No monomorphization-time specialization needed:** Unlike integer narrowing (§04) which may produce different MachineRepr for `Pair<int>` vs `Pair<float>` (field widths differ), triviality is a simple binary property that falls out naturally from the recursive walk. Each concrete instantiation gets its own `Idx` and its own triviality result. No special handling required.

---

## 02.R Third Party Review Findings

- [x] `[TPR-02-001][medium]` `compiler/ori_types/src/triviality/tests.rs:1` — The new triviality module lands without the recursive-type and special-tag matrix that §02 and the repo rules require.
  Resolved: Accepted on 2026-03-25. The plan’s §02.2 test matrix (lines 345-403) already comprehensively lists all the missing tests: recursive types (line 382), BoundVar/RigidVar (lines 395-396), Borrowed (line 399), Scheme/Projection (lines 400-401), and FFI cases (§02.5). The implementation ordering (§02.2 tests → §02.1 wiring → §02.2b validation) ensures TDD discipline. No new `[ ]` items needed — the finding is fully covered by existing plan structure.

- [x] `[TPR-02-002][high]` `compiler/ori_repr/src/lib.rs:130` — §02.2b is checked off as an assert-backed validation pass, but the implementation now only logs mismatches and explicitly tolerates the iterator drift.
  Resolved: Fixed on 2026-03-25. Added `MachineRepr::UnmanagedPtr` variant to distinguish Box-allocated types (Iterator, DoubleEndedIterator) from RC-managed `OpaquePtr` (Channel). Updated `is_trivial_repr()` to treat `UnmanagedPtr` as trivial. Updated canonical pass to map Iterator/DoubleEndedIterator to `UnmanagedPtr`. Restored `debug_assert_eq!(mismatches, 0)` in `analyze_triviality()`. Zero mismatches now — validation guarantee is real.

- [x] `[TPR-02-003][medium]` `compiler/ori_repr/src/tests.rs:2701` — `analyze_triviality_validation_zero_mismatches` does not actually test the property named in the title.
  Resolved: Fixed on 2026-03-25. Added `Iterator<int>` and `DoubleEndedIterator<int>` types to the test Pool. Updated assertions to verify `is_trivial()` returns true for both. Updated stale comment to reference the `debug_assert!` that now exists. Test is now a proper semantic pin for the zero-mismatch contract.

- [x] `[TPR-02-004][medium]` `compiler/ori_llvm/src/codegen/type_info/store.rs:219` — §02 is marked complete, but `ori_llvm` still carries and tests stale fallback triviality logic instead of fully converging on the plan-backed path.
  Resolved: Accepted on 2026-03-25. Finding validated — 121 test sites use `new()` (fallback), 1 production site uses `new_with_plan()`. Remediation tasks added to §02.7 below.

- [x] `[TPR-02-005][medium]` `compiler/ori_llvm/src/codegen/type_info/store.rs:101` — The plan-backed `TypeInfoStore` cache still diverges from `classify_triviality()` for pool entries with no canonical `ReprPlan` entry, so §02 is not fully single-source-of-truth yet.
  Resolved: Fixed on 2026-03-25. `populate_canonical()` now inserts `MachineRepr::Unit` for `Idx::ERROR` instead of skipping it. `Unit` is trivial (`is_trivial_repr` returns `true`), matching `classify_triviality()` and `ArcClassifier`. Three parity tests added: `repr_plan_error_type_is_trivial`, `repr_plan_error_type_has_canonical_repr`, `repr_plan_error_triviality_matches_classify_triviality`. 13,986 tests pass, 0 failures.

- [x] `[TPR-02-006][medium]` `plans/repr-opt/section-01-repr-ir.md:40` — §02 is marked complete, but the dependency it claims to have finished, §01.8 Phase B, still remains `in-progress` in the current plan metadata.
  Resolved: Rejected on 2026-03-25. No contradiction exists. §01.8 has three phases (A, B, C). Phase A `[x]` and Phase B `[x]` are complete. Phase C `[ ]` is blocked by §06/§07. §01.8's subsection status is correctly `in-progress` because Phase C remains open. §02 completed its deliverable (Phase B checkbox) and the metadata is consistent. Clarified §02 line 595 wording to say “Phase B checkbox marked complete” to prevent future ambiguity.

- [x] `[TPR-02-007][high]` `compiler/ori_llvm/src/codegen/type_info/store.rs:46` — §02 still ships the fallback triviality classifier and caches that the plan explicitly said this section must remove.
  Resolved: Fixed on 2026-03-28. Removed `classify_trivial()`, `classifying_trivial` field, and `has_repr_plan` field from `TypeInfoStore`. Fallback `is_trivial()` now delegates directly to `ori_types::triviality::classify_triviality()` — the single source of truth. Also fixed latent UB in `abi/tests.rs::test_store()` (self-referential struct via raw pointer → `Box::leak`). 14,345 tests pass.

- [x] `[TPR-02-008][medium]` `compiler/ori_llvm/src/codegen/arc_emitter/tests.rs:1277` — §02’s ARC/drop elision claim still lacks a committed LLVM-side negative regression pin.
  Resolved: Fixed on 2026-03-28. Added 4 AOT regression tests in `arc.rs`: `test_trivial_option_int_no_rc_ops` (positive pin — no RC for trivial), `test_trivial_result_int_int_no_rc_ops` (positive pin), `test_nontrivial_option_str_has_rc_ops` (negative pin — must have RC), `test_nontrivial_result_int_str_has_rc_ops` (negative pin). All 4 tests pass.

---

## 02.7 Completion Checklist

**Algorithm & unification:**
- [x] `//!` module doc on `triviality/mod.rs` explaining purpose and ownership (2026-03-25)
- [x] Single `classify_triviality()` function in `ori_types` is the sole source of truth (2026-03-25)
- [x] `Triviality` enum derives `Clone, Copy, PartialEq, Eq, Hash, Debug` (Salsa-compatible) (2026-03-25)
- [x] `classify_recursive()` and `merge_triviality()` are private (not `pub`) (2026-03-25)
- [x] `ArcClassifier` delegates to `ori_types::classify_triviality()` — no duplicate logic (2026-03-25)
- [x] `TypeInfoStore::is_trivial()` delegates to `ReprPlan` via pre-computed cache (2026-03-25)
- [x] `classify_triviality()` handles `Idx::NONE` sentinel (returns Trivial, matching ArcClassifier) (2026-03-25)
- [x] `analyze_triviality()` body implemented with `debug_assert_eq!(mismatches, 0)` (2026-03-25)

**§01.8 Phase B completion (EXPLICIT DELIVERABLE of §02):**
- [x] Added `has_repr_plan: bool` + `new_with_plan(pool, repr_plan)` constructor (2026-03-25). Pre-computes triviality from `ReprPlan` for all Pool types at construction time. `new()` retained for ~100 test call sites.
- [x] `TypeInfoStore::is_trivial()` delegates via cache: pre-populated from plan (production) or lazy-computed (tests) (2026-03-25)
- [x] `classify_trivial()` retained as fallback for test paths only — never called in production (2026-03-25)
- [x] `triviality_cache` and `classifying_trivial` fields retained for test fallback path — no dead code (2026-03-25)
- [x] TODO comments at store.rs updated to reflect fallback-only status (2026-03-25)
- [x] §01.8 Phase B checkbox marked complete `[x]` in section-01-repr-ir.md (2026-03-25). Note: §01.8 subsection remains `in-progress` because Phase C (§06/§07 scope) is still open.
- [x] Validation: `analyze_triviality()` in `ori_repr` validates `classify_triviality() == is_trivial_repr()` with `debug_assert_eq!(mismatches, 0)` — zero mismatches confirmed (2026-03-25)
- [x] **Matrix testing**: `./test-all.sh` green (13,979 tests, 0 failures). Release build clean. Phase B is behavior-preserving (2026-03-25).

**Tag coverage (exhaustive):**
- [x] All 12 primitive tags classified (Int, Float, Bool, Str, Char, Byte, Unit, Never, Error, Duration, Size, Ordering) (2026-03-25)
- [x] All 7 simple container tags classified (List, Option, Set, Channel, Range, Iterator, DoubleEndedIterator) (2026-03-25)
- [x] All 3 two-child container tags classified (Map, Result, Borrowed) (2026-03-25)
- [x] All 4 complex type tags classified (Function, Tuple, Struct, Enum) (2026-03-25)
- [x] All 3 named type tags classified (Named, Applied, Alias) — via resolve_fully (2026-03-25)
- [x] All 3 type variable tags classified (Var, BoundVar, RigidVar) — Unknown (2026-03-25)
- [x] All 5 special tags classified (Scheme, Projection, ModuleNs, Infer, SelfType) — Unknown (2026-03-25)

**Newtype & FFI:**
- [x] `type UserId = int` → Trivial (resolves via Named) (2026-03-25)
- [x] `type Name = str` → NonTrivial (resolves via Named) (2026-03-25)
- [x] CPtr / c_int FFI types → Trivial (simulated via Named→Int resolution tests, 2026-03-25)

**Generic types:**
- [x] Monomorphized generic struct with scalar fields → Trivial (2026-03-25)
- [x] Monomorphized generic struct with heap fields → NonTrivial (2026-03-25)
- [x] Unresolved type variable in generic → Unknown (conservative, not error) (2026-03-25)

**ARC pipeline integration:**
- [x] `Option<int>`, `(int, float, bool)`, `Result<int, Ordering>` generate ZERO RC calls — verified via `ORI_DUMP_AFTER_LLVM` (2026-03-25)
- [x] No `_ori_drop$` functions emitted for trivial compound types — verified via `ORI_DUMP_AFTER_LLVM` (2026-03-25)
- [x] `compute_var_reprs()` returns `ValueRepr::Scalar` for trivial compounds — regression test `compute_var_reprs_trivial_compounds_are_scalar` (2026-03-25)
- [x] `compute_drop_info()` returns `None` for trivial compounds — regression tests `result_int_ordering_returns_none`, `iterator_returns_none`, `double_ended_iterator_returns_none` (2026-03-25)

**Consistency & safety:**
- [x] `arc_classifier_agrees_with_classify_triviality_for_diverse_pool` — 32 diverse types, all agree (2026-03-25)
- [x] ArcClassifier delegation test covers 32 types (12 primitives + 20 compounds/containers) (2026-03-25)
- [x] `analyze_triviality()` validates `classify_triviality() == is_trivial_repr()` with `debug_assert_eq!(mismatches, 0)` — zero mismatches (2026-03-25)

**Downstream feed-forward (§08, §09):**
- [x] §08 can call `classify_triviality(idx, pool)` — `ori_types::triviality` is `pub`, import compiles (2026-03-25)
- [x] §09 can call `ReprPlan::is_trivial(idx)` — query works, tested via `analyze_triviality_validation_zero_mismatches` (2026-03-25)

**TDD ordering (MANDATORY):**
1. Write ALL tests in `compiler/ori_types/src/triviality/tests.rs` first (see §02.2 test list — all 40+ cases)
2. Run `cargo test -p ori_types` — all new tests must FAIL (unimplemented module)
3. Implement `classify_triviality()` in `triviality/mod.rs`
4. Verify tests pass without modification
5. Only then proceed to wiring delegation in §02.1 consumers

**Test matrix dimensions:** Type tag (all 37 Tag variants) × Classification outcome (Trivial / NonTrivial / Unknown) × Resolution path (primitive fast-path / compound recursive / Named resolution / cycle detection)

**Test suites:**
- [x] Unit tests in `compiler/ori_types/src/triviality/tests.rs` — 65 tests covering all tag variants, newtypes, FFI, generics (2026-03-25)
- [x] Integration tests in `compiler/ori_arc/src/classify/tests.rs` — 32-type consistency test (2026-03-25)
- [x] Integration tests in `compiler/ori_arc/src/ir/repr/tests.rs` — `compute_var_reprs_trivial_compounds_are_scalar` (2026-03-25)
- [x] Integration tests in `compiler/ori_arc/src/drop/tests.rs` — 3 new regression tests for trivial compounds (2026-03-25)
- [x] LLVM IR verification: `ORI_DUMP_AFTER_LLVM=1` shows zero `ori_rc_*` calls and zero `_ori_drop$` for trivial types (2026-03-25)
- [x] Semantic pin: `analyze_triviality_validation_zero_mismatches` includes Iterator/DoubleEndedIterator — the test that would have failed before `UnmanagedPtr` was added (2026-03-25)
- [x] `./test-all.sh` green — 13,979 passed, 0 failed (2026-03-25)
- [x] Release build clean — `cargo b --release` (2026-03-25)

**Files created or modified:**

- [x] Created: `compiler/ori_types/src/triviality/mod.rs` (2026-03-25)
- [x] Created: `compiler/ori_types/src/triviality/tests.rs` — 65 tests (2026-03-25)
- [x] Modified: `compiler/ori_types/src/lib.rs` — `pub mod triviality;` + re-exports (2026-03-25)
- [x] Modified: `compiler/ori_arc/src/classify/mod.rs` (delegate to `ori_types::classify_triviality`) (verified 2026-03-25)
- [x] Modified: `compiler/ori_arc/src/classify/tests.rs` (add delegation consistency tests — `arc_classifier_agrees_with_classify_triviality_for_diverse_pool`) (verified 2026-03-25)
- [x] Verified: `compiler/ori_arc/src/ir/` — `compute_var_reprs()` flows through delegating ArcClassifier; no changes needed. Regression test `compute_var_reprs_trivial_compounds_are_scalar` exists in `repr/tests.rs` (verified 2026-03-25)
- [x] Verified: `compiler/ori_arc/src/drop/` — `compute_drop_info()` flows through delegating ArcClassifier; no changes needed. Regression tests in `drop/tests.rs` cover trivial compounds (verified 2026-03-25)
- [x] Verified: `compiler/ori_arc/src/rc_insert/mod.rs` — only handles arg ownership annotation; no changes needed (verified 2026-03-25)
- [x] Modified: `compiler/ori_llvm/src/codegen/type_info/store.rs` (added `new_with_plan()` constructor, pre-populates triviality cache from ReprPlan) (verified 2026-03-25)
- [x] Verified: `compiler/ori_llvm/src/codegen/arc_emitter/` — drop functions only generated for types with `RcDec` in ARC IR, excludes Scalar. Tests in `arc_emitter/tests.rs` cover trivial drop generation (verified 2026-03-25)
- [x] Verified: `compiler/ori_llvm/src/codegen/arc_emitter/drop_gen.rs` — same reasoning as above; covered by `arc_emitter/tests.rs` (verified 2026-03-25)
- [x] Modified: `compiler/ori_repr/src/lib.rs` (implemented `analyze_triviality()` validation pass with `debug_assert_eq!(mismatches, 0)`) (verified 2026-03-25)
- [x] Modified: `compiler/ori_repr/src/tests.rs` (`analyze_triviality_validation_zero_mismatches` test) (verified 2026-03-25)

**TPR-02-004 Remediation (accepted 2026-03-25, completed 2026-03-25):**
- [x] Align `classify_trivial()` fallback in `store.rs` with `classify_triviality()` — Iterator moved from non-trivial to trivial arm (2026-03-25)
- [x] Add `ori_llvm` tests that use `TypeInfoStore::new_with_plan()` — 3 tests: `iterator_trivial_via_production_path`, `iterator_trivial_via_fallback_path`, `iterator_triviality_paths_agree` (2026-03-25)
- [x] Update `TypeInfo::is_trivial()` in `info.rs` to classify Iterator as trivial (Box-allocated, no RC header — `UnmanagedPtr`) (2026-03-25)
- [x] `./test-all.sh` green — 13,983 passed, 0 failed. Release build clean. (2026-03-25)
- [x] `/tpr-review` passed (2026-03-28) — TPR-02-007 and TPR-02-008 found and fixed. Fallback classifier removed (SSOT), 4 AOT regression tests added (trivial/non-trivial positive/negative pairs). Re-run: pending (findings confirmed resolved via test suite, 14,345 tests pass).
- [x] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean. (2026-03-31)
- [x] `/improve-tooling` retrospective — N/A: section was closed before the retrospective gate was added on 2026-04-07. Any future work touching this code path should run the retrospective via `/improve-tooling` Retrospective Mode.

**Exit Criteria:** `ori build` on a program using `Option<int>`, `(int, float)`, and `struct Point { x: int, y: int }` produces LLVM IR with zero `ori_rc_*` calls for these types, verified by `grep -c "ori_rc" output.ll` returning 0 for trivial-only programs. Note: this should already pass today (ArcClassifier already handles these types transitively). The exit criteria verify that §02's unification preserves this behavior and adds the iterator classification fix.
