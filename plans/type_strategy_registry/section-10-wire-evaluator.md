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
    title: "BuiltinMethodNames assertion test"
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

**Context:** The evaluator is the simplest wiring target. Unlike `ori_types` (Section 09), which must read return types and parameter specs for inference, `ori_eval` only needs the *existence* of methods per type for two purposes: (1) building the `BuiltinMethodResolver`'s `FxHashSet<(Name, Name)>` for O(1) resolution, and (2) cross-crate consistency testing. The evaluator's dispatch mechanism (`dispatch_builtin_method`, `CollectionMethodResolver`, etc.) does not change — it continues to pattern-match on `Value` variants and pre-interned `Name` fields. Only the *source of truth* for what methods exist shifts from hardcoded arrays to registry queries.

**Principle:** The evaluator is a *consumer* of the registry, not a *mirror* of it. The registry declares what methods a type has. The evaluator implements the runtime behavior. The registry-derived data validates that these two sets are aligned, but the evaluator's internal dispatch architecture remains intact.

**Phase boundary discipline:** `ori_registry` is a zero-dependency static data crate.
`ori_eval` reads `BUILTIN_TYPES` at interpreter startup (once, to build the `FxHashSet`)
and at test time. This is NOT phase bleeding — it is equivalent to replacing an inline
`const` array with an imported one. No registry concepts (`TypeTag`, `MethodDef`, `ReturnTag`)
leak into the evaluator's runtime dispatch path. The dispatch functions continue to use
`Value` variants and `Name` comparison, not registry types.

**Cargo.toml:** `ori_registry` is already listed as a dependency of `ori_eval` (added during Sections 03-08). No Cargo.toml changes are needed for this section.

### Implementation Ordering

The subsections are numbered 10.1-10.7 for exposition, but the implementation order
differs due to data dependencies:

