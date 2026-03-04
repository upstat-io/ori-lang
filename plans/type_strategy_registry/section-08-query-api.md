---
section: "08"
title: "Query API & Lookup Functions"
status: not-started
goal: "Define the public interface layer between ori_registry data and all consuming compiler phases"
depends_on: ["01", "02"]
consumed_by: ["09", "10", "11", "12", "13", "14"]
estimated_lines: 60
sections:
  - id: "08.1"
    title: "BUILTIN_TYPES master constant"
    status: not-started
  - id: "08.2"
    title: "find_type() function"
    status: not-started
  - id: "08.3"
    title: "find_method() function"
    status: not-started
  - id: "08.4"
    title: "Convenience iterators"
    status: not-started
  - id: "08.5"
    title: "Operator query strategy"
    status: not-started
  - id: "08.6"
    title: "Type lookup by name"
    status: not-started
  - id: "08.7"
    title: "Performance considerations"
    status: not-started
  - id: "08.8"
    title: "API documentation"
    status: not-started
---

# Section 08: Query API & Lookup Functions

**Context:** Sections 03-07 define the *data* (TypeDef instances for every builtin type). This section defines the *interface* through which compiler phases access that data. The query API must be:
1. **Pure** -- no allocation, no mutation, no IO (Salsa-compatible)
2. **Deterministic** -- same inputs always produce same outputs
3. **const-friendly** -- ideally `const fn` where Rust permits
4. **Discoverable** -- consumers can enumerate all types and methods

**Depends on:** Sections 01 (data model types) and 02 (crate structure). Sections 03-07 populate the data that these functions query, but the API shape is independent of specific type definitions.

**Relationship to existing code:** The functions defined here directly replace:
- `ori_ir::builtin_methods::find_method()` -- replaced by `ori_registry::find_method()`
- `ori_ir::builtin_methods::methods_for()` -- replaced by `ori_registry::methods_for()`
- `ori_ir::builtin_methods::method_names_for()` -- replaced by `ori_registry::method_names_for()`
- `ori_ir::builtin_methods::borrowing_method_names()` -- replaced by `ori_registry::borrowing_methods()`
- `ori_ir::builtin_methods::has_method()` -- replaced by `ori_registry::find_method().is_some()`
- `ori_ir::builtin_methods::method_borrows_receiver()` -- subsumed by `find_method().receiver`

**File:** `compiler/ori_registry/src/lib.rs` (query functions), `compiler/ori_registry/src/defs/mod.rs` (BUILTIN_TYPES assembly)

---

## 08.1 BUILTIN_TYPES Master Constant

The master array that enumerates every registered builtin type. All query functions iterate this array.

```rust
/// Every builtin type registered in the compiler.
///
/// This is the single source of truth. All query functions below
/// scan this array. Enforcement tests iterate it to verify that
/// every phase has a handler for every type and method.
///
/// Ordering: by TypeTag discriminant value (matches `ori_types::Tag` repr(u8)
/// ordering). This is deterministic, matches the type pool's pre-interned
/// primitive ordering, and means the array is implicitly sorted for future
/// binary search if needed.
pub const BUILTIN_TYPES: &[&TypeDef] = &[
    &INT,       // Tag::Int       = 0
    &FLOAT,     // Tag::Float     = 1
    &BOOL,      // Tag::Bool      = 2
    &STR,       // Tag::Str       = 3
    &CHAR,      // Tag::Char      = 4
    &BYTE,      // Tag::Byte      = 5
    // Unit (6), Never (7) have no methods -- excluded
    &ERROR,     // Tag::Error     = 8  (8 methods: message, trace, etc.)
    &DURATION,  // Tag::Duration  = 9
    &SIZE,      // Tag::Size      = 10
    &ORDERING,  // Tag::Ordering  = 11
    // --- containers (when added in Section 06-07) ---
    // &LIST,      // Tag::List      = 16
    // &MAP,       // Tag::Map       = 32
    // &SET,       // Tag::Set       = 18
    // &RANGE,     // Tag::Range     = 20
    // &OPTION,    // Tag::Option    = 17
    // &RESULT,    // Tag::Result    = 33
    // &ITERATOR,  // Tag::Iterator  = 21
    // &DOUBLE_ENDED_ITERATOR, // Tag::DoubleEndedIterator = 22
];
```

