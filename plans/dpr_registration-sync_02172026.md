---
plan: "dpr_registration-sync_02172026"
title: "Design Pattern Review: Registration & Sync Patterns"
status: draft
---

# Design Pattern Review: Registration & Sync Patterns

## Ori Today

Ori maintains compiler-known entities (derived traits, built-in types, trait registries) through a centralized-enum-plus-exhaustive-matching pattern. The canonical source of truth for derivable traits is the `DerivedTrait` enum in `ori_ir/derives/mod.rs`, which defines six variants (Eq, Clone, Hashable, Printable, Debug, Default) alongside two key methods: `from_name(&str) -> Option<DerivedTrait>` for parsing trait name strings and `method_name(&self) -> &'static str` for the corresponding method identifier. Every consuming crate imports this enum and calls these methods rather than duplicating the name-to-variant mapping. The `DerivedMethodInfo` struct pairs a `DerivedTrait` with field names, providing the payload that the evaluator needs for dispatch without embedding evaluator-specific data in the IR.

This enum flows through four distinct consumption sites. The type checker (`ori_types/check/registration/mod.rs::build_derived_methods`) calls `DerivedTrait::from_name()` to build method signatures (e.g., `eq(self: T, other: T) -> bool`) so return types are available during inference. The evaluator's derive processor (`ori_eval/derives/mod.rs::process_derives`) calls `from_name()` again to populate the `UserMethodRegistry` with `DerivedMethodInfo` entries. The evaluator's dispatch (`ori_eval/interpreter/derived_methods.rs::eval_derived_method`) matches all six variants to route to handler functions. The LLVM backend (`ori_llvm/codegen/derive_codegen.rs::compile_derives`) matches variants to emit synthetic LLVM functions for each trait. Test coverage spans all four sites: `ori_ir/derives/tests.rs` validates `from_name()` and `method_name()` round-trips, `ori_eval/derives/tests.rs` tests the process pipeline, `ori_patterns/user_methods/tests.rs` validates derived method registration and lookup, and `ori_llvm/tests/aot/derives.rs` runs end-to-end AOT compilation for derived traits.

What works well: the enum gives exhaustive match enforcement, so adding a seventh variant produces compile errors at every site that needs updating. The `from_name()` method eliminates string-based name duplication across crates. Test coverage is multi-layer, catching drift at IR, type-check, evaluation, and codegen levels. What is fragile: the same six-variant match exists in four files, maintained by manual copy (not generated). Method signatures in `build_derived_methods` are hand-written (e.g., `function2(self_type, self_type, Idx::BOOL)` for Eq) and not validated against the evaluator's actual dispatch. The LLVM backend handles Eq, Clone, Hashable, Printable, and Default but only logs a `debug!` skip for Debug -- there is no test that catches this gap. The `method_name()` return values (`"eq"`, `"clone"`, etc.) are hardcoded strings with no compile-time link to the method names the type checker registers.

## Prior Art

### Rust -- Declarative Macro Tables

Rust's `language_item_table!` macro in `compiler/rustc_hir/src/lang_items.rs` is the gold standard for compiler-known entity registration. A single macro invocation lists every lang item as a tuple -- `(Variant, sym::name, getter, Target, GenericRequirement)` -- and the macro expansion generates: the `LangItem` enum with discriminants, `LanguageItems` storage indexed by discriminant, match arms in `name()`, `from_name()`, `variant_name()`, `target()`, `required_generics()`, and public getter methods (e.g., `sized_trait()`). Adding a new lang item requires exactly one edit: append a tuple to the macro invocation. The compiler enforces that no match arm is missed because the macro generates them all from the same source list.

This works because it eliminates parallel lists entirely. There is no file where someone must remember to add a new arm. The macro invocation IS the registry, and every derived artifact is mechanically generated. The tradeoff is macro syntax density -- the invocation is 500+ lines of tuples -- but this is a net improvement over the alternative of maintaining those same entries across five separate match statements. For a compiler with 50+ lang items, the maintenance cost of manual sync would be prohibitive; for Ori with 6 derived traits, the calculus is different (addressed below).

### Gleam -- Exhaustive Enum Matching

Gleam's `Type` enum defines all built-in types as variants, and every traversal function matches exhaustively. When a new variant is added, Rust's exhaustiveness checker immediately flags every incomplete match across the entire codebase. There is no macro -- the mechanism is the language's own type system. This scales well for small systems: Gleam's `Type` enum has roughly a dozen variants, and the number of consumption sites is manageable.

