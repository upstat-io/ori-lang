---
section: "02"
title: "Transitive Triviality & ARC Elision"
status: in-progress
reviewed: true
third_party_review:
  status: findings
  updated: 2026-03-25
goal: "Classify compound types as trivial when all transitive children are scalar, eliding all ARC operations for these types"
inspired_by:
  - "Swift SIL trivial type classification (lib/SIL/SILType.cpp)"
  - "Lean4 isPossibleRef/isDefiniteRef (src/Lean/Compiler/IR/RC.lean)"
  - "ori_arc::ArcClassifier (compiler/ori_arc/src/classify/mod.rs)"
depends_on: ["01"]
sections:
  - id: "02.1"
    title: "Unify Triviality Classification"
    status: not-started
  - id: "02.2"
    title: "Transitive Walk with Cycle Detection"
    status: not-started
  - id: "02.2b"
    title: "Implement analyze_triviality() Stub & §01.8 Phase B"
    status: not-started
  - id: "02.3"
    title: "ARC Elision in ori_arc Pipeline"
    status: not-started
  - id: "02.4"
    title: "Drop Function Elision"
    status: not-started
  - id: "02.5"
    title: "Newtype & FFI Type Handling"
    status: not-started
  - id: "02.6"
    title: "Generic Type & Monomorphization Interaction"
    status: not-started
  - id: "02.7"
    title: "Completion Checklist"
    status: not-started
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

- [ ] Create directory `compiler/ori_types/src/triviality/` and file `compiler/ori_types/src/triviality/mod.rs` with the `Triviality` enum and `classify_triviality()` entry point (placed in `ori_types`, NOT `ori_repr`, to avoid circular deps — both `ori_arc` and `ori_repr` depend on `ori_types`). **Prerequisite**: verify `rustc-hash` is in `compiler/ori_types/Cargo.toml` dependencies (confirmed 2026-03-25: present):
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

- [ ] Wire up delegation from both consumers (no duplicate logic allowed):
  - `ori_arc::ArcClassifier::classify_by_tag()` (`compiler/ori_arc/src/classify/mod.rs:152`) — replace body with `ori_types::classify_triviality()` call; keep existing `RefCell<FxHashMap<Idx, ArcClass>>` cache and `RefCell<FxHashSet<Idx>>` cycle detection intact. Map: `Trivial` → `ArcClass::Scalar`, `NonTrivial` → `ArcClass::DefiniteRef`, `Unknown` → `ArcClass::PossibleRef`. Note: `ArcClassifier` already performs transitive classification (it recurses into Option/Result/Tuple/Struct/Enum children), so this delegation **unifies logic** rather than adding new capability — the actual RC elision behavior for compound types like `Option<int>` is unchanged. **Implementation detail**: `ArcClassifier.classify()` (line 89) wraps `classify_by_tag()` with its own cache lookup and cycle detection. After delegation, `classify_triviality()` also has internal cycle detection via its `visiting` set. This double cycle detection is harmless (both agree on the same recursive-type answer), but the `classifying` field on `ArcClassifier` becomes redundant. As an optional cleanup after delegation is verified: remove `classifying: RefCell<FxHashSet<Idx>>` from `ArcClassifier` and simplify `classify()` to cache-lookup → `classify_triviality()` → cache-store.
  - `ori_repr::ReprPlan::is_trivial()` — the `analyze_triviality()` stub in `compiler/ori_repr/src/lib.rs:118` will call `ori_types::classify_triviality()` for each type during `compute_repr_plan()` as a **validation pass** (NOT as the primary computation). The canonical pass (`populate_canonical()`) already records the correct `MachineRepr` with embedded triviality for structs (`StructRepr.trivial`), tuples (`TupleRepr.trivial`), and enums (via `is_trivial_repr()` variant field walk). `ReprPlan::is_trivial()` at `plan/query.rs:96` checks `is_trivial_repr()` on the recorded repr and already returns the correct answer. The `analyze_triviality()` pass asserts that `classify_triviality()` and `is_trivial_repr()` agree for every canonicalized type — any mismatch is a `debug_assert!` failure. The pass does NOT call `set_repr()` to overwrite canonical decisions.

