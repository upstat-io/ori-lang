---
plan: "type_strategy_registry"
section: "09"
title: "Wire Type Checker (ori_types)"
status: in-progress
depends_on:
  - "03"
  - "04"
  - "05"
  - "06"
  - "07"
  - "08"
subsections:
  - id: "09.1"
    title: "tag_to_type_tag() Bridge (ori_types::Tag -> ori_registry::TypeTag)"
    status: complete
  - id: "09.2"
    title: "return_tag_to_idx() Bridge (ori_registry::ReturnTag -> ori_types::Idx)"
    status: complete
  - id: "09.3"
    title: "Replace resolve_builtin_method() Dispatcher"
    status: complete
  - id: "09.4"
    title: "Replace All resolve_*_method Functions"
    status: complete
  - id: "09.5"
    title: "Replace TYPECK_BUILTIN_METHODS"
    status: complete
  - id: "09.6"
    title: "Associated Function Dispatch"
    status: in-progress
  - id: "09.7"
    title: "DEI_ONLY_METHODS Migration"
    status: complete
  - id: "09.8"
    title: "Well-Known Generic Type Interaction"
    status: complete
  - id: "09.9"
    title: "Validation & Regression Testing"
    status: complete
---

# Section 09: Wire Type Checker (ori_types)

## Overview

This is the most complex wiring section. The type checker (`ori_types`) is the primary consumer of builtin type knowledge in the compiler. It hard-codes method resolution across 20 `resolve_*_method()` functions, maintains a 390-entry `TYPECK_BUILTIN_METHODS` array, and uses a 5-entry `DEI_ONLY_METHODS` constant for DoubleEndedIterator gating.

All of this gets replaced by `ori_registry` lookups. The key challenge is that the type checker doesn't just need method *names* -- it needs to construct `Idx` return types in the pool, which requires bridging between the registry's `TypeTag` enum and the pool's `Idx` handles. Two bridge functions (`tag_to_type_tag` and `return_tag_to_idx`) mediate this translation.

**Net effect:** ~800 lines of hard-coded method resolution deleted, replaced by ~200-250 lines of bridge + lookup + helper code (more than the original ~80 line estimate due to ReturnTag variant coverage and special-case logic preservation).

## Design Decisions

### Cargo.toml dependency: already added

`ori_registry` is already listed as a dependency in `compiler/ori_types/Cargo.toml` (line 14: `ori_registry.workspace = true`). No Cargo.toml change is needed for this section. This was added during the ori_registry crate scaffolding (Section 02).

### `resolve_receiver_and_builtin` stays largely unchanged

The `resolve_receiver_and_builtin` function in `calls/method_call.rs` (line 284) is the outer entry point that:
1. Infers the receiver type, handles errors, instantiates schemes, defers type variables
2. Calls `resolve_builtin_method()` for builtin dispatch
3. Runs `check_infinite_iterator_consumed()` for iterator consumer warnings
4. Runs the DEI-only gating check (09.7)
5. Runs `check_range_float_iteration()` for `Range<float>` rejection

**This function is NOT replaced.** Only its internal calls change:
- Step 2: `resolve_builtin_method()` internals change (09.3), but the call site is unchanged
- Step 4: `DEI_ONLY_METHODS.contains()` becomes `is_dei_only_method()` (09.7)
- Steps 1, 3, 5: Unchanged

The `check_infinite_iterator_consumed()` function (line 422) and its helpers (`find_infinite_source`, `INFINITE_CONSUMING_METHODS`, `TRANSPARENT_ADAPTERS`, `BOUNDING_METHODS`) remain entirely unchanged -- they inspect AST structure, not type metadata.

The `check_range_float_iteration()` function (line 370) also remains unchanged -- it checks `elem == Idx::FLOAT` which is pool-level logic, not registry data.

The `unify_higher_order_constraints()` function (line 165) remains unchanged -- it performs post-return-type unification of closure arguments, which is inference logic that must stay in `ori_types`.

### Bridge functions live in ori_types, not ori_registry

The `tag_to_type_tag()` and `return_tag_to_idx()` functions live in `ori_types` (in a new `infer/expr/registry_bridge.rs`), not in `ori_registry`. The registry is pure data with zero dependencies. The bridge functions need `ori_types::Tag`, `ori_types::Idx`, `ori_types::Pool`, and `ori_registry::TypeTag` -- they inherently belong to the consuming crate.

### Import hygiene: ori_registry in ori_types

The `ori_registry` dependency is already in `compiler/ori_types/Cargo.toml`. Import structure:

- **`registry_bridge.rs`**: `use ori_registry::{TypeTag, ReturnTag, TypeProjection};` -- direct imports of the specific types needed. No glob imports.
- **`methods/mod.rs`**: `use ori_registry;` -- module-level import for `ori_registry::find_method()` call. Alternatively `use ori_registry::find_method;` for the single function used.
- **`calls/method_call.rs`**: `use ori_registry::is_dei_only;` -- single function import for DEI gating.

No re-export of `ori_registry` types from `ori_types`. Consumers that need registry types import from `ori_registry` directly.

### Special logic preserved alongside registry lookups

Some `resolve_*_method` functions contain logic beyond simple return-type mapping:

- **resolve_range_method**: Rejects `iter`/`to_list`/`collect` on `Range<float>`.
- **resolve_iterator_method**: DEI-only gating, DEI propagation for `map`/`filter`, pool construction for `next` (returns `(Option<T>, Iterator<T>)` tuple).
- **resolve_list_method**: Pool construction for `enumerate` (returns `[int, T]`), `zip` (fresh var for other element).
- **resolve_named_type_method**: Newtype `.unwrap()` resolution through TypeRegistry.

These cannot be replaced by a simple `find_method().returns -> return_tag_to_idx()` pipeline. The plan preserves this logic as post-lookup refinements that override or augment the registry's static return type.

### TYPECK_BUILTIN_METHODS eliminated, not replaced

The 390-entry `TYPECK_BUILTIN_METHODS` array currently serves one purpose: cross-crate consistency tests. After migration, the registry IS the source of truth. The consistency tests iterate `BUILTIN_TYPES` directly. There is no need for a separate array.

### Registry methods not in current TYPECK_BUILTIN_METHODS

The registry (Sections 03-07) defines some methods that do NOT currently appear in `TYPECK_BUILTIN_METHODS` or `resolve_*_method()`. These are methods that the registry correctly declares but the type checker has not yet implemented. Migration either:
- (a) Adds these methods to `resolve_computed_return()` so they become resolvable (new capability), OR
- (b) Documents them as registry-only methods that bypass `resolve_builtin_method()` (e.g., associated functions dispatched through a different code path)

**Known gaps (str type):**
- `str.as_bytes()` -> `[byte]` — in registry (`ReturnTag::List(TypeTag::Byte)`), NOT in typeck. Add to `resolve_computed_return`.
- `str.to_bytes()` -> `[byte]` — in registry, NOT in typeck. Add to `resolve_computed_return`.
- `str.add()` -> `str` — trait method for `Add` operator, in registry. Operator dispatch handles this; does NOT need `resolve_builtin_method` entry.
- `str.from_utf8()` -> `Result<str, Error>` — associated function (`MethodKind::Associated`) in registry. Associated function resolution flows through `calls/` module, not `resolve_builtin_method`. Add to `resolve_computed_return` only if associated functions are routed through the same path.
- `str.from_utf8_unchecked()` -> `str` — same as `from_utf8`, associated function.

**Operator trait methods across ALL types** (not just str):
The registry declares operator methods (`add`, `subtract`, `multiply`, `divide`, `negate`, `not`, `bit_and`, `bit_or`, `bit_xor`, `bit_not`, `shift_left`, `shift_right`, `floor_divide`, `remainder`, `power`) on int, float, byte, bool, Duration, Size, and str. NONE of these appear in `TYPECK_BUILTIN_METHODS` or `resolve_*_method()`. They are dispatched through the operator inference system (`infer/expr/operators/`), not the method resolution system. The enforcement test (Section 14) must account for this: registry methods with `trait_name` matching an operator trait (`Add`, `Sub`, `Mul`, `Div`, `FloorDiv`, `Rem`, `Pow`, `Neg`, `Not`, `BitAnd`, `BitOr`, `BitXor`, `BitNot`, `Shl`, `Shr`) are NOT expected to resolve via `resolve_builtin_method()`.

**Verification task:** After migration, run the new enforcement test (Section 14) that iterates `BUILTIN_TYPES` and verifies every method is resolvable. Any registry method NOT resolvable via `resolve_builtin_method()` or `resolve_computed_return()` must be either (a) an associated function handled elsewhere, (b) a trait operator method handled by operator dispatch, or (c) a genuine gap that must be fixed.

### resolve_named_type_method stays outside the registry

The `resolve_named_type_method` function handles *user-defined* types (structs, enums, newtypes), not builtins. It queries the `TypeRegistry` for newtype unwrap. This function is not part of the builtin registry migration and remains as-is.

---

## 09.1 tag_to_type_tag() Bridge (ori_types::Tag -> ori_registry::TypeTag)

**File:** `compiler/ori_types/src/infer/expr/registry_bridge.rs` (new file, ~180-200 lines)

This function maps the type checker's internal `Tag` enum to the registry's `TypeTag` enum, enabling registry lookups from the type checker's resolved type information.

### Current State

There is no bridge function today. The type checker dispatches directly on `Tag` in `resolve_builtin_method()`:

```rust
// BEFORE: compiler/ori_types/src/infer/expr/methods/mod.rs
pub(crate) fn resolve_builtin_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    tag: Tag,
    method_name: &str,
) -> Option<Idx> {
    match tag {
        Tag::List => resolve_list_method(engine, receiver_ty, method_name),
        Tag::Option => resolve_option_method(engine, receiver_ty, method_name),
        Tag::Result => resolve_result_method(engine, receiver_ty, method_name),
        Tag::Map => resolve_map_method(engine, receiver_ty, method_name),
        Tag::Set => resolve_set_method(engine, receiver_ty, method_name),
        Tag::Str => resolve_str_method(engine, method_name),
        Tag::Int => resolve_int_method(method_name),
        Tag::Float => resolve_float_method(method_name),
        Tag::Duration => resolve_duration_method(method_name),
        Tag::Size => resolve_size_method(method_name),
        Tag::Channel => resolve_channel_method(engine, receiver_ty, method_name),
        Tag::Range => resolve_range_method(engine, receiver_ty, method_name),
        Tag::Iterator | Tag::DoubleEndedIterator => {
            resolve_iterator_method(engine, receiver_ty, method_name)
        }
        Tag::Named | Tag::Applied => resolve_named_type_method(engine, receiver_ty, method_name),
        Tag::Error => resolve_error_method(engine, method_name),
        Tag::Bool => resolve_bool_method(method_name),
        Tag::Byte => resolve_byte_method(method_name),
        Tag::Char => resolve_char_method(method_name),
        Tag::Ordering => resolve_ordering_method(method_name),
        Tag::Tuple => resolve_tuple_method(receiver_ty, method_name),
        _ => None,
    }
}
```

