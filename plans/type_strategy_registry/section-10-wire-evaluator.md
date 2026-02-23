---
section: "10"
title: "Wire Evaluator (ori_eval)"
status: not-started
goal: "Replace all hardcoded method enumerations in ori_eval with registry-derived data, while preserving existing dispatch performance (O(1) Name comparison) and the resolver chain architecture"
depends_on:
  - "03"
  - "04"
  - "05"
  - "06"
  - "07"
  - "08"
sections:
  - id: "10.1"
    title: "Replace EVAL_BUILTIN_METHODS"
    status: not-started
  - id: "10.2"
    title: "Replace ITERATOR_METHOD_NAMES"
    status: not-started
  - id: "10.3"
    title: "BuiltinMethodNames struct — keep or replace"
    status: not-started
  - id: "10.4"
    title: "Method dispatch validation"
    status: not-started
  - id: "10.5"
    title: "Format variant sync"
    status: not-started
  - id: "10.6"
    title: "Evaluator enforcement test"
    status: not-started
  - id: "10.7"
    title: "Validation & regression"
    status: not-started
---

# Section 10: Wire Evaluator (ori_eval)

**Status:** Not Started
**Goal:** Replace all independently-maintained method enumerations in `ori_eval` with data derived from the `ori_registry` crate, while preserving the existing dispatch architecture (resolver chain, pre-interned `Name` comparison, `FxHashSet` lookup) and its O(1) runtime performance.

**Context:** The evaluator is the simplest wiring target. Unlike `ori_types` (Section 09), which must read return types and parameter specs for inference, `ori_eval` only needs the *existence* of methods per type for two purposes: (1) building the `BuiltinMethodResolver`'s `FxHashSet<(Name, Name)>` for O(1) resolution, and (2) cross-crate consistency testing. The evaluator's dispatch mechanism (`dispatch_builtin_method`, `CollectionMethodResolver`, etc.) does not change — it continues to pattern-match on `Value` variants and pre-interned `Name` fields. Only the *source of truth* for what methods exist shifts from hardcoded arrays to registry queries.

**Principle:** The evaluator is a *consumer* of the registry, not a *mirror* of it. The registry declares what methods a type has. The evaluator implements the runtime behavior. The registry-derived data validates that these two sets are aligned, but the evaluator's internal dispatch architecture remains intact.

---

## 10.1 Replace EVAL_BUILTIN_METHODS

### Current State

**File:** `compiler/ori_eval/src/methods/helpers/mod.rs` (lines 14-229)

`EVAL_BUILTIN_METHODS` is a hand-maintained `&[(&str, &str)]` array of 165 `(type_name, method_name)` pairs. It serves two purposes:

1. **`BuiltinMethodResolver` construction** (`compiler/ori_eval/src/interpreter/resolvers/builtin/mod.rs`, line 35): Each pair is interned into an `FxHashSet<(Name, Name)>` at interpreter startup for O(1) method existence checks during dispatch.

2. **Cross-crate consistency tests** (`compiler/oric/src/eval/tests/methods/consistency.rs`): The array is compared against `TYPECK_BUILTIN_METHODS` and `ori_ir::BUILTIN_METHODS` to detect drift between phases.

**Exported via:** `ori_eval/src/lib.rs` line 58 (`pub use methods::{dispatch_builtin_method_str, EVAL_BUILTIN_METHODS};`).

### After Migration

The array is eliminated. Both consumers switch to registry queries:

1. **`BuiltinMethodResolver::new()`** iterates the registry to build its `FxHashSet`:
   ```rust
   // BEFORE (methods/helpers/mod.rs + resolvers/builtin/mod.rs)
   pub const EVAL_BUILTIN_METHODS: &[(&str, &str)] = &[
       ("Duration", "add"),
       ("Duration", "clone"),
       // ... 165 entries
   ];

   // In BuiltinMethodResolver::new():
   let known_methods = crate::methods::EVAL_BUILTIN_METHODS
       .iter()
       .map(|(type_name, method_name)| {
           (interner.intern(type_name), interner.intern(method_name))
       })
       .collect();
   ```

   ```rust
   // AFTER (resolvers/builtin/mod.rs)
   use ori_registry::{BUILTIN_TYPES, TypeTag};

   // In BuiltinMethodResolver::new():
   let known_methods = BUILTIN_TYPES
       .iter()
       .flat_map(|type_def| {
           let type_name = interner.intern(type_def.tag.name());
           type_def.methods.iter().map(move |method| {
               (type_name, interner.intern(method.name))
           })
       })
       .collect();
   ```