- [ ] Make `TypeInfoStore::is_trivial()` delegate to `ReprPlan::is_trivial()`:
  - WHERE: `compiler/ori_llvm/src/codegen/type_info/store.rs:164` (the `is_trivial()` method) — this currently has its own transitive walk via `classify_trivial()`. After §01 is complete, replace the body to query `ReprPlan`. Until then, the existing walk in `store.rs` remains the implementation.
  - WHERE: `compiler/ori_llvm/src/codegen/type_info/store.rs:181` (the `classify_trivial()` helper) — mark as `// TODO(repr-opt/02): remove when TypeInfoStore delegates to ReprPlan`. Do NOT remove until ReprPlan is live.
  - **[DRIFT]** `compiler/ori_llvm/src/codegen/type_info/store.rs:209` — `TypeInfoStore::classify_trivial()` returns `false` for `TypeInfo::Iterator { .. }`, contradicting `ArcClassifier`'s `Scalar` classification. This drift is currently inert (no production codegen path calls `TypeInfoStore::is_trivial()`), but is resolved when §02.1 installs `classify_triviality()` as the single source of truth.
  - `ReprPlan` caches the result from the triviality pass
  - Codegen never re-computes triviality

- [ ] Add consistency test: for every type that `ArcClassifier` classifies as `Scalar`, `ReprPlan::is_trivial()` must return `true`, and vice versa
- [ ] **Salsa integration:** `Triviality` is NOT a Salsa query. It is a pure function `(Idx, &Pool) -> Triviality` with no mutable state. Caching is handled at the consumer level:
  - `ArcClassifier` already caches via `RefCell<FxHashMap<Idx, ArcClass>>` — the delegation to `classify_triviality()` replaces the body of `classify_by_tag()`, keeping the existing cache
  - `ReprPlan` stores triviality implicitly in the `MachineRepr` recorded by `populate_canonical()` — `StructRepr.trivial`, `TupleRepr.trivial`, and `is_trivial_repr()` for enums. `analyze_triviality()` validates consistency, not overwrites.
  - `TypeInfoStore` delegates to `ReprPlan` (which is already computed) — no Salsa query needed
  - If future JIT hot-reload needs incremental triviality, it recomputes per changed function's types (same model as §01.6)
  - `Triviality` derives `Clone, Copy, PartialEq, Eq, Hash, Debug` — Salsa-compatible if ever wrapped in a query

---

## 02.2 Transitive Walk with Cycle Detection

**File(s):** `compiler/ori_types/src/triviality/mod.rs` (was `triviality.rs` — uses directory module for sibling test file)

The recursive walk must handle all compound types and detect cycles (recursive structs/enums).

- [ ] Implement transitive classification (private helper — not `pub`):
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

- [ ] Write tests in `compiler/ori_types/src/triviality/tests.rs` (sibling convention: `triviality.rs` becomes `triviality/mod.rs` with `#[cfg(test)] mod tests;` at the bottom; test body in `triviality/tests.rs`) covering:

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

- [ ] Add `pub mod triviality;` to `compiler/ori_types/src/lib.rs` — WHERE: after line 27 (`mod type_error;`), before line 28 (`mod unify;`), maintaining alphabetical order. Also add `pub use triviality::{classify_triviality, Triviality};` to the re-export block.
- [ ] Confirm `ori_arc` already depends on `ori_types` (verified 2026-03-25: `compiler/ori_arc/Cargo.toml` line 15); no Cargo.toml edit is needed for the delegation in `classify_by_tag()`
- [ ] Confirm `ori_repr` already depends on `ori_types` (verified 2026-03-25: `compiler/ori_repr/Cargo.toml` line 11); no Cargo.toml edit is needed for `analyze_triviality()` importing `classify_triviality`
- [ ] Verify `cargo c` (check all) succeeds after wiring delegation

---

### 02.2b Implement `analyze_triviality()` Stub Body

