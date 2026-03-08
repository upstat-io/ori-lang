---
plan: "type_strategy_registry"
section: "13"
title: "Migrate ori_ir & Legacy Consolidation"
status: not-started
reviewed: false
goal: "Replace ori_ir::builtin_methods with ori_registry, consolidate all method metadata into the registry, deprecate superseded types, and update all consumers — without breaking any tests"
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
    status: not-started
  - id: "13.2"
    title: "ReturnSpec Expressiveness Gap Analysis"
    status: not-started
  - id: "13.3"
    title: "ParamSpec Expressiveness Gap Analysis"
    status: not-started
  - id: "13.4"
    title: "Delete ori_ir::builtin_methods Module"
    status: not-started
  - id: "13.5"
    title: "BuiltinType Deprecation Path"
    status: not-started
  - id: "13.6"
    title: "DerivedTrait Alignment"
    status: not-started
  - id: "13.7"
    title: "Format Spec Decision"
    status: not-started
  - id: "13.8"
    title: "Update All ori_ir Consumers"
    status: not-started
  - id: "13.9"
    title: "Validation & Regression"
    status: not-started
---

# Section 13: Migrate ori_ir & Legacy Consolidation

**Context:** Sections 09-12 wire the four consuming crates (`ori_types`, `ori_eval`, `ori_arc`, `ori_llvm`) to read from `ori_registry` instead of their own hard-coded registries. This section handles the remaining piece: `ori_ir` itself has a builtin method registry (`BUILTIN_METHODS`, 162 entries across 866 lines) that predates `ori_registry`. With all consumers migrated, the `ori_ir` registry is dead weight — a second source of truth that serves no purpose and will inevitably drift.

**Design rationale:** Option B (clean break) over Option A (re-export shim). The `ori_ir` `MethodDef`, `ParamSpec`, and `ReturnSpec` types are structurally different from `ori_registry`'s types (see 13.1-13.3). A compatibility shim would paper over these differences without eliminating them. A clean break is more work upfront but eliminates the drift risk permanently and removes ~400 lines of code.

**Ordering constraint:** This section MUST execute after Sections 09-12 complete. The consumers must already be reading from `ori_registry` before we delete what they previously read from. Attempting this section before wiring is complete will break every consumer simultaneously.

---

## 13.1 Field Mapping: ori_ir::MethodDef to ori_registry::MethodDef

The two `MethodDef` types serve the same purpose but have different structures. This subsection documents the exact mapping and identifies where they diverge.

### Field-by-Field Comparison

| ori_ir::MethodDef Field | Type | ori_registry::MethodDef Field | Type | Mapping |
|---|---|---|---|---|
| `receiver` | `BuiltinType` | `receiver` | `Ownership` | Two different concepts with the same field name: ori_ir's `receiver: BuiltinType` identifies WHICH type (now implicit via `TypeDef.tag`); the registry's `receiver: Ownership` describes HOW the receiver is passed (`Borrow`, `Owned`, or `Copy`). |
| `name` | `&'static str` | `name` | `&'static str` | **Identical.** |
| `params` | `&'static [ParamSpec]` | `params` | `&'static [ParamDef]` | See 13.3 for expressiveness gap. `ParamSpec::SelfType` maps to `ParamDef { ty: ReturnTag::SelfType, ... }`. |
| `returns` | `ReturnSpec` | `returns` | `ReturnTag` | See 13.2 for expressiveness gap. `ReturnSpec::SelfType` maps to `ReturnTag::SelfType`. `ReturnSpec::Type(BuiltinType::Int)` maps to `ReturnTag::Concrete(TypeTag::Int)`. |
| `trait_name` | `Option<&'static str>` | `trait_name` | `Option<&'static str>` | **Identical.** Preserved in registry for phases that need trait association (LLVM trait dispatch path). |
| `receiver_borrows` | `bool` | `receiver` | `Ownership` | `true` maps to `Ownership::Borrow`, `false` maps to `Ownership::Owned`. Copy types (int, float, bool, byte, char, Duration, Size, Ordering) use `Ownership::Copy`. Renamed to `receiver` (the ownership of the receiver). |

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

