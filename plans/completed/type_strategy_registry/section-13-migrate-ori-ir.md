---
plan: "type_strategy_registry"
section: "13"
title: "Migrate ori_ir & Legacy Consolidation"
status: complete
reviewed: true
goal: "Delete ori_ir::builtin_methods (now superseded by ori_registry), update all consumers to use ori_registry, confirm DerivedTrait and format spec stay in ori_ir — without breaking any tests"
depends_on:
  - "03"
  - "04"
  - "05"
  - "06"
  - "07"
  - "08"
subsections:
  - id: "13.1"
    title: "Field Mapping: ori_ir::MethodDef to ori_registry::MethodDef"
    status: complete
  - id: "13.2"
    title: "ReturnSpec Expressiveness Gap Analysis"
    status: complete
  - id: "13.3"
    title: "ParamSpec Expressiveness Gap Analysis"
    status: complete
  - id: "13.4"
    title: "Delete ori_ir::builtin_methods Module"
    status: complete
  - id: "13.5"
    title: "BuiltinType Deprecation Path"
    status: complete
  - id: "13.6"
    title: "DerivedTrait Alignment"
    status: complete
  - id: "13.7"
    title: "Format Spec Decision"
    status: complete
  - id: "13.8"
    title: "Update All ori_ir Consumers"
    status: complete
  - id: "13.9"
    title: "Validation & Regression"
    status: complete
---

# Section 13: Migrate ori_ir & Legacy Consolidation

**Context:** Sections 09-12 wire the four consuming crates (`ori_types`, `ori_eval`, `ori_arc`, `ori_llvm`) to read from `ori_registry` instead of their own hard-coded registries. This section handles the remaining piece: `ori_ir` itself has a builtin method registry (`BUILTIN_METHODS`, 123 entries across 861 lines) that predates `ori_registry`. With all consumers migrated, the `ori_ir` registry is dead weight — a second source of truth that serves no purpose and will inevitably drift.

> **Hygiene note:** `builtin_methods/mod.rs` at 861 lines exceeds the 500-line limit (BLOAT). Its module doc and field doc comments still claim "single source of truth" (lines 3, 83, 189) — stale since `ori_registry` now holds that role. The `MethodDef` struct derives only `Clone, Debug`, missing `PartialEq, Eq` per coding guidelines. All three issues are resolved by deletion (13.4), not individual fixes.

**Design rationale:** Option B (clean break) over Option A (re-export shim). The `ori_ir` `MethodDef`, `ParamSpec`, and `ReturnSpec` types are structurally different from `ori_registry`'s types (see 13.1-13.3). A compatibility shim would paper over these differences without eliminating them. A clean break is more work upfront but eliminates the drift risk permanently and removes ~945 lines of code.

**Ordering constraint:** This section depends on Sections 03-08 (registry populated with type definitions) and can execute independently of Sections 09-12. The only consumer of `ori_ir::builtin_methods` outside of `ori_ir` itself is `oric/src/eval/tests/methods/consistency.rs`. The four consuming crates (`ori_types`, `ori_eval`, `ori_arc`, `ori_llvm`) do not import from `ori_ir::builtin_methods`.

---

## 13.1 Field Mapping: ori_ir::MethodDef to ori_registry::MethodDef

The two `MethodDef` types serve the same purpose but have different structures. This subsection documents the exact mapping and identifies where they diverge.

### Field-by-Field Comparison

| ori_ir::MethodDef Field | Type | ori_registry::MethodDef Field | Type | Mapping |
|---|---|---|---|---|
| `receiver` | `BuiltinType` | *(implicit via `TypeDef.tag`)* | — | Two different concepts: ori_ir's `receiver: BuiltinType` identifies WHICH type (now implicit via `TypeDef.tag` in the registry). |
| `name` | `&'static str` | `name` | `&'static str` | **Identical.** |
| `params` | `&'static [ParamSpec]` | `params` | `&'static [ParamDef]` | See 13.3 for expressiveness gap. `ParamSpec::SelfType` maps to `ParamDef { ty: ReturnTag::SelfType, ... }`. |
| `returns` | `ReturnSpec` | `returns` | `ReturnTag` | See 13.2 for expressiveness gap. `ReturnSpec::SelfType` maps to `ReturnTag::SelfType`. `ReturnSpec::Type(BuiltinType::Int)` maps to `ReturnTag::Concrete(TypeTag::Int)`. |
| `trait_name` | `Option<&'static str>` | `trait_name` | `Option<&'static str>` | **Identical.** Preserved in registry for phases that need trait association (LLVM trait dispatch path). |
| `receiver_borrows` | `bool` | `receiver` | `Ownership` | `true` maps to `Ownership::Borrow`, `false` maps to `Ownership::Owned`. Per frozen decision 18, all primitive type receivers use `Ownership::Borrow` (not `Copy`). Renamed to `receiver` (the ownership of the receiver). |
| *(not in ori_ir)* | — | `pure` | `bool` | **New field.** Always `true` for primitive methods. |
| *(not in ori_ir)* | — | `backend_required` | `bool` | **New field.** `true` for methods both eval and LLVM must implement. |
| *(not in ori_ir)* | — | `kind` | `MethodKind` | **New field.** `Instance` for most; `Associated` for factory functions. |
| *(not in ori_ir)* | — | `dei_only` | `bool` | **New field.** `true` for DEI-only iterator methods. Always `false` for primitive methods. |
| *(not in ori_ir)* | — | `dei_propagation` | `DeiPropagation` | **New field.** Always `NotApplicable` for primitive methods. |

### Key Structural Differences

**1. Receiver is implicit, not explicit.**

In `ori_ir`, each `MethodDef` carries its `receiver: BuiltinType` field — the method knows which type it belongs to. In `ori_registry`, methods are nested inside `TypeDef.methods` — the receiver is the parent. This eliminates redundancy (a method on `INT` can only have receiver `Int`) but means the lookup path changes:

```rust
// BEFORE (ori_ir): method carries its receiver
let method = ori_ir::builtin_methods::find_method(BuiltinType::Int, "abs");
assert_eq!(method.receiver, BuiltinType::Int);

// AFTER (ori_registry): method lives inside its TypeDef
let method = ori_registry::find_method(TypeTag::Int, "abs");
// Receiver is TypeTag::Int (the TypeDef we looked up from)
```

**2. Ownership replaces bool.**

`receiver_borrows: bool` is replaced by `receiver: Ownership` (an enum: `Borrow`, `Owned`, `Copy`). This is strictly more expressive — `Copy` captures the semantic difference between "borrowed because it's a reference type" and "trivially copied because it's a value type." For the current 123 entries in `BUILTIN_METHODS`, every single one has `receiver_borrows: true`. Per frozen decision 18, all primitive type method receivers use `Ownership::Borrow` (not `Copy`) because the ARC pass checks `receiver == Borrow` to skip RC ops. Methods on Arc types (str, list, map, etc.) that borrow also use `Ownership::Borrow`; consuming methods (e.g., `option.unwrap()`, `iterator.collect()`) use `Ownership::Owned`. The `Copy` variant is reserved for future use.

**3. `trait_name` is preserved.**