1. **10.5 (Format variant sync)** — no-op, verify only. Can be done first or in parallel.
2. **10.3 (BuiltinMethodNames assertion test)** — independent, adds a test in `ori_eval` only. Can be done first or in parallel with the main commit. **Requires** creating `methods/tests.rs` and adding `#[cfg(test)] mod tests;` to `methods/mod.rs`.
3. **10.6 prerequisites** — remove `#[cfg(test)]` from `all_iterator_variants()` and `from_name()` in `resolvers/mod.rs`. Must be done before 10.6's test can compile.
4. **10.1 + 10.2 + 10.4 + 10.6 (main commit)** — these MUST be done atomically in a single commit:
   - 10.1: Replace `EVAL_BUILTIN_METHODS` with registry iteration in `BuiltinMethodResolver::new()`
   - 10.2: Remove `ITERATOR_METHOD_NAMES`
   - 10.4: Create `dispatch_coverage.rs` with `METHODS_NOT_YET_IN_EVAL` (migrated from 10.6's deletions)
   - 10.6: Rewrite `consistency.rs` — remove 6 tests, 6 allowlists, 3 helpers; add new enforcement test
5. **10.7 (Validation)** — run the full verification suite after the main commit.

**WARNING — Large commit risk:** The main commit (step 4) touches 5+ files across 2 crates and involves migrating a 232-entry allowlist. Budget for iteration. Consider doing 10.3 and 10.5 in a preparatory commit to reduce the main commit's scope.

---

## 10.1 Replace EVAL_BUILTIN_METHODS

### Current State

**File:** `compiler/ori_eval/src/methods/helpers/mod.rs` (lines 12-264)

`EVAL_BUILTIN_METHODS` is a hand-maintained `&[(&str, &str)]` array of 230 `(type_name, method_name)` pairs. It serves two purposes:

1. **`BuiltinMethodResolver` construction** (`compiler/ori_eval/src/interpreter/resolvers/builtin/mod.rs`, line 35): Each pair is interned into an `FxHashSet<(Name, Name)>` at interpreter startup for O(1) method existence checks during dispatch.

2. **Cross-crate consistency tests** (`compiler/oric/src/eval/tests/methods/consistency.rs`): The array is compared against `ori_registry::BUILTIN_TYPES` and `ori_ir::BUILTIN_METHODS` to detect drift between phases. (Note: the consistency tests have already been partially migrated to use `ori_registry` as the source of truth -- test names now reference "registry" rather than "typeck".)

**Exported via:** `ori_eval/src/lib.rs` line 58 (`pub use methods::{dispatch_builtin_method_str, EVAL_BUILTIN_METHODS};`).

### After Migration

The array is eliminated. Both consumers switch to registry queries:

1. **`BuiltinMethodResolver::new()`** iterates `BUILTIN_TYPES` to build its `FxHashSet`:
   ```rust
   // BEFORE (methods/helpers/mod.rs + resolvers/builtin/mod.rs)
   pub const EVAL_BUILTIN_METHODS: &[(&str, &str)] = &[
       ("Duration", "add"),
       ("Duration", "clone"),
       // ... 230 entries
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
   use ori_registry::BUILTIN_TYPES;

   /// Map registry PascalCase type names to evaluator convention.
   ///
   /// The evaluator's `get_value_type_name()` (via `TypeNames`) uses lowercase
   /// for 5 types that the registry stores as PascalCase:
   /// List, Map, Range, Tuple, Error.
   ///
   /// Must stay in sync with `TypeNames::new()` in `interpreter/interned_names.rs`
   /// and `Value::type_name()` in `ori_patterns/src/value/conversions.rs`.
   fn eval_type_name(registry_name: &str) -> &str {
       match registry_name {
           "List" => "list",
           "Map" => "map",
           "Range" => "range",
           "Tuple" => "tuple",
           "Error" => "error",
           other => other, // int, float, str, Duration, Size, etc. already match
       }
   }

   // In BuiltinMethodResolver::new():
   let known_methods = BUILTIN_TYPES
       .iter()
       .flat_map(|type_def| {
           let type_name = interner.intern(eval_type_name(type_def.name));
           type_def.methods.iter().map(move |method| {
               (type_name, interner.intern(method.name))
           })
       })
       .collect();
   ```

   **Name mapping sync point:** `eval_type_name()` must stay in sync with three locations:
   1. `TypeNames::new()` in `interpreter/interned_names.rs` (lines 72-93) -- the strings interned here determine what `get_value_type_name()` returns
   2. `Value::type_name()` in `ori_patterns/src/value/conversions.rs` (lines 83-114) -- the static type name strings
   3. `legacy_type_name()` in `oric/src/eval/tests/methods/consistency.rs` (lines 11-20) -- the existing test helper that does the same mapping

   **Known tech debt:** `eval_type_name()` is a manual mirror of casing conventions. The proper fix is to normalize the evaluator's type names to match the registry (or vice versa). Track for Section 14 or a separate cleanup task. A test in 10.7 verifies the mapping covers all types that need it.

2. **Consistency tests** no longer need `EVAL_BUILTIN_METHODS` -- the registry IS the source of truth. The old `eval_methods_recognized_by_registry` and `registry_methods_implemented_in_eval` tests in `consistency.rs` are replaced by a single registry-level enforcement test (see 10.6).

### Migration Steps

- [ ] Add `eval_type_name()` mapping function to `resolvers/builtin/mod.rs`
- [ ] In `BuiltinMethodResolver::new()`, replace `crate::methods::EVAL_BUILTIN_METHODS.iter()` with `ori_registry::BUILTIN_TYPES.iter().flat_map(...)` using `eval_type_name()`
- [ ] Remove `pub const EVAL_BUILTIN_METHODS` from `methods/helpers/mod.rs`
- [ ] Remove `pub use helpers::EVAL_BUILTIN_METHODS` from `methods/mod.rs` (line 31)
- [ ] Update `lib.rs` line 58: change `pub use methods::{dispatch_builtin_method_str, EVAL_BUILTIN_METHODS}` to `pub use methods::dispatch_builtin_method_str`
- [ ] Verify `cargo check -p ori_eval` passes
- [ ] **Atomic commit:** Steps 10.1, 10.2, 10.4, and 10.6 MUST be in a single commit because `consistency.rs` is the only external consumer of `EVAL_BUILTIN_METHODS` and `ITERATOR_METHOD_NAMES`, and `TYPECK_METHODS_NOT_IN_EVAL` (removed in 10.6) is the source data for `METHODS_NOT_YET_IN_EVAL` (created in 10.4)

### What Does NOT Change

- The `dispatch_builtin_method()` function in `methods/mod.rs` (lines 402-438) -- it pattern-matches on `Value` variants, not on an array
- The `dispatch_*_method()` sub-dispatch functions (numeric, collections, variants, units, ordering, error) — they use pre-interned `Name` comparison internally
- The `BuiltinMethodResolver`'s `FxHashSet` lookup strategy — the set is still built at startup, still O(1) per lookup
- The resolver chain priority order (User=0, Collection=1, Builtin=2)

---

## 10.2 Replace ITERATOR_METHOD_NAMES

### Current State

**File:** `compiler/ori_eval/src/interpreter/resolvers/mod.rs` (lines 234-259)

`ITERATOR_METHOD_NAMES` is a hand-maintained `&[&str]` array of 24 sorted method names for Iterator + DoubleEndedIterator methods. It serves one purpose:

1. **Cross-crate consistency test** (`consistency.rs` lines 783-810): `iterator_registry_methods_match_eval_resolver()` compares this array against `ori_registry::methods_for(TypeTag::Iterator)` entries. (Note: this test was previously named `iterator_typeck_methods_match_eval_resolver` and compared against `TYPECK_BUILTIN_METHODS`; it has already been migrated to use the registry.)

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
// The enforcement test (10.6) uses BUILTIN_TYPES filtered by TypeTag::Iterator.
// DEI methods are included in Iterator's TypeDef via the dei_only flag.
```

### Migration Steps

- [ ] Remove `pub const ITERATOR_METHOD_NAMES` from `resolvers/mod.rs`
- [ ] Remove `pub use interpreter::resolvers::ITERATOR_METHOD_NAMES` from `lib.rs` line 68
- [ ] Verify `cargo check -p ori_eval` passes
- [ ] **Atomic commit:** Done in the same commit as 10.1, 10.4, and 10.6 (see 10.1 ordering note)

### What Does NOT Change

- The `CollectionMethodResolver` and its `MethodNames` struct — it builds its own pre-interned names at construction time
- The `resolve_iterator_method()` dispatch chain — it uses `Name` comparison against its own `MethodNames`, not against this array
- The `CollectionMethod` enum and its `all_iterator_variants()` method
- The `is_iterator_method()` predicate

### Decision: Should CollectionMethodResolver read from registry?

**No.** The `CollectionMethodResolver`'s `MethodNames` struct pre-interns method names for O(1) `Name` comparison at dispatch time. This is an optimization detail. The resolver does not need to know *what* methods exist at compile time — it only needs interned names to compare against at runtime. The `MethodNames::new()` constructor is already minimal (one `interner.intern()` call per name). Replacing it with a registry iteration would add complexity for no performance gain.

The enforcement test (10.6) validates that every method the registry declares for Iterator/DoubleEndedIterator has a corresponding `CollectionMethod` variant and resolver dispatch path. This catches drift without coupling the resolver to the registry at runtime.

---

## 10.3 BuiltinMethodNames Assertion Test

### Current State

**File:** `compiler/ori_eval/src/methods/mod.rs` (lines 42-171)

`BuiltinMethodNames` is a 97-field struct where each field holds a pre-interned `Name` for a builtin method. It is constructed once per `Interpreter` via `BuiltinMethodNames::new(interner)` and bundled into `DispatchCtx` for method dispatch. Every sub-dispatch function (`dispatch_int_method`, `dispatch_str_method`, etc.) compares `method: Name` against fields like `ctx.names.add`, `ctx.names.clone_`, etc. using `u32 == u32` comparison instead of string matching.

This struct is purely a performance optimization -- it does not define what methods exist. It provides named handles for fast equality testing during dispatch.

### Decision Analysis

| Criterion | Option A: Keep + Assert | Option B: Replace with Registry Lookup |
|-----------|-------------------------|----------------------------------------|
| **Runtime cost** | O(1) `u32 == u32` per method call (current) | O(1) `u32 == u32` if interned, O(n) string compare if not |
| **Startup cost** | 97 `intern()` calls (current) | Same if pre-interning; or lazy intern on first use |
| **Code change** | Add assertion test only (~20 lines) | Rewrite all 6 `dispatch_*_method` functions |
| **Correctness risk** | Low -- struct is validated, dispatch unchanged | Medium -- refactoring 6 dispatch functions risks subtle bugs |
| **Registry coupling** | Loose -- test-time only | Tight -- runtime dependency |
| **Maintenance burden** | When registry adds a method name, must add field (but test catches the gap) | When registry adds a method, it's auto-available (but dispatch still needs a handler) |

### Recommendation: Option A -- Keep the Optimization, Add Assertion Test

The `BuiltinMethodNames` struct is an optimization detail (interned names for O(1) dispatch), not a source of truth. The registry is the source of truth for what methods exist; `BuiltinMethodNames` is the mechanism for dispatching them quickly.

Replacing it would require rewriting every `dispatch_*_method` function to look up method names differently, with no user-visible benefit. The existing code is correct, fast, and well-tested.

**What to add:** A test that validates:
1. **Compile-time exhaustiveness** -- an exhaustive destructure of `BuiltinMethodNames` that fails to compile if a field is added without updating the test
2. **Runtime registry comparison** -- every method-name field (excluding known non-method fields like `duration` and `size`) appears in `BUILTIN_TYPES`

**Location:** `BuiltinMethodNames` is `pub(crate)`, so the test MUST live in `ori_eval` (not `oric`). Per test conventions, it goes in `compiler/ori_eval/src/methods/tests.rs` (new file), with `#[cfg(test)] mod tests;` added to `methods/mod.rs`.

```rust
// In compiler/ori_eval/src/methods/tests.rs (new file — requires adding
// `#[cfg(test)] mod tests;` to methods/mod.rs)
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
        // Common trait methods
        add, sub, mul, div, rem, neg,
        compare, equals, clone_, to_str, debug, hash,
        contains, len, length, is_empty, not, unwrap, concat,
        // Numeric (int-specific)
        floor_div, bit_and, bit_or, bit_xor, bit_not, shl, shr,
        // String-specific
        to_uppercase, to_lowercase, trim, starts_with, ends_with, escape,
        replace, repeat, split, substring,
        // List-specific
        first, last, pop, push, set, slice, sort, sort_stable, take, skip, drop_,
        // Map-specific
        contains_key, get, keys, values, entries,
        // Map + Set shared
        insert, remove, union, intersection, difference, to_list,
        // Option-specific
        unwrap_or, is_some, is_none, ok_or,
        // Result-specific
        is_ok, is_err, unwrap_err,
        // Ordering
        is_less, is_equal, is_greater, is_less_or_equal, is_greater_or_equal,
        reverse, then,
        // Type names for associated function dispatch
        duration, size,
        // Duration/Size operator aliases
        subtract, multiply, divide, remainder, negate,
        // Duration accessors
        nanoseconds, microseconds, milliseconds, seconds, minutes, hours,
        // Size accessors
        bytes, kilobytes, megabytes, gigabytes, terabytes,
        // Iterator
        iter,
        // Conversion (Into trait)
        into,
        // Traceable trait (Error type)
        trace, trace_entries, has_trace, with_trace, message,
    } = names;

    // The destructure above ensures exhaustiveness at compile time.
    // Suppress unused variable warnings.
    let _ = (add, sub, mul, div, rem, neg, compare, equals, clone_,
             to_str, debug, hash, contains, len, length, is_empty, not,
             unwrap, concat, floor_div, bit_and, bit_or, bit_xor, bit_not,
             shl, shr, to_uppercase, to_lowercase, trim, starts_with,
             ends_with, escape, replace, repeat, split, substring,
             first, last, pop, push, set, slice, sort, sort_stable,
             take, skip, drop_, contains_key, get, keys, values, entries,
             insert, remove, union, intersection, difference, to_list,
             unwrap_or, is_some, is_none, ok_or, is_ok, is_err, unwrap_err,
             is_less, is_equal, is_greater, is_less_or_equal,
             is_greater_or_equal, reverse, then, duration, size,
             subtract, multiply, divide, remainder, negate,
             nanoseconds, microseconds, milliseconds, seconds,
             minutes, hours, bytes, kilobytes, megabytes, gigabytes,
             terabytes, iter, into, trace, trace_entries, has_trace,
             with_trace, message);

    // Note: The struct also contains type names (duration, size) used for
    // associated function dispatch, not method dispatch. These are expected
    // to NOT appear in the method name set.
}
```

The exhaustive destructure provides compile-time enforcement: adding a field to `BuiltinMethodNames` without updating the test causes a compile error.

The test must ALSO validate at runtime that each field name corresponds to a registry method. The following code block completes the test by comparing field names against `BUILTIN_TYPES`:

**Known non-method fields:** `duration` and `size` are type names used for associated function dispatch, not method names. They must be excluded from the registry comparison. `length` is an alias for `len` on `str` -- verify either the alias or the canonical name exists in the registry.
```rust
    // Collect interned names → actual strings for registry comparison
    let field_names: BTreeSet<&str> = [
        "add", "sub", "mul", "div", "rem", "neg", "compare", "equals",
        "clone", "to_str", "debug", "hash", "contains", "len", "length",
        "is_empty", "not", "unwrap", "concat",
        "floor_div", "bit_and", "bit_or", "bit_xor", "bit_not", "shl", "shr",
        "to_uppercase", "to_lowercase", "trim", "starts_with", "ends_with",
        "escape", "replace", "repeat", "split", "substring",
        "first", "last", "pop", "push", "set", "slice", "sort", "sort_stable",
        "take", "skip", "drop",
        "contains_key", "get", "keys", "values", "entries",
        "insert", "remove", "union", "intersection", "difference", "to_list",
        "unwrap_or", "is_some", "is_none", "ok_or",
        "is_ok", "is_err", "unwrap_err",
        "is_less", "is_equal", "is_greater", "is_less_or_equal",
        "is_greater_or_equal", "reverse", "then",
        "subtract", "multiply", "divide", "remainder", "negate",
        "nanoseconds", "microseconds", "milliseconds", "seconds", "minutes", "hours",
        "bytes", "kilobytes", "megabytes", "gigabytes", "terabytes",
        "iter", "into",
        "trace", "trace_entries", "has_trace", "with_trace", "message",
    ].into_iter().collect();

    // Non-method fields (type names for associated function dispatch)
    let non_method_fields: BTreeSet<&str> = ["duration", "size"].into_iter().collect();

    // All method names from registry
    let registry_method_names: BTreeSet<&str> = BUILTIN_TYPES
        .iter()
        .flat_map(|td| td.methods.iter().map(|m| m.name))
        .collect();

    // Every field name (except known non-method fields) must exist in registry
    let method_fields: BTreeSet<&str> = field_names
        .difference(&non_method_fields)
        .copied()
        .collect();
    let missing: Vec<_> = method_fields
        .difference(&registry_method_names)
        .collect();
    assert!(
        missing.is_empty(),
        "BuiltinMethodNames has fields not in registry: {missing:?}"
    );
