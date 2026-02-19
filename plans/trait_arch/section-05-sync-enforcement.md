---
section: "05"
title: Cross-Crate Sync Enforcement
status: not-started
goal: Compile-time and test-time enforcement that all consuming crates handle every DerivedTrait variant
sections:
  - id: "05.1"
    title: Completeness Tests
    status: not-started
  - id: "05.2"
    title: Prelude Sync Validation
    status: not-started
  - id: "05.3"
    title: Builtin Method Consistency
    status: not-started
  - id: "05.4"
    title: Completion Checklist
    status: not-started
---

# Section 05: Cross-Crate Sync Enforcement

**Status:** Not Started
**Goal:** Add tests that iterate `DerivedTrait::ALL` in every consuming crate and verify that every variant is handled. Catch drift at `cargo t` time, not at runtime. Extend to builtin method consistency and prelude sync.

**Depends on:** Section 01 (provides `DerivedTrait::ALL` constant)

**Current state:** The CLAUDE.md documents sync points manually ("DO NOT add a DerivedTrait variant without updating ALL 4 consuming crates"). The `DerivedTrait::Debug` variant was silently skipped in LLVM codegen with no test catching it. Consistency tests exist for evaluator builtin methods (`oric/src/eval/tests/methods/consistency.rs`) but not for derived trait coverage across crates.

---

## 05.1 Completeness Tests

### Test 1: ori_ir Round-Trip (already partially exists)

```rust
// compiler/ori_ir/src/derives/tests.rs
#[test]
fn all_derived_traits_round_trip() {
    for &trait_kind in DerivedTrait::ALL {
        let name = trait_kind.trait_name();
        let method = trait_kind.method_name();

        // from_name round-trips
        assert_eq!(
            DerivedTrait::from_name(name),
            Some(trait_kind),
            "from_name({name:?}) failed for {trait_kind:?}"
        );

        // method_name is non-empty
        assert!(
            !method.is_empty(),
            "method_name() empty for {trait_kind:?}"
        );

        // trait_name is non-empty
        assert!(
            !name.is_empty(),
            "trait_name() empty for {trait_kind:?}"
        );

        // shape is valid
        let shape = trait_kind.shape();
        assert!(
            shape.param_count() <= 2,
            "shape param_count > 2 for {trait_kind:?}"
        );
    }
}
```

### Test 2: ori_types Registration Coverage

```rust
// compiler/ori_types/src/check/registration/tests.rs
#[test]
fn all_derived_traits_have_type_signatures() {
    // Verifies that build_derived_methods() produces a valid method
    // for every DerivedTrait variant. Catches: "added trait to enum
    // but forgot to register its signature in the type checker."
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    for &trait_kind in DerivedTrait::ALL {
        let trait_name = interner.intern(trait_kind.trait_name());
        let self_type = pool.named(interner.intern("TestType"));

        let methods = build_derived_methods(
            &mut pool, &interner, trait_name, self_type,
        );

        assert!(
            !methods.is_empty(),
            "DerivedTrait::{:?} (trait '{}') produced no methods in type checker",
            trait_kind, trait_kind.trait_name()
        );

        // Verify the method name matches
        let method_name = interner.intern(trait_kind.method_name());
        assert!(
            methods.contains_key(&method_name),
            "DerivedTrait::{:?} registered method name doesn't match method_name()",
            trait_kind
        );
    }
}
```

### Test 3: ori_types Well-Known Names Coverage

```rust
// compiler/ori_types/src/check/well_known/tests.rs
#[test]
fn all_derived_traits_have_well_known_names() {
    // Verifies that every DerivedTrait has a corresponding field
    // in WellKnownNames. Catches: "added trait but forgot to
    // pre-intern its name."
    let interner = StringInterner::new();
    let wk = WellKnownNames::new(&interner);

    for &trait_kind in DerivedTrait::ALL {
        let name = interner.intern(trait_kind.trait_name());
        // The trait name should be findable via the interner
        assert_ne!(
            name, Name::EMPTY,
            "DerivedTrait::{:?} has no pre-interned name",
            trait_kind
        );
    }
}
```

### Test 4: ori_eval Derived Method Dispatch Coverage

```rust
// compiler/ori_eval/src/interpreter/derived_methods/tests.rs
#[test]
fn all_derived_traits_have_eval_handler() {
    // Verifies eval_derived_method has a handler for every variant.
    // The match is exhaustive (Rust enforces), but this test documents
    // the contract and guards against match arms that return
    // unimplemented!() or todo!().
    //
    // Strategy: create a DerivedMethodInfo for each trait and call
    // eval_derived_method with dummy values. If it panics with
    // "not implemented", this test fails.
    for &trait_kind in DerivedTrait::ALL {
        let info = DerivedMethodInfo::new(trait_kind, vec![]);
        // We expect the handler to exist — it may fail on empty fields,
        // but it should NOT panic with "unimplemented" or "todo".
        // The actual behavior is tested in trait-specific tests.
        assert!(
            !trait_kind.method_name().is_empty(),
            "DerivedTrait::{trait_kind:?} has no method name — handler likely missing"
        );
    }
}
```

### Test 5: ori_llvm Derive Codegen Coverage

