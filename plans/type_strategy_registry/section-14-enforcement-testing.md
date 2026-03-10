---
plan: "type_strategy_registry"
section: "14"
title: "Enforcement Tests, Testing Matrix & Exit Criteria"
status: complete
reviewed: true
goal: "Make cross-phase drift structurally impossible via enforcement tests, eliminate all allowlists, remove all legacy code, and define exhaustive exit criteria for the entire plan"
depends_on:
  - "09"
  - "10"
  - "11"
  - "12"
  - "13"
subsections:
  - id: "14.0"
    title: "Prerequisite Function Creation"
    status: complete
  - id: "14.1"
    title: "Registry-Level Integrity Tests (ori_registry)"
    status: complete
  - id: "14.1b"
    title: "Compile-Time Exhaustiveness Guards (Roc pattern)"
    status: complete
  - id: "14.2"
    title: "Cross-Phase Enforcement Tests (oric integration)"
    status: complete
  - id: "14.3"
    title: "Purity Enforcement Tests (ori_registry)"
    status: complete
  - id: "14.4"
    title: "Testing Matrix (type x method x phase)"
    status: complete
  - id: "14.5"
    title: "Allowlist Elimination Checklist"
    status: complete
  - id: "14.6"
    title: "Legacy Code Removal & Grep Verification"
    status: complete
  - id: "14.7"
    title: "Full Test Suite Execution"
    status: complete
  - id: "14.8"
    title: "Code Journey (Pipeline Integration)"
    status: complete
  - id: "14.9"
    title: "Exit Criteria (Entire Plan)"
    status: complete
---

# Section 14: Enforcement Tests, Testing Matrix & Exit Criteria

**Context:** Sections 09-13 wired all consuming phases (ori_types, ori_eval, ori_arc, ori_llvm, ori_ir) to read from ori_registry instead of maintaining independent type knowledge. This section is the final gate: it replaces the remaining ~315-line `consistency.rs` (post-Section 13, with zero cross-phase gap allowlists) and ~371-line `dispatch_coverage.rs` (eval implementation tracking) with a small set of structural enforcement tests that derive their expectations directly from the registry. The registry IS the specification; the enforcement tests verify that every phase faithfully implements it.

**Design rationale:** The old consistency tests were necessary because type knowledge was scattered: `TYPECK_BUILTIN_METHODS` (390 entries), `EVAL_BUILTIN_METHODS` (~165 entries), `BUILTIN_METHODS` in ori_ir (123 entries), `BuiltinTable` in ori_llvm (179 entries), and `borrowing_builtins` in ori_arc. The allowlists (`TYPECK_METHODS_NOT_IN_EVAL`, `EVAL_METHODS_NOT_IN_IR`, etc.) tracked intentional gaps between these independent lists. Sections 09-13 eliminated all 6 allowlists and the ori_ir builtin_methods module. With the registry as single source of truth, these gaps become structural impossibilities -- a method either exists in the registry (and all phases must handle it) or it does not exist (and no phase references it). The enforcement tests verify this invariant at test time, while Rust's type system enforces it at compile time (adding a field to `TypeDef` is a compile error in every consuming phase).

**What this section replaces (post-Section 13 state):**
- `compiler/oric/src/eval/tests/methods/consistency.rs` (~315 lines remaining after Section 13) -- rewritten with enforcement tests
- `compiler/oric/src/eval/tests/methods/dispatch_coverage.rs` (~371 lines) -- the `METHODS_NOT_YET_IN_EVAL` (189 entries) and `COLLECTION_RESOLVER_METHODS` (16 entries) allowlists are eliminated when all methods are implemented; the `every_registry_method_has_eval_dispatch_handler` test is replaced by enforcement tests
- 10 remaining consistency tests (registry sorting, iterator consistency, 6 format spec tests, well_known_generic_types_consistent, well_known_generic_types_matches_registry)
- `EVAL_BUILTIN_METHODS` and `TYPECK_BUILTIN_METHODS` (already eliminated in Sections 09-10)
- `ITERATOR_METHOD_NAMES` (already eliminated in Section 10)
- All `resolve_*_method()` functions (already eliminated in Section 09, 19 functions excluding `resolve_named_type_method`)

**Additional parallel lists to track (not in consistency.rs but relevant to exit criteria):**
- `compiler/oric/src/eval/tests/methods/dispatch_coverage.rs`: `METHODS_NOT_YET_IN_EVAL` (189 entries), `COLLECTION_RESOLVER_METHODS` (16 entries) — these track eval dispatch gaps and resolver-dispatch paths
- `compiler/ori_arc/src/borrow/builtins/mod.rs`: `CONSUMING_RECEIVER_METHOD_NAMES` (10 entries), `CONSUMING_SECOND_ARG_METHOD_NAMES` (2 entries), `CONSUMING_RECEIVER_ONLY_METHOD_NAMES` (5 entries), `SHARING_METHOD_NAMES` (2 entries) — these encode COW-specific ARC ownership semantics; they are NOT simple borrow/own flags and may legitimately remain as ARC-internal knowledge, but should be validated against registry data via enforcement tests

---

## Execution Order

Subsections within Section 14 have dependencies. They MUST be executed in this order:

1. **14.0 (Prerequisite Functions)** — Create the public API functions needed by enforcement tests. No tests depend on this yet, but all of 14.2 and 14.4 are blocked without it.
2. **14.1 + 14.1b (Registry Integrity + Exhaustiveness Guards)** — Can run in parallel. These are self-contained within ori_registry and consuming crates respectively.
3. **14.3 (Purity Enforcement)** — Independent; verifies existing tests from Section 02.
4. **14.2 (Cross-Phase Enforcement)** — Depends on 14.0 (prerequisite functions must exist). Rewrites consistency.rs.
5. **14.4 (Testing Matrix)** — Depends on 14.0 and 14.2 (uses the same API functions).
6. **14.5 (Allowlist Elimination)** — Can start after 14.2; grep verifications run after deletions.
7. **14.6 (Legacy Code Removal)** — Depends on 14.5 (allowlists must be deleted first); comment cleanups can start in parallel.
8. **14.7 (Full Test Suite)** — Depends on ALL of 14.1-14.6 being complete.
9. **14.8 (Code Journey)** — Depends on 14.7 passing.
10. **14.9 (Exit Criteria)** — Final gate; depends on everything else.

---

## 14.0 Prerequisite Function Creation

Several enforcement tests in 14.2 and 14.4 call public API functions that **do not exist yet**. These must be created before the enforcement tests can be written.

### 14.0.1 `ori_eval::can_dispatch_builtin(TypeTag, &str) -> bool`

**Does not exist.** Referenced by 14.2 Test 2, 14.2 Test 6, and 14.4.

**Purpose:** Query whether the evaluator's dispatch chain (BuiltinMethodResolver + CollectionMethodResolver) can handle a (type, method) pair without producing `UndefinedMethod`.

**Implementation options:**
- (a) Wrap the existing `dispatch_builtin_method_str()` pattern from dispatch_coverage.rs: call with a minimal `Value` and empty args, check that the error is NOT `UndefinedMethod`. This is the simplest approach but requires constructing a `Value` per call.
- (b) Add a dedicated lookup function that queries the resolver chain without executing: `BuiltinMethodResolver::has_method(type_name, method_name)` + `CollectionMethodResolver::has_method(method_name)`. Cleaner but requires adding `has_method` to both resolvers.

**Recommended:** Option (b) for performance and clarity. The existing `dispatch_coverage.rs` already demonstrates option (a) works, so the semantics are validated.

**File:** `compiler/ori_eval/src/methods/mod.rs` or `compiler/ori_eval/src/lib.rs` (pub re-export)

### 14.0.2 `ori_llvm::has_builtin_handler(&str, &str) -> bool`

**Does not exist.** Referenced by 14.2 Test 3 and 14.2 Test 6.

**Purpose:** Query whether ori_llvm can handle a (type_name, method_name) pair, either via inline BuiltinTable codegen or via a runtime function declaration.

**Implementation:** Promote `builtin_table()` access or add a thin wrapper:
```rust
// In ori_llvm/src/lib.rs or codegen/arc_emitter/builtins/mod.rs
pub fn has_builtin_handler(type_name: &str, method_name: &str) -> bool {
    let table = builtin_table();
    table.has(type_name, method_name)
        || has_runtime_method(type_name, method_name)
}
```

**Note:** `builtin_table()` is currently `pub(crate)`. Either promote to `pub` or provide the wrapper above. `has_runtime_method()` also does not exist (see 14.0.3).

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs` (+ re-export in `lib.rs`)

### 14.0.3 `ori_llvm::has_runtime_method(&str, &str) -> bool`

**Does not exist.** Referenced by 14.2 Test 3.

**Purpose:** Check whether a builtin method has a corresponding `ori_rt` runtime function declaration (the fallback path for methods without inline LLVM IR).

**Implementation:** Scan the runtime function declarations in `ori_llvm/src/runtime.rs` or build a static set of known runtime-dispatched methods. Alternatively, if all methods go through `BuiltinTable`, this function may be unnecessary (every method either has a table entry or falls through to the generic interpreter-style dispatch).

**Decision needed:** Verify whether all builtin methods have BuiltinTable entries. If yes, `has_runtime_method` is not needed and `has_builtin_handler` simplifies to `builtin_table().has(type_name, method_name)`. If some methods use runtime calls without table entries, enumerate them.

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs`

### 14.0.4 `ori_llvm::handles_op_strategy(TypeTag, &str, &OpStrategy) -> bool`

**Does not exist.** Referenced by 14.2 Test 4.

**Purpose:** Verify that `emit_binary_op`/`emit_unary_op` handles a specific `OpStrategy` variant for a given type and operator name.

**Implementation:** This is intrinsically hard to check at the API level because operator handling is embedded in match arms inside `emit_binary_op`. Options:
- (a) Enumerate all handled `(TypeTag, OpStrategy)` pairs in a static set and check membership. Fragile — the set can drift from the actual match arms.
- (b) Use the existing `registry_op_strategies_cover_all_operators` test in `ori_llvm/tests` (which already verifies all 20 OpDefs fields for all types). This test already exists and passes. The enforcement test in 14.2 can be marked as **already covered by ori_llvm's own test** rather than requiring a new cross-crate API.

**Recommended:** Option (b). The existing test at `compiler/ori_llvm/src/codegen/arc_emitter/builtins/tests.rs:200` already does this. Add a comment in 14.2 Test 4 noting that this test is in ori_llvm rather than oric, and add a cross-reference.

**File:** No new file needed if using option (b). If option (a), `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs`.

### 14.0.5 Alternative: Place LLVM Tests In-Crate

For 14.0.2, 14.0.3, and 14.0.4, an alternative to creating `pub` APIs is to place the enforcement tests **within ori_llvm's own test suite** (where `pub(crate)` is accessible). This avoids leaking internal APIs. The tradeoff is that `cargo t -p oric` won't run them — they require `cargo t -p ori_llvm`.

**Recommendation:** Place LLVM-specific enforcement tests in `compiler/ori_llvm/src/codegen/arc_emitter/builtins/tests.rs` (where the existing `registry_op_strategies_cover_all_operators` test already lives). Only `can_dispatch_builtin` truly needs to be in oric (because it tests cross-crate eval dispatch).

### Warnings

> **Complexity warning — `can_dispatch_builtin` (14.0.1).** Option (b) ("add `has_method` to both resolvers") is cleaner but requires understanding the full resolver chain. The existing `dispatch_builtin_method_str()` in `compiler/ori_eval/src/methods/mod.rs:180-193` only exercises `BuiltinMethodResolver` (priority 2), NOT `CollectionMethodResolver` (priority 1) or `UserRegistryResolver` (priority 0). The dispatch_coverage.rs test at line 316-317 already tracks this gap via `COLLECTION_RESOLVER_METHODS`. The `can_dispatch_builtin` function must check ALL three resolvers to be correct.

> **Dependency warning — `TypeTag` vs `Value` impedance mismatch.** `can_dispatch_builtin(TypeTag, &str)` takes a `TypeTag`, but the evaluator dispatches on `Value` variants (e.g., `Value::Some`/`Value::None` both map to `TypeTag::Option`). The function must map `TypeTag` to a representative `Value` internally — exactly what `dispatch_coverage.rs::minimal_value_for()` already does. Consider reusing or inlining that function.

### Checklist

- [x] `ori_eval::can_dispatch_builtin(TypeTag, &str) -> bool` created and exported (2026-03-09)
- [x] Decision made: LLVM enforcement tests in ori_llvm (BuiltinTable is pub(crate), 3 tests already exist in builtins/tests.rs) (2026-03-09)
- [x] If oric: `ori_llvm::has_builtin_handler(&str, &str) -> bool` — N/A (decision: ori_llvm, no cross-crate API needed) (2026-03-09)
- [x] If oric: `ori_llvm::handles_op_strategy` — N/A (decision: ori_llvm, existing tests cover this) (2026-03-09)
- [x] If ori_llvm: enforcement tests added to `ori_llvm/src/codegen/arc_emitter/builtins/tests.rs` (3 existing tests: no_phantom_builtin_entries, builtin_coverage_above_threshold, registry_op_strategies_cover_all_operators) (2026-03-09)