2. **Consistency tests** no longer need `EVAL_BUILTIN_METHODS` — the registry IS the source of truth. The old `eval_methods_recognized_by_typeck` and `typeck_methods_implemented_in_eval` tests in `consistency.rs` are replaced by a single registry-level enforcement test (see 10.6).

### Migration Steps

- [ ] In `BuiltinMethodResolver::new()`, replace `crate::methods::EVAL_BUILTIN_METHODS.iter()` with `ori_registry::BUILTIN_TYPES.iter().flat_map(...)` to build `known_methods`
- [ ] Remove `pub const EVAL_BUILTIN_METHODS` from `methods/helpers/mod.rs`
- [ ] Remove `pub use methods::EVAL_BUILTIN_METHODS` from `lib.rs` line 58
- [ ] Update `pub use methods::{dispatch_builtin_method_str, EVAL_BUILTIN_METHODS}` to `pub use methods::dispatch_builtin_method_str`
- [ ] Verify `cargo check -p ori_eval` passes
- [ ] Temporarily `#[allow(unused_imports)]` in `consistency.rs` if the enforcement test rewrite (10.6) is not yet complete

### What Does NOT Change

- The `dispatch_builtin_method()` function in `methods/mod.rs` (lines 351-387) — it pattern-matches on `Value` variants, not on an array
- The `dispatch_*_method()` sub-dispatch functions (numeric, collections, variants, units, ordering, error) — they use pre-interned `Name` comparison internally
- The `BuiltinMethodResolver`'s `FxHashSet` lookup strategy — the set is still built at startup, still O(1) per lookup
- The resolver chain priority order (User=0, Collection=1, Builtin=2)

---

## 10.2 Replace ITERATOR_METHOD_NAMES

### Current State

**File:** `compiler/ori_eval/src/interpreter/resolvers/mod.rs` (lines 232-257)

`ITERATOR_METHOD_NAMES` is a hand-maintained `&[&str]` array of 24 sorted method names for Iterator + DoubleEndedIterator methods. It serves one purpose:

1. **Cross-crate consistency test** (`consistency.rs` lines 726-749): `iterator_typeck_methods_match_eval_resolver()` compares this array against `TYPECK_BUILTIN_METHODS` entries for `"Iterator"` and `"DoubleEndedIterator"` types.

Unlike `EVAL_BUILTIN_METHODS`, this array is NOT used in production dispatch. The `CollectionMethodResolver` resolves iterator methods via its own pre-interned `MethodNames` struct (lines 11-41 of `collection/mod.rs`), built independently during `CollectionMethodResolver::new()`.

**Exported via:** `ori_eval/src/lib.rs` line 68 (`pub use interpreter::resolvers::ITERATOR_METHOD_NAMES;`).

### After Migration

The array is eliminated. The consistency test switches to registry queries:

```rust
// BEFORE (resolvers/mod.rs)
pub const ITERATOR_METHOD_NAMES: &[&str] = &[
    "all", "any", "chain", "collect", "count", "cycle",
    // ... 24 entries
];

// AFTER — eliminated entirely
// The consistency test (10.6) uses:
//   ori_registry::find_type(TypeTag::Iterator).methods
//   ori_registry::find_type(TypeTag::DoubleEndedIterator).methods
```

### Migration Steps

- [ ] Remove `pub const ITERATOR_METHOD_NAMES` from `resolvers/mod.rs`
- [ ] Remove `pub use interpreter::resolvers::ITERATOR_METHOD_NAMES` from `lib.rs` line 68
- [ ] Verify `cargo check -p ori_eval` passes
- [ ] Temporarily `#[allow(unused_imports)]` in `consistency.rs` if the enforcement test rewrite (10.6) is not yet complete

### What Does NOT Change

- The `CollectionMethodResolver` and its `MethodNames` struct — it builds its own pre-interned names at construction time
- The `resolve_iterator_method()` dispatch chain — it uses `Name` comparison against its own `MethodNames`, not against this array
- The `CollectionMethod` enum and its `all_iterator_variants()` method
- The `is_iterator_method()` predicate

### Decision: Should CollectionMethodResolver read from registry?

