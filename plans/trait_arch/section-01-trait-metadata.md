---
section: "01"
title: Trait Metadata Registry
status: complete
goal: Single declarative macro generates DerivedTrait enum + all metadata, eliminating manual match-arm maintenance
sections:
  - id: "01.1"
    title: define_derived_traits! Macro
    status: complete
  - id: "01.2"
    title: Signature Shape Metadata
    status: complete
  - id: "01.3"
    title: Constraint Metadata
    status: complete
  - id: "01.4"
    title: Migration
    status: complete
  - id: "01.5"
    title: Completion Checklist
    status: complete
---

# Section 01: Trait Metadata Registry

**Status:** Complete
**Goal:** Replace the hand-written `DerivedTrait` enum with a `define_derived_traits!` macro that generates the enum, all accessor methods, an `ALL` constant for iteration, and structured metadata that downstream crates can query instead of hard-coding.

**Reference compilers:**
- **Rust** `compiler/rustc_hir/src/lang_items.rs` — `language_item_table!` macro declares 50+ lang items as tuples; macro generates enum, `from_name()`, getter methods, and storage. Adding a new lang item = one tuple.
- **Swift** `lib/Sema/DerivedConformances.cpp` — Per-trait `canDerive*()` checks encoded as static methods on a conformance descriptor.
- **Gleam** `compiler-core/src/type_.rs` — Exhaustive enum matching; no macro, relies on Rust compiler's exhaustiveness checking.

**Current state:** `ori_ir/derives/mod.rs` defines `DerivedTrait` as a hand-written enum (7 variants) with manually-synchronized `from_name()` and `method_name()` match arms. No `ALL` constant, no `trait_name()` inverse, no structured metadata. The DPR at `plans/dpr_registration-sync_02172026.md` designed this macro; this section implements it.

---

## 01.1 `define_derived_traits!` Macro

### Design

The macro takes a table of tuples and generates everything currently written by hand, plus new capabilities:

```rust
// compiler/ori_ir/src/derives/mod.rs

/// Declare all derived traits in a single location.
///
/// Each entry is: (Variant, "TraitName", "method_name", Shape, Supertrait, SumSupport)
///
/// Generates:
/// - `DerivedTrait` enum with all variants
/// - `from_name(&str) -> Option<DerivedTrait>` — parse trait name
/// - `method_name(&self) -> &'static str` — method identifier
/// - `trait_name(&self) -> &'static str` — trait name string
/// - `shape(&self) -> DerivedMethodShape` — parameter/return shape
/// - `requires_supertrait(&self) -> Option<DerivedTrait>` — supertrait constraint
/// - `supports_sum_types(&self) -> bool` — derivable on enums?
/// - `ALL: &[DerivedTrait]` — all variants for iteration
/// - `COUNT: usize` — variant count
macro_rules! define_derived_traits {
    ($(
        ($variant:ident, $trait_name:literal, $method_name:literal,
         $shape:expr, $supertrait:expr, $sum_support:expr)
    ),+ $(,)?) => {
        /// A derived trait that can be auto-implemented.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum DerivedTrait {
            $( #[doc = concat!("The `", $trait_name, "` trait.")] $variant, )+
        }

        impl DerivedTrait {
            /// All derived trait variants, for iteration in tests and registration.
            pub const ALL: &[DerivedTrait] = &[ $( DerivedTrait::$variant, )+ ];

            /// Number of derived trait variants.
            pub const COUNT: usize = [ $( DerivedTrait::$variant, )+ ].len();

            /// Parse a trait name string into a `DerivedTrait`.
            pub fn from_name(s: &str) -> Option<DerivedTrait> {
                match s {
                    $( $trait_name => Some(DerivedTrait::$variant), )+
                    _ => None,
                }
            }

            /// Get the method name for this derived trait.
            pub fn method_name(&self) -> &'static str {
                match self {
                    $( DerivedTrait::$variant => $method_name, )+
                }
            }

            /// Get the trait name string for this derived trait.
            pub fn trait_name(&self) -> &'static str {
                match self {
                    $( DerivedTrait::$variant => $trait_name, )+
                }
            }

            /// Get the parameter/return shape for this derived method.
            pub fn shape(&self) -> DerivedMethodShape {
                match self {
                    $( DerivedTrait::$variant => $shape, )+
                }
            }

            /// Get the required supertrait, if any.
            pub fn requires_supertrait(&self) -> Option<DerivedTrait> {
                match self {
                    $( DerivedTrait::$variant => $supertrait, )+
                }
            }

            /// Whether this trait can be derived for sum types (enums).
            pub fn supports_sum_types(&self) -> bool {
                match self {
                    $( DerivedTrait::$variant => $sum_support, )+
                }
            }
        }
    };
}
```

### Invocation

```rust
define_derived_traits! {
    // (Variant,    "TraitName",   "method",  Shape,                          Supertrait,                  SumTypes)
    (Eq,          "Eq",          "eq",      DerivedMethodShape::BinaryPredicate, None,                        true),
    (Clone,       "Clone",       "clone",   DerivedMethodShape::UnaryIdentity,   None,                        true),
    (Hashable,    "Hashable",    "hash",    DerivedMethodShape::UnaryToInt,      Some(DerivedTrait::Eq),      true),
    (Printable,   "Printable",   "to_str",  DerivedMethodShape::UnaryToStr,      None,                        true),
    (Debug,       "Debug",       "debug",   DerivedMethodShape::UnaryToStr,      None,                        true),
    (Default,     "Default",     "default", DerivedMethodShape::Nullary,         None,                        false),
    (Comparable,  "Comparable",  "compare", DerivedMethodShape::BinaryToOrdering, Some(DerivedTrait::Eq),     true),
}
```

### Key Decisions

1. **Macro in same file.** The macro is private to `ori_ir/derives/mod.rs`. It is not exported. Downstream crates import `DerivedTrait` and its methods — they never see the macro.

2. **Shape metadata is in `ori_ir`, not consuming crates.** The DPR recommended keeping signature metadata in consuming crates. We move it to `ori_ir` because: (a) the shape is a property of the trait definition, not the consumer; (b) `ori_types` and `ori_llvm` both need it and should not duplicate it; (c) `DerivedMethodShape` uses no `ori_types`-specific types (no `Idx`, no `Pool`).

3. **Supertrait constraint is in `ori_ir`.** "Hashable requires Eq" is a language-level fact, not a type-checker implementation detail. Encoding it here lets the type checker query it rather than hard-code it.

4. **Sum type support is in `ori_ir`.** "Default cannot be derived for sum types" is a language-level constraint. Encoding it here eliminates the hard-coded `DerivedTrait::Default` check in `register_derived_impl()`.

- [x] Write the `define_derived_traits!` macro in `ori_ir/derives/mod.rs`
- [x] Write the invocation with all 7 current traits
- [x] Verify the macro generates identical API to the current hand-written code
- [x] Run `cargo t -p ori_ir` — all existing tests pass unchanged

---

## 01.2 Signature Shape Metadata

### `DerivedMethodShape` Enum

This enum describes the parameter and return type shape of each derived method, abstracting over the concrete `Idx` types that only `ori_types` knows about.

```rust
// compiler/ori_ir/src/derives/mod.rs (above the macro)