Decision: `trait_name: Option<&'static str>` stays on `ori_registry::MethodDef`. The LLVM codegen trait dispatch path needs to distinguish `compare` (trait method on `Comparable`) from `abs` (direct method). Removing `trait_name` would force the LLVM backend to maintain its own mapping, recreating the drift problem we are eliminating.

### Migration Impact

No code changes in `ori_ir` itself for this mapping — `ori_ir::MethodDef` is being deleted, not modified. The mapping is used by consumers (13.8) when updating their imports.

### Checklist

- [x] For each of the 123 `BUILTIN_METHODS` entries, confirm a matching `MethodDef` exists in `ori_registry` with the same `(type, name)` pair: `grep` both registries and diff the `(receiver.name(), name)` sets (2026-03-09: verified — all 123 entries have matching registry entries; registry is superset with 257 total methods)
- [x] Confirm every `receiver_borrows: true` entry maps to `Ownership::Borrow` (all 123 entries have `receiver_borrows: true`) (2026-03-09: verified — all 123 map to Ownership::Borrow)
- [x] Confirm zero entries have `receiver_borrows: false` (there are none today; if any appear, they map to `Ownership::Owned`) (2026-03-09: verified — zero entries with receiver_borrows: false)
- [x] For each entry with `trait_name: Some(X)`, confirm the `ori_registry` entry has the same `trait_name` value (2026-03-09: verified — all trait_name values match exactly)

---

## 13.2 ReturnSpec Expressiveness Gap Analysis

`ori_ir::ReturnSpec` has 7 variants. `ori_registry` uses `ReturnTag` for return types (see Section 01). For primitive/compound types, `ReturnTag::Concrete(TypeTag)` wraps concrete types; for generic types, richer variants like `ReturnTag::ElementType`, `ReturnTag::OptionOf(TypeProjection)` etc. are used. This subsection analyzes whether the mapping from `ReturnSpec` is straightforward.

### Variant-by-Variant Analysis

| ori_ir::ReturnSpec | Current Usage | ori_registry Equivalent | Sufficient? |
|---|---|---|---|
| `SelfType` | `clone`, `abs`, `add`, `sub`, etc. — returns same type as receiver | `ReturnTag::SelfType` | Yes. `ReturnTag::SelfType` exists for exactly this purpose. |
| `Type(BuiltinType::Int)` | `str.len`, `Duration.nanoseconds`, `hash`, etc. — returns specific type | `ReturnTag::Concrete(TypeTag::Int)` (etc.) | Yes. Wrapped via `From<TypeTag>`. |
| `Type(BuiltinType::Ordering)` | `compare` — returns Ordering | `ReturnTag::Concrete(TypeTag::Ordering)` | Yes. Wrapped via `From<TypeTag>`. |
| `Void` | Not used in current `BUILTIN_METHODS` (zero entries) | `ReturnTag::Unit` | Yes. Never needed for the 123 entries being migrated. |
| `ElementType` | Not used in current `BUILTIN_METHODS` (zero entries) | N/A | Not needed. Used only for collection methods (List, Iterator, etc.) which are handled by Section 06/07. |
| `OptionElement` | Not used in current `BUILTIN_METHODS` (zero entries) | N/A | Not needed. Same — collection methods only. |
| `ListElement` | Not used in current `BUILTIN_METHODS` (zero entries) | N/A | Not needed. Same. |
| `InnerType` | Not used in current `BUILTIN_METHODS` (zero entries) | N/A | Not needed. Option/Result methods only. |

### Conclusion

**`ReturnTag::Concrete(TypeTag)` is fully sufficient for all 123 entries in BUILTIN_METHODS.** Every `ReturnSpec::Type(X)` maps to `ReturnTag::Concrete(TypeTag::X)` and every `ReturnSpec::SelfType` maps to `ReturnTag::SelfType`. The complex return spec variants (`ElementType`, `OptionElement`, `ListElement`, `InnerType`) are never used in the current `ori_ir` registry. They exist because `ori_ir` anticipated needing them for collection types, but collection types were never added to `BUILTIN_METHODS`. In `ori_registry`, collection types are handled by Sections 06-07, where `ReturnTag` variants (e.g., `ReturnTag::ElementType`, `ReturnTag::Fresh`) capture the structural return type templates, and the type checker's existing inference logic handles closure-dependent return types.

### Decision: ReturnTag for all return types

- **Registry `MethodDef.returns`**: Always `ReturnTag` (Section 01 frozen schema). For primitive/compound types, this is `ReturnTag::Concrete(TypeTag::Int)` etc. For generic types, richer variants like `ReturnTag::ElementType`, `ReturnTag::OptionOf(TypeProjection::Element)`, `ReturnTag::Fresh` are used (see Section 01).
- **No need for ReturnSpec in ori_registry.** The legacy `ori_ir::ReturnSpec` type is retired.

### Checklist

- [x] Verify zero uses of `ElementType`, `OptionElement`, `ListElement`, `InnerType` in `BUILTIN_METHODS` (2026-03-09: verified — zero uses of all four variants)
- [x] Verify all `ReturnSpec::SelfType` usages map to `ReturnTag::SelfType` (2026-03-09: verified — ~50 uses, all map correctly)
- [x] Verify all `ReturnSpec::Type(X)` usages map to `ReturnTag::Concrete(TypeTag::X)` (2026-03-09: verified — Int, Bool, Ordering, Str variants all map correctly)

---

## 13.3 ParamSpec Expressiveness Gap Analysis

`ori_ir::ParamSpec` has 6 variants. `ori_registry` uses `ParamDef` with `ReturnTag` + `Ownership` (see Section 01). This subsection analyzes the mapping.

### Variant-by-Variant Analysis

| ori_ir::ParamSpec | Current Usage (BUILTIN_METHODS) | ori_registry Equivalent | Sufficient? |
|---|---|---|---|
| `SelfType` | `compare(other: Self)`, `add(other: Self)`, `min(other: Self)` | `ParamDef { ty: ReturnTag::SelfType, ownership: Ownership::Borrow }` | Yes. `ReturnTag::SelfType` maps exactly. |
| `Int` | `Duration.mul(n: int)`, `Duration.div(n: int)`, `Size.mul(n: int)`, `Size.div(n: int)` | `ParamDef { name: "n", ty: ReturnTag::Concrete(TypeTag::Int), ownership: Ownership::Copy }` | Yes. Direct mapping. |
| `Str` | `str.contains(s: str)`, `str.starts_with(s: str)`, `str.ends_with(s: str)`, `str.concat(s: str)`, `str.add(s: str)` | `ParamDef { name: "s", ty: ReturnTag::Concrete(TypeTag::Str), ownership: Ownership::Borrow }` | Yes. Direct mapping. |
| `Bool` | Not used in current `BUILTIN_METHODS` (zero entries) | `ParamDef { name: _, ty: ReturnTag::Concrete(TypeTag::Bool), ... }` | Yes, if ever needed. |
| `Any` | Not used in current `BUILTIN_METHODS` (zero entries) | Not needed for primitives. For generics, the type checker handles polymorphism separately. | Not needed for this migration. |
| `Closure` | Not used in current `BUILTIN_METHODS` (zero entries) | Not needed for primitives. Iterator/collection closure inference stays in the type checker. | Not needed for this migration. |

### Conclusion