### New Implementation

```rust
// AFTER: compiler/ori_types/src/infer/expr/registry_bridge.rs

use ori_registry::TypeTag;
use crate::Tag;

/// Map the type checker's Tag to the registry's TypeTag.
///
/// Returns `None` for tags that are not builtin types (Named, Applied,
/// Var, Function, etc.) -- these are handled by trait/impl dispatch,
/// not the builtin registry.
#[must_use]
pub(crate) fn tag_to_type_tag(tag: Tag) -> Option<TypeTag> {
    match tag {
        Tag::Int => Some(TypeTag::Int),
        Tag::Float => Some(TypeTag::Float),
        Tag::Bool => Some(TypeTag::Bool),
        Tag::Str => Some(TypeTag::Str),
        Tag::Char => Some(TypeTag::Char),
        Tag::Byte => Some(TypeTag::Byte),
        Tag::Duration => Some(TypeTag::Duration),
        Tag::Size => Some(TypeTag::Size),
        Tag::Ordering => Some(TypeTag::Ordering),
        Tag::Error => Some(TypeTag::Error),
        Tag::List => Some(TypeTag::List),
        Tag::Option => Some(TypeTag::Option),
        Tag::Result => Some(TypeTag::Result),
        Tag::Map => Some(TypeTag::Map),
        Tag::Set => Some(TypeTag::Set),
        Tag::Channel => Some(TypeTag::Channel),
        Tag::Range => Some(TypeTag::Range),
        Tag::Iterator => Some(TypeTag::Iterator),
        Tag::DoubleEndedIterator => Some(TypeTag::DoubleEndedIterator),
        Tag::Tuple => Some(TypeTag::Tuple),
        // Not builtin registry types:
        Tag::Named | Tag::Applied | Tag::Alias
        | Tag::Var | Tag::BoundVar | Tag::RigidVar
        | Tag::Function | Tag::Scheme
        | Tag::Struct | Tag::Enum
        | Tag::Unit | Tag::Never
        | Tag::Borrowed
        | Tag::Projection | Tag::ModuleNs | Tag::Infer | Tag::SelfType => None,
    }
}
```

### Design Notes

- `Tag::Borrowed` returns `None` because it is a reserved-for-future-use variant (value 34) with no methods.
- `Tag::Unit` and `Tag::Never` return `None` because these types have no methods. They are structurally special (unit is `()`, never is bottom) and are never the receiver of a method call.
- `Tag::Named` and `Tag::Applied` return `None` because they represent user-defined types. These go through `resolve_named_type_method` and trait/impl dispatch, which remain unchanged.
- `Tag::Error` maps to `TypeTag::Error` because error has builtin methods (`message`, `to_str`, `trace`, etc.).
- The match is exhaustive -- adding a new `Tag` variant without updating this function is a compile error.

### Tasks

- [x] Create `compiler/ori_types/src/infer/expr/registry_bridge.rs` (directory module: `registry_bridge/mod.rs`)
- [x] Implement `tag_to_type_tag()` with exhaustive match on all `Tag` variants
- [x] Add `#[must_use]` on `tag_to_type_tag()` (returns `Option`)
- [x] Add `mod registry_bridge;` declaration in `compiler/ori_types/src/infer/expr/mod.rs`
- [x] Add `//!` module doc at top of `registry_bridge.rs`
- [x] **Test file**: Create `compiler/ori_types/src/infer/expr/registry_bridge/tests.rs` with `#[cfg(test)] mod tests;` at bottom of `registry_bridge/mod.rs`
- [x] Add unit tests: every `Tag` variant with builtin methods maps to the correct `TypeTag`
- [x] Add unit test: `Tag::Named`, `Tag::Applied`, `Tag::Var`, `Tag::Function` all return `None`
- [x] Verify `cargo c -p ori_types` compiles

---

## 09.2 return_tag_to_idx() Bridge (ori_registry::ReturnTag -> ori_types::Idx)

**File:** `compiler/ori_types/src/infer/expr/registry_bridge.rs` (same file as 09.1)

This is the more complex bridge. It converts the registry's `TypeTag` return types into `Idx` handles in the type pool. Primitive tags are trivial (compile-time constants). Generic/parameterized return types require pool construction.

### The Challenge

A registry `MethodDef` declares `returns: ReturnTag::Concrete(TypeTag::Option)` for a method like `list.first()`. But the type checker needs `Idx` for `Option<T>` where `T` is the list's element type. The bridge function must:

1. Map `ReturnTag::Concrete(TypeTag::X)` to fixed `Idx` constants or pool-constructed types.
2. Map `ReturnTag::SelfType` to the receiver type (`receiver_ty`).
3. Map container-relative tags (`ReturnTag::ElementType`, `ReturnTag::OptionOf(TypeProjection::Element)`, etc.) to pool-constructed types using the receiver's inner type(s).
4. Map `ReturnTag::Fresh` to `engine.pool_mut().fresh_var()` for higher-order methods.

### Implementation

```rust
// AFTER: compiler/ori_types/src/infer/expr/registry_bridge.rs

use ori_registry::{TypeTag, ReturnTag};
use crate::{Idx, Tag};
use super::super::InferEngine;

/// Convert a registry TypeTag to a pool Idx, using the receiver type
/// to resolve parameterized return types.
///
/// `receiver_ty` is the resolved receiver type (e.g., `List<int>`,
/// `Iterator<str>`). Used to extract inner type parameters when the
/// return type is parameterized (e.g., `ReturnTag::Concrete(TypeTag::Option)` on a list
/// means `Option<elem>` where `elem` is the list's element type).
pub(crate) fn return_tag_to_idx(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    return_tag: ReturnTag,
) -> Idx {
    match return_tag {
        // === Concrete types: delegate to TypeTag mapping ===
        ReturnTag::Concrete(type_tag) => match type_tag {
            // Primitives: compile-time constants
            TypeTag::Int => Idx::INT,
            TypeTag::Float => Idx::FLOAT,
            TypeTag::Bool => Idx::BOOL,
            TypeTag::Str => Idx::STR,
            TypeTag::Char => Idx::CHAR,
            TypeTag::Byte => Idx::BYTE,
            TypeTag::Unit => Idx::UNIT,
            TypeTag::Ordering => Idx::ORDERING,
            TypeTag::Duration => Idx::DURATION,
            TypeTag::Size => Idx::SIZE,
            TypeTag::Error => Idx::ERROR,

            // Parameterized: construct in pool
            TypeTag::Option => {
                let elem = extract_elem(engine, receiver_ty);
                engine.pool_mut().option(elem)
            }
            TypeTag::List => {
                let elem = extract_elem(engine, receiver_ty);
                engine.pool_mut().list(elem)
            }
            TypeTag::Set => {
                let elem = extract_elem(engine, receiver_ty);
                engine.pool_mut().set(elem)
            }
            TypeTag::Iterator => {
                let elem = extract_elem(engine, receiver_ty);
                engine.pool_mut().iterator(elem)
            }
            TypeTag::DoubleEndedIterator => {
                let elem = extract_elem(engine, receiver_ty);
                engine.pool_mut().double_ended_iterator(elem)
            }
            TypeTag::Result => {
                let ok_ty = engine.pool().result_ok(receiver_ty);
                let err_ty = engine.pool().result_err(receiver_ty);
                engine.pool_mut().result(ok_ty, err_ty)
            }
            TypeTag::Map => {
                let key_ty = engine.pool().map_key(receiver_ty);
                let value_ty = engine.pool().map_value(receiver_ty);
                engine.pool_mut().map(key_ty, value_ty)
            }

            // Remaining concrete tags — ICE if reached (not currently
            // used as method return types; add pool construction if needed)
            TypeTag::Never | TypeTag::Channel | TypeTag::Range
            | TypeTag::Tuple | TypeTag::Function => {
                unreachable!(
                    "TypeTag::{:?} appeared as a method return type — \
                     add pool construction for this tag",
                    type_tag
                );
            }
        },

        // === Signature-level types (not on TypeTag) ===
        ReturnTag::SelfType => receiver_ty,
        ReturnTag::Fresh => engine.pool_mut().fresh_var(),
        ReturnTag::Unit => Idx::UNIT,

        // === Container-relative types ===
        ReturnTag::ElementType => extract_elem(engine, receiver_ty),
        ReturnTag::OptionOf(proj) => {
            let inner = resolve_projection(engine, receiver_ty, proj);
            engine.pool_mut().option(inner)
        }
        ReturnTag::ListOf(proj) => {
            let inner = resolve_projection(engine, receiver_ty, proj);
            engine.pool_mut().list(inner)
        }
        ReturnTag::IteratorOf(proj) => {
            let inner = resolve_projection(engine, receiver_ty, proj);
            engine.pool_mut().iterator(inner)
        }
        ReturnTag::DoubleEndedIteratorOf(proj) => {
            let inner = resolve_projection(engine, receiver_ty, proj);
            engine.pool_mut().double_ended_iterator(inner)
        }
        ReturnTag::OkType => extract_ok(engine, receiver_ty),
        ReturnTag::ErrType => extract_err(engine, receiver_ty),
        ReturnTag::KeyType => engine.pool().map_key(receiver_ty),
        ReturnTag::ValueType => engine.pool().map_value(receiver_ty),
        ReturnTag::ListKeyValue => {
            let key_ty = engine.pool().map_key(receiver_ty);
            let value_ty = engine.pool().map_value(receiver_ty);
            let tuple = engine.pool_mut().tuple(&[key_ty, value_ty]);
            engine.pool_mut().list(tuple)
        }

        // === Fixed-inner wrappers ===
        ReturnTag::List(inner_tag) => {
            let inner = type_tag_to_idx(inner_tag);
            engine.pool_mut().list(inner)
        }
        ReturnTag::Option(inner_tag) => {
            let inner = type_tag_to_idx(inner_tag);
            engine.pool_mut().option(inner)
        }
        ReturnTag::DoubleEndedIterator(inner_tag) => {
            let inner = type_tag_to_idx(inner_tag);
            engine.pool_mut().double_ended_iterator(inner)
        }

        // === Composite returns ===
        ReturnTag::ListOfTupleIntElement => {
            let elem = extract_elem(engine, receiver_ty);
            let pair = engine.pool_mut().tuple(&[Idx::INT, elem]);
            engine.pool_mut().list(pair)
        }
        ReturnTag::MapIterator => {
            let key_ty = engine.pool().map_key(receiver_ty);
            let value_ty = engine.pool().map_value(receiver_ty);
            let pair = engine.pool_mut().tuple(&[key_ty, value_ty]);
            engine.pool_mut().iterator(pair)
        }
        ReturnTag::IteratorOfTupleIntElement => {
            let elem = extract_elem(engine, receiver_ty);
            let pair = engine.pool_mut().tuple(&[Idx::INT, elem]);
            engine.pool_mut().iterator(pair)
        }
        ReturnTag::NextResult => {
            let elem = extract_elem(engine, receiver_ty);
            let option_elem = engine.pool_mut().option(elem);
            engine.pool_mut().tuple(&[option_elem, receiver_ty])
        }
        ReturnTag::ResultOfProjectionFresh(proj) => {
            let ok_ty = resolve_projection(engine, receiver_ty, proj);
            let err_ty = engine.pool_mut().fresh_var();
            engine.pool_mut().result(ok_ty, err_ty)
        }
    }
}

/// Extract the primary element type from a container receiver.
///
/// For `List<T>`, `Set<T>`, `Option<T>`, `Iterator<T>`, `DEI<T>`,
/// `Channel<T>`, `Range<T>`: returns `T`.
/// For `Map<K, V>`: returns `K` (the primary element; use map-specific
/// extraction for V).
///
/// # Panics
///
/// ICE if called on a non-container type (primitive, tuple). The caller
/// must ensure the receiver is a container before calling this function.
fn extract_elem(engine: &InferEngine<'_>, receiver_ty: Idx) -> Idx {
    let tag = engine.pool().tag(receiver_ty);
    match tag {
        Tag::List => engine.pool().list_elem(receiver_ty),
        Tag::Set => engine.pool().set_elem(receiver_ty),
        Tag::Option => engine.pool().option_inner(receiver_ty),
        Tag::Iterator | Tag::DoubleEndedIterator => engine.pool().iterator_elem(receiver_ty),
        Tag::Channel => engine.pool().channel_elem(receiver_ty),
        Tag::Range => engine.pool().range_elem(receiver_ty),
        Tag::Map => engine.pool().map_key(receiver_ty),
        Tag::Result => engine.pool().result_ok(receiver_ty),
        _ => unreachable!("extract_elem called on non-container type {tag:?}"),
    }
}
```

