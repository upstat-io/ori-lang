---
section: "05"
title: Type Classification for ARC
status: not-started
goal: Classify every type as Scalar/PossibleRef/DefiniteRef so ARC analysis can skip trivial types entirely
sections:
  - id: "05.1"
    title: ArcClass Enum & Type Rules
    status: not-started
  - id: "05.2"
    title: Transitive Classification
    status: not-started
  - id: "05.3"
    title: Pool Integration
    status: not-started
---

# Section 05: Type Classification for ARC

**Status:** Not Started
**Goal:** Before any ARC analysis runs, classify every type so we know which ones need reference counting and which can be skipped entirely. This eliminates unnecessary RC operations on scalars -- a huge performance win.

**Crate:** `ori_arc` (no LLVM dependency). This is the `ArcClassification` trait referenced in Section 01. It operates purely on `Pool`/`Idx` — no LLVM types involved. The `TypeInfo` enum in `ori_llvm` (Section 01) queries this classification when deciding whether to emit retain/release calls.

**Reference compilers:**
- **Lean 4** `src/Lean/Compiler/IR/Basic.lean` -- `isScalar`, `isPossibleRef`, `isDefiniteRef` on IRType
- **Koka** `src/Backend/C/Parc.hs` -- `ValueRepr` with scan fields count
- **Swift** `include/swift/SIL/SILValue.h` -- OwnershipKind five-value lattice (Any/Unowned/Owned/Guaranteed/None)

**Key design principle — Monomorphized classification (from Lean 4):**
Classification runs on **concrete types after type parameter substitution**. Generic types like `option[T]` are never classified directly — only their concrete instantiations are classified. This means:
- `option[int]` is classified as **Scalar** (tag + int, no heap pointer)
- `option[str]` is classified as **DefiniteRef** (contains a DefiniteRef field)
- `option[T]` where `T` is an unresolved type variable is **PossibleRef** (conservative)

This is strictly more precise than classifying `option[T]` as a fixed category. The `PossibleRef` class is only used when a type variable remains unresolved — which should only happen in generic function bodies before monomorphization.

**Monomorphization happens BEFORE ARC IR lowering.** The ARC pipeline receives only concrete, monomorphized functions — all type parameters have been substituted with concrete types. This means `PossibleRef` should never be encountered during ARC analysis. It is kept as a debug safety net: add `debug_assert!(class != ArcClass::PossibleRef)` at ARC IR lowering entry points. If `PossibleRef` is encountered after monomorphization, this indicates a compiler bug (a type variable leaked through monomorphization).

**Pool accessibility:** `ori_arc` depends on `ori_types` (which exports `Pool` publicly), so all Pool inherent methods are accessible from `ori_arc`. This is the expected dependency path.

---

## 05.1 ArcClass Enum & Type Rules

```rust
/// ARC classification for a type.
///
/// Determines whether values of this type need reference counting.
/// This classification is the foundation for all ARC optimization.
///
/// Inspired by Lean 4's three-way classification methods
/// (`isScalar`, `isPossibleRef`, `isDefiniteRef` on `IRType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcClass {
    /// No reference counting needed. The value is purely stack/register.
    /// Examples: int, float, bool, char, byte, unit, duration, size, ordering.
    Scalar,

    /// Definitely contains a reference-counted heap pointer.
    /// Every value of this type needs retain/release.
    /// Examples: str, [T], {K: V}, set[T].
    DefiniteRef,

    /// Might contain a reference-counted pointer depending on unresolved type variables.
    /// Conservatively treated as needing RC. Only used when type variables remain
    /// unresolved (before monomorphization). After monomorphization, every type
    /// classifies as either Scalar or DefiniteRef.
    /// Examples: generic T, unresolved type variables.
    PossibleRef,
}