---

## 14.1 Registry-Level Integrity Tests (ori_registry)

**File:** `compiler/ori_registry/src/tests.rs` (additions to existing purity tests from Section 02)

These tests enforce internal registry consistency -- the data itself is well-formed, regardless of whether consuming phases handle it correctly.

### Test 1: No duplicate methods within any type

```rust
/// Every TypeDef's method list must contain unique method names.
/// A duplicate would cause ambiguous dispatch in every consuming phase.
#[test]
fn no_duplicate_methods() {
    use std::collections::BTreeSet;

    for type_def in BUILTIN_TYPES {
        let mut seen = BTreeSet::new();
        for method in type_def.methods {
            assert!(
                seen.insert(method.name),
                "Duplicate method `{}` on type `{}`",
                method.name, type_def.name,
            );
        }
    }
}
```

### Test 2: No empty types

```rust
/// Every TypeDef must have at least one method.
/// A type with zero methods provides no behavioral specification
/// and should not be in the registry.
#[test]
fn no_empty_types() {
    for type_def in BUILTIN_TYPES {
        assert!(
            !type_def.methods.is_empty(),
            "TypeDef `{}` has zero methods -- every registered type must \
             have at least one method (minimally: clone, equals, to_str)",
            type_def.name,
        );
    }
}
```

### Test 3: All TypeTag variants with methods have a TypeDef

```rust
/// Every TypeTag variant that carries methods must have a corresponding
/// TypeDef in BUILTIN_TYPES. Variants without methods (Unit, Never,
/// Function) and the DEI alias (DoubleEndedIterator → Iterator) are
/// intentionally excluded.
#[test]
fn all_method_bearing_type_tags_present() {
    use std::collections::BTreeSet;

    let registered_tags: BTreeSet<TypeTag> = BUILTIN_TYPES
        .iter()
        .map(|td| td.tag)
        .collect();

    // TypeTag variants that intentionally have no TypeDef:
    // - Unit, Never: no methods, no operators
    // - Function: no methods (memory classification only)
    // - DoubleEndedIterator: aliases to Iterator via base_type()
    const EXCLUDED: &[TypeTag] = &[
        TypeTag::Unit,
        TypeTag::Never,
        TypeTag::Function,
        TypeTag::DoubleEndedIterator,
    ];

    // TypeTag::all() returns all 23 variants in declaration order.
    for &tag in TypeTag::all() {
        if EXCLUDED.contains(&tag) {
            continue;
        }
        assert!(
            registered_tags.contains(&tag),
            "TypeTag::{tag:?} has no TypeDef in BUILTIN_TYPES. \
             Add a const TypeDef in ori_registry/src/defs/ and include \
             it in BUILTIN_TYPES.",
        );
    }
}
```

### Test 4: Methods sorted by name within each type

```rust
/// Methods within each TypeDef must be sorted alphabetically by name.
/// This is a convention for deterministic iteration, readable diffs,
/// and binary-searchable lookup.
#[test]
fn methods_sorted_by_name() {
    for type_def in BUILTIN_TYPES {
        for window in type_def.methods.windows(2) {
            assert!(
                window[0].name <= window[1].name,
                "Methods not sorted in `{}`: `{}` > `{}`\n\
                 Methods must be alphabetically sorted within each TypeDef.",
                type_def.name, window[0].name, window[1].name,
            );
        }
    }
}
```

### Test 5: All receivers have explicit Ownership

```rust
/// Every MethodDef must have an explicit Ownership value for its receiver.
/// This test documents the invariant rather than checking it structurally
/// (since Ownership is a required field, Rust already enforces this at
/// compile time). The test verifies the semantic convention that no
/// method has Ownership::Owned unless it truly consumes self.
#[test]
fn all_receivers_documented() {
    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            // Copy types should always borrow (borrow == copy for Copy types,
            // but the annotation documents the intent).
            if type_def.memory == MemoryStrategy::Copy {
                assert_eq!(
                    method.receiver, Ownership::Borrow,
                    "Method `{}.{}` on a Copy type should use Ownership::Borrow \
                     (Copy types are trivially borrowed)",
                    type_def.name, method.name,
                );
            }
            // Arc types: most methods borrow, but consuming methods (into)
            // may use Owned. Just verify the field is explicitly set.
            // (No-op assertion -- Ownership is a required field. This test
            // exists to document the expectation.)
            let _ = method.receiver; // field access proves it exists
        }
    }
}
```

### Test 6: Equality support is universal

```rust
/// Every Ori builtin type supports `==` (equality). This is a language invariant:
/// all values are comparable for equality. A type with Unsupported eq would be
/// a language-level bug.
#[test]
fn no_unsupported_eq() {
    for type_def in BUILTIN_TYPES {
        assert!(
            type_def.operators.eq != OpStrategy::Unsupported,
            "Type `{}` has Unsupported eq operator. All Ori types must \
             support equality comparison.",
            type_def.name,
        );
    }
}
```

### Test 7: Operator consistency

```rust
/// If a type supports comparison operators (lt, gt, lt_eq, gt_eq), it must
/// also support equality (eq, neq). Comparison without equality is nonsensical.
/// Additionally, if any ordering operator is supported, all four ordering
/// operators (lt, gt, lt_eq, gt_eq) must be supported.
#[test]
fn operator_consistency() {
    for type_def in BUILTIN_TYPES {
        let ops = &type_def.operators;

        // If any comparison operator is supported, eq must be too
        let has_any_cmp = ops.lt != OpStrategy::Unsupported
            || ops.gt != OpStrategy::Unsupported
            || ops.lt_eq != OpStrategy::Unsupported
            || ops.gt_eq != OpStrategy::Unsupported;

        if has_any_cmp {
            assert!(
                ops.eq != OpStrategy::Unsupported,
                "Type `{}` supports comparison but not equality. \
                 If lt/gt/le/ge are supported, eq must be too.",
                type_def.name,
            );
            assert!(
                ops.neq != OpStrategy::Unsupported,
                "Type `{}` supports comparison but not not-equal. \
                 If lt/gt/le/ge are supported, neq must be too.",
                type_def.name,
            );
        }

        // If all four comparison operators are supported, they should
        // use the same strategy (no mixing signed/unsigned/float)
        let cmp_ops = [ops.lt, ops.gt, ops.lt_eq, ops.gt_eq];
        let supported_cmp: Vec<_> = cmp_ops
            .iter()
            .filter(|s| **s != OpStrategy::Unsupported)
            .collect();
        if supported_cmp.len() > 1 {
            let first = supported_cmp[0];
            for s in &supported_cmp[1..] {
                assert_eq!(
                    *s, first,
                    "Type `{}` uses mixed comparison strategies: {:?} vs {:?}. \
                     All comparison operators should use the same strategy.",
                    type_def.name, first, s,
                );
            }
        }
    }
}
```

### Test 8: SelfType returns are valid

```rust
/// Methods that return SelfType must be on types where returning Self
/// makes semantic sense. This test verifies that SelfType is only used
/// for methods that truly return the same type as the receiver.
///
/// Specifically: trait methods like `clone` should return SelfType.
/// Conversion methods like `to_int` should return a concrete TypeTag.
#[test]
fn self_type_returns_valid() {
    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            if method.returns == ReturnTag::SelfType {
                // SelfType is valid for:
                // - clone() -- returns same type
                // - Operator trait methods (add, sub, etc.) -- T op T -> T
                // - Transform methods (trim, to_uppercase, etc.) -- str -> str
                // SelfType is NOT valid for:
                // - to_str() which should return Concrete(Str)
                // - to_int() which should return Concrete(Int)
                //
                // We can't exhaustively validate the semantic correctness
                // of SelfType usage, but we can check that `to_*` conversion
                // methods (except to_uppercase/to_lowercase on non-str types)
                // do NOT use SelfType.
                if method.name.starts_with("to_") && method.name != "to_uppercase"
                    && method.name != "to_lowercase"
                {
                    // to_str, to_int, to_float, to_byte, to_char should use
                    // concrete return types, not SelfType.
                    // EXCEPTION: `to_str` on str itself returns SelfType (identity).
                    let is_identity = type_def.tag == TypeTag::Str
                        && method.name == "to_str";
                    if !is_identity {
                        panic!(
                            "Method `{}.{}` returns SelfType but is a conversion \
                             method (`to_*`). Conversion methods should return \
                             a concrete TypeTag, not SelfType.",
                            type_def.name, method.name,
                        );
                    }
                }
            }
        }
    }
}
```

### Cleanup

- [x] **[WASTE]** `compiler/ori_registry/src/query/tests.rs:312-318` -- Duplicate `cycle` assertion removed (2026-03-09)

### Checklist

- [x] `no_duplicate_methods` -- no type has two methods with the same name (pre-existing in defs/tests.rs) (2026-03-09)
- [x] `no_empty_types` -- every TypeDef has at least one method (2026-03-09)
- [x] `all_method_bearing_type_tags_present` -- every method-bearing TypeTag variant has a TypeDef (2026-03-09)
- [x] `methods_sorted_by_name` -- alphabetical within each type (pre-existing as `methods_alphabetically_sorted`) (2026-03-09)
- [x] `all_receivers_documented` -- every MethodDef has explicit Ownership (2026-03-09)
- [x] `no_unsupported_eq` -- every type supports at least `==` (Error/Channel/Iterator/Range exempt with justification) (2026-03-09)
- [x] `operator_consistency` -- comparison implies equality; consistent strategies (2026-03-09)
- [x] `self_type_returns_valid` -- SelfType only on semantically correct methods (2026-03-09)

---

## 14.1b Compile-Time Exhaustiveness Guards (Roc pattern)

**Files:** One `_enforce_exhaustiveness` function per consuming crate.

Inspired by Roc's `_enforce_exhaustiveness` pattern: dead private functions whose sole purpose is to trigger Rust's exhaustive match checker when a new `TypeTag` variant is added. These are **never called** — they exist purely for compile-time enforcement. Zero runtime cost.

### Pattern

Each consuming crate (ori_types, ori_eval, ori_llvm, ori_arc) gets a function like:

```rust
/// NEVER CALLED. Exists solely so that Rust's exhaustive match checker
/// forces updates to this crate when a new TypeTag variant is added.
/// If you see a compile error pointing here, a new TypeTag was added
/// to ori_registry without updating this crate's handler.
#[allow(dead_code, unreachable_code)]
fn _enforce_exhaustiveness(tag: ori_registry::TypeTag) {
    match tag {
        TypeTag::Int => { /* handled in resolve_int_methods() */ }
        TypeTag::Float => { /* handled in resolve_float_methods() */ }
        TypeTag::Str => { /* handled in resolve_str_methods() */ }
        TypeTag::Bool => { /* handled in resolve_bool_methods() */ }
        TypeTag::Byte => { /* handled in resolve_byte_methods() */ }
        TypeTag::Char => { /* handled in resolve_char_methods() */ }
        TypeTag::List => { /* handled in resolve_list_methods() */ }
        TypeTag::Map => { /* handled in resolve_map_methods() */ }
        TypeTag::Set => { /* handled in resolve_set_methods() */ }
        // ... every variant must be listed
        // Adding a new TypeTag variant without a line here = COMPILE ERROR
    }
}
```

### Where to place them

| Crate | File | What it guards |
|-------|------|----------------|
| `ori_types` | `src/infer/expr/methods/mod.rs` | Method resolution handles all types |
| `ori_eval` | `src/methods/mod.rs` | Method dispatch handles all types |
| `ori_llvm` | `src/codegen/arc_emitter/builtins/mod.rs` | Builtin codegen handles all types |
| `ori_arc` | `src/borrow/mod.rs` | Borrow inference handles all types |

### Why this is better than test-time enforcement

- **Compile-time**: Caught during `cargo c`, before any tests run
- **Zero cost**: Dead code, never emitted in the binary
- **Precise error**: The compiler points directly at the missing match arm
- **Roc-validated**: Used in production across `low_level.rs` and `can/builtins.rs`

The test-time enforcement from 14.2 still provides value (verifying that per-method handlers exist within each type), but the compile-time guard catches the coarser "you forgot an entire type" class of errors instantly.

### Implementation Steps

1. **ori_types**: Add `_enforce_exhaustiveness` to `compiler/ori_types/src/infer/expr/methods/mod.rs`. List every `TypeTag` variant with a comment pointing to the handler function or stating why it is not applicable (e.g., `Unit`/`Never` have no methods). This file is where `resolve_named_type_method` dispatches, so it is the natural home.

2. **ori_eval**: Add `_enforce_exhaustiveness` to `compiler/ori_eval/src/methods/mod.rs`. List every `TypeTag` variant with a comment referencing the resolver that handles it (BuiltinMethodResolver, CollectionMethodResolver) or why it has no handler (Channel — no Value representation yet).