### Design decisions

**Ordering convention: TypeTag discriminant value.** Alternatives considered:
- *Alphabetical by name*: Human-readable in source, but the discriminant order already has a semantic grouping (primitives 0-11, simple containers 16-31, two-child 32-47) that aligns with how phases process types.
- *Arbitrary/insertion order*: No advantages, harder to maintain.
- *TypeTag discriminant*: Matches `Tag` repr(u8) ordering, which matches the type pool's pre-interned indices. If we ever need binary search, the array is already sorted by tag value.

**Types without methods (Unit, Never) are excluded.** They have no behavioral specification to declare. Including them would add empty `TypeDef` entries that every enforcement test must special-case. **Error IS included** — it has 8 methods (`message`, `trace`, `trace_entries`, `has_trace`, `with_trace`, `to_str`, `debug`, `clone`) defined in Section 05. If a future change adds methods to Unit (unlikely), add the entry then.

### Implementation tasks

- [ ] Define `BUILTIN_TYPES` in `defs/mod.rs` (assembles refs from `defs/int.rs`, `defs/str.rs`, etc.)
- [ ] Re-export from `lib.rs`
- [ ] Add a compile-time assertion: `BUILTIN_TYPES.len() > 0` (catches accidental empty array)
- [ ] Add enforcement test: every entry's `tag` field is unique (no duplicate type registrations)
- [ ] Add enforcement test: entries are sorted by `tag` discriminant value

---

## 08.2 find_type() Function

```rust
/// Look up a builtin type definition by its type tag.
///
/// Returns `None` for tags not in the registry (user-defined types,
/// Unit, Never, type variables, etc.). Consuming phases should
/// fall through to trait dispatch or error reporting when this returns `None`.
///
/// # Examples
///
/// ```
/// use ori_registry::{find_type, TypeTag};
///
/// let int_def = find_type(TypeTag::Int).unwrap();
/// assert_eq!(int_def.name, "int");
///
/// // Types without methods/operators are not in BUILTIN_TYPES
/// assert!(find_type(TypeTag::Unit).is_none());
/// ```
pub const fn find_type(tag: TypeTag) -> Option<&'static TypeDef> {
    let mut i = 0;
    while i < BUILTIN_TYPES.len() {
        if BUILTIN_TYPES[i].tag as u8 == tag as u8 {
            return Some(BUILTIN_TYPES[i]);
        }
        i += 1;
    }
    None
}
```

### Design decisions

**const fn: yes.** The function body uses only `while` + index comparison, which is stable in const contexts since Rust 1.46. The `tag as u8` comparison avoids needing `PartialEq` in const (which requires nightly `const_trait_impl`). Since `TypeTag` will be a `#[repr(u8)]` enum mirroring `ori_types::Tag`, the `as u8` cast is lossless and correct.

**Linear scan, not HashMap.** The array has 6-20 entries. A linear scan of `&[&TypeDef]` is:
- ~3 ns for 6 entries, ~10 ns for 20 entries (cache-line resident, branch-predicted)
- Zero initialization cost (no `LazyLock`, no `HashMap::new()`)
- `const fn` compatible (HashMap is not const-constructible in stable Rust)
- Deterministic (Salsa-compatible -- no hash seed randomization)

A `HashMap<TypeTag, &TypeDef>` would add ~200 bytes of metadata, require `LazyLock` or `phf`, and gain nothing measurable at this scale.

**Returns `Option`, not panic.** The caller may pass any `TypeTag` variant — including `Unit`, `Never`, `Function`, etc. — which are valid tags but not builtin types with method definitions. Returning `None` lets the caller decide the fallback (trait dispatch, error, etc.).

**DEI aliasing via `base_type()`.** Callers looking up methods should use `find_method()` or `methods_for()` (which call `base_type()` internally) rather than `find_type()` directly. `find_type(TypeTag::DoubleEndedIterator)` returns `None` because no separate TypeDef exists — Section 07's single-TypeDef model stores all iterator methods on `TypeTag::Iterator`.

### Implementation tasks

- [ ] Implement `find_type()` as `const fn` in `lib.rs`
- [ ] Unit test: every `BUILTIN_TYPES` entry is findable by its tag
- [ ] Unit test: non-registry tags (`TypeTag::Unit`, `TypeTag::Never`, `TypeTag::Function`) return `None`
- [ ] Unit test: determinism -- calling twice returns same pointer