### Helper: type_tag_to_idx (TypeTag -> Idx constant)

```rust
/// Map a registry TypeTag to a pool Idx constant.
///
/// Only handles concrete types with fixed Idx constants. Panics on
/// parameterized types (List, Map, etc.) — those require pool construction
/// and should not appear inside `ReturnTag::List(TypeTag)` etc.
fn type_tag_to_idx(tag: TypeTag) -> Idx {
    match tag {
        TypeTag::Int => Idx::INT,
        TypeTag::Float => Idx::FLOAT,
        TypeTag::Bool => Idx::BOOL,
        TypeTag::Str => Idx::STR,
        TypeTag::Char => Idx::CHAR,
        TypeTag::Byte => Idx::BYTE,
        TypeTag::Unit => Idx::UNIT,
        TypeTag::Ordering => Idx::ORDERING,
        TypeTag::Duration => Idx::DURATION,
        TypeTag::Size => Idx::SIZE,
        TypeTag::Error => Idx::ERROR,
        _ => unreachable!(
            "type_tag_to_idx: TypeTag::{tag:?} is parameterized — \
             cannot appear as a fixed inner type in ReturnTag wrappers"
        ),
    }
}
```

### Helper: resolve_projection (TypeProjection -> Idx)

```rust
/// Map a TypeProjection to a concrete Idx from the receiver type.
fn resolve_projection(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    proj: TypeProjection,
) -> Idx {
    match proj {
        TypeProjection::Element => extract_elem(engine, receiver_ty),
        TypeProjection::Key => engine.pool().map_key(receiver_ty),
        TypeProjection::Value => engine.pool().map_value(receiver_ty),
        TypeProjection::Ok => engine.pool().result_ok(receiver_ty),
        TypeProjection::Err => engine.pool().result_err(receiver_ty),
        TypeProjection::Fixed(tag) => type_tag_to_idx(tag),
    }
}
```

### Design Notes