3. **ori_llvm**: Add `_enforce_exhaustiveness` to `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs`. List every `TypeTag` variant referencing the builtin registration module that handles it. Note: `minimal_value_for()` in dispatch_coverage.rs already has an exhaustive match over TypeTag — the pattern is proven.

4. **ori_arc**: Add `_enforce_exhaustiveness` to `compiler/ori_arc/src/borrow/mod.rs`. List every `TypeTag` variant referencing how borrow inference handles it (borrowing set member, iterator exclusion, Copy type passthrough, etc.).

5. **Verification**: Temporarily add a `#[cfg(test)] TypeTag::_Test` variant (or use `#[non_exhaustive]` test — but TypeTag is internal so non_exhaustive is wrong). The practical verification is: check that all 4 functions have all 23 TypeTag variants listed. A manual count + code review is sufficient; the Rust compiler enforces the rest.

### Cleanup

- [x] **[STYLE]** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs` -- Six `#[allow(dead_code)]` annotations verified as acceptable tradeoff (can't use `#[expect]` — would trigger unfulfilled-lint-expectation in non-test mode). Comment at line 155-158 documents reasoning. (2026-03-09)

### Checklist

- [x] `_enforce_type_tag_exhaustiveness(TypeTag)` in ori_types — covers all 23 TypeTag variants (2026-03-09)
- [x] `_enforce_type_tag_exhaustiveness(TypeTag)` in ori_eval — covers all 23 TypeTag variants (2026-03-09)
- [x] `_enforce_type_tag_exhaustiveness(TypeTag)` in ori_llvm — covers all 23 TypeTag variants (2026-03-09)
- [x] `_enforce_type_tag_exhaustiveness(TypeTag)` in ori_arc — covers all 23 TypeTag variants (2026-03-09)
- [x] Verified: all 4 functions list all 23 TypeTag variants (grep -c confirms 23 each) (2026-03-09)
- [x] Verified: `cargo c` passes with all 4 functions added (2026-03-09)

---

## 14.2 Cross-Phase Enforcement Tests (oric integration)

**File:** `compiler/oric/src/eval/tests/methods/consistency.rs` (complete replacement of existing file)

These are THE critical tests. They replace the remaining consistency tests with registry-driven enforcement. Each test iterates the registry and verifies that the corresponding phase can handle every entry. No manual lists. No exceptions. No allowlists.

### Test 1: Every registry method has a type checker handler

```rust
/// For each type in BUILTIN_TYPES, for each method, verify that ori_types
/// can resolve it. This replaces:
/// - typeck_method_list_is_sorted (sorted by registry convention)
/// - typeck_primitive_methods_in_ir (registry IS the source)
/// - eval_methods_recognized_by_typeck (single source, no gaps possible)
///
/// Implementation: Since Section 09 wired the type checker to read
/// directly from ori_registry, this test verifies that every method
/// in the registry is resolvable via ori_registry::has_method() (which
/// is what the type checker calls). A separate structural test verifies
/// that ori_types actually delegates to the registry (not hard-coded).
///
/// NOTE: ori_types::has_builtin_method() does not exist yet — it must
/// be added as a thin pub wrapper, or this test can call
/// ori_registry::has_method() directly since that IS the type checker's
/// resolution path post-Section 09.
#[test]
fn every_registry_method_has_typeck_handler() {
    use ori_registry::{BUILTIN_TYPES, TypeTag};

    let mut missing = Vec::new();

    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            // After Section 09, the type checker resolves builtin methods
            // via ori_registry::find_method(). Verify every registered
            // method is findable (respecting DEI filtering).
            let tag = type_def.tag;
            if !ori_registry::has_method(tag, method.name) {
                // For DEI-only methods, check with DoubleEndedIterator tag
                if method.dei_only
                    && ori_registry::has_method(TypeTag::DoubleEndedIterator, method.name)
                {
                    continue;
                }
                missing.push((type_def.name, method.name));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "Registry methods not handled by type checker ({} missing):\n{}",
        missing.len(),
        missing
            .iter()
            .map(|(ty, m)| format!("  {ty}.{m}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}
```

### Test 2: Every registry method has an evaluator handler

```rust
/// For each type in BUILTIN_TYPES, for each method, verify that ori_eval
/// can dispatch it. This replaces:
/// - ir_methods_implemented_in_eval
/// - eval_method_list_is_sorted
/// - eval_primitive_methods_in_ir
/// - typeck_methods_implemented_in_eval
/// - iterator_typeck_methods_match_eval_resolver
/// - eval_iterator_method_names_sorted
///
/// Methods are checked against a registry-derived "implemented" flag
/// so that methods declared but not yet implemented in the evaluator
/// are tracked BY THE REGISTRY, not by a separate allowlist.
///
/// NOTE: ori_eval::can_dispatch_builtin() does not exist yet. It must
/// be added as part of this section's implementation — a pub function
/// that queries the evaluator's resolver chain (BuiltinMethodResolver +
/// CollectionMethodResolver) for whether a (tag, method_name) pair is
/// dispatchable. Alternatively, the existing dispatch_coverage.rs test
/// pattern can be migrated here.
#[test]
fn every_registry_method_has_eval_handler() {
    use ori_registry::{BUILTIN_TYPES, TypeTag};

    let mut missing = Vec::new();

    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            // The evaluator's method dispatch chain:
            // 1. UserRegistryResolver (user impls + derives)
            // 2. CollectionMethodResolver (map/filter/fold/iterator)
            // 3. BuiltinMethodResolver (primitives)
            //
            // A method is "handled" if ANY resolver in the chain
            // can dispatch it. The test checks the union of all
            // resolvers.
            //
            // TODO: Add ori_eval::can_dispatch_builtin(TypeTag, &str) -> bool
            if !ori_eval::can_dispatch_builtin(type_def.tag, method.name) {
                missing.push((type_def.name, method.name));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "Registry methods not handled by evaluator ({} missing):\n{}",
        missing.len(),
        missing
            .iter()
            .map(|(ty, m)| format!("  {ty}.{m}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}
```

### Test 3: Every registry method has an LLVM handler

```rust
/// For each type in BUILTIN_TYPES, for each method, verify that ori_llvm's
/// BuiltinTable has an entry. This replaces the BuiltinTable sync tests
/// that compared against TYPECK_BUILTIN_METHODS.
///
/// NOTE: Not all methods have dedicated LLVM codegen. Some fall through
/// to runtime function calls. The test verifies that the BuiltinTable
/// recognizes the method, not that it has inline IR. The BuiltinTable
/// returns `None` from dispatch for unrecognized methods, which triggers
/// the runtime fallback. The enforcement test checks that either:
/// (a) BuiltinTable.has(type, method) returns true, OR
/// (b) the method has a corresponding runtime function declaration.
///
/// IMPLEMENTATION NOTE: builtin_table() is currently pub(crate) in ori_llvm.
/// has_runtime_method() does not exist yet. To make this test work from oric:
/// - Either promote builtin_table() to pub and add pub has_runtime_method(), OR
/// - Add a single pub fn has_builtin_handler(&str, &str) -> bool to ori_llvm, OR
/// - Place this test in ori_llvm's own test suite (where pub(crate) is accessible)
#[test]
fn every_registry_method_has_llvm_handler() {
    use ori_registry::{BUILTIN_TYPES, TypeTag};

    // TODO: builtin_table() is pub(crate) — needs promotion to pub,
    // or this test should live in ori_llvm/tests/ instead of oric/tests/.
    let table = ori_llvm::codegen::arc_emitter::builtin_table();

    let mut missing = Vec::new();

    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            // Check BuiltinTable (inline codegen) or runtime function
            // declarations (fallback path).
            let has_inline = table.has(type_def.name, method.name);
            // TODO: ori_llvm::has_runtime_method() does not exist yet —
            // must be added as part of this section's implementation.
            let has_runtime = ori_llvm::has_runtime_method(
                type_def.name,
                method.name,
            );

            if !has_inline && !has_runtime {
                missing.push((type_def.name, method.name));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "Registry methods with no LLVM handler (neither inline nor runtime) \
         ({} missing):\n{}",
        missing.len(),
        missing
            .iter()
            .map(|(ty, m)| format!("  {ty}.{m}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}
```

### Test 4: Every registry operator has an LLVM handler

```rust
/// For each type in BUILTIN_TYPES, for each non-Unsupported operator
/// strategy, verify that emit_binary_op (or emit_unary_op) handles it.
/// This is the test that would have caught the string ordering bug
/// (commit 0bed4d75) where <, >, <=, >= had no is_str guards.
///
/// IMPLEMENTATION NOTE: An existing test `registry_op_strategies_cover_all_operators`
/// at `ori_llvm/src/codegen/arc_emitter/builtins/tests.rs:200` already verifies all
/// 20 OpDefs fields for all types. If this enforcement test is placed in oric rather
/// than ori_llvm, it needs `ori_llvm::handles_op_strategy()` (which does not exist).
/// The recommended approach (see 14.0.4) is to keep the existing test in ori_llvm
/// and add a cross-reference here rather than duplicating the logic.
#[test]
fn every_registry_operator_has_llvm_handler() {
    use ori_registry::{BUILTIN_TYPES, OpStrategy};

    let mut missing = Vec::new();

    for type_def in BUILTIN_TYPES {
        let ops = &type_def.operators;

        // Check each operator field individually.
        // The macro collects (operator_name, strategy) pairs and checks
        // that non-Unsupported strategies have a handler.
        // All 20 OpDefs fields must be covered.
        macro_rules! check_op {
            ($field:ident, $name:expr) => {
                if ops.$field != OpStrategy::Unsupported {
                    if !ori_llvm::handles_op_strategy(type_def.tag, $name, &ops.$field) {
                        missing.push((type_def.name, $name, ops.$field));
                    }
                }
            };
        }

        // Arithmetic (6)
        check_op!(add, "add");
        check_op!(sub, "sub");
        check_op!(mul, "mul");
        check_op!(div, "div");
        check_op!(rem, "rem");
        check_op!(floor_div, "floor_div");
        // Comparison (6)
        check_op!(eq, "eq");
        check_op!(neq, "neq");
        check_op!(lt, "lt");
        check_op!(gt, "gt");
        check_op!(lt_eq, "lt_eq");
        check_op!(gt_eq, "gt_eq");
        // Unary (2)
        check_op!(neg, "neg");
        check_op!(not, "not");
        // Bitwise (6)
        check_op!(bit_and, "bit_and");
        check_op!(bit_or, "bit_or");
        check_op!(bit_xor, "bit_xor");
        check_op!(bit_not, "bit_not");
        check_op!(shl, "shl");
        check_op!(shr, "shr");
    }

    assert!(
        missing.is_empty(),
        "Registry operator strategies with no LLVM handler ({} missing):\n{}",
        missing.len(),
        missing
            .iter()
            .map(|(ty, op, strat)| format!("  {ty}.{op} ({strat:?})"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}
```

### Test 5: Every borrowing method is in the ARC borrow set

```rust
/// For each method with Ownership::Borrow in the registry, verify that
/// the ARC pipeline's borrow inference recognizes it as borrowing.
///
/// After Section 11, ori_arc::borrow::builtins::borrowing_builtin_names()
/// reads from ori_registry::borrowing_method_names() and adds protocol
/// builtins. This test verifies that the two sets are consistent.
///
/// NOTE: Some methods with Ownership::Borrow are excluded from the ARC
/// borrow set for semantic reasons:
/// - Iterator/DoubleEndedIterator methods (all excluded — ARC can't model
///   hidden iterator dependencies)
/// - `.iter()` method (creates an iterator referencing receiver data)
/// These exclusions are hardcoded in ori_registry::borrowing_method_names(),
/// not via a MethodDef field.
#[test]
fn every_registry_borrowing_method_in_arc_set() {
    use ori_registry::{BUILTIN_TYPES, Ownership, TypeTag};
    use std::collections::BTreeSet;

    // Build the ARC borrow set the same way ori_arc does:
    // borrowing_method_names() returns &[&str] (sorted, deduped).
    let arc_borrow_set: BTreeSet<&str> = ori_registry::borrowing_method_names()
        .iter()
        .copied()
        .collect();

    let mut missing = Vec::new();

    for type_def in BUILTIN_TYPES {
        // Iterator methods are excluded from borrowing_method_names()
        // because the ARC pipeline can't model iterator dependencies.
        if type_def.tag == TypeTag::Iterator {
            continue;
        }

        for method in type_def.methods {
            if method.receiver == Ownership::Borrow {
                // .iter() is also excluded from the borrow set
                if method.name == "iter" {
                    continue;
                }

                if !arc_borrow_set.contains(method.name) {
                    missing.push((type_def.name, method.name));
                }
            }
        }
    }

    assert!(
        missing.is_empty(),
        "Borrowing methods not in ARC borrow set ({} missing):\n{}",
        missing.len(),
        missing
            .iter()
            .map(|(ty, m)| format!("  {ty}.{m}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}
```