---

## 08.3 find_method() Function

```rust
/// Look up a method on a builtin type by tag and method name.
///
/// This is the primary query function -- called by the type checker
/// (for return type resolution), the ARC pass (for borrow inference),
/// and the LLVM backend (for codegen dispatch).
///
/// Returns `None` if the type is not in the registry or the method
/// does not exist on that type. Consumers should fall through to
/// trait dispatch or "unknown method" diagnostics.
///
/// # Examples
///
/// ```
/// use ori_registry::{find_method, TypeTag};
///
/// let abs = find_method(TypeTag::Int, "abs").unwrap();
/// assert_eq!(abs.returns, ReturnTag::SelfType);
///
/// // "foo" is not a method on int
/// assert!(find_method(TypeTag::Int, "foo").is_none());
/// ```
pub fn find_method(tag: TypeTag, name: &str) -> Option<&'static MethodDef> {
    let type_def = find_type(tag.base_type())?;
    type_def.methods.iter().find(|m| {
        m.name == name && (tag != TypeTag::Iterator || !m.dei_only)
    })
}
```

### Design decisions

**Not const fn.** String comparison (`m.name == name` where both are `&str`) requires `PartialEq for str` which is not const-stable. This function must be a regular `fn`. This is acceptable -- it is never called in const contexts (always called from runtime type checker / codegen logic).

**Two-step lookup (find type, then scan methods).** This is clearer than a single flat scan of all methods across all types. The two-step approach also naturally short-circuits when the type itself is not in the registry (returns `None` immediately without scanning any methods).

**DEI-aware via `base_type()` + `dei_only` filter.** `TypeTag::DoubleEndedIterator.base_type()` returns `TypeTag::Iterator`, so both tags resolve to the same `TypeDef`. The `dei_only` filter then restricts plain `Iterator` lookups to non-DEI methods, while `DoubleEndedIterator` lookups see all methods. This implements Section 07's single-TypeDef decision — no separate `TypeDef` for DEI exists in `BUILTIN_TYPES`.

**Returns `&'static MethodDef`.** All data is in `static` / `const` storage. The returned reference is valid for the entire program lifetime. This is crucial for Salsa compatibility -- no allocation, no lifetime management, no interior mutability.

### Implementation tasks

- [ ] Implement `find_method()` in `lib.rs`
- [ ] Unit test: known methods return correct MethodDef (check name, returns, receiver fields)
- [ ] Unit test: unknown method name on known type returns `None`
- [ ] Unit test: any method name on unknown type returns `None`
- [ ] Unit test: alias methods work (e.g., both `"length"` and `"len"` on str, if both are registered)

---

## 08.4 Convenience Iterators

Three iterator functions used by diagnostics, enforcement tests, and the ARC pass.

```rust
/// All methods available on a builtin type tag.
///
/// DEI-aware: for `TypeTag::Iterator`, excludes `dei_only` methods.
/// For `TypeTag::DoubleEndedIterator`, includes all methods.
/// Returns an empty iterator for types not in the registry.
/// Used by enforcement tests to verify phase coverage.
///
/// # Examples
///
/// ```
/// use ori_registry::{methods_for, TypeTag};
///
/// let int_methods: Vec<_> = methods_for(TypeTag::Int).collect();
/// assert!(int_methods.iter().any(|m| m.name == "abs"));
/// ```
pub fn methods_for(tag: TypeTag) -> impl Iterator<Item = &'static MethodDef> {
    find_type(tag.base_type())
        .map(|td| td.methods.iter())
        .into_iter()
        .flatten()
        .filter(move |m| tag != TypeTag::Iterator || !m.dei_only)
}

/// All method names defined on a builtin type.
///
/// Used by diagnostics for "did you mean...?" suggestions.
/// Returns an empty iterator for types not in the registry.
pub fn method_names_for(tag: TypeTag) -> impl Iterator<Item = &'static str> {
    methods_for(tag).map(|m| m.name)
}