**No.** The `CollectionMethodResolver`'s `MethodNames` struct pre-interns method names for O(1) `Name` comparison at dispatch time. This is an optimization detail. The resolver does not need to know *what* methods exist at compile time — it only needs interned names to compare against at runtime. The `MethodNames::new()` constructor is already minimal (one `interner.intern()` call per name). Replacing it with a registry iteration would add complexity for no performance gain.

The enforcement test (10.6) validates that every method the registry declares for Iterator/DoubleEndedIterator has a corresponding `CollectionMethod` variant and resolver dispatch path. This catches drift without coupling the resolver to the registry at runtime.

---

## 10.3 BuiltinMethodNames Struct

### Current State

**File:** `compiler/ori_eval/src/methods/mod.rs` (lines 40-144)

`BuiltinMethodNames` is a 68-field struct where each field holds a pre-interned `Name` for a builtin method. It is constructed once per `Interpreter` via `BuiltinMethodNames::new(interner)` and bundled into `DispatchCtx` for method dispatch. Every sub-dispatch function (`dispatch_int_method`, `dispatch_str_method`, etc.) compares `method: Name` against fields like `ctx.names.add`, `ctx.names.clone_`, etc. using `u32 == u32` comparison instead of string matching.

This struct is purely a performance optimization — it does not define what methods exist. It provides named handles for fast equality testing during dispatch.

### Decision Analysis

| Criterion | Option A: Keep + Assert | Option B: Replace with Registry Lookup |
|-----------|-------------------------|----------------------------------------|
| **Runtime cost** | O(1) `u32 == u32` per method call (current) | O(1) `u32 == u32` if interned, O(n) string compare if not |
| **Startup cost** | 68 `intern()` calls (current) | Same if pre-interning; or lazy intern on first use |
| **Code change** | Add assertion test only (~20 lines) | Rewrite all 6 `dispatch_*_method` functions |
| **Correctness risk** | Low — struct is validated, dispatch unchanged | Medium — refactoring 6 dispatch functions risks subtle bugs |
| **Registry coupling** | Loose — test-time only | Tight — runtime dependency |
| **Maintenance burden** | When registry adds a method name, must add field (but test catches the gap) | When registry adds a method, it's auto-available (but dispatch still needs a handler) |

### Recommendation: Option A — Keep the Optimization, Add Assertion Test

The `BuiltinMethodNames` struct is an optimization detail (interned names for O(1) dispatch), not a source of truth. The registry is the source of truth for what methods exist; `BuiltinMethodNames` is the mechanism for dispatching them quickly.

Replacing it would require rewriting every `dispatch_*_method` function to look up method names differently, with no user-visible benefit. The existing code is correct, fast, and well-tested.

**What to add:** A test that asserts every method name used by `BuiltinMethodNames` corresponds to a method declared in the registry. This catches the case where a field is added to the struct for a method name that doesn't exist in the registry (dead dispatch path) or where the registry declares a method that has no corresponding `BuiltinMethodNames` field (missing dispatch path).