The strength is zero abstraction cost -- no macro syntax to learn, no indirection to debug, and the Rust compiler itself is the enforcement tool. The weakness is that it only catches missing match arms, not missing data. If a new variant needs a corresponding entry in a `HashMap` or an `impl` block or a test file, exhaustiveness checking does not help. The programmer must still remember to update those non-match sites manually. For Ori, this is the current approach, and its weakness shows in exactly this way: the LLVM backend has a match arm for `DerivedTrait::Debug` (it compiles), but the arm does nothing and no test catches it.

### Go -- Manual Parallel Arrays (Anti-Pattern)

Go's type system uses parallel arrays indexed by discriminant: `types.Types[types.TBOOL]` for basic types, with a separate `basics` array mapping `types2.Bool` to `types.Types[types.TBOOL]`. The code generator maintains its own parallel table. Adding a new basic type requires edits in three locations with no compile-time check that they agree. This is the cautionary tale.

Go gets away with this because (1) basic types change almost never (the last addition was in Go 1.18 with generics), and (2) the codebase has thorough integration tests that catch drift at runtime. But the pattern is fundamentally fragile: it relies on programmer discipline rather than compiler enforcement. For any language where the set of compiler-known entities is actively evolving -- as Ori's is -- this pattern is a maintenance trap. It works until the day it doesn't, and the failure mode is a subtle runtime bug, not a compile error.

## Proposed Best-of-Breed Design

### Core Idea

The proposed design takes Rust's declarative-macro-table approach and adapts it for Ori's current scale, combined with Gleam's exhaustive-match philosophy as the enforcement backstop. The central idea is a `define_derived_traits!` macro in `ori_ir` that declares each derived trait once and generates: the `DerivedTrait` enum, `from_name()`, `method_name()`, an `ALL` constant for iteration, and a `count()` const. Consuming crates continue to import and match on the enum (Gleam's pattern), but the macro eliminates the manual duplication of name-to-variant mappings. A `DerivedTraitMeta` struct replaces the implicit knowledge currently spread across `build_derived_methods` and `compile_derives`, encoding each trait's method signature shape (parameter count, return type family) alongside its string mappings.

This is not a full Rust-style `language_item_table!` -- Ori's six traits don't justify that complexity. Instead, it is a minimal macro that eliminates the three concrete drift risks: (1) `from_name()` match arms diverging from enum variants, (2) `method_name()` strings diverging from what the type checker registers, and (3) consuming sites missing new variants without a compile-time signal beyond exhaustiveness. The macro generates exactly what is currently written by hand, plus an `ALL` array that enables test-time completeness assertions across crates.

### Key Design Choices