```

### Migration Steps

- [ ] Add `#[cfg(test)] mod tests;` at the bottom of `compiler/ori_eval/src/methods/mod.rs`
- [ ] Create `compiler/ori_eval/src/methods/tests.rs`
- [ ] Add `builtin_method_names_match_registry` test containing both the exhaustive destructure (compile-time check) AND the registry comparison (runtime check)
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

The `minimal_value_for()` helper constructs the simplest possible `Value` for each `TypeTag`. The match must be exhaustive to catch future `TypeTag` additions at compile time. Some arms (Unit, Never, Function, DoubleEndedIterator) are unreachable from `BUILTIN_TYPES` iteration but are included for exhaustiveness:

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
        TypeTag::Result => Some(Value::ok(Value::Void)),
        TypeTag::List => Some(Value::list(vec![])),
        TypeTag::Map => Some(Value::Map(Heap::new(Default::default()))),
        TypeTag::Set => Some(Value::Set(Heap::new(Default::default()))),
        TypeTag::Range => Some(Value::Range(RangeValue::exclusive(0, 0))),
        TypeTag::Tuple => Some(Value::tuple(vec![])),
        TypeTag::Error => Some(Value::error("test")),
        // Types without a direct Value representation or not in BUILTIN_TYPES.
        // Unit/Never/Function are excluded from BUILTIN_TYPES (no methods).
        // DoubleEndedIterator aliases to Iterator via TypeTag::base_type().
        // Channel has a Value representation gap (no Channel variant in Value).
        // Iterator is dispatched by CollectionMethodResolver, not BuiltinMethodResolver.
        TypeTag::Unit
        | TypeTag::Never
        | TypeTag::Iterator
        | TypeTag::DoubleEndedIterator
        | TypeTag::Channel
        | TypeTag::Function => None,
    }
}
```

**WARNING — Complexity risk:** The `Heap::new(Default::default())` calls for Map and Set
depend on `IndexMap` and `IndexSet` implementing `Default`. Verify this compiles. If not,
use explicit empty constructors (`IndexMap::new()` / `IndexSet::new()`).

**Note on Iterator/DoubleEndedIterator:** These are dispatched by `CollectionMethodResolver`, not `BuiltinMethodResolver`. They cannot be tested via `dispatch_builtin_method_str`. A separate test covers them (see 10.6).

**Handling unimplemented methods:** The registry declares ALL methods a type has, but the evaluator only implements a subset. The `TYPECK_METHODS_NOT_IN_EVAL` allowlist in `consistency.rs` currently documents 232 such gaps.

**Solution:** Create a `METHODS_NOT_YET_IN_EVAL` allowlist in `dispatch_coverage.rs`, migrated from `TYPECK_METHODS_NOT_IN_EVAL` in `consistency.rs` (with type names mapped via `eval_type_name()` convention). The registry is the source of truth for what SHOULD exist; this allowlist tracks what DOESN'T yet exist. It must shrink monotonically per Section 14's progressive enforcement principle.

```rust
/// Methods declared in the registry but not yet implemented in eval.
/// This list must shrink monotonically. Adding entries requires justification.
/// Removing entries (= implementing methods) is always welcome.
const METHODS_NOT_YET_IN_EVAL: &[(&str, &str)] = &[
    // Migrated from TYPECK_METHODS_NOT_IN_EVAL in consistency.rs
    // (type names use eval convention: List->list, Map->map, etc.)
    ("Channel", "close"),
    // ... (migrate full 232-entry list from consistency.rs)
];
```

**Handling CollectionMethodResolver-dispatched methods:** `dispatch_builtin_method_str` only exercises the `BuiltinMethodResolver` path. Methods dispatched by `CollectionMethodResolver` for non-Iterator types must also be filtered out, or they will produce false "no such method" failures. The affected methods are:

| Type | Methods via CollectionMethodResolver |
|------|--------------------------------------|
| list | map, filter, fold, find, any, all, join |
| map | map, filter |
| range | map, filter, fold, find, any, all, collect |
| Ordering | then_with |
| Iterator/DEI | all methods (already skipped via `minimal_value_for` returning `None`) |

Add these to a `COLLECTION_RESOLVER_METHODS` filter set in the test, or build the set dynamically from `CollectionMethod` variants.

### Migration Steps

- [ ] Create `compiler/oric/src/eval/tests/methods/dispatch_coverage.rs`
- [ ] Add `mod dispatch_coverage;` to `compiler/oric/src/eval/tests/methods/mod.rs`
- [ ] Migrate all 232 entries from `TYPECK_METHODS_NOT_IN_EVAL` in `consistency.rs` into `METHODS_NOT_YET_IN_EVAL` in `dispatch_coverage.rs`, applying `eval_type_name()` mapping (5 types change case: List->list, Map->map, Range->range, Tuple->tuple, Error->error)
- [ ] Add `COLLECTION_RESOLVER_METHODS` filter set listing methods dispatched by `CollectionMethodResolver` for non-Iterator types (see table above)
- [ ] Implement `minimal_value_for()` helper with exhaustive `TypeTag` match
- [ ] Implement `every_registry_method_has_eval_dispatch_handler` test that skips methods in both filter sets
- [ ] Verify test passes: `cargo test -p oric -- every_registry_method_has_eval_dispatch`
- [ ] For any unexpected "no such method" failure: determine whether it is a missing eval handler (fix the handler) or a registry method that should be filtered (add to appropriate filter set with justification)

**Atomic commit:** 10.4 must be in the same commit as 10.1, 10.2, and 10.6 because `TYPECK_METHODS_NOT_IN_EVAL` (removed in 10.6) is the source data for `METHODS_NOT_YET_IN_EVAL` (created in 10.4).

### What Does NOT Change

- `CollectionMethodResolver` dispatch chain — it handles its own set of methods
- `UserRegistryResolver` — it handles user-defined and derived methods
- The resolver priority chain (User=0, Collection=1, Builtin=2)
- Any `dispatch_*_method` function implementation

---

## 10.5 Format Variant Sync

### Current State

**File:** `compiler/ori_eval/src/interpreter/prelude.rs` (lines 73-95)

`register_format_variants()` hardcodes `FormatType`, `Alignment`, and `Sign` variant strings for the `Formattable` trait's `FormatSpec` struct. These appear in 4 independent locations:

1. `ori_ir/src/format_spec.rs` — enum definitions (source of truth)
2. `ori_types/src/check/registration/builtin_types.rs` — type registration
3. `ori_eval/src/interpreter/prelude.rs` — `register_format_variants()` globals
4. `ori_rt/src/format/mod.rs` — runtime enum + parse

Current sync enforcement: source-scanning tests in `consistency.rs` (lines 825-994) that read the `.rs` files and grep for variant name strings. The tests in `ori_rt` guard `ori_rt <-> ori_ir` sync via variant count assertions.

### Decision: Should Format Variants Move to the Registry?

**No.** Format variants are TYPE VARIANTS (enum constructors), not method behavior. They represent the values of the `FormatType`, `Alignment`, and `Sign` enums, not methods on types. The registry's domain is "what methods does type X have and how do they behave?" Format variants are a different concern: "what are the possible values of enum type Y?"

Moving them into the registry would conflate two separate domains:
- **Type behavioral specification** (methods, operators, memory strategy) — registry domain
- **Enum variant enumeration** (FormatType::Binary, Alignment::Left, Sign::Plus) — `ori_ir` domain

### Recommendation: Leave in `ori_ir`, Keep Existing Sync Tests

The existing 4-way sync tests work correctly today. They enforce consistency by scanning source files — not elegant, but effective and already proven. These tests will be revisited in Section 14 (Enforcement & Exit) as part of the broader "can we make this cleaner?" pass.

If a future iteration of the registry adds support for enum variant declarations (not just method declarations), format variants could migrate at that point. But that is out of scope for this plan.

### Migration Steps

- [ ] No code changes to `register_format_variants()` in `interpreter/prelude.rs`
- [ ] No code changes to `consistency.rs` format variant sync tests
- [ ] Document in Section 14 that format variant sync is a candidate for future registry extension
- [ ] Verify existing format variant tests still pass: `cargo test -p oric -- format_type_variants`

---

## 10.6 Evaluator Enforcement Test

### Purpose

Replace the 6 cross-crate consistency tests in `compiler/oric/src/eval/tests/methods/consistency.rs` that compare `EVAL_BUILTIN_METHODS` against `ori_registry::BUILTIN_TYPES` (and `ori_ir::BUILTIN_METHODS`) with a single registry-level enforcement test. The old tests become redundant because both phases now read from the same source of truth.

**File size note:** `consistency.rs` is currently 1070 lines (over the 500-line limit). After removing the 6 tests, 6 allowlists, and 3 helper functions (~700 lines), the remaining content (~370 lines) should be well under the limit. If it exceeds 500 lines after adding the new enforcement test, split the format variant sync tests into `format_consistency.rs` (with `mod format_consistency;` in `mod.rs`).

### Tests Being Replaced

| Old Test | Lines | What It Checks | Why Redundant |
|----------|-------|----------------|---------------|
| `ir_methods_implemented_in_eval` | 132-153 | Every IR method has an eval dispatch | Registry IS the IR method list |
| `eval_primitive_methods_in_ir` | 158-179 | Every eval method is in IR | Eval reads from registry = IR |
| `eval_method_list_is_sorted` | 184-193 | `EVAL_BUILTIN_METHODS` sorted | Array eliminated |
| `eval_methods_recognized_by_registry` | 733-750 | Every eval method has a registry entry | Both read from registry |
| `registry_methods_implemented_in_eval` | 756-773 | Every registry method has an eval handler | Both read from registry |
| `eval_iterator_method_names_sorted` | 814-823 | `ITERATOR_METHOD_NAMES` sorted | Array eliminated |

### Tests Being Kept (Modified)

| Test | Lines | What It Checks | Why Still Needed |
|------|-------|----------------|------------------|
| `iterator_registry_methods_match_eval_resolver` | 783-810 | Iterator methods match between registry and eval | Replaced by `iterator_methods_match_registry` -- compares registry Iterator/DEI methods against `CollectionMethod::all_iterator_variants()` |
| `registry_methods_sorted_per_type` | 689-701 | Registry methods sorted per type | Kept — validates registry data quality |
| `registry_primitive_methods_in_ir` | 705-727 | Registry methods in IR | Kept until Section 13 completes ori_ir migration |
| All format variant sync tests | 825-994 | FormatType/Alignment/Sign sync | Kept (see 10.5) |
| `well_known_generic_types_consistent` | 1023-1070 | Well-known generic resolution | Kept — unrelated to method registry |

### Allowlists Being Eliminated

These `const` arrays in `consistency.rs` are eliminated because they tracked gaps between independently-maintained lists. When both phases read from the same registry, gaps are structurally impossible:

| Allowlist | Lines | Entries | Purpose | Why Eliminated |
|-----------|-------|---------|---------|----------------|
| `COLLECTION_TYPES` | 45-57 | 11 | Types not yet in IR registry | Registry includes all types |
| `IR_METHODS_DISPATCHED_VIA_RESOLVERS` | 65-78 | 10 | IR methods dispatched via resolvers, not direct dispatch | Dispatch coverage test (10.4) validates this directly |
| `EVAL_METHODS_NOT_IN_IR` | 82-119 | 26 | Eval methods not in IR | Registry IS the IR |
| `EVAL_METHODS_NOT_IN_TYPECK` | 200-262 | 55 | Eval methods not recognized by typeck | Both read from registry |
| `TYPECK_METHODS_NOT_IN_IR` | 266-424 | 146 | Typeck methods not in IR | Both read from registry |
| `TYPECK_METHODS_NOT_IN_EVAL` | 429-685 | 232 | Typeck methods not in eval | Both read from registry |

**Phasing note:** The table above shows the final end-state (all eliminated). In practice, `TYPECK_METHODS_NOT_IN_IR` cannot be removed by Section 10 alone -- it is consumed by `registry_primitive_methods_in_ir` (being kept), which validates registry/IR alignment until Section 13 consolidates `BUILTIN_METHODS` into the registry. All other allowlists are removed in Section 10.

**What Section 10 removes now:** `COLLECTION_TYPES`, `IR_METHODS_DISPATCHED_VIA_RESOLVERS`, `EVAL_METHODS_NOT_IN_IR`, `EVAL_METHODS_NOT_IN_TYPECK`, `TYPECK_METHODS_NOT_IN_EVAL` (content migrated to `METHODS_NOT_YET_IN_EVAL` in dispatch_coverage.rs).

**What Section 10 keeps for now:** `TYPECK_METHODS_NOT_IN_IR` (removed by Section 13 when ori_ir's `BUILTIN_METHODS` is consolidated).

### New Enforcement Test

```rust
// In compiler/oric/src/eval/tests/methods/consistency.rs (rewritten)