`receiver_borrows: bool` is replaced by `receiver: Ownership` (an enum: `Borrow`, `Owned`, `Copy`). This is strictly more expressive — `Copy` captures the semantic difference between "borrowed because it's a reference type" and "trivially copied because it's a value type." For the current 162 entries in `BUILTIN_METHODS`, every single one has `receiver_borrows: true`. During migration, the registry enriches this: methods on Copy types (int, float, bool, byte, char, Duration, Size, Ordering) use `Ownership::Copy`; methods on Arc types (str, list, map, etc.) that borrow use `Ownership::Borrow`; consuming methods (e.g., `option.unwrap()`, `iterator.collect()`) use `Ownership::Owned`.

**3. `trait_name` is preserved.**

Decision: `trait_name: Option<&'static str>` stays on `ori_registry::MethodDef`. The LLVM codegen trait dispatch path needs to distinguish `compare` (trait method on `Comparable`) from `abs` (direct method). Removing `trait_name` would force the LLVM backend to maintain its own mapping, recreating the drift problem we are eliminating.

### Migration Impact

No code changes in `ori_ir` itself for this mapping — `ori_ir::MethodDef` is being deleted, not modified. The mapping is used by consumers (13.8) when updating their imports.

### Checklist

- [ ] Verify all 162 `BUILTIN_METHODS` entries map cleanly to `ori_registry` types (run cross-reference script)
- [ ] Confirm every `receiver_borrows: true` maps to `Ownership::Borrow`
- [ ] Confirm every `receiver_borrows: false` maps to `Ownership::Owned` (currently zero such entries)
- [ ] Confirm `trait_name` values match between ori_ir and ori_registry for all entries

---

## 13.2 ReturnSpec Expressiveness Gap Analysis

`ori_ir::ReturnSpec` has 7 variants. `ori_registry` uses `ReturnTag` for return types (see Section 01). For primitive/compound types, `ReturnTag::Concrete(TypeTag)` wraps concrete types; for generic types, richer variants like `ReturnTag::ElementType`, `ReturnTag::OptionOf(TypeProjection)` etc. are used. This subsection analyzes whether the mapping from `ReturnSpec` is straightforward.

### Variant-by-Variant Analysis

| ori_ir::ReturnSpec | Current Usage | ori_registry Equivalent | Sufficient? |
|---|---|---|---|
| `SelfType` | `clone`, `abs`, `add`, `sub`, etc. — returns same type as receiver | `ReturnTag::SelfType` | Yes. `ReturnTag::SelfType` exists for exactly this purpose. |
| `Type(BuiltinType::Int)` | `str.len`, `Duration.nanoseconds`, `hash`, etc. — returns specific type | `ReturnTag::Concrete(TypeTag::Int)` (etc.) | Yes. Wrapped via `From<TypeTag>`. |
| `Type(BuiltinType::Ordering)` | `compare` — returns Ordering | `ReturnTag::Concrete(TypeTag::Ordering)` | Yes. Wrapped via `From<TypeTag>`. |
| `Void` | Not used in current `BUILTIN_METHODS` (zero entries) | `ReturnTag::Unit` | Yes. Never needed for the 162 entries being migrated. |
| `ElementType` | Not used in current `BUILTIN_METHODS` (zero entries) | N/A | Not needed. Used only for collection methods (List, Iterator, etc.) which are handled by Section 06/07. |
| `OptionElement` | Not used in current `BUILTIN_METHODS` (zero entries) | N/A | Not needed. Same — collection methods only. |
| `ListElement` | Not used in current `BUILTIN_METHODS` (zero entries) | N/A | Not needed. Same. |
| `InnerType` | Not used in current `BUILTIN_METHODS` (zero entries) | N/A | Not needed. Option/Result methods only. |

### Conclusion

**`ReturnTag::Concrete(TypeTag)` is fully sufficient for all 162 entries in BUILTIN_METHODS.** Every `ReturnSpec::Type(X)` maps to `ReturnTag::Concrete(TypeTag::X)` and every `ReturnSpec::SelfType` maps to `ReturnTag::SelfType`. The complex return spec variants (`ElementType`, `OptionElement`, `ListElement`, `InnerType`) are never used in the current `ori_ir` registry. They exist because `ori_ir` anticipated needing them for collection types, but collection types were never added to `BUILTIN_METHODS`. In `ori_registry`, collection types are handled by Sections 06-07, where `ReturnTag` variants (e.g., `ReturnTag::ElementType`, `ReturnTag::Fresh`) capture the structural return type templates, and the type checker's existing inference logic handles closure-dependent return types.

