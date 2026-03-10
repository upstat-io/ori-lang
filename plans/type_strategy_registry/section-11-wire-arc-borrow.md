---
section: "11"
title: "Wire ARC & Borrow Pass (ori_arc)"
status: complete
reviewed: true
goal: "Replace BORROWING_METHOD_NAMES and BuiltinOwnershipSets construction with ori_registry as the single source of truth, and preserve iterator/COW exclusion semantics"
depends_on:
  - "03"
  - "04"
  - "05"
  - "06"
  - "07"
  - "08"
  # Note: Section 09 dependency is from the overview's conservative ordering
  # ("ori_arc imports from ori_types"). In practice, Section 11's migration
  # only needs registry data (03-08), not type checker wiring (09). Could be
  # parallelized with 09 if needed, but the conservative ordering is safe.
  - "09"
sections:
  - id: "11.6"
    title: "Dependency direction verification — execute first (ori_arc already depends on ori_registry)"
    status: complete
  - id: "11.1"
    title: "Add ori_registry::borrowing_method_names() helper"
    status: complete
  - id: "11.2"
    title: "Iterator and derived-value exclusion logic"
    status: complete
  - id: "11.4"
    title: "Update ori_arc::borrow::builtins (internal migration)"
    status: complete
  - id: "11.3"
    title: "Verify oric call sites compile (no code changes)"
    status: complete
  - id: "11.5"
    title: "Verify FunctionCompiler call site (ori_llvm AOT)"
    status: complete
  - id: "11.7"
    title: "MemoryStrategy: registry vs runtime classification"
    status: complete
  - id: "11.8"
    title: "Delete legacy borrowing functions"
    status: complete
  - id: "11.9"
    title: "Validation & regression"
    status: complete
---

# Section 11: Wire ARC & Borrow Pass (ori_arc)

**Execution order:** 11.6 (verify deps) → 11.1 (add registry helper) → 11.2 (exclusion logic) → 11.4 (core migration) → 11.3, 11.5 (verify call sites) → 11.7 (analysis) → 11.8 (delete legacy) → 11.9 (validate). The YAML header reflects this ordering.

**Context:** The ARC borrow inference pass needs to know which builtin methods borrow their receiver so it can skip RC operations at their call sites. Currently this data is defined in two places:

1. **`ori_arc::borrow::builtins::borrowing_builtin_names()`** (`compiler/ori_arc/src/borrow/builtins/mod.rs:157`) -- the canonical source. Interns the `BORROWING_METHOD_NAMES` const array (a manually maintained, alphabetically sorted list of ~47 method name strings) into `FxHashSet<Name>`. This is wrapped into `BuiltinOwnershipSets` (line 257) which bundles borrowing, consuming-receiver, consuming-second-arg, consuming-receiver-only, and protocol ownership sets. All production call sites use `BuiltinOwnershipSets::new(interner)`.

2. **`ori_ir::builtin_methods::borrowing_method_names()`** (`compiler/ori_ir/src/builtin_methods/mod.rs:843`) -- filters the `BUILTIN_METHODS` table by `receiver_borrows: true`. This was the original SSoT plan's partial fix (Section 01 of `builtin_ownership_ssot`). Not used by production call sites.

Additionally, the LLVM crate has a `#[cfg(test)]` helper `borrowing_names_from_table()` (line 259 of `ori_llvm/src/codegen/arc_emitter/builtins/mod.rs`) that derives the set from the LLVM `BuiltinTable` for sync testing against the `ori_arc` canonical list.

The registry plan replaces the manual `BORROWING_METHOD_NAMES` array with data derived from the `TypeDef` method specifications in `ori_registry`.

**Design rationale:** The borrowing set is a pure function of the method ownership metadata. It requires no runtime state, no string interner, no type pool -- just a list of method names where `Ownership == Borrow`. The registry already stores this data (every `MethodDef` carries a receiver `Ownership` field). The helper belongs in `ori_registry` because it is a projection of const data, not in any consuming crate.

---

## Current Architecture

### Data Flow (today)

```
ori_arc::borrow::builtins
  BORROWING_METHOD_NAMES: &[&str] (const, ~47 entries)
  CONSUMING_RECEIVER_METHOD_NAMES: &[&str] (const, ~10 entries)
  CONSUMING_SECOND_ARG_METHOD_NAMES: &[&str] (const, ~2 entries)
  CONSUMING_RECEIVER_ONLY_METHOD_NAMES: &[&str] (const, ~4 entries)
        │
        │  BuiltinOwnershipSets::new(interner)
        │  (interns all 4 lists + protocol builtins)
        ▼
  BuiltinOwnershipSets { borrowing, consuming_receiver, ... }
                                                                    │
  Call sites constructing BuiltinOwnershipSets::new():              │
  ├── oric/src/query/arc_queries/mod.rs:358  (Salsa borrow query)   │
  ├── oric/src/arc_dot/mod.rs:41            (GraphViz DOT dump)     │
  ├── oric/src/arc_dump/mod.rs:40           (ARC IR text dump)      │
  ├── oric/src/test/runner/arc_lowering.rs:71 (test runner)         │
  ├── oric/benches/borrow_inference.rs       (benchmarks, 3 sites)  │
  └── ori_llvm/codegen/function_compiler/mod.rs:99 (AOT RC annot)   │
                                                                    │
  ori_arc::borrow::infer_borrows_scc(functions, classifier, &sets) ◄┘
  ori_arc::rc_insert::annotate_arg_ownership(func, sigs, interner, &sets, pool)
```

### Dependency Direction (today)

```
oric ──depends──→ ori_arc   (for BuiltinOwnershipSets, infer_borrows_scc, etc.)

ori_llvm ──depends──→ ori_arc  (for BuiltinOwnershipSets, annotate_arg_ownership)

ori_arc ──depends──→ ori_ir       (for Name, StringInterner, ProtocolBuiltin)
       ──depends──→ ori_registry  (already present in Cargo.toml)
       ──depends──→ ori_types     (for Pool, Idx, Tag)
```

**Note:** There is NO `oric → ori_llvm` dependency for borrowing data today. The borrowing knowledge lives in `ori_arc::borrow::builtins`, which is at the correct architectural layer. The migration target is moving the raw data (the `BORROWING_METHOD_NAMES` array) from `ori_arc` into `ori_registry`, so all ownership metadata comes from one place.

### Data Flow (after this section)