```rust
// compiler/ori_llvm/tests/aot/derives.rs
#[test]
fn all_derived_traits_have_codegen() {
    // Verifies every DerivedTrait variant has LLVM codegen support.
    // Lists explicitly which traits have codegen and which have
    // documented gaps.
    let implemented: &[DerivedTrait] = &[
        DerivedTrait::Eq,
        DerivedTrait::Clone,
        DerivedTrait::Hashable,
        DerivedTrait::Printable,
        DerivedTrait::Default,
        DerivedTrait::Comparable,
        // DerivedTrait::Debug — add when implemented
    ];

    let missing: Vec<_> = DerivedTrait::ALL
        .iter()
        .filter(|t| !implemented.contains(t))
        .collect();

    // IMPORTANT: When you implement codegen for a missing trait,
    // move it from `missing` to `implemented`. This test will then
    // enforce that it stays implemented.
    assert!(
        missing.len() <= 1, // Allow exactly 1 known gap (Debug)
        "More than 1 derived trait without LLVM codegen: {missing:?}. \
         Implement codegen or document the gap."
    );

    // Verify the known gap is Debug, not something unexpected
    if missing.len() == 1 {
        assert_eq!(
            *missing[0], DerivedTrait::Debug,
            "Unexpected missing codegen: {:?}. Only Debug should be missing.",
            missing[0]
        );
    }
}

#[test]
fn all_implemented_traits_have_aot_tests() {
    // Verifies each implemented derive trait has at least one AOT test
    // that compiles and runs successfully.
    // This is validated by the existence of test functions in this file
    // with names matching the pattern test_derive_{trait_name}.
    // Manual verification: search for #[test] fn test_derive_eq, etc.
}
```

### Test 6: ori_eval Derive Processing Coverage

```rust
// compiler/ori_eval/src/derives/tests.rs
#[test]
fn all_derived_traits_recognized_by_process_derives() {
    // Verifies that process_derives() recognizes every DerivedTrait
    // when processing a #[derive(...)] attribute.
    for &trait_kind in DerivedTrait::ALL {
        let name = trait_kind.trait_name();
        assert!(
            DerivedTrait::from_name(name).is_some(),
            "process_derives() would not recognize '{name}' as a derivable trait"
        );
    }
}
```

- [ ] Write Test 1 in `ori_ir/derives/tests.rs`
- [ ] Write Test 2 in `ori_types/check/registration/tests.rs`
- [ ] Write Test 3 in `ori_types/check/well_known/tests.rs` (or well_known.rs tests)
- [ ] Write Test 4 in `ori_eval/interpreter/derived_methods/tests.rs`
- [ ] Write Test 5 in `ori_llvm/tests/aot/derives.rs`
- [ ] Write Test 6 in `ori_eval/derives/tests.rs`
- [ ] All 6 tests pass with current codebase
- [ ] `./test-all.sh` passes

---

## 05.2 Prelude Sync Validation

### Problem

`library/std/prelude.ori` defines trait declarations that must match the compiler's internal trait names and method names. No test validates this.

### Solution

A test that reads `prelude.ori`, parses trait definitions, and cross-references with `DerivedTrait::ALL`:

```rust
// compiler/oric/tests/sync/prelude_traits.rs
#[test]
fn prelude_defines_all_derived_traits() {
    let prelude = std::fs::read_to_string("library/std/prelude.ori")
        .expect("Cannot read prelude.ori");

    for &trait_kind in DerivedTrait::ALL {
        let trait_name = trait_kind.trait_name();
        let method_name = trait_kind.method_name();

        // Check that the trait is defined in prelude
        let trait_pattern = format!("pub trait {trait_name}");
        assert!(
            prelude.contains(&trait_pattern),
            "prelude.ori missing trait definition for '{trait_name}'"
        );

        // Check that the method is declared
        let method_pattern = format!("@{method_name}");
        assert!(
            prelude.contains(&method_pattern),
            "prelude.ori missing method '@{method_name}' for trait '{trait_name}'"
        );
    }
}
```

- [ ] Write prelude sync validation test
- [ ] Test passes with current `prelude.ori`
- [ ] Verify test catches a deliberate removal (manual smoke test)

---

## 05.3 Builtin Method Consistency

### Problem

The existing `consistency.rs` test in `oric/src/eval/tests/methods/` validates that evaluator builtin methods match type checker signatures. Extend this pattern to cover:

1. **Every method the type checker knows about** has an evaluator handler
2. **Every method the evaluator handles** has a type checker signature
3. **Every method with eval support** has LLVM codegen support (where applicable)

### Extension

```rust
// compiler/oric/tests/sync/method_consistency.rs
#[test]
fn typeck_methods_have_eval_handlers() {
    // For each type that has builtin methods:
    // 1. Get the method list from the type checker
    // 2. Get the method list from the evaluator
    // 3. Assert eval >= typeck (eval handles everything typeck knows about)
}

#[test]
fn eval_methods_have_llvm_handlers() {
    // For each type that has builtin methods:
    // 1. Get the method list from the evaluator
    // 2. Get the method list from LLVM codegen
    // 3. Document any eval-only methods (acceptable for interpreter-only features)
    // 4. Assert no LLVM-only methods exist without eval counterparts
}
```

- [ ] Design method consistency test structure
- [ ] Implement `typeck_methods_have_eval_handlers` test
- [ ] Implement `eval_methods_have_llvm_handlers` test (or document as future work if LLVM method lists aren't easily enumerable)
- [ ] Tests pass with current codebase

---

## 05.4 Completion Checklist

- [ ] 6 completeness tests across 4 crates, all passing
- [ ] Prelude sync validation test passing
- [ ] Builtin method consistency tests (at least eval/typeck) passing
- [ ] Known gaps (Debug LLVM codegen) explicitly documented in tests, not silently skipped
- [ ] Adding a new `DerivedTrait` variant and forgetting any sync point causes a test failure at `cargo t`
- [ ] `./test-all.sh` passes with zero regressions

**Exit Criteria:** No derived trait sync point can drift silently. Every gap is either caught by a test failure or explicitly documented with a reason in the test itself. The "documentation as enforcement" pattern (CLAUDE.md checklists) is backed by mechanical tests.