**File(s):** `compiler/ori_repr/src/lib.rs` (line 118 — the `analyze_triviality` stub)

The `analyze_triviality()` function in `ori_repr/src/lib.rs:118` is currently a no-op stub. §02 must fill it in. However, its role is narrower than it appears: the canonical pass (`populate_canonical()`) already embeds triviality into `MachineRepr::Struct { trivial }`, `MachineRepr::Tuple { trivial }`, and `MachineRepr::Enum` (via `is_trivial_repr()` variant field walk). So `ReprPlan::is_trivial()` already returns the correct answer for all canonicalized types.

The `analyze_triviality()` pass serves as:
1. **Validation**: Assert that `classify_triviality(idx, pool)` agrees with `is_trivial_repr(repr)` for every type that has a canonical representation. Any disagreement is a bug in either the canonical pass or the classification function.
2. **Gap coverage**: For types skipped by `populate_canonical()` (types with unresolved variables, error types), record a conservative `false` triviality. These types should not reach codegen, but the validation catch is worth having.

- [ ] Implement `analyze_triviality()` body in `compiler/ori_repr/src/lib.rs`:
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
- [ ] Add `ori_types` as a dependency of `ori_repr` for `classify_triviality` — WHERE: `compiler/ori_repr/Cargo.toml` (verify `ori_types` is already listed; it should be from §01)
- [ ] Write a test in `compiler/ori_repr/src/tests.rs` that constructs a Pool with `Option<int>`, `(int, float)`, `struct Point { x: int, y: int }`, and `Result<int, str>`, runs `compute_repr_plan()`, and asserts the validation pass produces zero mismatches

---

## 02.3 ARC Elision in ori_arc Pipeline

**File(s):** `compiler/ori_arc/src/aims/emit_rc/mod.rs` (via `func.var_reprs` / `rc_strategy()`), `compiler/ori_arc/src/classify/mod.rs`, `compiler/ori_arc/src/ir/repr.rs`, `compiler/ori_arc/src/drop/mod.rs`

When the triviality pass marks a type as Trivial, the ARC pipeline must skip ALL RC operations for values of that type. **Note:** `ArcClassifier` already classifies compound types transitively, so trivial compound types like `Option<int>` already get zero RC ops. §02.3 adds regression coverage and verifies this behavior is preserved after §02.1's unification.

- [ ] Verify the AIMS pipeline already correctly handles trivial compound types. The AIMS pipeline gates RC via pre-computed `func.var_reprs` (from `compute_var_reprs()` in step 3 of the pipeline), checking `repr == ValueRepr::Scalar` in `rc_strategy()` (`aims/emit_rc/mod.rs:82`). Since `ArcClassifier` already classifies `Option<int>` as `Scalar` (via transitive recursion), `Option<int>` already gets `ValueRepr::Scalar` and ZERO RC ops today. No change to `aims/emit_rc/` is needed — the delegation in §02.1 unifies the logic but does not change the ARC pipeline's behavior for compound types.
  - Verify: `Option<int>` already gets `ValueRepr::Scalar` from `compute_var_reprs()` (regression test)
  - Verify: `(int, float)` already gets `ValueRepr::Scalar` from `compute_var_reprs()` (regression test)
  - Verify: `Result<int, Ordering>` already gets `ValueRepr::Scalar` from `compute_var_reprs()` (regression test)

- [ ] Verify `compute_var_reprs()` already returns `ValueRepr::Scalar` for trivial compound types:
  - WHERE: `compiler/ori_arc/src/ir/repr.rs:198` — `compute_var_reprs()` function
  - `ValueRepr` has four variants: `Scalar`, `RcPointer`, `Aggregate`, `FatValue`
  - `ArcClassifier` already classifies `Option<int>` as `Scalar` (via transitive recursion at `classify_by_tag()` line 177: `Tag::Option => self.classify(self.pool.option_inner(idx))`). Since `ValueRepr::from_arc_class(Scalar, ...)` returns `Scalar`, `Option<int>` already gets `ValueRepr::Scalar` today.
  - After §02.1 delegation, this behavior is preserved — the delegation unifies logic, not results
  - Verify: add **regression tests** to the existing file `compiler/ori_arc/src/ir/repr/tests.rs` (408 lines — room for additions) asserting `compute_var_reprs()` returns `ValueRepr::Scalar` for `Option<int>`, `(int, float)`, `Result<int, bool>` — these should pass both before and after §02.1