**ParamDef with ReturnTag is fully sufficient for all 123 entries in BUILTIN_METHODS.** Only three ParamSpec variants are actually used: `SelfType` (most common — trait methods and arithmetic), `Int` (Duration/Size multiplication/division), and `Str` (string comparison/contains methods). All three map directly to `ParamDef` with the corresponding `ReturnTag::SelfType` or `ReturnTag::Concrete(TypeTag)`.

The complex variants (`Any`, `Closure`, `Bool`) are unused in `BUILTIN_METHODS`. In `ori_registry`, collection and iterator methods that take closures use `ParamDef` with `ReturnTag::Fresh` for parameter declaration; the actual closure type inference stays in the type checker.

### Decision: ParamDef with ReturnTag + Ownership

No compatibility shim needed. The mapping is:
- `ParamSpec::SelfType` --> `ParamDef { name: "other", ty: ReturnTag::SelfType, ownership: Ownership::Borrow }`
- `ParamSpec::Int` --> `ParamDef { name: "n", ty: ReturnTag::Concrete(TypeTag::Int), ownership: Ownership::Copy }`
- `ParamSpec::Str` --> `ParamDef { name: "s", ty: ReturnTag::Concrete(TypeTag::Str), ownership: Ownership::Borrow }`

### Checklist

- [x] Verify zero uses of `ParamSpec::Bool`, `ParamSpec::Any`, `ParamSpec::Closure` in `BUILTIN_METHODS` (2026-03-09: verified — zero uses of all three variants)
- [x] Verify all `ParamSpec::SelfType` map to `ParamDef { ty: ReturnTag::SelfType, ... }` (2026-03-09: verified — 28 uses, all map correctly)
- [x] Verify all `ParamSpec::Int` map to `ParamDef { ty: ReturnTag::Concrete(TypeTag::Int), ... }` (2026-03-09: verified — 5 uses for Duration/Size mul/div and str.repeat)
- [x] Verify all `ParamSpec::Str` map to `ParamDef { ty: ReturnTag::Concrete(TypeTag::Str), ... }` (2026-03-09: verified — 6 uses for str contains/starts_with/ends_with/add/concat/replace)

---

## 13.4 Delete ori_ir::builtin_methods Module

This is the core deletion. The module spans 945 lines across two files and defines types, constants, and query functions that are fully superseded by `ori_registry`.

### What Gets Deleted

| File | Lines | Contents |
|---|---|---|
| `compiler/ori_ir/src/builtin_methods/mod.rs` | 861 | `ParamSpec` enum, `ReturnSpec` enum, `MethodDef` struct + impl, `BUILTIN_METHODS` static (123 entries), 4 query functions |
| `compiler/ori_ir/src/builtin_methods/tests.rs` | 84 | 7 unit tests for the query functions |

**Total deleted: ~945 lines.**

### Types Retired

| Type | Replacement | Notes |
|---|---|---|
| `ori_ir::builtin_methods::ParamSpec` | `ori_registry::ParamDef` | Different structure (enum vs struct), same purpose |
| `ori_ir::builtin_methods::ReturnSpec` | `ori_registry::ReturnTag` (wraps `TypeTag` via `Concrete()`) | Superset — adds `ElementType`, `OptionOf`, `ListOf`, `Fresh`, etc. for generic types |
| `ori_ir::builtin_methods::MethodDef` | `ori_registry::MethodDef` | Different structure (receiver on MethodDef vs on parent TypeDef) |

### Query Functions Retired

| ori_ir Function | Replacement | Notes |
|---|---|---|
| `find_method(BuiltinType, &str) -> Option<&MethodDef>` | `ori_registry::find_method(TypeTag, &str) -> Option<&MethodDef>` | Same signature pattern, different types |
| `methods_for(BuiltinType) -> impl Iterator<Item = &MethodDef>` | `ori_registry::find_type(tag).map(|t| t.methods.iter())` | Navigates through TypeDef |
| `has_method(BuiltinType, &str) -> bool` | `ori_registry::find_method(tag, name).is_some()` | Trivial wrapper, inline at call sites |
| `method_names_for(BuiltinType) -> impl Iterator<Item = &str>` | `ori_registry::find_type(tag).map(|t| t.methods.iter().map(|m| m.name))` | Navigates through TypeDef |

### Static Retired

| Static | Replacement | Notes |
|---|---|---|
| `BUILTIN_METHODS: &[MethodDef]` (123 entries) | `ori_registry::BUILTIN_TYPES: &[&TypeDef]` (with nested methods) | Structural change: flat list --> nested by type |

### Internal Dependency Check

Before deletion, confirm that no OTHER module inside `ori_ir/src/` imports from `builtin_methods`. Verified via grep: only `lib.rs:41` (`pub mod builtin_methods;`) references it. The `builtin_type/` module does NOT depend on `builtin_methods/` — `BuiltinType` is defined independently in `builtin_type/mod.rs` and is *used by* `builtin_methods/mod.rs`, not the reverse. This means the deletion is safe with no intra-crate cascading.

### Deletion Steps (ordered)

1. **Verify no remaining consumers** (prerequisite from 13.8) — `grep -r 'ori_ir::builtin_methods' compiler/` returns zero hits outside of `ori_ir` itself
2. **Verify no intra-crate consumers** — `grep -r 'use crate::builtin_methods\|use super::builtin_methods' compiler/ori_ir/src/` returns zero hits (excluding `builtin_methods/` itself)
3. **Delete `compiler/ori_ir/src/builtin_methods/tests.rs`** — the entire file
4. **Delete `compiler/ori_ir/src/builtin_methods/mod.rs`** — the entire file
5. **Remove directory** `compiler/ori_ir/src/builtin_methods/` (if now empty)
6. **Update `compiler/ori_ir/src/lib.rs`** — remove `pub mod builtin_methods;` from line 41
7. **Verify** `cargo c -p ori_ir` compiles clean
8. **Verify** `cargo doc -p ori_ir` builds clean (no broken intra-doc links)

### BEFORE / AFTER for lib.rs

**BEFORE** (`compiler/ori_ir/src/lib.rs`, lines 38-42):
```rust
mod arena;
pub mod ast;
pub mod builtin_constants;
pub mod builtin_methods;
mod builtin_type;
```

**AFTER:**
```rust
mod arena;
pub mod ast;
pub mod builtin_constants;
mod builtin_type;
```

One line removed. No other changes to `lib.rs`.

### Checklist

- [x] `grep -r 'ori_ir::builtin_methods' compiler/` returns only ori_ir-internal hits (tests.rs, mod.rs) (2026-03-09: verified zero hits — consumer in consistency.rs removed first)
- [x] `grep -r 'builtin_methods::' compiler/` returns zero hits outside ori_ir (2026-03-09: verified zero hits)
- [x] `grep -r 'use crate::builtin_methods\|use super::builtin_methods' compiler/ori_ir/src/` returns zero hits (excluding builtin_methods/ itself) (2026-03-09: verified zero hits)
- [x] Delete `compiler/ori_ir/src/builtin_methods/tests.rs` (2026-03-09)
- [x] Delete `compiler/ori_ir/src/builtin_methods/mod.rs` (2026-03-09)
- [x] Remove `compiler/ori_ir/src/builtin_methods/` directory (2026-03-09)
- [x] Remove `pub mod builtin_methods;` from `compiler/ori_ir/src/lib.rs` (2026-03-09)
- [x] `cargo c -p ori_ir` compiles clean with zero warnings (2026-03-09)
- [x] `cargo doc -p ori_ir` builds clean with no broken intra-doc links (2026-03-09: verified clean)
- [x] `cargo t -p ori_ir` passes (remaining tests unaffected) (2026-03-09)