/// The parameter/return shape of a derived method's signature.
///
/// Consuming crates (ori_types, ori_llvm) use this to construct type-correct
/// signatures without hard-coding per-trait parameter lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DerivedMethodShape {
    /// `(self: T, other: T) -> bool` (Eq)
    BinaryPredicate,
    /// `(self: T) -> T` (Clone)
    UnaryIdentity,
    /// `(self: T) -> int` (Hashable)
    UnaryToInt,
    /// `(self: T) -> str` (Printable, Debug)
    UnaryToStr,
    /// `() -> T` (Default) — no self parameter
    Nullary,
    /// `(self: T, other: T) -> Ordering` (Comparable)
    BinaryToOrdering,
}

impl DerivedMethodShape {
    /// Whether the method takes a `self` parameter.
    pub fn has_self(&self) -> bool {
        !matches!(self, DerivedMethodShape::Nullary)
    }

    /// Whether the method takes an `other: Self` parameter.
    pub fn has_other(&self) -> bool {
        matches!(
            self,
            DerivedMethodShape::BinaryPredicate | DerivedMethodShape::BinaryToOrdering
        )
    }

    /// Number of parameters (including self).
    pub fn param_count(&self) -> usize {
        match self {
            DerivedMethodShape::Nullary => 0,
            DerivedMethodShape::UnaryIdentity
            | DerivedMethodShape::UnaryToInt
            | DerivedMethodShape::UnaryToStr => 1,
            DerivedMethodShape::BinaryPredicate
            | DerivedMethodShape::BinaryToOrdering => 2,
        }
    }
}
```

### Consumption in `ori_types`

The type checker's `build_derived_methods()` currently hard-codes each trait's signature:

```rust
// BEFORE (in build_derived_methods):
let signature = match trait_kind {
    DerivedTrait::Eq => checker.pool_mut().function2(self_type, self_type, Idx::BOOL),
    DerivedTrait::Clone => checker.pool_mut().function1(self_type, self_type),
    DerivedTrait::Hashable => checker.pool_mut().function1(self_type, Idx::INT),
    DerivedTrait::Printable => checker.pool_mut().function1(self_type, Idx::STR),
    DerivedTrait::Debug => checker.pool_mut().function1(self_type, Idx::STR),
    DerivedTrait::Default => checker.pool_mut().function0(self_type),
    DerivedTrait::Comparable => checker.pool_mut().function2(self_type, self_type, ordering_type),
};