### Decision: ReturnTag for all return types

- **Registry `MethodDef.returns`**: Always `ReturnTag` (Section 01 frozen schema). For primitive/compound types, this is `ReturnTag::Concrete(TypeTag::Int)` etc. For generic types, richer variants like `ReturnTag::ElementType`, `ReturnTag::OptionOf(TypeProjection::Element)`, `ReturnTag::Fresh` are used (see Section 01).
- **No need for ReturnSpec in ori_registry.** The legacy `ori_ir::ReturnSpec` type is retired.

### Checklist

- [ ] Verify zero uses of `ElementType`, `OptionElement`, `ListElement`, `InnerType` in `BUILTIN_METHODS`
- [ ] Verify all `ReturnSpec::SelfType` usages map to `ReturnTag::SelfType`
- [ ] Verify all `ReturnSpec::Type(X)` usages map to `ReturnTag::Concrete(TypeTag::X)`

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

**ParamDef with ReturnTag is fully sufficient for all 162 entries in BUILTIN_METHODS.** Only three ParamSpec variants are actually used: `SelfType` (most common — trait methods and arithmetic), `Int` (Duration/Size multiplication/division), and `Str` (string comparison/contains methods). All three map directly to `ParamDef` with the corresponding `ReturnTag::SelfType` or `ReturnTag::Concrete(TypeTag)`.

The complex variants (`Any`, `Closure`, `Bool`) are unused in `BUILTIN_METHODS`. In `ori_registry`, collection and iterator methods that take closures use `ParamDef` with `ReturnTag::Fresh` for parameter declaration; the actual closure type inference stays in the type checker.

### Decision: ParamDef with ReturnTag + Ownership

No compatibility shim needed. The mapping is:
- `ParamSpec::SelfType` --> `ParamDef { name: "other", ty: ReturnTag::SelfType, ownership: Ownership::Borrow }`
- `ParamSpec::Int` --> `ParamDef { name: "n", ty: ReturnTag::Concrete(TypeTag::Int), ownership: Ownership::Copy }`
- `ParamSpec::Str` --> `ParamDef { name: "s", ty: ReturnTag::Concrete(TypeTag::Str), ownership: Ownership::Borrow }`

### Checklist

- [ ] Verify zero uses of `ParamSpec::Bool`, `ParamSpec::Any`, `ParamSpec::Closure` in `BUILTIN_METHODS`
- [ ] Verify all `ParamSpec::SelfType` map to `ParamDef { ty: ReturnTag::SelfType, ... }`
- [ ] Verify all `ParamSpec::Int` map to `ParamDef { ty: ReturnTag::Concrete(TypeTag::Int), ... }`
- [ ] Verify all `ParamSpec::Str` map to `ParamDef { ty: ReturnTag::Concrete(TypeTag::Str), ... }`

---

## 13.4 Delete ori_ir::builtin_methods Module

This is the core deletion. The module spans 866 lines across two files and defines types, constants, and query functions that are fully superseded by `ori_registry`.

### What Gets Deleted

| File | Lines | Contents |
|---|---|---|
| `compiler/ori_ir/src/builtin_methods/mod.rs` | 866 | `ParamSpec` enum, `ReturnSpec` enum, `MethodDef` struct + impl, `BUILTIN_METHODS` static (162 entries), 6 query functions |
| `compiler/ori_ir/src/builtin_methods/tests.rs` | 85 | 5 unit tests for the query functions |

**Total deleted: ~950 lines.**

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
| `borrowing_method_names() -> impl Iterator<Item = &str>` | `ori_registry` query: filter all methods where `receiver == Ownership::Borrow` | More precise — type-qualified, not just name-based |
| `method_borrows_receiver(BuiltinType, &str) -> Option<bool>` | `ori_registry::find_method(tag, name).map(|m| m.receiver == Ownership::Borrow)` | Inlined at call sites |
| `methods_for(BuiltinType) -> impl Iterator<Item = &MethodDef>` | `ori_registry::find_type(tag).map(|t| t.methods.iter())` | Navigates through TypeDef |
| `has_method(BuiltinType, &str) -> bool` | `ori_registry::find_method(tag, name).is_some()` | Trivial wrapper, inline at call sites |
| `method_names_for(BuiltinType) -> impl Iterator<Item = &str>` | `ori_registry::find_type(tag).map(|t| t.methods.iter().map(|m| m.name))` | Navigates through TypeDef |