/// Classification rules by Ori type (monomorphized — concrete types only):
///
/// | Type | ArcClass | Reason |
/// |------|----------|--------|
/// | int, float, bool, char, byte | Scalar | Pure value types |
/// | unit | Scalar | Zero-size type (single value) |
/// | never | Scalar | Uninhabited — no values exist at runtime, code unreachable |
/// | duration, size, ordering | Scalar | Wrapped integers |
/// | str | DefiniteRef | Heap-allocated string data |
/// | [T] | DefiniteRef | Heap-allocated array |
/// | {K: V} | DefiniteRef | Heap-allocated hash table |
/// | set[T] | DefiniteRef | Heap-allocated set |
/// | chan<T> | DefiniteRef | Heap-allocated channel |
/// | (P) -> R | DefiniteRef | Heap-allocated closure (see note below) |
/// | option[int] | Scalar | All fields scalar (tag + int) |
/// | option[str] | DefiniteRef | Contains DefiniteRef field |
/// | result[int, int] | Scalar | All fields scalar |
/// | result[str, int] | DefiniteRef | Contains DefiniteRef field |
/// | (int, float) | Scalar | All elements scalar |
/// | (int, str) | DefiniteRef | Contains DefiniteRef element |
/// | struct { all scalar fields } | Scalar | Transitively scalar |
/// | struct { any ref field } | DefiniteRef | Contains ref field |
/// | enum { all scalar variants } | Scalar | All variant payloads scalar |
/// | enum { any ref variant } | DefiniteRef | Contains ref variant |
/// | error | Scalar | Should not reach codegen (type-checker error) |
/// | range[T] (T: Scalar) | Scalar | All fields scalar (start, end, step) |
/// | range[T] (T: DefiniteRef) | DefiniteRef | Contains DefiniteRef field |
/// | T (type variable) | PossibleRef | Unresolved — conservative |
///
/// Note: Generic types (option[T], result[T, E], etc.) are NEVER classified
/// directly. Only concrete instantiations after monomorphization are classified.
/// The PossibleRef class exists solely for unresolved type variables.
///
/// **Closure type classification trade-off:** No-capture closures technically
/// need no RC (their env_ptr is null, no heap allocation). However, the closure
/// *type* `(P) -> R` does not encode whether captures exist — that is a property
/// of the specific closure *value*, not the type. Since classification operates
/// on types (not values), all closure types are conservatively classified as
/// DefiniteRef. A future optimization could use a separate "known-no-capture"
/// fast path at the value level, but the type-level classification remains
/// DefiniteRef for soundness.
```

- [ ] Define `ArcClass` enum
- [ ] Implement classification for all primitive types
- [ ] Implement classification for collection types
- [ ] Implement classification for type variables (conservative: PossibleRef)

## 05.2 Transitive Classification

For compound types, classification depends on contained types. Because classification is **monomorphized**, compound types containing only concrete scalar fields classify as Scalar — no PossibleRef ambiguity.

```rust
/// Compute ArcClass transitively for compound types.
///
/// After monomorphization, all field types are concrete, so the result
/// is always either Scalar or DefiniteRef (never PossibleRef, unless
/// an unresolved type variable leaked through — which is a bug).
fn classify_compound(fields: &[Idx], pool: &Pool) -> ArcClass {
    let mut has_ref = false;
    let mut has_possible = false;

    for &field_idx in fields {
        match classify(field_idx, pool) {
            ArcClass::Scalar => {} // doesn't change anything
            ArcClass::DefiniteRef => {
                has_ref = true;
            }
            ArcClass::PossibleRef => {
                // Should only happen if an unresolved type variable leaked.
                // In monomorphized code, this is a bug. Treat conservatively.
                has_possible = true;
            }
        }
    }

    if has_ref {
        ArcClass::DefiniteRef  // contains a definite ref
    } else if has_possible {
        ArcClass::PossibleRef  // unresolved type variable (should not happen post-mono)
    } else {
        ArcClass::Scalar  // all fields are scalar
    }
}
```

Key insight from monomorphization:
- `option[int]` has fields `[i8, int]` — both Scalar, so `option[int]` is **Scalar**
- `option[str]` has fields `[i8, str]` — `str` is DefiniteRef, so `option[str]` is **DefiniteRef**
- `(int, float, bool)` — all Scalar, so the tuple is **Scalar**
- `(int, [str])` — `[str]` is DefiniteRef, so the tuple is **DefiniteRef**

- [ ] Implement transitive classification for tuples
- [ ] Implement transitive classification for structs
- [ ] Implement transitive classification for enums (union of variant classifications)
- [ ] Handle recursive types (memoize during traversal)
- [ ] Verify monomorphization: after type substitution, no PossibleRef in concrete code

## 05.3 Pool Integration

**Pool flattening prerequisite:** The `ArcClassification` trait implementation requires the same Pool flattening prerequisite as Section 01.5. Specifically, Pool must expose `struct_fields()` and `enum_variants()` accessor methods for transitive classification of user-defined types. Without these methods, `arc_class()` cannot inspect struct field types or enum variant payloads to determine whether a compound type is Scalar or DefiniteRef. Cross-reference Section 01.5 for the full Pool flattening refactor description (~750-1100 lines).

**Pool extra array layout for compound types:** The Pool stores compound type information in a flattened extra array. `classify_compound` iterates over field type indices stored in this array. The concrete layouts are:

- **Struct extra layout:** `[field_count: u32, field_type_idx_0: u32, field_type_idx_1: u32, ...]` — `struct_fields(idx)` returns a slice of `field_count` type indices starting after the count.
- **Enum extra layout:** `[variant_count: u32, variant_0_field_count: u32, variant_0_field_0_idx: u32, ..., variant_1_field_count: u32, variant_1_field_0_idx: u32, ...]` — `enum_variants(idx)` iterates variant-by-variant, reading each variant's field count followed by that many field type indices.
- **Tuple extra layout:** `[element_count: u32, element_type_idx_0: u32, element_type_idx_1: u32, ...]` — same structure as struct fields.

These layouts allow `classify_compound` to iterate over all contained type indices without allocating. The `u32` indices refer to other entries in the Pool, enabling recursive classification via `classify(field_idx, pool)`.

```rust
/// Extension trait on Pool for ARC classification.
///
/// Lives in `ori_arc` crate. No LLVM dependency.
/// Used by all ARC analysis passes (Sections 06-09).
/// Also queried by `ori_llvm::TypeInfo` for emit_retain/emit_release decisions.
pub trait ArcClassification {
    fn arc_class(&self, idx: Idx) -> ArcClass;
    fn is_scalar(&self, idx: Idx) -> bool { self.arc_class(idx) == ArcClass::Scalar }
    fn needs_rc(&self, idx: Idx) -> bool { self.arc_class(idx) != ArcClass::Scalar }
}
```

- [ ] Implement `ArcClassification` for Pool (in `ori_arc`)
- [ ] Cache classification results (compute once per Idx)
- [ ] `ori_llvm::TypeInfo` queries `ArcClassification` via Pool — no duplication of classification logic

---

**Exit Criteria:** Every concrete (monomorphized) `Idx` in the Pool returns a correct `ArcClass`. Scalar types are correctly identified and will never have RC operations emitted. After monomorphization, compound types classify as either Scalar or DefiniteRef — PossibleRef only appears for unresolved type variables (which should not exist in concrete code).
