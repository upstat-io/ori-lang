---
section: "11"
title: "Wire ARC & Borrow Pass (ori_arc)"
status: not-started
goal: "Replace all borrowing_builtins construction with ori_registry as the single source of truth, fix dependency direction, and preserve iterator exclusion semantics"
depends_on:
  - "08"
  - "02"
sections:
  - id: "11.1"
    title: "Add ori_registry::borrowing_method_names() helper"
    status: not-started
  - id: "11.2"
    title: "Iterator and derived-value exclusion logic"
    status: not-started
  - id: "11.3"
    title: "Update compile_common.rs call sites (oric)"
    status: not-started
  - id: "11.4"
    title: "Update evaluator.rs call site (ori_llvm JIT)"
    status: not-started
  - id: "11.5"
    title: "Update FunctionCompiler call site (ori_llvm AOT)"
    status: not-started
  - id: "11.6"
    title: "Fix dependency direction (ori_arc → ori_registry)"
    status: not-started
  - id: "11.7"
    title: "MemoryStrategy: registry vs runtime classification"
    status: not-started
  - id: "11.8"
    title: "Delete legacy borrowing functions"
    status: not-started
  - id: "11.9"
    title: "Validation & regression"
    status: not-started
---

# Section 11: Wire ARC & Borrow Pass (ori_arc)

**Status:** Not Started
**Goal:** The ARC/borrow pass (`ori_arc`) and all its call sites consume borrowing ownership data exclusively from `ori_registry`. The backwards dependency on `ori_llvm` is eliminated. The legacy `borrowing_method_names()` in `ori_ir` and `borrowing_builtin_names()` in `ori_llvm` are deleted. Iterator exclusion semantics are preserved exactly.

**Context:** The ARC borrow inference pass (`ori_arc::infer_borrows`) needs to know which builtin methods borrow their receiver so it can skip RC operations at their call sites. Currently this data flows through two different functions depending on which code path constructs it:

1. **`ori_ir::builtin_methods::borrowing_method_names()`** -- filters the `BUILTIN_METHODS` table by `receiver_borrows: true`. This was the original SSoT plan's partial fix (Section 01 of `builtin_ownership_ssot`).

2. **`ori_llvm::codegen::arc_emitter::builtins::borrowing_builtin_names()`** -- iterates the LLVM `BuiltinTable` (a `LazyLock` singleton built from 7 submodule `REGISTERED` arrays), filters by `receiver_borrowed: true`, and excludes Iterator methods and `.iter()`. This is what all 4 production call sites actually use.

The registry plan replaces both with a single helper in `ori_registry` that derives the borrowing set from the `TypeDef` method specifications.

**Design rationale:** The borrowing set is a pure function of the method ownership metadata. It requires no runtime state, no string interner, no type pool -- just a list of method names where `Ownership == Borrow`. The registry already stores this data (every `MethodDef` carries a receiver `Ownership` field). The helper belongs in `ori_registry` because it is a projection of const data, not in any consuming crate.

---

## Current Architecture

### Data Flow (today)

```
ori_llvm::codegen::arc_emitter::builtins
  BuiltinTable (LazyLock singleton)
    7 submodule REGISTERED arrays
      BuiltinRegistration { receiver_borrowed: bool, ... }
        │
        │  borrowing_builtin_names(interner)
        │  (filters by receiver_borrowed, excludes Iterator/iter)
        ▼
  FxHashSet<Name>  ───────────────────────────────────────────────┐
                                                                   │
  4 call sites:                                                    │
  ├── oric/commands/compile_common.rs:184  (AOT borrow inference)  │
  ├── oric/commands/compile_common.rs:218  (AOT cached path)       │
  ├── ori_llvm/evaluator.rs:376           (JIT borrow inference)   │
  └── ori_llvm/codegen/function_compiler/mod.rs:106 (AOT RC annot) │
                                                                   │
  ori_arc::borrow::infer_borrows(functions, classifier, &set) ◄───┘
  ori_arc::rc_insert::annotate_arg_ownership(func, sigs, interner, &set)
```