### Static Retired

| Static | Replacement | Notes |
|---|---|---|
| `BUILTIN_METHODS: &[MethodDef]` (162 entries) | `ori_registry::BUILTIN_TYPES: &[&TypeDef]` (with nested methods) | Structural change: flat list --> nested by type |

### Deletion Steps (ordered)

1. **Verify no remaining consumers** (prerequisite from 13.8) — `grep -r 'ori_ir::builtin_methods' compiler/` returns zero hits outside of `ori_ir` itself
2. **Delete `compiler/ori_ir/src/builtin_methods/tests.rs`** — the entire file
3. **Delete `compiler/ori_ir/src/builtin_methods/mod.rs`** — the entire file
4. **Remove directory** `compiler/ori_ir/src/builtin_methods/` (if now empty)
5. **Update `compiler/ori_ir/src/lib.rs`** — remove `pub mod builtin_methods;` from line 40
6. **Verify** `cargo c -p ori_ir` compiles clean

### BEFORE / AFTER for lib.rs

**BEFORE** (`compiler/ori_ir/src/lib.rs`, lines 37-41):
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

- [ ] `grep -r 'ori_ir::builtin_methods' compiler/` returns only ori_ir-internal hits (tests.rs, mod.rs)
- [ ] `grep -r 'builtin_methods::' compiler/` returns zero hits outside ori_ir
- [ ] Delete `compiler/ori_ir/src/builtin_methods/tests.rs`
- [ ] Delete `compiler/ori_ir/src/builtin_methods/mod.rs`
- [ ] Remove `compiler/ori_ir/src/builtin_methods/` directory
- [ ] Remove `pub mod builtin_methods;` from `compiler/ori_ir/src/lib.rs`
- [ ] `cargo c -p ori_ir` compiles clean with zero warnings
- [ ] `cargo t -p ori_ir` passes (remaining tests unaffected)

---

## 13.5 BuiltinType Deprecation Path

`ori_ir::BuiltinType` is a separate enum from `ori_registry::TypeTag`, but they cover overlapping ground. This subsection decides what happens to `BuiltinType`.

### Current State

`ori_ir::BuiltinType` (18 variants): `Int`, `Float`, `Bool`, `Str`, `Char`, `Byte`, `Unit`, `Never`, `Duration`, `Size`, `Ordering`, `List`, `Map`, `Option`, `Result`, `Range`, `Set`, `Channel`.

`ori_registry::TypeTag` (designed in Section 01): covers the same types, possibly with different variant naming.

### Key Difference: BuiltinType Has TypeId Conversions

`BuiltinType` provides `from_type_id(TypeId) -> Option<Self>` and `type_id(self) -> Option<TypeId>`. These bridge the IR representation (`TypeId`, a `u32` index) to a high-level type identity. `TypeTag` does not have this — it is a pure-data tag with no dependency on `TypeId` (which lives in `ori_ir`).

### Decision: Keep BuiltinType in ori_ir, Add From<BuiltinType> for TypeTag

**BuiltinType stays in `ori_ir`.** Rationale:

1. **BuiltinType depends on TypeId** (`from_type_id`, `type_id`). TypeId is an `ori_ir` type. Moving BuiltinType to `ori_registry` would force `ori_registry` to depend on `ori_ir`, violating the purity contract.

2. **BuiltinType is used widely outside the method registry.** It appears in pattern matching, code generation, type checking — contexts where `TypeTag` could substitute but where the migration would be large and tangential to this plan's goal.

3. **The two enums serve different layers.** `BuiltinType` is an IR-level type identity (coupled to `TypeId`). `TypeTag` is a registry-level type tag (pure data, no coupling). They are complementary, not redundant.

### Bridge Implementation

Add `From<BuiltinType> for TypeTag` in `ori_registry` (or as a standalone function in consuming crates, since `ori_registry` cannot depend on `ori_ir`). Since `ori_registry` has zero dependencies, this conversion must be implemented in a consuming crate or in `ori_ir` itself:

```rust
// In ori_ir (since it can depend on ori_registry):
// NO — ori_ir does NOT depend on ori_registry. They are peers at Layer 0.

// In each consuming crate that needs the bridge:
fn builtin_type_to_tag(bt: BuiltinType) -> TypeTag {
    match bt {
        BuiltinType::Int => TypeTag::Int,
        BuiltinType::Float => TypeTag::Float,
        BuiltinType::Bool => TypeTag::Bool,
        BuiltinType::Str => TypeTag::Str,
        BuiltinType::Char => TypeTag::Char,
        BuiltinType::Byte => TypeTag::Byte,
        BuiltinType::Unit => TypeTag::Unit,
        BuiltinType::Never => TypeTag::Never,
        BuiltinType::Duration => TypeTag::Duration,
        BuiltinType::Size => TypeTag::Size,
        BuiltinType::Ordering => TypeTag::Ordering,
        BuiltinType::List => TypeTag::List,
        BuiltinType::Map => TypeTag::Map,
        BuiltinType::Option => TypeTag::Option,
        BuiltinType::Result => TypeTag::Result,
        BuiltinType::Range => TypeTag::Range,
        BuiltinType::Set => TypeTag::Set,
        BuiltinType::Channel => TypeTag::Channel,
    }
}
```

### Long-Term Deprecation (Out of Scope for This Section)

Over time, consumers should migrate from `BuiltinType` to `TypeTag` where possible. This is a separate effort — this section only ensures the bridge exists and the deletion of `builtin_methods` does not break anything.

### Checklist

- [ ] Verify `BuiltinType` is NOT used in `builtin_methods/mod.rs` query function signatures (it is — as receiver type — but those functions are being deleted)
- [ ] Verify `BuiltinType` has uses OUTSIDE `builtin_methods/` (it does — `builtin_type/mod.rs`, various consumers)
- [ ] Confirm decision: BuiltinType stays in ori_ir
- [ ] Document bridge function pattern for consuming crates that need both types
- [ ] Verify `cargo c -p ori_ir` passes after `builtin_methods` deletion (BuiltinType still compiles because it does not depend on the deleted module)

---

## 13.6 DerivedTrait Alignment

`DerivedTrait` lives in `ori_ir::derives` and has a 4-way sync contract with `ori_types`, `ori_eval`, `ori_llvm`, and `library/std`. This subsection decides how `DerivedTrait` relates to `ori_registry`.

### Current State

`DerivedTrait` (7 variants): `Eq`, `Clone`, `Hashable`, `Printable`, `Debug`, `Default`, `Comparable`.

Defined via `define_derived_traits!` macro in `compiler/ori_ir/src/derives/mod.rs`. The macro generates `from_name()`, `method_name()`, `trait_name()`, `shape()`, `requires_supertrait()`, `supports_sum_types()`, and `ALL`/`COUNT` constants.

Each derived trait produces a method (e.g., `Eq` produces `eq`, `Clone` produces `clone`). These methods appear in the `ori_registry` type definitions as regular `MethodDef` entries with `trait_name` set (e.g., `MethodDef { name: "clone", trait_name: Some("Clone"), ... }`).

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

- [ ] Verify `DerivedTrait` has zero references to `builtin_methods` module (it does not reference it)
- [ ] Verify `derives/mod.rs` compiles independently of `builtin_methods/mod.rs`
- [ ] Confirm decision: DerivedTrait stays in ori_ir, unchanged
- [ ] Verify `trait_name` values on `ori_registry::MethodDef` entries match `DerivedTrait::trait_name()` for corresponding traits (e.g., `"Comparable"` matches `DerivedTrait::Comparable.trait_name()`)

---

## 13.7 Format Spec Decision

`FormatType`, `Align`, `Sign`, `ParsedFormatSpec`, and `parse_format_spec()` live in `ori_ir::format_spec`. They have a 4-way sync contract with `ori_types`, `ori_eval`, and `ori_rt`. This subsection decides what happens to them.

### Current State

- **`ori_ir::format_spec`** (300 lines): Defines the enums (`FormatType`, `Align`, `Sign`), the `ParsedFormatSpec` struct, and the `parse_format_spec()` parser function.
- **Consumers**: `ori_types` (validates format type vs expression type), `ori_eval` (applies formatting at runtime), `ori_rt` (runtime format calls, guarded by its own variant count test).
- **Consistency tests**: `compiler/oric/src/eval/tests/methods/consistency.rs` has 6 tests verifying variant sync between `ori_ir::format_spec` and `ori_types`/`ori_eval` registrations (lines 776-933).