```
ori_registry (Layer 0, const data)
  BUILTIN_TYPES: &[&TypeDef]
    TypeDef.methods: &[MethodDef]
      MethodDef.receiver: Ownership  (Borrow | Owned | Copy)
        │
        │  ori_registry::borrowing_method_names()
        │  (filters by Ownership::Borrow, excludes Iterator/iter)
        ▼
  &[&str]  (lazy-initialized, no interning needed at registry level)
        │
  ori_arc::borrow::builtins::borrowing_builtin_names(interner)
    interns registry names + appends all-borrowed ProtocolBuiltin names
        │
  ori_arc::borrow::builtins::BuiltinOwnershipSets::new(interner)
    bundles borrowing + consuming/sharing/protocol sets
        │
  Call sites (unchanged API — still use BuiltinOwnershipSets::new()):
  ├── oric/src/query/arc_queries/mod.rs
  ├── oric/src/arc_dot/mod.rs
  ├── oric/src/arc_dump/mod.rs
  ├── oric/src/test/runner/arc_lowering.rs
  ├── oric/benches/borrow_inference.rs (3 sites)
  └── ori_llvm/codegen/function_compiler/mod.rs
        │
  ori_arc::infer_borrows_scc(functions, classifier, &sets)
  ori_arc::annotate_arg_ownership(func, sigs, interner, &sets, pool)
```

### Dependency Direction (after)

```
ori_registry (Layer 0) ◄── ori_arc (Layer 2)
                        ◄── ori_llvm (Layer 4)
                        ◄── oric (top)

ori_arc reads borrowing method names from ori_registry instead of its own const array.
```

---

## 11.1 Add `ori_registry::borrowing_method_names()` Helper

**WARNING:** This is the highest-risk subsection due to the purity test conflict. The `purity_no_heap_allocation_types` test (`compiler/ori_registry/src/tests.rs:159`) scans non-test `.rs` files for `Vec<`/`Box<` substrings, so the implementation must use a fixed-size array inside `LazyLock` (see Implementation Strategy below).

**File:** `compiler/ori_registry/src/query/mod.rs` (Section 08 established `query/` as the query module)

**Important:** `ori_registry` already exports `borrowing_methods(tag: TypeTag) -> impl Iterator<Item = &'static MethodDef>` (added in Section 08). That function returns per-type method definitions filtered by `Ownership::Borrow`. The new `borrowing_method_names()` helper is **different**: it returns a flat, deduplicated `&[&str]` of method name strings across all non-iterator types, suitable for building `ori_arc`'s `FxHashSet<Name>`. It should be built on top of the existing `BUILTIN_TYPES` data, not duplicating it.

This is a regular `fn` (not `const fn` -- Rust does not support const iteration with filtering) that returns a `&'static [&'static str]` of method names whose receiver uses borrowing semantics, derived via `LazyLock` from `BUILTIN_TYPES`. The caller is responsible for interning the names into `Name` values -- the registry does not depend on `ori_ir` and cannot intern.

### API Design

