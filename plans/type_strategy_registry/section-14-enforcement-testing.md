---
plan: "type_strategy_registry"
section: "14"
title: "Enforcement Tests, Testing Matrix & Exit Criteria"
status: not-started
goal: "Make cross-phase drift structurally impossible via enforcement tests, eliminate all allowlists, remove all legacy code, and define exhaustive exit criteria for the entire plan"
depends_on:
  - "09"
  - "10"
  - "11"
  - "12"
  - "13"
subsections:
  - id: "14.1"
    title: "Registry-Level Integrity Tests (ori_registry)"
    status: not-started
  - id: "14.2"
    title: "Cross-Phase Enforcement Tests (oric integration)"
    status: not-started
  - id: "14.3"
    title: "Purity Enforcement Tests (ori_registry)"
    status: not-started
  - id: "14.4"
    title: "Testing Matrix (type x method x phase)"
    status: not-started
  - id: "14.5"
    title: "Allowlist Elimination Checklist"
    status: not-started
  - id: "14.6"
    title: "Legacy Code Removal & Grep Verification"
    status: not-started
  - id: "14.7"
    title: "Full Test Suite Execution"
    status: not-started
  - id: "14.8"
    title: "Code Journey (Pipeline Integration)"
    status: not-started
  - id: "14.9"
    title: "Exit Criteria (Entire Plan)"
    status: not-started
---

# Section 14: Enforcement Tests, Testing Matrix & Exit Criteria

**Status:** Not Started
**Goal:** Make cross-phase drift structurally impossible. Eliminate every allowlist. Remove every line of legacy code. Define the "done" criteria for the entire Type Strategy Registry plan.

**Context:** Sections 09-13 wired all consuming phases (ori_types, ori_eval, ori_arc, ori_llvm, ori_ir) to read from ori_registry instead of maintaining independent type knowledge. This section is the final gate: it replaces the ~1,010-line `consistency.rs` (with its 560+ allowlist entries across 6 arrays) with a small set of structural enforcement tests that derive their expectations directly from the registry. No manual lists. No gap tracking. No "known missing" arrays. The registry IS the specification; the enforcement tests verify that every phase faithfully implements it.

**Design rationale:** The old consistency tests were necessary because type knowledge was scattered: `TYPECK_BUILTIN_METHODS` (426 entries), `EVAL_BUILTIN_METHODS` (~165 entries), `BUILTIN_METHODS` in ori_ir (162 entries), `BuiltinTable` in ori_llvm (179 entries), and `borrowing_builtins` in ori_arc. The allowlists (`TYPECK_METHODS_NOT_IN_EVAL`, `EVAL_METHODS_NOT_IN_IR`, etc.) tracked intentional gaps between these independent lists. With the registry as single source of truth, these gaps become structural impossibilities -- a method either exists in the registry (and all phases must handle it) or it does not exist (and no phase references it). The enforcement tests verify this invariant at test time, while Rust's type system enforces it at compile time (adding a field to `TypeDef` is a compile error in every consuming phase).

**What this section replaces:**
- `compiler/oric/src/eval/tests/methods/consistency.rs` (~1,010 lines) -- entire file deleted
- 9 main consistency tests + 6 format variant tests + 1 well-known test
- 6 allowlist arrays totaling 560+ entries
- 2 exported constant arrays (`EVAL_BUILTIN_METHODS`, `TYPECK_BUILTIN_METHODS`)
- 1 exported constant array (`ITERATOR_METHOD_NAMES`)
- All `resolve_*_method()` functions (18 functions in ori_types)

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

### Test 3: All TypeTag variants have a TypeDef

