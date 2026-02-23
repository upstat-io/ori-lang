---
section: "05"
title: "Enforcement Tests"
status: not-started
goal: "Make it structurally impossible to add a builtin without ownership metadata"
files:
  - compiler/ori_llvm/src/codegen/arc_emitter/builtins/tests.rs
  - compiler/ori_ir/src/builtin_methods/tests.rs
  - compiler/oric/src/eval/tests/methods/consistency.rs
---

# Section 05: Enforcement Tests

**Status:** Not Started
**Goal:** Make it structurally impossible to add a codegen builtin without an `ori_ir` MethodDef. The `MethodDef` struct already enforces `receiver_borrows` at compile time (it's a required field with no default). The enforcement test closes the second gap: codegen handlers that lack a MethodDef entirely.

---

## 05.0 Enforcement Chain

The structural guarantee works in two layers:

1. **Compile-time (Rust type system):** Every `MethodDef::new()` call requires `receiver_borrows: bool`. You cannot construct a `MethodDef` without specifying ownership. No `Default` impl, no builder with optional fields.

2. **Test-time (enforcement test):** Every `BuiltinRegistration` in `ori_llvm` must have a corresponding `MethodDef` in `ori_ir`. If someone adds a codegen handler without a MethodDef, the test fails.

Together: codegen handler → must have MethodDef → MethodDef has `receiver_borrows` → ownership is always declared.

---

## 05.1 New Test: `every_codegen_builtin_has_ir_method_def`

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/tests.rs`

This is the critical enforcement test. It bridges the gap between codegen registration and IR ownership metadata.

```rust
use ori_ir::builtin_methods::{find_method, BuiltinType};

/// Every codegen builtin handler must have a corresponding MethodDef in
/// ori_ir::builtin_methods. This ensures ownership metadata (receiver_borrows)
/// exists for every method that codegen can emit inline IR for.
///
/// If this test fails, you added a codegen handler in declare_builtins! without
/// adding a MethodDef entry in ori_ir. Add the MethodDef first — it enforces
/// explicit ownership declaration at compile time.
#[test]
fn every_codegen_builtin_has_ir_method_def() {
    let table = builtin_table();
    let mut missing = Vec::new();

    for (type_name, method_name) in table.all_registered() {
        // Map codegen type_name to BuiltinType
        let builtin_type = match BuiltinType::from_name(type_name) {
            Some(bt) => bt,
            None => {
                // Types without BuiltinType mapping (e.g., user types that get
                // generic clone) are not part of the IR registry
                continue;
            }
        };

        // Check for MethodDef in IR registry
        if find_method(builtin_type, method_name).is_none() {
            // Check codegen aliases (e.g., "length" → "len")
            let canonical = match method_name {
                "length" => "len",
                "is_equal" => "equals",
                _ => method_name,
            };
            if canonical != method_name
                && find_method(builtin_type, canonical).is_some()
            {
                continue;
            }
            missing.push(format!("  {}.{}", type_name, method_name));
        }
    }

    assert!(
        missing.is_empty(),
        "Codegen builtins without MethodDef in ori_ir \
         (ownership not declared):\n{}\n\n\
         Fix: Add MethodDef entries in compiler/ori_ir/src/builtin_methods/ \
         with explicit receiver_borrows value.",
        missing.join("\n"),
    );
}
```

### BuiltinType::from_name() Requirement

This test requires `BuiltinType::from_name(&str) -> Option<BuiltinType>`. Verify this exists or add it:

```rust
impl BuiltinType {
    /// Map a codegen type name to BuiltinType.
    /// Names follow TYPECK convention: lowercase for primitives, PascalCase for named types.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "int" => Some(Self::Int),
            "float" => Some(Self::Float),
            "bool" => Some(Self::Bool),
            "str" => Some(Self::Str),
            "char" => Some(Self::Char),
            "byte" => Some(Self::Byte),
            "Duration" => Some(Self::Duration),
            "Size" => Some(Self::Size),
            "Ordering" => Some(Self::Ordering),
            "list" => Some(Self::List),
            "map" => Some(Self::Map),
            "Set" => Some(Self::Set),
            "Option" => Some(Self::Option),
            "Result" => Some(Self::Result),
            "range" => Some(Self::Range),
            "Iterator" => Some(Self::Iterator),
            "DoubleEndedIterator" => Some(Self::DoubleEndedIterator),
            "tuple" => Some(Self::Tuple),
            "error" => Some(Self::Error),
            "Channel" => Some(Self::Channel),
            _ => None,
        }
    }
}
```

---

## 05.2 Update `no_phantom_builtin_entries`

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/tests.rs`

No structural changes needed — this test checks that every codegen entry has a TYPECK backing. It doesn't reference `receiver_borrowed`. However, consider adding an IR registry check as a secondary validation:

```rust
// After the existing phantom check, add:
// Also verify the entry exists in IR (redundant with
// every_codegen_builtin_has_ir_method_def, but provides context)
```

**Decision:** Leave `no_phantom_builtin_entries` unchanged. The new `every_codegen_builtin_has_ir_method_def` test covers the IR side.

---