```rust
// In ori_eval/src/methods/tests.rs (new test)
#[test]
fn builtin_method_names_match_registry() {
    use ori_registry::BUILTIN_TYPES;
    use std::collections::BTreeSet;

    // Collect all method names from the registry for types dispatched
    // by BuiltinMethodResolver (excludes Iterator, dispatched by CollectionMethodResolver)
    let registry_method_names: BTreeSet<&str> = BUILTIN_TYPES
        .iter()
        .flat_map(|td| td.methods.iter().map(|m| m.name))
        .collect();

    // Collect all method names from BuiltinMethodNames fields
    let interner = ori_ir::SharedInterner::default();
    let names = BuiltinMethodNames::new(&interner);

    // Verify each BuiltinMethodNames field name exists in registry
    // (This list mirrors the struct fields — adding a field without
    // adding it here causes a compile error via exhaustive destructure)
    let BuiltinMethodNames {
        add, sub, mul, div, rem, neg,
        compare, equals, clone_, to_str, debug, hash,
        contains, len, is_empty, not, unwrap, concat,
        floor_div, bit_and, bit_or, bit_xor, bit_not, shl, shr,
        to_uppercase, to_lowercase, trim, starts_with, ends_with, escape,
        first, last,
        contains_key, keys, values,
        unwrap_or, is_some, is_none, ok_or,
        is_ok, is_err,
        is_less, is_equal, is_greater, is_less_or_equal, is_greater_or_equal,
        reverse, then,
        duration, size,
        subtract, multiply, divide, remainder, negate,
        nanoseconds, microseconds, milliseconds, seconds, minutes, hours,
        bytes, kilobytes, megabytes, gigabytes, terabytes,
        iter, into,
        trace, trace_entries, has_trace, with_trace, message,
    } = names;

    // The destructure above ensures exhaustiveness.
    // The test below validates each name against the registry.
    let _ = (add, sub, mul, div, rem, neg, compare, equals, clone_,
             to_str, debug, hash, contains, len, is_empty, not, unwrap,
             concat, floor_div, bit_and, bit_or, bit_xor, bit_not, shl,
             shr, to_uppercase, to_lowercase, trim, starts_with, ends_with,
             escape, first, last, contains_key, keys, values, unwrap_or,
             is_some, is_none, ok_or, is_ok, is_err, is_less, is_equal,
             is_greater, is_less_or_equal, is_greater_or_equal, reverse,
             then, duration, size, subtract, multiply, divide, remainder,
             negate, nanoseconds, microseconds, milliseconds, seconds,
             minutes, hours, bytes, kilobytes, megabytes, gigabytes,
             terabytes, iter, into, trace, trace_entries, has_trace,
             with_trace, message);

    // Note: The struct also contains type names (duration, size) used for
    // associated function dispatch, not method dispatch. These are expected
    // to NOT appear in the method name set.
}
```

The key enforcement mechanism is the **exhaustive destructure**: if a field is added to `BuiltinMethodNames` without adding it to this test, the code will not compile. This is compile-time enforcement, not runtime checking.

### Migration Steps

- [ ] Add `builtin_method_names_match_registry` test to `methods/tests.rs` (or `methods/mod.rs` inline if a `tests.rs` sibling already exists)
- [ ] Verify the test passes: `cargo test -p ori_eval -- builtin_method_names`
- [ ] No changes to the `BuiltinMethodNames` struct itself
- [ ] No changes to `DispatchCtx` or any `dispatch_*_method` function

---

## 10.4 Method Dispatch Validation

### Purpose

Add a test that validates the evaluator can dispatch every method the registry declares for each type it implements. This is the evaluator-side analog of the type checker's "every registry method has a type signature" test (Section 09).

### Current State

Currently, dispatch coverage is implicitly tested by spec tests (`tests/spec/`) and unit tests in each `dispatch_*_method` function. There is no systematic test that iterates all registry-declared methods and verifies each one has a dispatch handler.

The `BuiltinMethodResolver` answers "does this method exist?" (via `FxHashSet` lookup). But a method existing in the resolver does not guarantee `dispatch_builtin_method()` has a handler for it — the resolver could return `MethodResolution::Builtin`, and then dispatch could fall through to the `_` arm and return `no_such_method`.

### After Migration

A new test iterates the registry, constructs minimal `Value` instances for each type, and asserts that calling each declared method does not produce a `no_such_method` error. This is a smoke test, not a correctness test — it verifies dispatch routing, not behavior.

```rust
// In compiler/oric/src/eval/tests/methods/dispatch_coverage.rs (new file)

/// Every method the registry declares for a builtin type must be dispatchable
/// by the evaluator without producing `no_such_method`.
///
/// This test does NOT verify correct behavior — only that the dispatch chain
/// routes to a handler. Argument errors (wrong count, wrong type) are acceptable;
/// "no such method" is not.
#[test]
fn every_registry_method_has_eval_dispatch_handler() {
    use ori_ir::SharedInterner;
    use ori_registry::BUILTIN_TYPES;

    let interner = SharedInterner::default();

    for type_def in BUILTIN_TYPES {
        let receiver = minimal_value_for(type_def.tag);
        let Some(receiver) = receiver else {
            // Skip types that have no Value representation (e.g., Channel)
            continue;
        };

        for method in type_def.methods {
            let result = ori_eval::dispatch_builtin_method_str(
                receiver.clone(),
                method.name,
                vec![], // no args — we expect argument errors, not "no such method"
                &interner,
            );

            match result {
                Ok(_) => {} // method dispatched and happened to succeed with 0 args
                Err(e) => {
                    let msg = e.to_string();
                    assert!(
                        !msg.contains("no such method"),
                        "Registry declares {}.{} but evaluator has no dispatch handler: {}",
                        type_def.tag.name(),
                        method.name,
                        msg,
                    );
                    // Argument count/type errors are expected and acceptable
                }
            }
        }
    }
}
```