---

## 13.5 BuiltinType Deprecation Path

`ori_ir::BuiltinType` is a separate enum from `ori_registry::TypeTag`, but they cover overlapping ground. This subsection decides what happens to `BuiltinType`.

### Current State

`ori_ir::BuiltinType` (18 variants): `Int`, `Float`, `Bool`, `Str`, `Char`, `Byte`, `Unit`, `Never`, `Duration`, `Size`, `Ordering`, `List`, `Map`, `Option`, `Result`, `Range`, `Set`, `Channel`.

`ori_registry::TypeTag` (designed in Section 01): covers the same types, possibly with different variant naming.

### Key Difference: BuiltinType Has TypeId Conversions

`BuiltinType` provides `from_type_id(TypeId) -> Option<Self>` and `type_id(self) -> Option<TypeId>`. These bridge the IR representation (`TypeId`, a `u32` index) to a high-level type identity. `TypeTag` does not have this — it is a pure-data tag with no dependency on `TypeId` (which lives in `ori_ir`).

### Decision: Keep BuiltinType in ori_ir, No Bridge Function Needed Today

**BuiltinType stays in `ori_ir`.** Rationale:

1. **BuiltinType depends on TypeId** (`from_type_id`, `type_id`). TypeId is an `ori_ir` type. Moving BuiltinType to `ori_registry` would force `ori_registry` to depend on `ori_ir`, violating the purity contract.

2. **The two enums serve different layers.** `BuiltinType` is an IR-level type identity (coupled to `TypeId`). `TypeTag` is a registry-level type tag (pure data, no coupling). They are complementary, not redundant.

### Bridge Function Analysis

**No `builtin_type_to_tag()` bridge function is needed for this section.** Codebase grep confirms:

- `BuiltinType` is used **only within `ori_ir` itself**: `builtin_type/mod.rs` (definition), `builtin_type/tests.rs`, `builtin_methods/mod.rs` (being deleted), `builtin_methods/tests.rs` (being deleted), and `lib.rs` (re-export).
- **Zero consuming crates** (`ori_types`, `ori_eval`, `ori_arc`, `ori_llvm`, `oric`) import or reference `BuiltinType`.
- After `builtin_methods` deletion, `BuiltinType` will have no consumers outside its own definition module and `lib.rs` re-export. The `pub use builtin_type::BuiltinType;` in `lib.rs` becomes dead public API.

The original plan stated "BuiltinType is used widely outside the method registry" and proposed a bridge function "in any consuming crate that needs to convert." This was incorrect — the consumers were already migrated to `TypeTag` in Sections 09-12, or never used `BuiltinType` in the first place.

### Dead Re-Export Note

After deletion of `builtin_methods`, the `pub use builtin_type::BuiltinType;` re-export in `lib.rs` will have zero external consumers. Options:

1. **Leave as-is** (conservative) — the re-export is harmless and removal is tangential to this plan's goal.
2. **Demote to `pub(crate)`** — reduces public surface area. Requires verifying no external consumer needs it (already confirmed: zero).
3. **Remove BuiltinType entirely** — delete `builtin_type/` module and re-export. Requires removing `BuiltinType::from_type_id` / `BuiltinType::type_id` usages within ori_ir tests.

**Recommendation:** Option 1 (leave as-is) for this section. Full deprecation is a separate effort if desired.

### Variant Coverage Note (for future bridge if ever needed)

`TypeTag` has 23 variants; `BuiltinType` has 18 variants. The 5 TypeTag variants with NO `BuiltinType` equivalent are: `Error`, `Tuple`, `Function`, `Iterator`, `DoubleEndedIterator`. A future bridge function would need to handle only the 18 shared variants; code paths encountering the 5 extra TypeTag variants would return `None` from a `try_from` conversion. This is moot for the `builtin_methods` deletion since those 5 types were never IN `BUILTIN_METHODS` anyway.

### Checklist

- [x] Verify `BuiltinType` is NOT used in `builtin_methods/mod.rs` query function signatures (it is — as receiver type — but those functions are being deleted) (2026-03-09: verified — BuiltinType used as receiver in find_method/methods_for query signatures)
- [x] Verify `BuiltinType` has uses OUTSIDE `builtin_methods/` (2026-03-09: verified — defined in builtin_type/mod.rs, re-exported from lib.rs. Correction: NO consuming crate uses it — only ori_ir-internal)
- [x] Confirm decision: BuiltinType stays in ori_ir (2026-03-09: confirmed — depends on TypeId from ori_ir, cannot move to ori_registry without violating purity)
- [x] Verify no consuming crate needs `builtin_type_to_tag()` bridge (2026-03-09: confirmed — grep shows zero BuiltinType references in ori_types, ori_eval, ori_arc, ori_llvm, oric)
- [x] Verify `cargo c -p ori_ir` passes after `builtin_methods` deletion (BuiltinType still compiles because it does not depend on the deleted module) (2026-03-09: verified — clean compilation)
- [x] Verify `pub use builtin_type::BuiltinType;` in lib.rs has no external consumers after deletion (dead public API — acceptable, see Dead Re-Export Note) (2026-03-09: verified — grep shows zero external references)

### Cleanup

- [x] **[WASTE]** `compiler/ori_ir/src/builtin_type/mod.rs:3-4` — Module doc updated to reflect actual scope: "IR-level type identity enum, bridging `TypeId` indices to named variants within `ori_ir`." (2026-03-09)

---

## 13.6 DerivedTrait Alignment

`DerivedTrait` lives in `ori_ir::derives` and has a 4-way sync contract with `ori_types`, `ori_eval`, `ori_llvm`, and `library/std`. This subsection decides how `DerivedTrait` relates to `ori_registry`.

### Current State

`DerivedTrait` (7 variants): `Eq`, `Clone`, `Hashable`, `Printable`, `Debug`, `Default`, `Comparable`.

Defined via `define_derived_traits!` macro in `compiler/ori_ir/src/derives/mod.rs`. The macro generates `from_name()`, `method_name()`, `trait_name()`, `shape()`, `requires_supertrait()`, `supports_sum_types()`, and `ALL`/`COUNT` constants.

Each derived trait produces a method (e.g., `Eq` produces `equals`, `Clone` produces `clone`). These methods appear in the `ori_registry` type definitions as regular `MethodDef` entries with `trait_name` set (e.g., `MethodDef { name: "clone", trait_name: Some("Clone"), ... }`).

**Note:** `DerivedTrait::Eq.method_name()` returns `"eq"` (the derivation strategy method), while the builtin registry uses `"equals"` (the trait spec method name). Both refer to the same `Eq` trait but use different identifiers. The registry's `"equals"` matches the Ori spec (`trait Eq { @equals (self, other: Self) -> bool }`).

### Decision: DerivedTrait Stays in ori_ir

**DerivedTrait does NOT move to ori_registry.** Rationale:

1. **DerivedTrait is about code generation, not type behavioral specification.** It describes HOW to auto-implement a trait for user-defined types (`DeriveStrategy`, `FieldOp`, `CombineOp`). The registry describes WHAT methods exist on builtin types. These are orthogonal concerns.

2. **DerivedTrait has non-data behavior.** The `strategy()` method returns a `DeriveStrategy` with composition logic. This violates the `ori_registry` purity contract (no functions with logic).

3. **DerivedTrait has dependencies.** It uses `crate::Name` (interned strings from `ori_ir`). The registry has zero dependencies.

4. **The 4-way sync contract is about derived traits on user types, not builtins.** Builtin types have hard-coded trait implementations (e.g., `int.compare` is a native LLVM comparison, not a derived field-by-field comparison). The derived trait machinery is for `type Point = (x: int, y: int) derives(Eq, Clone)`.

### Interaction Between DerivedTrait and Registry

Derived trait methods appear in the registry as regular method entries. For example, `INT.methods` contains `MethodDef { name: "compare", trait_name: Some("Comparable"), ... }`. This entry declares that `int` has a `compare` method associated with the `Comparable` trait. The registry does not care whether this method is implemented natively (builtins) or via derivation (user types) — that distinction belongs to the backends.

The `trait_name` field on `ori_registry::MethodDef` is the connection point. When LLVM codegen encounters a `compare` call, it checks `trait_name`:
- If `Some("Comparable")` and the receiver is a builtin type, it emits native comparison.
- If `Some("Comparable")` and the receiver is a user type with `derives(Comparable)`, it uses the `DeriveStrategy`.

This separation is clean: the registry says "this method exists and is associated with this trait." The `DerivedTrait` machinery says "for user types, here is how to auto-generate this method."

### Checklist

- [x] Verify `DerivedTrait` has zero references to `builtin_methods` module (it does not reference it) (2026-03-09: verified — zero imports or references)
- [x] Verify `derives/mod.rs` compiles independently of `builtin_methods/mod.rs` (2026-03-09: verified — imports only crate::Name and strategy submodule)
- [x] Confirm decision: DerivedTrait stays in ori_ir, unchanged (2026-03-09: confirmed — has non-data behavior, depends on Name, orthogonal to registry)
- [x] Verify `trait_name` values on `ori_registry::MethodDef` entries match `DerivedTrait::trait_name()` for corresponding traits (e.g., `"Comparable"` matches `DerivedTrait::Comparable.trait_name()`) (2026-03-09: verified — all 7 derived trait_name values match exactly: Eq, Clone, Hashable, Printable, Debug, Default, Comparable)

---

## 13.7 Format Spec Decision

`FormatType`, `Align`, `Sign`, `ParsedFormatSpec`, and `parse_format_spec()` live in `ori_ir::format_spec`. They have a 4-way sync contract with `ori_types`, `ori_eval`, and `ori_rt`. This subsection decides what happens to them.

### Current State

- **`ori_ir::format_spec`** (299 lines): Defines the enums (`FormatType`, `Align`, `Sign`), the `ParsedFormatSpec` struct, and the `parse_format_spec()` parser function.
- **Consumers**: `ori_types` (validates format type vs expression type), `ori_eval` (applies formatting at runtime), `ori_rt` (runtime format calls, guarded by its own variant count test).
- **Consistency tests**: `compiler/oric/src/eval/tests/methods/consistency.rs` has 6 tests verifying variant sync between `ori_ir::format_spec` and `ori_types`/`ori_eval` registrations (lines 394-470).

### Decision: Format Spec Stays in ori_ir

**Format spec types do NOT move to ori_registry.** Rationale:

1. **`parse_format_spec()` is a parser function with logic.** It contains 120+ lines of parsing logic (alignment, sign, width, precision, type character). This violates the `ori_registry` purity contract.

2. **`FormatSpecError` contains `String`.** The error type uses heap allocation (`TrailingCharacters(String)`, `InvalidWidth(String)`). This violates the registry's no-heap-allocation constraint.

3. **Format specs are about syntax, not type behavior.** They describe how to format a value in a template string (`{x:>10.2f}`), not what methods exist on a type. The registry is about type behavioral contracts.

4. **The existing sync mechanism works.** The consistency tests in `consistency.rs` verify variant alignment between `ori_ir`, `ori_types`, `ori_eval`, and `ori_rt`. This is the same pattern the registry eliminates for method metadata — but format specs have only 3 small enums (8 + 3 + 3 = 14 variants total), and the sync tests are cheap and reliable.

### Future Consideration

If `ori_registry` ever gains a `FormattingDefs` struct on `TypeDef` (declaring which format types are valid for which type — e.g., `int` supports `Binary`/`Octal`/`Hex` but `str` does not), the enum definitions could move to `ori_registry` and the parser could stay in `ori_ir`. But this is speculative and not part of this plan.

### Impact on Consistency Tests

The 6 format spec consistency tests in `consistency.rs` (lines 394-470) use `ori_ir::format_spec::FormatType`, `ori_ir::format_spec::Align`, and `ori_ir::format_spec::Sign`. These imports are UNAFFECTED by the deletion of `builtin_methods`. The format spec module is a separate module in `ori_ir` (`format_spec.rs`), not part of `builtin_methods/`.

### Checklist

- [x] Verify `format_spec.rs` has zero dependencies on `builtin_methods/` (confirmed: no import) (2026-03-09: verified — imports only std::fmt)
- [x] Verify format spec consistency tests compile after `builtin_methods` deletion (2026-03-09: verified — consistency tests use ori_ir::format_spec directly, no builtin_methods reference)
- [x] Confirm decision: format spec stays in ori_ir, unchanged (2026-03-09: confirmed — contains parsing logic, heap allocation in errors, orthogonal to registry)
- [x] ~~Add a plan-reference comment in `format_spec.rs`~~ Removed — plan references in source code are noise per hygiene rules; the decision is documented here

---

## 13.8 Update All ori_ir Consumers

> **Complexity warning:** This subsection has the most moving parts — code changes in `consistency.rs` (deleting ~220 lines), stale comment cleanup across 4 files, doc updates across 5 design docs, and MEMORY.md updates. Execute the code changes first, run `cargo t -p oric` to verify, then do the doc/comment cleanup. The doc updates are mechanical but spread across many files — use grep verification after each batch.

With `builtin_methods` deleted, all code that previously imported from `ori_ir::builtin_methods` must be updated. This subsection identifies every consumer and specifies the exact change.

### Consumer Inventory

Grep results for `ori_ir::builtin_methods` in the compiler directory (excluding `ori_ir` itself and plan documents):

| File | Import | Used For |
|---|---|---|
| `compiler/oric/src/eval/tests/methods/consistency.rs:12` | `use ori_ir::builtin_methods::BUILTIN_METHODS;` | Building `ir_method_set()` for cross-phase consistency tests |

This is the **only** runtime consumer. All other references are in documentation, plan files, or the `ori_ir` crate itself.

### Consumer 1: consistency.rs (Primary Migration Target)

**File:** `compiler/oric/src/eval/tests/methods/consistency.rs`

**Current usage:**
```rust
use ori_ir::builtin_methods::BUILTIN_METHODS;

fn ir_method_set() -> BTreeSet<(&'static str, &'static str)> {
    BUILTIN_METHODS
        .iter()
        .map(|m| (m.receiver.name(), m.name))
        .collect()
}
```