## 05.3 Update `builtin_coverage_above_threshold`

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/tests.rs`

After Section 02 expands the IR registry, this test can optionally be updated to measure codegen coverage against the IR registry (in addition to TYPECK). But this is optional — the existing TYPECK-based coverage is still valid.

**Action:** No changes required. The threshold percentage may need adjustment if the TYPECK entry count changes, but the test logic is sound.

---

## 05.4 New IR Registry Tests

**File:** `compiler/ori_ir/src/builtin_methods/tests.rs`

```rust
#[test]
fn all_current_methods_borrow_receiver() {
    // Currently all builtin methods borrow. When a consuming method is added,
    // update this test to be more specific.
    for method in all_methods() {
        assert!(
            method.receiver_borrows,
            "{:?}.{} should borrow its receiver",
            method.receiver, method.name
        );
    }
}

#[test]
fn borrowing_method_names_nonempty() {
    let names: Vec<_> = borrowing_method_names().collect();
    assert!(!names.is_empty(), "should have borrowing methods");
    // Spot-check representative methods
    assert!(names.contains(&"compare"), "compare should borrow");
    assert!(names.contains(&"len"), "len should borrow");
    assert!(names.contains(&"clone"), "clone should borrow");
}

#[test]
fn method_borrows_receiver_query() {
    assert_eq!(
        method_borrows_receiver(BuiltinType::Str, "len"),
        Some(true),
        "str.len should borrow"
    );
    assert_eq!(
        method_borrows_receiver(BuiltinType::Int, "nonexistent"),
        None,
        "nonexistent method should return None"
    );
}

#[test]
fn every_type_has_at_least_one_method() {
    use std::collections::HashSet;
    let types: HashSet<_> = all_methods().map(|m| m.receiver).collect();
    // All types with builtin methods should be represented
    assert!(types.contains(&BuiltinType::Int));
    assert!(types.contains(&BuiltinType::Str));
    assert!(types.contains(&BuiltinType::List));
    // ... etc for all 20 types
}

#[test]
fn no_duplicate_method_entries() {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    let mut duplicates = Vec::new();
    for method in all_methods() {
        let key = (method.receiver, method.name);
        if !seen.insert(key) {
            duplicates.push(format!("{:?}.{}", method.receiver, method.name));
        }
    }
    assert!(
        duplicates.is_empty(),
        "Duplicate MethodDef entries:\n{}",
        duplicates.join("\n")
    );
}
```

---

## 05.5 TypeFlow Enforcement Test

**File:** `compiler/ori_ir/src/builtin_methods/tests.rs`

```rust
#[test]
fn higher_order_methods_have_type_flow() {
    // Every method with a Closure param that transforms the output type
    // should have non-Standard TypeFlow. Methods like filter, any, all
    // take closures but don't transform the output type — those are
    // correctly Standard.
    for method in all_methods() {
        if matches!(method.name, "map" | "flat_map" | "fold" | "rfold") {
            assert_ne!(
                method.type_flow,
                TypeFlow::Standard,
                "{:?}.{} takes a closure and needs TypeFlow",
                method.receiver, method.name
            );
        }
    }
}
```

This test ensures that known higher-order type-transforming methods always carry the correct `TypeFlow`. Adding a new `map`-like method without specifying `TypeFlow` will fail this test.

---

## 05.6 Consistency Test Updates

**File:** `compiler/oric/src/eval/tests/methods/consistency.rs`

### New Test: `ir_registry_covers_all_typeck_types`

```rust
#[test]
fn ir_registry_covers_all_typeck_types() {
    use std::collections::HashSet;
    use ori_ir::builtin_methods::all_methods;

    let ir_types: HashSet<&str> = all_methods()
        .map(|m| m.receiver.name())
        .collect();

    let typeck_types: HashSet<&str> = TYPECK_BUILTIN_METHODS.iter()
        .map(|(type_name, _)| *type_name)
        .collect();

    let mut missing = Vec::new();
    for ty in &typeck_types {
        if !ir_types.contains(ty) {
            missing.push(*ty);
        }
    }

    assert!(
        missing.is_empty(),
        "TYPECK has methods for types missing from IR registry:\n  {}",
        missing.join("\n  ")
    );
}
```

### Remove `COLLECTION_TYPES` Gap List

After Section 02, all 11 previously-missing types are in the IR registry. The `COLLECTION_TYPES` array and any test that references it should be removed or updated.

### Reduce `TYPECK_METHODS_NOT_IN_IR`

After Section 02, many entries in this list are now covered. The remaining entries should be:
- Factory methods (e.g., `Duration.from_hours`) — these are associated functions, not methods
- Conversion aliases (e.g., `Duration.as_micros`) — some TYPECK entries that weren't added to IR
- Any method genuinely only in TYPECK (deferred to later)

Update the list to reflect only the genuine remaining gaps.

### Reduce `EVAL_METHODS_NOT_IN_IR`

Same treatment — remove entries now covered by new IR entries.

---

## 05.6 Verification

- [ ] `cargo t -p ori_ir` — new IR tests pass
- [ ] `./llvm-test.sh` — enforcement test passes (no missing MethodDefs)
- [ ] `cargo t -p oric` — consistency tests pass
- [ ] `COLLECTION_TYPES` list is empty or deleted
- [ ] `TYPECK_METHODS_NOT_IN_IR` list is reduced
- [ ] `EVAL_METHODS_NOT_IN_IR` list is reduced