```rust
/// Every variant of TypeTag must have a corresponding TypeDef in BUILTIN_TYPES.
/// If a TypeTag variant exists without a TypeDef, consuming phases will fail
/// to look up methods for that type at runtime.
#[test]
fn all_type_tags_present() {
    use std::collections::BTreeSet;

    let registered_tags: BTreeSet<TypeTag> = BUILTIN_TYPES
        .iter()
        .map(|td| td.tag)
        .collect();

    // Every TypeTag variant that represents a concrete builtin type
    // must appear in the registry. SelfType and Fresh live on
    // ReturnTag (not TypeTag), so no exclusion is needed here.
    let expected_tags = TypeTag::all_concrete();

    for tag in expected_tags {
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

### Checklist

- [ ] `no_duplicate_methods` -- no type has two methods with the same name
- [ ] `no_empty_types` -- every TypeDef has at least one method
- [ ] `all_type_tags_present` -- every TypeTag variant has a TypeDef
- [ ] `methods_sorted_by_name` -- alphabetical within each type
- [ ] `all_receivers_documented` -- every MethodDef has explicit Ownership
- [ ] `no_unsupported_eq` -- every type supports at least `==`
- [ ] `operator_consistency` -- comparison implies equality; consistent strategies
- [ ] `self_type_returns_valid` -- SelfType only on semantically correct methods

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

### Checklist

- [ ] `_enforce_exhaustiveness(TypeTag)` in ori_types — covers all TypeTag variants
- [ ] `_enforce_exhaustiveness(TypeTag)` in ori_eval — covers all TypeTag variants
- [ ] `_enforce_exhaustiveness(TypeTag)` in ori_llvm — covers all TypeTag variants
- [ ] `_enforce_exhaustiveness(TypeTag)` in ori_arc — covers all TypeTag variants
- [ ] Verified: adding a dummy `TypeTag::_Test` variant causes compile errors in all 4 crates

---

## 14.2 Cross-Phase Enforcement Tests (oric integration)

**File:** `compiler/oric/src/eval/tests/methods/consistency.rs` (complete replacement of existing file)

These are THE critical tests. They replace ALL 9 existing consistency tests and ALL 6 allowlist arrays. Each test iterates the registry and verifies that the corresponding phase can handle every entry. No manual lists. No exceptions. No allowlists.

### Test 1: Every registry method has a type checker handler

```rust
/// For each type in BUILTIN_TYPES, for each method, verify that ori_types
/// can resolve it. This replaces:
/// - typeck_method_list_is_sorted (sorted by registry convention)
/// - typeck_primitive_methods_in_ir (registry IS the source)
/// - eval_methods_recognized_by_typeck (single source, no gaps possible)
///
/// Implementation: Call the type checker's method resolution function
/// for each (type, method) pair and verify it returns a valid result.
#[test]
fn every_registry_method_has_typeck_handler() {
    use ori_registry::{BUILTIN_TYPES, TypeTag};

    let mut missing = Vec::new();

    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            // Query ori_types for this method.
            // The exact mechanism depends on how Section 09 wires the
            // type checker: either a direct registry lookup
            // (find_method(tag, name).returns -> Idx) or a
            // registry-driven resolve_builtin_method() that reads
            // from ori_registry instead of hard-coded match arms.
            //
            // The test verifies that the type checker recognizes the
            // method and returns a non-error type index for it.
            if !ori_types::has_builtin_method(type_def.tag, method.name) {
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
            // After Section 10, ori_eval exposes a function like:
            // ori_eval::can_dispatch_builtin(tag, method_name) -> bool
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
#[test]
fn every_registry_method_has_llvm_handler() {
    use ori_registry::{BUILTIN_TYPES, TypeTag};

    let table = ori_llvm::codegen::arc_emitter::builtin_table();

    let mut missing = Vec::new();

    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            // Check BuiltinTable (inline codegen) or runtime function
            // declarations (fallback path).
            let has_inline = table.has(type_def.name, method.name);
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
/// Implementation: The LLVM backend exposes a function that checks
/// whether a given OpStrategy is handled for a given TypeTag.
#[test]
fn every_registry_operator_has_llvm_handler() {
    use ori_registry::{BUILTIN_TYPES, OpStrategy};

    let mut missing = Vec::new();

    for type_def in BUILTIN_TYPES {
        let ops = &type_def.operators;

        // Check each operator field individually.
        // The macro collects (operator_name, strategy) pairs and checks
        // that non-Unsupported strategies have a handler.
        macro_rules! check_op {
            ($field:ident, $name:expr) => {
                if ops.$field != OpStrategy::Unsupported {
                    if !ori_llvm::handles_op_strategy(type_def.tag, $name, &ops.$field) {
                        missing.push((type_def.name, $name, ops.$field));
                    }
                }
            };
        }

        check_op!(add, "add");
        check_op!(sub, "sub");
        check_op!(mul, "mul");
        check_op!(div, "div");
        check_op!(rem, "rem");
        check_op!(floor_div, "floor_div");
        check_op!(eq, "eq");
        check_op!(neq, "neq");
        check_op!(lt, "lt");
        check_op!(gt, "gt");
        check_op!(lt_eq, "lt_eq");
        check_op!(gt_eq, "gt_eq");
        check_op!(neg, "neg");
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
/// This replaces the backwards dependency from ori_arc -> ori_llvm
/// (borrowing_builtin_names). After Section 11, ori_arc reads
/// Ownership directly from ori_registry.
///
/// NOTE: Some methods with Ownership::Borrow may be excluded from the
/// ARC borrow set for semantic reasons (e.g., iter() borrows its
/// receiver but creates an iterator that holds a hidden reference).
/// These exclusions are documented in the registry via a flag
/// (e.g., `arc_excludes_borrow: true`) rather than in a separate
/// allowlist.
#[test]
fn every_registry_borrowing_method_in_arc_set() {
    use ori_registry::{BUILTIN_TYPES, Ownership};

    let mut missing = Vec::new();

    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            if method.receiver == Ownership::Borrow {
                // After Section 11, ori_arc reads ownership directly
                // from the registry. This test verifies that the ARC
                // borrow inference set includes every method the
                // registry marks as borrowing.
                //
                // Methods with arc_excludes_borrow are intentionally
                // excluded (e.g., iter() on str).
                if method.arc_excludes_borrow {
                    continue;
                }

                if !ori_arc::is_borrowing_builtin(type_def.name, method.name) {
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

            if !ori_eval::can_dispatch_builtin(type_def.tag, method.name) {
                eval_missing.push((type_def.name, method.name));
            }

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
/// This replaces the 6 format variant consistency tests from the old
/// consistency.rs (lines 776-933). The test logic is identical; it
/// scans source files for string patterns.
///
/// NOTE: This test is format-spec-specific, not registry-specific.
/// It may eventually move to a dedicated format spec consistency module.
/// It is included here because the old consistency.rs housed it and
/// this section is the designated replacement.
#[test]
fn format_spec_variants_synced() {
    // Reuse the existing ir_format_type_names(), ir_align_names(),
    // ir_sign_names() helpers and source-scanning logic.
    // See the old consistency.rs lines 776-933 for the exact
    // implementation. The test bodies are unchanged -- only the
    // file location changes (from the old consistency.rs to the
    // new enforcement test file).
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
/// This replaces well_known_generic_types_consistent from the old
/// consistency.rs (lines 936-1009). Post-registry, the set of
/// well-known generics can be derived from BUILTIN_TYPES by filtering
/// for types with generic parameters.
#[test]
fn well_known_generic_types_consistent() {
    // After registry wiring, this test derives the expected list
    // from BUILTIN_TYPES.iter().filter(|td| td.is_generic()) instead
    // of maintaining the WELL_KNOWN_GENERIC_TYPES const array.
    //
    // The consumer verification (checking that resolve_well_known_generic
    // is called in all three resolution functions) remains unchanged.
    let well_known: Vec<&str> = ori_registry::BUILTIN_TYPES
        .iter()
        .filter(|td| td.is_generic())
        .map(|td| td.name)
        .collect();

    // Verify the source file contains all types
    // (same source-scanning logic as old consistency.rs)
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

### Checklist

- [ ] `every_registry_method_has_typeck_handler` -- replaces 3 old tests
- [ ] `every_registry_method_has_eval_handler` -- replaces 6 old tests
- [ ] `every_registry_method_has_llvm_handler` -- replaces BuiltinTable sync tests
- [ ] `every_registry_operator_has_llvm_handler` -- new (would have caught string ordering bug)
- [ ] `every_registry_borrowing_method_in_arc_set` -- replaces borrowing_builtin_names dependency
- [ ] `backend_required_methods_fully_implemented` -- new (enforces `backend_required` flag)
- [ ] `pure_method_sanity` -- new (validates `pure` flag consistency)
- [ ] `format_spec_variants_synced` -- migrated from old consistency.rs
- [ ] `well_known_generic_types_consistent` -- migrated and registry-derived

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

### Checklist

- [ ] `purity_cargo_toml_has_no_dependencies` -- passes
- [ ] `purity_core_enums_are_copy` -- passes
- [ ] `purity_type_defs_are_const` -- passes
- [ ] `purity_no_unsafe_code` -- passes (from Section 02)
- [ ] `purity_no_mutable_api` -- passes (from Section 02)
- [ ] `purity_no_heap_allocation_types` -- passes (from Section 02)

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

            let has_typeck = ori_types::has_builtin_method(
                type_def.tag, method.name,
            );
            let has_eval = ori_eval::can_dispatch_builtin(
                type_def.tag, method.name,
            );
            let has_llvm = {
                let table = ori_llvm::codegen::arc_emitter::builtin_table();
                table.has(type_def.name, method.name)
                    || ori_llvm::has_runtime_method(type_def.name, method.name)
            };
            let arc_borrow = method.receiver == Ownership::Borrow
                && !method.arc_excludes_borrow;

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

- [ ] `testing_matrix_coverage` test written and passing
- [ ] All four phase columns show 100% coverage
- [ ] ARC borrow column matches registry Ownership annotations
- [ ] Test output shows total method count across all types

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
- [ ] `grep -r "COLLECTION_TYPES" compiler/ --include='*.rs'` returns 0 results
- [ ] `grep -r "COLLECTION_TYPES" compiler/oric/ --include='*.rs'` returns 0 results

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
- [ ] `grep -r "IR_METHODS_DISPATCHED_VIA_RESOLVERS" compiler/ --include='*.rs'` returns 0 results

### 14.5.3 EVAL_METHODS_NOT_IN_IR (80 entries)

**What it tracked:** Evaluator methods for primitive types that were not in the `ori_ir` builtin method registry. These were methods the evaluator supported but ori_ir did not declare (Duration/Size operator aliases, float.hash, str.to_str, str.iter, error methods, Into trait methods).

**Entries:** 80 `(type, method)` pairs (see consistency.rs lines 50-80)

**Why no longer needed:** `ori_ir`'s `BUILTIN_METHODS` is superseded by `ori_registry`'s `BUILTIN_TYPES`. There is no separate IR registry to be "not in". The registry contains every method; ori_ir delegates to it.

**Replacement test:** N/A -- the concept of "eval methods not in IR" is eliminated. The registry IS the single source.

**Verification:**
- [ ] `grep -r "EVAL_METHODS_NOT_IN_IR" compiler/ --include='*.rs'` returns 0 results

### 14.5.4 EVAL_METHODS_NOT_IN_TYPECK (63 entries)

**What it tracked:** Evaluator methods that the type checker did not recognize. These were methods that worked at runtime but would produce type errors if called from user code. Includes operator trait methods (handled via operator inference, not method resolution) and error type methods.

**Entries:** 63 `(type, method)` pairs (see consistency.rs lines 161-223)

**Why no longer needed:** Both the type checker and evaluator read from the same registry. If a method exists in the registry, both phases handle it. Operator methods are explicitly included in the registry (with `trait_name` set) and the type checker resolves them through the registry rather than through separate operator inference paths.

**Replacement test:** `every_registry_method_has_typeck_handler` + `every_registry_method_has_eval_handler` -- both iterate the same registry.

**Verification:**
- [ ] `grep -r "EVAL_METHODS_NOT_IN_TYPECK" compiler/ --include='*.rs'` returns 0 results

### 14.5.5 TYPECK_METHODS_NOT_IN_IR (143 entries)

**What it tracked:** Type checker methods for primitive types that were not in the `ori_ir` builtin method registry. This was the largest single gap list, covering Duration/Size conversion methods, char/byte predicates, float math methods, int conversion methods, and str utility methods.

**Entries:** 143 `(type, method)` pairs (see consistency.rs lines 227-369)

**Why no longer needed:** Same as 14.5.3 -- `ori_ir` is superseded by `ori_registry`.

**Replacement test:** N/A -- concept eliminated.

**Verification:**
- [ ] `grep -r "TYPECK_METHODS_NOT_IN_IR" compiler/ --include='*.rs'` returns 0 results

### 14.5.6 TYPECK_METHODS_NOT_IN_EVAL (260 entries)

**What it tracked:** Type checker methods that were not implemented in the evaluator. These were methods that type-checked successfully but would fail at runtime with "no such method". This was the largest allowlist at 260 entries, covering Channel (9), DoubleEndedIterator (5), Iterator (18), Duration (22), Ordering (2), Option (7), Result (9), Set (9), Size (18), bool (1), byte (6), char (10), float (32), int (16), list (38), map (7), range (5), str (22).

**Entries:** 260 `(type, method)` pairs (see consistency.rs lines 374-633)

**Why no longer needed:** After Sections 09 and 10, both phases read from the registry. Methods that are declared in the registry must be handled by both phases. Any method that type-checks must also evaluate.

**Replacement test:** `every_registry_method_has_eval_handler` -- no exceptions, no allowlist.

**Verification:**
- [ ] `grep -r "TYPECK_METHODS_NOT_IN_EVAL" compiler/ --include='*.rs'` returns 0 results

### 14.5.7 TYPECK_BUILTIN_METHODS (426 entries)

**What it tracked:** The exported constant in `ori_types` listing every `(type, method)` pair the type checker recognizes. Used by consistency tests for cross-checking.

**Entries:** 426 `(type, method)` pairs in `ori_types/src/infer/expr/methods/mod.rs`

**Why no longer needed:** The type checker reads directly from `ori_registry`. It does not maintain its own method list. Enforcement tests iterate the registry, not `TYPECK_BUILTIN_METHODS`.

**Replacement:** `ori_registry::BUILTIN_TYPES` enumeration replaces `TYPECK_BUILTIN_METHODS` enumeration.

**Verification:**
- [ ] `grep -r "TYPECK_BUILTIN_METHODS" compiler/ --include='*.rs'` returns 0 results
- [ ] `grep -r "pub const TYPECK_BUILTIN_METHODS" compiler/ --include='*.rs'` returns 0 results

### 14.5.8 EVAL_BUILTIN_METHODS (~165 entries)

**What it tracked:** The exported constant in `ori_eval` listing every `(type, method)` pair the evaluator's direct dispatch handles.

**Entries:** ~165 `(type, method)` pairs in `ori_eval/src/methods/helpers/mod.rs`

**Why no longer needed:** The evaluator reads method lists from the registry. Direct dispatch vs resolver dispatch is an internal implementation detail, not exposed.

**Replacement:** `ori_registry::BUILTIN_TYPES` enumeration + `can_dispatch_builtin()` function.

**Verification:**
- [ ] `grep -r "EVAL_BUILTIN_METHODS" compiler/ --include='*.rs'` returns 0 results
- [ ] `grep -r "pub const EVAL_BUILTIN_METHODS" compiler/ --include='*.rs'` returns 0 results

### 14.5.9 ITERATOR_METHOD_NAMES (~35 entries)

**What it tracked:** The exported constant in `ori_eval` listing method names for Iterator/DoubleEndedIterator types, used by the CollectionMethodResolver.

**Entries:** ~35 method names in `ori_eval/src/interpreter/resolvers/mod.rs`

**Why no longer needed:** The resolver reads iterator method names from `ori_registry::find_type(TypeTag::Iterator).methods` and `ori_registry::find_type(TypeTag::DoubleEndedIterator).methods`.

**Replacement:** `ori_registry::BUILTIN_TYPES` enumeration for Iterator/DEI types.

**Verification:**
- [ ] `grep -r "ITERATOR_METHOD_NAMES" compiler/ --include='*.rs'` returns 0 results
- [ ] `grep -r "pub const ITERATOR_METHOD_NAMES" compiler/ --include='*.rs'` returns 0 results

### 14.5.10 DEI_ONLY_METHODS (5 entries)

**What it tracked:** Method names that require DoubleEndedIterator but not plain Iterator (`next_back`, `rev`, `last`, `rfind`, `rfold`).

**Entries:** 5 method names in `ori_types/src/infer/expr/methods/mod.rs`

**Why no longer needed:** Derivable from the registry: methods on `TypeTag::DoubleEndedIterator` that are not on `TypeTag::Iterator`.

**Replacement:** `ori_registry::find_type(TypeTag::DoubleEndedIterator).methods` minus `ori_registry::find_type(TypeTag::Iterator).methods`.

**Verification:**
- [ ] `grep -r "DEI_ONLY_METHODS" compiler/ --include='*.rs'` returns 0 results

### Master Checklist

- [ ] Delete `COLLECTION_TYPES` (11 entries)
- [ ] Delete `IR_METHODS_DISPATCHED_VIA_RESOLVERS` (14 entries)
- [ ] Delete `EVAL_METHODS_NOT_IN_IR` (80 entries)
- [ ] Delete `EVAL_METHODS_NOT_IN_TYPECK` (63 entries)
- [ ] Delete `TYPECK_METHODS_NOT_IN_IR` (143 entries)
- [ ] Delete `TYPECK_METHODS_NOT_IN_EVAL` (260 entries)
- [ ] Delete `TYPECK_BUILTIN_METHODS` (426 entries) from `ori_types`
- [ ] Delete `EVAL_BUILTIN_METHODS` (~165 entries) from `ori_eval`
- [ ] Delete `ITERATOR_METHOD_NAMES` (~35 entries) from `ori_eval`
- [ ] Delete `DEI_ONLY_METHODS` (5 entries) from `ori_types`
- [ ] All 10 grep verifications pass (0 results each)
- [ ] Total lines eliminated: ~1,200+ across allowlists and exported constants

---

## 14.6 Legacy Code Removal & Grep Verification

### 14.6.1 Files to Delete

| File | Lines | Reason |
|------|-------|--------|
| None (file is replaced, not deleted) | | `consistency.rs` is rewritten with enforcement tests, not deleted |

The old `consistency.rs` (~1,010 lines) is not deleted as a file -- it is completely rewritten. The new version contains the enforcement tests from Section 14.2 and migrated tests from 14.2.6-7, with zero allowlists.

### 14.6.2 Functions to Delete

These `resolve_*_method()` functions in `ori_types` are replaced by registry lookups:

| Function | File | Lines | Replacement |
|----------|------|-------|-------------|
| `resolve_int_method()` | `ori_types/src/infer/expr/methods/mod.rs` | ~15 | `find_method(Int, name).returns` |
| `resolve_float_method()` | same | ~15 | `find_method(Float, name).returns` |
| `resolve_bool_method()` | same | ~10 | `find_method(Bool, name).returns` |
| `resolve_byte_method()` | same | ~15 | `find_method(Byte, name).returns` |
| `resolve_char_method()` | same | ~12 | `find_method(Char, name).returns` |
| `resolve_str_method()` | same | ~25 | `find_method(Str, name).returns` |
| `resolve_duration_method()` | same | ~20 | `find_method(Duration, name).returns` |
| `resolve_size_method()` | same | ~20 | `find_method(Size, name).returns` |
| `resolve_ordering_method()` | same | ~8 | `find_method(Ordering, name).returns` |
| `resolve_error_method()` | same | ~10 | `find_method(Error, name).returns` |
| `resolve_list_method()` | same | ~25 | `find_method(List, name).returns` |
| `resolve_map_method()` | same | ~15 | `find_method(Map, name).returns` |
| `resolve_set_method()` | same | ~12 | `find_method(Set, name).returns` |
| `resolve_range_method()` | same | ~10 | `find_method(Range, name).returns` |
| `resolve_option_method()` | same | ~12 | `find_method(Option, name).returns` |
| `resolve_result_method()` | same | ~15 | `find_method(Result, name).returns` |
| `resolve_iterator_method()` | same | ~20 | `find_method(Iterator, name).returns` |
| `resolve_dei_method()` | same | ~10 | `find_method(DoubleEndedIterator, name).returns` |

**Estimated deletion:** ~280 lines of match-arm-heavy resolve functions.

### 14.6.3 Grep Verification Checklist

Every grep below must return 0 results. These verify that all legacy code has been removed.

**Allowlist constants:**
- [ ] `grep -r "TYPECK_BUILTIN_METHODS" compiler/ --include='*.rs'` -- 0 results
- [ ] `grep -r "EVAL_BUILTIN_METHODS" compiler/ --include='*.rs'` -- 0 results
- [ ] `grep -r "ITERATOR_METHOD_NAMES" compiler/ --include='*.rs'` -- 0 results
- [ ] `grep -r "DEI_ONLY_METHODS" compiler/ --include='*.rs'` -- 0 results
- [ ] `grep -r "TYPECK_METHODS_NOT_IN" compiler/ --include='*.rs'` -- 0 results
- [ ] `grep -r "EVAL_METHODS_NOT_IN" compiler/ --include='*.rs'` -- 0 results
- [ ] `grep -r "IR_METHODS_DISPATCHED_VIA_RESOLVERS" compiler/ --include='*.rs'` -- 0 results
- [ ] `grep -r "COLLECTION_TYPES" compiler/oric/ --include='*.rs'` -- 0 results (the allowlist; general "collection types" usage in other contexts is fine)

**Legacy resolve functions:**
- [ ] `grep -r "resolve_str_method\|resolve_int_method\|resolve_float_method" compiler/ --include='*.rs'` -- 0 results
- [ ] `grep -r "resolve_bool_method\|resolve_byte_method\|resolve_char_method" compiler/ --include='*.rs'` -- 0 results
- [ ] `grep -r "resolve_duration_method\|resolve_size_method\|resolve_ordering_method" compiler/ --include='*.rs'` -- 0 results
- [ ] `grep -r "resolve_error_method\|resolve_list_method\|resolve_map_method" compiler/ --include='*.rs'` -- 0 results
- [ ] `grep -r "resolve_set_method\|resolve_range_method\|resolve_option_method" compiler/ --include='*.rs'` -- 0 results
- [ ] `grep -r "resolve_result_method\|resolve_iterator_method\|resolve_dei_method" compiler/ --include='*.rs'` -- 0 results

**Legacy borrow/ownership infrastructure:**
- [ ] `grep -r "receiver_borrowed" compiler/ --include='*.rs'` -- 0 results (replaced by `Ownership::Borrow` in registry)
- [ ] `grep -r "borrowing_builtin_names" compiler/ori_llvm/ --include='*.rs'` -- 0 results (ori_arc reads from registry directly)
- [ ] `grep -r "receiver_borrows" compiler/ori_ir/ --include='*.rs'` -- 0 results (ori_ir delegates to registry)

**Legacy type guards in LLVM:**
- [ ] `grep -rn "is_str.*emit_binary\|emit_binary.*is_str" compiler/ori_llvm/ --include='*.rs'` -- 0 results (replaced by OpStrategy dispatch)
- [ ] `grep -rn "is_float.*emit_binary\|emit_binary.*is_float" compiler/ori_llvm/ --include='*.rs'` -- 0 results (replaced by OpStrategy dispatch)

**Legacy ori_ir BUILTIN_METHODS:**
- [ ] `grep -r "BUILTIN_METHODS" compiler/ori_ir/ --include='*.rs'` -- 0 results (migrated to ori_registry or removed; ori_ir may re-export from registry)

### 14.6.4 Lines of Code Impact

| Component | Lines Deleted | Lines Added | Net |
|-----------|--------------|-------------|-----|
| `consistency.rs` (rewrite) | ~1,010 | ~300 (enforcement tests) | -710 |
| `TYPECK_BUILTIN_METHODS` + resolve functions | ~700 | 0 | -700 |
| `EVAL_BUILTIN_METHODS` + helpers | ~200 | 0 | -200 |
| `ITERATOR_METHOD_NAMES` | ~35 | 0 | -35 |
| `ori_ir BUILTIN_METHODS` | ~162 | 0 | -162 |
| `ori_llvm receiver_borrowed` | ~179 | 0 | -179 |
| `ori_llvm borrowing_builtin_names` | ~25 | 0 | -25 |
| `ori_llvm type guards (is_str, is_float)` | ~20 | 0 | -20 |
| `ori_arc borrowing_builtins parameter` | ~20 | 0 | -20 |
| **Total** | **~2,351** | **~300** | **-2,051** |

Note: This is the combined impact of Sections 09-14. Section 14 itself adds ~300 lines of enforcement tests while all the deletions happen across Sections 09-13. The table documents the full plan impact.

---

## 14.7 Full Test Suite Execution

### Escalating Test Runs

Each step must pass before proceeding to the next. Failures at any level must be investigated and resolved before continuing.

**Level 1: Compilation**
- [ ] `cargo c` -- all workspace crates compile cleanly
- [ ] `cargo c -p ori_registry` -- registry crate compiles
- [ ] `cargo bl` -- LLVM build compiles (includes ori_registry)

**Level 2: Unit Tests (per-crate)**
- [ ] `cargo t -p ori_registry` -- registry integrity + purity tests pass
- [ ] `cargo t -p ori_types` -- type checker tests pass (no regressions from wiring)
- [ ] `cargo t -p ori_eval` -- evaluator tests pass (no regressions from wiring)
- [ ] `cargo t -p ori_ir` -- IR tests pass (reduced after migration)
- [ ] `cargo t -p ori_arc` -- ARC tests pass (new dependency direction)

**Level 3: Integration Tests**
- [ ] `cargo t -p oric` -- integration + enforcement tests pass (this is where the new cross-phase enforcement tests live)
- [ ] `./llvm-test.sh` -- LLVM unit tests pass (operator strategy dispatch verified)

**Level 4: Spec Tests**
- [ ] `cargo st` -- all spec tests pass (end-to-end language behavior unchanged)
- [ ] `cargo st tests/spec/types/` -- type-specific spec tests pass
- [ ] `cargo st tests/spec/traits/` -- trait spec tests pass (includes iterator, derive)
- [ ] `cargo st tests/spec/methods/` -- method spec tests pass

**Level 5: Full Suite**
- [ ] `./test-all.sh` -- everything passes
- [ ] `./clippy-all.sh` -- no warnings
- [ ] `./fmt-all.sh` -- formatting clean

**Level 6: Release Verification**
- [ ] `cargo blr` -- release build compiles
- [ ] `./test-all.sh` with release binary -- all tests pass under release optimization

### Checklist

- [ ] All 6 levels pass in order
- [ ] No test was skipped, disabled, or marked `#[ignore]`
- [ ] No `#[allow(clippy)]` added without justification
- [ ] No test was modified to pass (tests that fail indicate code bugs, not test bugs)

---

## 14.8 Code Journey (Pipeline Integration)

Run `/code-journey` to test the pipeline end-to-end with progressively
complex Ori programs. This catches issues that unit tests and spec tests
miss: silent wrong code generation, phase boundary mismatches, cascading
failures across compiler stages, and eval-vs-LLVM behavioral divergence.

- [ ] Run `/code-journey` — journeys escalate until the compiler breaks down
- [ ] All CRITICAL findings from journey results triaged (fixed or tracked)
- [ ] Eval and AOT paths produce identical results for all passing journeys
- [ ] Journey results archived in `plans/code-journeys/`

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

- [ ] **Adding a field to `TypeDef`** produces a compile error in every consuming phase (ori_types, ori_eval, ori_arc, ori_llvm) because each phase destructures or reads `TypeDef` fields.
- [ ] **Adding a `TypeTag` variant** produces a compile error in every consuming phase via `_enforce_exhaustiveness()` dead functions (Roc pattern). Caught at `cargo c` time, before any tests run.
- [ ] **Adding a method to a `TypeDef`** is caught by enforcement tests (not compile errors -- method lists are slices). The `every_registry_method_has_*_handler` tests fail for the new method until all phases implement it.
- [ ] **`MethodDef` fields are required** (no defaults, no `Option<T>` for essential fields). Omitting a field when constructing a `MethodDef` is a compile error — including the new `pure` and `backend_required` flags.
- [ ] **`ori_registry` has zero dependencies.** The `purity_cargo_toml_has_no_dependencies` test enforces this. Adding any dependency is a test failure.
- [ ] **All `TypeDef` constants are `const`-constructible.** The `purity_type_defs_are_const` test enforces this with `const _:` declarations.
- [ ] **Core enum types are `Copy`.** The `purity_core_enums_are_copy` test enforces this. Losing `Copy` is a compile error in consuming phases.

### Behavioral Guarantees (test-time)

These guarantees are enforced by the cross-phase enforcement tests. They hold as long as `cargo t -p oric` passes.

- [ ] **Every registry method has a type checker handler.** `every_registry_method_has_typeck_handler` iterates all `BUILTIN_TYPES` methods and verifies ori_types resolves each one.
- [ ] **Every registry method has an evaluator handler.** `every_registry_method_has_eval_handler` iterates all `BUILTIN_TYPES` methods and verifies ori_eval dispatches each one.
- [ ] **Every registry method has an LLVM handler.** `every_registry_method_has_llvm_handler` iterates all `BUILTIN_TYPES` methods and verifies ori_llvm handles each one (inline codegen or runtime function).
- [ ] **Every non-Unsupported operator strategy has an LLVM handler.** `every_registry_operator_has_llvm_handler` iterates all `BUILTIN_TYPES` operator strategies and verifies emit_binary_op/emit_unary_op handles each non-Unsupported entry.
- [ ] **Every borrowing method is in the ARC borrow set.** `every_registry_borrowing_method_in_arc_set` verifies that `Ownership::Borrow` methods appear in ori_arc's borrow inference set.
- [ ] **Every backend-required method is in all backends.** `backend_required_methods_fully_implemented` iterates all `BUILTIN_TYPES` methods with `backend_required: true` and verifies both eval and llvm handle them.
- [ ] **Pure method annotations are consistent.** `pure_method_sanity` verifies that `pure: true` methods don't consume their receiver and that a reasonable percentage of methods are marked pure.
- [ ] **No duplicate methods within any type.** `no_duplicate_methods` catches copy-paste errors and merge conflicts.
- [ ] **All TypeTag variants have TypeDefs.** `all_type_tags_present` catches new TypeTag variants without corresponding definitions.
- [ ] **Methods are sorted.** `methods_sorted_by_name` maintains deterministic iteration order.
- [ ] **Operators are consistent.** `operator_consistency` catches comparison-without-equality bugs.
- [ ] **Format spec variants are synced.** `format_spec_variants_synced` prevents drift between ori_ir enums and phase registrations.
- [ ] **Well-known generics are consistent.** `well_known_generic_types_consistent` prevents Pool tag unification failures.

### Legacy Removal (grep-time)

These guarantees are verified by running the grep commands from Section 14.6.3. All must return 0 results.

- [ ] Zero matches for `TYPECK_BUILTIN_METHODS` in `compiler/`
- [ ] Zero matches for `EVAL_BUILTIN_METHODS` in `compiler/`
- [ ] Zero matches for `ITERATOR_METHOD_NAMES` in `compiler/`
- [ ] Zero matches for `DEI_ONLY_METHODS` in `compiler/`
- [ ] Zero matches for `TYPECK_METHODS_NOT_IN` in `compiler/`
- [ ] Zero matches for `EVAL_METHODS_NOT_IN` in `compiler/`
- [ ] Zero matches for `IR_METHODS_DISPATCHED_VIA_RESOLVERS` in `compiler/`
- [ ] Zero matches for `COLLECTION_TYPES` in `compiler/oric/`
- [ ] Zero matches for `resolve_str_method` and all 17 sibling resolve functions in `compiler/`
- [ ] Zero matches for `receiver_borrowed` in `compiler/`
- [ ] Zero matches for `borrowing_builtin_names` in `compiler/ori_llvm/`
- [ ] Zero matches for `receiver_borrows` in `compiler/ori_ir/`
- [ ] Zero matches for legacy `BUILTIN_METHODS` in `compiler/ori_ir/`

### Correctness (runtime)

These guarantees are verified by running the full test suite.

- [ ] `./test-all.sh` passes with zero failures
- [ ] `./llvm-test.sh` passes with zero failures
- [ ] `cargo st` passes with zero failures
- [ ] `./clippy-all.sh` passes with zero warnings
- [ ] `./fmt-all.sh` passes (no formatting changes needed)
- [ ] `cargo blr && ./test-all.sh` passes (release build regression check)
- [ ] No existing test was deleted, modified, or marked `#[ignore]` to achieve a passing suite
- [ ] No `#[allow(clippy)]` was added without a `reason = "..."` justification
- [ ] Code journey passes — eval/AOT match, no CRITICAL findings unaddressed

### Documentation

These guarantees verify that the plan's output is documented and discoverable.

- [ ] `ori_registry/src/lib.rs` has a crate-level `//!` doc comment explaining the mission, purity contract, and usage pattern
- [ ] Every `pub` item in `ori_registry` has a `///` doc comment
- [ ] `.claude/rules/` updated with registry patterns (how to add a new type, how to add a new method, which tests to run)
- [ ] `plans/builtin_ownership_ssot/` marked as SUPERSEDED by type_strategy_registry
- [ ] `plans/roadmap/` sections updated to reference ori_registry where they previously referenced ori_ir BUILTIN_METHODS
- [ ] This section (14) documents the complete elimination checklist
- [ ] The index.md in `plans/type_strategy_registry/` is updated with final status

### Plan Completion Summary

When all exit criteria above are satisfied:

1. `ori_registry` is the single source of truth for all builtin type behavioral specifications
2. Every compiler phase (ori_types, ori_eval, ori_arc, ori_llvm) reads from ori_registry
3. Cross-phase drift is structurally impossible (compile-time) or immediately detected (test-time)
4. Zero allowlists remain
5. Zero legacy parallel lists remain
6. ~2,000 lines of manual sync infrastructure have been eliminated
7. Adding a new builtin method requires exactly one change: a `MethodDef` entry in `ori_registry`. All enforcement tests then guide the implementer to add handlers in each phase.

The Type Strategy Registry plan is complete.
