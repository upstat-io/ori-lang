---
section: "02"
title: "Transitive Triviality & ARC Elision"
status: not-started
reviewed: false
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
1. `ori_arc::ArcClassifier` (re-exported at crate root; internal module `ori_arc::classify` is `pub(crate)`) — used during ARC IR lowering, classifies as `Scalar`/`DefiniteRef`/`PossibleRef`
2. `ori_llvm::codegen::type_info::TypeInfoStore::is_trivial()` — used during LLVM codegen, walks type tree

These MUST agree, but they use different algorithms and data structures, and they **currently disagree** on iterators: `ArcClassifier` classifies `Iterator`/`DoubleEndedIterator` as `Scalar` (Box-allocated, no RC header), while `TypeInfoStore::is_trivial()` classifies them as non-trivial (heap-backed). Unifying them ensures no redundant RC operations on types that can never hold heap references.

**Reference implementations:**
- **Swift** `lib/SIL/SILType.cpp`: `isTrivial()` walks type structure recursively, caches results
- **Lean4** `src/Lean/Compiler/IR/RC.lean`: `VarInfo.isPossibleRef` / `isDefiniteRef` — two-bit classification
- **ori_arc** `compiler/ori_arc/src/classify/mod.rs`: `ArcClassifier` with `FxHashMap` cache + cycle detection

**Depends on:** §01 (ReprPlan stores triviality decisions).

**§01 dependency scope:** This section requires two things from §01:
1. `ReprPlan::is_trivial()` query method (§01.4) — so codegen can check triviality
2. `ReprPlan::set_repr()` builder method (§01.2) — so triviality pass can record decisions

If §01 is incomplete, the core algorithm (`classify_triviality()` in `ori_types`) can still be implemented and tested standalone. The `ArcClassifier` delegation (02.1) and the ARC elision (02.3) do NOT require `ReprPlan` — they only require the shared `classify_triviality()` function. The `ReprPlan` integration (02.1 bullet 3, "Make TypeInfoStore delegate") is the only step that truly blocks on §01 completion. Mark it as deferred if §01 is not ready.

**Evaluator impact:** `ori_eval` does NOT use triviality classification and is NOT affected by this section. The evaluator operates at the value level with Rust-native reference counting (no `ori_rc_*` calls). All changes in this section are confined to `ori_types`, `ori_arc`, and `ori_llvm`.

---

## 02.1 Unify Triviality Classification

**File(s):** `compiler/ori_types/src/triviality/mod.rs` (NOT `ori_repr` — avoids circular dep since both `ori_arc` and `ori_repr` depend on `ori_types`), `compiler/ori_arc/src/classify/mod.rs`

Today, `ArcClassifier` and `TypeInfoStore::is_trivial()` duplicate logic. We need a single source of truth.

- [ ] Create `compiler/ori_types/src/triviality/mod.rs` with the `Triviality` enum and `classify_triviality()` entry point (placed in `ori_types`, NOT `ori_repr`, to avoid circular deps — both `ori_arc` and `ori_repr` depend on `ori_types`):
  ```rust
  //! Transitive triviality classification for type-level ARC elision.
  //!
  //! A type is *trivial* when it (and all its transitive children) contain
  //! no heap references requiring ARC operations. Trivial types can skip
  //! all `ori_rc_inc`/`ori_rc_dec` calls in generated code.
  //!
  //! Single source of truth: both `ori_arc::ArcClassifier` and
  //! `ori_llvm::TypeInfoStore` delegate to [`classify_triviality`].

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
      let mut visiting = FxHashSet::default();
      classify_recursive(idx, pool, &mut visiting)
  }
  ```

- [ ] Wire up delegation from both consumers (no duplicate logic allowed):
  - `ori_arc::ArcClassifier::classify()` delegates to `ori_types::classify_triviality()`, mapping: `Trivial` → `ArcClass::Scalar`, `NonTrivial` → `ArcClass::DefiniteRef`, `Unknown` → `ArcClass::PossibleRef`
  - `ori_repr::ReprPlan::is_trivial()` delegates to `ori_types::classify_triviality()`