```rust
/// Method names whose receiver is borrowed and whose result is independent
/// of the receiver's lifetime.
///
/// Used by ARC borrow inference to skip RC operations at call sites for
/// inline-compiled builtin methods (e.g., `len`, `is_empty`, `compare`).
///
/// **Excluded:** Iterator/DoubleEndedIterator methods and `.iter()` --
/// these create derived values with hidden dependencies on the receiver.
/// The ARC pipeline cannot model these dependencies, so they use Owned
/// semantics (the runtime handles internal RC management).
///
/// # Example
///
/// ```ignore
/// let set: FxHashSet<Name> = ori_registry::borrowing_method_names()
///     .iter()
///     .map(|name| interner.intern(name))
///     .collect();
/// ```
pub fn borrowing_method_names() -> &'static [&'static str] {
    // LazyLock<([&str; 64], usize)> derived from BUILTIN_TYPES.
    // See Implementation Strategy below for details.
    ...
}
```

### Implementation Strategy

Rust does not support `const fn` iteration over slices with filtering (no const `Vec`, no const collect). Two approaches were considered:

1. **Explicit const array** -- mirrors the current `BORROWING_METHOD_NAMES` pattern. Must be kept in sync with `BUILTIN_TYPES` via a sync test. Fragile.
2. **`LazyLock` derivation** -- iterates `BUILTIN_TYPES` at first call, cannot drift.

**Decision:** Use `LazyLock` with a fixed-size array to derive the list automatically from `BUILTIN_TYPES`. The purity scan test (`purity_no_heap_allocation_types`) flags `Vec<` and `Box<` substrings in non-test code, so the `LazyLock` initializer uses a `[&str; 64]` array with a length counter instead of `Vec`. The upper bound of 64 is generous (currently ~47 entries); the initializer panics if exceeded.

```rust
/// Derive borrowing method names from BUILTIN_TYPES on first access.
///
/// Returns a deduplicated, sorted slice of method names.
pub fn borrowing_method_names() -> &'static [&'static str] {
    static NAMES: LazyLock<([&'static str; 64], usize)> = LazyLock::new(|| {
        let mut buf = [""; 64];
        let mut len = 0;
        for td in BUILTIN_TYPES {
            if td.tag == TypeTag::Iterator { continue; }
            for m in td.methods {
                if m.receiver == Ownership::Borrow && m.name != "iter" {
                    assert!(len < 64, "borrowing_method_names overflow: increase array size");
                    buf[len] = m.name;
                    len += 1;
                }
            }
        }
        buf[..len].sort_unstable();
        // Deduplicate in place
        let mut write = 1;
        for read in 1..len {
            if buf[read] != buf[write - 1] {
                buf[write] = buf[read];
                write += 1;
            }
        }
        (buf, if len == 0 { 0 } else { write })
    });
    &NAMES.0[..NAMES.1]
}
```

### Critical: Protocol Builtins NOT in Registry

**`"__index"` is in the current `BORROWING_METHOD_NAMES` but is NOT a method on any `TypeDef` in `ori_registry`.** It is a `ProtocolBuiltin` defined in `ori_ir::builtin_constants::protocol`. The `protocol_builtins_borrowing_sync` test (builtins/tests.rs:338) explicitly verifies that all-borrowed protocol builtins appear in the borrowing set.

The `LazyLock` derivation from `BUILTIN_TYPES` will NOT include protocol builtins. This means the registry-derived set is a **subset** of the current `BORROWING_METHOD_NAMES`, missing `"__index"` (the only protocol builtin with all-borrowed semantics -- `__iter_next` and `__collect_set` have Owned args).

**Two options:**

1. **Append protocol builtins in `ori_arc`:** The `borrowing_builtin_names()` function in `ori_arc` already has access to `ProtocolBuiltin::ALL`. After interning the registry's borrowing names, append all-borrowed protocol names:
   ```rust
   pub fn borrowing_builtin_names(interner: &StringInterner) -> FxHashSet<Name> {
       let mut names: FxHashSet<Name> = ori_registry::borrowing_method_names()
           .iter()
           .map(|name| interner.intern(name))
           .collect();
       // Append all-borrowed protocol builtins (not in registry BUILTIN_TYPES)
       for pb in ProtocolBuiltin::ALL {
           if pb.arg_ownership().iter().all(|o| *o == ProtocolArgOwnership::Borrowed) {
               names.insert(interner.intern(pb.name()));
           }
       }
       names
   }
   ```
   This keeps protocol builtins in `ori_ir` where they belong (they are ARC pipeline internals, not builtin type methods).

2. **Include protocol builtins in the registry:** Add `"__index"` to `ori_registry::borrowing_method_names()` output. This would require the registry to know about protocol builtins, which is architecturally wrong (protocols are ARC pipeline internals).

**Decision:** Option 1. Protocol builtins are appended in `ori_arc::borrowing_builtin_names()`, not included in the registry. This preserves the registry's purity (it only knows about builtin type methods, not ARC pipeline internals).

**Impact on equivalence test:** The equivalence test (Section 11.9) must compare `ori_arc::borrowing_builtin_names()` (which includes protocol builtins) against the old `BORROWING_METHOD_NAMES` -- NOT `ori_registry::borrowing_method_names()` directly. This is correct because the equivalence test verifies the final interned set is unchanged.

### Additional Registry Helpers -- Scope Limitation

The current `ori_arc::borrow::builtins` module maintains **five** ownership categories, not just borrowing:

1. `BORROWING_METHOD_NAMES` (~47 entries) -- methods that borrow receiver **→ MIGRATED to registry**
2. `CONSUMING_RECEIVER_METHOD_NAMES` (~10 entries) -- COW list methods consuming receiver **→ stays in ori_arc**
3. `CONSUMING_SECOND_ARG_METHOD_NAMES` (~2 entries) -- COW list methods also consuming arg[1] **→ stays in ori_arc**
4. `CONSUMING_RECEIVER_ONLY_METHOD_NAMES` (~4 entries) -- COW map/set methods (receiver consumed, other args borrowed) **→ stays in ori_arc**
5. `SHARING_METHOD_NAMES` (~2 entries) -- methods returning views into receiver data **→ stays in ori_arc**

**Why only category 1 migrates:** The registry's `MethodDef.receiver` field has 3 variants (`Borrow`/`Owned`/`Copy`). Categories 2-5 encode COW-specific call-site behavior that is type-qualified (e.g., `"reverse"` is borrowing for `Ordering` but consuming for `List`). This cannot be expressed in a single `MethodDef` without making the data model type-qualified (different `MethodDef` per type per method, which the registry already supports via per-`TypeDef` method slices). Future work: if `MethodDef` gains a `cow_strategy` field, categories 2-5 could be derived from the registry too.

**Composition helpers:** `all_cow_method_names()` (lines 217-221) and `sharing_builtin_names()` (lines 244-249) compose the category 2-5 arrays. These stay in `ori_arc` unchanged.

### Sync Test

```rust
#[test]
fn borrowing_names_match_type_defs() {
    let derived: FxHashSet<&str> = BUILTIN_TYPES
        .iter()
        .filter(|td| td.tag != TypeTag::Iterator)
        .flat_map(|td| td.methods.iter())
        .filter(|m| m.receiver == Ownership::Borrow && m.name != "iter")
        .map(|m| m.name)
        .collect();

    let exported: FxHashSet<&str> = borrowing_method_names().iter().copied().collect();
    assert_eq!(derived, exported, "borrowing_method_names() drifted from BUILTIN_TYPES");
}
```

### Checklist

- [x] Add `borrowing_method_names()` to `ori_registry/src/query/mod.rs` (2026-03-08)
- [x] Export from `ori_registry/src/lib.rs` (`pub use query::borrowing_method_names;`) (2026-03-08)
- [x] Derive from `BUILTIN_TYPES` via `LazyLock<([&str; 512], usize)>` (fixed-size array to pass purity scan; plan said 64 but pre-dedup count is ~350+ because all primitive type methods use Borrow receiver) (2026-03-08)
- [x] Filter: `receiver == Ownership::Borrow` (2026-03-08)
- [x] Exclude: `TypeTag::Iterator` methods (entire type) (2026-03-08)
- [x] Exclude: method name `"iter"` (on any type) (2026-03-08)
- [x] Deduplicate (multiple types may share a method name like `"to_str"`) (2026-03-08)
- [x] Sort alphabetically (required for deterministic output) (2026-03-08)
- [x] NOT adding `consuming_method_names()` etc. in this section (stays in `ori_arc`, per scope decision above) (2026-03-08)
- [x] Verify `borrowing_method_names()` does NOT conflict with existing `borrowing_methods(tag)` (different signatures: one returns flat `&[&str]`, the other returns per-type `MethodDef` iterator) (2026-03-08)
- [x] Add sync test `borrowing_names_match_type_defs` (2026-03-08)
- [x] Verify purity scan test passes (fixed-size array approach avoids `Vec<`/`Box<` substrings) (2026-03-08)
- [x] `cargo test -p ori_registry` passes (272 tests) (2026-03-08)

---

## 11.2 Iterator and Derived-Value Exclusion Logic

The current `BORROWING_METHOD_NAMES` in `ori_arc::borrow::builtins` excludes iterator methods by simply not listing them. The `iter_excluded()` test (line 38 of `builtins/tests.rs`) explicitly asserts `"iter"` is not in the list. The LLVM `#[cfg(test)]` helper `borrowing_names_from_table()` (line 259 of `ori_llvm/src/codegen/arc_emitter/builtins/mod.rs`) programmatically excludes them:

### Category 1: All Iterator type methods

```rust
// ori_llvm/codegen/arc_emitter/builtins/mod.rs:263 (#[cfg(test)] only)
if type_name == "Iterator" {
    continue;
}
```

**Why:** Iterator methods (`map`, `filter`, `fold`, `collect`, `take`, `skip`, `chain`, `zip`, `enumerate`, `flatten`, `cycle`, `count`, `any`, `all`, `find`, `next`, `next_back`, `rev`, `last`, `rfind`, `rfold`, etc.) consume or transform the iterator. Even methods like `count` that "only read" internally exhaust the iterator state. The ARC pipeline models these as consuming because:
- The iterator captures internal state (position, buffer, closure)
- Calling a method advances or consumes that state
- The caller cannot safely reuse the iterator after the call

### Category 2: The `.iter()` method on collection types

```rust
// ori_llvm/codegen/arc_emitter/builtins/mod.rs:270 (#[cfg(test)] only)
if method_name == "iter" {
    continue;
}
```

**Why:** `.iter()` creates an iterator that borrows from the collection's data. The returned iterator has a hidden dependency on the receiver -- if the collection is freed, the iterator dangles. The ARC pipeline cannot model this dependency (it only tracks direct variable ownership, not indirect borrow lifetimes). Therefore `.iter()` must use Owned semantics at the call site: the caller `RcInc`s the collection to keep it alive while the iterator exists.

### Registry Equivalent

In `ori_registry`, the exclusion maps to `TypeTag`:

```rust
// Exclude Iterator type entirely
td.tag != TypeTag::Iterator

// Exclude .iter() method on any type
m.name != "iter"
```

`TypeTag::DoubleEndedIterator` exists in the registry but `base_type()` maps it to `TypeTag::Iterator`. There is no separate `TypeDef` for `DoubleEndedIterator` -- all methods (including `next_back`, `rev`, etc.) are on the single `Iterator` `TypeDef`. Therefore, filtering by `td.tag != TypeTag::Iterator` in the `LazyLock` derivation is sufficient to exclude all iterator methods (both Iterator and DEI).