The `minimal_value_for()` helper constructs the simplest possible `Value` for each `TypeTag`:

```rust
fn minimal_value_for(tag: TypeTag) -> Option<Value> {
    match tag {
        TypeTag::Int => Some(Value::int(0)),
        TypeTag::Float => Some(Value::Float(0.0)),
        TypeTag::Bool => Some(Value::Bool(false)),
        TypeTag::Str => Some(Value::string("")),
        TypeTag::Char => Some(Value::Char(' ')),
        TypeTag::Byte => Some(Value::Byte(0)),
        TypeTag::Duration => Some(Value::Duration(0)),
        TypeTag::Size => Some(Value::Size(0)),
        TypeTag::Ordering => Some(Value::ordering_equal()),
        TypeTag::Option => Some(Value::None),
        TypeTag::Result => Some(Value::Ok(Box::new(Value::Void))),
        TypeTag::List => Some(Value::List(Heap::new(vec![]))),
        TypeTag::Map => Some(Value::Map(Heap::new(Default::default()))),
        TypeTag::Set => Some(Value::Set(Heap::new(Default::default()))),
        TypeTag::Range => Some(Value::Range(RangeValue::new(0, 0))),
        TypeTag::Tuple => Some(Value::Tuple(Heap::new(vec![]))),
        // Types without a direct Value representation
        TypeTag::Iterator
        | TypeTag::DoubleEndedIterator
        | TypeTag::Channel
        | TypeTag::Error => None,
    }
}
```

**Note on Iterator/DoubleEndedIterator:** These are dispatched by `CollectionMethodResolver`, not `BuiltinMethodResolver`. They cannot be tested via `dispatch_builtin_method_str`. A separate test covers them (see 10.6).

**Note on Error:** The `Value::Error` variant requires constructing an `ErrorValue`, which needs a message and optional trace. If Error methods are in the registry, a minimal `Value::Error(ErrorValue::new("test"))` can be used. The `None` return in `minimal_value_for` is a placeholder — adjust once the registry's Error type definition is finalized (Section 05).

### Migration Steps

- [ ] Create `compiler/oric/src/eval/tests/methods/dispatch_coverage.rs`
- [ ] Add `mod dispatch_coverage;` to `compiler/oric/src/eval/tests/methods/mod.rs`
- [ ] Implement `every_registry_method_has_eval_dispatch_handler` test
- [ ] Implement `minimal_value_for()` helper
- [ ] Verify test passes: `cargo test -p oric -- every_registry_method_has_eval_dispatch`
- [ ] If any method fails with "no such method", investigate: is it a missing dispatch handler (eval bug) or a registry method that should be excluded (registry bug)?

### What Does NOT Change

- `CollectionMethodResolver` dispatch chain — it handles its own set of methods
- `UserRegistryResolver` — it handles user-defined and derived methods
- The resolver priority chain (User=0, Collection=1, Builtin=2)
- Any `dispatch_*_method` function implementation

---

## 10.5 Format Variant Sync

### Current State

**File:** `compiler/ori_eval/src/interpreter/mod.rs` (lines 519-541)

`register_format_variants()` hardcodes `FormatType`, `Alignment`, and `Sign` variant strings for the `Formattable` trait's `FormatSpec` struct. These appear in 4 independent locations:

1. `ori_ir/src/format_spec.rs` — enum definitions (source of truth)
2. `ori_types/src/check/registration/builtin_types.rs` — type registration
3. `ori_eval/src/interpreter/mod.rs` — `register_format_variants()` globals
4. `ori_rt/src/format/mod.rs` — runtime enum + parse

Current sync enforcement: source-scanning tests in `consistency.rs` (lines 776-933) that read the `.rs` files and grep for variant name strings. The tests in `ori_rt` guard `ori_rt <-> ori_ir` sync via variant count assertions.

### Decision: Should Format Variants Move to the Registry?

**No.** Format variants are TYPE VARIANTS (enum constructors), not method behavior. They represent the values of the `FormatType`, `Alignment`, and `Sign` enums, not methods on types. The registry's domain is "what methods does type X have and how do they behave?" Format variants are a different concern: "what are the possible values of enum type Y?"