- [ ] Make `TypeInfoStore::is_trivial()` delegate to `ReprPlan::is_trivial()`:
  - `ReprPlan` caches the result from the triviality pass
  - Codegen never re-computes triviality

- [ ] Add consistency test: for every type that `ArcClassifier` classifies as `Scalar`, `ReprPlan::is_trivial()` must return `true`, and vice versa
- [ ] **Salsa integration:** `Triviality` is NOT a Salsa query. It is a pure function `(Idx, &Pool) -> Triviality` with no mutable state. Caching is handled at the consumer level:
  - `ArcClassifier` already caches via `RefCell<FxHashMap<Idx, ArcClass>>` — the delegation to `classify_triviality()` replaces the body of `classify_by_tag()`, keeping the existing cache
  - `ReprPlan` stores triviality as a field in `ReprDecision` — computed once during `analyze_triviality()`, never recomputed
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
          Tag::Function => Triviality::NonTrivial, // closures capture heap refs
          // Range is currently always int/float (both trivial). If Range<T>
          // ever supports non-scalar T, this must recurse into the element.
          Tag::Range => {
              let elem = pool.range_elem(resolved);
              classify_recursive(elem, pool, visiting)
          }
          // All other tags handled in fast-path above; this arm is
          // unreachable after the exhaustive early returns.
          _ => Triviality::Unknown,
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

## 02.3 ARC Elision in ori_arc Pipeline

**File(s):** `compiler/ori_arc/src/rc_insert/mod.rs`, `compiler/ori_arc/src/classify/mod.rs`

When the triviality pass marks a type as Trivial, the ARC pipeline must skip ALL RC operations for values of that type.

- [ ] In `rc_insert`, check triviality before inserting `RcInc`/`RcDec`:
  - If the variable's type is `Trivial` in the `ReprPlan`, skip RC insertion entirely
  - This must work for compound types: `Option<int>` variables must skip RC even though `Option` itself has a generic form that might need RC

- [ ] In `compute_var_reprs()` (`compiler/ori_arc/src/ir/repr.rs`), set `ValueRepr::Scalar` for all trivial types:
  - `ValueRepr` has four variants: `Scalar`, `RcPointer`, `Aggregate`, `FatValue`
  - Currently, `Option<T>` always gets `ValueRepr::Aggregate` regardless of T (driven by `ValueRepr::from_arc_class()` + `from_ref_tag()`)
  - With triviality, `Option<int>` should get `ValueRepr::Scalar` (no RC fields)
  - The fix point is `ValueRepr::from_arc_class()`: when `ArcClass::Scalar`, result is already `Scalar`; the real change is making `ArcClassifier` return `Scalar` for trivially-composed compound types

- [ ] Update `drop/mod.rs` — `compute_drop_info()` should return `None` for trivial types:
  - Currently, `compute_drop_info()` returns `None` only when `classifier.is_scalar(ty)` is true (line 135). For `PossibleRef` types whose children are all scalar, it returns `Some(DropInfo { kind: DropKind::Trivial })` — a trivial drop that still generates a drop function (just `ori_rc_free` + ret).
  - With unified triviality, these should return `None` (no drop function needed at all)
  - Note: `compute_fields_drop()` already returns `DropKind::Trivial` (not `DropKind::Fields([])`) when no fields need RC; the issue is that `compute_drop_info` wraps it in `Some(DropInfo)` instead of returning `None`

---