### Dependency Direction (today)

```
oric ──depends──→ ori_llvm  (for borrowing_builtin_names)
     ──depends──→ ori_arc   (for infer_borrows)

ori_llvm ──depends──→ ori_arc  (for annotate_arg_ownership)
         ──uses──→ own BuiltinTable (for borrowing_builtin_names)

ori_arc ──depends──→ ori_ir   (for Name, but NOT for borrowing data)
```

The `oric → ori_llvm` dependency for `borrowing_builtin_names` is architecturally wrong: the CLI driver should not reach into the LLVM codegen layer to get ARC metadata. The data originates from method ownership declarations that belong at Layer 0.

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
  &[&str]  (const, no interning needed at registry level)
        │
  Call sites intern into FxHashSet<Name> locally:
  ├── oric/commands/compile_common.rs
  ├── ori_llvm/evaluator.rs
  └── ori_llvm/codegen/function_compiler/mod.rs
        │
  ori_arc::infer_borrows(functions, classifier, &set)
  ori_arc::annotate_arg_ownership(func, sigs, interner, &set)
```

### Dependency Direction (after)

```
ori_registry (Layer 0) ◄── ori_arc (Layer 2)
                        ◄── ori_llvm (excluded)
                        ◄── oric (top)

No phase reaches into another phase for ownership metadata.
```

---

## 11.1 Add `ori_registry::borrowing_method_names()` Helper

**File:** `compiler/ori_registry/src/lib.rs` (or a dedicated `queries.rs` if Section 08 split it out)

This is a `const fn` (or regular `fn` if const iteration isn't stable for the pattern) that returns the set of method names whose receiver uses borrowing semantics. The caller is responsible for interning the names into `Name` values -- the registry does not depend on `ori_ir` and cannot intern.

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
    // Computed at compile time or via LazyLock from BUILTIN_TYPES.
    // Implementation depends on whether const iteration is feasible
    // (see design decision below).
    &BORROWING_METHODS
}
```

### Implementation Strategy

Because `ori_registry` is a pure-data crate with zero dependencies, and because Rust does not support `const fn` iteration over slices with filtering (no const `Vec`, no const collect), the borrowing method list must be spelled out explicitly as a `const` array. This is acceptable because:

1. The data changes rarely (only when a new builtin method is added).
2. A sync test (Section 14) verifies the explicit list matches `BUILTIN_TYPES` method definitions.
3. The list is small (currently ~20-30 names).

```rust
/// Borrowing method names, pre-filtered from BUILTIN_TYPES.
///
/// Invariant: every name here appears in some TypeDef.methods with
/// receiver == Ownership::Borrow AND is NOT an Iterator/DEI method
/// AND is NOT `.iter()`. Enforced by test `borrowing_names_match_type_defs`.
static BORROWING_METHODS: &[&str] = &[
    // int methods
    "abs",
    "to_str",
    // float methods
    "floor", "ceil", "round", "sqrt",
    // str methods
    "length", "is_empty", "contains", "starts_with", "ends_with",
    "to_upper", "to_lower", "trim", "trim_start", "trim_end",
    // ... (populated from TypeDef.methods where receiver == Borrow)
    // list methods
    "len",
    // Comparable trait methods
    "compare",
    // Eq trait methods
    "eq",
    // Hashable trait methods
    "hash",
    // Printable/Debug trait methods
    "to_str",
    // ... etc.
];
```

### Alternative: Runtime Derivation

If the explicit list is too fragile, the alternative is a `LazyLock` or a one-time function that iterates `BUILTIN_TYPES` at first call:

```rust
/// Derive borrowing method names from BUILTIN_TYPES at runtime.
///
/// Returns a deduplicated, sorted slice of method names.
pub fn borrowing_method_names() -> &'static [&'static str] {
    static NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
        let mut names: Vec<&str> = BUILTIN_TYPES
            .iter()
            .filter(|td| td.tag != TypeTag::Iterator)
            .flat_map(|td| td.methods.iter())
            .filter(|m| m.receiver == Ownership::Borrow && m.name != "iter")
            .map(|m| m.name)
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    });
    &NAMES
}
```

**Decision:** Use the `LazyLock` approach. It derives from `BUILTIN_TYPES` automatically, cannot drift, and the `LazyLock` dependency is in `std` (no external crate needed). The purity tests (Section 02.3) allow `LazyLock` because it does not allocate on the heap in the public API sense -- it initializes once and returns a `&'static` reference. The `Vec` is internal to the initializer.

**However:** This introduces a heap allocation (`Vec`) inside the `LazyLock` initializer. Section 02.3's `purity_no_heap_allocation_types` test scans for `Vec<` in source files and would flag this. Two options:

1. **Exempt the query module** from the heap scan (add a `// ori_registry:allow-heap` marker that the test recognizes).
2. **Use a fixed-size array** with a const upper bound (e.g., `[&str; 64]` with a length counter).

The recommended approach is option 1: the purity contract is about the public API types (no `Vec` in function signatures), not about internal implementation details of lazy initialization. Add a comment to the scan test explaining that `LazyLock` initializers may use `Vec` internally.

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

- [ ] Add `borrowing_method_names()` to `ori_registry/src/lib.rs`
- [ ] Derive from `BUILTIN_TYPES` via `LazyLock` (or explicit list with sync test)
- [ ] Filter: `receiver == Ownership::Borrow`
- [ ] Exclude: `TypeTag::Iterator` methods
- [ ] Exclude: method name `"iter"`
- [ ] Deduplicate (multiple types may share a method name like `"to_str"`)
- [ ] Add sync test `borrowing_names_match_type_defs`
- [ ] `cargo test -p ori_registry` passes

---

## 11.2 Iterator and Derived-Value Exclusion Logic

The current `borrowing_builtin_names()` in `ori_llvm` excludes two categories:

### Category 1: All Iterator type methods

```rust
// ori_llvm/codegen/arc_emitter/builtins/mod.rs:271
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
// ori_llvm/codegen/arc_emitter/builtins/mod.rs:279
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

If `DoubleEndedIterator` has its own `TypeTag` (e.g., `TypeTag::DoubleEndedIterator`), it must also be excluded. The current `ori_llvm` code only checks `"Iterator"` because DoubleEndedIterator methods are registered under the `"Iterator"` type name in the `BuiltinTable`. If the registry separates them, both tags must be excluded:

```rust
td.tag != TypeTag::Iterator && td.tag != TypeTag::DoubleEndedIterator
```

### Correctness Invariant

The exclusion logic is a **safety invariant**, not a performance optimization. Getting it wrong causes use-after-free:

- **False positive (incorrectly excluded):** A method that should be in the borrowing set is excluded. Result: the ARC pass treats its args as Owned, emitting unnecessary `RcInc`/`RcDec`. Performance penalty but correct.
- **False negative (incorrectly included):** A method that creates a derived value (like `.iter()`) is included in the borrowing set. Result: the ARC pass skips `RcInc` for the receiver, which may be freed while the derived value still exists. **Use-after-free.**

Therefore, when in doubt, **exclude** (err toward Owned semantics).

### Checklist

- [ ] `TypeTag::Iterator` methods excluded from borrowing set
- [ ] `"iter"` method excluded regardless of type
- [ ] If `TypeTag::DoubleEndedIterator` exists as a separate tag, also excluded
- [ ] Document the safety invariant in the helper's doc comment
- [ ] No other methods create derived values with hidden receiver dependencies (audit `BUILTIN_TYPES` methods)

---

## 11.3 Update `compile_common.rs` Call Sites (oric)

**File:** `compiler/oric/src/commands/compile_common.rs`

Two call sites, both calling `ori_llvm::codegen::arc_emitter::borrowing_builtin_names(interner)`.

### Call Site 1: `run_borrow_inference()` (line 184)

**BEFORE:**
```rust
let borrowing_builtins = ori_llvm::codegen::arc_emitter::borrowing_builtin_names(interner);
let sigs = ori_arc::infer_borrows(&arc_functions, classifier, &borrowing_builtins);
```

**AFTER:**
```rust
let borrowing_builtins: FxHashSet<Name> = ori_registry::borrowing_method_names()
    .iter()
    .map(|name| interner.intern(name))
    .collect();