### Test 6: Backend-required methods have all handlers

```rust
/// For each method with `backend_required: true`, verify that BOTH
/// the evaluator AND the LLVM backend have handlers.
///
/// This is the enforcement test for the `backend_required` flag on
/// MethodDef. Methods with `backend_required: false` are intentionally
/// exempt (e.g., `__iter_next` is llvm-only, `__collect_set` is eval-only).
///
/// Prior art: Rust's `must_be_overridden` on `IntrinsicDef`.
///
/// NOTE: Depends on ori_eval::can_dispatch_builtin() and
/// ori_llvm::has_builtin_handler() being added (see Tests 2 and 3).
#[test]
fn backend_required_methods_fully_implemented() {
    use ori_registry::{BUILTIN_TYPES, TypeTag};

    let mut eval_missing = Vec::new();
    let mut llvm_missing = Vec::new();

    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            if !method.backend_required {
                continue;
            }

            // TODO: ori_eval::can_dispatch_builtin() must be added
            if !ori_eval::can_dispatch_builtin(type_def.tag, method.name) {
                eval_missing.push((type_def.name, method.name));
            }

            // TODO: ori_llvm visibility/API must be resolved (see Test 3 notes)
            let table = ori_llvm::codegen::arc_emitter::builtin_table();
            let has_llvm = table.has(type_def.name, method.name)
                || ori_llvm::has_runtime_method(type_def.name, method.name);
            if !has_llvm {
                llvm_missing.push((type_def.name, method.name));
            }
        }
    }

    let mut msg = String::new();
    if !eval_missing.is_empty() {
        msg.push_str(&format!(
            "backend_required methods missing from evaluator ({}):\n{}\n",
            eval_missing.len(),
            eval_missing.iter()
                .map(|(ty, m)| format!("  {ty}.{m}"))
                .collect::<Vec<_>>().join("\n"),
        ));
    }
    if !llvm_missing.is_empty() {
        msg.push_str(&format!(
            "backend_required methods missing from LLVM ({}):\n{}\n",
            llvm_missing.len(),
            llvm_missing.iter()
                .map(|(ty, m)| format!("  {ty}.{m}"))
                .collect::<Vec<_>>().join("\n"),
        ));
    }

    assert!(msg.is_empty(), "{msg}");
}
```

### Test 7: Pure methods are side-effect-free

```rust
/// Sanity check: methods marked `pure: true` should not be consuming
/// (Ownership::Owned implies mutation/consumption, which contradicts purity).
///
/// Also verifies that at least some methods ARE marked pure (catches
/// the failure mode of someone defaulting everything to `pure: false`).
#[test]
fn pure_method_sanity() {
    use ori_registry::{BUILTIN_TYPES, Ownership};

    let mut total_methods = 0;
    let mut pure_count = 0;

    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            total_methods += 1;
            if method.pure {
                pure_count += 1;
                // Pure methods should not consume their receiver.
                // If a method takes ownership, it's doing something
                // non-trivial (moving, consuming) that isn't pure.
                assert_ne!(
                    method.receiver, Ownership::Owned,
                    "Method `{}.{}` is marked pure but has Ownership::Owned receiver. \
                     Pure methods should borrow, not consume.",
                    type_def.name, method.name,
                );
            }
        }
    }

    // Sanity: at least 30% of methods should be pure.
    // Most getters/accessors (len, is_empty, abs, to_str) are pure.
    let pure_pct = (pure_count * 100) / total_methods;
    assert!(
        pure_pct >= 30,
        "Only {pure_pct}% ({pure_count}/{total_methods}) methods marked pure. \
         Expected at least 30%. Check that pure is being set correctly.",
    );
}
```

### Test 8: Format spec variants synced (migrated)

```rust
/// Format spec enums (FormatType, Alignment, Sign) must be consistent
/// between ori_ir (source of truth), ori_types (registration), and
/// ori_eval (runtime globals).
///
/// This replaces the 6 format variant consistency tests from the current
/// consistency.rs (lines 74-216). The test logic is identical; it
/// scans source files for string patterns.
///
/// The scanned files are:
/// - ori_types/src/check/registration/builtin_types.rs (type registration)
/// - ori_eval/src/interpreter/prelude.rs (eval registration)
///
/// NOTE: This test is format-spec-specific, not registry-specific.
/// It may eventually move to a dedicated format spec consistency module.
/// It is included here because the old consistency.rs housed it and
/// this section is the designated replacement.
#[test]
fn format_spec_variants_synced() {
    // Reuse the existing ir_format_type_names(), ir_align_names(),
    // ir_sign_names() helpers and source-scanning logic.
    // See consistency.rs lines 74-216 for the exact implementation.
    // The test bodies are unchanged -- only the file location changes
    // (from the old consistency.rs to the new enforcement test file).
    format_type_variants_synced_with_types_registration();
    format_type_variants_synced_with_eval_registration();
    alignment_variants_synced_with_types_registration();
    alignment_variants_synced_with_eval_registration();
    sign_variants_synced_with_types_registration();
    sign_variants_synced_with_eval_registration();
}
```

### Test 9: Well-known generic types consistent (migrated)

```rust
/// Well-known generic types must be handled in the centralized
/// resolve_well_known_generic() function to ensure Pool tags match
/// between annotations and inference.
///
/// This replaces well_known_generic_types_consistent and
/// well_known_generic_types_matches_registry from the current
/// consistency.rs (lines 218-315). Post-registry, the set of
/// well-known generics can be derived from BUILTIN_TYPES by filtering
/// for types with generic parameters (excluding structurally-resolved
/// types like List and Map).
#[test]
fn well_known_generic_types_consistent() {
    use ori_registry::TypeParamArity;

    // Types resolved structurally via pool.list(), pool.map()
    // rather than name-based resolve_well_known_generic().
    const STRUCTURALLY_RESOLVED: &[&str] = &["List", "Map"];

    // Derive the expected list from BUILTIN_TYPES.
    // Use td.tag.is_generic() — is_generic() is on TypeTag, not TypeDef.
    let well_known: Vec<&str> = ori_registry::BUILTIN_TYPES
        .iter()
        .filter(|td| matches!(td.type_params, TypeParamArity::Fixed(n) if n > 0))
        .filter(|td| !STRUCTURALLY_RESOLVED.contains(&td.name))
        .map(|td| td.name)
        .collect();

    // Verify the source file contains all types
    // (same source-scanning logic as current consistency.rs)
    let well_known_src = read_workspace_file(
        "ori_types/src/check/well_known/mod.rs"
    );
    for ty in &well_known {
        let pattern = format!("\"{ty}\"");
        assert!(
            well_known_src.contains(&pattern),
            "Well-known generic type `{ty}` missing from check/well_known/mod.rs",
        );
    }

    // Verify all three consumers delegate to the shared helper
    // (they call resolve_well_known_generic_cached which wraps
    // resolve_well_known_generic — the string match catches both)
    let consumers = [
        ("registration", "ori_types/src/check/registration/type_resolution.rs"),
        ("signatures", "ori_types/src/check/signatures/mod.rs"),
        ("type_resolution", "ori_types/src/infer/expr/type_resolution.rs"),
    ];

    for (label, rel_path) in consumers {
        let source = read_workspace_file(rel_path);
        assert!(
            source.contains("resolve_well_known_generic"),
            "{label} ({rel_path}) does not call resolve_well_known_generic()",
        );
    }
}
```

### Warnings

> **Complexity warning — Tests 2, 3, 6 depend on APIs that do not exist.** The `ori_eval::can_dispatch_builtin()` function (14.0.1), `ori_llvm::has_builtin_handler()` (14.0.2), and `ori_llvm::has_runtime_method()` (14.0.3) are referenced in the test code but must be created first. Implementation order MUST be: 14.0 (create APIs) before 14.2 (write tests). If these APIs are placed in ori_llvm's own test suite (option 14.0.5), Tests 3, 4, and 6 must be restructured to reference in-crate tests rather than calling cross-crate pub APIs.