This function builds a set of `(type_name, method_name)` pairs from the IR registry for use in 1 consistency test:
- `registry_primitive_methods_in_ir()` — every registry method for primitive types should be in the IR registry

**BEFORE** (current state):
```rust
use ori_ir::builtin_methods::BUILTIN_METHODS;  // line 12

/// Build the set of `(type_name, method_name)` from the IR registry.
fn ir_method_set() -> BTreeSet<(&'static str, &'static str)> {  // lines 35-41 (incl. doc comment)
    BUILTIN_METHODS
        .iter()
        .map(|m| (m.receiver.name(), m.name))
        .collect()
}
```

**AFTER** (delete both the import and the function):

The `registry_method_pairs()` function already exists (line 20) and is the registry-based replacement. The `ir_method_set()` function and its `BUILTIN_METHODS` import are simply deleted. The `registry_primitive_methods_in_ir` test that calls `ir_method_set()` is also deleted since the registry IS the source of truth.

**Allowlist Elimination**

When `ori_registry` becomes the single source of truth, the remaining allowlists in this file become unnecessary:

| Allowlist | Lines | Status After Migration |
|---|---|---|
| `COLLECTION_TYPES` (11 entries) | 43-59 (incl. doc comment) | **Eliminated if** all collection types are in the registry (they are — Sections 06-07 complete). Remove this allowlist and its doc comment. |
| `TYPECK_METHODS_NOT_IN_IR` (146 entries) | 61-227 (incl. doc comment) | **Eliminated.** Registry is the superset — it includes methods from all phases. The `ir_method_set()` function and the `registry_primitive_methods_in_ir` test that uses this allowlist are both deleted. |
| `WELL_KNOWN_GENERIC_TYPES` (7 entries) | 477-485 | **Unchanged.** This tracks well-known generic type resolution consistency in `ori_types`, unrelated to the IR method registry. |

**Net impact on consistency.rs:** The 1 IR-related test (`registry_primitive_methods_in_ir`) is deleted along with `ir_method_set()`, `COLLECTION_TYPES`, and `TYPECK_METHODS_NOT_IN_IR`. The remaining 9 tests (registry sorting, iterator consistency, 6 format spec tests, well-known generics) are untouched.

**Post-migration line count:** Current file is 538 lines. Deleting import (1), `ir_method_set` + doc comment (7, lines 35-41), `COLLECTION_TYPES` + doc comment (17, lines 43-59), the comment block above `TYPECK_METHODS_NOT_IN_IR` (9, lines 60-68), `TYPECK_METHODS_NOT_IN_IR` (159, lines 69-227), section comment (1, line 247), `registry_primitive_methods_in_ir` test + doc + blank (25, lines 248-272), and updating module doc (1) removes ~220 lines, leaving ~318 lines. This is well under the 500-line limit (though test files are exempt). The file is clean post-migration.

**Module doc update:** The `//!` header on lines 1-2 says "and the `ori_ir` builtin method registry." After deletion, update to: "Tests for consistency between the evaluator, `ori_registry` type definitions, and format spec registration." Remove the IR reference.

**Note on Sections 09-10:** Sections 09-10 (wire typeck, wire eval) are already complete. The allowlists that originally tracked eval-vs-typeck and eval-vs-IR drift (`EVAL_METHODS_NOT_IN_TYPECK`, `TYPECK_METHODS_NOT_IN_EVAL`, `EVAL_METHODS_NOT_IN_IR`, `IR_METHODS_DISPATCHED_VIA_RESOLVERS`) were eliminated during those sections.

### Consumer 2: ori_arc (Indirect — Already Migrated by Section 11)

`ori_arc` does not import from `ori_ir::builtin_methods` directly. It receives `borrowing_builtins: &FxHashSet<Name>` as a parameter from `ori_llvm`. Section 11 (Wire ARC/Borrow) changes the source of this set from `ori_llvm`'s hard-coded list to `ori_registry` queries. No changes needed in this section.

### Consumer 3: ori_llvm (Indirect — Already Migrated by Section 12)

`ori_llvm` references `TYPECK_BUILTIN_METHODS` (from `ori_types`) in its consistency tests, not `ori_ir::builtin_methods`. Section 12 (Wire LLVM Backend) migrates these to `ori_registry`. No changes needed in this section for the LLVM crate itself.

### Consumer 4: Stale Comments & Documentation (Post-Deletion Cleanup)

After the deletion of `builtin_methods` and its allowlists, several files have comments or doc strings that reference the deleted module, deleted allowlists, or deleted functions. These become misleading and must be updated.

| File | Line(s) | Stale Reference | Action |
|---|---|---|---|
| `compiler/oric/src/eval/tests/methods/dispatch_coverage.rs` | 18-21 | Doc comment paragraph: "`TYPECK_METHODS_NOT_IN_IR` in `consistency.rs` covers the narrower registry-vs-IR gap. Both allowlists will be eliminated by Section 13." | Delete lines 18-21 entirely (the cross-reference paragraph). The remaining doc comment (lines 14-17: "This list must shrink monotonically...") is self-contained. |
| `compiler/ori_registry/src/query/mod.rs` | 294-298 | Doc on `legacy_type_name()`: "used by `BUILTIN_METHODS` (IR) and LLVM codegen" and "compatibility layer until the IR registry (`BUILTIN_METHODS`) migrates to `PascalCase` naming (Section 13)" | Replace lines 294-298 with: `/// Map registry PascalCase type names to the lowercase convention` / `/// used by LLVM codegen tests and eval consistency tests.` Remove the `BUILTIN_METHODS` reference and the "compatibility layer" sentence. (Note: `legacy_type_name` is used only in test code, not production code.) |
| `.claude/rules/ir.md` | 61 | "- `builtin_methods/`: Built-in method name constants" | Delete this line. The module no longer exists. |
| `compiler/oric/src/eval/tests/methods/consistency.rs` | 62, 68-69 | Comments referencing `ori_ir/src/builtin_methods/mod.rs` (lines become irrelevant after the function/allowlist are deleted) | These lines are deleted as part of 13.8 (ir_method_set + TYPECK_METHODS_NOT_IN_IR deletion), so no separate action needed — but verify they don't survive as orphaned comments. |

**Note:** `dispatch_coverage.rs` does NOT import from `ori_ir::builtin_methods` — only its doc COMMENT references it. This is a documentation fix only, not a code change.

### Checklist (Stale Comment Cleanup)

- [x] Delete `dispatch_coverage.rs` lines 18-21 (the `TYPECK_METHODS_NOT_IN_IR` cross-reference paragraph) (2026-03-09)
- [x] Update `ori_registry/src/query/mod.rs` `legacy_type_name()` doc to remove `BUILTIN_METHODS` reference (2026-03-09)
- [x] Delete `builtin_methods/` line from `.claude/rules/ir.md` (2026-03-09)
- [x] **[DRIFT]** `.claude/rules/ir.md` — Key Files section updated: removed `builtin_methods/`, added `builtin_type/` and `format_spec.rs` (2026-03-09)
- [x] Verify no orphaned comments remain in `consistency.rs` after allowlist deletion (2026-03-09: grep confirms zero references)

### Consumer 5: docs/compiler/design/ Stale References