- [ ] Verify `compute_drop_info()` already returns `None` for trivial compound types:
  - WHERE: `compiler/ori_arc/src/drop/mod.rs:130-142` (`compute_drop_info()` function)
  - `compute_drop_info()` returns `None` when `classifier.is_scalar(ty)` is true (line 135). Since `ArcClassifier` already classifies `Option<int>` as `Scalar`, `compute_drop_info(option_int_idx, ...)` already returns `None` today.
  - After §02.1 delegation, this behavior is preserved
  - Note: `compute_fields_drop()` returns `DropKind::Trivial` when no fields need RC — this is only reached for types that ARE `DefiniteRef`/`PossibleRef` (e.g., `[int]` gets `DropKind::Trivial` because its element is scalar, but the list itself is still heap-allocated and needs a drop function for `ori_rc_free`)
  - Verify: add **regression tests** to the existing file `compiler/ori_arc/src/drop/tests.rs` (717 lines — exceeds 500-line limit for production code, but tests are exempt per CLAUDE.md; if further additions push past ~800 lines, consider extracting to `drop/tests/` directory module) asserting `compute_drop_info(option_int_idx, &classifier, &pool)` returns `None` — this should pass both before and after §02.1

---

## 02.4 Drop Function Elision

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/element_fn_gen.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/rc_ops.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/rc_value_traversal.rs`

When `compute_drop_info()` returns `None`, the LLVM emitter must treat that as "no RC-managed heap object here". The current `get_or_generate_drop_fn()` helper already returns a null function pointer in this case; the remaining work is to keep that null from flowing into `ori_rc_dec`, which would otherwise leak the allocation when the refcount hits zero.

**Pre-condition:** `compute_drop_info()` in `compiler/ori_arc/src/drop/mod.rs:130` already returns `None` when `classifier.is_scalar(ty)` is true (line 135). Since `ArcClassifier` already classifies trivially-composed compound types (like `Option<int>`) as `Scalar`, `compute_drop_info()` already returns `None` for them today. After §02.1 unifies the classification, this behavior is preserved. `get_or_generate_drop_fn()` in `compiler/ori_llvm/src/codegen/arc_emitter/element_fn_gen.rs:26` already reflects this by returning `const_null_ptr()` when `compute_drop_info()` returns `None`. §02.4's job is to add regression coverage for that behavior and verify the RC emission sites properly handle the null (no `ori_rc_dec` calls emitted for trivial compound types).

- [ ] Add a regression test around `get_or_generate_drop_fn()` (`compiler/ori_llvm/src/codegen/arc_emitter/element_fn_gen.rs`) asserting that trivial types return a null drop-function pointer and do not populate `drop_fn_cache`
- [ ] Audit `ori_rc_dec` call emission sites — verify that when `ArcClassifier` classifies a type as `Scalar`, NO `RcDec` instruction is emitted for it (the AIMS pipeline gates this via `rc_strategy()` at `aims/emit_rc/mod.rs:75-82`, which checks `repr == ValueRepr::Scalar` and returns `None` for the strategy). Specifically audit:
  - `compiler/ori_llvm/src/codegen/arc_emitter/rc_ops.rs:88` (`emit_rc_dec`) — verify it never receives a `Scalar`-typed variable (the ARC IR should not contain `RcDec` for scalars)
  - `compiler/ori_llvm/src/codegen/arc_emitter/rc_value_traversal.rs` (253 lines) — verify field traversal skips scalar-typed fields in aggregates
  - `compiler/ori_llvm/src/codegen/arc_emitter/element_fn_gen.rs:67-70` (`get_or_generate_elem_dec_fn`) — already returns null for scalar elements (confirmed line 69: `if self.classifier.is_scalar(element_type)`)
  - **Expected outcome**: all three sites already handle scalars correctly; this is a verification audit, not a code change
- [ ] Verify: `ORI_DUMP_AFTER_LLVM=1 ori build trivial_test.ori` should NOT contain `_ori_drop$` functions for `Option<int>`, `(int, float)`, `Result<int, bool>`
- [ ] Verify: `ORI_LOG=ori_llvm=debug ori build trivial_test.ori 2>&1 | grep "drop"` shows no drop function generation for trivial types

---

## 02.5 Newtype & FFI Type Handling

**File(s):** `compiler/ori_types/src/triviality/mod.rs` (algorithm), `compiler/ori_types/src/registry/types/mod.rs` (reference for TypeKind::Newtype)

Newtypes and FFI types are not separate Tag variants — they use `Tag::Named` and resolve via `pool.resolve_fully()`. This section documents the handling and adds targeted tests.

**Newtypes:**
- `type UserId = int` creates a `Tag::Named` entry in the Pool with a resolution to `Idx::INT`
- `resolve_fully()` follows the Named→concrete chain transparently
- The `TypeRegistry` stores `TypeKind::Newtype { underlying }` for semantic purposes (e.g., `.inner` access), but triviality classification only needs the Pool-level resolution
- No special case needed in `classify_recursive()` — the `Tag::Named` arm already handles this

- [ ] Verify: `type UserId = int` → `resolve_fully()` → `Tag::Int` → `Trivial`
- [ ] Verify: `type Wrapper = [int]` → `resolve_fully()` → `Tag::List` → `NonTrivial`
- [ ] Verify: nested newtype `type Id = UserId` → `resolve_fully()` chains to `Tag::Int` → `Trivial`
- [ ] Edge case: newtype wrapping a generic parameter that hasn't been monomorphized → the Named won't resolve → `Unknown` (typeck bug if reached in production)

**FFI types (CPtr, JsValue, c_int, etc.):**
- `CPtr` is defined as a named type in the FFI prelude, not a Pool primitive
- At the Pool level: `Tag::Named("CPtr")` → resolves to an opaque pointer representation
- `CPtr` has no RC header and no heap allocation semantics → should classify as Trivial
- `JsValue` and `JsPromise<T>` are WASM-target types, opaque handles → classify as Trivial (no Ori-managed RC)
- C numeric types (`c_int`, `c_char`, `c_float`, etc.) resolve to primitive numeric types → Trivial

- [ ] Verify: `CPtr` → `resolve_fully()` → opaque pointer → Trivial
- [ ] Verify: `c_int` → `resolve_fully()` → `Tag::Int` (or appropriate numeric) → Trivial
- [ ] Verify: `Option<CPtr>` → Trivial (CPtr inner is trivial)
- [ ] Add test for FFI struct containing only C types → Trivial

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

- [ ] Verify: `Pair<int>` (struct with fields `int, int`) → Trivial
- [ ] Verify: `Pair<str>` (struct with fields `str, str`) → NonTrivial
- [ ] Verify: `Option<Pair<int>>` → Trivial (recursion through Option inner to Pair struct to Int fields)
- [ ] Verify: unresolved `Pair<T>` where T is still a Var → Unknown (not an error — just conservative)
- [ ] Verify: `Result<Pair<int>, Pair<float>>` → Trivial (both arms trivial)

**No monomorphization-time specialization needed:** Unlike integer narrowing (§04) which may produce different MachineRepr for `Pair<int>` vs `Pair<float>` (field widths differ), triviality is a simple binary property that falls out naturally from the recursive walk. Each concrete instantiation gets its own `Idx` and its own triviality result. No special handling required.

---

## 02.R Third Party Review Findings

- [ ] `[TPR-02-001][medium]` `compiler/ori_types/src/triviality/tests.rs:1` — The new triviality module lands without the recursive-type and special-tag matrix that §02 and the repo rules require.
  Evidence: The new tests cover many happy-path primitives and containers, but there is still no regression coverage for recursive structs/enums, `BoundVar`/`RigidVar`, `Borrowed`, `Scheme`/`Projection`/`ModuleNs`/`Infer`/`SelfType`, `Applied`/`Alias`, or the FFI cases called out in §02.5; `rg` over the file only finds “cycle” in the header comment.
  Impact: The riskiest correctness branches in `classify_recursive()` — especially cycle detection and conservative `Unknown` fallbacks — remain unpinned, so the current unstaged implementation does not yet meet the plan’s required TDD/matrix standard even though the core algorithm is now in tree.
  Required plan update: Add the missing recursive, special-tag, and FFI matrix tests before treating §02.1/§02.2 as actively underway, then update the checklist items that this live work has already started.

---

## 02.7 Completion Checklist

**Algorithm & unification:**
- [ ] `//!` module doc on `triviality/mod.rs` explaining purpose and ownership
- [ ] Single `classify_triviality()` function in `ori_types` is the sole source of truth
- [ ] `Triviality` enum derives `Clone, Copy, PartialEq, Eq, Hash, Debug` (Salsa-compatible)
- [ ] `classify_recursive()` and `merge_triviality()` are private (not `pub`)
- [ ] `ArcClassifier` delegates to `ori_types::classify_triviality()` — no duplicate logic
- [ ] `TypeInfoStore::is_trivial()` delegates to `ReprPlan` — no duplicate logic
- [ ] `classify_triviality()` handles `Idx::NONE` sentinel (returns Trivial, matching ArcClassifier)
- [ ] `analyze_triviality()` stub body implemented in `compiler/ori_repr/src/lib.rs` (see §02.2b)