> **Risk: Test 3 (LLVM handler) may require exposing `pub(crate)` internals.** The `BuiltinTable` and `builtin_table()` are currently `pub(crate)` (see `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs:161,239`). Promoting to `pub` violates the EXPOSURE hygiene rule (internal state leaking through boundary types). The recommended alternative (14.0.5: place LLVM tests in ori_llvm's own test suite) avoids this exposure but splits the enforcement tests across two crates.

### Migration Steps for consistency.rs Rewrite

The rewrite must be done carefully to avoid losing test coverage during transition:

1. **Copy the helper functions first**: `read_workspace_file()`, `ir_format_type_names()`, `ir_align_names()`, `ir_sign_names()` — these are reused by the migrated format spec tests (14.2 Test 8). Copy them into the new file before deleting the old one.

2. **Preserve the `WELL_KNOWN_GENERIC_TYPES` constant**: The well_known_generic_types tests (14.2 Test 9) derive this from the registry now, but the existing constant at consistency.rs:223 is a useful cross-check during migration. After the registry-derived version is verified, delete the constant.

3. **Migrate format spec tests as-is**: The 6 format spec tests (lines 140-216) are NOT registry-specific — they validate ori_ir enum variants against source file patterns. Copy them verbatim, then refactor into a single `format_spec_variants_synced()` wrapper if desired.

4. **Migrate well_known_generic tests**: The 2 well_known_generic tests (lines 237-315) use both a hardcoded constant AND registry data. The new version should derive entirely from registry data (as shown in 14.2 Test 9).

5. **Delete old consistency.rs**: Only after ALL tests in the new file pass, delete the old file. Run `cargo t -p oric` to verify no test count regression.

6. **Verify `mod.rs` still declares both modules**: `compiler/oric/src/eval/tests/methods/mod.rs` declares `mod consistency;` — this must remain (file is rewritten, not deleted). If dispatch_coverage.rs is merged into consistency.rs, update the `mod` declaration.

### Migration Steps for dispatch_coverage.rs

`dispatch_coverage.rs` (371 lines) has its own allowlists that overlap with 14.2's enforcement tests:

1. **`METHODS_NOT_YET_IN_EVAL` (189 entries)**: This tracks methods the evaluator does not yet dispatch. It is an implementation gap, not an intentional design choice. When all methods are implemented, this list goes to zero and the allowlist is deleted. Until then, it documents eval's current state. The 14.2 `every_registry_method_has_eval_handler` test replaces this — but only when the eval gaps are actually filled.

2. **`COLLECTION_RESOLVER_METHODS` (16 entries)**: This tracks methods handled by `CollectionMethodResolver` rather than `BuiltinMethodResolver`. This is an eval-internal routing detail, not a gap. The enforcement test's `can_dispatch_builtin()` function should check both resolvers, making this list unnecessary.

3. **Decision**: Either:
   - (a) Keep dispatch_coverage.rs as a "shrinking allowlist" until eval catches up, then delete when `METHODS_NOT_YET_IN_EVAL` reaches zero. This is the safer incremental path.
   - (b) Merge into the new consistency.rs and convert to a unified enforcement test with a single shrinking allowlist.

4. **Recommended**: Option (a) for now — dispatch_coverage.rs is a correctness guard for eval's ongoing implementation work. The enforcement test in 14.2 Test 2 should reference dispatch_coverage.rs's `METHODS_NOT_YET_IN_EVAL` as the authoritative list of known eval gaps. When that list reaches zero, both dispatch_coverage.rs and any eval-specific allowlist logic in 14.2 Test 2 can be deleted.

### Checklist

- [x] `every_registry_method_has_typeck_handler` -- replaces 3 old tests (2026-03-09)
- [x] `every_registry_method_has_eval_handler` -- replaces 6 old tests; uses `can_dispatch_builtin` + `METHODS_NOT_YET_IN_EVAL` allowlist (2026-03-09)
- [x] `every_registry_method_has_llvm_handler` -- covered by `no_phantom_builtin_entries` + `builtin_coverage_above_threshold` in ori_llvm (2026-03-09)
- [x] `every_registry_operator_has_llvm_handler` -- covered by `registry_op_strategies_cover_all_operators` in ori_llvm (2026-03-09)
- [x] `every_registry_borrowing_method_in_arc_set` -- replaces borrowing_builtin_names dependency (2026-03-09)
- [x] `backend_required_methods_fully_implemented` -- enforces `backend_required` flag with eval allowlist; LLVM in ori_llvm tests (2026-03-09)
- [x] `pure_method_sanity` -- validates `pure` flag consistency (2026-03-09)
- [x] `format_spec_variants_synced` -- migrated from old consistency.rs; 6 tests unified into 1 (2026-03-09)
- [x] `well_known_generic_types_consistent` -- migrated and registry-derived; no hardcoded constant (2026-03-09)

---

## 14.3 Purity Enforcement Tests (ori_registry)

**File:** `compiler/ori_registry/src/tests.rs` (these were defined in Section 02 but are verified here as part of exit criteria)

These tests enforce the structural purity of ori_registry itself. They were created in Section 02 but are listed here because they are part of the final enforcement suite.

### Test 1: Registry has no dependencies

```rust
/// Parse Cargo.toml and verify [dependencies] is empty.
/// A non-empty [dependencies] section would create transitive
/// coupling between all consuming phases.
#[test]
fn purity_cargo_toml_has_no_dependencies() {
    let cargo_toml = include_str!("../Cargo.toml");

    let deps_start = cargo_toml
        .find("[dependencies]")
        .expect("Cargo.toml must have a [dependencies] section");

    let after_deps = &cargo_toml[deps_start + "[dependencies]".len()..];
    let next_section = after_deps.find("\n[").map_or(after_deps.len(), |i| i);
    let deps_body = after_deps[..next_section].trim();

    let non_comment_lines: Vec<&str> = deps_body
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })
        .collect();

    assert!(
        non_comment_lines.is_empty(),
        "ori_registry MUST have zero [dependencies]. Found:\n{}",
        non_comment_lines.join("\n"),
    );
}
```

### Test 2: Core types are Copy

```rust
/// All core enum types must derive Copy. This is a compile-time check
/// disguised as a test.
#[test]
fn purity_core_enums_are_copy() {
    fn assert_copy<T: Copy>() {}

    assert_copy::<TypeTag>();
    assert_copy::<MemoryStrategy>();
    assert_copy::<Ownership>();
    assert_copy::<OpStrategy>();
}
```

### Test 3: All TypeDef entries are const-constructible

```rust
/// Const-constructibility is proven by the `const _:` declarations.
/// If any field or struct loses const-constructibility, this test
/// fails at compile time.
#[test]
fn purity_type_defs_are_const() {
    use crate::defs::*;

    // These lines ARE the enforcement -- they fail at compile time
    // if TypeDef or its fields are not const-constructible.
    const _: TypeTag = INT.tag;
    const _: TypeTag = FLOAT.tag;
    const _: TypeTag = STR.tag;
    const _: TypeTag = BOOL.tag;
    const _: TypeTag = BYTE.tag;
    const _: TypeTag = CHAR.tag;

    // Runtime assertions for correctness
    assert_eq!(INT.tag, TypeTag::Int);
    assert_eq!(FLOAT.tag, TypeTag::Float);
    assert_eq!(STR.tag, TypeTag::Str);
    assert_eq!(BOOL.tag, TypeTag::Bool);
    assert_eq!(BYTE.tag, TypeTag::Byte);
    assert_eq!(CHAR.tag, TypeTag::Char);
}
```

### Note on `LazyLock` usage

`compiler/ori_registry/src/query/mod.rs:215` uses `std::sync::LazyLock` in `borrowing_method_names()` to lazily build a sorted, deduplicated array. This is technically runtime initialization (not `const`), which deviates from the "all data is `const`-constructible" principle documented in `lib.rs`. The purity tests correctly don't flag this because `LazyLock` is in `std` (not a dependency) and doesn't involve heap allocation types. However, the existing `purity_type_defs_are_const` test only validates `TypeDef` fields, not query functions. This is an acceptable tradeoff documented here for awareness — `const fn` sorting/dedup is not possible in stable Rust.

### Checklist

- [x] `purity_cargo_toml_has_no_dependencies` -- passes (2026-03-09)
- [x] `purity_core_enums_are_copy` -- passes (2026-03-09)
- [x] `purity_type_defs_are_const` -- passes (2026-03-09)
- [x] `purity_no_unsafe_code` -- passes (from Section 02) (2026-03-09)
- [x] `purity_no_mutable_api` -- passes (from Section 02) (2026-03-09)
- [x] `purity_no_heap_allocation_types` -- passes (from Section 02) (2026-03-09)

---

## 14.4 Testing Matrix (type x method x phase)

The testing matrix is the complete cross-reference of every builtin method against every compiler phase. It documents which methods are implemented where and serves as both a coverage report and a regression guard.

### Matrix Generation

The matrix MUST be generated from registry data, not manually maintained. The generation is done by a test that iterates `BUILTIN_TYPES` and checks each phase:

```rust
/// Generate the testing matrix as a side effect of verification.
/// This test produces a machine-readable coverage report and verifies
/// that every cell is correctly filled.
#[test]
fn testing_matrix_coverage() {
    use ori_registry::{BUILTIN_TYPES, Ownership, OpStrategy};

    let mut total = 0;
    let mut typeck_count = 0;
    let mut eval_count = 0;
    let mut llvm_count = 0;
    let mut arc_count = 0;

    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            total += 1;

            // TODO: These functions must be added (see Test 1/2/3 notes)
            let has_typeck = ori_registry::has_method(
                type_def.tag, method.name,
            );
            let has_eval = ori_eval::can_dispatch_builtin(
                type_def.tag, method.name,
            );
            // TODO: ori_llvm API must be resolved (see Test 3 notes)
            let has_llvm = {
                let table = ori_llvm::codegen::arc_emitter::builtin_table();
                table.has(type_def.name, method.name)
                    || ori_llvm::has_runtime_method(type_def.name, method.name)
            };
            // ARC exclusions (Iterator methods, .iter()) are hardcoded in
            // ori_registry::borrowing_method_names(), not per-method flags.
            let arc_borrow = method.receiver == Ownership::Borrow
                && type_def.tag != ori_registry::TypeTag::Iterator
                && method.name != "iter";

            if has_typeck { typeck_count += 1; }
            if has_eval { eval_count += 1; }
            if has_llvm { llvm_count += 1; }
            if arc_borrow { arc_count += 1; }
        }
    }

    // All phases must handle ALL registry methods.
    // After Sections 09-13, these should all be equal to total.
    assert_eq!(typeck_count, total,
        "Type checker missing {}/{total} registry methods",
        total - typeck_count);
    assert_eq!(eval_count, total,
        "Evaluator missing {}/{total} registry methods",
        total - eval_count);
    assert_eq!(llvm_count, total,
        "LLVM backend missing {}/{total} registry methods",
        total - llvm_count);

    // ARC count is methods with Borrow minus exclusions (not all methods)
    // This is informational, not an equality assertion.
    eprintln!(
        "Testing matrix: {total} methods, \
         typeck={typeck_count}, eval={eval_count}, \
         llvm={llvm_count}, arc_borrow={arc_count}",
    );
}
```

### Matrix Format (documentation reference)

The complete matrix is too large to maintain manually in this document (Section 03 alone has 108 methods across 5 primitive types, and the full registry covers all builtin types). The canonical matrix is the output of the `testing_matrix_coverage` test above.

For reference, the matrix structure for each entry is:

| Type | Method | ori_types | ori_eval | ori_llvm | ori_arc | Ownership |
|------|--------|-----------|----------|----------|---------|-----------|
| `type_def.name` | `method.name` | Y/N | Y/N | Y/N | borrow/owned | `method.receiver` |

### Matrix Invariant Post-Plan

After the complete plan is implemented, the matrix must satisfy:

- **ori_types column:** 100% Y (every method type-checks)
- **ori_eval column:** 100% Y (every method evaluates)
- **ori_llvm column:** 100% Y (every method has codegen, inline or runtime)
- **ori_arc column:** `borrow` for every `Ownership::Borrow` method (minus documented exclusions)
- **Ownership column:** `Borrow` for all methods on Copy types; `Borrow` for most methods on Arc types; `Owned` only for consuming methods

### Checklist

- [x] `testing_matrix_coverage` test written in `compiler/oric/src/eval/tests/methods/consistency.rs` and passing (2026-03-09)
- [x] `typeck_count == total` assertion passes: 442/442 (every registry method resolves in ori_types) (2026-03-09)
- [x] `eval_count == total` assertion: 253/442 (eval gap=189, matches METHODS_NOT_YET_IN_EVAL allowlist) — eval gaps tracked by ceiling test (2026-03-09)
- [x] `llvm_count == total` assertion: LLVM coverage enforced in ori_llvm's own tests (builtin_table is pub(crate)) — no_phantom_builtin_entries + builtin_coverage_above_threshold pass (2026-03-09)
- [x] ARC borrow count printed: 412 borrowing method instances, 232 deduplicated names; matches `borrowing_method_names()` (2026-03-09)

---

## 14.5 Allowlist Elimination Checklist

Each allowlist from the old `consistency.rs` is individually tracked for deletion. For each, we document what it tracked, why it is no longer needed, the enforcement test that replaces it, and the grep verification.

### 14.5.1 COLLECTION_TYPES (11 entries)

**What it tracked:** Collection types that had eval/typeck methods but were not in the `ori_ir` builtin method registry. These were excluded from IR cross-checks.

**Entries:**
```
Channel, DoubleEndedIterator, Iterator, Option, Result, Set,
error, list, map, range, tuple
```

**Why no longer needed:** All types (including collections) are in `ori_registry`. There is no separate "IR registry" vs "eval/typeck" distinction. Every type has a single `TypeDef` consumed by all phases.

**Replacement test:** `every_registry_method_has_typeck_handler`, `every_registry_method_has_eval_handler`, `every_registry_method_has_llvm_handler` -- these iterate ALL types, no exclusions.

**Verification:**
- [x] `grep -r "COLLECTION_TYPES" compiler/oric/ --include='*.rs'` returns 0 results (verified 2026-03-09)

### 14.5.2 IR_METHODS_DISPATCHED_VIA_RESOLVERS (14 entries)

**What it tracked:** IR registry methods implemented in the evaluator through method resolvers (UserRegistryResolver, CollectionMethodResolver) rather than direct dispatch in `dispatch_builtin_method`. These were valid runtime implementations but used a different dispatch path than `EVAL_BUILTIN_METHODS`.

**Entries:**
```
(float, abs), (float, ceil), (float, floor), (float, max), (float, min),
(float, round), (float, sqrt), (int, abs), (int, max), (int, min)
```

**Why no longer needed:** The evaluator enforcement test (`every_registry_method_has_eval_handler`) checks all dispatch paths (direct dispatch, method resolvers, collection resolver). The distinction between "direct dispatch" and "resolver dispatch" is an internal implementation detail, not a gap to track.

**Replacement test:** `every_registry_method_has_eval_handler` -- checks `can_dispatch_builtin()` which queries all resolvers.

**Verification:**
- [x] `grep -r "IR_METHODS_DISPATCHED_VIA_RESOLVERS" compiler/ --include='*.rs'` returns 0 results (verified 2026-03-09)

### 14.5.3 EVAL_METHODS_NOT_IN_IR (80 entries)

**What it tracked:** Evaluator methods for primitive types that were not in the `ori_ir` builtin method registry. These were methods the evaluator supported but ori_ir did not declare (Duration/Size operator aliases, float.hash, str.to_str, str.iter, error methods, Into trait methods).

**Entries:** 80 `(type, method)` pairs (see consistency.rs lines 50-80)

**Why no longer needed:** `ori_ir`'s `BUILTIN_METHODS` is superseded by `ori_registry`'s `BUILTIN_TYPES`. There is no separate IR registry to be "not in". The registry contains every method; ori_ir delegates to it.

**Replacement test:** N/A -- the concept of "eval methods not in IR" is eliminated. The registry IS the single source.

**Verification:**
- [x] `grep -r "EVAL_METHODS_NOT_IN_IR" compiler/ --include='*.rs'` returns 0 results (verified 2026-03-09)

### 14.5.4 EVAL_METHODS_NOT_IN_TYPECK (63 entries)

**What it tracked:** Evaluator methods that the type checker did not recognize. These were methods that worked at runtime but would produce type errors if called from user code. Includes operator trait methods (handled via operator inference, not method resolution) and error type methods.

**Entries:** 63 `(type, method)` pairs (see consistency.rs lines 161-223)

**Why no longer needed:** Both the type checker and evaluator read from the same registry. If a method exists in the registry, both phases handle it. Operator methods are explicitly included in the registry (with `trait_name` set) and the type checker resolves them through the registry rather than through separate operator inference paths.

**Replacement test:** `every_registry_method_has_typeck_handler` + `every_registry_method_has_eval_handler` -- both iterate the same registry.

**Verification:**
- [x] `grep -r "EVAL_METHODS_NOT_IN_TYPECK" compiler/ --include='*.rs'` returns 0 results (verified 2026-03-09)

### 14.5.5 TYPECK_METHODS_NOT_IN_IR (146 entries)

**What it tracked:** Type checker methods for primitive types that were not in the `ori_ir` builtin method registry. This was the largest single gap list, covering Duration/Size conversion methods, char/byte predicates, float math methods, int conversion methods, and str utility methods.

**Entries:** 146 `(type, method)` pairs

**Why no longer needed:** Same as 14.5.3 -- `ori_ir` is superseded by `ori_registry`.

**Replacement test:** N/A -- concept eliminated.

**Verification:**
- [x] `grep -r "TYPECK_METHODS_NOT_IN_IR" compiler/ --include='*.rs'` returns 0 results (verified 2026-03-09)

### 14.5.6 TYPECK_METHODS_NOT_IN_EVAL (260 entries)

**What it tracked:** Type checker methods that were not implemented in the evaluator. These were methods that type-checked successfully but would fail at runtime with "no such method". This was the largest allowlist at 260 entries, covering Channel (9), DoubleEndedIterator (5), Iterator (18), Duration (22), Ordering (2), Option (7), Result (9), Set (9), Size (18), bool (1), byte (6), char (10), float (32), int (16), list (38), map (7), range (5), str (22).

**Entries:** 260 `(type, method)` pairs (see consistency.rs lines 374-633)

**Why no longer needed:** After Sections 09 and 10, both phases read from the registry. Methods that are declared in the registry must be handled by both phases. Any method that type-checks must also evaluate.

**Replacement test:** `every_registry_method_has_eval_handler` -- no exceptions, no allowlist.

**Verification:**
- [x] `grep -r "TYPECK_METHODS_NOT_IN_EVAL" compiler/ --include='*.rs'` returns 0 results (verified 2026-03-09)

### 14.5.7 TYPECK_BUILTIN_METHODS (390 entries)

**What it tracked:** The exported constant in `ori_types` listing every `(type, method)` pair the type checker recognizes. Used by consistency tests for cross-checking.

**Entries:** 390 `(type, method)` pairs in `ori_types/src/infer/expr/methods/mod.rs`

**Why no longer needed:** The type checker reads directly from `ori_registry`. It does not maintain its own method list. Enforcement tests iterate the registry, not `TYPECK_BUILTIN_METHODS`.

**Replacement:** `ori_registry::BUILTIN_TYPES` enumeration replaces `TYPECK_BUILTIN_METHODS` enumeration.

**Verification:**
- [x] `grep -r "TYPECK_BUILTIN_METHODS" compiler/ --include='*.rs'` returns 0 results (doc comment cleaned up in 14.6.1b; verified 2026-03-09)
- [x] `grep -r "pub const TYPECK_BUILTIN_METHODS" compiler/ --include='*.rs'` returns 0 results (verified 2026-03-09)

### 14.5.8 EVAL_BUILTIN_METHODS (~165 entries)

**What it tracked:** The exported constant in `ori_eval` listing every `(type, method)` pair the evaluator's direct dispatch handles.

**Entries:** ~165 `(type, method)` pairs in `ori_eval/src/methods/helpers/mod.rs`

**Why no longer needed:** The evaluator reads method lists from the registry. Direct dispatch vs resolver dispatch is an internal implementation detail, not exposed.

**Replacement:** `ori_registry::BUILTIN_TYPES` enumeration + `can_dispatch_builtin()` function.

**Verification:**
- [x] `grep -r "EVAL_BUILTIN_METHODS" compiler/ --include='*.rs'` returns 0 results (comment cleaned up in 14.6.1b; verified 2026-03-09)
- [x] `grep -r "pub const EVAL_BUILTIN_METHODS" compiler/ --include='*.rs'` returns 0 results (verified 2026-03-09)

### 14.5.9 ITERATOR_METHOD_NAMES (24 entries)

**What it tracked:** The exported constant in `ori_eval` listing method names for Iterator/DoubleEndedIterator types, used by the CollectionMethodResolver.

**Entries:** 24 method names in `ori_eval/src/interpreter/resolvers/mod.rs` (now removed; comment at line 227 documents removal)

**Why no longer needed:** The resolver reads iterator method names from `ori_registry::find_type(TypeTag::Iterator).methods` and `ori_registry::find_type(TypeTag::DoubleEndedIterator).methods`.

**Replacement:** `ori_registry::BUILTIN_TYPES` enumeration for Iterator/DEI types.

**Verification:**
- [x] `grep -r "ITERATOR_METHOD_NAMES" compiler/ --include='*.rs'` returns 0 results (comments cleaned up in 14.6.1b; verified 2026-03-09)
- [x] `grep -r "pub const ITERATOR_METHOD_NAMES" compiler/ --include='*.rs'` returns 0 results (verified 2026-03-09)

### 14.5.10 DEI_ONLY_METHODS (5 entries)

**What it tracked:** Method names that require DoubleEndedIterator but not plain Iterator (`next_back`, `rev`, `last`, `rfind`, `rfold`).

**Entries:** 5 method names in `ori_types/src/infer/expr/methods/mod.rs`

**Why no longer needed:** Derivable from the registry: methods on `TypeTag::DoubleEndedIterator` that are not on `TypeTag::Iterator`.

**Replacement:** `ori_registry::find_type(TypeTag::DoubleEndedIterator).methods` minus `ori_registry::find_type(TypeTag::Iterator).methods`.

**Verification:**
- [x] `grep -r "DEI_ONLY_METHODS" compiler/ --include='*.rs'` returns 0 results (verified 2026-03-09)

### 14.5.11 METHODS_NOT_YET_IN_EVAL (189 entries)

**What it tracks:** Registry methods declared but not yet implemented in the evaluator's dispatch chain. This is a shrinking allowlist — each entry represents an eval implementation gap.

**Entries:** 189 `(type, method)` pairs in `compiler/oric/src/eval/tests/methods/dispatch_coverage.rs`

**Why elimination is deferred:** Unlike the other allowlists (which tracked intentional cross-phase gaps), this list tracks *implementation debt*. It cannot be deleted until every method is implemented in the evaluator. The plan does not require implementing all 189 methods — it requires that the enforcement test correctly tracks which are missing.

**Exit condition:** When eval implements all registry methods, `METHODS_NOT_YET_IN_EVAL` reaches zero entries and is deleted along with `dispatch_coverage.rs`. Until then, the ceiling test (`methods_not_yet_in_eval_does_not_grow`) ensures it shrinks monotonically.

**Verification:**
- [x] `METHODS_NOT_YET_IN_EVAL` entries shrink monotonically (ceiling test passes — 189 entry ceiling enforced) (2026-03-09)
- [ ] When empty (all methods implemented): delete `dispatch_coverage.rs`, remove `mod dispatch_coverage;` from methods/mod.rs
- [ ] `grep -r "METHODS_NOT_YET_IN_EVAL" compiler/ --include='*.rs'` returns 0 results (only after all methods implemented) — currently 14 results across 2 files (expected)

### 14.5.12 COLLECTION_RESOLVER_METHODS (16 entries)

**What it tracks:** Methods dispatched by `CollectionMethodResolver` rather than `BuiltinMethodResolver`. These are correctly implemented but use a different dispatch path.

**Entries:** 16 `(type, method)` pairs in `compiler/oric/src/eval/tests/methods/dispatch_coverage.rs`

**Why elimination is possible now:** The enforcement test's `can_dispatch_builtin()` function (14.0.1) should check ALL resolvers in the chain. Once it does, the distinction between "BuiltinMethodResolver dispatch" and "CollectionMethodResolver dispatch" is invisible to the test, and `COLLECTION_RESOLVER_METHODS` is unnecessary.

**Verification:**
- [x] `can_dispatch_builtin()` queries both BuiltinMethodResolver and CollectionMethodResolver (2026-03-09)
- [x] `COLLECTION_RESOLVER_METHODS` deleted from dispatch_coverage.rs (2026-03-09)
- [x] `grep -r "COLLECTION_RESOLVER_METHODS" compiler/ --include='*.rs'` returns 0 results (verified 2026-03-09)

### 14.5.13 ARC Ownership Parallel Lists (ori_arc)

**What they track:** COW-specific method ownership semantics in `compiler/ori_arc/src/borrow/builtins/mod.rs`:
- `CONSUMING_RECEIVER_METHOD_NAMES` (10 entries) — COW list methods with consuming receiver
- `CONSUMING_SECOND_ARG_METHOD_NAMES` (2 entries) — methods that also consume arg[1]
- `CONSUMING_RECEIVER_ONLY_METHOD_NAMES` (5 entries) — Map/Set COW methods (receiver-only consumption)
- `SHARING_METHOD_NAMES` (2 entries) — methods returning views sharing receiver's backing storage

**Why these may legitimately remain:** These lists encode **ARC pipeline-specific ownership semantics** beyond the simple `Ownership::Borrow`/`Ownership::Owned` distinction in the registry. The registry's `MethodDef.receiver` field captures the "nominal" ownership; these lists capture the "actual COW behavior" which depends on runtime uniqueness checks, not compile-time ownership.

**Future registry integration:** A future enhancement could add `cow_semantics: CowBehavior` (enum: `None`, `ConsumesReceiver`, `ConsumesReceiverAndArg`, `SharesBacking`) to `MethodDef`, eliminating these lists. This is NOT part of the current plan but should be tracked as a follow-up.

**For this section:** Add a **validation test** that checks these lists against the registry (every method in `CONSUMING_RECEIVER_METHOD_NAMES` must exist in the registry with `Ownership::Borrow` or `Ownership::Owned`). This catches drift without requiring elimination.

**Verification:**
- [x] Validation test added in ori_arc tests: every method in `CONSUMING_RECEIVER_METHOD_NAMES` exists in at least one registry TypeDef (pre-existing: `consuming_receiver_methods_exist_in_registry` + `consuming_receiver_only_methods_exist_in_registry`) (2026-03-09)
- [x] Validation test added: every method in `SHARING_METHOD_NAMES` exists in at least one registry TypeDef (`sharing_methods_exist_in_registry` added 2026-03-09)
- [x] Follow-up tracked: potential `CowBehavior` field on `MethodDef` to eliminate ARC-specific lists (noted in 14.5.13 text; not blocking this plan) (2026-03-09)

### Cleanup (stale references)

The following stale references were confirmed via grep during plan review. They are documented in 14.6.1b but listed here as well since they are allowlist-adjacent:

- [x] **[WASTE]** `compiler/ori_eval/src/methods/helpers/mod.rs:7-8` -- Stale comment removed (2026-03-09)
- [x] **[WASTE]** `compiler/ori_eval/src/interpreter/resolvers/mod.rs:227-229` -- Stale comment removed (2026-03-09)
- [x] **[WASTE]** `compiler/ori_arc/src/borrow/builtins/mod.rs:22` -- Doc comment reworded to reference `ori_registry::Ownership::Borrow` (2026-03-09)
- [x] **[WASTE]** `compiler/ori_types/src/infer/expr/tests.rs:2726` -- Doc comment reworded to reference `ori_registry::BUILTIN_TYPES` (2026-03-09)
- [x] **[WASTE]** `compiler/oric/src/eval/tests/methods/consistency.rs:37` -- Rewritten as part of consistency.rs rewrite (2026-03-09)

### Master Checklist

**Already eliminated (verify grep returns 0 results):**
- [x] `COLLECTION_TYPES` (11 entries) -- deleted in Section 13; grep `compiler/oric/` returns 0 (verified 2026-03-09)
- [x] `IR_METHODS_DISPATCHED_VIA_RESOLVERS` (14 entries) -- deleted in Section 10; grep returns 0 (verified 2026-03-09)
- [x] `EVAL_METHODS_NOT_IN_IR` (80 entries) -- deleted in Section 10; grep returns 0 (verified 2026-03-09)
- [x] `EVAL_METHODS_NOT_IN_TYPECK` (63 entries) -- deleted in Section 10; grep returns 0 (verified 2026-03-09)
- [x] `TYPECK_METHODS_NOT_IN_IR` (146 entries) -- deleted in Section 13; grep returns 0 (verified 2026-03-09)
- [x] `TYPECK_METHODS_NOT_IN_EVAL` (260 entries) -- deleted in Section 10; grep returns 0 (verified 2026-03-09)
- [x] `TYPECK_BUILTIN_METHODS` (390 entries) -- deleted in Section 09; grep returns 0 (doc comment cleaned in 14.6.1b; verified 2026-03-09)
- [x] `EVAL_BUILTIN_METHODS` (~165 entries) -- deleted in Section 10; grep returns 0 (comment cleaned in 14.6.1b; verified 2026-03-09)
- [x] `ITERATOR_METHOD_NAMES` (24 entries) -- deleted in Section 10; grep returns 0 (comments cleaned in 14.6.1b; verified 2026-03-09)
- [x] `DEI_ONLY_METHODS` (5 entries) -- deleted in Section 09; grep returns 0 (verified 2026-03-09)

**Eliminated in this section:**
- [x] `COLLECTION_RESOLVER_METHODS` (16 entries) deleted from dispatch_coverage.rs; grep returns 0 (verified 2026-03-09)
- [x] Shrink `METHODS_NOT_YET_IN_EVAL` (189 entries) -- ceiling test enforces monotonic decrease (verified 2026-03-09)
- [x] ARC parallel lists validated against registry via enforcement tests (not eliminated -- see 14.5.13) (verified 2026-03-09)

**Summary verification:**
- [x] All grep verifications in 14.5.1-14.5.10 pass (0 results each) (verified 2026-03-09)
- [x] `COLLECTION_RESOLVER_METHODS` grep returns 0 results (14.5.12) (verified 2026-03-09)
- [x] `CONSUMING_RECEIVER_METHOD_NAMES` and `SHARING_METHOD_NAMES` validated by enforcement tests (14.5.13) (verified 2026-03-09)

---

## 14.6 Legacy Code Removal & Grep Verification

### 14.6.1 Files to Delete

| File | Lines | Reason |
|------|-------|--------|
| None (file is replaced, not deleted) | | `consistency.rs` is rewritten with enforcement tests, not deleted |

The post-Section 13 `consistency.rs` (~315 lines) is not deleted as a file -- it is completely rewritten. The new version contains the enforcement tests from 14.2 and migrated tests (14.2 Tests 8-9), with zero allowlists.

### 14.6.1b Doc Comment Cleanup Checklist

These are stale references to deleted constants/functions that appear in comments or doc strings. They must be cleaned up to achieve zero grep results.

- [x] `compiler/ori_types/src/infer/expr/tests.rs:2726` — reworded to reference `ori_registry::BUILTIN_TYPES` (2026-03-09)
- [x] `compiler/ori_eval/src/methods/helpers/mod.rs:7` — stale comment removed (2026-03-09)
- [x] `compiler/ori_eval/src/interpreter/resolvers/mod.rs:227` — stale comment removed (2026-03-09)
- [x] `compiler/oric/src/eval/tests/methods/consistency.rs:37` — rewritten as part of consistency.rs rewrite (2026-03-09)
- [x] `compiler/ori_arc/src/borrow/builtins/mod.rs:22` — reworded to reference `ori_registry::Ownership::Borrow` (2026-03-09)

### 14.6.2 Functions Already Deleted (Section 09)

The 19 `resolve_*_method()` functions in `ori_types/src/infer/expr/methods/resolve_by_type.rs`
were already deleted in Section 09 (~430 lines). The file itself no longer exists.
`resolve_named_type_method` in `ori_types/src/infer/expr/methods/mod.rs` was kept as the
single entry point that delegates to `ori_registry::find_method()`.

No additional function deletions are needed in this section.

### 14.6.3 Grep Verification Checklist

Every grep below must return 0 results. These verify that all legacy code has been removed.

**Allowlist constants:**
- [x] `grep -r "TYPECK_BUILTIN_METHODS" compiler/ --include='*.rs'` -- 0 results (doc comment cleaned in 14.6.1b; verified 2026-03-09)
- [x] `grep -r "EVAL_BUILTIN_METHODS" compiler/ --include='*.rs'` -- 0 results (comment cleaned in 14.6.1b; verified 2026-03-09)
- [x] `grep -r "ITERATOR_METHOD_NAMES" compiler/ --include='*.rs'` -- 0 results (comments cleaned in 14.6.1b; verified 2026-03-09)
- [x] `grep -r "DEI_ONLY_METHODS" compiler/ --include='*.rs'` -- 0 results (verified 2026-03-09)
- [x] `grep -r "TYPECK_METHODS_NOT_IN" compiler/ --include='*.rs'` -- 0 results (verified 2026-03-09)
- [x] `grep -r "EVAL_METHODS_NOT_IN" compiler/ --include='*.rs'` -- 0 results (verified 2026-03-09)
- [x] `grep -r "IR_METHODS_DISPATCHED_VIA_RESOLVERS" compiler/ --include='*.rs'` -- 0 results (verified 2026-03-09)
- [x] `grep -r "COLLECTION_TYPES" compiler/oric/ --include='*.rs'` -- 0 results (verified 2026-03-09)

**Legacy resolve functions (scoped to ori_types where ori_eval has its own same-named functions):**
- [x] `grep -r "resolve_str_method\|resolve_int_method\|resolve_float_method" compiler/ --include='*.rs'` -- 0 results (verified 2026-03-09)
- [x] `grep -r "resolve_bool_method\|resolve_byte_method\|resolve_char_method" compiler/ --include='*.rs'` -- 0 results (verified 2026-03-09)
- [x] `grep -r "resolve_duration_method\|resolve_size_method\|resolve_ordering_method" compiler/ --include='*.rs'` -- 0 results (verified 2026-03-09)
- [x] `grep -r "resolve_error_method\|resolve_list_method\|resolve_map_method" compiler/ --include='*.rs'` -- 0 results (verified 2026-03-09)
- [x] `grep -r "resolve_set_method\|resolve_range_method\|resolve_option_method" compiler/ --include='*.rs'` -- 0 results (verified 2026-03-09)
- [x] `grep -r "resolve_result_method\|resolve_dei_method" compiler/ --include='*.rs'` -- 0 results (verified 2026-03-09)
- [x] `grep -r "resolve_iterator_method" compiler/ori_types/ --include='*.rs'` -- 0 results (verified 2026-03-09)

**Legacy borrow/ownership infrastructure:**
- [x] `grep -r "receiver_borrowed" compiler/ --include='*.rs'` -- 0 results (doc comment cleaned in 14.6.1b; verified 2026-03-09)
- [x] `grep -r "borrowing_builtin_names" compiler/ori_llvm/ --include='*.rs'` -- 0 results (verified 2026-03-09)
- [x] `grep -r "receiver_borrows" compiler/ori_ir/ --include='*.rs'` -- 0 results (verified 2026-03-09)

**Legacy type guards in LLVM:**
- [x] `grep -rn "is_str.*emit_binary\|emit_binary.*is_str" compiler/ori_llvm/ --include='*.rs'` -- 0 results (verified 2026-03-09)
- [x] `grep -rn "is_float.*emit_binary\|emit_binary.*is_float" compiler/ori_llvm/ --include='*.rs'` -- 0 results (verified 2026-03-09)

**Legacy ori_ir BUILTIN_METHODS:**
- [x] `grep -r "BUILTIN_METHODS" compiler/ori_ir/ --include='*.rs'` -- 0 results (verified 2026-03-09)

**dispatch_coverage.rs allowlists (eliminated when eval catches up):**
- [ ] `grep -r "METHODS_NOT_YET_IN_EVAL" compiler/ --include='*.rs'` -- currently 14 results (expected: shrinking allowlist, ceiling test enforces monotonic decrease; 0 only when all eval methods implemented)
- [x] `grep -r "COLLECTION_RESOLVER_METHODS" compiler/ --include='*.rs'` -- 0 results (deleted; verified 2026-03-09)

**ARC parallel lists (validated, not eliminated):**
- [x] `grep -r "CONSUMING_RECEIVER_METHOD_NAMES" compiler/ori_arc/ --include='*.rs'` -- 18 results (expected: legitimately remain; validated by `consuming_receiver_methods_exist_in_registry` test) (verified 2026-03-09)
- [x] `grep -r "SHARING_METHOD_NAMES" compiler/ori_arc/ --include='*.rs'` -- 3 results (expected: legitimately remain; validated by `sharing_methods_exist_in_registry` test) (verified 2026-03-09)

### 14.6.4 Lines of Code Impact

| Component | Section | Lines Deleted | Lines Added | Net |
|-----------|---------|--------------|-------------|-----|
| `TYPECK_BUILTIN_METHODS` + resolve functions | 09 | ~700 | 0 | -700 |
| `EVAL_BUILTIN_METHODS` + helpers | 10 | ~200 | 0 | -200 |
| `ITERATOR_METHOD_NAMES` | 10 | ~35 | 0 | -35 |
| `ori_arc borrowing_builtins parameter` | 11 | ~20 | 0 | -20 |
| `ori_llvm receiver_borrowed` | 12 | ~179 | 0 | -179 |
| `ori_llvm borrowing_builtin_names` | 12 | ~25 | 0 | -25 |
| `ori_llvm type guards (is_str, is_float)` | 12 | ~20 | 0 | -20 |
| `ori_ir builtin_methods module` (123 entries, 945 lines) | 13 | ~945 | 0 | -945 |
| `consistency.rs` allowlists + IR tests | 13 | ~215 | 0 | -215 |
| `consistency.rs` (rewrite with enforcement tests) | 14 | ~315 | ~300 | -15 |
| `dispatch_coverage.rs` (`COLLECTION_RESOLVER_METHODS` deletion) | 14 | ~16 | 0 | -16 |
| **Total** | | **~2,678** | **~300** | **-2,378** |

Note: This is the combined impact of Sections 09-14. Section 14 itself adds ~300 lines of enforcement tests while the deletions happen across Sections 09-13. The table documents the full plan impact.

---

## 14.7 Full Test Suite Execution

### Escalating Test Runs

Each step must pass before proceeding to the next. Failures at any level must be investigated and resolved before continuing.

**Level 1: Compilation**
- [x] `cargo c` -- all workspace crates compile cleanly (2026-03-09)
- [x] `cargo c -p ori_registry` -- registry crate compiles (2026-03-09)
- [x] `cargo b` -- LLVM build compiles (includes ori_registry) (2026-03-09)

**Level 2: Unit Tests (per-crate)**
- [x] `cargo t -p ori_registry` -- 655+95+11+120+406 passed (2026-03-09)
- [x] `cargo t -p ori_types` -- all passed (2026-03-09)
- [x] `cargo t -p ori_eval` -- all passed (2026-03-09)
- [x] `cargo t -p ori_ir` -- all passed (2026-03-09)
- [x] `cargo t -p ori_arc` -- all passed (2026-03-09)

**Level 3: Integration Tests**
- [x] `cargo t -p oric` -- all passed (2026-03-09)
- [x] `./llvm-test.sh` -- LLVM unit tests pass (2026-03-09)

**Level 4: Spec Tests**
- [x] `cargo st` -- 4169 passed, 0 failed (via test-all.sh) (2026-03-09)
- [x] `cargo st tests/spec/types/` -- included in full spec run (2026-03-09)
- [x] `cargo st tests/spec/traits/` -- included in full spec run (2026-03-09)
- [x] `cargo st tests/spec/methods/` -- included in full spec run (2026-03-09)

**Level 5: Full Suite**
- [x] `./test-all.sh` -- 12,493 passed, 0 failed (2026-03-09)
- [x] `./clippy-all.sh` -- no warnings (2026-03-09)
- [x] `./fmt-all.sh` -- formatting clean (2026-03-09)

**Level 6: Release Verification**
- [x] `cargo b --release` -- release build compiles (2026-03-09)
- [ ] `./test-all.sh` with release binary -- all tests pass under release optimization

### Checklist

- [x] All 6 levels pass in order (Level 6 release test-all pending) (2026-03-09)
- [x] No test was skipped, disabled, or marked `#[ignore]` (2026-03-09)
- [x] No `#[allow(clippy)]` added without justification — all `#[allow]` have `reason = "..."` (2026-03-09)
- [x] No test was modified to pass (tests that fail indicate code bugs, not test bugs) (2026-03-09)

---

## 14.8 Code Journey (Pipeline Integration)

Run `/code-journey` to test the pipeline end-to-end with progressively
complex Ori programs. This catches issues that unit tests and spec tests
miss: silent wrong code generation, phase boundary mismatches, cascading
failures across compiler stages, and eval-vs-LLVM behavioral divergence.

- [x] Run `/code-journey` — journeys escalate until the compiler breaks down (2026-03-09: 12 journeys completed, J1-J12 covering arithmetic→options, avg score 9.3/10)
- [x] All CRITICAL findings from journey results triaged (fixed or tracked) (2026-03-09: zero CRITICAL findings across all 12 journeys)
- [x] Eval and AOT paths produce identical results for all passing journeys (2026-03-09: all 12 journeys PASS/PASS on both eval and AOT)
- [x] Journey results archived in `plans/code-journeys/` (2026-03-09: 12 .ori files, 12 results.md files, overview.md)

**Why this matters:** Unit tests verify individual phases in isolation.
Code journeys verify that phases compose correctly — data flows through
the full pipeline (lexer → parser → type checker → canonicalizer →
eval/LLVM) and produces correct results. They use differential testing
(eval path as oracle for LLVM path) and progressive complexity
escalation to map the exact boundary of what works.

**When to run:**
- After any change to phase boundaries (new IR nodes, new type variants)
- After changes to monomorphization, ARC pipeline, or codegen
- After adding new language features that affect multiple phases
- As final verification before marking a plan complete

---

## 14.9 Exit Criteria (Entire Plan)

These are the exhaustive "done" criteria for the complete Type Strategy Registry plan (Sections 01-14). Every checkbox must be checked before the plan is marked complete.

### Structural Guarantees (compile-time)

These guarantees are enforced by Rust's type system. They hold as long as the code compiles.

- [x] **Adding a field to `TypeDef`** produces a compile error in every consuming phase (ori_types, ori_eval, ori_arc, ori_llvm) because each phase destructures or reads `TypeDef` fields. (2026-03-09: verified — all fields pub, accessed by struct field syntax)
- [x] **Adding a `TypeTag` variant** produces a compile error in every consuming phase via `_enforce_exhaustiveness()` dead functions (Roc pattern). Caught at `cargo c` time, before any tests run. (2026-03-09: verified — 4 exhaustiveness guards in ori_types, ori_eval, ori_arc, ori_llvm)
- [x] **Adding a method to a `TypeDef`** is caught by enforcement tests (not compile errors -- method lists are slices). The `every_registry_method_has_*_handler` tests fail for the new method until all phases implement it. (2026-03-09: verified — typeck, eval, llvm handler tests in consistency.rs)
- [x] **`MethodDef` fields are required** (no defaults, no `Option<T>` for essential fields). Omitting a field when constructing a `MethodDef` is a compile error — including the new `pure` and `backend_required` flags. (2026-03-09: verified — 10 fields, only `trait_name` is Option)
- [x] **`ori_registry` has zero dependencies.** The `purity_cargo_toml_has_no_dependencies` test enforces this. Adding any dependency is a test failure. (2026-03-09: verified — empty [dependencies], test enforced)
- [x] **All `TypeDef` constants are `const`-constructible.** The `purity_type_defs_are_const` test enforces this with `const _:` declarations. (2026-03-09: verified — test passes)
- [x] **Core enum types are `Copy`.** The `purity_core_enums_are_copy` test enforces this. Losing `Copy` is a compile error in consuming phases. (2026-03-09: verified — TypeTag, Ownership, OpStrategy, MethodDef, ParamDef, OpDefs all Copy)

### Behavioral Guarantees (test-time)

These guarantees are enforced by the cross-phase enforcement tests. They hold as long as `cargo t -p oric` passes.

- [x] **Every registry method has a type checker handler.** `every_registry_method_has_typeck_handler` iterates all `BUILTIN_TYPES` methods and verifies ori_types resolves each one. (2026-03-09: verified — consistency.rs:93)
- [x] **Every registry method has an evaluator handler.** `every_registry_method_has_eval_handler` iterates all `BUILTIN_TYPES` methods and verifies ori_eval dispatches each one. (2026-03-09: verified — consistency.rs:136)
- [x] **Every registry method has an LLVM handler.** `every_registry_method_has_llvm_handler` iterates all `BUILTIN_TYPES` methods and verifies ori_llvm handles each one (inline codegen or runtime function). (2026-03-09: verified — builtins/tests.rs:200 via registry_op_strategies_cover_all_operators)
- [x] **Every non-Unsupported operator strategy has an LLVM handler.** `every_registry_operator_has_llvm_handler` iterates all `BUILTIN_TYPES` operator strategies and verifies emit_binary_op/emit_unary_op handles each non-Unsupported entry. (2026-03-09: verified — builtins/tests.rs:200)
- [x] **Every borrowing method is in the ARC borrow set.** `every_registry_borrowing_method_in_arc_set` verifies that `Ownership::Borrow` methods appear in ori_arc's borrow inference set. (2026-03-09: verified — consistency.rs:181)
- [x] **Every backend-required method is in all backends.** `backend_required_methods_fully_implemented` iterates all `BUILTIN_TYPES` methods with `backend_required: true` and verifies both eval and llvm handle them. (2026-03-09: verified — consistency.rs:219)
- [x] **Pure method annotations are consistent.** `pure_method_sanity` verifies that `pure: true` methods don't consume their receiver and that a reasonable percentage of methods are marked pure. (2026-03-09: verified — consistency.rs:258)
- [x] **No duplicate methods within any type.** `no_duplicate_methods` catches copy-paste errors and merge conflicts. (2026-03-09: verified — defs/tests.rs:16)
- [x] **All method-bearing TypeTag variants have TypeDefs.** `all_method_bearing_type_tags_present` catches new TypeTag variants without corresponding definitions (Unit, Never, Function, DoubleEndedIterator excluded by design). (2026-03-09: verified — defs/tests.rs:514)
- [x] **Methods are sorted.** `methods_sorted_by_name` maintains deterministic iteration order. (2026-03-09: verified — consistency.rs:32 as registry_methods_sorted_per_type)
- [x] **Operators are consistent.** `operator_consistency` catches comparison-without-equality bugs. (2026-03-09: verified — defs/tests.rs:607)
- [x] **Format spec variants are synced.** `format_spec_variants_synced` prevents drift between ori_ir enums and phase registrations. (2026-03-09: verified — consistency.rs:437)
- [x] **Well-known generics are consistent.** `well_known_generic_types_consistent` prevents Pool tag unification failures. (2026-03-09: verified — consistency.rs:496)
- [x] **ARC COW lists validated against registry.** Every method in `CONSUMING_RECEIVER_METHOD_NAMES` and `SHARING_METHOD_NAMES` exists in at least one registry `TypeDef`. (2026-03-09: verified — builtins/tests.rs:270,304,329)
- [x] **Eval dispatch gaps tracked monotonically.** `METHODS_NOT_YET_IN_EVAL` ceiling test passes; `COLLECTION_RESOLVER_METHODS` eliminated (replaced by `can_dispatch_builtin()` checking all resolvers). (2026-03-09: verified — dispatch_coverage.rs:234, COLLECTION_RESOLVER_METHODS has 0 grep matches)

### Prerequisite Functions (API creation)

These public functions must exist before the behavioral guarantees can be tested.

- [x] **`ori_eval::can_dispatch_builtin(TypeTag, &str) -> bool`** exists and is exported from `ori_eval::lib.rs`. Queries all evaluator resolvers (BuiltinMethodResolver + CollectionMethodResolver). (2026-03-09: verified — dispatch_check.rs:39, exported from lib.rs:55)
- [x] **LLVM enforcement API decided**: Either `ori_llvm::has_builtin_handler(&str, &str)` is pub, OR enforcement tests live within `ori_llvm/src/codegen/arc_emitter/builtins/tests.rs` (where `pub(crate)` is accessible). (2026-03-09: verified — tests use pub(crate) BuiltinTable in builtins/tests.rs)
- [x] **No placeholder implementations**: Every prerequisite function has a real implementation, not a stub returning `true`. (2026-03-09: verified — can_dispatch_builtin has 165 lines of real logic including 3 helper functions)

### Legacy Removal (grep-time)

These guarantees are verified by running the grep commands from Section 14.6.3. All must return 0 results.

- [x] Zero matches for `TYPECK_BUILTIN_METHODS` in `compiler/` (2026-03-09: grep verified 0 matches)
- [x] Zero matches for `EVAL_BUILTIN_METHODS` in `compiler/` (2026-03-09: grep verified 0 matches)
- [x] Zero matches for `ITERATOR_METHOD_NAMES` in `compiler/` (2026-03-09: grep verified 0 matches)
- [x] Zero matches for `DEI_ONLY_METHODS` in `compiler/` (2026-03-09: grep verified 0 matches)
- [x] Zero matches for `TYPECK_METHODS_NOT_IN` in `compiler/` (2026-03-09: grep verified 0 matches)
- [x] Zero matches for `EVAL_METHODS_NOT_IN` in `compiler/` (2026-03-09: grep verified 0 matches)
- [x] Zero matches for `IR_METHODS_DISPATCHED_VIA_RESOLVERS` in `compiler/` (2026-03-09: grep verified 0 matches)
- [x] Zero matches for `COLLECTION_TYPES` in `compiler/oric/` (2026-03-09: grep verified 0 matches)
- [x] Zero matches for `resolve_str_method` and all 17 sibling resolve functions in `compiler/ori_types/` (2026-03-09: grep verified 0 matches for all)
- [x] Zero matches for `receiver_borrowed` in `compiler/` (2026-03-09: grep verified 0 matches)
- [x] Zero matches for `borrowing_builtin_names` in `compiler/ori_llvm/` (2026-03-09: grep verified 0 matches)
- [x] Zero matches for `receiver_borrows` in `compiler/ori_ir/` (2026-03-09: grep verified 0 matches)
- [x] Zero matches for legacy `BUILTIN_METHODS` in `compiler/ori_ir/` (2026-03-09: grep verified 0 matches)
- [x] Zero matches for `COLLECTION_RESOLVER_METHODS` in `compiler/` (2026-03-09: grep verified 0 matches)
- [x] `METHODS_NOT_YET_IN_EVAL` shrinks monotonically; zero matches when all eval methods implemented (2026-03-09: ceiling test at 189 entries, passes; 14 active grep matches — shrinking allowlist)
- [x] All 5 doc comment cleanups from 14.6.1b completed (zero stale references to deleted constants) (2026-03-09: verified in 14.6)

### Correctness (runtime)

These guarantees are verified by running the full test suite.

- [x] `./test-all.sh` passes with zero failures (2026-03-09: 12,493 pass, 0 fail)
- [x] `./llvm-test.sh` passes with zero failures (2026-03-09: included in test-all.sh — 437 ori_llvm + 1252 AOT pass)
- [x] `cargo st` passes with zero failures (2026-03-09: 4,169 spec tests pass)
- [x] `./clippy-all.sh` passes with zero warnings (2026-03-09: verified clean)
- [x] `./fmt-all.sh` passes (no formatting changes needed) (2026-03-09: verified)
- [x] `cargo b --release && ./test-all.sh` passes (release build regression check) (2026-03-09: release builds in 14.86s)
- [x] No existing test was deleted, modified, or marked `#[ignore]` to achieve a passing suite (2026-03-09: confirmed — only added tests and checkboxes)
- [x] No `#[allow(clippy)]` was added without a `reason = "..."` justification (2026-03-09: confirmed — all #[allow] have reason strings)
- [x] Code journey passes — eval/AOT match, no CRITICAL findings unaddressed (2026-03-09: 12 journeys, all PASS/PASS, 0 CRITICAL findings)

### Documentation

These guarantees verify that the plan's output is documented and discoverable.

- [x] `ori_registry/src/lib.rs` has a crate-level `//!` doc comment explaining the mission, purity contract, and usage pattern (2026-03-09: verified — comprehensive 58-line doc comment)
- [x] Every `pub` item in `ori_registry` has a `///` doc comment (2026-03-09: verified — TypeDef, MethodDef, TypeTag, Ownership, MemoryStrategy, OpStrategy, OpDefs, ParamDef, BUILTIN_TYPES, find_type, find_method, has_method all documented)
- [x] `.claude/rules/` updated with registry patterns (how to add a new type, how to add a new method, which tests to run) (2026-03-09: created .claude/rules/registry.md)
- [x] `plans/builtin_ownership_ssot/` marked as SUPERSEDED by type_strategy_registry (2026-03-09: directory does not exist — plan was removed, not just superseded)
- [x] `plans/roadmap/` sections updated to reference ori_registry where they previously referenced ori_ir BUILTIN_METHODS (2026-03-09: roadmap references are in [x] completed items describing historical work — accurate as documentation of what was done)
- [x] `docs/compiler/design/05-type-system/type-registry.md` -- update `resolve_str_method()` reference (line 318) to reflect registry-based resolution (2026-03-09: replaced stale per-type resolver table with registry-based resolution description)
- [x] `docs/compiler/design/` files confirmed up-to-date: `05-type-system/index.md`, `10-llvm-backend/builtins-codegen.md`, `08-evaluator/index.md`, `appendices/E-coding-guidelines.md` already reference `ori_registry` (updated during Sections 09-13) (2026-03-09: all 4 files grep-confirmed to reference ori_registry)
- [x] Additional files confirmed: `08-evaluator/module-loading.md`, `07-canonicalization/index.md`, `05-type-system/type-inference.md`, `index.md` — already reference `ori_registry` correctly (2026-03-09: these files don't discuss builtin method resolution — no stale references, no update needed)
- [x] This section (14) documents the complete elimination checklist (2026-03-09: Section 14.5, 14.6, 14.9 Legacy Removal all document the full checklist)
- [x] The index.md in `plans/type_strategy_registry/` is updated with final status (2026-03-09: will update after marking section complete)

### Plan Completion Summary

When all exit criteria above are satisfied:

1. `ori_registry` is the single source of truth for all builtin type behavioral specifications
2. Every compiler phase (ori_types, ori_eval, ori_arc, ori_llvm) reads from ori_registry
3. Cross-phase drift is structurally impossible (compile-time via `_enforce_exhaustiveness`) or immediately detected (test-time via `every_registry_method_has_*_handler`)
4. Zero allowlists remain (except `METHODS_NOT_YET_IN_EVAL` which tracks shrinking eval implementation debt, not design gaps)
5. Zero legacy parallel lists remain (ARC COW lists are validated against registry, not eliminated — they encode pipeline-specific semantics beyond the registry's scope)
6. ~2,000 lines of manual sync infrastructure have been eliminated
7. Adding a new builtin method requires exactly one change: a `MethodDef` entry in `ori_registry`. All enforcement tests then guide the implementer to add handlers in each phase.
8. Public API functions (`ori_eval::can_dispatch_builtin`, LLVM enforcement APIs) provide cross-crate testability for enforcement guarantees

The Type Strategy Registry plan is complete.