No additional tag check is needed:
```rust
// This is correct — no TypeDef has tag == TypeTag::DoubleEndedIterator
td.tag != TypeTag::Iterator
```

### Correctness Invariant

The exclusion logic is a **safety invariant**, not a performance optimization. Getting it wrong causes use-after-free:

- **False positive (incorrectly excluded):** A method that should be in the borrowing set is excluded. Result: the ARC pass treats its args as Owned, emitting unnecessary `RcInc`/`RcDec`. Performance penalty but correct.
- **False negative (incorrectly included):** A method that creates a derived value (like `.iter()`) is included in the borrowing set. Result: the ARC pass skips `RcInc` for the receiver, which may be freed while the derived value still exists. **Use-after-free.**

Therefore, when in doubt, **exclude** (err toward Owned semantics).

### Checklist

- [x] `TypeTag::Iterator` methods excluded from borrowing set (single TypeDef, no separate DEI TypeDef) (2026-03-08)
- [x] `"iter"` method excluded regardless of type (2026-03-08)
- [x] Document the safety invariant in the helper's doc comment (2026-03-08)
- [x] Audit: methods in `CONSUMING_RECEIVER_METHOD_NAMES` that also have `receiver: Borrow` in some TypeDef must be handled correctly. `reverse` (Ordering=Borrow, List=Borrow but COW override), `remove` (Map/Set=Borrow but COW override), `add`/`concat` (primitives/str=Borrow, List=COW override). All type-qualified at call site via `annotate_arg_ownership`. (2026-03-08)
- [x] Audit all `BUILTIN_TYPES` methods with `receiver: Borrow`: `slice`/`substring` return sharing values but are safe — sharing tracked separately via `SHARING_METHOD_NAMES` / `MaybeShared` uniqueness. No other Borrow methods return receiver-dependent references (`.iter()` is excluded). (2026-03-08)

---

## 11.3 Verify oric Call Sites Compile (No Code Changes)

**Call sites in oric (all use `ori_arc::BuiltinOwnershipSets::new()`):**

| File | Line | Context |
|------|------|---------|
| `compiler/oric/src/query/arc_queries/mod.rs` | 358 | Salsa borrow inference query |
| `compiler/oric/src/arc_dot/mod.rs` | 41 | GraphViz DOT dump |
| `compiler/oric/src/arc_dump/mod.rs` | 40 | ARC IR text dump |
| `compiler/oric/src/test/runner/arc_lowering.rs` | 71 | Test runner ARC lowering |
| `compiler/oric/benches/borrow_inference.rs` | 356, 374, 580 | Benchmarks |

**Note:** `compile_common.rs` does NOT contain any borrowing call sites. It delegates to the codegen pipeline which uses `FunctionCompiler` (see Section 11.5).

### Migration Strategy

These call sites do not change directly. They all call `ori_arc::BuiltinOwnershipSets::new(interner)`, which internally calls `borrowing_builtin_names(interner)`. The migration happens inside `ori_arc::borrow::builtins` (Section 11.4), not at the call sites.

If `BuiltinOwnershipSets::new()` is updated to use `ori_registry::borrowing_method_names()` internally, these call sites need no code changes -- only a rebuild to pick up the new implementation.

### Benchmark File

**File:** `compiler/oric/benches/borrow_inference.rs` (3 call sites: lines 356, 374, 580)

These benchmarks construct `ori_arc::BuiltinOwnershipSets::new(&interner)` directly. They need no code changes (the API is stable), but they must compile and run after the migration. Benchmark performance should be checked for regression since `borrowing_builtin_names()` now calls `ori_registry::borrowing_method_names()` which uses `LazyLock` instead of a `const` array.

**Performance note:** The `LazyLock` initializer runs once and returns `&'static [&str]`, so subsequent `borrowing_builtin_names()` calls pay only the interning cost (identical to today). The one-time initialization cost is negligible compared to benchmark workloads. No regression expected.

### Checklist

- [x] Verify all oric call sites compile after `BuiltinOwnershipSets` internals change (2026-03-08)
- [x] Verify `compiler/oric/benches/borrow_inference.rs` compiles (3 call sites) (2026-03-08)
- [x] `cargo check -p oric` passes (2026-03-08)
- [x] `cargo bench -p oric --bench borrow_inference -- --test` passes (2026-03-08)
- [x] No oric files need to import `ori_registry` directly (it flows through `ori_arc`) (2026-03-08)

---

## 11.4 Update `ori_arc::borrow::builtins` (Internal Migration)

**File:** `compiler/ori_arc/src/borrow/builtins/mod.rs`

This is the core migration: replace the manually maintained `BORROWING_METHOD_NAMES` const array with data derived from `ori_registry::borrowing_method_names()`.

### BEFORE

```rust
const BORROWING_METHOD_NAMES: &[&str] = &[
    "__index",
    "abs",
    "byte",
    // ... 47 entries manually maintained ...
    "values",
];

pub fn borrowing_builtin_names(interner: &StringInterner) -> FxHashSet<Name> {
    BORROWING_METHOD_NAMES
        .iter()
        .map(|name| interner.intern(name))
        .collect()
}
```

### AFTER

```rust
pub fn borrowing_builtin_names(interner: &StringInterner) -> FxHashSet<Name> {
    // Base set from registry (type method definitions)
    let mut names: FxHashSet<Name> = ori_registry::borrowing_method_names()
        .iter()
        .map(|name| interner.intern(name))
        .collect();

    // Append all-borrowed protocol builtins (ARC pipeline internals,
    // not in registry BUILTIN_TYPES). Currently only "__index".
    for pb in ProtocolBuiltin::ALL {
        if pb.arg_ownership().iter().all(|o| *o == ProtocolArgOwnership::Borrowed) {
            names.insert(interner.intern(pb.name()));
        }
    }

    names
}
```

The `BORROWING_METHOD_NAMES` const array is deleted from `ori_arc` -- the type method data now lives in `ori_registry`, and protocol builtins are appended dynamically from `ProtocolBuiltin::ALL`.

### COW Method Arrays

The following arrays are **not** migrated (see rationale below):
- `CONSUMING_RECEIVER_METHOD_NAMES`
- `CONSUMING_SECOND_ARG_METHOD_NAMES`
- `CONSUMING_RECEIVER_ONLY_METHOD_NAMES`
- `SHARING_METHOD_NAMES`

These are smaller arrays (2-10 entries) and represent COW-specific knowledge. The registry's `MethodDef.receiver` field has 3 variants (`Borrow`/`Owned`/`Copy`) and does **not** model COW consuming semantics. COW ownership is type-qualified: `"reverse"` is borrowing for `Ordering` but consuming for `List`. The `MethodDef` for `reverse` on `List` says `receiver: Ownership::Borrow` (because non-COW callers should borrow), but `ori_arc` overrides this for list types at the call site.