/// All methods on a builtin type that borrow their receiver.
///
/// Used by the ARC pass to determine which method calls do NOT
/// require an `rc_inc` on the receiver. A method borrows its
/// receiver when `method.receiver == Ownership::Borrow`.
///
/// # Examples
///
/// ```
/// use ori_registry::{borrowing_methods, TypeTag};
///
/// // str.length borrows (reads length without consuming the string)
/// let borrowed: Vec<_> = borrowing_methods(TypeTag::Str).collect();
/// assert!(borrowed.iter().any(|m| m.name == "length"));
/// ```
pub fn borrowing_methods(tag: TypeTag) -> impl Iterator<Item = &'static MethodDef> {
    methods_for(tag).filter(|m| m.receiver == Ownership::Borrow)
}
```

### Additional helper: has_method

```rust
/// Check if a method exists on a builtin type.
///
/// Convenience wrapper around `find_method(...).is_some()`.
/// Useful in boolean contexts where the full MethodDef is not needed.
#[inline]
pub fn has_method(tag: TypeTag, name: &str) -> bool {
    find_method(tag, name).is_some()
}
```

### Design decisions

**`methods_for` returns `impl Iterator`, not `&[MethodDef]`.** The caller cannot assume the internal storage layout. Today it is a flat slice on `TypeDef.methods`; in the future it could be segmented (e.g., inherent methods + trait methods). The iterator abstraction hides this.

**No `all_borrowing_method_names()` global function.** The current `ori_ir::builtin_methods::borrowing_method_names()` iterates ALL types and yields deduplicated borrowing method names. This is used by `ori_arc` to build a `FxHashSet<Name>`. In the new design, `ori_arc` should query per-type (`borrowing_methods(tag)`) rather than building a global set. If a global set is still needed during migration, it can be constructed by the consumer:

```rust
// In ori_arc, during migration:
let borrowing_set: FxHashSet<Name> = BUILTIN_TYPES
    .iter()
    .flat_map(|td| borrowing_methods(td.tag))
    .map(|m| interner.intern(m.name))
    .collect();