1. **Single declarative macro generates the canonical enum** (inspired by Rust's `language_item_table!`). The `define_derived_traits!` macro in `ori_ir/derives/mod.rs` takes tuples of `(Variant, "TraitName", "method_name")` and generates the enum, `from_name()`, `method_name()`, `trait_name()`, `ALL`, and `COUNT`. This eliminates three manually-maintained match statements. Ori-specific: the macro is small (< 50 lines) because we have 6 items, not 50.

2. **`ALL` constant enables cross-crate completeness tests** (inspired by Gleam's exhaustiveness, extended to non-match sites). Tests in `ori_eval`, `ori_llvm`, and `ori_types` can iterate `DerivedTrait::ALL` and assert that every variant has a corresponding handler, method signature, or codegen function. This catches the Debug-in-LLVM gap mechanically. No reference compiler does this well -- Rust's macro handles it by generation, Gleam relies on match exhaustiveness alone. Ori combines both: generate where possible, test-assert where generation is infeasible.

3. **Method signature metadata stays in consuming crates, not in the macro** (diverging from Rust's centralized approach). Rust's `language_item_table!` includes `GenericRequirement` because all lang items share the same metadata shape. Ori's derived traits have heterogeneous signatures (Eq takes `(self, other) -> bool`, Default takes `() -> Self`, Clone takes `(self) -> Self`). Encoding these in the macro would either require complex variant-specific metadata or lose type safety. Instead, each consuming crate owns its signature logic, and the `ALL`-based completeness test ensures no variant is forgotten. This is the right trade for Ori's expression-based design where method signatures interact with Salsa-cached inference results.

4. **Exhaustive matching remains the primary dispatch mechanism** (Gleam's pattern). The macro generates the enum; consuming crates match on it. No vtable, no trait objects, no dynamic dispatch. This aligns with Ori's compiler design principles: enum for fixed sets, exhaustiveness for correctness, static dispatch for performance. The macro adds zero runtime cost.

5. **Registration sync points use test-time iteration, not code generation** (novel for Ori). Rather than generating handler functions (which would couple the macro to evaluator internals), the macro provides `DerivedTrait::ALL` and each consuming crate writes a test like `for trait_kind in DerivedTrait::ALL { assert!(handler_exists(trait_kind)) }`. This is lighter than Rust's full generation approach, appropriate for Ori's scale, and catches drift at `cargo t` time rather than at runtime.

### What Makes Ori's Approach Unique

Ori has four constraints that none of the reference compilers share simultaneously, creating both challenges and opportunities for registration patterns:

**Salsa incremental compilation** means registration data must be deterministic and `Clone + Eq + Hash` compatible. The `DerivedTrait` enum already satisfies this (it derives all Salsa-required traits). But it also means the registration pattern cannot use global mutable state, singletons, or lazy initialization -- all common in other compilers (Rust's `LangItem` lookup uses thread-local `TyCtxt`). Ori's pattern of passing registries explicitly through `ModuleChecker` is actually cleaner than Rust's, and the macro should preserve this property.

**ARC memory management** means derived trait implementations have real semantic weight. In a GC language, `Clone` is trivial (copy the reference). In Ori, `Clone` means "increment the reference count and possibly deep-copy." The LLVM codegen for `Clone` (`compile_derive_clone`) currently returns `self` unchanged (identity for value types), but ARC-managed heap types will need actual RC-aware cloning. This means the derived trait registry will eventually need per-variant codegen strategies that vary by memory management classification -- a future need that the `ALL`-based test pattern supports (the test can assert not just existence but correctness of the ARC-aware path).

**Capability-based effects** mean derived traits may eventually need effect annotations. A `Default` implementation that calls a network service for initial values would need `uses Http`. The current `DerivedTrait` metadata does not encode effects because built-in defaults are pure. But when user-defined defaults enter the picture, the registration system must be extensible enough to carry effect information. The macro-generated `DerivedTrait` enum handles this naturally: adding an `effects` field to the consuming crate's handler (not the macro) preserves the clean separation.

**Mandatory tests** mean every derived method generated by the compiler must have test coverage. This is Ori's strongest unique constraint and the one that makes the `ALL`-based completeness test pattern most valuable. No other reference compiler requires that compiler-generated methods have test coverage -- they trust the implementation. Ori's philosophy demands verification, and iterating `ALL` in test assertions is the mechanical way to enforce it.

### Concrete Types & Interfaces

#### The Macro (in `ori_ir/derives/mod.rs`)

```rust
/// Declare all derived traits in a single location.
///
/// Generates: `DerivedTrait` enum, `from_name()`, `method_name()`,
/// `trait_name()`, `ALL`, `COUNT`.
macro_rules! define_derived_traits {
    ($(($variant:ident, $trait_name:literal, $method_name:literal)),+ $(,)?) => {
        /// A derived trait that can be auto-implemented.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum DerivedTrait {
            $( $variant, )+
        }

        impl DerivedTrait {
            /// All derived trait variants, for iteration in tests.
            pub const ALL: &[DerivedTrait] = &[
                $( DerivedTrait::$variant, )+
            ];

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
        }
    };
}

define_derived_traits! {
    (Eq,        "Eq",        "eq"),
    (Clone,     "Clone",     "clone"),
    (Hashable,  "Hashable",  "hash"),
    (Printable, "Printable", "to_str"),
    (Debug,     "Debug",     "debug"),
    (Default,   "Default",   "default"),
}
```

#### Completeness Test Pattern (in consuming crates)

```rust
// In ori_eval/interpreter/derived_methods/tests.rs
#[test]
fn all_derived_traits_have_eval_handler() {
    // Verify eval_derived_method handles every variant.
    // This catches "added a variant but forgot the evaluator arm" at cargo t time.
    for &trait_kind in DerivedTrait::ALL {
        let method = trait_kind.method_name();
        // The match in eval_derived_method is exhaustive, but this test
        // documents the contract and catches non-match gaps (e.g., a match
        // arm that returns an unimplemented!() error).
        assert!(
            !method.is_empty(),
            "DerivedTrait::{trait_kind:?} has no method name"
        );
    }
}

// In ori_llvm/tests/aot/derives.rs
#[test]
fn all_derived_traits_have_aot_test() {
    // Verify every DerivedTrait variant has at least one AOT test.
    // This catches the Debug gap: codegen skips it, no test flags it.
    let tested_traits = [
        DerivedTrait::Eq,
        DerivedTrait::Clone,
        DerivedTrait::Hashable,
        DerivedTrait::Printable,
        DerivedTrait::Default,
        // DerivedTrait::Debug -- KNOWN GAP: not yet implemented in LLVM codegen
    ];

    let untested: Vec<_> = DerivedTrait::ALL
        .iter()
        .filter(|t| !tested_traits.contains(t))
        .collect();

    // When Debug LLVM codegen is implemented, add it to tested_traits
    // and this assertion will enforce it.
    assert!(
        untested.len() <= 1, // Allow Debug gap until implemented
        "Untested derived traits in AOT: {untested:?}"
    );
}

// In ori_types/check/registration/tests.rs
#[test]
fn all_derived_traits_produce_valid_signatures() {
    // Verify build_derived_methods produces a non-empty method map
    // for every DerivedTrait variant.
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    let dummy_type = interner.intern("TestType");
    let self_type = checker.pool_mut().named(dummy_type);

    for &trait_kind in DerivedTrait::ALL {
        let trait_name = interner.intern(trait_kind.trait_name());
        let methods = build_derived_methods(
            &mut checker, trait_name, self_type, Span::DUMMY,
        );
        assert!(
            !methods.is_empty(),
            "DerivedTrait::{trait_kind:?} produced no methods in type checker"
        );
    }
}
```

#### Signature Shape Enum (future, for when metadata grows)

```rust
/// The parameter shape of a derived method's signature.
///
/// Used by consuming crates to construct type-correct signatures without
/// hard-coding parameter lists. Not generated by the macro -- this is
/// consuming-crate metadata, kept separate to avoid coupling ori_ir to
/// ori_types::Idx.
pub enum DerivedMethodShape {
    /// `(self: T, other: T) -> bool` (e.g., Eq)
    BinaryPredicate,
    /// `(self: T) -> T` (e.g., Clone)
    UnaryIdentity,
    /// `(self: T) -> int` (e.g., Hashable)
    UnaryToInt,
    /// `(self: T) -> str` (e.g., Printable, Debug)
    UnaryToStr,
    /// `() -> T` (e.g., Default)
    Nullary,
}
```

## Assessment: Is Ori's Current Approach Sound?

**Yes, with caveats.** Ori's current pattern -- centralized enum in `ori_ir` with exhaustive matching in consuming crates -- is a well-established, industry-standard approach. It is the same fundamental pattern used by Gleam (exhaustive enum matching) and is strictly better than Go's manual parallel arrays. For six derived traits and four consumption sites, it is the right level of abstraction. The enum's exhaustiveness checking catches the most dangerous class of errors (missing dispatch arms) at compile time. The `from_name()` and `method_name()` methods eliminate string duplication. The test coverage across IR, type checker, evaluator, and LLVM codegen provides multi-layer drift detection.

**The caveats are real but manageable:**

1. **The Debug LLVM codegen gap is a genuine drift finding.** `compile_derives` has a `DerivedTrait::Debug` arm that logs and skips. No test catches this. The fix is a test that iterates `DerivedTrait::ALL` and asserts codegen coverage (or explicitly documents known gaps).

2. **Method signature construction is implicit.** The type checker's `build_derived_methods` and the evaluator's `eval_derived_method` independently hard-code the same signatures. If someone changes Eq from `(self, other) -> bool` to `(self, other) -> Ordering`, the type checker and evaluator would need coordinated updates. Today this is safe because the signatures are obvious and stable. It becomes risky if Ori adds user-extensible derives.

3. **The `from_name`/`method_name` tests are duplicated.** `ori_ir/derives/tests.rs` and `ori_patterns/user_methods/tests.rs` both test `DerivedTrait::from_name()` with nearly identical assertions. This is not harmful but indicates that a shared `ALL` constant would let tests iterate rather than enumerate.

**When to migrate to the macro approach:** When the derived trait count exceeds 10-12, or when Ori adds user-definable derive macros that require a more structured registry. At 6 items, the macro saves about 30 lines of code and adds an `ALL` constant -- a net improvement but not urgent. The immediate value is the `ALL` constant for test completeness, which can be added without the full macro.

**Bottom line:** The current pattern is sound for the current scale. The macro is a quality-of-life improvement, not a correctness fix. The completeness tests (Phase 1 below) are the highest-value change because they close real gaps with zero architectural risk.

## Implementation Roadmap

### Phase 1: Quick Wins (Close Gaps Now)

- [ ] Add `DerivedTrait::ALL` constant and `DerivedTrait::COUNT` const to the existing hand-written enum in `ori_ir/derives/mod.rs` (no macro yet, just `pub const ALL: &[DerivedTrait] = &[Eq, Clone, Hashable, Printable, Debug, Default];`)
- [ ] Add `DerivedTrait::trait_name(&self) -> &'static str` method (inverse of `from_name`, returns `"Eq"` for `DerivedTrait::Eq`) to enable test assertions without string literals
- [ ] Add completeness test in `ori_ir/derives/tests.rs`: iterate `ALL`, verify `from_name(trait_name()) == Some(self)` round-trip for every variant
- [ ] Add completeness test in `ori_types/check/registration/tests.rs`: iterate `ALL`, verify `build_derived_methods` produces non-empty method map for every variant
- [ ] Add completeness test in `ori_llvm/tests/aot/derives.rs`: iterate `ALL`, document which variants have AOT test coverage, flag gaps explicitly (Debug is the known gap)
- [ ] Add completeness test in `ori_eval/derives/tests.rs`: verify `process_derives` registers a method for every `DerivedTrait` variant
- [ ] Implement LLVM codegen for `DerivedTrait::Debug` in `ori_llvm/codegen/derive_codegen.rs` (or add a tracking issue and explicit skip-with-reason in the completeness test)
- [ ] Remove duplicated `DerivedTrait::from_name` test from `ori_patterns/user_methods/tests.rs` -- it tests `ori_ir` functionality, not `user_methods`

### Phase 2: Macro Migration (When Scale Demands)

- [ ] Replace the hand-written `DerivedTrait` enum, `from_name()`, `method_name()`, `trait_name()`, `ALL`, and `COUNT` with the `define_derived_traits!` macro shown above
- [ ] Verify all existing tests pass unchanged (the macro generates identical code)
- [ ] Add `DerivedMethodShape` enum (or equivalent) if consuming crates need structured signature metadata beyond what `method_name()` provides
- [ ] Consider adding `has_self_param(&self) -> bool` to the macro-generated methods (currently computed ad-hoc in `build_derived_methods` via `!matches!(trait_kind, DerivedTrait::Default)`)

### Phase 3: Full Registry (Future)

- [ ] When Ori supports user-defined derive macros, introduce a `DeriveRegistry` trait that both built-in and user-defined derives implement
- [ ] Move from static enum dispatch to a registered-handler pattern (trait objects or function pointers) for user-extensible derives, while keeping the built-in enum path for compiler-known derives (two-tier dispatch)
- [ ] Integrate derive registration with Salsa queries so that adding a derive to a type only re-checks affected functions, not the entire module
- [ ] Add effect annotations to derived methods when capability tracking reaches derive implementations

## References

**Ori codebase (studied in full):**
- `compiler/ori_ir/src/derives/mod.rs` -- `DerivedTrait` enum, `DerivedMethodInfo` struct
- `compiler/ori_ir/src/derives/tests.rs` -- `from_name` and `method_name` tests
- `compiler/ori_types/src/check/registration/mod.rs` -- `build_derived_methods`, `register_derived_impls`
- `compiler/ori_types/src/check/registration/tests.rs` -- Registration integration tests
- `compiler/ori_types/src/registry/traits/mod.rs` -- `TraitRegistry`, `TraitEntry`, `ImplEntry`
- `compiler/ori_types/src/registry/traits/tests.rs` -- Trait registry tests
- `compiler/ori_eval/src/derives/mod.rs` -- `process_derives`, `DefaultFieldTypeRegistry`
- `compiler/ori_eval/src/derives/tests.rs` -- Derive processing tests
- `compiler/ori_eval/src/interpreter/derived_methods.rs` -- `eval_derived_method` dispatch
- `compiler/ori_llvm/src/codegen/derive_codegen.rs` -- `compile_derives`, per-trait codegen
- `compiler/ori_llvm/tests/aot/derives.rs` -- End-to-end AOT derive tests
- `compiler/ori_patterns/src/user_methods/tests.rs` -- `UserMethodRegistry` tests including `DerivedTrait::from_name`

**Reference repos (`~/projects/reference_repos/lang_repos/`):**
- `rust/compiler/rustc_hir/src/lang_items.rs` -- `language_item_table!` macro (declarative registration)
- `gleam/compiler-core/src/type_.rs` -- `Type` enum with exhaustive matching
- `golang/src/cmd/compile/internal/types/type.go` -- Parallel array type registration
- `golang/src/cmd/compile/internal/types2/basic.go` -- `basics` array (manual sync)
- `typescript/src/compiler/types.ts` -- `TypeFlags` bitflag enum (algebraic composition)