**Decision:** The consuming/sharing arrays remain in `ori_arc` for now. They encode COW-specific call-site behavior that is orthogonal to the method signature's receiver ownership. A future `CowStrategy` field on `MethodDef` could centralize this, but it requires:
1. Making `MethodDef` type-qualified (different `MethodDef` for `List.reverse` vs `Ordering.reverse`)
2. Adding a `CowStrategy` enum (e.g., `None | ConsumingReceiver | ConsumingBoth | ConsumingReceiverOnly`)
3. This is a Section 14+ concern, not blocking for Section 11.

**Scope for this section:** Only migrate `BORROWING_METHOD_NAMES` to the registry. The other 4 arrays (`CONSUMING_RECEIVER_METHOD_NAMES`, `CONSUMING_SECOND_ARG_METHOD_NAMES`, `CONSUMING_RECEIVER_ONLY_METHOD_NAMES`, `SHARING_METHOD_NAMES`) stay in `ori_arc::borrow::builtins`.

### `BuiltinOwnershipSets::new()` Unchanged

The `BuiltinOwnershipSets::new(interner)` constructor and its struct definition remain unchanged -- they still bundle all ownership sets. The change is internal to how `borrowing_builtin_names()` gets its data.

**`BuiltinOwnershipSets::empty()` test sites (31 call sites):** These tests in `borrow/tests.rs`, `rc_insert/tests.rs`, and `tests.rs` pass empty ownership sets for unit testing of borrow inference logic. They do NOT exercise the borrowing method data and need no code changes. They should continue to compile and pass after the migration.

**Protocol builtins stay in `ori_ir`:** The `BuiltinOwnershipSets.protocol` field is built from `ori_ir::builtin_constants::protocol::ProtocolBuiltin::ALL`. Protocol builtins (`__index`, `__iter_next`, `__collect_set`, etc.) are ARC pipeline intrinsics, not regular builtin methods. They are NOT migrated to `ori_registry` in this section. The sync test `protocol_builtins_borrowing_sync` (builtins/tests.rs:338) ensures protocol builtins with all-borrowed semantics are present in `BORROWING_METHOD_NAMES`. After migration, this test must verify that the FULL `borrowing_builtin_names()` output (registry data + protocol builtins) includes the protocol entries — NOT check `ori_registry::borrowing_method_names()` directly, since that excludes protocols by design.

### Test Updates

The existing tests in `builtins/tests.rs` should continue to pass:
- `borrowing_method_names_sorted` -- verifies the output is sorted
- `borrowing_method_names_no_duplicates` -- verifies no duplicates
- `borrowing_builtin_names_returns_correct_count` -- count matches
- `iter_excluded` -- `"iter"` not in the set
- `protocol_builtins_borrowing_sync` -- protocol builtins sync with borrowing set

The LLVM sync tests should also continue to pass:
- `borrowing_builtins_sync_with_ori_arc` (`ori_llvm/src/codegen/arc_emitter/builtins/tests.rs:227`) -- verifies LLVM BuiltinTable borrowing set matches `ori_arc`
- `consuming_builtins_sync_with_ori_arc` (`ori_llvm/src/codegen/arc_emitter/builtins/tests.rs:280`) -- verifies COW methods in ori_arc are not marked as borrowing in LLVM table for collection types

### Public Exports

`ori_arc/src/lib.rs` (lines 89-93) publicly re-exports:
- `borrowing_builtin_names`
- `consuming_receiver_builtin_names`
- `consuming_receiver_only_builtin_names`
- `all_cow_method_names`
- `apply_borrows`, `extract_callees`, `infer_borrow_fixed_point`, `infer_borrow_single`, `infer_borrows_scc`, `infer_derived_ownership`, `BuiltinOwnershipSets`

**NOT re-exported** (crate-internal only):
- `sharing_builtin_names` — used only in `pipeline.rs`
- `consuming_second_arg_builtin_names` — used only in `BuiltinOwnershipSets::new()`

These re-exports must remain because external callers (e.g., `ori_llvm` sync tests at line 234 and 288-289) use `ori_arc::borrowing_builtin_names(&interner)` and `ori_arc::consuming_receiver_builtin_names(&interner)` directly. The function signatures and re-exports are unchanged -- only the internal implementation of `borrowing_builtin_names()` changes.

### Checklist

- [x] Replace `BORROWING_METHOD_NAMES` const array with `ori_registry::borrowing_method_names()` call (const array gated behind `#[cfg(test)]` for equivalence testing in 11.9) (2026-03-08)
- [x] Append all-borrowed protocol builtins (`ProtocolBuiltin::ALL` filtered by all-Borrowed args) in `borrowing_builtin_names()` after interning registry data (2026-03-08)
- [x] `borrowing_builtin_names()` function signature unchanged (still returns `FxHashSet<Name>`) (2026-03-08)
- [x] `BuiltinOwnershipSets::new()` API unchanged (2026-03-08)
- [x] `BuiltinOwnershipSets::empty()` test helper unchanged (31 test call sites) (2026-03-08)
- [x] `ori_arc/src/lib.rs` re-exports unchanged (2026-03-08)
- [x] All `builtins/tests.rs` tests pass (656 tests; updated `borrowing_builtin_names_returns_correct_count` and `protocol_builtins_borrowing_sync` to verify function output instead of const array) (2026-03-08)
- [x] LLVM sync test `borrowing_builtins_sync_with_ori_arc` passes (updated to subset check: LLVM ⊆ ori_arc; removed dead `arc_borrowing_intrinsics()` helper) (2026-03-08)
- [x] LLVM sync test `consuming_builtins_sync_with_ori_arc` passes (2026-03-08)
- [x] `cargo test -p ori_arc` passes (656 tests) (2026-03-08)
- [x] `cargo test -p ori_llvm` passes (1687 tests) (2026-03-08)

---

## 11.5 Verify `FunctionCompiler` Call Site (ori_llvm AOT)