Moving them into the registry would conflate two separate domains:
- **Type behavioral specification** (methods, operators, memory strategy) — registry domain
- **Enum variant enumeration** (FormatType::Binary, Alignment::Left, Sign::Plus) — `ori_ir` domain

### Recommendation: Leave in `ori_ir`, Keep Existing Sync Tests

The existing 4-way sync tests work correctly today. They enforce consistency by scanning source files — not elegant, but effective and already proven. These tests will be revisited in Section 14 (Enforcement & Exit) as part of the broader "can we make this cleaner?" pass.

If a future iteration of the registry adds support for enum variant declarations (not just method declarations), format variants could migrate at that point. But that is out of scope for this plan.

### Migration Steps

- [ ] No code changes to `register_format_variants()` or `interpreter/mod.rs`
- [ ] No code changes to `consistency.rs` format variant sync tests
- [ ] Document in Section 14 that format variant sync is a candidate for future registry extension
- [ ] Verify existing format variant tests still pass: `cargo test -p oric -- format_type_variants`

---

## 10.6 Evaluator Enforcement Test

### Purpose

Replace the 6 cross-crate consistency tests in `compiler/oric/src/eval/tests/methods/consistency.rs` that compare `EVAL_BUILTIN_METHODS` against `TYPECK_BUILTIN_METHODS` with a single registry-level enforcement test. The old tests become redundant because both phases now read from the same source of truth.

### Tests Being Replaced

| Old Test | Lines | What It Checks | Why Redundant |
|----------|-------|----------------|---------------|
| `ir_methods_implemented_in_eval` | 93-114 | Every IR method has an eval dispatch | Registry IS the IR method list |
| `eval_primitive_methods_in_ir` | 119-140 | Every eval method is in IR | Eval reads from registry = IR |
| `eval_method_list_is_sorted` | 145-154 | `EVAL_BUILTIN_METHODS` sorted | Array eliminated |
| `eval_methods_recognized_by_typeck` | 677-694 | Every eval method has a typeck entry | Both read from registry |
| `typeck_methods_implemented_in_eval` | 700-716 | Every typeck method has an eval handler | Both read from registry |
| `eval_iterator_method_names_sorted` | 753-762 | `ITERATOR_METHOD_NAMES` sorted | Array eliminated |

### Tests Being Kept (Modified)

| Test | Lines | What It Checks | Why Still Needed |
|------|-------|----------------|------------------|
| `iterator_typeck_methods_match_eval_resolver` | 726-749 | Iterator methods match between typeck and eval | Modified to compare registry Iterator/DEI methods against `CollectionMethod::all_iterator_variants()` |
| `typeck_method_list_is_sorted` | 637-646 | `TYPECK_BUILTIN_METHODS` sorted | Kept until Section 09 eliminates `TYPECK_BUILTIN_METHODS` |
| `typeck_primitive_methods_in_ir` | 650-671 | Typeck methods in IR | Kept until Section 09 completes |
| All format variant sync tests | 776-933 | FormatType/Alignment/Sign sync | Kept (see 10.5) |
| `well_known_generic_types_consistent` | 962-1009 | Well-known generic resolution | Kept — unrelated to method registry |

### Allowlists Being Eliminated

These `const` arrays in `consistency.rs` are eliminated because they tracked gaps between independently-maintained lists. When both phases read from the same registry, gaps are structurally impossible:

| Allowlist | Lines | Purpose | Why Eliminated |
|-----------|-------|---------|----------------|
| `COLLECTION_TYPES` | 13-25 | Types not yet in IR registry | Registry includes all types |
| `IR_METHODS_DISPATCHED_VIA_RESOLVERS` | 33-46 | IR methods dispatched via resolvers, not direct dispatch | Dispatch coverage test (10.4) validates this directly |
| `EVAL_METHODS_NOT_IN_IR` | 50-80 | Eval methods not in IR | Registry IS the IR |
| `EVAL_METHODS_NOT_IN_TYPECK` | 161-223 | Eval methods not recognized by typeck | Both read from registry |
| `TYPECK_METHODS_NOT_IN_IR` | 227-369 | Typeck methods not in IR | Both read from registry |
| `TYPECK_METHODS_NOT_IN_EVAL` | 374-633 | Typeck methods not in eval | Both read from registry |