## 02.4 Drop Function Elision

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/drop_gen.rs`

When `compute_drop_info()` returns `None`, the LLVM drop function generator must NOT emit a drop function. This saves code size and eliminates dead function definitions.

- [ ] In `get_or_generate_drop_fn()` (`compiler/ori_llvm/src/codegen/arc_emitter/element_fn_gen.rs`) and `generate_drop_fn()` (`drop_gen.rs`), return a null function pointer for trivial types
- [ ] In `ori_rc_dec` call sites, skip the call entirely when drop_fn is null
- [ ] Verify: `ORI_LOG=ori_llvm=debug ori build` should NOT emit `_ori_drop$` functions for `Option<int>`, `(int, float)`, `Result<int, bool>`, etc.

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

## 02.7 Completion Checklist

**Algorithm & unification:**
- [ ] `//!` module doc on `triviality/mod.rs` explaining purpose and ownership
- [ ] Single `classify_triviality()` function in `ori_types` is the sole source of truth
- [ ] `Triviality` enum derives `Clone, Copy, PartialEq, Eq, Hash, Debug` (Salsa-compatible)
- [ ] `classify_recursive()` and `merge_triviality()` are private (not `pub`)
- [ ] `ArcClassifier` delegates to `ori_types::classify_triviality()` — no duplicate logic
- [ ] `TypeInfoStore::is_trivial()` delegates to `ReprPlan` — no duplicate logic
- [ ] `classify_triviality()` handles `Idx::NONE` sentinel (returns Trivial, matching ArcClassifier)

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
- [ ] `compute_var_reprs()` returns `ValueRepr::Scalar` for all trivially-classified types
- [ ] `compute_drop_info()` returns `None` (not `Some(DropKind::Trivial)`) for trivially-classified types

**Consistency & safety:**
- [ ] Consistency test: `ArcClassifier` and `ReprPlan` agree on every type in the Pool
- [ ] Consistency test: for a Pool with 50+ diverse types, no classification disagreement between old and new code paths

**Test suites:**
- [ ] Unit tests in `compiler/ori_types/src/triviality/tests.rs` — all tag variants covered
- [ ] Integration tests in `compiler/ori_arc/src/classify/tests.rs` — ArcClassifier delegation verified
- [ ] Integration tests in `compiler/ori_llvm/tests/aot/` — LLVM IR verified for trivial compound types
- [ ] Ori spec tests in `tests/spec/` — at least one `.ori` file exercising trivial compound types end-to-end
- [ ] `./test-all.sh` green
- [ ] `./llvm-test.sh` green
- [ ] `./diagnostics/valgrind-aot.sh` clean (no leaks introduced by elision)
- [ ] `./clippy-all.sh` green

**Files created or modified:**

- [ ] Created: `compiler/ori_types/src/triviality/mod.rs` (new module — algorithm + `#[cfg(test)] mod tests;`)
- [ ] Created: `compiler/ori_types/src/triviality/tests.rs` (new — unit tests, sibling convention)
- [ ] Modified: `compiler/ori_types/src/lib.rs` (add `pub mod triviality;`)
- [ ] Modified: `compiler/ori_arc/src/classify/mod.rs` (delegate to `ori_types::classify_triviality`)
- [ ] Modified: `compiler/ori_arc/src/classify/tests.rs` (add delegation consistency tests)
- [ ] Modified: `compiler/ori_arc/src/ir/repr.rs` (`compute_var_reprs` uses unified classification)
- [ ] Modified: `compiler/ori_arc/src/drop/mod.rs` (`compute_drop_info` returns None for trivial)
- [ ] Modified: `compiler/ori_arc/src/rc_insert/mod.rs` (skip RC for trivial types) — only if §01 ReprPlan is available; otherwise ArcClassifier delegation alone achieves RC elision
- [ ] Modified: `compiler/ori_llvm/src/codegen/type_info/store.rs` (delegate `is_trivial` to ReprPlan — deferred until §01 complete)
- [ ] Modified: `compiler/ori_llvm/src/codegen/arc_emitter/element_fn_gen.rs` (null drop fn for trivial)
- [ ] Modified: `compiler/ori_llvm/src/codegen/arc_emitter/drop_gen.rs` (skip generation for trivial)

**Exit Criteria:** `ori build` on a program using `Option<int>`, `(int, float)`, and `struct Point { x: int, y: int }` produces LLVM IR with zero `ori_rc_*` calls for these types, verified by `grep -c "ori_rc" output.ll` returning 0 for trivial-only programs.