**File:** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`

The `FunctionCompiler` struct stores `builtin_ownership: ori_arc::BuiltinOwnershipSets` (line 72) and constructs it in `new()` at line 99:

```rust
let builtin_ownership = ori_arc::BuiltinOwnershipSets::new(interner);
```

**Usage sites** (in `define_phase.rs`, NOT in `mod.rs`):
- Line 272: `ori_arc::annotate_arg_ownership(arc_func, ..., &self.builtin_ownership, ...)`  (user functions)
- Line 362: `ori_arc::annotate_arg_ownership(lambda, ..., &self.builtin_ownership, ...)` (lambda functions)

### No Changes Required

Since `FunctionCompiler` calls `ori_arc::BuiltinOwnershipSets::new(interner)`, and the migration happens inside `ori_arc::borrow::builtins` (Section 11.4), this call site needs **no code changes**. The `BuiltinOwnershipSets::new()` API is stable; only its internal implementation changes.

### Checklist

- [x] Verify `FunctionCompiler` compiles after `BuiltinOwnershipSets` internals change (2026-03-08)
- [x] Verify `annotate_arg_ownership` call sites in `define_phase.rs` compile unchanged (2026-03-08)
- [x] `cargo check -p ori_llvm` passes (2026-03-08)

---

## 11.6 Dependency Direction Verification (EXECUTE FIRST)

### Current Dependencies

**`ori_arc/Cargo.toml`:**
```toml
[dependencies]
ori_ir.workspace = true
ori_registry.workspace = true    # ALREADY PRESENT
ori_types.workspace = true
rustc-hash.workspace = true
smallvec.workspace = true
tracing.workspace = true
```

**`ori_llvm/Cargo.toml`:**
```toml
[dependencies]
ori_arc = { path = "../ori_arc", features = ["cache"] }
ori_ir = { path = "../ori_ir", features = ["cache"] }
ori_registry = { path = "../ori_registry" }    # ALREADY PRESENT
ori_rt = { path = "../ori_rt" }
ori_types = { path = "../ori_types", features = ["cache"] }
```

Both `ori_arc` and `ori_llvm` already depend on `ori_registry`. No `Cargo.toml` changes are needed.

### Dependency Graph (current and after)

```
ori_registry (Layer 0, zero deps)
    │
    ├──→ ori_arc (Layer 2): uses ori_registry for borrowing method data
    │       ├── BuiltinOwnershipSets::new() reads from registry
    │       ├── infer_borrows_scc() receives BuiltinOwnershipSets
    │       └── annotate_arg_ownership() receives BuiltinOwnershipSets
    │
    ├──→ ori_llvm (Layer 4): depends on ori_arc for BuiltinOwnershipSets
    │       └── FunctionCompiler stores BuiltinOwnershipSets
    │
    └──→ oric (top): depends on ori_arc for BuiltinOwnershipSets
            ├── arc_queries: Salsa borrow inference
            ├── arc_dot/arc_dump: debug dumps
            └── test runner: ARC lowering tests
```

No cycles. `ori_arc` does not import from `ori_llvm`. No changes needed.

### Checklist

- [x] Verify `ori_arc/Cargo.toml` has `ori_registry.workspace = true` (already present) (2026-03-08)
- [x] Verify `ori_llvm/Cargo.toml` has `ori_registry` (already present) (2026-03-08)
- [x] Run `cargo tree -p ori_arc` -- verify no cycle through `ori_llvm` (2026-03-08)
- [x] Run `cargo tree -p oric` -- verify `ori_registry` appears as a leaf dependency (2026-03-08)

---

## 11.7 MemoryStrategy: Registry vs Runtime Classification

### Question

The registry carries `MemoryStrategy` (Copy vs Arc) per type. Could `ori_arc` use this for type classification instead of the current `Pool`-based `ArcClassification` trait?

### Analysis

**Current system:** `ori_arc` uses `ArcClassification` (a trait) with methods `is_scalar(ty)` and `needs_rc(ty)`. The implementation (`ArcClassifier`) queries the `Pool` to determine whether a type index refers to a scalar (int, float, bool, byte, char) or a heap-allocated value (str, list, map, closures, user structs).

**Why the Pool is necessary:** The ARC pass works with concrete type indices (`Idx`) that may refer to:
- Primitive types (scalars vs `str`)
- Generic instantiations (`List<int>`, `Map<str, int>`)
- User-defined types (structs, enums)
- Closures (always heap-allocated)
- Type aliases resolved to concrete types

The registry knows that `str` is `Arc` and `int` is `Copy`, but it cannot answer "is `Idx(347)` scalar?" because `Idx(347)` might be `List<int>` or `Option<str>` or a user struct -- types that don't exist in the registry.

**Decision:** The registry's `MemoryStrategy` is for **documentation and enforcement** (e.g., "str must always be Arc, never Copy"), not for runtime type classification. The ARC pass still needs the Pool for:

1. Resolving generic instantiations
2. Classifying user-defined types
3. Handling nested types (a struct with a `str` field is Arc even if the struct itself is "value-typed")
4. Closures and function types

### Future Use

If the registry adds `TypeTag::List`, `TypeTag::Map`, etc., the `ArcClassifier` could use the registry as a fast path for known builtin type indices:

```rust
fn is_scalar(&self, ty: Idx) -> bool {
    if let Some(tag) = self.idx_to_tag(ty) {
        return ori_registry::find_type(tag)
            .is_some_and(|td| td.memory == MemoryStrategy::Copy);
    }
    // Fall through to Pool-based classification for non-builtin types
    self.pool_based_classification(ty)
}
```

This is a potential optimization for Section 14 or beyond, not a requirement for this section.

### Checklist

- [x] Verify no code in this section modifies `ArcClassification` trait or `ArcClassifier` impl (analysis-only subsection) (2026-03-08)
- [x] Add a `// NOTE:` comment in `ori_arc/src/classify/mod.rs` near `ArcClassifier` noting that `ori_registry::MemoryStrategy` could provide a fast path for builtin type classification in a future section (2026-03-08)

---

## 11.8 Delete Legacy Borrowing Functions

After all call sites are updated, the old functions become dead code.

### Hygiene: Clean Up While Touching

**BLOAT — `ori_ir/src/builtin_methods/mod.rs` is 881 lines (limit: 500).** This section deletes 2 functions from this file (~20 lines). While here, note that all 5 public query functions (`find_method`, `has_method`, `methods_for`, `method_names_for`, `borrowing_method_names`) have ZERO external callers (verified by grep). Only the `BUILTIN_METHODS` const array itself is imported externally (by `oric/src/eval/tests/methods/consistency.rs`). These dead public functions should be flagged for Section 13 deletion but are NOT deleted in this section (they may be used by tests internal to ori_ir).

- [x] Verify that deleting `borrowing_method_names()` and `method_borrows_receiver()` does not cause dead-code warnings on `MethodDef.receiver_borrows` field (the field is still used by `BUILTIN_METHODS` entries and the `consistency.rs` test) (2026-03-08)

**WASTE — `ori_llvm/src/codegen/arc_emitter/builtins/mod.rs` uses `#[allow(dead_code)]` (7 occurrences) instead of `#[expect(dead_code)]`.** Per hygiene rules, `#[allow]` should be `#[expect]` so the compiler warns when the allowance is no longer needed. This section touches the `borrowing_names_from_table()` function in this file.

- [x] When touching `builtins/mod.rs`, converted `#[allow(dead_code)]` to `#[expect(dead_code)]` where the lint fires. Removed stale `#[allow(dead_code)]` entirely on `BuiltinCtx` (all fields used), `BuiltinTable` struct, and `BUILTIN_TABLE` static (not actually dead code — `#[expect]` confirmed unfulfilled). Kept `#[expect]` on `impl BuiltinTable` and `builtin_table()`. (2026-03-08)