```

This keeps the registry free of `Name` (which lives in `ori_ir`) and free of `FxHashSet` (which is a runtime dependency).

### Implementation tasks

- [ ] Implement `methods_for()` in `lib.rs`
- [ ] Implement `method_names_for()` in `lib.rs`
- [ ] Implement `borrowing_methods()` in `lib.rs`
- [ ] Implement `has_method()` in `lib.rs`
- [ ] Unit test: `methods_for` on known type returns non-empty iterator
- [ ] Unit test: `methods_for` on unknown type returns empty iterator
- [ ] Unit test: `method_names_for` returns all expected names for a type (spot-check int, str)
- [ ] Unit test: `borrowing_methods` returns only methods with `Ownership::Borrow`
- [ ] Unit test: `borrowing_methods` count < total `methods_for` count (some methods are owned)
- [ ] Unit test: `has_method` agrees with `find_method(...).is_some()` for several cases

---

## 08.5 Operator Query Strategy

### The dependency problem

The natural signature would be:

```rust
pub fn operator_strategy(tag: TypeTag, op: BinOp) -> OpStrategy
```

But `BinOp` is defined in `ori_ir`, and `ori_registry` has **zero dependencies**. Three options:

| Option | Approach | Pros | Cons |
|--------|----------|------|------|
| **(a)** ori_registry defines its own `RegistryBinOp` enum | Clean API, self-contained | Duplicate enum, consumers must map `BinOp` -> `RegistryBinOp` |
| **(b)** Consumers map their `BinOp` to OpDefs fields themselves | No duplicate enum, registry stays minimal | Every consumer writes its own mapping boilerplate |
| **(c)** No wrapper -- consumers access `type_def.operators.add` (etc.) directly | Simplest, no indirection, no mapping | Slightly more verbose at call sites |

### Decision: Option (c) -- direct field access

Consumers access `OpDefs` fields directly:

```rust
// In ori_llvm emit_binary_op:
let type_def = find_type(tag)?;
let strategy = match op {
    BinOp::Add => type_def.operators.add,
    BinOp::Sub => type_def.operators.sub,
    BinOp::Mul => type_def.operators.mul,
    BinOp::Div => type_def.operators.div,
    BinOp::Mod => type_def.operators.rem,
    BinOp::FloorDiv => type_def.operators.floor_div,
    BinOp::Eq  => type_def.operators.eq,
    BinOp::NotEq => type_def.operators.neq,
    BinOp::Lt  => type_def.operators.lt,
    BinOp::Gt  => type_def.operators.gt,
    BinOp::LtEq => type_def.operators.lt_eq,
    BinOp::GtEq => type_def.operators.gt_eq,
    // ...
};
```

**Rationale:**
1. The match is exactly what consumers already write today (with `is_str` guards instead of strategy lookup). Replacing the guard-based dispatch with strategy-based dispatch is the point of the registry, and the match itself is the consumer's responsibility.
2. Option (a) adds a parallel enum that must stay in sync with `BinOp` -- the exact kind of drift this project eliminates.
3. Option (b) and (c) are effectively the same; (c) is just the explicit form.
4. The match in the consumer is exhaustive on `BinOp`, so adding a new operator to `BinOp` forces the consumer to handle it (Rust exhaustiveness). Adding a new field to `OpDefs` forces the consumer to handle it (struct field required). Both directions are compile-time enforced.

### No `operator_strategy()` function in ori_registry

The `OpDefs` struct (from Section 01) is public, its fields are public, and consumers access them directly. No wrapper function is needed or desired.

### Implementation tasks

- [ ] Ensure `OpDefs` and all its fields are `pub` (verified in Section 01)
- [ ] Add doc comment on `OpDefs` explaining that consumers match their own `BinOp` to fields
- [ ] Add enforcement test: every `OpDefs` field that is not `OpStrategy::Unsupported` has a handler in ori_llvm (this test lives in Section 14, but the API shape decision is made here)

---

## 08.6 Type Lookup by Name

```rust
/// Look up a builtin type definition by its string name.
///
/// Matches the `TypeDef.name` field: lowercase for primitives
/// (`"int"`, `"str"`, `"bool"`), proper-case for special types
/// (`"Duration"`, `"Ordering"`, `"Iterator"`).
///
/// Used by consistency tests and diagnostics (mapping a string
/// type name to its full definition).
///
/// # Examples
///
/// ```
/// use ori_registry::find_type_by_name;
///
/// let str_def = find_type_by_name("str").unwrap();
/// assert!(str_def.methods.len() > 5);
///
/// assert!(find_type_by_name("FooBar").is_none());
/// ```
pub fn find_type_by_name(name: &str) -> Option<&'static TypeDef> {
    BUILTIN_TYPES.iter().find(|td| td.name == name).copied()
}
```

### Design decisions

**Not const fn.** Same reason as `find_method()` -- string comparison is not const-stable.

**Case-sensitive.** The `TypeDef.name` field stores the canonical name as it appears in Ori source code. `"int"` matches, `"Int"` does not. This is correct because Ori is case-sensitive and the canonical names are defined by the type definitions in Sections 03-07.

**Linear scan.** Same justification as `find_type()` -- the array is tiny. A `phf` map keyed by name would be a premature optimization.

### Implementation tasks

- [ ] Implement `find_type_by_name()` in `lib.rs`
- [ ] Unit test: every `BUILTIN_TYPES` entry is findable by its `.name` field
- [ ] Unit test: unknown name returns `None`
- [ ] Unit test: case sensitivity -- `"Int"` does not find `int` (whose name is `"int"`)

---

## 08.7 Performance Considerations

### Current scale

| Metric | Count | Source |
|--------|-------|--------|
| Registered types | 6-12 (primitives) + 8-10 (containers) = ~14-22 | Sections 03-07 |
| Methods per type | 3-25 | Varies: bool has ~3, str has ~20, Iterator has ~25 |
| Total methods | ~150-250 | Sum across all types |
| Calls per compilation | Hundreds to low thousands | Once per method call site in user code |

### Why linear scan is correct

All query functions use linear scan over `BUILTIN_TYPES` (find_type: scan types) or `TypeDef.methods` (find_method: scan methods within one type). The analysis:

**find_type() -- scan of ~14-22 `&TypeDef` references:**
- Each comparison is a single `u8 == u8` (TypeTag discriminant)
- The entire array fits in a single cache line (22 pointers = 176 bytes on 64-bit)
- Worst case: ~22 comparisons = ~5 ns (branch-predicted, data cache-resident)
- Called O(method_call_sites) per compilation -- say 500 calls = ~2.5 us total

**find_method() -- scan of ~3-25 `MethodDef` entries after find_type:**
- Each comparison is `strcmp` on short strings (method names are 2-15 chars)
- `MethodDef` entries are contiguous in .rodata (cache-line friendly)
- Worst case: 25 comparisons on ~8-char strings = ~50 ns
- Called O(method_call_sites) per compilation -- say 500 calls = ~25 us total

For context, a single Salsa query re-execution costs 100-500 ns. The registry lookups are noise.

### Future optimization options (NOT planned for initial implementation)

If profiling ever shows registry lookups as a bottleneck (extremely unlikely at this scale), these options exist in order of increasing complexity:

1. **Sorted arrays + binary search.** Since `BUILTIN_TYPES` is sorted by TypeTag discriminant, `find_type()` could use binary search. Saves ~3 comparisons on a 20-element array. Not worth the code complexity.

2. **Direct indexing by TypeTag.** Since TypeTag is `#[repr(u8)]`, a 256-element array indexed by `tag as u8` gives O(1) lookup. Wastes ~234 entries (all `None`). Viable if we want guaranteed O(1) but the sparse array is ugly.