let sigs = ori_arc::infer_borrows(&arc_functions, classifier, &borrowing_builtins);
```

### Call Site 2: `run_arc_pipeline_cached()` (line 218-220)

**BEFORE:**
```rust
let borrowing_builtins =
    ori_llvm::codegen::arc_emitter::borrowing_builtin_names(interner);
return ori_arc::infer_borrows(&arc_functions, classifier, &borrowing_builtins);
```

**AFTER:**
```rust
let borrowing_builtins: FxHashSet<Name> = ori_registry::borrowing_method_names()
    .iter()
    .map(|name| interner.intern(name))
    .collect();
return ori_arc::infer_borrows(&arc_functions, classifier, &borrowing_builtins);
```

### Extract Helper

Both call sites do the same interning. Extract a helper to avoid duplication:

```rust
/// Build the borrowing builtins set from the registry.
///
/// Interns `ori_registry::borrowing_method_names()` into `Name` values
/// for consumption by `ori_arc::infer_borrows` and `annotate_arg_ownership`.
fn borrowing_builtins_set(interner: &StringInterner) -> FxHashSet<Name> {
    ori_registry::borrowing_method_names()
        .iter()
        .map(|name| interner.intern(name))
        .collect()
}
```

Then both call sites become:
```rust
let borrowing_builtins = borrowing_builtins_set(interner);
```

### Dependency Impact

After this change, `compile_common.rs` no longer calls into `ori_llvm::codegen::arc_emitter`. If this was the only reason `oric` imported that path, the import can be removed. Verify by checking other uses of `ori_llvm::codegen::arc_emitter` in `oric`.

### Checklist

- [ ] Replace line 184 with registry-based construction
- [ ] Replace lines 218-220 with registry-based construction
- [ ] Extract `borrowing_builtins_set()` helper (or inline if the pattern is simple enough)
- [ ] Remove unused `ori_llvm::codegen::arc_emitter` import from `compile_common.rs` (if no other uses)
- [ ] `cargo check -p oric` passes
- [ ] `cargo check -p oric --features llvm` passes

---

## 11.4 Update `evaluator.rs` Call Site (ori_llvm JIT)

**File:** `compiler/ori_llvm/src/evaluator.rs` (line 376-378)

This is the JIT execution path. It constructs the borrowing set and passes it directly to `ori_arc::infer_borrows`.

### BEFORE

```rust
let borrowing_builtins =
    crate::codegen::arc_emitter::borrowing_builtin_names(interner);
ori_arc::infer_borrows(&arc_functions, &classifier, &borrowing_builtins)
```

### AFTER

```rust
let borrowing_builtins: FxHashSet<Name> = ori_registry::borrowing_method_names()
    .iter()
    .map(|name| interner.intern(name))
    .collect();
ori_arc::infer_borrows(&arc_functions, &classifier, &borrowing_builtins)
```

### Import Changes

- Add: `use ori_registry;` (or `use rustc_hash::FxHashSet;` if not already imported)
- Remove: the `crate::codegen::arc_emitter::borrowing_builtin_names` call (the `pub use` re-export in `arc_emitter/mod.rs` may become unused)

### Checklist

- [ ] Replace lines 376-378 with registry-based construction
- [ ] Add `ori_registry` import
- [ ] Verify `cargo check -p ori_llvm` passes (requires `ori_registry` in `ori_llvm/Cargo.toml`, added in Section 02.4)

---

## 11.5 Update `FunctionCompiler` Call Site (ori_llvm AOT)

**File:** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs` (line 106)