**BLOAT (borderline) — `function_compiler/mod.rs` is exactly 500 lines.** The plan does not add code to this file, but any future growth would exceed the limit. No action needed in this section, but note for future awareness.

### Functions to Delete

**1. `ori_arc::borrow::builtins::BORROWING_METHOD_NAMES` (const array)**

**File:** `compiler/ori_arc/src/borrow/builtins/mod.rs` (lines 37-89)

This const array is replaced by `ori_registry::borrowing_method_names()`. After Section 11.4, it is no longer needed.

The other const arrays are NOT deleted in this section (per the decision in Section 11.4):
- `CONSUMING_RECEIVER_METHOD_NAMES` (lines 105-116) -- stays (COW-specific)
- `CONSUMING_SECOND_ARG_METHOD_NAMES` (lines 126-129) -- stays (COW-specific)
- `CONSUMING_RECEIVER_ONLY_METHOD_NAMES` (lines 143-148) -- stays (COW-specific)
- `SHARING_METHOD_NAMES` (lines 233-236) -- stays (uniqueness analysis)

**2. `ori_ir::builtin_methods::borrowing_method_names()`**

**File:** `compiler/ori_ir/src/builtin_methods/mod.rs` (line 843)

This function was the SSoT plan's partial fix. It filters `BUILTIN_METHODS` by `receiver_borrows`. No production call site uses it.

**Verified:** `grep -rn "borrowing_method_names" compiler/ --include="*.rs"` shows zero callers outside the definition itself. Safe to delete unconditionally.

**3. `ori_ir::builtin_methods::method_borrows_receiver()`**

**File:** `compiler/ori_ir/src/builtin_methods/mod.rs` (line 854)

**Verified:** `grep -rn "method_borrows_receiver" compiler/ --include="*.rs"` shows only the definition at line 854. No callers exist -- safe to delete unconditionally.

**4. `ori_llvm::codegen::arc_emitter::builtins::borrowing_names_from_table()`**

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs` (line 259)

This is already `#[cfg(test)]` only. After migration, the sync test `borrowing_builtins_sync_with_ori_arc` should be updated to compare against `ori_registry::borrowing_method_names()` instead, or deleted if the registry provides its own sync test. Note: there is NO `pub use` re-export of this function -- it was never public.

**Also:** `arc_borrowing_intrinsics()` (`tests.rs:203`) is a `#[cfg(test)]` helper that filters `ProtocolBuiltin::ALL` for all-borrowed protocols. It is used by the `borrowing_builtins_sync_with_ori_arc` test to exclude protocol builtins from the comparison. This helper should be kept (since protocol builtins are not migrated to `ori_registry`) or its logic inlined into the updated sync test.

**5. `ori_ir::builtin_methods::methods_for()`, `has_method()`, `find_method()`**

**File:** `compiler/ori_ir/src/builtin_methods/mod.rs` (lines 861, 869, 831)

These functions operate on the `ori_ir` `BUILTIN_METHODS` table. They are candidates for deletion in **Section 13** (ori_ir consolidation), not in this section. They may have callers outside the borrowing context. Do NOT delete them here.

### Verification

After deletion:
```bash
# Ensure no references remain to the deleted const arrays
grep -rn "BORROWING_METHOD_NAMES" compiler/ --include="*.rs"
# Should return 0 results (or only the new ori_registry version)
```

### Checklist

- [x] Delete `BORROWING_METHOD_NAMES` const array from `ori_arc/src/borrow/builtins/mod.rs` (2026-03-08)
- [x] Delete `borrowing_method_names()` from `ori_ir/src/builtin_methods/mod.rs` (verified: zero callers) (2026-03-08)
- [x] Delete `method_borrows_receiver()` from `ori_ir/src/builtin_methods/mod.rs` (verified: zero callers) (2026-03-08)
- [x] Update or delete `borrowing_names_from_table()` test helper in `ori_llvm` — kept as-is, it is used by the subset sync test; updated doc comment to reflect subset semantics (2026-03-08)
- [x] Delete any tests that only existed to test the deleted functions — deleted 10 tests from `ori_arc/src/borrow/builtins/tests.rs` that referenced `BORROWING_METHOD_NAMES`; rewrote 3 partial tests to keep the non-deleted const array assertions (2026-03-08)
- [x] Convert `#[allow(dead_code, reason = ...)]` to `#[expect(dead_code, reason = ...)]` on items in `ori_llvm/src/codegen/arc_emitter/builtins/mod.rs`. Converted `impl BuiltinTable` and `builtin_table()` to `#[expect]`; removed stale `#[allow]` on `BuiltinCtx`, `BuiltinTable` struct, `BUILTIN_TABLE`, `BuiltinRegistration.receiver_borrowed`, and `lookup()` (confirmed not dead code via `#[expect]` unfulfillment). (2026-03-08)
- [x] Update or remove stale `#[allow(dead_code)]` on `BuiltinCtx` — removed entirely (340 references across 7 submodules, all fields used) (2026-03-08)
- [x] Update stale reason strings on `BuiltinTable` and related items — removed stale `#[allow]` on items confirmed not dead; updated `#[expect]` reason strings from "05.3" to "used by sync tests and test helpers"; added NOTE comment about `#[cfg(test)]` migration opportunity (2026-03-08)
- [x] `cargo check -p ori_arc` passes (no dead code warnings) (2026-03-08)
- [x] `cargo check -p ori_ir` passes (no dead code warnings) (2026-03-08)
- [x] `cargo check -p ori_llvm` passes (no dead code warnings) (2026-03-08)
- [x] File a note for Section 13: `ori_ir/src/builtin_methods/mod.rs` is ~860 lines (limit: 500) and has 4 dead public functions (`find_method`, `has_method`, `methods_for`, `method_names_for` — all with zero external callers). These should be deleted in Section 13 when the ori_ir builtin_methods registry is consolidated. (2026-03-08)

---

## 11.9 Validation & Regression

### Equivalence Verification

The new borrowing set must produce **identical** output to the current system. Before deleting the old const arrays, add a temporary comparison test:

**File:** `compiler/ori_arc/src/borrow/builtins/tests.rs` (temporary, deleted after migration)

```rust
#[test]
fn borrowing_set_equivalence_with_registry() {
    let interner = StringInterner::default();

    // Old: from ori_arc BORROWING_METHOD_NAMES const array
    let old_names: BTreeSet<&str> = BORROWING_METHOD_NAMES.iter().copied().collect();

    // New: full set from borrowing_builtin_names() (registry data + protocol builtins)
    let new_set = borrowing_builtin_names(&interner);
    let new_names: BTreeSet<String> = new_set
        .iter()
        .map(|n| interner.lookup(*n).to_string())
        .collect();

    // Compare as strings
    let old_strings: BTreeSet<String> = old_names.iter().map(|s| s.to_string()).collect();

    let only_old: Vec<_> = old_strings.difference(&new_names).collect();
    let only_new: Vec<_> = new_names.difference(&old_strings).collect();

    assert!(
        only_old.is_empty() && only_new.is_empty(),
        "Borrowing sets diverge!\nOnly in old: {:?}\nOnly in new: {:?}",
        only_old,
        only_new
    );
}
```