- **`ReturnTag::SelfType`** resolves to the receiver type. For `Clone` trait's `clone()` method on `int`, the return is `ReturnTag::SelfType` which resolves to `Idx::INT`. For `list.clone()`, it resolves to the `List<T>` type. This is the most common return type for trait methods.
- **`ReturnTag::Fresh`** creates a fresh type variable. This is used for higher-order methods like `map`, `flat_map`, `fold` where the return type depends on the closure argument and cannot be statically determined from the registry alone. The `unify_higher_order_constraints` function in `calls/method_call.rs` (line 165) resolves these variables later.
- **`extract_elem`** is the key helper -- it extracts the "primary inner type" from any container. For most containers this is the element type `T`. For `Map<K, V>` it returns `K`. Methods that need `V` (like `map.values()`) use direct pool accessors.
- **`resolve_projection`** maps a `TypeProjection` variant to a pool `Idx` from the receiver type: `Element` → `extract_elem()`, `Key` → `pool.map_key()`, `Value` → `pool.map_value()`, `Ok` → `pool.result_ok()`, `Err` → `pool.result_err()`, `Fixed(tag)` → `tag_to_idx(tag)`. This enables the `OptionOf`/`ListOf`/`IteratorOf`/`DoubleEndedIteratorOf` variants to compose with any projection.
- **`TypeTag::Channel`/`Range`/`Tuple` as return types**: Currently no method returns these types (a Channel method never returns a new Channel; Range methods return List/Iterator/Int/Bool; Tuple is only used in computed return types like `enumerate`). The `extract_elem` function uses `unreachable!()` for non-container types per the "no silent fallbacks" rule (frozen decision #7).

### Tasks

- [x] Implement `return_tag_to_idx()` in `registry_bridge.rs` — all 22 ReturnTag variants
- [x] Implement `extract_elem()` helper
- [x] Implement `resolve_projection()` helper (TypeProjection → Idx)
- [x] Implement `type_tag_to_idx()` helper (TypeTag → Idx for fixed concrete types)
- [x] Add unit tests: `ReturnTag::Concrete(TypeTag::Int)` -> `Idx::INT`, etc. (all primitives)
- [x] Add unit test: `ReturnTag::SelfType` -> receiver_ty
- [x] Add unit test: `ReturnTag::OptionOf(TypeProjection::Element)` on `List<int>` receiver -> `Option<int>`
- [x] Add unit test: `ReturnTag::ListOf(TypeProjection::Element)` on `Set<str>` receiver -> `[str]`
- [x] Add unit test: `ReturnTag::IteratorOf(TypeProjection::Element)` on `List<int>` receiver -> `Iterator<int>`
- [x] Add unit test: `ReturnTag::Fresh` -> fresh var (check it's a `Tag::Var`)
- [x] Add unit test: `ReturnTag::List(TypeTag::Byte)` -> `[byte]` (fixed-inner wrapper)
- [x] Add unit test: `ReturnTag::Option(TypeTag::Int)` -> `Option<int>` (fixed-inner wrapper)
- [x] Add unit test: `ReturnTag::DoubleEndedIterator(TypeTag::Char)` -> `DEI<char>`
- [x] Add unit test: `ReturnTag::NextResult` on `Iterator<int>` -> `(Option<int>, Iterator<int>)`
- [x] Add unit test: `ReturnTag::ResultOfProjectionFresh(Fixed(Str))` on str -> `Result<str, fresh>`
- [x] Add unit test: `ReturnTag::MapIterator` on `Map<str, int>` -> `Iterator<(str, int)>`
- [x] Add unit test: `ReturnTag::ListKeyValue` on `Map<str, int>` -> `[(str, int)]`
- [x] Add unit test: `ReturnTag::ListOfTupleIntElement` on `List<str>` -> `[(int, str)]`
- [x] Add unit test: `ReturnTag::IteratorOfTupleIntElement` on `Iterator<str>` -> `Iterator<(int, str)>`
- [x] Verify `cargo c -p ori_types` compiles

---

## 09.3 Replace resolve_builtin_method() Dispatcher

**File:** `compiler/ori_types/src/infer/expr/methods/mod.rs`

The central dispatcher `resolve_builtin_method()` currently matches on `Tag` and dispatches to 20 type-specific functions (in `methods/resolve_by_type.rs`). After migration, it performs a single registry lookup and converts the result.

### Current Implementation

See the full listing in 09.1 above. The function is a match dispatching to 20 type-specific resolvers, all defined in `methods/resolve_by_type.rs`.

### New Implementation

```rust
// AFTER: compiler/ori_types/src/infer/expr/methods/mod.rs

use super::registry_bridge::{tag_to_type_tag, return_tag_to_idx};
use computed_returns::resolve_computed_return;

/// Resolve a built-in method call on a known type tag.
///
/// Performs a single registry lookup via `ori_registry::find_method()`,
/// then converts the return TypeTag to an Idx via `return_tag_to_idx()`.
///
/// Methods with computed return types (e.g., list.enumerate, iterator.next,
/// list.zip) fall through to `resolve_computed_return()` which handles
/// the pool construction that the registry's static TypeTag cannot express.
pub(crate) fn resolve_builtin_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    tag: Tag,
    method_name: &str,
) -> Option<Idx> {
    // Named/Applied types are not in the builtin registry
    if matches!(tag, Tag::Named | Tag::Applied) {
        return resolve_named_type_method(engine, receiver_ty, method_name);
    }

    // Try computed-return methods first (these need pool construction
    // beyond what a static TypeTag can express)
    if let Some(idx) = resolve_computed_return(engine, receiver_ty, tag, method_name) {
        return Some(idx);
    }

    // Registry lookup: tag -> TypeTag -> find_method -> TypeTag -> Idx
    let type_tag = tag_to_type_tag(tag)?;
    let method_def = ori_registry::find_method(type_tag, method_name)?;
    Some(return_tag_to_idx(engine, receiver_ty, method_def.returns))
}
```

### resolve_computed_return(): Methods Requiring Pool Construction

Some methods have return types that depend on runtime pool state and cannot be expressed as a single static `TypeTag`. These are handled by a dedicated function that returns `Some(Idx)` for computed cases and `None` to fall through to the registry lookup.

> **WARNING: Function size violation.** The code sample below shows the LOGICAL content of `resolve_computed_return()` as a single function for clarity, but implementation MUST split this into per-type helpers as described above. The monolithic version is ~238 lines and would be a BLOAT finding.

**File:** `compiler/ori_types/src/infer/expr/methods/computed_returns.rs` (new file, ~250 lines)

```rust
/// Methods whose return types require dynamic pool construction.
///
/// These are methods where the static `TypeTag` in the registry is not
/// sufficient. The registry declares them with `ReturnTag::Fresh` or
/// a simpler approximation, but the type checker needs precise types.
///
/// Dispatches to per-type helpers to stay within function size limits.
#[must_use]
pub(super) fn resolve_computed_return(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    tag: Tag,
    method: &str,
) -> Option<Idx> {
    match tag {
        Tag::List => computed_list_return(engine, receiver_ty, method),
        Tag::Option => computed_option_return(engine, receiver_ty, method),
        Tag::Result => computed_result_return(engine, receiver_ty, method),
        Tag::Map => computed_map_return(engine, receiver_ty, method),
        Tag::Set => computed_set_return(engine, receiver_ty, method),
        Tag::Str => computed_str_return(engine, receiver_ty, method),
        Tag::Channel => computed_channel_return(engine, receiver_ty, method),
        Tag::Range => computed_range_return(engine, receiver_ty, method),
        Tag::Iterator | Tag::DoubleEndedIterator =>
            computed_iterator_return(engine, receiver_ty, tag, method),
        Tag::Error => computed_error_return(engine, receiver_ty, method),
        _ => None,
    }
}
```

The per-type helpers below show the match arms grouped by type. Each helper is a standalone function in `computed_returns.rs`:

```rust
// --- Per-type helpers (each 15-40 lines) ---

fn computed_list_return(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    let elem = engine.pool().list_elem(receiver_ty);
    match method {
        "first" | "last" | "pop" | "get" => Some(engine.pool_mut().option(elem)),
        "iter" => Some(engine.pool_mut().double_ended_iterator(elem)),
        "enumerate" => {
            let pair = engine.pool_mut().tuple(&[Idx::INT, elem]);
            Some(engine.pool_mut().list(pair))
        }
        "zip" => {
            let other_elem = engine.pool_mut().fresh_var();
            let pair = engine.pool_mut().tuple(&[elem, other_elem]);
            Some(engine.pool_mut().list(pair))
        }
        // Higher-order list methods — return type depends on closure argument.
        // Return fresh var; unify_higher_order_constraints resolves later.
        "map" | "filter" | "flat_map" | "find" | "any" | "all"
            | "fold" | "reduce" | "for_each" | "take_while" | "skip_while"
            | "chunk" | "window" | "min" | "max" | "sum" | "product"
            | "min_by" | "max_by" | "sort_by" | "group_by" | "partition" => {
            Some(engine.pool_mut().fresh_var())
        }
        _ => None,
    }
}

fn computed_option_return(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    match method {
        "unwrap" | "expect" | "unwrap_or" => {
            Some(engine.pool().option_inner(receiver_ty))
        }
        "ok_or" => {
            let inner = engine.pool().option_inner(receiver_ty);
            let err_ty = engine.pool_mut().fresh_var();
            Some(engine.pool_mut().result(inner, err_ty))
        }
        "iter" => {
            let inner = engine.pool().option_inner(receiver_ty);
            Some(engine.pool_mut().iterator(inner))
        }
        // Higher-order option methods — return type depends on closure
        "map" | "and_then" | "flat_map" | "filter" | "or_else" => {
            Some(engine.pool_mut().fresh_var())
        }
        _ => None,
    }
}

fn computed_map_return(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    match method {
        "get" => {
            let value_ty = engine.pool().map_value(receiver_ty);
            Some(engine.pool_mut().option(value_ty))
        }
        "iter" => {
            let key_ty = engine.pool().map_key(receiver_ty);
            let value_ty = engine.pool().map_value(receiver_ty);
            let pair = engine.pool_mut().tuple(&[key_ty, value_ty]);
            Some(engine.pool_mut().iterator(pair))
        }
        "keys" => {
            let key_ty = engine.pool().map_key(receiver_ty);
            Some(engine.pool_mut().list(key_ty))
        }
        "values" => {
            let value_ty = engine.pool().map_value(receiver_ty);
            Some(engine.pool_mut().list(value_ty))
        }
        "entries" => {
            let key_ty = engine.pool().map_key(receiver_ty);
            let value_ty = engine.pool().map_value(receiver_ty);
            let pair = engine.pool_mut().tuple(&[key_ty, value_ty]);
            Some(engine.pool_mut().list(pair))
        }
        _ => None,
    }
}

fn computed_set_return(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    match method {
        "to_list" | "into" => {
            let elem = engine.pool().set_elem(receiver_ty);
            Some(engine.pool_mut().list(elem))
        }
        _ => None,
    }
}

fn computed_str_return(
    engine: &mut InferEngine<'_>,
    _receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    match method {
        "chars" => Some(engine.pool_mut().list(Idx::CHAR)),
        "bytes" => Some(engine.pool_mut().list(Idx::BYTE)),
        "split" | "lines" => Some(engine.pool_mut().list(Idx::STR)),
        "index_of" | "last_index_of" | "to_int" | "parse_int" => {
            Some(engine.pool_mut().option(Idx::INT))
        }
        "to_float" | "parse_float" => Some(engine.pool_mut().option(Idx::FLOAT)),
        "into" => Some(Idx::ERROR),  // str.into() -> Error type
        "iter" => Some(engine.pool_mut().double_ended_iterator(Idx::CHAR)),
        "as_bytes" | "to_bytes" => Some(engine.pool_mut().list(Idx::BYTE)),
        // str.from_utf8 is an associated function (MethodKind::Associated).
        // Associated function dispatch flows through a different path in the type checker
        // (see resolve_associated_function in calls/). This computed return case only
        // fires if the type checker resolves it through the builtin method path.
        "from_utf8" => {
            let err_ty = engine.pool_mut().fresh_var();
            Some(engine.pool_mut().result(Idx::STR, err_ty))
        }
        "from_utf8_unchecked" => Some(Idx::STR),
        _ => None,
    }
}

fn computed_result_return(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    match method {
        "unwrap" | "expect" | "unwrap_or" => {
            Some(engine.pool().result_ok(receiver_ty))
        }
        "unwrap_err" | "expect_err" => {
            Some(engine.pool().result_err(receiver_ty))
        }
        "ok" => {
            let ok_ty = engine.pool().result_ok(receiver_ty);
            Some(engine.pool_mut().option(ok_ty))
        }
        "err" => {
            let err_ty = engine.pool().result_err(receiver_ty);
            Some(engine.pool_mut().option(err_ty))
        }
        // Higher-order result methods — return type depends on closure
        "map" | "map_err" | "and_then" | "or_else" => {
            Some(engine.pool_mut().fresh_var())
        }
        "trace_entries" => computed_trace_entries(engine),
        _ => None,
    }
}

fn computed_channel_return(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    match method {
        "recv" | "receive" | "try_recv" | "try_receive" => {
            let elem = engine.pool().channel_elem(receiver_ty);
            Some(engine.pool_mut().option(elem))
        }
        _ => None,
    }
}

fn computed_range_return(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    let elem = engine.pool().range_elem(receiver_ty);
    // Range<float> iteration rejection is handled in
    // resolve_receiver_and_builtin() via check_range_float_iteration()
    match method {
        "iter" => Some(engine.pool_mut().double_ended_iterator(elem)),
        "to_list" | "collect" => Some(engine.pool_mut().list(elem)),
        _ => None,
    }
}

fn computed_iterator_return(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    tag: Tag,
    method: &str,
) -> Option<Idx> {
    let elem = engine.pool().iterator_elem(receiver_ty);
    let is_dei = tag == Tag::DoubleEndedIterator;
    match method {
        "next" => {
            let option_elem = engine.pool_mut().option(elem);
            Some(engine.pool_mut().tuple(&[option_elem, receiver_ty]))
        }
        "next_back" if is_dei => {
            let option_elem = engine.pool_mut().option(elem);
            Some(engine.pool_mut().tuple(&[option_elem, receiver_ty]))
        }
        "map" => {
            let new_elem = engine.pool_mut().fresh_var();
            if is_dei {
                Some(engine.pool_mut().double_ended_iterator(new_elem))
            } else {
                Some(engine.pool_mut().iterator(new_elem))
            }
        }
        "filter" => Some(receiver_ty), // preserves iterator kind
        "take" | "skip" | "chain" | "cycle" => {
            Some(engine.pool_mut().iterator(elem))
        }
        "flatten" | "flat_map" => {
            let new_elem = engine.pool_mut().fresh_var();
            Some(engine.pool_mut().iterator(new_elem))
        }
        "enumerate" => {
            let pair = engine.pool_mut().tuple(&[Idx::INT, elem]);
            Some(engine.pool_mut().iterator(pair))
        }
        "zip" => {
            let other_elem = engine.pool_mut().fresh_var();
            let pair = engine.pool_mut().tuple(&[elem, other_elem]);
            Some(engine.pool_mut().iterator(pair))
        }
        "collect" => Some(engine.pool_mut().list(elem)),
        "find" => Some(engine.pool_mut().option(elem)),
        "last" | "rfind" if is_dei => Some(engine.pool_mut().option(elem)),
        "rev" if is_dei => Some(receiver_ty),
        // fold/rfold/count/join/any/all/for_each are handled by registry
        // (simple return types: fresh, INT, STR, BOOL, UNIT)
        _ => None,
    }
}

fn computed_error_return(
    engine: &mut InferEngine<'_>,
    _receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    match method {
        "trace_entries" => computed_trace_entries(engine),
        _ => None,
    }
}

// Shared helper for Error.trace_entries and Result.trace_entries.
// Current typeck returns unconstrained fresh_var() for both
// (resolve_by_type.rs:310 and resolve_by_type.rs:88). Migration
// constructs [TraceEntry] explicitly.
//
// Requires interner (set_interner must be called in all contexts,
// including tests — see test infrastructure task).
fn computed_trace_entries(engine: &mut InferEngine<'_>) -> Option<Idx> {
    let name = engine.intern_name("TraceEntry")
        .expect("trace_entries bridge requires interner — call set_interner() in test setup");
    let trace_entry = engine.pool_mut().named(name);
    Some(engine.pool_mut().list(trace_entry))
}
```

### Inventory of All resolve_* Functions and Their Fate

| Function | Lines (resolve_by_type.rs) | Simple Lookup? | Computed Returns | Migration Path |
|----------|---------------------------|---------------|-----------------|----------------|
| `resolve_int_method` | 167-179 | YES | None | Fully replaced by registry lookup |
| `resolve_float_method` | 181-193 | YES | None | Fully replaced by registry lookup |
| `resolve_bool_method` | 315-323 | YES | None | Fully replaced by registry lookup |
| `resolve_byte_method` | 325-337 | YES | None | Fully replaced by registry lookup |
| `resolve_char_method` | 339-349 | YES | None | Fully replaced by registry lookup |
| `resolve_ordering_method` | 290-303 | YES | None | Fully replaced by registry lookup |
| `resolve_duration_method` | 195-210 | YES | None | Fully replaced by registry lookup |
| `resolve_size_method` | 212-226 | YES | None | Fully replaced by registry lookup |
| `resolve_tuple_method` | 422-431 | YES | None | Fully replaced by registry lookup |
| `resolve_error_method` | 305-313 | MOSTLY | `trace_entries` -> `[TraceEntry]` (bridge) | 1 computed case (fix bare fresh_var), rest via registry |
| `resolve_str_method` | 143-165 | MOSTLY | `chars`, `bytes`, `split`, `lines`, `index_of`, `last_index_of`, `to_int`, `parse_int`, `to_float`, `parse_float`, `into`, `iter` | 12 computed cases |
| `resolve_channel_method` | 228-241 | MOSTLY | `recv`/`receive`/`try_recv`/`try_receive` -> `Option<T>` | 4 computed cases |
| `resolve_list_method` | 10-46 | NO | `first`/`last`/`pop`/`get`, `iter`, `enumerate`, `zip`, + 20 HO methods | Complex, many computed |
| `resolve_option_method` | 48-71 | NO | `ok_or` -> Result, `iter`, HO methods | Several computed |
| `resolve_result_method` | 73-95 | NO | `ok`, `err`, HO methods, `trace_entries` -> `[TraceEntry]` (bridge, same as Error) | Several computed |
| `resolve_map_method` | 97-122 | NO | `get`, `iter`, `keys`, `values`, `entries` | 5 computed cases |
| `resolve_set_method` | 124-141 | MOSTLY | `to_list`/`into` -> List<T> | 2 computed cases |
| `resolve_range_method` | 243-261 | NO | `iter`, `to_list`/`collect` + float rejection | 3 computed + special logic |
| `resolve_iterator_method` | 358-420 | NO | `next`/`next_back`, `map`, `filter`, adapters, consumers | Nearly all computed |
| `resolve_named_type_method` | 266-287 | N/A | Newtype unwrap | NOT migrated (user-defined types) |

**Summary:**
- **9 functions** (int, float, bool, byte, char, ordering, duration, size, tuple) are **trivially replaced** -- every match arm is a static type tag.
- **4 functions** (error, str, channel, set) are **mostly replaced** -- a few arms need computed returns, rest are static.
- **5 functions** (list, option, result, map, range) have **significant computed returns** -- many arms need pool construction.
- **1 function** (iterator) is **almost entirely computed** -- the registry provides existence checking but nearly every return type requires pool work.
- **1 function** (named_type) is **not migrated** -- handles user-defined types.

### Tasks

- [x] Implement new `resolve_builtin_method()` dispatcher with registry lookup in `methods/mod.rs`
- [x] Create `compiler/ori_types/src/infer/expr/methods/computed_returns.rs` (new file)
- [x] Add `mod computed_returns;` declaration in `methods/mod.rs`
- [x] Add `//!` module doc at top of `computed_returns.rs`
- [x] Implement `resolve_computed_return()` as a thin dispatcher delegating to per-type helpers
- [x] Implement per-type helpers for methods needing computed returns. Only 2 helpers needed (not 10) — `return_tag_to_idx()` handles all non-Fresh cases via the registry's expressive `ReturnTag` model:
  - `computed_list_return`: `zip` → `List<(T, U)>` (all other Fresh list methods → `fresh_var()`)
  - `computed_iterator_return`: `map` (DEI propagation), `zip` → `Iterator<(T, U)>`, `flatten`/`flat_map` → `Iterator<U>`
  - Option, Result, Map, Set, Str, Channel, Range, Error: all handled by `return_tag_to_idx()` (OptionOf, ListOf, ResultOfProjectionFresh, etc.) — no computed returns needed
- [x] Implement shared `computed_trace_entries()` helper (used by both Result and Error) — constructs `[TraceEntry]` via `intern_name("TraceEntry")` with `fresh_var()` fallback when interner unavailable
- [x] **Verify file size**: `computed_returns.rs` is ~90 lines (well under 500-line limit)
- [x] **Verify `resolve_receiver_and_builtin` unchanged**: `check_infinite_iterator_consumed`, `check_range_float_iteration`, and `unify_higher_order_constraints` continue to work (4169 spec tests pass)
- [x] **Update test infrastructure:** Added `test_engine!` macro to `infer/expr/tests.rs` — all 99 `InferEngine::new` call sites now have interner set. Two-arg `test_engine!(pool, engine)` and three-arg `test_engine!(interner, pool, engine)` forms. Fixed `test_infer_ident_unbound` to use interned name instead of `Name::from_raw(999)`. 551 lib tests + 4169 spec tests pass.
- [x] Verify computed returns match current behavior exactly (use existing tests) — `trace_entries` intentional change deferred pending test infrastructure update
- [x] Delete the 9 trivially-replaced functions after tests pass
- [x] Delete the remaining 10 type-specific functions after computed returns are verified
- [x] Keep `resolve_named_type_method()` unchanged
- [x] Verify `cargo t -p ori_types` passes (551 passed)
- [x] Verify `cargo st` passes (4169 passed, 0 failed)

---

## 09.4 Replace All resolve_*_method Functions

This subsection is the detailed execution plan for deleting each of the 19 type-specific resolver functions (all except `resolve_named_type_method`). Each deletion is verified independently.

### Phase A: Trivial Replacements (9 functions)

These functions contain ONLY static return type mappings. Every match arm maps to a fixed `Idx` constant. The registry lookup + `return_tag_to_idx()` handles them completely.

**1. `resolve_int_method` (lines 167-179)**
```rust
// DELETE: Every arm maps to a constant
"abs"|"min"|"max"|"clamp"|"pow"|"signum"|"clone"|"hash" => Idx::INT
"to_float"|"into"|"f" => Idx::FLOAT
"to_str"|"debug" => Idx::STR
"to_byte"|"byte" => Idx::BYTE
"is_positive"|"is_negative"|"is_zero"|"is_even"|"is_odd"|"equals" => Idx::BOOL
"compare" => Idx::ORDERING
```
**Registry coverage:** INT TypeDef (Section 03.1) covers all 22 methods.

**2. `resolve_float_method` (lines 181-193)**
```rust
// DELETE: Every arm maps to a constant
"abs"|"sqrt"|...|"clone" => Idx::FLOAT
"floor"|"ceil"|"round"|"trunc"|"to_int"|"hash" => Idx::INT
"to_str"|"debug" => Idx::STR
"is_nan"|...|"equals" => Idx::BOOL
"compare" => Idx::ORDERING
```
**Registry coverage:** FLOAT TypeDef (Section 03.2) covers all 37 methods.

**3. `resolve_bool_method` (lines 315-323)**
```rust
// DELETE: Every arm maps to a constant
"to_str"|"debug" => Idx::STR
"to_int"|"hash" => Idx::INT
"clone"|"equals" => Idx::BOOL
"compare" => Idx::ORDERING
```
**Registry coverage:** BOOL TypeDef (Section 03.3) covers all 7 methods.

**4. `resolve_byte_method` (lines 325-337)**
```rust
// DELETE: Every arm maps to a constant
"to_int"|"hash" => Idx::INT
"to_char" => Idx::CHAR
"to_str"|"debug" => Idx::STR
"is_ascii"|...|"equals" => Idx::BOOL
"clone" => Idx::BYTE
"compare" => Idx::ORDERING
```
**Registry coverage:** BYTE TypeDef (Section 03.4) covers all 12 methods.

**5. `resolve_char_method` (lines 339-349)**
```rust
// DELETE: Every arm maps to a constant
"to_str"|"debug" => Idx::STR
"to_int"|"to_byte"|"hash" => Idx::INT
"is_digit"|...|"equals" => Idx::BOOL
"to_uppercase"|"to_lowercase"|"clone" => Idx::CHAR
"compare" => Idx::ORDERING
```
**Registry coverage:** CHAR TypeDef (Section 03.5) covers all 16 methods.

**6. `resolve_ordering_method` (lines 290-303)**
```rust
// DELETE: Every arm maps to a constant
"is_less"|...|"equals" => Idx::BOOL
"reverse"|"clone"|"compare"|"then"|"then_with" => Idx::ORDERING
"hash" => Idx::INT
"to_str"|"debug" => Idx::STR
```
**Registry coverage:** ORDERING TypeDef (Section 05) covers all 14 methods.

**7. `resolve_duration_method` (lines 195-210)**
```rust
// DELETE: Every arm maps to a constant
"to_seconds"|...|"as_nanos" => Idx::FLOAT
"to_str"|"format"|"debug" => Idx::STR
"abs"|"from_*"|"zero"|"clone" => Idx::DURATION
"is_zero"|...|"equals" => Idx::BOOL
"nanoseconds"|...|"hash" => Idx::INT
"compare" => Idx::ORDERING
```
**Registry coverage:** DURATION TypeDef (Section 05) covers all methods.

**8. `resolve_size_method` (lines 212-226)**
```rust
// DELETE: Every arm maps to a constant
"to_bytes"|"as_bytes"|"to_kb"|...|"hash" => Idx::INT
"to_str"|"format"|"debug" => Idx::STR
"is_zero"|"equals" => Idx::BOOL
"from_bytes"|...|"zero"|"clone" => Idx::SIZE
"compare" => Idx::ORDERING
```
**Registry coverage:** SIZE TypeDef (Section 05) covers all methods.

**9. `resolve_tuple_method` (lines 422-431)**
```rust
// DELETE: Every arm maps to a constant or receiver_ty
"len"|"hash" => Idx::INT
"compare" => Idx::ORDERING
"equals" => Idx::BOOL
"clone" => receiver_ty  // ReturnTag::SelfType handles this
"debug" => Idx::STR
```
**Registry coverage:** TUPLE TypeDef (Section 06) covers all 5 methods.

### Phase B: Mostly-Static Replacements (4 functions)

These functions have a few computed cases that go into `resolve_computed_return()`, with the remaining arms handled by the registry.

**10. `resolve_error_method` (lines 305-313)**
- Static: `message`/`to_str`/`debug`/`trace` -> STR, `has_trace` -> BOOL, `clone`/`with_trace` -> ERROR
- Computed: `trace_entries` -> `[TraceEntry]` (handled in `resolve_computed_return`; **migration must replace current bare `fresh_var()` with explicit `list(TraceEntry)` construction** — see Section 05 bridge task)
- **7 static, 1 computed**

**11. `resolve_str_method` (lines 143-165)**
- Static: `len`/`byte_len`/`hash`/`length` -> INT, `is_empty`/.../`equals` -> BOOL, `to_uppercase`/.../`to_str` -> STR, `compare` -> ORDERING
- Computed: `chars` -> `List<Char>`, `bytes` -> `List<Byte>`, `split`/`lines` -> `List<Str>`, `index_of`/`last_index_of`/`to_int`/`parse_int` -> `Option<Int>`, `to_float`/`parse_float` -> `Option<Float>`, `into` -> `Idx::ERROR`, `iter` -> `DEI<Char>`
- **~25 static, 12 computed** (note: `iter` returns `DEI<Char>` which can be registry-expressed as `ReturnTag::DoubleEndedIterator(TypeTag::Char)`, but str is not a generic container -- we handle it as computed)

**12. `resolve_channel_method` (lines 228-241)**
- Static: `send`/`close` -> UNIT, `is_closed`/`is_empty` -> BOOL, `len` -> INT
- Computed: `recv`/`receive`/`try_recv`/`try_receive` -> `Option<T>`
- **5 static, 4 computed**

**13. `resolve_set_method` (lines 124-141)**
- Static: `len`/`hash` -> INT, `is_empty`/`contains`/`equals` -> BOOL, `insert`/.../`clone` -> SelfType, `iter` -> Iterator<T>, `debug` -> STR
- Computed: `to_list`/`into` -> `List<T>`
- **~10 static (iter handled by registry as TypeTag::Iterator), 2 computed**

### Phase C: Complex Replacements (5 functions)

These functions have many computed returns and require extensive `resolve_computed_return` coverage.

**14. `resolve_list_method` (lines 10-46)**
- Static via registry: `len`/`length`/`count`/`hash` -> INT, `is_empty`/`contains`/`equals` -> BOOL, `compare` -> ORDERING, `join`/`debug` -> STR, `reverse`/`sort`/.../`clone` -> SelfType (16 arms)
- Computed: `first`/`last`/`pop`/`get` -> `Option<T>`, `iter` -> `DEI<T>`, `enumerate` -> `[(int, T)]`, `zip` -> `[(T, U)]`, 21 HO methods -> `fresh_var()` (28 arms)

**15. `resolve_option_method` (lines 48-71)**
- Static via registry: `is_some`/`is_none`/`equals` -> BOOL, `compare` -> ORDERING, `hash` -> INT, `or`/`clone` -> SelfType, `debug` -> STR (8 arms)
- Computed: `unwrap`/`expect`/`unwrap_or` -> inner, `ok_or` -> `Result<T, E>`, `iter` -> `Iterator<T>`, 5 HO methods -> `fresh_var()` (10 arms)

**16. `resolve_result_method` (lines 73-95)**
- Static via registry: `is_ok`/`is_err`/`has_trace`/`equals` -> BOOL, `compare` -> ORDERING, `hash` -> INT, `clone` -> SelfType, `debug`/`trace` -> STR (9 arms)
- Computed: `unwrap`/`expect`/`unwrap_or` -> ok_ty, `unwrap_err`/`expect_err` -> err_ty, `ok` -> `Option<T>`, `err` -> `Option<E>`, `trace_entries` -> `[TraceEntry]`, 4 HO methods -> `fresh_var()` (12 arms)

**17. `resolve_map_method` (lines 97-122)**
- Static via registry: `len`/`length`/`hash` -> INT, `is_empty`/`contains_key`/`contains`/`equals` -> BOOL, `insert`/.../`clone` -> SelfType, `debug` -> STR (12 arms)
- Computed: `get` -> `Option<V>`, `iter` -> `Iterator<(K, V)>`, `keys` -> `[K]`, `values` -> `[V]`, `entries` -> `[(K, V)]` (5 arms)

**18. `resolve_range_method` (lines 243-261)** -- 3 computed + float rejection logic
- Static via registry: `len`/`count` -> INT, `is_empty`/`contains` -> BOOL, `step_by` -> SelfType (5 arms)
- Computed: `iter` -> `DEI<T>`, `to_list`/`collect` -> `[T]` (3 arms, with `Range<float>` rejection)

### Phase D: Iterator (special case)

> **WARNING: High complexity.** The iterator migration has the most behavioral subtleties of any type. DEI-to-Iterator downgrade semantics, filter's kind preservation, and conditional DEI-only arms (`last`/`rfind`/`rev`) must be tested exhaustively before the old function is deleted.

**19. `resolve_iterator_method` (lines 358-420)** -- Nearly entirely computed.

Every adapter and consumer has a computed return type:
- `next`/`next_back` -> `(Option<T>, Iterator<T>)` tuple
- `map` -> Iterator/DEI propagation with fresh var
- `filter` -> preserves receiver kind
- `take`/`skip`/`chain`/`cycle` -> downgrade to Iterator
- `flatten`/`flat_map` -> Iterator with fresh var
- `enumerate` -> Iterator of `(Int, T)` tuple
- `zip` -> Iterator of `(T, U)` tuple
- `fold`/`rfold` -> fresh var
- `collect` -> `List<T>`
- `find`/`last`/`rfind` -> `Option<T>`
- `count` -> INT, `join` -> STR, `any`/`all` -> BOOL, `for_each` -> UNIT

Only 4 arms are truly static: `count` -> INT, `join` -> STR, `any`/`all` -> BOOL, `for_each` -> UNIT. These are handled by the registry. Everything else is in `resolve_computed_return`.

### Tasks

- [x] Phase A: Delete 9 trivially-replaced functions, verify `cargo t -p ori_types` after each (completed in 09.3)
- [x] Phase B: Move computed cases to per-type helpers in `computed_returns.rs`, delete 4 functions (completed in 09.3)
- [x] Phase C: Move computed cases to per-type helpers in `computed_returns.rs`, delete 5 functions (completed in 09.3)
- [x] Phase D: Move iterator computed cases to `computed_iterator_return()`, delete `resolve_iterator_method` (completed in 09.3)
- [x] Verify `resolve_named_type_method` is unchanged and still works (confirmed: only resolver remaining in resolve_by_type.rs)
- [x] Run `cargo st` after all deletions (4169 passed in 09.3)
- [x] Run `./test-all.sh` after all deletions (verified in 09.3)

---

## 09.5 Replace TYPECK_BUILTIN_METHODS

**File:** `compiler/ori_types/src/infer/expr/methods/mod.rs`

### Current State

`TYPECK_BUILTIN_METHODS` is a `const` array of 390 `(&str, &str)` pairs -- `(type_name, method_name)`. It is sorted by type then method name. It is used in:

1. **`compiler/oric/src/eval/tests/methods/consistency.rs`**: Cross-crate consistency tests comparing typeck, eval, IR, and LLVM method lists.
2. **`compiler/ori_types/src/infer/expr/tests.rs`**: Internal tests verifying the array is sorted and complete.

### Migration

**The array is deleted.** The registry becomes the source of truth.

The consistency tests in `consistency.rs` are migrated to iterate `ori_registry::BUILTIN_TYPES` and compare against eval/IR/LLVM **in this section** (not deferred to Section 14). The `TYPECK_BUILTIN_METHODS` array becomes redundant because:

1. The type checker reads from the registry (after 09.3-09.4).
2. The consistency tests compare registry entries against eval/LLVM (Section 14).
3. There is no need for a separate list of "what the type checker knows" -- the registry IS what it knows.

### Sorted-Order Test

The current test verifying `TYPECK_BUILTIN_METHODS` is sorted becomes a registry-level test (Section 14) that verifies `TypeDef.methods` is sorted alphabetically for each type.

### Before/After

```rust
// BEFORE: 390 entries
pub const TYPECK_BUILTIN_METHODS: &[(&str, &str)] = &[
    ("Channel", "close"),
    ("Channel", "is_closed"),
    // ... 388 more entries ...
    ("tuple", "len"),
];

// AFTER: deleted entirely
// Consuming code uses:
//   ori_registry::BUILTIN_TYPES.iter()
//     .flat_map(|td| td.methods.iter().map(|m| (td.name, m.name)))
```

### Public API Change

`TYPECK_BUILTIN_METHODS` is currently re-exported from `ori_types/src/lib.rs` (line 39):

```rust
pub use infer::{
    check_expr, infer_expr, resolve_parsed_type, ExprIndex, InferEngine, TypeEnv,
    TYPECK_BUILTIN_METHODS,
};
```

This export must be removed. The following external consumers reference `TYPECK_BUILTIN_METHODS` and must be migrated:

1. **`compiler/oric/src/eval/tests/methods/consistency.rs`** (lines 8, 625, 642, 665, 678, 692, 715, 734) — the primary consumer. **Migrated to `ori_registry::BUILTIN_TYPES` in THIS section (09.5)**, not deferred to Section 14. Section 14 documents the enforcement tests that build on this migration.2. **`compiler/ori_llvm/src/codegen/arc_emitter/builtins/tests.rs`** (lines 17, 82, 143) — LLVM codegen sync tests. **Migrated to `ori_registry::BUILTIN_TYPES` in THIS section (09.5)**, not deferred to Section 12. Section 12 documents additional LLVM-specific wiring.3. **`compiler/ori_types/src/infer/expr/tests.rs`** (line 2081) — internal consistency test `typeck_builtin_methods_all_resolve`. This test is deleted (replaced by registry-derived tests).
4. **`compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs`** (comments only, lines 15, 108, 160) — update comments to reference `ori_registry::BUILTIN_TYPES`.
5. **`compiler/ori_llvm/src/codegen/type_info/info.rs`** (comments only, lines 234, 240) — update comments.
6. **`compiler/ori_registry/src/defs/str.rs`** (comment only, line 96) — update comment.
7. **`compiler/ori_types/src/registry/methods/mod.rs`** (doc comment, line 6) — update doc comment.

### Tasks

- [x] **Migrate ALL consumers FIRST** (before deleting the constant):  - `compiler/oric/src/eval/tests/methods/consistency.rs` — replaced with `registry_method_pairs()` using `ori_registry::BUILTIN_TYPES` + `legacy_type_name()` bridge
  - `compiler/ori_llvm/src/codegen/arc_emitter/builtins/tests.rs` — replaced with `registry_method_set()` using `ori_registry::BUILTIN_TYPES` + `legacy_type_name()` bridge
  - `compiler/ori_types/src/infer/expr/tests.rs` — deleted `typeck_builtin_methods_all_resolve` test
  - `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs` — updated comments to reference `ori_registry`
  - `compiler/ori_llvm/src/codegen/type_info/info.rs` — updated comments
  - `compiler/ori_registry/src/defs/str.rs` — updated comment
  - `compiler/ori_types/src/registry/methods/mod.rs` — updated doc comment
  - `compiler/oric/src/ir_dump/expr.rs` — updated comment
- [x] Remove `TYPECK_BUILTIN_METHODS` from `methods/mod.rs` (425 lines deleted)
- [x] Remove `TYPECK_BUILTIN_METHODS` from `pub use` in all 3 re-export sites:  - `lib.rs` line 39: removed
  - `infer/mod.rs`: removed
  - `infer/expr/mod.rs`: removed
- [x] **Do NOT remove `DEI_ONLY_METHODS` here** — preserved for 09.7
- [x] Delete sorted-order test from `infer/expr/tests.rs` (deleted with `typeck_builtin_methods_all_resolve`)
- [x] Verify `cargo c -p ori_types` compiles (no remaining references)
- [x] Verify `cargo t -p oric` passes (consistency tests updated)
- [x] Verify `cargo t -p ori_llvm` passes (LLVM sync tests updated)
- [x] Grep entire codebase for `TYPECK_BUILTIN_METHODS` — zero hits in `compiler/`

---

## 09.6 Associated Function Dispatch

**File:** `compiler/ori_types/src/infer/expr/calls/` (associated function call paths)

### Problem

The registry declares some methods as `MethodKind::Associated` (e.g., `Duration.from_seconds()`, `Size.from_bytes()`, `str.from_utf8()`). These are called as `Type.method()`, not `value.method()`. In the current type checker, associated functions for Duration and Size are handled in `resolve_duration_method` and `resolve_size_method` alongside instance methods -- there is no separate dispatch path.

### Decision: No Change Needed

Associated functions that currently resolve through `resolve_*_method()` (Duration factory methods, Size factory methods) will continue to resolve through the registry lookup in `resolve_builtin_method()`. The `MethodKind::Associated` flag on the `MethodDef` is consumed by other phases (LLVM codegen for call convention) but does NOT affect the type checker's resolution logic -- the type checker only needs the return type, which is the same regardless of method kind.

For `str.from_utf8()` and `str.from_utf8_unchecked()`, these are new methods in the registry that the type checker does not currently handle. They are added to `resolve_computed_return()` (see 09.3). The type checker resolves `Type.method()` calls by first resolving the type name to a tag, then calling `resolve_builtin_method()` with that tag -- the same path as instance methods.

### Tasks

- [x] Verify Duration/Size associated functions (factory methods) still resolve after migration (4169 spec tests pass)
- [x] Verify `str.from_utf8()` and `str.from_utf8_unchecked()` resolve through the new path (type-checks OK)
- [ ] Add spec test: `str.from_utf8(bytes: [104, 105])` returns `Result<str, Error>` <!-- blocked: eval doesn't implement str.from_utf8 yet (Section 10) -->
- [x] No code changes needed in this subsection -- behavior falls out of 09.3 changes

---

## 09.7 DEI_ONLY_METHODS Migration

**File:** `compiler/ori_types/src/infer/expr/methods/mod.rs` and `compiler/ori_types/src/infer/expr/calls/method_call.rs`

### Current State

```rust
// compiler/ori_types/src/infer/expr/methods/mod.rs
pub const DEI_ONLY_METHODS: &[&str] = &["last", "next_back", "rev", "rfind", "rfold"];
```

Used in `resolve_receiver_and_builtin()` (calls/method_call.rs, line 335) to emit a diagnostic when a DEI-only method is called on a plain Iterator:

```rust
// compiler/ori_types/src/infer/expr/calls/method_call.rs
if tag == Tag::Iterator {
    if let Some(name_str) = method_str {
        if DEI_ONLY_METHODS.contains(&name_str) {
            engine.push_error(TypeCheckError::unsatisfied_bound(
                span,
                format!(
                    "`{name_str}` requires a DoubleEndedIterator, \
                     but this is an Iterator (use .iter() on a list, range, \
                     or string to get a DoubleEndedIterator)"
                ),
            ));
            return ReceiverDispatch::Return {
                ret_ty: Idx::ERROR,
                receiver_ty: Idx::ERROR,
            };
        }
    }
}
```

### Migration Strategy

After the registry is in place, the DEI-only check reads the `dei_only` flag directly from the single Iterator `TypeDef` (Section 07 Decision 1). No separate `TypeDef` for `DoubleEndedIterator` exists — all iterator methods live on one `TypeDef`, with `dei_only: true` marking DEI-exclusive methods.

```rust
// AFTER: compiler/ori_types/src/infer/expr/calls/method_call.rs

// Replace DEI_ONLY_METHODS.contains(&name_str) with:
//
fn is_dei_only_method(method_name: &str) -> bool {
    ori_registry::is_dei_only(method_name)
}
```

This is structurally derived from the registry data rather than maintained as a parallel constant. Adding a new method with `dei_only: true` to the Iterator TypeDef automatically makes it DEI-only — no manual sync required. The `find_method` call uses `TypeTag::DoubleEndedIterator` to ensure the full method set is searched (plain `TypeTag::Iterator` would filter out DEI-only methods via the `base_type()` + `dei_only` logic in Section 08).

### Calling Site Update

```rust
// AFTER: compiler/ori_types/src/infer/expr/calls/method_call.rs
if tag == Tag::Iterator {
    if let Some(name_str) = method_str {
        if is_dei_only_method(name_str) {
            engine.push_error(TypeCheckError::unsatisfied_bound(
                span,
                format!(
                    "`{name_str}` requires a DoubleEndedIterator, \
                     but this is an Iterator (use .iter() on a list, range, \
                     or string to get a DoubleEndedIterator)"
                ),
            ));
            return ReceiverDispatch::Return {
                ret_ty: Idx::ERROR,
                receiver_ty: Idx::ERROR,
            };
        }
    }
}
```

The calling site is almost unchanged -- just the predicate source changes from array lookup to registry derivation.

### Tasks

- [x] Implement `is_dei_only_method()` — uses `ori_registry::is_dei_only()` directly (no wrapper needed)
- [x] Replace `DEI_ONLY_METHODS.contains(&name_str)` call site in `calls/method_call.rs` with `ori_registry::is_dei_only(name_str)`
- [x] Delete `DEI_ONLY_METHODS` constant from `methods/mod.rs` and its import in `calls/method_call.rs`
- [x] Verify the 5 DEI-only methods are correctly gated — existing tests in `ori_registry/src/defs/iterator/tests.rs` cover all 5 + negative cases
- [x] Unit tests already exist in `ori_registry`: `query_is_dei_only_true_for_dei_methods`, `query_is_dei_only_false_for_non_dei_methods`, `query_is_dei_only_false_for_unknown_methods`
- [x] Uses `ori_registry::is_dei_only()` directly (exported from query API)
- [x] `cargo st tests/spec/traits/iterator/` passes (4169 passed, 0 failed)
- [x] `cargo t -p ori_types` passes (all pass)

---

## 09.8 Well-Known Generic Type Interaction

**File:** `compiler/ori_types/src/check/well_known/mod.rs`

### Current State

`WellKnownNames` pre-interns names for 8 well-known generic types:

```rust
// Well-known generic type names (WellKnownNames fields, line 60-67)
pub option: Name,            // "Option"
pub result: Name,            // "Result"
pub set: Name,               // "Set"
pub channel: Name,           // "Channel"
pub chan: Name,               // "Chan"
pub range: Name,              // "Range"
pub iterator: Name,           // "Iterator"
pub double_ended_iterator: Name, // "DoubleEndedIterator"
```

These are used for:
1. **Type resolution** (`resolve_generic`): mapping parsed type names to Pool constructors.
2. **Trait satisfaction** (`type_satisfies_trait`): bitfield-based O(1) trait checks.
3. **Concrete type detection** (`is_concrete`): whether a name+arity is a well-known type.

### Decision: Registry Complements, Does Not Subsume

`WellKnownNames` and `ori_registry` serve **different purposes**:

| Concern | WellKnownNames | ori_registry |
|---------|---------------|-------------|
| **What it answers** | "Is this *name* a known type?" | "What *methods/operators* does this type have?" |
| **Key operation** | `Name` -> `Idx` (type resolution) | `(TypeTag, method_name)` -> `MethodDef` |
| **When used** | Parse time (type annotations) | Inference time (method calls) |
| **Data** | Interned `Name` handles | Static `&str` method names |
| **Performance model** | O(1) Name comparison | O(n) linear scan or O(1) phf |

The registry does NOT replace `WellKnownNames` because:
1. The registry uses `TypeTag` (an enum), not `Name` (an interned u32). Type resolution from parsed names needs Name comparison.
2. The registry has no concept of trait satisfaction -- it declares methods, not trait conformance.
3. The registry is `const` data with `&str`; `WellKnownNames` uses runtime-interned `Name` handles tied to a specific `StringInterner`.

**The two systems are complementary:**
- `WellKnownNames` answers: "What type does this name refer to?" (parser -> type checker bridge)
- `ori_registry` answers: "What can this type do?" (type checker method resolution)

### Future Optimization Opportunity

The registry's `TypeDef.traits` field (e.g., `&["Eq", "Clone", "Hashable"]`) could eventually subsume the `TraitSet` bitfield in `WellKnownNames`. But this is a Section 14 optimization, not a Section 09 concern.

### Tasks

- [x] Document the WellKnownNames/registry boundary in `registry_bridge.rs` module doc
- [x] Verify no WellKnownNames code is broken by registry integration (all well_known tests pass)
- [x] Verify `resolve_generic` still works (4169 spec tests pass)
- [x] Verify `type_satisfies_trait` still works (well_known tests pass including `compound_type_satisfaction_via_pool`)
- [x] No code changes needed in `well_known/mod.rs` for this section

---

## 09.9 Validation & Regression Testing

### Pre-Migration Baseline

Before any changes, establish a test baseline:

- [x] Run `cargo t -p ori_types` -- 7,717 passed (baseline)
- [x] Run `cargo st` -- 4,169 passed, 42 skipped (baseline)
- [x] Run `./test-all.sh` -- 12,463 passed, 0 failed (baseline)
- [x] Run `cargo t -p oric eval::tests::methods::consistency` -- 16 passed (baseline)

### Per-Subsection Verification

After each subsection is complete, run the following:

| Subsection | Minimum Tests |
|-----------|--------------|
| 09.1 (tag_to_type_tag) | `cargo c -p ori_types` |
| 09.2 (return_tag_to_idx) | `cargo c -p ori_types`, unit tests in registry_bridge |
| 09.3 (replace dispatcher) | `cargo t -p ori_types`, `cargo st` |
| 09.4 (replace all resolvers) | `cargo t -p ori_types`, `cargo st`, `./test-all.sh` |
| 09.5 (replace TYPECK_BUILTIN_METHODS) | `cargo c -p ori_types`, `cargo t -p oric` |
| 09.6 (associated function dispatch) | `cargo t -p ori_types`, `cargo st` (verify Duration/Size factory methods, str.from_utf8) |
| 09.7 (DEI_ONLY_METHODS) | `cargo st tests/spec/traits/iterator/` |
| 09.8 (WellKnownNames) | `cargo t -p ori_types` (no changes expected) |

### Full Regression Gate

After ALL subsections are complete:

- [x] `cargo t -p ori_types` -- 7,722 passed (>= 7,717 baseline, +5 new tests)
- [x] `cargo st` -- 4,169 passed (= baseline)
- [x] `./test-all.sh` -- 12,463 passed, 0 failed (all green)
- [x] `cargo t -p oric eval::tests::methods::consistency` -- 16 passed (= baseline)

### Coverage Verification

For each deleted `resolve_*_method` function, verify every match arm is covered:

| Deleted Function | Method Count | Verification |
|-----------------|-------------|-------------|
| `resolve_int_method` | 22 | All 22 in INT TypeDef |
| `resolve_float_method` | 37 | All 37 in FLOAT TypeDef |
| `resolve_bool_method` | 7 | All 7 in BOOL TypeDef |
| `resolve_byte_method` | 12 | All 12 in BYTE TypeDef |
| `resolve_char_method` | 16 | All 16 in CHAR TypeDef |
| `resolve_ordering_method` | 14 | All 14 in ORDERING TypeDef |
| `resolve_duration_method` | 35 | All 35 in DURATION TypeDef |
| `resolve_size_method` | 24 | All 24 in SIZE TypeDef |
| `resolve_tuple_method` | 5 | All 5 in TUPLE TypeDef |
| `resolve_error_method` | 8 | 7 in ERROR TypeDef + 1 computed |
| `resolve_str_method` | 38 | ~26 in STR TypeDef + 12 computed |
| `resolve_channel_method` | 9 | 5 in CHANNEL TypeDef + 4 computed |
| `resolve_set_method` | 16 | ~14 in SET TypeDef + 2 computed |
| `resolve_list_method` | 57 | ~5 in LIST TypeDef + rest computed |
| `resolve_option_method` | 18 | ~5 in OPTION TypeDef + rest computed |
| `resolve_result_method` | 21 | ~5 in RESULT TypeDef + rest computed |
| `resolve_map_method` | 18 | ~5 in MAP TypeDef + rest computed |
| `resolve_range_method` | 8 | 2 in RANGE TypeDef + rest computed |
| `resolve_iterator_method` | 19 (+5 DEI) | 4 in ITERATOR TypeDef + rest computed |

### New Tests to Write

**File: `registry_bridge/tests.rs`** (sibling test file for the new bridge module):
- [x] `#[test] fn builtin_tags_map_correctly()` + `all_tag_variants_covered()` -- every Tag with builtin methods maps to a TypeTag (20 tags verified)
- [x] `#[test] fn non_builtin_tags_return_none()` -- Named, Applied, Var, Function and 14 others return None
- [x] `#[test] fn concrete_primitives_return_fixed_idx()` -- all 11 primitive TypeTags map to correct Idx constants
- [x] `#[test] fn self_type_returns_receiver()` -- SelfType returns receiver_ty (tested with str and List<int>)
- [x] `#[test] fn option_of_element_on_list()` + 6 more projection tests -- Option/List/Iterator/DEI/NextResult construct correctly in pool

**File: `infer/expr/tests.rs`** (existing test file for method resolution):
- [x] `#[test] fn registry_method_coverage_complete()` -- iterates all `BUILTIN_TYPES`, verifies every instance method resolves via `resolve_builtin_method()` (supersedes old `every_resolved_method_still_resolvable` — the registry is the single source of truth now)
- [x] `#[test] fn str_as_bytes_resolves()` -- `resolve_builtin_method(str, "as_bytes")` returns `[byte]`
- [x] `#[test] fn str_to_bytes_resolves()` -- `resolve_builtin_method(str, "to_bytes")` returns `[byte]`
- [x] `#[test] fn str_from_utf8_resolves()` -- `resolve_builtin_method(str, "from_utf8")` returns `Result<str, fresh>` (associated function still resolves via registry)
- [x] `#[test] fn registry_method_coverage_complete()` -- (same as above, covers this requirement)

**File: `infer/expr/tests.rs`** (DEI gating tests):
- [x] `#[test] fn dei_only_methods_correct()` -- `is_dei_only` returns true for exactly 5 methods (last, next_back, rev, rfind, rfold)

### Grep Verification

After full migration, these identifiers must have zero hits outside of test/doc files:

- [x] `grep -r "resolve_int_method" compiler/ori_types/` -- 0 hits ✓
- [x] `grep -r "resolve_float_method" compiler/ori_types/` -- 0 hits ✓
- [x] `grep -r "resolve_bool_method" compiler/ori_types/` -- 0 hits ✓
- [x] `grep -r "resolve_byte_method" compiler/ori_types/` -- 0 hits ✓
- [x] `grep -r "resolve_char_method" compiler/ori_types/` -- 0 hits ✓
- [x] `grep -r "resolve_ordering_method" compiler/ori_types/` -- 0 hits ✓
- [x] `grep -r "resolve_duration_method" compiler/ori_types/` -- 0 hits ✓
- [x] `grep -r "resolve_size_method" compiler/ori_types/` -- 0 hits ✓
- [x] `grep -r "resolve_tuple_method" compiler/ori_types/` -- 0 hits ✓
- [x] `grep -r "resolve_error_method" compiler/ori_types/` -- 0 hits ✓
- [x] `grep -r "resolve_str_method" compiler/ori_types/` -- 0 hits ✓
- [x] `grep -r "resolve_channel_method" compiler/ori_types/` -- 0 hits ✓
- [x] `grep -r "resolve_set_method" compiler/ori_types/` -- 0 hits ✓
- [x] `grep -r "resolve_list_method" compiler/ori_types/` -- 0 hits ✓
- [x] `grep -r "resolve_option_method" compiler/ori_types/` -- 0 hits ✓
- [x] `grep -r "resolve_result_method" compiler/ori_types/` -- 0 hits ✓
- [x] `grep -r "resolve_map_method" compiler/ori_types/` -- 0 hits ✓
- [x] `grep -r "resolve_range_method" compiler/ori_types/` -- 0 hits ✓
- [x] `grep -r "resolve_iterator_method" compiler/ori_types/` -- 0 hits ✓
- [x] `grep -r "TYPECK_BUILTIN_METHODS"` (entire repo) -- 0 hits ✓
- [x] `grep -r "DEI_ONLY_METHODS"` (entire repo) -- 0 hits ✓
- [x] `grep -r "resolve_by_type"` in `compiler/ori_types/` -- 0 hits (module deleted, function inlined into mod.rs) ✓
- [x] `resolve_builtin_method` is `pub(crate)` in `methods/mod.rs` — not re-exported as standalone ✓

---

## Implementation Tasks (Summary)

### 09.1 tag_to_type_tag() Bridge
- [x] Create `compiler/ori_types/src/infer/expr/registry_bridge/mod.rs`
- [x] Implement `tag_to_type_tag()` with exhaustive match
- [x] Add `mod registry_bridge;` in `infer/expr/mod.rs`
- [x] Unit tests for all Tag variants
- [x] `cargo c -p ori_types` passes

### 09.2 return_tag_to_idx() Bridge
- [x] Implement `return_tag_to_idx()` covering all 22 ReturnTag variants
- [x] Implement `extract_elem()` helper
- [x] Implement `resolve_projection()` helper
- [x] Implement `type_tag_to_idx()` helper
- [x] Unit tests for all ReturnTag variants (22 tests minimum)
- [x] `cargo c -p ori_types` passes

### 09.3 Replace Dispatcher
- [x] New `resolve_builtin_method()` with registry lookup in `methods/mod.rs`
- [x] Create `methods/computed_returns.rs` with thin dispatcher + 2 per-type helpers
- [x] Delete all 19 old resolve_* functions from `resolve_by_type.rs` (only `resolve_named_type_method` remains)
- [x] `cargo t -p ori_types` and `cargo st` pass

### 09.4 Delete resolve_* Functions
- [x] Phase A: 9 trivial deletions (merged into 09.3)
- [x] Phase B: 4 mostly-static deletions (merged into 09.3)
- [x] Phase C: 5 complex deletions (merged into 09.3)
- [x] Phase D: Iterator deletion (merged into 09.3)
- [x] `./test-all.sh` passes

### 09.5 Delete TYPECK_BUILTIN_METHODS
- [x] **Migrate ALL consumers first** (same commit as deletion)
- [x] Remove constant and all 3 pub export sites
- [x] `cargo c -p ori_types`, `cargo t -p oric`, and `cargo t -p ori_llvm` pass

### 09.6 Associated Function Dispatch
- [x] Verify Duration/Size factory methods still resolve (4169 spec tests pass)
- [x] Verify str.from_utf8/from_utf8_unchecked resolve (type-checks OK, eval not implemented)
- [x] No code changes (behavior from 09.3)

### 09.7 DEI_ONLY_METHODS
- [x] Use `ori_registry::is_dei_only()` directly (no wrapper needed)
- [x] Replace calling site in `calls/method_call.rs`
- [x] Delete constant from `methods/mod.rs`
- [x] `cargo st` passes (4169 passed)

### 09.8 WellKnownNames
- [x] Document boundary in `registry_bridge.rs` module doc
- [x] Verify no breakage (all tests pass)

### 09.9 Validation
- [x] Pre-migration baseline (7717/4169/12463/16)
- [x] Per-subsection verification (all 09.1-09.8 verified during implementation)
- [x] Full regression gate (7722/4169/12463/16 — all >= baseline)
- [x] Coverage verification (`registry_method_coverage_complete` test covers all 20 TypeDefs)
- [x] Grep verification (all 23 identifiers confirmed 0 hits)
- [x] New unit tests (5 new tests in infer/expr/tests.rs, 22 existing in registry_bridge/tests.rs)

---

## Exit Criteria

- [x] **All 19 `resolve_*_method` functions deleted** (except `resolve_named_type_method`, inlined into `methods/mod.rs`)
- [x] **`TYPECK_BUILTIN_METHODS` deleted** from `methods/mod.rs`, `infer/expr/mod.rs`, `infer/mod.rs`, and `lib.rs`
- [x] **`DEI_ONLY_METHODS` deleted** from `methods/mod.rs`
- [x] **Single registry lookup** in `resolve_builtin_method()` + `resolve_computed_return()`
- [x] **DEI gating** uses `ori_registry::is_dei_only()` derived from registry, not constant array
- [x] **`WellKnownNames`** unchanged and functional
- [x] **`registry_bridge.rs`** contains `tag_to_type_tag()`, `return_tag_to_idx()`, `extract_elem()`, `resolve_projection()`, `type_tag_to_idx()`, with `#[must_use]` on both pub functions
- [x] **`computed_returns.rs`** contains `resolve_computed_return()` dispatcher + per-type helpers (99 lines total)
- [x] **All consumers of `TYPECK_BUILTIN_METHODS`** migrated in same commit as its deletion
- [x] **`cargo t -p ori_types`** passes (7,722 >= 7,717 baseline)
- [x] **`cargo st`** passes (4,169 = baseline)
- [x] **`./test-all.sh`** passes (12,463 passed, 0 failed)
- [x] **Grep verification** clean: all 23 identifiers confirmed 0 hits
- [x] **File size verification**: all new files under 500 lines:
  - `registry_bridge/mod.rs`: 296 lines
  - `computed_returns.rs`: 99 lines
  - `methods/mod.rs`: 104 lines (dispatcher + inlined resolve_named_type_method)
  - `resolve_by_type.rs`: deleted entirely ✓
- [x] **Net line count** in `methods/` + `registry_bridge/`: mod.rs (104) + computed_returns.rs (99) + registry_bridge/mod.rs (296) = 499 lines total (vs ~916 before = ~417 line reduction)