3. **`phf` (perfect hash function) at build time.** The `phf` crate generates compile-time perfect hash maps. Would give O(1) lookup with minimal space. Adds a build dependency. Overkill for <25 entries.

4. **const HashMap (future Rust).** When Rust stabilizes `const` HashMap construction, the simplest O(1) solution becomes available. Track [rust-lang/rust#102575](https://github.com/rust-lang/rust/issues/102575).

### Decision

**Ship the simplest implementation (linear scan). Optimize only if profiling shows need.** The entire query API is ~60 lines of trivial iteration. It is correct, deterministic, const-friendly (find_type), allocation-free, and measurably fast enough by multiple orders of magnitude. Premature optimization here would add complexity with zero user-visible benefit.

### Cache friendliness

All registry data lives in the binary's `.rodata` segment:
- `TypeDef` constants: contiguous, read-only, memory-mapped
- `MethodDef` arrays: contiguous within each TypeDef, read-only
- No heap allocation, no `LazyLock`, no initialization order dependency
- First access page-faults the data in; subsequent accesses are L1-cache resident

This is strictly better than the current `ori_ir::BUILTIN_METHODS` (a `&'static [MethodDef]` flat array) because the data is structured by type, improving spatial locality for per-type queries.

---

## 08.8 API Documentation

Every public item in `ori_registry` must have documentation. The query API is the crate's primary user-facing surface.

### Module-level documentation (`lib.rs`)

```rust
//! Pure-data registry of builtin type behavior for the Ori compiler.
//!
//! This crate is the single source of truth for every builtin type's
//! methods, operators, ownership semantics, and memory strategy. It has
//! **zero dependencies** and contains **no logic** -- only `const` data
//! definitions and simple lookup functions.
//!
//! # Query Model
//!
//! All queries follow the pattern: tag -> TypeDef -> field/method.
//!
//! ```
//! use ori_registry::{find_type, find_method, TypeTag, Ownership};
//!
//! // Look up a type
//! let str_def = find_type(TypeTag::Str).unwrap();
//! assert_eq!(str_def.name, "str");
//!
//! // Look up a method
//! let length = find_method(TypeTag::Str, "length").unwrap();
//! assert_eq!(length.receiver, Ownership::Borrow);
//!
//! // Operator strategies are accessed via TypeDef.operators
//! let add_strategy = str_def.operators.add;
//! ```
//!
//! # Design Invariants
//!
//! 1. **Zero dependencies** -- Cargo.toml has no `[dependencies]`
//! 2. **No logic** -- only `const fn` constructors and linear-scan lookups
//! 3. **All data `const`** -- baked into `.rodata`, no heap allocation
//! 4. **Deterministic** -- safe for Salsa memoization
//! 5. **Structural enforcement** -- adding a field to `TypeDef` is a
//!    compile error in every consuming phase that doesn't handle it
```

### Per-function documentation requirements

Every public function must include:
1. A one-line summary (imperative mood: "Look up...", "Return...", "Check...")
2. A `# Examples` section with a working code block
3. Documentation of `None` / empty-iterator return conditions
4. Cross-references to consuming phases where relevant

### Documentation for TypeDef and MethodDef

The data model types (Section 01) carry their own docs. The query API docs should cross-reference them but not duplicate their field-level documentation. Example:

```rust
/// Look up a method on a builtin type by tag and method name.
///
/// Returns the [`MethodDef`] if found, which contains:
/// - `name`: the method name (same as the `name` argument)
/// - `receiver`: [`Ownership::Borrow`] or [`Ownership::Owned`]
/// - `returns`: the return type specification
/// - `params`: parameter type specifications
/// - `trait_name`: the associated trait, if any
///
/// See [`MethodDef`] for full field documentation.
```

### Implementation tasks

- [ ] Write module-level `//!` doc for `lib.rs` (the crate root)
- [ ] Write `///` doc for every public function (find_type, find_method, methods_for, method_names_for, borrowing_methods, has_method, find_type_by_name)
- [ ] Write `///` doc for `BUILTIN_TYPES` constant
- [ ] Include `# Examples` with working code blocks in every public function doc
- [ ] Run `cargo doc -p ori_registry --no-deps` and verify all docs render correctly
- [ ] Verify no `missing_docs` warnings (consider `#![warn(missing_docs)]` in lib.rs)

---

## Complete Public API Summary

The full public surface of ori_registry's query layer:

```rust
// === Master constant ===
pub const BUILTIN_TYPES: &[&TypeDef];

// === Primary lookups ===
pub const fn find_type(tag: TypeTag) -> Option<&'static TypeDef>;
pub fn find_method(tag: TypeTag, name: &str) -> Option<&'static MethodDef>;
pub fn find_type_by_name(name: &str) -> Option<&'static TypeDef>;

// === Convenience iterators ===
pub fn methods_for(tag: TypeTag) -> impl Iterator<Item = &'static MethodDef>;
pub fn method_names_for(tag: TypeTag) -> impl Iterator<Item = &'static str>;
pub fn borrowing_methods(tag: TypeTag) -> impl Iterator<Item = &'static MethodDef>;

// === Predicates ===
pub fn has_method(tag: TypeTag, name: &str) -> bool;

// === Operator queries: direct field access ===
// No wrapper function. Consumers access TypeDef.operators.{add, sub, mul, ...} directly.
```

**Not included (and why):**
- `operator_strategy(tag, op)` -- would require `BinOp` dependency; consumers access `OpDefs` fields directly (08.5)
- `all_borrowing_method_names()` -- global deduplication belongs in the consumer, not the registry (08.4)
- `find_method_across_types(name)` -- no use case; methods are always resolved on a known receiver type
- Any function returning `Vec`, `HashMap`, or other allocated types -- the registry is allocation-free

---

## Completion Checklist

- [ ] `BUILTIN_TYPES` constant defined and populated (from Sections 03-07 definitions)
- [ ] `find_type()` implemented as `const fn`
- [ ] `find_method()` implemented
- [ ] `find_type_by_name()` implemented
- [ ] `methods_for()`, `method_names_for()`, `borrowing_methods()` implemented
- [ ] `has_method()` implemented
- [ ] All functions have `///` doc comments with `# Examples`
- [ ] Module-level `//!` documentation written
- [ ] `#![warn(missing_docs)]` enabled and clean
- [ ] `cargo doc -p ori_registry --no-deps` renders without warnings
- [ ] Unit tests for all functions (happy path + None/empty cases)
- [ ] Enforcement test: every `BUILTIN_TYPES` entry findable by tag AND by name
- [ ] Enforcement test: no duplicate tags in `BUILTIN_TYPES`
- [ ] Enforcement test: `BUILTIN_TYPES` is sorted by TypeTag discriminant
- [ ] `cargo c -p ori_registry` passes
- [ ] `cargo t -p ori_registry` passes (all query tests green)
- [ ] No `[dependencies]` in Cargo.toml (purity preserved)

**Exit Criteria:** All consuming phases (ori_types, ori_eval, ori_arc, ori_llvm) can compile with only `ori_registry` as their dependency for type queries. Every query function is documented, tested, and deterministic. The API is sufficient to replace `ori_ir::builtin_methods::find_method()`, `methods_for()`, `method_names_for()`, `borrowing_method_names()`, `has_method()`, and `method_borrows_receiver()` without loss of functionality.