**Note:** The `TYPECK_*` allowlists are eliminated in coordination with Section 09 (Wire Type Checker). If Section 10 completes before Section 09, the `TYPECK_*` allowlists remain until Section 09 is done. The `EVAL_*` and `IR_*` allowlists can be removed as soon as Section 10 is complete.

### New Enforcement Test

```rust
// In compiler/oric/src/eval/tests/methods/consistency.rs (rewritten)

/// Every method in the registry for types that the evaluator handles must have
/// a dispatch handler. This replaces the old EVAL_BUILTIN_METHODS <-> TYPECK
/// cross-comparison tests.
#[test]
fn registry_methods_dispatchable_by_eval() {
    use ori_registry::BUILTIN_TYPES;
    use std::collections::BTreeSet;

    // Methods dispatched by BuiltinMethodResolver — read from registry
    let builtin_methods: BTreeSet<(&str, &str)> = BUILTIN_TYPES
        .iter()
        .flat_map(|td| {
            td.methods.iter().map(move |m| (td.tag.name(), m.name))
        })
        .collect();

    // Methods dispatched by CollectionMethodResolver — read from registry
    // for Iterator and DoubleEndedIterator types
    let iter_types = [TypeTag::Iterator, TypeTag::DoubleEndedIterator];
    let iter_methods: BTreeSet<&str> = BUILTIN_TYPES
        .iter()
        .filter(|td| iter_types.contains(&td.tag))
        .flat_map(|td| td.methods.iter().map(|m| m.name))
        .collect();

    // Verify iterator methods match CollectionMethod variants
    let eval_iter_methods: BTreeSet<&str> = CollectionMethod::all_iterator_variants()
        .iter()
        .map(|&(name, _)| name)
        .collect();

    let in_registry_not_eval: Vec<_> = iter_methods.difference(&eval_iter_methods).collect();
    let in_eval_not_registry: Vec<_> = eval_iter_methods.difference(&iter_methods).collect();

    assert!(
        in_registry_not_eval.is_empty(),
        "Registry has iterator methods not in eval CollectionMethod: {in_registry_not_eval:?}"
    );
    assert!(
        in_eval_not_registry.is_empty(),
        "Eval CollectionMethod has iterator methods not in registry: {in_eval_not_registry:?}"
    );
}
```

### Migration Steps