This is the AOT compilation path. `FunctionCompiler::new()` constructs the borrowing set once and stores it in the `borrowing_builtins` field. The field is then used at 3 points:
- Line 403: `annotate_arg_ownership` for user functions
- Line 483: `annotate_arg_ownership` for lambda functions
- Line 701: `annotate_arg_ownership` for derived trait functions

### BEFORE (constructor, line 106)

```rust
let borrowing_builtins = crate::codegen::arc_emitter::borrowing_builtin_names(interner);
```

### AFTER

```rust
let borrowing_builtins: FxHashSet<Name> = ori_registry::borrowing_method_names()
    .iter()
    .map(|name| interner.intern(name))
    .collect();
```

### Field and Usage Sites

The `borrowing_builtins: FxHashSet<Name>` field on `FunctionCompiler` (line 82) stays the same type. The 3 usage sites (lines 403, 483, 701) reference `&self.borrowing_builtins` and are unchanged -- only the construction changes.

### Import Changes

- Add: `use ori_registry;` (at the crate level or in the file)
- The field type `FxHashSet<Name>` is already imported

### Checklist

- [ ] Replace line 106 with registry-based construction
- [ ] Verify field type unchanged (`FxHashSet<Name>`)
- [ ] Verify 3 usage sites (lines 403, 483, 701) compile unchanged
- [ ] `cargo check -p ori_llvm` passes

---

## 11.6 Fix Dependency Direction (ori_arc -> ori_registry)

### Current Dependencies

**`ori_arc/Cargo.toml`:**
```toml
[dependencies]
ori_ir.workspace = true
ori_types.workspace = true
rustc-hash.workspace = true
smallvec.workspace = true
tracing.workspace = true
```

`ori_arc` does **not** currently depend on `ori_registry`. It receives the borrowing set as a parameter (`&FxHashSet<Name>`) from the caller. This means `ori_arc` itself does not need a direct dependency on `ori_registry` -- the callers (`oric`, `ori_llvm`) construct the set and pass it in.

### Decision: Keep Indirect or Go Direct?

**Option A: Keep `ori_arc` dependency-free from `ori_registry`** (recommended)

The `infer_borrows` and `annotate_arg_ownership` functions already accept `&FxHashSet<Name>` as parameters. Callers construct the set from `ori_registry`. This preserves `ori_arc`'s current API and avoids adding a dependency.

Pros:
- No `Cargo.toml` change for `ori_arc`
- `ori_arc` stays focused on ARC analysis, not data sourcing
- Parameter injection is a clean pattern (testable with any set)

Cons:
- 3-4 call sites each do the same `intern` + `collect` (mitigated by helper function)

**Option B: `ori_arc` depends on `ori_registry` and builds the set internally**

Add `ori_registry.workspace = true` to `ori_arc/Cargo.toml`. Add a function `ori_arc::borrowing_builtins(interner: &StringInterner) -> FxHashSet<Name>` that wraps the registry call. Callers use this instead of building the set themselves.

Pros:
- Single construction point
- Callers don't need to know about `ori_registry`

Cons:
- Adds a dependency edge (`ori_arc → ori_registry`)
- `ori_arc` gains string interning responsibility (not its job)
- `ori_registry` was already added to `ori_arc/Cargo.toml` in Section 02.4

### Recommendation

**Option A** for `infer_borrows` and `annotate_arg_ownership` (keep the parameter). But since Section 02.4 already adds `ori_registry` to `ori_arc/Cargo.toml`, the dependency exists regardless. The `MemoryStrategy` data from `ori_registry` may be used by `ori_arc` for future type classification (see 11.7). So the dependency is justified but the borrowing set construction stays in the callers.