**§01.8 Phase B completion (EXPLICIT DELIVERABLE of §02):**
- [ ] Add `repr_plan: Option<&'tcx ori_repr::ReprPlan>` field to `TypeInfoStore` struct (`compiler/ori_llvm/src/codegen/type_info/store.rs:37`). **Approach**: add a `new_with_plan(pool, repr_plan)` constructor alongside the existing `new(pool)` (which passes `None`). This avoids updating ~100 test call sites that don't need `ReprPlan`. The two production call sites must use `new_with_plan()`: `compiler/ori_llvm/src/evaluator/compile.rs:153` (JIT path) and `compiler/oric/src/commands/codegen_pipeline.rs:263` (AOT path). `TypeLayoutResolver` already follows this pattern (see `compiler/ori_llvm/src/codegen/type_info/mod.rs:76`).
- [ ] `TypeInfoStore::is_trivial()` body replaced: delegate to `self.repr_plan.map_or_else(|| self.classify_trivial(idx), |p| p.is_trivial(idx))` — when `ReprPlan` is available, use it; fall back to existing walk when not
- [ ] `TypeInfoStore::classify_trivial()` helper marked dead and removed (or gated `#[cfg(test)]` only)
- [ ] `triviality_cache` and `classifying_trivial` fields removed from `TypeInfoStore` struct definition in `compiler/ori_llvm/src/codegen/type_info/store.rs`
- [ ] Remove TODO comments at lines 49-50 and 57-58 of `store.rs` that reference `repr-opt §02` (they become stale after completion)
- [ ] §01.8 Phase B status updated from `not-started` to `complete` in `section-01-repr-ir.md` YAML header
- [ ] Validation test: `assert_eq!(repr_plan.is_trivial(idx), type_info_store.is_trivial(idx))` for all types in a representative Pool (the Phase B validation described in §01.8)
- [ ] **Matrix testing for Phase B code change**: after replacing `is_trivial()` body, run full test suite (`timeout 150 ./test-all.sh`) in both debug and release; any failure = regression from the delegation. The Phase B change MUST be behavior-preserving — `ReprPlan::is_trivial()` and the old `classify_trivial()` must agree for all types that reach codegen.