/// Verify that every Iterator/DoubleEndedIterator method in the registry has
/// a corresponding CollectionMethod variant in the evaluator, and vice versa.
/// This replaces the old iterator_registry_methods_match_eval_resolver test.
#[test]
fn iterator_methods_match_registry() {
    use ori_registry::{BUILTIN_TYPES, TypeTag};
    use std::collections::BTreeSet;

    // Registry iterator methods (DEI methods are on the Iterator TypeDef
    // with dei_only flag; BUILTIN_TYPES has no separate DoubleEndedIterator entry)
    let registry_iter_methods: BTreeSet<&str> = BUILTIN_TYPES
        .iter()
        .filter(|td| td.tag == TypeTag::Iterator)
        .flat_map(|td| td.methods.iter().map(|m| m.name))
        .collect();

    // Eval iterator methods from CollectionMethod, excluding __-prefixed
    // protocol methods (__collect_set, __iter_next) that the registry
    // intentionally omits
    let eval_iter_methods: BTreeSet<&str> = CollectionMethod::all_iterator_variants()
        .iter()
        .map(|&(name, _)| name)
        .filter(|name| !name.starts_with("__"))
        .collect();

    let in_registry_not_eval: Vec<_> = registry_iter_methods
        .difference(&eval_iter_methods).collect();
    let in_eval_not_registry: Vec<_> = eval_iter_methods
        .difference(&registry_iter_methods).collect();

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

**Prerequisites (can be done in a preparatory commit):**

- [ ] Remove `#[cfg(test)]` from `CollectionMethod::all_iterator_variants()` in `resolvers/mod.rs` (line 192) and make it `pub`
- [ ] Remove `#[cfg(test)]` from `CollectionMethod::from_name()` in `resolvers/mod.rs` (line 140) and make it `pub`

**Tests to remove from `consistency.rs`:**

- [ ] Remove `ir_methods_implemented_in_eval` test
- [ ] Remove `eval_primitive_methods_in_ir` test
- [ ] Remove `eval_method_list_is_sorted` test
- [ ] Remove `eval_methods_recognized_by_registry` test
- [ ] Remove `registry_methods_implemented_in_eval` test
- [ ] Remove `eval_iterator_method_names_sorted` test

**Allowlists to remove from `consistency.rs`:**

- [ ] Remove `COLLECTION_TYPES`
- [ ] Remove `IR_METHODS_DISPATCHED_VIA_RESOLVERS`
- [ ] Remove `EVAL_METHODS_NOT_IN_IR`
- [ ] Remove `EVAL_METHODS_NOT_IN_TYPECK`
- [ ] Remove `TYPECK_METHODS_NOT_IN_EVAL` (content migrated to `METHODS_NOT_YET_IN_EVAL` in 10.4's `dispatch_coverage.rs`)

**Helpers to remove from `consistency.rs`:**

- [ ] Remove `ir_method_set()` (lines 122-127) -- only used by tests being removed
- [ ] Remove `legacy_type_name()` (lines 11-20) if no remaining test uses it; otherwise keep
- [ ] Remove `registry_method_pairs()` (lines 27-40) if no remaining test uses it; otherwise keep

**Allowlist to keep:**

- [ ] Keep `TYPECK_METHODS_NOT_IN_IR` until Section 13 (used by `registry_primitive_methods_in_ir` test)

**New test and imports:**

- [ ] Add `iterator_methods_match_registry` test (see code above), filtering `__`-prefixed protocol methods from the eval side
- [ ] Remove `use ori_eval::EVAL_BUILTIN_METHODS` import from `consistency.rs`
- [ ] Remove `use ori_eval::ITERATOR_METHOD_NAMES` import from `consistency.rs`
- [ ] Add `use ori_eval::interpreter::resolvers::CollectionMethod` import to `consistency.rs`
- [ ] Confirm `use ori_registry::{BUILTIN_TYPES, TypeTag}` is already present (added during Sections 03-08)

**Tests to keep unchanged:**

- [ ] All format variant sync tests (see 10.5)
- [ ] `well_known_generic_types_consistent`
- [ ] `registry_methods_sorted_per_type`
- [ ] `registry_primitive_methods_in_ir` (until Section 13)

**Verification:**

- [ ] Verify all remaining tests pass: `cargo test -p oric -- consistency`
- [ ] Verify `consistency.rs` is under 500 lines after cleanup

### Coordination with Section 09

Section 09 eliminates `TYPECK_BUILTIN_METHODS` from `ori_types`, but `consistency.rs` already uses `ori_registry` for type checker comparisons (test names reference "registry", not "typeck"). No ordering dependency exists between Sections 09 and 10.

Regardless of ordering:
- `TYPECK_METHODS_NOT_IN_IR` remains until Section 13 (bridges registry to `ori_ir`)
- `TYPECK_METHODS_NOT_IN_EVAL` is removed in Section 10 (content migrated to `METHODS_NOT_YET_IN_EVAL` in `dispatch_coverage.rs`)
- `registry_methods_sorted_per_type` and `registry_primitive_methods_in_ir` tests remain (validate registry/IR alignment)
- All `EVAL_*` and `IR_*` allowlists are removed in Section 10

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
| Iterator method mismatch | `iterator_methods_match_registry` test fails | Sync `CollectionMethod` enum with registry |
| `BuiltinMethodResolver` fails to resolve a valid method | Spec tests fail with runtime "no such method" | Verify `BuiltinMethodResolver::new()` correctly iterates registry |
| `BuiltinMethodNames` has a field for a name the registry doesn't declare | `builtin_method_names_match_registry` test fails | Remove stale field or add method to registry |
| Name mapping mismatch (`eval_type_name`) | `BuiltinMethodResolver` silently fails to resolve methods for List/Map/Range/Tuple/Error | Verify `eval_type_name()` matches `TypeNames::new()` and `Value::type_name()` -- all 5 types must map correctly |
| Performance regression from startup overhead | Measurable only at scale; unlikely with ~200 methods | Profile interpreter startup if suspected; the FxHashSet build is amortized across all method calls |

### Grep Verification

After all changes are complete, verify no stale references remain:

```bash
# Should return 0 results (arrays eliminated):
grep -r "EVAL_BUILTIN_METHODS" compiler/ --include="*.rs"
grep -r "ITERATOR_METHOD_NAMES" compiler/ --include="*.rs"

# Should return 0 results (allowlists eliminated from consistency.rs):
grep -r "EVAL_METHODS_NOT_IN" compiler/ --include="*.rs"
grep -r "IR_METHODS_DISPATCHED_VIA" compiler/ --include="*.rs"
grep -r "COLLECTION_TYPES" compiler/oric/ --include="*.rs"

# Should return 0 results after cleanup (or only in eval_type_name()):
grep -r "legacy_type_name" compiler/ --include="*.rs"

# Verify #[cfg(test)] removed from all_iterator_variants():
grep -B1 "all_iterator_variants" compiler/ori_eval/src/interpreter/resolvers/mod.rs
# Should NOT show #[cfg(test)] on the line above the function
```

---

## Exit Criteria

All of the following must be true before this section is marked complete:

1. **`EVAL_BUILTIN_METHODS` eliminated:** The 230-entry array no longer exists in `methods/helpers/mod.rs`; no code imports it
2. **`ITERATOR_METHOD_NAMES` eliminated:** The array no longer exists in `resolvers/mod.rs`; no code imports it
3. **`BuiltinMethodResolver` reads from registry:** `BuiltinMethodResolver::new()` builds its `FxHashSet` from `ori_registry::BUILTIN_TYPES`, not from a hardcoded array
4. **Name mapping in place:** `eval_type_name()` maps registry PascalCase names to evaluator convention for 5 types: List, Map, Range, Tuple, Error
5. **`BuiltinMethodNames` validated:** An exhaustive-destructure test in `methods/tests.rs` verifies every field name is a registry-declared method (compile-time exhaustiveness AND runtime registry comparison)
6. **Dispatch coverage test passes:** `every_registry_method_has_eval_dispatch_handler` in `dispatch_coverage.rs` passes (with `METHODS_NOT_YET_IN_EVAL` allowlist for unimplemented methods and `COLLECTION_RESOLVER_METHODS` filter for resolver-dispatched methods)
7. **Iterator method sync validated:** `iterator_methods_match_registry` in `consistency.rs` passes -- registry Iterator/DEI methods match `CollectionMethod::all_iterator_variants()` (excluding `__`-prefixed protocol methods)
8. **Eval-side allowlists eliminated:** `EVAL_METHODS_NOT_IN_IR`, `EVAL_METHODS_NOT_IN_TYPECK`, `IR_METHODS_DISPATCHED_VIA_RESOLVERS`, `COLLECTION_TYPES`, and `TYPECK_METHODS_NOT_IN_EVAL` removed from `consistency.rs` (`TYPECK_METHODS_NOT_IN_EVAL` content migrated to `METHODS_NOT_YET_IN_EVAL` in `dispatch_coverage.rs`)
9. **`all_iterator_variants()` public:** The `#[cfg(test)]` gate is removed; the method is `pub` and accessible from `oric` enforcement tests
10. **Dispatch architecture unchanged:** `dispatch_builtin_method()`, `CollectionMethodResolver`, `UserRegistryResolver`, and all `dispatch_*_method` functions are unmodified
11. **Format variant tests unchanged:** All format variant sync tests pass without modification
12. **Full test suite green:** `cargo test -p ori_eval`, `cargo test -p oric`, `cargo st`, and `./test-all.sh` all pass
13. **Grep clean:** No stale references to eliminated arrays or allowlists in production code (see grep verification above)
14. **Test file conventions met:** `methods/mod.rs` has `#[cfg(test)] mod tests;`; test body is in `methods/tests.rs`
15. **`consistency.rs` under 500 lines:** File is within the limit after removing tests and allowlists
16. **Phase boundary intact:** No registry types (`TypeTag`, `MethodDef`, `ReturnTag`) appear in evaluator runtime dispatch paths -- only in startup code (`BuiltinMethodResolver::new()`) and test code