### Dependency Graph Verification

After all changes:

```
ori_registry (Layer 0, zero deps)
    │
    ├──→ oric (Layer 5): constructs FxHashSet<Name> from registry
    │       ├── passes to ori_arc::infer_borrows()
    │       └── passes to ori_arc::annotate_arg_ownership() (via ori_llvm)
    │
    ├──→ ori_llvm (excluded): constructs FxHashSet<Name> from registry
    │       ├── FunctionCompiler stores as field
    │       └── passes to ori_arc::annotate_arg_ownership()
    │
    └──→ ori_arc (Layer 2): receives FxHashSet<Name> as parameter
            ├── infer_borrows() uses it for unknown-callee classification
            └── annotate_arg_ownership() uses it for builtin method detection
```

No cycles. `ori_arc` does not import from `ori_llvm`. `oric` does not reach into `ori_llvm` for borrowing data.

### Checklist

- [ ] Verify `ori_arc/Cargo.toml` has `ori_registry.workspace = true` (from Section 02.4)
- [ ] Verify `oric` no longer imports `ori_llvm::codegen::arc_emitter::borrowing_builtin_names`
- [ ] Verify `ori_llvm` no longer needs `pub use builtins::borrowing_builtin_names` in `arc_emitter/mod.rs`
- [ ] Run `cargo tree -p ori_arc` -- verify no cycle through `ori_llvm`
- [ ] Run `cargo tree -p oric` -- verify `ori_registry` appears as a leaf dependency

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

- [ ] Document that `MemoryStrategy` is not used for runtime classification in this section
- [ ] No changes to `ArcClassification` trait or `ArcClassifier` impl
- [ ] Add a comment in `ori_arc` noting the registry MemoryStrategy exists for future use

---

## 11.8 Delete Legacy Borrowing Functions

After all call sites are updated, the old functions become dead code.

### Functions to Delete

**1. `ori_llvm::codegen::arc_emitter::builtins::borrowing_builtin_names()`**

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs` (lines 255-286)

This function iterates `BuiltinTable`, filters by `receiver_borrowed`, excludes Iterator/iter, and returns `FxHashSet<Name>`. It is replaced by `ori_registry::borrowing_method_names()`.

Also delete the `pub use` re-export:
**File:** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs` (line 17)
```rust
pub use builtins::borrowing_builtin_names;  // DELETE
```

**2. `ori_ir::builtin_methods::borrowing_method_names()`**

**File:** `compiler/ori_ir/src/builtin_methods/mod.rs` (lines 822-832)

This function was the SSoT plan's partial fix. It filters `BUILTIN_METHODS` by `receiver_borrows`. No production call site uses it (all use the `ori_llvm` version), but it may be referenced by tests.

Check for callers before deleting:
```bash
grep -rn "borrowing_method_names" compiler/ --include="*.rs"
```

If only referenced by tests within `ori_ir`, delete both the function and its tests.

**3. `ori_ir::builtin_methods::method_borrows_receiver()`**

**File:** `compiler/ori_ir/src/builtin_methods/mod.rs` (lines 834-839)

Check if any call sites remain after the migration. If unused, delete.

### Verification

After deletion:
```bash
# Ensure no references remain
grep -rn "borrowing_method_names\|borrowing_builtin_names" compiler/ --include="*.rs"
# Should return 0 results (or only the new ori_registry function)
```

### Checklist