**Tag coverage (exhaustive):**
- [ ] All 12 primitive tags classified (Int, Float, Bool, Str, Char, Byte, Unit, Never, Error, Duration, Size, Ordering)
- [ ] All 7 simple container tags classified (List, Option, Set, Channel, Range, Iterator, DoubleEndedIterator)
- [ ] All 3 two-child container tags classified (Map, Result, Borrowed)
- [ ] All 4 complex type tags classified (Function, Tuple, Struct, Enum)
- [ ] All 3 named type tags classified (Named, Applied, Alias) — via resolve_fully
- [ ] All 3 type variable tags classified (Var, BoundVar, RigidVar) — Unknown
- [ ] All 5 special tags classified (Scheme, Projection, ModuleNs, Infer, SelfType) — Unknown

**Newtype & FFI:**
- [ ] `type UserId = int` → Trivial (resolves via Named)
- [ ] `type Name = str` → NonTrivial (resolves via Named)
- [ ] CPtr / c_int FFI types → Trivial (resolve via Named to opaque/primitive)

**Generic types:**
- [ ] Monomorphized generic struct with scalar fields → Trivial
- [ ] Monomorphized generic struct with heap fields → NonTrivial
- [ ] Unresolved type variable in generic → Unknown (conservative, not error)

**ARC pipeline integration:**
- [ ] `Option<int>`, `(int, float, bool)`, `Result<int, Ordering>` generate ZERO `ori_rc_inc`/`ori_rc_dec` calls in LLVM IR
- [ ] No `_ori_drop$` functions emitted for trivial compound types
- [ ] `compute_var_reprs()` returns `ValueRepr::Scalar` for all trivially-classified types (already true today via `ArcClassifier` transitive recursion — add regression tests)
- [ ] `compute_drop_info()` returns `None` for all trivially-classified types (already true today via `ArcClassifier` — add regression tests)