### Decision: Format Spec Stays in ori_ir

**Format spec types do NOT move to ori_registry.** Rationale:

1. **`parse_format_spec()` is a parser function with logic.** It contains 120+ lines of parsing logic (alignment, sign, width, precision, type character). This violates the `ori_registry` purity contract.

2. **`FormatSpecError` contains `String`.** The error type uses heap allocation (`TrailingCharacters(String)`, `InvalidWidth(String)`). This violates the registry's no-heap-allocation constraint.

3. **Format specs are about syntax, not type behavior.** They describe how to format a value in a template string (`{x:>10.2f}`), not what methods exist on a type. The registry is about type behavioral contracts.

4. **The existing sync mechanism works.** The consistency tests in `consistency.rs` verify variant alignment between `ori_ir`, `ori_types`, `ori_eval`, and `ori_rt`. This is the same pattern the registry eliminates for method metadata — but format specs have only 3 small enums (8 + 3 + 3 = 14 variants total), and the sync tests are cheap and reliable.

### Future Consideration

If `ori_registry` ever gains a `FormattingDefs` struct on `TypeDef` (declaring which format types are valid for which type — e.g., `int` supports `Binary`/`Octal`/`Hex` but `str` does not), the enum definitions could move to `ori_registry` and the parser could stay in `ori_ir`. But this is speculative and not part of this plan.

### Impact on Consistency Tests

The 6 format spec consistency tests in `consistency.rs` (lines 776-933) use `ori_ir::format_spec::FormatType`, `ori_ir::format_spec::Align`, and `ori_ir::format_spec::Sign`. These imports are UNAFFECTED by the deletion of `builtin_methods`. The format spec module is a separate module in `ori_ir` (`format_spec.rs`), not part of `builtin_methods/`.

### Checklist

- [ ] Verify `format_spec.rs` has zero dependencies on `builtin_methods/` (confirmed: no import)
- [ ] Verify format spec consistency tests compile after `builtin_methods` deletion
- [ ] Confirm decision: format spec stays in ori_ir, unchanged
- [ ] Document this decision so future contributors do not attempt to move format specs to the registry

---

## 13.8 Update All ori_ir Consumers

With `builtin_methods` deleted, all code that previously imported from `ori_ir::builtin_methods` must be updated. This subsection identifies every consumer and specifies the exact change.

### Consumer Inventory

Grep results for `ori_ir::builtin_methods` in the compiler directory (excluding `ori_ir` itself and plan documents):

| File | Import | Used For |
|---|---|---|
| `compiler/oric/src/eval/tests/methods/consistency.rs:7` | `use ori_ir::builtin_methods::BUILTIN_METHODS;` | Building `ir_method_set()` for cross-phase consistency tests |

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

This function builds a set of `(type_name, method_name)` pairs from the IR registry for use in 3 consistency tests:
- `ir_methods_implemented_in_eval()` — every IR method has an eval handler
- `eval_primitive_methods_in_ir()` — every eval primitive method is in IR
- `typeck_primitive_methods_in_ir()` — every typeck primitive method is in IR

**BEFORE:**
```rust
use ori_ir::builtin_methods::BUILTIN_METHODS;

fn ir_method_set() -> BTreeSet<(&'static str, &'static str)> {
    BUILTIN_METHODS
        .iter()
        .map(|m| (m.receiver.name(), m.name))
        .collect()
}
```

**AFTER:**
```rust
use ori_registry::{BUILTIN_TYPES, TypeDef};

fn registry_method_set() -> BTreeSet<(&'static str, &'static str)> {
    BUILTIN_TYPES
        .iter()
        .flat_map(|type_def: &&TypeDef| {
            type_def.methods.iter().map(move |m| (type_def.name, m.name))
        })
        .collect()
}
```

The function is renamed from `ir_method_set` to `registry_method_set` to reflect the new source. The structural change: instead of iterating a flat list and reading `m.receiver.name()`, we iterate nested `TypeDef.methods` and read `type_def.name` from the parent.

**Allowlist Elimination**

When `ori_registry` becomes the single source of truth, several allowlists in this file become unnecessary:

| Allowlist | Lines | Status After Migration |
|---|---|---|
| `COLLECTION_TYPES` (11 entries) | 13-25 | **Eliminated if** Sections 06-07 add collection types to the registry. If not yet added, the allowlist shrinks to only types not yet in the registry. |
| `IR_METHODS_DISPATCHED_VIA_RESOLVERS` (10 entries) | 33-46 | **Eliminated.** The registry includes ALL methods regardless of dispatch mechanism. The concept of "dispatched via resolvers" is an eval implementation detail, not a registry concern. |
| `EVAL_METHODS_NOT_IN_IR` (19 entries) | 50-80 | **Eliminated.** Registry is the superset — it includes methods from all phases. |
| `TYPECK_METHODS_NOT_IN_IR` (143 entries) | 227-369 | **Eliminated.** Same — registry is the superset. |
| `EVAL_METHODS_NOT_IN_TYPECK` (63 entries) | 161-223 | **Unchanged.** This tracks eval-vs-typeck drift, not IR involvement. Persists until both phases read from the registry (Sections 09-10). |
| `TYPECK_METHODS_NOT_IN_EVAL` (260 entries) | 374-633 | **Unchanged.** Same — tracks typeck-vs-eval drift. |

**Net impact on consistency.rs:** The 3 IR-related tests (`ir_methods_implemented_in_eval`, `eval_primitive_methods_in_ir`, `typeck_primitive_methods_in_ir`) are rewritten to use `registry_method_set()`. The 4 allowlists they reference (`COLLECTION_TYPES`, `IR_METHODS_DISPATCHED_VIA_RESOLVERS`, `EVAL_METHODS_NOT_IN_IR`, `TYPECK_METHODS_NOT_IN_IR`) are either eliminated or reduced. The remaining tests (typeck-vs-eval, iterator, format spec, well-known generics) are untouched.

**Note on Sections 09-10 dependency:** If Sections 09-10 (wire typeck, wire eval) are completed before Section 13, then `EVAL_METHODS_NOT_IN_TYPECK` and `TYPECK_METHODS_NOT_IN_EVAL` may also be eliminated. But Section 13 does not depend on that — it only requires Sections 03-08 (registry populated) and that 09-12 have migrated their own consumers away from `ori_ir::builtin_methods`.

### Consumer 2: ori_arc (Indirect — Already Migrated by Section 11)

`ori_arc` does not import from `ori_ir::builtin_methods` directly. It receives `borrowing_builtins: &FxHashSet<Name>` as a parameter from `ori_llvm`. Section 11 (Wire ARC/Borrow) changes the source of this set from `ori_llvm`'s hard-coded list to `ori_registry` queries. No changes needed in this section.

### Consumer 3: ori_llvm (Indirect — Already Migrated by Section 12)

`ori_llvm` references `TYPECK_BUILTIN_METHODS` (from `ori_types`) in its consistency tests, not `ori_ir::builtin_methods`. Section 12 (Wire LLVM Backend) migrates these to `ori_registry`. No changes needed in this section for the LLVM crate itself.

### oric Cargo.toml Update

After migration, `oric` needs `ori_registry` in its dependencies (already added in Section 02.4). Verify it is present:

```toml
# compiler/oric/Cargo.toml
[dependencies]
ori_registry.workspace = true   # Added in Section 02
```

If not yet present, add it.

### Checklist

- [ ] Update `consistency.rs` line 7: replace `use ori_ir::builtin_methods::BUILTIN_METHODS` with `use ori_registry::{BUILTIN_TYPES, TypeDef}`
- [ ] Rename `ir_method_set()` to `registry_method_set()` throughout `consistency.rs`
- [ ] Rewrite `registry_method_set()` to iterate `BUILTIN_TYPES` with nested `TypeDef.methods`
- [ ] Remove `COLLECTION_TYPES` allowlist (if all collection types in registry) or reduce it
- [ ] Remove `IR_METHODS_DISPATCHED_VIA_RESOLVERS` allowlist
- [ ] Remove `EVAL_METHODS_NOT_IN_IR` allowlist
- [ ] Remove `TYPECK_METHODS_NOT_IN_IR` allowlist
- [ ] Update test function names: `ir_methods_implemented_in_eval` --> `registry_methods_implemented_in_eval` (or similar)
- [ ] Update test assertions and error messages to reference `ori_registry` instead of `ori_ir`
- [ ] Verify `oric/Cargo.toml` has `ori_registry` dependency
- [ ] `cargo t -p oric` passes after all changes

---

## 13.9 Validation & Regression