- [ ] Rewrite `consistency.rs` to use registry queries instead of `EVAL_BUILTIN_METHODS` and `ITERATOR_METHOD_NAMES`
- [ ] Remove `ir_methods_implemented_in_eval` test
- [ ] Remove `eval_primitive_methods_in_ir` test
- [ ] Remove `eval_method_list_is_sorted` test
- [ ] Remove `eval_methods_recognized_by_typeck` test
- [ ] Remove `typeck_methods_implemented_in_eval` test
- [ ] Remove `eval_iterator_method_names_sorted` test
- [ ] Remove `COLLECTION_TYPES` allowlist
- [ ] Remove `IR_METHODS_DISPATCHED_VIA_RESOLVERS` allowlist
- [ ] Remove `EVAL_METHODS_NOT_IN_IR` allowlist
- [ ] Remove `EVAL_METHODS_NOT_IN_TYPECK` allowlist
- [ ] Keep `TYPECK_METHODS_NOT_IN_IR` and `TYPECK_METHODS_NOT_IN_EVAL` until Section 09 completes (they depend on `TYPECK_BUILTIN_METHODS` which is Section 09's domain)
- [ ] Add `registry_methods_dispatchable_by_eval` test
- [ ] Keep all format variant sync tests unchanged (see 10.5)
- [ ] Keep `well_known_generic_types_consistent` test unchanged
- [ ] Remove `use ori_eval::EVAL_BUILTIN_METHODS` import
- [ ] Remove `use ori_eval::ITERATOR_METHOD_NAMES` import
- [ ] Add `use ori_registry::{BUILTIN_TYPES, TypeTag}` import
- [ ] Verify all remaining tests pass: `cargo test -p oric -- consistency`

### Coordination with Section 09

If Section 10 completes before Section 09:
- The `TYPECK_*` allowlists, `typeck_method_list_is_sorted`, and `typeck_primitive_methods_in_ir` tests remain in `consistency.rs`
- The `EVAL_*` and `IR_*` allowlists are removed
- The `import` of `TYPECK_BUILTIN_METHODS` remains

If Section 09 completes before Section 10:
- Section 09 will have already removed `TYPECK_BUILTIN_METHODS` — so the old typeck-vs-eval tests will already be gone
- Section 10 only needs to remove `EVAL_BUILTIN_METHODS` and `ITERATOR_METHOD_NAMES`

If both sections are done simultaneously, all allowlists and comparison tests are removed together and replaced by the unified registry enforcement tests.

---

## 10.7 Validation & Regression

### Test Commands

Execute in this order. Each command must pass before proceeding to the next.

- [ ] `cargo check -p ori_eval` — evaluator compiles with registry dependency
- [ ] `cargo test -p ori_eval` — all evaluator unit tests pass
- [ ] `cargo test -p oric -- consistency` — rewritten consistency tests pass
- [ ] `cargo test -p oric -- dispatch_coverage` — new dispatch coverage test passes
- [ ] `cargo st` — all spec tests pass (spec tests exercise the evaluator end-to-end)
- [ ] `./test-all.sh` — full test suite passes (includes clippy, fmt, all crates)

### Specific Regressions to Watch

| Risk | How to Detect | Mitigation |
|------|--------------|------------|
| Method missing from registry that eval dispatches | `dispatch_coverage` test fails with "no such method" | Add method to registry (likely a Section 03-07 gap) |
| Registry declares method that eval doesn't dispatch | `dispatch_coverage` test fails | Add dispatch handler or mark as resolver-dispatched |
| Iterator method mismatch | `registry_methods_dispatchable_by_eval` iterator check fails | Sync `CollectionMethod` enum with registry |
| `BuiltinMethodResolver` fails to resolve a valid method | Spec tests fail with runtime "no such method" | Verify `BuiltinMethodResolver::new()` correctly iterates registry |
| `BuiltinMethodNames` has a field for a name the registry doesn't declare | `builtin_method_names_match_registry` test fails | Remove stale field or add method to registry |
| Performance regression from startup overhead | Measurable only at scale; unlikely with ~200 methods | Profile interpreter startup if suspected; the FxHashSet build is amortized across all method calls |

### Grep Verification

After all changes are complete, verify no stale references remain:

```bash
# Should return 0 results (array eliminated):
grep -r "EVAL_BUILTIN_METHODS" compiler/ --include="*.rs"

# Should return 0 results (array eliminated):
grep -r "ITERATOR_METHOD_NAMES" compiler/ --include="*.rs"

# Should return 0 results in non-test files (allowlists eliminated):
grep -r "EVAL_METHODS_NOT_IN" compiler/ --include="*.rs" | grep -v "tests/"

# Should return 0 results in non-test files (allowlists eliminated):
grep -r "IR_METHODS_DISPATCHED_VIA" compiler/ --include="*.rs" | grep -v "tests/"
```

---

## Exit Criteria

All of the following must be true before this section is marked complete:

1. **`EVAL_BUILTIN_METHODS` eliminated:** The array no longer exists in `methods/helpers/mod.rs`; no code imports it
2. **`ITERATOR_METHOD_NAMES` eliminated:** The array no longer exists in `resolvers/mod.rs`; no code imports it
3. **`BuiltinMethodResolver` reads from registry:** `BuiltinMethodResolver::new()` builds its `FxHashSet` from `ori_registry::BUILTIN_TYPES`, not from a hardcoded array
4. **`BuiltinMethodNames` validated:** An exhaustive-destructure test verifies every field name is a registry-declared method
5. **Dispatch coverage test passes:** Every registry-declared method for implemented types can be dispatched without "no such method"
6. **Iterator method sync validated:** Registry Iterator/DEI methods match `CollectionMethod::all_iterator_variants()`
7. **Eval-side allowlists eliminated:** `EVAL_METHODS_NOT_IN_IR`, `EVAL_METHODS_NOT_IN_TYPECK`, `IR_METHODS_DISPATCHED_VIA_RESOLVERS`, and `COLLECTION_TYPES` removed from `consistency.rs`
8. **Dispatch architecture unchanged:** `dispatch_builtin_method()`, `CollectionMethodResolver`, `UserRegistryResolver`, and all `dispatch_*_method` functions are unmodified
9. **Format variant tests unchanged:** All 6 format variant sync tests pass without modification
10. **Full test suite green:** `cargo test -p ori_eval`, `cargo test -p oric`, `cargo st`, and `./test-all.sh` all pass
11. **Grep clean:** No stale references to eliminated arrays or allowlists in production code