**Consistency & safety:**
- [ ] Consistency test: `ArcClassifier` and `ReprPlan` agree on every type in the Pool
- [ ] Consistency test: for a Pool with 50+ diverse types, no classification disagreement between old and new code paths
- [ ] Consistency test: `classify_triviality()` agrees with `is_trivial_repr()` for every canonicalized type in the Pool (the `analyze_triviality()` validation from §02.2b passes with zero mismatches)

**Downstream feed-forward (§08, §09):**
- [ ] §08 can call `classify_triviality(idx, pool)` to skip escape analysis for trivial types (verify import compiles)
- [ ] §09 can call `ReprPlan::is_trivial(idx)` to set `RcStrategy::None` for trivial types (verify query works)

**TDD ordering (MANDATORY):**
1. Write ALL tests in `compiler/ori_types/src/triviality/tests.rs` first (see §02.2 test list — all 40+ cases)
2. Run `cargo test -p ori_types` — all new tests must FAIL (unimplemented module)
3. Implement `classify_triviality()` in `triviality/mod.rs`
4. Verify tests pass without modification
5. Only then proceed to wiring delegation in §02.1 consumers

**Test matrix dimensions:** Type tag (all 37 Tag variants) × Classification outcome (Trivial / NonTrivial / Unknown) × Resolution path (primitive fast-path / compound recursive / Named resolution / cycle detection)