Several design documentation files reference `TYPECK_BUILTIN_METHODS`, `EVAL_BUILTIN_METHODS`, or `ori_ir::builtin_methods` — all of which are eliminated by Sections 09-10 and this section. These become misleading after the migration.

| File | Stale References | Action |
|---|---|---|
| `docs/compiler/design/05-type-system/type-registry.md` | References `TYPECK_BUILTIN_METHODS` constant and code sample (line 291-294) | Update to describe `ori_registry::BUILTIN_TYPES` as the manifest |
| `docs/compiler/design/05-type-system/index.md` | References `TYPECK_BUILTIN_METHODS` constant array (line 227) | Update to reference `ori_registry::BUILTIN_TYPES` |
| `docs/compiler/design/10-llvm-backend/builtins-codegen.md` | References `TYPECK_BUILTIN_METHODS` sync tests, sorted-list constraint (lines 46-48, 204, 216) | Update to describe `ori_registry` as the sync mechanism |
| `docs/compiler/design/08-evaluator/index.md` | References `EVAL_BUILTIN_METHODS`, `TYPECK_BUILTIN_METHODS`, and consistency test pattern (line 228) | Update to describe `ori_registry`-based consistency |
| `docs/compiler/design/appendices/E-coding-guidelines.md` | References `TYPECK_BUILTIN_METHODS` as example of sorted validated lists (line 382) | Update example to `BUILTIN_TYPES` or remove the specific reference |

**Note:** These documentation updates are lower priority than the code changes but should be done before Section 14 (which declares the plan complete). Stale docs that describe eliminated architecture will confuse future readers.

### Consumer 6: MEMORY.md Stale References

`~/.claude/projects/-home-eric-projects-ori-lang/memory/MEMORY.md` contains two stale references:
- Line 34: "`TYPECK_BUILTIN_METHODS` must be sorted alphabetically by (type, method) — enforced by test" — this constant was eliminated in Section 09.
- Line 99: "8. `TYPECK_BUILTIN_METHODS` — add `("Iterator", "zip")`" (in "Adding a new iterator method" steps) — this step is now "add to `ori_registry` type definition."

These should be updated to reference `ori_registry::BUILTIN_TYPES` or removed, since `TYPECK_BUILTIN_METHODS` no longer exists.

### oric Cargo.toml Update

After migration, `oric` needs `ori_registry` in its dependencies (already added in Section 02.4). Verify it is present:

```toml
# compiler/oric/Cargo.toml
[dependencies]
ori_registry.workspace = true   # Added in Section 02
```

If not yet present, add it.

### Checklist

- [x] Remove `use ori_ir::builtin_methods::BUILTIN_METHODS;` import (line 12) (2026-03-09)
- [x] Delete `ir_method_set()` function and its doc comment (lines 35-41) (2026-03-09)
- [x] Delete `COLLECTION_TYPES` allowlist and its doc comment (lines 43-59) — all collection types are in registry (2026-03-09)
- [x] Delete `TYPECK_METHODS_NOT_IN_IR` allowlist (lines 69-227, 146 entries) — doc comment at lines 61-68 also deleted (2026-03-09)
- [x] Delete the comment block above `TYPECK_METHODS_NOT_IN_IR` (lines 60-68) that references `ori_ir/src/builtin_methods/mod.rs` (2026-03-09)
- [x] Delete `registry_primitive_methods_in_ir` test including its `#[test]` attr and doc comment (lines 249-272) (2026-03-09)
- [x] Delete the section comment "Registry ↔ IR alignment (kept until Section 13)" (line 247) (2026-03-09)
- [x] Update `consistency.rs` module doc (lines 1-2) to remove "and the `ori_ir` builtin method registry" reference (2026-03-09)
- [x] Verify remaining 9 tests compile and pass (they do not reference `ir_method_set` or `BUILTIN_METHODS`) (2026-03-09: all 9 pass)
- [x] Complete all items in "Checklist (Stale Comment Cleanup)" above (Consumer 4: `dispatch_coverage.rs`, `legacy_type_name()` doc, `.claude/rules/ir.md`) (2026-03-09)
- [x] Verify `oric/Cargo.toml` has `ori_registry` dependency (2026-03-09: confirmed present)
- [x] Update `docs/compiler/design/05-type-system/type-registry.md` to replace `TYPECK_BUILTIN_METHODS` references with `ori_registry::BUILTIN_TYPES` (2026-03-09)
- [x] Update `docs/compiler/design/05-type-system/index.md` to replace `TYPECK_BUILTIN_METHODS` reference (2026-03-09)
- [x] Update `docs/compiler/design/10-llvm-backend/builtins-codegen.md` to replace `TYPECK_BUILTIN_METHODS` references (2026-03-09)
- [x] Update `docs/compiler/design/08-evaluator/index.md` to replace `EVAL_BUILTIN_METHODS` and `TYPECK_BUILTIN_METHODS` references (2026-03-09)
- [x] Update `docs/compiler/design/appendices/E-coding-guidelines.md` to replace `TYPECK_BUILTIN_METHODS` example (2026-03-09)
- [x] Update `MEMORY.md` to remove/replace stale `TYPECK_BUILTIN_METHODS` references (lines 34 and 99) (2026-03-09: updated to reference `ori_registry`)
- [x] `cargo t -p oric` passes after all changes (2026-03-09: all tests pass, 0 failures)

---

## 13.9 Validation & Regression

This subsection is the gate check. Every item must pass before Section 13 is marked complete.

### Build Verification

- [x] `cargo c -p ori_ir` — ori_ir compiles clean with `builtin_methods` removed (2026-03-09)
- [x] `cargo c -p ori_registry` — registry compiles (sanity check) (2026-03-09)
- [x] `cargo c -p oric` — oric compiles with updated consistency.rs (2026-03-09)
- [x] `cargo c --workspace` — full workspace compiles (2026-03-09)
- [x] `cargo b` — LLVM build compiles (2026-03-09)
- [x] `cargo doc -p ori_ir --no-deps` — documentation builds clean (2026-03-09)

### Test Verification

- [x] `cargo t -p ori_ir` — remaining ori_ir tests pass (arena, ast, derives, format_spec, builtin_type, etc.) (2026-03-09)
- [x] `cargo t -p ori_registry` — registry purity and method tests pass (2026-03-09)
- [x] `cargo t -p oric` — consistency tests pass with registry data source (2026-03-09: all 9 pass)
- [x] `./test-all.sh` — full test suite passes (2026-03-09: 12,471 passed, 0 failed)
- [x] `./llvm-test.sh` — LLVM tests pass (2026-03-09: included in test-all.sh run)

### Grep Verification

Verify zero remaining references to the deleted module:

- [x] `grep -r 'builtin_methods' compiler/ori_ir/` — returns zero hits (module deleted) (2026-03-09)
- [x] `grep -r 'ori_ir::builtin_methods' compiler/` — returns zero hits (all consumers migrated) (2026-03-09)
- [x] `grep -r 'ori_ir::builtin_methods' plans/` — only historical references in completed plan sections (acceptable) (2026-03-09: 6 files, all in type_strategy_registry plan itself)
- [x] `grep -r 'BUILTIN_METHODS' compiler/oric/src/eval/tests/` — returns zero hits (2026-03-09)
- [x] `grep -r 'BUILTIN_METHODS' compiler/ori_registry/` — returns zero hits (2026-03-09)
- [x] `grep -r 'builtin_methods' compiler/ori_registry/` — returns zero hits (2026-03-09)
- [x] `grep -r 'builtin_methods' .claude/rules/` — returns zero hits (ir.md updated) (2026-03-09)
- [x] `grep -r 'TYPECK_METHODS_NOT_IN_IR' compiler/` — returns zero hits (2026-03-09)
- [x] `grep -r 'COLLECTION_TYPES' compiler/oric/src/eval/tests/methods/consistency.rs` — returns zero hits (2026-03-09)
- [x] `grep -r 'ir_method_set' compiler/` — returns zero hits (2026-03-09)
- [x] `grep -rn 'single source of truth.*builtin' compiler/ori_ir/` — returns zero hits (2026-03-09)
- [x] `grep -r 'TYPECK_BUILTIN_METHODS' docs/compiler/` — returns zero hits (design docs updated) (2026-03-09)
- [x] `grep -r 'EVAL_BUILTIN_METHODS' docs/compiler/` — returns zero hits (design docs updated) (2026-03-09)
- [x] `grep -r 'builtin_methods' docs/compiler/` — returns zero hits (design docs updated) (2026-03-09)
- [x] `grep -r 'TYPECK_BUILTIN_METHODS' ~/.claude/projects/*/memory/MEMORY.md` — returns zero hits (MEMORY.md updated) (2026-03-09)

**Note:** Two historical comments in consuming crates reference eliminated constants but are NOT action items for Section 13:
- `ori_types/src/infer/expr/tests.rs:2726` — "Supersedes the old `TYPECK_BUILTIN_METHODS` constant" (accurate history from Section 09)
- `ori_eval/src/methods/helpers/mod.rs:7` — "EVAL_BUILTIN_METHODS array removed" (accurate history from Section 10)
These are cleanup candidates for Section 14's grep verification but are not stale in the misleading sense — they document what was removed, not what exists.

### Structural Verification

- [x] `ori_ir` does NOT depend on `ori_registry` (they are peers at Layer 0) (2026-03-09: verified via cargo tree)
- [x] `ori_registry` does NOT depend on `ori_ir` (zero dependencies) (2026-03-09: verified via cargo tree)
- [x] `cargo tree -p ori_ir` — does not show `ori_registry` (2026-03-09)
- [x] `cargo tree -p ori_registry` — shows zero dependencies (2026-03-09: only dev-dependency pretty_assertions)

### Code Quality Verification

- [x] `./clippy-all.sh` — zero warnings (2026-03-09)
- [x] `./fmt-all.sh` — all files formatted (2026-03-09)
- [x] No dead code warnings from removed imports (2026-03-09: clean compile, zero warnings)
- [x] No `#[allow(unused_imports)]` escape hatches added (2026-03-09: verified)
- [x] Verify `legacy_type_name()` still has live consumers after deletion (2026-03-09: `ori_llvm/.../builtins/tests.rs` uses it — note: `consistency.rs` `registry_method_pairs()` was also removed since its only caller was the deleted test)
- [x] Verify `builtin_type/mod.rs` module doc updated (no longer claims "across compiler backends") (2026-03-09)

---

## Implementation Order

Within this section, the subsections must be executed in this order:

```
13.1 Field mapping analysis (document only — no code changes)
13.2 ReturnSpec gap analysis (document only — no code changes)
13.3 ParamSpec gap analysis (document only — no code changes)
  │
  ├── Can be done in parallel ───┐
  │                              │
13.5 BuiltinType decision (document decision, no code changes)
13.6 DerivedTrait decision (document decision, no code changes)
13.7 Format spec decision (document decision, no code changes)
  │                              │
  └──────────────────────────────┘
  │
13.8 Update all ori_ir consumers (CODE CHANGE — update consistency.rs, stale comments/docs, design docs, MEMORY.md)
  │
13.4 Delete ori_ir::builtin_methods (CODE CHANGE — delete module + update lib.rs)
  │
13.9 Validation & regression (VERIFICATION — all builds, tests, grep checks)
```

**Critical ordering:** 13.8 BEFORE 13.4. Update consumers first, then delete. If you delete first, the consumers break and you cannot run incremental tests to verify each consumer migration is correct.

**Stale comment cleanup** (Consumer 4 in 13.8) can happen either before or after 13.4 since those files don't import from `builtin_methods`. However, doing it as part of 13.8 keeps the deletion commit clean — all references are gone before the module itself is removed.

> **Risk note:** The 13.9 grep verification step (`grep -r 'builtin_methods'`) will catch any references missed by 13.8. Run it immediately after 13.4 (before moving to 13.9's full test suite) to catch issues early. The most likely missed reference is a stale comment in a file not listed in Consumer 4 — always re-grep after deletion.

---

## Exit Criteria

All of the following must be true before this section is marked complete:

1. **Module deleted:** `compiler/ori_ir/src/builtin_methods/` directory does not exist
2. **lib.rs updated:** `pub mod builtin_methods;` removed from `compiler/ori_ir/src/lib.rs`
3. **Zero references:** `grep -r 'ori_ir::builtin_methods' compiler/` returns zero hits
4. **ori_ir compiles:** `cargo c -p ori_ir` with zero warnings
5. **ori_ir tests pass:** `cargo t -p ori_ir` — all remaining tests green
6. **Consistency tests cleaned:** `consistency.rs` no longer imports `ori_ir::builtin_methods::BUILTIN_METHODS`; `ir_method_set()` function deleted
7. **Allowlists eliminated:** `COLLECTION_TYPES` and `TYPECK_METHODS_NOT_IN_IR` are deleted (the only two IR-related allowlists remaining in consistency.rs)
8. **BuiltinType preserved:** `ori_ir::BuiltinType` compiles and its tests pass
9. **DerivedTrait preserved:** `ori_ir::derives` module compiles and its tests pass
10. **Format spec preserved:** `ori_ir::format_spec` module compiles and its tests pass
11. **Full suite passes:** `./test-all.sh` green
12. **LLVM suite passes:** `./llvm-test.sh` green
13. **No dependency cycles:** `ori_ir` does not depend on `ori_registry`; `ori_registry` does not depend on `ori_ir`
14. **Net deletion:** Zero lines added to `ori_ir`; `builtin_methods/` (~945 lines) fully removed
15. **Stale comments cleaned:** `grep -r 'TYPECK_METHODS_NOT_IN_IR' compiler/` and `grep -r 'ir_method_set' compiler/` both return zero hits
16. **Documentation updated:** `.claude/rules/ir.md` no longer references `builtin_methods/` and includes `builtin_type/` and `format_spec.rs` in Key Files; `legacy_type_name()` doc no longer references `BUILTIN_METHODS`; `builtin_type/mod.rs` module doc reflects ori_ir-internal scope
17. **Docs build clean:** `cargo doc -p ori_ir --no-deps` succeeds with no warnings
18. **Design docs updated:** `docs/compiler/design/` files no longer reference `TYPECK_BUILTIN_METHODS`, `EVAL_BUILTIN_METHODS`, or `ori_ir::builtin_methods`
19. **MEMORY.md updated:** stale `TYPECK_BUILTIN_METHODS` references removed or replaced with `ori_registry` equivalents