- [ ] Delete `borrowing_builtin_names()` from `ori_llvm/src/codegen/arc_emitter/builtins/mod.rs`
- [ ] Delete `pub use builtins::borrowing_builtin_names` from `ori_llvm/src/codegen/arc_emitter/mod.rs`
- [ ] Delete `borrowing_method_names()` from `ori_ir/src/builtin_methods/mod.rs` (if no callers)
- [ ] Delete `method_borrows_receiver()` from `ori_ir/src/builtin_methods/mod.rs` (if no callers)
- [ ] Delete any tests that only existed to test the deleted functions
- [ ] `cargo check -p ori_llvm` passes (no unused import warnings)
- [ ] `cargo check -p ori_ir` passes (no dead code warnings)
- [ ] `grep -rn "borrowing_method_names\|borrowing_builtin_names"` returns only `ori_registry`

---

## 11.9 Validation & Regression

### Equivalence Verification

The new borrowing set must produce **identical** output to the current system. Before deleting the old functions, add a temporary comparison test:

**File:** `compiler/ori_llvm/tests/aot/borrowing_equivalence.rs` (temporary, deleted after migration)

```rust
#[test]
fn borrowing_set_equivalence() {
    let interner = StringInterner::new();

    // Old: from ori_llvm BuiltinTable
    let old_set = ori_llvm::codegen::arc_emitter::borrowing_builtin_names(&interner);

    // New: from ori_registry
    let new_set: FxHashSet<Name> = ori_registry::borrowing_method_names()
        .iter()
        .map(|name| interner.intern(name))
        .collect();

    // Convert to sorted strings for readable diff
    let old_names: BTreeSet<&str> = old_set
        .iter()
        .map(|n| interner.try_lookup(*n).unwrap())
        .collect();
    let new_names: BTreeSet<&str> = new_set
        .iter()
        .map(|n| interner.try_lookup(*n).unwrap())
        .collect();

    let only_old: Vec<_> = old_names.difference(&new_names).collect();
    let only_new: Vec<_> = new_names.difference(&old_names).collect();

    assert!(
        only_old.is_empty() && only_new.is_empty(),
        "Borrowing sets diverge!\nOnly in old: {:?}\nOnly in new: {:?}",
        only_old,
        only_new
    );
}
```

Run this test **before** deleting the old functions. Once it passes, the old functions can be safely removed.

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

### Checklist

- [ ] Write temporary equivalence test (old set == new set)
- [ ] Run equivalence test and verify it passes
- [ ] `cargo t -p ori_arc` passes
- [ ] `cargo t -p ori_registry` passes
- [ ] `./llvm-test.sh` passes
- [ ] `cargo st` passes
- [ ] `./test-all.sh` passes
- [ ] `cargo b --release && ./test-all.sh` passes (release mode)
- [ ] `./clippy-all.sh` passes
- [ ] Delete temporary equivalence test after old functions are removed
- [ ] Final `./test-all.sh` after cleanup

---

## Exit Criteria

All of the following must be true before this section is marked complete:

1. **All 4 call sites updated:** `compile_common.rs` (x2), `evaluator.rs`, `function_compiler/mod.rs` all use `ori_registry::borrowing_method_names()` for borrowing set construction
2. **`ori_llvm::codegen::arc_emitter::borrowing_builtin_names()` deleted:** Function and its `pub use` re-export removed
3. **`ori_ir::builtin_methods::borrowing_method_names()` deleted:** If no remaining callers
4. **`cargo check -p ori_arc` passes:** No compilation errors
5. **`cargo check -p ori_llvm` passes:** No compilation errors, no dead code warnings
6. **`cargo check -p oric` passes:** No compilation errors
7. **Iterator exclusion preserved:** `TypeTag::Iterator` methods and `.iter()` are NOT in the borrowing set
8. **Equivalence verified:** Old and new borrowing sets produce identical `FxHashSet<Name>` content
9. **No dependency cycles:** `cargo tree` shows no path from `ori_arc` or `ori_registry` back to `ori_llvm`
10. **`oric` does not import `ori_llvm::codegen::arc_emitter`** for borrowing data
11. **All tests pass:** `./test-all.sh` green, including release mode (`cargo b --release && ./test-all.sh`)
12. **Clippy clean:** `./clippy-all.sh` produces no new warnings