**Test suites:**
- [ ] Unit tests in `compiler/ori_types/src/triviality/tests.rs` (new file) — all 37 tag variants covered; this is the primary test matrix
- [ ] Integration tests in `compiler/ori_arc/src/classify/tests.rs` (existing, 501 lines) — ArcClassifier delegation verified; specifically add a test asserting `Iterator<int>` classified as `Scalar` (regression guard for the live drift). No existing test covers Iterator classification (confirmed 2026-03-25).
- [ ] Integration tests in `compiler/ori_arc/src/ir/repr/tests.rs` (existing, 408 lines) — `compute_var_reprs()` regression tests for trivial compound types
- [ ] Integration tests in `compiler/ori_arc/src/drop/tests.rs` (existing, 717 lines) — `compute_drop_info()` regression tests for trivial compound types
- [ ] Integration tests in `compiler/ori_llvm/tests/aot/` — LLVM IR verified for trivial compound types; verify `ORI_DUMP_AFTER_LLVM=1` shows zero `ori_rc_*` calls for `Option<int>`, `(int, float)`, `struct Point { x: int, y: int }`
- [ ] Regression test: write a test that creates 100K `Option<int>` values in a loop and verifies (via `ORI_CHECK_LEAKS=1`) exactly 0 RC allocations. Note: this test should already pass today (ArcClassifier already classifies `Option<int>` as Scalar), so it serves as a **regression guard** against future breakage, not a semantic pin for new behavior.
- [ ] Semantic pin test for Iterator unification: write a test verifying that `TypeInfoStore::is_trivial()` (or its ReprPlan replacement) returns `true` for `Iterator<int>` — this is the one classification that actually CHANGES in §02 (TypeInfoStore currently returns `false` for iterators)
- [ ] Ori spec tests in `tests/spec/` — at least one `.ori` file exercising trivial compound types end-to-end
- [ ] `./test-all.sh` green
- [ ] `./llvm-test.sh` green (run after debug build AND release build — `cargo b --release`)
- [ ] `./diagnostics/valgrind-aot.sh` clean (no leaks introduced by elision)
- [ ] `./clippy-all.sh` green

**Files created or modified:**

- [ ] Created: `compiler/ori_types/src/triviality/mod.rs` (new module — algorithm + `#[cfg(test)] mod tests;`)
- [ ] Created: `compiler/ori_types/src/triviality/tests.rs` (new — unit tests, sibling convention)
- [ ] Modified: `compiler/ori_types/src/lib.rs` (add `pub mod triviality;`)
- [ ] Modified: `compiler/ori_arc/src/classify/mod.rs` (delegate to `ori_types::classify_triviality`)
- [ ] Modified: `compiler/ori_arc/src/classify/tests.rs` (add delegation consistency tests)
- [ ] NOT modified: `compiler/ori_arc/src/ir/repr.rs` — `compute_var_reprs()` calls `classifier.arc_class()` which flows through `ArcClassifier::classify()` → `classify_by_tag()`. The delegation change in `classify/mod.rs` is sufficient; `repr.rs` needs no changes. Add regression tests only.
- [ ] NOT modified: `compiler/ori_arc/src/drop/mod.rs` — `compute_drop_info()` calls `classifier.is_scalar()` which flows through the same ArcClassifier. No changes needed; add regression tests only.
- [ ] NOT modified: `compiler/ori_arc/src/rc_insert/mod.rs` — this module only handles arg ownership annotation; RC insertion is the AIMS pipeline's job via pre-computed `func.var_reprs` (which derives from `ArcClassifier`)
- [ ] Modified: `compiler/ori_llvm/src/codegen/type_info/store.rs` (delegate `is_trivial` to ReprPlan — this IS §01.8 Phase B, which is §02's deliverable)
- [ ] NOT modified: `compiler/ori_llvm/src/codegen/arc_emitter/element_fn_gen.rs` — `get_or_generate_drop_fn()` already returns null for scalar types (via `compute_drop_info()` returning `None`). The AIMS pipeline never emits `RcInc`/`RcDec` for Scalar-classified types, so the LLVM emitter never requests drop functions for them. Add regression tests only.
- [ ] NOT modified: `compiler/ori_llvm/src/codegen/arc_emitter/drop_gen.rs` — same reasoning; drop functions are only generated for types that have `RcDec` instructions in the ARC IR, which excludes Scalar types.
- [ ] Modified: `compiler/ori_repr/src/lib.rs` (implement `analyze_triviality()` stub body — validation pass)
- [ ] Modified: `compiler/ori_repr/src/tests.rs` (add triviality validation tests)

**Exit Criteria:** `ori build` on a program using `Option<int>`, `(int, float)`, and `struct Point { x: int, y: int }` produces LLVM IR with zero `ori_rc_*` calls for these types, verified by `grep -c "ori_rc" output.ll` returning 0 for trivial-only programs. Note: this should already pass today (ArcClassifier already handles these types transitively). The exit criteria verify that §02's unification preserves this behavior and adds the iterator classification fix.