// AFTER (using shape metadata):
let signature = match trait_kind.shape() {
    DerivedMethodShape::BinaryPredicate =>
        checker.pool_mut().function2(self_type, self_type, Idx::BOOL),
    DerivedMethodShape::UnaryIdentity =>
        checker.pool_mut().function1(self_type, self_type),
    DerivedMethodShape::UnaryToInt =>
        checker.pool_mut().function1(self_type, Idx::INT),
    DerivedMethodShape::UnaryToStr =>
        checker.pool_mut().function1(self_type, Idx::STR),
    DerivedMethodShape::Nullary =>
        checker.pool_mut().function0(self_type),
    DerivedMethodShape::BinaryToOrdering =>
        checker.pool_mut().function2(self_type, self_type, ordering_type),
};
```

The `AFTER` version matches on shape, not trait identity. Adding a new trait with `BinaryPredicate` shape requires zero changes to this function.

- [x] Define `DerivedMethodShape` enum with all 6 variants
- [x] Implement `has_self()`, `has_other()`, `param_count()`
- [x] Verify `DerivedMethodShape` derives `Clone, Copy, Debug, PartialEq, Eq, Hash` (Salsa compatible)
- [x] Unit tests: each shape returns correct metadata

---

## 01.3 Constraint Metadata

### Supertrait Requirements

Currently, the type checker hard-codes supertrait checks:

```rust
// BEFORE (in register_derived_impl):
if trait_kind == DerivedTrait::Hashable {
    // Check that the type also derives Eq
    if !derives.contains(&DerivedTrait::Eq) {
        push_error(E2029, ...);
    }
}
```

With metadata:

```rust
// AFTER:
if let Some(required) = trait_kind.requires_supertrait() {
    if !derives.contains(&required) {
        push_error(E2029_like, trait_kind, required, ...);
    }
}
```

This generalizes the check: if a future trait (e.g., `Serialize`) requires `Eq`, it just needs `Some(DerivedTrait::Eq)` in the macro invocation.

### Sum Type Support

Currently:

```rust
// BEFORE:
if is_sum_type && trait_kind == DerivedTrait::Default {
    push_error(E2028, ...);
}
```

With metadata:

```rust
// AFTER:
if is_sum_type && !trait_kind.supports_sum_types() {
    push_error(E2028_like, trait_kind, ...);
}
```

- [x] Migrate supertrait check in `register_derived_impl()` to use `requires_supertrait()`
- [x] Migrate sum type check in `register_derived_impl()` to use `supports_sum_types()`
- [x] Verify error codes E2028 and E2029 produce identical diagnostics after migration
- [x] Unit tests: constraint queries return correct values for all 7 traits

---

## 01.4 Migration

### Step-by-Step

1. **Add `DerivedMethodShape` enum** to `ori_ir/derives/mod.rs` — new code, no changes to existing API
2. **Add the `define_derived_traits!` macro** to `ori_ir/derives/mod.rs` — new code, not yet invoked
3. **Replace the hand-written enum** with the macro invocation — the generated API is identical, so all downstream code compiles unchanged
4. **Run `cargo t -p ori_ir`** — existing `derives/tests.rs` must pass unchanged
5. **Run `./test-all.sh`** — full suite must pass unchanged
6. **Update `ori_types/check/registration/mod.rs`** to use `shape()` and constraint methods
7. **Update `ori_eval/derives/mod.rs`** to use `supports_sum_types()` if applicable
8. **Run `./test-all.sh`** again — full suite must pass

### Backward Compatibility

The macro generates the exact same public API:
- `DerivedTrait` enum with the same variants in the same order
- `from_name()` with the same string matching
- `method_name()` with the same return values
- `DerivedMethodInfo` struct is untouched (it's not generated by the macro)

New additions (`ALL`, `COUNT`, `trait_name()`, `shape()`, `requires_supertrait()`, `supports_sum_types()`) are purely additive.

- [x] Step 1: Add `DerivedMethodShape` — `cargo c` passes
- [x] Step 2: Add macro definition — `cargo c` passes
- [x] Step 3: Replace enum with invocation — `cargo t -p ori_ir` passes
- [x] Step 4: Full `./test-all.sh` passes
- [x] Step 5: Migrate `ori_types` to use `shape()` — `./test-all.sh` passes
- [x] Step 6: Migrate `ori_types` to use constraint methods — `./test-all.sh` passes

---

## 01.5 Completion Checklist

- [x] `DerivedMethodShape` enum defined with all 6 variants
- [x] `define_derived_traits!` macro defined and invoked
- [x] Generated API is identical to previous hand-written code
- [x] `DerivedTrait::ALL` and `COUNT` available for downstream use
- [x] `trait_name()` method available (inverse of `from_name()`)
- [x] `shape()` method returns correct `DerivedMethodShape` for all 7 traits
- [x] `requires_supertrait()` returns correct constraints for all 7 traits
- [x] `supports_sum_types()` returns correct values for all 7 traits
- [x] `ori_types` uses `shape()` for signature construction
- [x] `ori_types` uses constraint methods for validation
- [x] Unit tests: `from_name()` / `trait_name()` / `method_name()` round-trips for all variants
- [x] Unit tests: `shape()` metadata correctness for all variants
- [x] Unit tests: constraint correctness for all variants
- [x] `./test-all.sh` passes with zero regressions

**Exit Criteria:** `DerivedTrait` is defined by a single macro invocation. All metadata is queryable. No consuming crate hard-codes trait-specific knowledge that is available from the metadata. The `ALL` constant enables completeness assertions in Section 05.
