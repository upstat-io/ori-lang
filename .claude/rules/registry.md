---
paths:
  - "**registry**"
---

# ori_registry

Pure-data crate. Zero dependencies, zero logic, zero `unsafe`. All data is `const`-constructible and lives in `.rodata`. Single source of truth for builtin type behavior across all compiler phases.

## Key Types

| Type | Purpose |
|------|---------|
| `TypeTag` | `#[repr(u8)]` enum, 23 variants. Identity discriminant for all builtin types. |
| `TypeDef` | Complete behavioral spec: tag, name, memory strategy, type params, methods, operators. |
| `MethodDef` | 10 required fields: name, receiver, params, returns, trait_name, pure, backend_required, kind, dei_only, dei_propagation. |
| `OpDefs` | 20 `OpStrategy` fields (one per operator). `Unsupported` = type error. |
| `MemoryStrategy` | `Copy` / `Arc` / `Structural` — drives ARC classification. |
| `Ownership` | `Borrow` / `Owned` / `Copy` — receiver and parameter passing. |
| `ReturnTag` | Return/param type: `Concrete(TypeTag)`, `SelfType`, projections (`ElementType`, `KeyType`, etc.), wrappers (`Option(TypeTag)`, `ListOf(TypeProjection)`). |

## Query API

```rust
find_type(TypeTag::Str) -> Option<&'static TypeDef>
find_method(TypeTag::Str, "len") -> Option<&'static MethodDef>  // DEI-aware
has_method(TypeTag::Int, "abs") -> bool
methods_for(TypeTag::Int) -> &'static [MethodDef]               // unfiltered
method_names_for(TypeTag::Iterator) -> impl Iterator<Item = &str> // DEI-filtered
borrowing_method_names() -> &'static [&'static str]             // sorted, deduped
```

DEI aliasing: `DoubleEndedIterator` resolves to `Iterator` via `TypeTag::base_type()`. Plain `Iterator` queries exclude `dei_only` methods.

## Adding a New Type

1. Add variant to `TypeTag` in `tags/mod.rs`
2. Add to `ALL_TYPE_TAGS` array in same file
3. Update `TypeTag::name()` match arm
4. Update `TypeTag::is_primitive()` / `is_generic()` if applicable
5. Create `defs/type_name.rs` (or `defs/type_name/mod.rs` for large types)
   - Define `static METHODS: &[MethodDef]` (sorted alphabetically by name)
   - Define `pub static TYPE_NAME: TypeDef` with all 6 fields
6. Add `mod type_name;` and `pub use self::type_name::TYPE_NAME;` in `defs/mod.rs`
7. Add `&TYPE_NAME` to `BUILTIN_TYPES` array (in `TypeTag` discriminant order)
8. Update `type_tag_all_contains_every_variant` expected count
9. Add `_enforce_exhaustiveness` match arm in 4 consuming crates:
   - `ori_types/src/infer/expr/methods/mod.rs`
   - `ori_eval/src/methods/mod.rs`
   - `ori_arc/src/borrow/mod.rs`
   - `ori_llvm/src/codegen/arc_emitter/builtins/mod.rs`
10. Implement handlers in each consuming crate

## Adding a New Method

Add a `MethodDef` to the type's `METHODS` slice. **Must be sorted alphabetically** — `methods_sorted_by_name` test enforces this.

Use convenience constructors:
- `MethodDef::primitive(name, params, returns, trait_name, receiver)` — pure, backend_required, instance, no DEI
- `MethodDef::compound(name, params, returns, trait_name, receiver, backend_required)` — configurable backend_required
- `MethodDef::associated(name, params, returns)` — factory function, backend_required: false
- `MethodDef::associated_backend(name, params, returns)` — factory function, backend_required: true

Common param patterns: `&[]` (no params), `&ONE_SELF_COPY`, `&ONE_SELF_BORROW`, `&ONE_SELF_OWNED`, `&TWO_SELF_COPY`. Custom params via `&[ParamDef { name, ty, ownership }]`.

Copy types: receiver must be `Ownership::Borrow` (enforced by `all_receivers_documented` test).

## Sync Points

When you add/change a method with `backend_required: true`:
1. **ori_types** — type checker method resolution (`infer/expr/methods/`)
2. **ori_eval** — evaluator method dispatch (`methods/`)
3. **ori_llvm** — LLVM codegen (`codegen/arc_emitter/builtins/`)
4. **ori_arc** — borrow inference (if ownership semantics change)

Enforcement tests in each consuming crate verify coverage against the registry. A new `backend_required: true` method causes test failures until all backends implement it.

## Tests to Run

```bash
# Registry integrity (sorted, no duplicates, purity, operator consistency)
cargo test -p ori_registry

# Cross-phase enforcement tests (in oric)
cargo test -p oric -- consistency

# LLVM builtin coverage + sync tests
cargo test -p ori_llvm -- builtins

# ARC borrow set sync tests
cargo test -p ori_arc -- builtins

# Full suite (always after registry changes)
./test-all.sh
```

## Invariants

- **Zero dependencies** — `Cargo.toml` has empty `[dependencies]` (enforced by `purity_cargo_toml_has_no_dependencies`)
- **No unsafe** — scanned and enforced by `purity_no_unsafe_code`
- **No heap types** — no `String`, `Vec`, `Box`, `Arc`, `HashMap` in source (enforced by `purity_no_heap_allocation_types`)
- **No `&mut` in public API** — enforced by `purity_no_mutable_api`
- **Methods sorted alphabetically** per TypeDef (enforced by `methods_sorted_by_name`)
- **Every TypeTag has a TypeDef** except `Unit`, `Never`, `Function`, `DoubleEndedIterator` (enforced by `all_type_tags_present`)