This subsection is the gate check. Every item must pass before Section 13 is marked complete.

### Build Verification

- [ ] `cargo c -p ori_ir` — ori_ir compiles clean with `builtin_methods` removed
- [ ] `cargo c -p ori_registry` — registry compiles (sanity check)
- [ ] `cargo c -p oric` — oric compiles with updated consistency.rs
- [ ] `cargo c --workspace` — full workspace compiles
- [ ] `cargo b` — LLVM build compiles (if ori_llvm tests reference moved)

### Test Verification

- [ ] `cargo t -p ori_ir` — remaining ori_ir tests pass (arena, ast, derives, format_spec, builtin_type, etc.)
- [ ] `cargo t -p ori_registry` — registry purity and method tests pass
- [ ] `cargo t -p oric` — consistency tests pass with registry data source
- [ ] `./test-all.sh` — full test suite passes
- [ ] `./llvm-test.sh` — LLVM tests pass

### Grep Verification

Verify zero remaining references to the deleted module:

- [ ] `grep -r 'builtin_methods' compiler/ori_ir/` — returns zero hits (module deleted)
- [ ] `grep -r 'ori_ir::builtin_methods' compiler/` — returns zero hits (all consumers migrated)
- [ ] `grep -r 'ori_ir::builtin_methods' plans/` — only historical references in superseded plans (acceptable)
- [ ] `grep -r 'BUILTIN_METHODS' compiler/oric/src/eval/tests/` — returns zero hits (replaced with BUILTIN_TYPES)

### Structural Verification

- [ ] `ori_ir` does NOT depend on `ori_registry` (they are peers at Layer 0)
- [ ] `ori_registry` does NOT depend on `ori_ir` (zero dependencies)
- [ ] `cargo tree -p ori_ir` — does not show `ori_registry`
- [ ] `cargo tree -p ori_registry` — shows zero dependencies

### Code Quality Verification

- [ ] `./clippy-all.sh` — zero warnings related to the migration
- [ ] `./fmt-all.sh` — all files formatted
- [ ] No dead code warnings from removed imports
- [ ] No `#[allow(unused_imports)]` escape hatches added

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
13.8 Update all ori_ir consumers (CODE CHANGE — update consistency.rs)
  │
13.4 Delete ori_ir::builtin_methods (CODE CHANGE — delete module + update lib.rs)
  │
13.9 Validation & regression (VERIFICATION — all builds and tests)
```

**Critical ordering:** 13.8 BEFORE 13.4. Update consumers first, then delete. If you delete first, the consumers break and you cannot run incremental tests to verify each consumer migration is correct.

---

## Exit Criteria

All of the following must be true before this section is marked complete:

1. **Module deleted:** `compiler/ori_ir/src/builtin_methods/` directory does not exist
2. **lib.rs updated:** `pub mod builtin_methods;` removed from `compiler/ori_ir/src/lib.rs`
3. **Zero references:** `grep -r 'ori_ir::builtin_methods' compiler/` returns zero hits
4. **ori_ir compiles:** `cargo c -p ori_ir` with zero warnings
5. **ori_ir tests pass:** `cargo t -p ori_ir` — all remaining tests green
6. **Consistency tests migrated:** `consistency.rs` uses `ori_registry::BUILTIN_TYPES` instead of `ori_ir::builtin_methods::BUILTIN_METHODS`
7. **Allowlists reduced:** `COLLECTION_TYPES`, `IR_METHODS_DISPATCHED_VIA_RESOLVERS`, `EVAL_METHODS_NOT_IN_IR`, and `TYPECK_METHODS_NOT_IN_IR` are either eliminated or reduced to only entries not yet covered by the registry
8. **BuiltinType preserved:** `ori_ir::BuiltinType` compiles and its 4 test functions pass
9. **DerivedTrait preserved:** `ori_ir::derives` module compiles and its tests pass
10. **Format spec preserved:** `ori_ir::format_spec` module compiles and its tests pass
11. **Full suite passes:** `./test-all.sh` green
12. **LLVM suite passes:** `./llvm-test.sh` green (if applicable)
13. **No dependency cycles:** `ori_ir` does not depend on `ori_registry`; `ori_registry` does not depend on `ori_ir`
14. **Net deletion:** Approximately 950 lines deleted from `ori_ir`, zero lines added to `ori_ir`