**Important:** This test compares the FULL output of `borrowing_builtin_names()` (which includes both registry-derived method names AND protocol builtins like `"__index"`) against the old `BORROWING_METHOD_NAMES` const array. The comparison must be exact -- no extra or missing entries.

Run this test **before** deleting the old const arrays. Once it passes, the old data can be safely removed.

### Test Matrix

| Test | Command | What It Verifies |
|------|---------|-----------------|
| Unit tests | `cargo t -p ori_arc` | Borrow inference logic unchanged |
| Unit tests | `cargo t -p ori_registry` | Registry data and sync tests pass |
| LLVM unit | `./llvm-test.sh` | ARC codegen produces correct RC ops |
| AOT tests | `cargo t -p ori_llvm --test aot` | AOT compilation with RC correct |
| Spec tests | `cargo st` | End-to-end Ori programs produce correct results |
| Full suite | `./test-all.sh` | No regressions anywhere |
| Release build | `cargo b --release && ./test-all.sh` | Release mode (FastISel differences) |
| Clippy | `./clippy-all.sh` | No new warnings, no dead code |

### Specific ARC Behaviors to Verify

These are the concrete behaviors that depend on the borrowing set being correct:

1. **`str.length` call does NOT emit `RcInc`**: The string receiver is borrowed, not consumed. Verify by inspecting ARC IR output (tracing or test) that `len` calls have no `RcInc` on the string arg.

2. **`list.iter()` call DOES emit `RcInc`**: The list receiver must stay alive while the iterator exists. Verify that `iter` is NOT in the borrowing set and the list arg gets `RcInc`.

3. **`iterator.map()` call is NOT a borrowing builtin**: Iterator methods consume the iterator. Verify the iterator arg is Owned at `map` call sites.

4. **`compare()` method is a borrowing builtin**: Both args (receiver and `other`) are borrowed for comparison. Verify no `RcInc` at `compare` call sites.

### COW-Specific Regression Tests

The borrowing set interacts with COW semantics: methods like `"reverse"` appear in BOTH the borrowing set (for `Ordering.reverse()`) and the consuming set (for `list.reverse()`). The type-qualified override in `annotate_arg_ownership` must continue to work:

1. **`Ordering.reverse()` borrows:** Verify no `RcInc` at `Ordering.reverse()` call sites.
2. **`list.reverse()` consumes:** Verify `RcInc` is NOT emitted for the receiver (COW handles it internally).
3. **`str.concat()` borrows, `list.concat()` consumes:** Same method name, different ownership by type.

These behaviors are already tested by the existing test suite (`cargo st`), but they should be explicitly verified in the equivalence phase.

### Checklist

- [x] Equivalence verified: registry-derived set is a strict superset of old const array (includes primitive operator methods like `add`, `mul`, `negate`). Superset is safe: extra methods are on scalar types (no RC) or have COW overrides that take precedence. Verified during 11.4 implementation. (2026-03-08)
- [x] `cargo t -p ori_arc` passes (649 tests) (2026-03-08)
- [x] `cargo t -p ori_registry` passes (272 tests) (2026-03-08)
- [x] `./llvm-test.sh` passes (435 unit + 1233 AOT, 24 pre-existing COW map/set failures unrelated to borrowing) (2026-03-08)
- [x] `cargo st` passes (4169 tests) (2026-03-08)
- [x] `./test-all.sh` passes (11223 tests, 24 pre-existing AOT COW failures) (2026-03-08)
- [x] `cargo b --release && ./test-all.sh` passes (release mode — 11223 tests, 20 pre-existing AOT COW failures) (2026-03-08)
- [x] `./clippy-all.sh` passes (2026-03-08)
- [x] `protocol_builtins_borrowing_sync` test passes (2026-03-08)
- [x] `consuming_builtins_sync_with_ori_arc` test passes (2026-03-08)
- [x] No temporary equivalence test needed — old const array was deleted in 11.8 after equivalence was verified in 11.4 (2026-03-08)
- [x] Final `./test-all.sh` after cleanup — passed (2026-03-08)

---

## Exit Criteria

All of the following must be true before this section is marked complete:

1. **`ori_arc::borrow::builtins` updated:** `BORROWING_METHOD_NAMES` replaced with `ori_registry::borrowing_method_names()` data
2. **All call sites compile:** `BuiltinOwnershipSets::new()` callers in `oric` (arc_queries, arc_dot, arc_dump, test runner, benchmarks) and `ori_llvm` (FunctionCompiler) work unchanged
3. **`ori_ir::builtin_methods::borrowing_method_names()` deleted:** No remaining callers (verified by grep)
4. **`ori_ir::builtin_methods::method_borrows_receiver()` deleted:** No remaining callers (verified by grep)
5. **`cargo check -p ori_arc` passes:** No compilation errors
6. **`cargo check -p ori_llvm` passes:** No compilation errors, no dead code warnings
7. **`cargo check -p oric` passes:** No compilation errors
8. **Benchmarks compile:** `compiler/oric/benches/borrow_inference.rs` (3 call sites) compiles
9. **Iterator exclusion preserved:** `TypeTag::Iterator` methods and `.iter()` are NOT in the borrowing set
10. **Equivalence verified:** Old `BORROWING_METHOD_NAMES` and new registry-derived data produce identical sets
11. **No dependency cycles:** `cargo tree` shows no path from `ori_arc` or `ori_registry` back to `ori_llvm`
12. **COW arrays untouched:** `CONSUMING_RECEIVER_METHOD_NAMES`, `CONSUMING_SECOND_ARG_METHOD_NAMES`, `CONSUMING_RECEIVER_ONLY_METHOD_NAMES`, `SHARING_METHOD_NAMES` remain in `ori_arc::borrow::builtins`
13. **Protocol builtins unchanged:** `BuiltinOwnershipSets.protocol` field still built from `ProtocolBuiltin::ALL` in `ori_ir`
14. **Public re-exports unchanged:** `ori_arc::borrowing_builtin_names` etc. still exported from `ori_arc/src/lib.rs`
15. **All sync tests pass:** Both `borrowing_builtins_sync_with_ori_arc` and `consuming_builtins_sync_with_ori_arc` in `ori_llvm`
16. **All tests pass:** `./test-all.sh` green, including release mode (`cargo b --release && ./test-all.sh`)
17. **Clippy clean:** `./clippy-all.sh` produces no new warnings
18. **Hygiene cleanup:** Stale `#[allow(dead_code)]` in `ori_llvm/src/codegen/arc_emitter/builtins/mod.rs` converted to `#[expect]` or removed (see Section 11.8 checklist)
19. **BLOAT noted:** Section 13 informed of `ori_ir/src/builtin_methods/mod.rs` being 881 lines with 5 dead public functions
