---
plan: "type_strategy_registry"
section: "09"
title: "Wire Type Checker (ori_types)"
status: not-started
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
    status: not-started
  - id: "09.2"
    title: "type_tag_to_idx() Bridge (ori_registry::TypeTag -> ori_types::Idx)"
    status: not-started
  - id: "09.3"
    title: "Replace resolve_builtin_method() Dispatcher"
    status: not-started
  - id: "09.4"
    title: "Replace All resolve_*_method Functions"
    status: not-started
  - id: "09.5"
    title: "Replace TYPECK_BUILTIN_METHODS"
    status: not-started
  - id: "09.6"
    title: "TypeFlow Integration (calls.rs)"
    status: not-started
  - id: "09.7"
    title: "DEI_ONLY_METHODS Migration"
    status: not-started
  - id: "09.8"
    title: "Well-Known Generic Type Interaction"
    status: not-started
  - id: "09.9"
    title: "Validation & Regression Testing"
    status: not-started
---

# Section 09: Wire Type Checker (ori_types)

## Overview

This is the most complex wiring section. The type checker (`ori_types`) is the primary consumer of builtin type knowledge in the compiler. It hard-codes method resolution for 20+ types across 18 `resolve_*_method()` functions, maintains a 426-entry `TYPECK_BUILTIN_METHODS` array, hard-codes TypeFlow constraints in `calls.rs`, and uses a 5-entry `DEI_ONLY_METHODS` constant for DoubleEndedIterator gating.

All of this gets replaced by `ori_registry` lookups. The key challenge is that the type checker doesn't just need method *names* -- it needs to construct `Idx` return types in the pool, which requires bridging between the registry's `TypeTag` enum and the pool's `Idx` handles. Two bridge functions (`tag_to_type_tag` and `type_tag_to_idx`) mediate this translation.

**Net effect:** ~800 lines of hard-coded method resolution deleted, replaced by ~80 lines of bridge + lookup code.

## Design Decisions

### Bridge functions live in ori_types, not ori_registry

The `tag_to_type_tag()` and `type_tag_to_idx()` functions live in `ori_types` (likely `infer/expr/methods.rs` or a new `infer/expr/registry_bridge.rs`), not in `ori_registry`. The registry is pure data with zero dependencies. The bridge functions need `ori_types::Tag`, `ori_types::Idx`, `ori_types::Pool`, and `ori_registry::TypeTag` -- they inherently belong to the consuming crate.

### Special logic preserved alongside registry lookups

Some `resolve_*_method` functions contain logic beyond simple return-type mapping:

- **resolve_range_method**: Rejects `iter`/`to_list`/`collect` on `Range<float>`.
- **resolve_iterator_method**: DEI-only gating, DEI propagation for `map`/`filter`, pool construction for `next` (returns `(Option<T>, Iterator<T>)` tuple).
- **resolve_list_method**: Pool construction for `enumerate` (returns `[int, T]`), `zip` (fresh var for other element).
- **resolve_named_type_method**: Newtype `.unwrap()` resolution through TypeRegistry.

These cannot be replaced by a simple `find_method().returns -> type_tag_to_idx()` pipeline. The plan preserves this logic as post-lookup refinements that override or augment the registry's static return type.

### TypeFlow becomes a field on MethodDef, not a separate lookup

The `unify_higher_order_constraints` function in `calls.rs` currently matches on string method names (`"map"`, `"flat_map"`, `"fold"/"rfold"`). After migration, `MethodDef` carries a `type_flow: Option<TypeFlow>` field. The lookup becomes `find_method(tag, name).type_flow` -- no more string matching.

### TYPECK_BUILTIN_METHODS eliminated, not replaced

The 426-entry `TYPECK_BUILTIN_METHODS` array currently serves one purpose: cross-crate consistency tests. After migration, the registry IS the source of truth. The consistency tests iterate `BUILTIN_TYPES` directly. There is no need for a separate array.

### resolve_named_type_method stays outside the registry

The `resolve_named_type_method` function handles *user-defined* types (structs, enums, newtypes), not builtins. It queries the `TypeRegistry` for newtype unwrap. This function is not part of the builtin registry migration and remains as-is.

---

## 09.1 tag_to_type_tag() Bridge (ori_types::Tag -> ori_registry::TypeTag)

**File:** `compiler/ori_types/src/infer/expr/registry_bridge.rs` (new file)

This function maps the type checker's internal `Tag` enum to the registry's `TypeTag` enum, enabling registry lookups from the type checker's resolved type information.

### Current State

There is no bridge function today. The type checker dispatches directly on `Tag` in `resolve_builtin_method()`:

```rust
// BEFORE: compiler/ori_types/src/infer/expr/methods.rs (lines 432-463)
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
        | Tag::Projection | Tag::ModuleNs | Tag::Infer | Tag::SelfType => None,
    }
}
```

### Design Notes

- `Tag::Unit` and `Tag::Never` return `None` because these types have no methods. They are structurally special (unit is `()`, never is bottom) and are never the receiver of a method call.
- `Tag::Named` and `Tag::Applied` return `None` because they represent user-defined types. These go through `resolve_named_type_method` and trait/impl dispatch, which remain unchanged.
- `Tag::Error` maps to `TypeTag::Error` because error has builtin methods (`message`, `to_str`, `trace`, etc.).
- The match is exhaustive -- adding a new `Tag` variant without updating this function is a compile error.

### Tasks

- [ ] Create `compiler/ori_types/src/infer/expr/registry_bridge.rs`
- [ ] Implement `tag_to_type_tag()` with exhaustive match on all `Tag` variants
- [ ] Add `mod registry_bridge;` declaration in `compiler/ori_types/src/infer/expr/mod.rs`
- [ ] Add unit tests: every `Tag` variant with builtin methods maps to the correct `TypeTag`
- [ ] Add unit test: `Tag::Named`, `Tag::Applied`, `Tag::Var`, `Tag::Function` all return `None`
- [ ] Verify `cargo c -p ori_types` compiles

---

## 09.2 type_tag_to_idx() Bridge (ori_registry::TypeTag -> ori_types::Idx)

**File:** `compiler/ori_types/src/infer/expr/registry_bridge.rs` (same file as 09.1)

This is the more complex bridge. It converts the registry's `TypeTag` return types into `Idx` handles in the type pool. Primitive tags are trivial (compile-time constants). Generic/parameterized return types require pool construction.

### The Challenge

A registry `MethodDef` declares `returns: TypeTag::Option` for a method like `list.first()`. But the type checker needs `Idx` for `Option<T>` where `T` is the list's element type. The bridge function must:

1. Map primitive `TypeTag`s to fixed `Idx` constants (e.g., `TypeTag::Int` -> `Idx::INT`).
2. Map `TypeTag::SelfType` to the receiver type (`receiver_ty`).
3. Map parameterized `TypeTag`s (e.g., `TypeTag::Option`) to pool-constructed types using the receiver's inner type(s).
4. Map `TypeTag::FreshVar` to `engine.pool_mut().fresh_var()` for higher-order methods.

### Implementation

```rust
// AFTER: compiler/ori_types/src/infer/expr/registry_bridge.rs

use ori_registry::TypeTag;
use crate::{Idx, Tag};
use super::super::InferEngine;

/// Convert a registry TypeTag to a pool Idx, using the receiver type
/// to resolve parameterized return types.
///
/// `receiver_ty` is the resolved receiver type (e.g., `List<int>`,
/// `Iterator<str>`). Used to extract inner type parameters when the
/// return type is parameterized (e.g., `TypeTag::Option` on a list
/// means `Option<elem>` where `elem` is the list's element type).
pub(crate) fn type_tag_to_idx(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    type_tag: TypeTag,
) -> Idx {
    match type_tag {
        // === Primitives: compile-time constants ===
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

        // === Self type: receiver itself ===
        TypeTag::SelfType => receiver_ty,

        // === Parameterized: construct in pool ===
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
            // Result return type: preserves both Ok and Err types
            let ok_ty = engine.pool().result_ok(receiver_ty);
            let err_ty = engine.pool().result_err(receiver_ty);
            engine.pool_mut().result(ok_ty, err_ty)
        }
        TypeTag::Map => {
            let key_ty = engine.pool().map_key(receiver_ty);
            let value_ty = engine.pool().map_value(receiver_ty);
            engine.pool_mut().map(key_ty, value_ty)
        }

        // === Fresh type variable: higher-order methods ===
        TypeTag::FreshVar => engine.pool_mut().fresh_var(),

        // === Tags that should not appear as return types ===
        TypeTag::Channel | TypeTag::Range | TypeTag::Tuple => {
            // These rarely/never appear as method return types.
            // If they do in the future, add pool construction here.
            Idx::ERROR
        }
    }
}

/// Extract the primary element type from a container receiver.
///
/// For `List<T>`, `Set<T>`, `Option<T>`, `Iterator<T>`, `DEI<T>`,
/// `Channel<T>`, `Range<T>`: returns `T`.
/// For `Map<K, V>`: returns `K` (the primary element; use map-specific
/// extraction for V).
/// For non-containers (primitives, tuples): returns `Idx::ERROR`.
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
        _ => Idx::ERROR,
    }
}
```

### Design Notes

- **`TypeTag::SelfType`** resolves to the receiver type. For `Clone` trait's `clone()` method on `int`, the return is `TypeTag::SelfType` which resolves to `Idx::INT`. For `list.clone()`, it resolves to the `List<T>` type. This is the most common return type for trait methods.
- **`TypeTag::FreshVar`** creates a fresh type variable. This is used for higher-order methods like `map`, `flat_map`, `fold` where the return type depends on the closure argument and cannot be statically determined from the registry alone. The `unify_higher_order_constraints` function (09.6) resolves these variables later.
- **`extract_elem`** is the key helper -- it extracts the "primary inner type" from any container. For most containers this is the element type `T`. For `Map<K, V>` it returns `K`. Methods that need `V` (like `map.values()`) use direct pool accessors.
- **`TypeTag::Channel`/`Range`/`Tuple` as return types**: Currently no method returns these types (a Channel method never returns a new Channel; Range methods return List/Iterator/Int/Bool; Tuple is only used in computed return types like `enumerate`). The `Idx::ERROR` fallback is defensive, not a workaround.

### Tasks

- [ ] Implement `type_tag_to_idx()` in `registry_bridge.rs`
- [ ] Implement `extract_elem()` helper
- [ ] Add unit tests: `TypeTag::Int` -> `Idx::INT`, `TypeTag::Float` -> `Idx::FLOAT`, etc. (all primitives)
- [ ] Add unit test: `TypeTag::SelfType` -> receiver_ty
- [ ] Add unit test: `TypeTag::Option` on `List<int>` receiver -> `Option<int>`
- [ ] Add unit test: `TypeTag::List` on `Set<str>` receiver -> `[str]`
- [ ] Add unit test: `TypeTag::Iterator` on `List<int>` receiver -> `Iterator<int>`
- [ ] Add unit test: `TypeTag::FreshVar` -> fresh var (check it's a `Tag::Var`)
- [ ] Verify `cargo c -p ori_types` compiles

---

## 09.3 Replace resolve_builtin_method() Dispatcher

**File:** `compiler/ori_types/src/infer/expr/methods.rs`

The central dispatcher `resolve_builtin_method()` currently matches on `Tag` and dispatches to 18+ type-specific functions. After migration, it performs a single registry lookup and converts the result.

### Current Implementation (lines 432-463)

See the full listing in 09.1 above. The function is a 30-line match dispatching to 18+ type-specific resolvers.

### New Implementation

```rust
// AFTER: compiler/ori_types/src/infer/expr/methods.rs

use super::registry_bridge::{tag_to_type_tag, type_tag_to_idx};

/// Resolve a built-in method call on a known type tag.
///
/// Performs a single registry lookup via `ori_registry::find_method()`,
/// then converts the return TypeTag to an Idx via `type_tag_to_idx()`.
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
    Some(type_tag_to_idx(engine, receiver_ty, method_def.returns))
}
```

### resolve_computed_return(): Methods Requiring Pool Construction

Some methods have return types that depend on runtime pool state and cannot be expressed as a single static `TypeTag`. These are handled by a dedicated function that returns `Some(Idx)` for computed cases and `None` to fall through to the registry lookup.

```rust
/// Methods whose return types require dynamic pool construction.
///
/// These are methods where the static `TypeTag` in the registry is not
/// sufficient. The registry declares them with `TypeTag::FreshVar` or
/// a simpler approximation, but the type checker needs precise types.
fn resolve_computed_return(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    tag: Tag,
    method: &str,
) -> Option<Idx> {
    match (tag, method) {
        // --- List computed returns ---
        (Tag::List, "enumerate") => {
            let elem = engine.pool().list_elem(receiver_ty);
            let pair = engine.pool_mut().tuple(&[Idx::INT, elem]);
            Some(engine.pool_mut().list(pair))
        }
        (Tag::List, "zip") => {
            let elem = engine.pool().list_elem(receiver_ty);
            let other_elem = engine.pool_mut().fresh_var();
            let pair = engine.pool_mut().tuple(&[elem, other_elem]);
            Some(engine.pool_mut().list(pair))
        }

        // --- Option computed returns ---
        (Tag::Option, "ok_or") => {
            let inner = engine.pool().option_inner(receiver_ty);
            let err_ty = engine.pool_mut().fresh_var();
            Some(engine.pool_mut().result(inner, err_ty))
        }

        // --- Map computed returns ---
        (Tag::Map, "get") => {
            let value_ty = engine.pool().map_value(receiver_ty);
            Some(engine.pool_mut().option(value_ty))
        }
        (Tag::Map, "iter") => {
            let key_ty = engine.pool().map_key(receiver_ty);
            let value_ty = engine.pool().map_value(receiver_ty);
            let pair = engine.pool_mut().tuple(&[key_ty, value_ty]);
            Some(engine.pool_mut().iterator(pair))
        }
        (Tag::Map, "keys") => {
            let key_ty = engine.pool().map_key(receiver_ty);
            Some(engine.pool_mut().list(key_ty))
        }
        (Tag::Map, "values") => {
            let value_ty = engine.pool().map_value(receiver_ty);
            Some(engine.pool_mut().list(value_ty))
        }
        (Tag::Map, "entries") => {
            let key_ty = engine.pool().map_key(receiver_ty);
            let value_ty = engine.pool().map_value(receiver_ty);
            let pair = engine.pool_mut().tuple(&[key_ty, value_ty]);
            Some(engine.pool_mut().list(pair))
        }

        // --- Set computed returns ---
        (Tag::Set, "to_list" | "into") => {
            let elem = engine.pool().set_elem(receiver_ty);
            Some(engine.pool_mut().list(elem))
        }

        // --- Str computed returns ---
        (Tag::Str, "chars") => Some(engine.pool_mut().list(Idx::CHAR)),
        (Tag::Str, "bytes") => Some(engine.pool_mut().list(Idx::BYTE)),
        (Tag::Str, "split" | "lines") => Some(engine.pool_mut().list(Idx::STR)),
        (Tag::Str, "index_of" | "last_index_of" | "to_int" | "parse_int") => {
            Some(engine.pool_mut().option(Idx::INT))
        }
        (Tag::Str, "to_float" | "parse_float") => {
            Some(engine.pool_mut().option(Idx::FLOAT))
        }
        (Tag::Str, "into") => Some(Idx::ERROR),  // str.into() -> Error type

        // --- Channel computed returns ---
        (Tag::Channel, "recv" | "receive" | "try_recv" | "try_receive") => {
            let elem = engine.pool().channel_elem(receiver_ty);
            Some(engine.pool_mut().option(elem))
        }

        // --- Range computed returns ---
        (Tag::Range, "iter" | "to_list" | "collect") => {
            let elem = engine.pool().range_elem(receiver_ty);
            // Range<float> iteration rejection is handled in
            // resolve_receiver_and_builtin() via check_range_float_iteration()
            match method {
                "iter" => Some(engine.pool_mut().double_ended_iterator(elem)),
                "to_list" | "collect" => Some(engine.pool_mut().list(elem)),
                _ => unreachable!(),
            }
        }

        // --- Iterator/DEI computed returns ---
        (Tag::Iterator | Tag::DoubleEndedIterator, "next") => {
            let elem = engine.pool().iterator_elem(receiver_ty);
            let option_elem = engine.pool_mut().option(elem);
            Some(engine.pool_mut().tuple(&[option_elem, receiver_ty]))
        }
        (Tag::DoubleEndedIterator, "next_back") => {
            let elem = engine.pool().iterator_elem(receiver_ty);
            let option_elem = engine.pool_mut().option(elem);
            Some(engine.pool_mut().tuple(&[option_elem, receiver_ty]))
        }
        (Tag::Iterator | Tag::DoubleEndedIterator, "map") => {
            let new_elem = engine.pool_mut().fresh_var();
            let is_dei = tag == Tag::DoubleEndedIterator;
            if is_dei {
                Some(engine.pool_mut().double_ended_iterator(new_elem))
            } else {
                Some(engine.pool_mut().iterator(new_elem))
            }
        }
        (Tag::Iterator | Tag::DoubleEndedIterator, "filter") => {
            // filter preserves iterator kind (DEI stays DEI)
            Some(receiver_ty)
        }
        (Tag::Iterator | Tag::DoubleEndedIterator, "take" | "skip" | "chain" | "cycle") => {
            let elem = engine.pool().iterator_elem(receiver_ty);
            Some(engine.pool_mut().iterator(elem))
        }
        (Tag::Iterator | Tag::DoubleEndedIterator, "flatten" | "flat_map") => {
            let new_elem = engine.pool_mut().fresh_var();
            Some(engine.pool_mut().iterator(new_elem))
        }
        (Tag::Iterator | Tag::DoubleEndedIterator, "enumerate") => {
            let elem = engine.pool().iterator_elem(receiver_ty);
            let pair = engine.pool_mut().tuple(&[Idx::INT, elem]);
            Some(engine.pool_mut().iterator(pair))
        }
        (Tag::Iterator | Tag::DoubleEndedIterator, "zip") => {
            let elem = engine.pool().iterator_elem(receiver_ty);
            let other_elem = engine.pool_mut().fresh_var();
            let pair = engine.pool_mut().tuple(&[elem, other_elem]);
            Some(engine.pool_mut().iterator(pair))
        }
        (Tag::Iterator | Tag::DoubleEndedIterator, "collect") => {
            let elem = engine.pool().iterator_elem(receiver_ty);
            Some(engine.pool_mut().list(elem))
        }
        (Tag::Iterator | Tag::DoubleEndedIterator, "find") => {
            let elem = engine.pool().iterator_elem(receiver_ty);
            Some(engine.pool_mut().option(elem))
        }
        (Tag::DoubleEndedIterator, "last" | "rfind") => {
            let elem = engine.pool().iterator_elem(receiver_ty);
            Some(engine.pool_mut().option(elem))
        }
        (Tag::DoubleEndedIterator, "rev") => Some(receiver_ty),

        // --- Error computed returns ---
        (Tag::Error, "trace_entries") => Some(engine.pool_mut().fresh_var()),

        _ => None,
    }
}
```

### Inventory of All resolve_* Functions and Their Fate

| Function | Lines | Simple Lookup? | Computed Returns | Migration Path |
|----------|-------|---------------|-----------------|----------------|
| `resolve_int_method` | 613-625 | YES | None | Fully replaced by registry lookup |
| `resolve_float_method` | 627-639 | YES | None | Fully replaced by registry lookup |
| `resolve_bool_method` | 761-769 | YES | None | Fully replaced by registry lookup |
| `resolve_byte_method` | 771-783 | YES | None | Fully replaced by registry lookup |
| `resolve_char_method` | 785-795 | YES | None | Fully replaced by registry lookup |
| `resolve_ordering_method` | 736-749 | YES | None | Fully replaced by registry lookup |
| `resolve_duration_method` | 641-656 | YES | None | Fully replaced by registry lookup |
| `resolve_size_method` | 658-672 | YES | None | Fully replaced by registry lookup |
| `resolve_tuple_method` | 868-877 | YES | None | Fully replaced by registry lookup |
| `resolve_error_method` | 751-759 | MOSTLY | `trace_entries` -> fresh_var | 1 computed case, rest via registry |
| `resolve_str_method` | 589-611 | MOSTLY | `chars`, `bytes`, `split`, `lines`, `index_of`, `last_index_of`, `to_int`, `parse_int`, `to_float`, `parse_float`, `into` | 11 computed cases |
| `resolve_channel_method` | 674-687 | MOSTLY | `recv`/`receive`/`try_recv`/`try_receive` -> `Option<T>` | 4 computed cases |
| `resolve_list_method` | 465-500 | NO | `first`/`last`/`pop`/`get`, `iter`, `enumerate`, `zip`, + 20 HO methods | Complex, many computed |
| `resolve_option_method` | 502-525 | NO | `ok_or` -> Result, `iter`, HO methods | Several computed |
| `resolve_result_method` | 527-549 | NO | `ok`, `err`, HO methods | Several computed |
| `resolve_map_method` | 551-572 | NO | `get`, `iter`, `keys`, `values`, `entries` | 5 computed cases |
| `resolve_set_method` | 574-587 | MOSTLY | `to_list`/`into` -> List<T> | 2 computed cases |
| `resolve_range_method` | 689-707 | NO | `iter`, `to_list`/`collect` + float rejection | 3 computed + special logic |
| `resolve_iterator_method` | 804-866 | NO | `next`/`next_back`, `map`, `filter`, adapters, consumers | Nearly all computed |
| `resolve_named_type_method` | 712-733 | N/A | Newtype unwrap | NOT migrated (user-defined types) |

**Summary:**
- **9 functions** (int, float, bool, byte, char, ordering, duration, size, tuple) are **trivially replaced** -- every match arm is a static type tag.
- **4 functions** (error, str, channel, set) are **mostly replaced** -- a few arms need computed returns, rest are static.
- **5 functions** (list, option, result, map, range) have **significant computed returns** -- many arms need pool construction.
- **1 function** (iterator) is **almost entirely computed** -- the registry provides existence checking but nearly every return type requires pool work.
- **1 function** (named_type) is **not migrated** -- handles user-defined types.

### Tasks

- [ ] Implement new `resolve_builtin_method()` dispatcher with registry lookup
- [ ] Implement `resolve_computed_return()` for all computed cases
- [ ] Verify computed returns match current behavior exactly (use existing tests)
- [ ] Delete the 9 trivially-replaced functions after tests pass
- [ ] Delete the remaining 9 type-specific functions after computed returns are verified
- [ ] Keep `resolve_named_type_method()` unchanged
- [ ] Verify `cargo t -p ori_types` passes
- [ ] Verify `cargo st` passes

---

## 09.4 Replace All resolve_*_method Functions

This subsection is the detailed execution plan for deleting each of the 18 type-specific resolver functions. Each deletion is verified independently.

### Phase A: Trivial Replacements (9 functions)

These functions contain ONLY static return type mappings. Every match arm maps to a fixed `Idx` constant. The registry lookup + `type_tag_to_idx()` handles them completely.

**1. `resolve_int_method` (lines 613-625)**
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

**2. `resolve_float_method` (lines 627-639)**
```rust
// DELETE: Every arm maps to a constant
"abs"|"sqrt"|...|"clone" => Idx::FLOAT
"floor"|"ceil"|"round"|"trunc"|"to_int"|"hash" => Idx::INT
"to_str"|"debug" => Idx::STR
"is_nan"|...|"equals" => Idx::BOOL
"compare" => Idx::ORDERING
```
**Registry coverage:** FLOAT TypeDef (Section 03.2) covers all 37 methods.

**3. `resolve_bool_method` (lines 761-769)**
```rust
// DELETE: Every arm maps to a constant
"to_str"|"debug" => Idx::STR
"to_int"|"hash" => Idx::INT
"clone"|"equals" => Idx::BOOL
"compare" => Idx::ORDERING
```
**Registry coverage:** BOOL TypeDef (Section 03.3) covers all 7 methods.

**4. `resolve_byte_method` (lines 771-783)**
```rust
// DELETE: Every arm maps to a constant
"to_int"|"hash" => Idx::INT
"to_char" => Idx::CHAR
"to_str"|"debug" => Idx::STR
"is_ascii"|...|"equals" => Idx::BOOL
"clone" => Idx::BYTE
"compare" => Idx::ORDERING
```
**Registry coverage:** BYTE TypeDef (Section 03.4) covers all 11 methods.

**5. `resolve_char_method` (lines 785-795)**
```rust
// DELETE: Every arm maps to a constant
"to_str"|"debug" => Idx::STR
"to_int"|"to_byte"|"hash" => Idx::INT
"is_digit"|...|"equals" => Idx::BOOL
"to_uppercase"|"to_lowercase"|"clone" => Idx::CHAR
"compare" => Idx::ORDERING
```
**Registry coverage:** CHAR TypeDef (Section 03.5) covers all 16 methods.

**6. `resolve_ordering_method` (lines 736-749)**
```rust
// DELETE: Every arm maps to a constant
"is_less"|...|"equals" => Idx::BOOL
"reverse"|"clone"|"compare"|"then"|"then_with" => Idx::ORDERING
"hash" => Idx::INT
"to_str"|"debug" => Idx::STR
```
**Registry coverage:** ORDERING TypeDef (Section 05) covers all 12 methods.

**7. `resolve_duration_method` (lines 641-656)**
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

**8. `resolve_size_method` (lines 658-672)**
```rust
// DELETE: Every arm maps to a constant
"to_bytes"|"as_bytes"|"to_kb"|...|"hash" => Idx::INT
"to_str"|"format"|"debug" => Idx::STR
"is_zero"|"equals" => Idx::BOOL
"from_bytes"|...|"zero"|"clone" => Idx::SIZE
"compare" => Idx::ORDERING
```
**Registry coverage:** SIZE TypeDef (Section 05) covers all methods.

**9. `resolve_tuple_method` (lines 868-877)**
```rust
// DELETE: Every arm maps to a constant or receiver_ty
"len"|"hash" => Idx::INT
"compare" => Idx::ORDERING
"equals" => Idx::BOOL
"clone" => receiver_ty  // TypeTag::SelfType handles this
"debug" => Idx::STR
```
**Registry coverage:** TUPLE TypeDef (Section 06) covers all 5 methods.

### Phase B: Mostly-Static Replacements (4 functions)

These functions have a few computed cases that go into `resolve_computed_return()`, with the remaining arms handled by the registry.

**10. `resolve_error_method` (lines 751-759)**
- Static: `message`/`to_str`/`debug`/`trace` -> STR, `has_trace` -> BOOL, `clone`/`with_trace` -> ERROR
- Computed: `trace_entries` -> `fresh_var()` (handled in `resolve_computed_return`)
- **7 static, 1 computed**

**11. `resolve_str_method` (lines 589-611)**
- Static: `len`/`byte_len`/`hash`/`length` -> INT, `is_empty`/.../`equals` -> BOOL, `to_uppercase`/.../`to_str` -> STR, `compare` -> ORDERING
- Computed: `chars` -> `List<Char>`, `bytes` -> `List<Byte>`, `split`/`lines` -> `List<Str>`, `index_of`/`last_index_of`/`to_int`/`parse_int` -> `Option<Int>`, `to_float`/`parse_float` -> `Option<Float>`, `into` -> `Idx::ERROR`, `iter` -> `DEI<Char>`
- **~25 static, 12 computed** (note: `iter` returns `DEI<Char>` which can be registry-expressed as `TypeTag::DoubleEndedIterator` with Char element, but str is not a generic container -- we handle it as computed)

**12. `resolve_channel_method` (lines 674-687)**
- Static: `send`/`close` -> UNIT, `is_closed`/`is_empty` -> BOOL, `len` -> INT
- Computed: `recv`/`receive`/`try_recv`/`try_receive` -> `Option<T>`
- **5 static, 4 computed**

**13. `resolve_set_method` (lines 574-587)**
- Static: `len`/`hash` -> INT, `is_empty`/`contains`/`equals` -> BOOL, `insert`/.../`clone` -> SelfType, `iter` -> Iterator<T>, `debug` -> STR
- Computed: `to_list`/`into` -> `List<T>`
- **~10 static (iter handled by registry as TypeTag::Iterator), 2 computed**

### Phase C: Complex Replacements (5 functions)

These functions have many computed returns and require extensive `resolve_computed_return` coverage.

**14. `resolve_list_method` (lines 465-500)** -- 22+ computed arms
**15. `resolve_option_method` (lines 502-525)** -- 6+ computed arms
**16. `resolve_result_method` (lines 527-549)** -- 6+ computed arms
**17. `resolve_map_method` (lines 551-572)** -- 5 computed arms
**18. `resolve_range_method` (lines 689-707)** -- 3 computed + float rejection logic

### Phase D: Iterator (special case)

**19. `resolve_iterator_method` (lines 804-866)** -- Nearly entirely computed.

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

- [ ] Phase A: Delete 9 trivially-replaced functions, verify `cargo t -p ori_types` after each
- [ ] Phase B: Move computed cases to `resolve_computed_return`, delete 4 functions
- [ ] Phase C: Move computed cases to `resolve_computed_return`, delete 5 functions
- [ ] Phase D: Move iterator computed cases, delete `resolve_iterator_method`
- [ ] Verify `resolve_named_type_method` is unchanged and still works
- [ ] Run `cargo st` after all deletions
- [ ] Run `./test-all.sh` after all deletions

---

## 09.5 Replace TYPECK_BUILTIN_METHODS

**File:** `compiler/ori_types/src/infer/expr/methods.rs` (lines 13-426)

### Current State

`TYPECK_BUILTIN_METHODS` is a `const` array of 426 `(&str, &str)` pairs -- `(type_name, method_name)`. It is sorted by type then method name. It is used in:

1. **`compiler/oric/src/eval/tests/methods/consistency.rs`**: Cross-crate consistency tests comparing typeck, eval, IR, and LLVM method lists.
2. **`compiler/ori_types/src/infer/expr/tests.rs`**: Internal tests verifying the array is sorted and complete.

### Migration

**The array is deleted.** The registry becomes the source of truth.

The consistency tests in `consistency.rs` are migrated to iterate `ori_registry::BUILTIN_TYPES` and compare against eval/IR/LLVM. The `TYPECK_BUILTIN_METHODS` array becomes redundant because:

1. The type checker reads from the registry (after 09.3-09.4).
2. The consistency tests compare registry entries against eval/LLVM (Section 14).
3. There is no need for a separate list of "what the type checker knows" -- the registry IS what it knows.

### Sorted-Order Test

The current test verifying `TYPECK_BUILTIN_METHODS` is sorted becomes a registry-level test (Section 14) that verifies `TypeDef.methods` is sorted alphabetically for each type.

### Before/After

```rust
// BEFORE: 426 lines
pub const TYPECK_BUILTIN_METHODS: &[(&str, &str)] = &[
    ("Channel", "close"),
    ("Channel", "is_closed"),
    // ... 424 more entries ...
    ("tuple", "len"),
];

// AFTER: deleted entirely
// Consuming code uses:
//   ori_registry::BUILTIN_TYPES.iter()
//     .flat_map(|td| td.methods.iter().map(|m| (td.name, m.name)))
```

### Public API Change

`TYPECK_BUILTIN_METHODS` is currently re-exported from `ori_types/src/lib.rs` (line 38):

```rust
pub use infer::{
    check_expr, infer_expr, resolve_parsed_type, ExprIndex, InferEngine, TypeEnv,
    TYPECK_BUILTIN_METHODS,
};
```

This export must be removed. Any external code referencing it must be migrated to use `ori_registry` directly.

### Tasks

- [ ] Remove `TYPECK_BUILTIN_METHODS` from `methods.rs`
- [ ] Remove `TYPECK_BUILTIN_METHODS` from `pub use` in `lib.rs`
- [ ] Remove `DEI_ONLY_METHODS` from `methods.rs` (handled separately in 09.7)
- [ ] Migrate consistency tests in `consistency.rs` to use `ori_registry::BUILTIN_TYPES`
- [ ] Delete sorted-order test from `infer/expr/tests.rs`
- [ ] Verify `cargo c -p ori_types` compiles (no remaining references)
- [ ] Verify `cargo t -p oric` passes (consistency tests updated)
- [ ] Grep entire codebase for `TYPECK_BUILTIN_METHODS` -- zero hits

---

## 09.6 TypeFlow Integration (calls.rs)

**File:** `compiler/ori_types/src/infer/expr/calls.rs` (lines 700-764)

### Current State

The `unify_higher_order_constraints` function hard-codes method name strings to determine how closure arguments constrain the return type:

```rust
// BEFORE: compiler/ori_types/src/infer/expr/calls.rs (lines 700-764)
fn unify_higher_order_constraints(
    engine: &mut InferEngine<'_>,
    method: Name,
    ret_ty: Idx,
    arg_types: &[Idx],
) {
    let Some(method_str) = engine.lookup_name(method) else {
        return;
    };

    match method_str {
        "map" => {
            // Closure (T) -> U. Unify iterator elem var with U.
            ...
        }
        "flat_map" => {
            // Closure (T) -> Iterator<U>. Unify elem var with U.
            ...
        }
        "fold" | "rfold" => {
            // Unify ret_ty with initial value and closure return.
            ...
        }
        _ => {}
    }
}
```

This function matches on 4 string literals. Adding a new higher-order method requires updating this function manually.

### New State

The `MethodDef` in `ori_registry` gains a `type_flow: Option<TypeFlow>` field. The `TypeFlow` enum encodes how closure arguments constrain the return type:

```rust
// In ori_registry (Section 01/07)
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypeFlow {
    /// Closure return becomes the new element type.
    /// Used by: map (on Iterator, List, Option, Result)
    ClosureOutputBecomesElement,

    /// Closure returns Iterator<U>; U becomes the new element type.
    /// Used by: flat_map
    ClosureOutputFlatElement,

    /// Return type = initial accumulator = closure return.
    /// Used by: fold, rfold, reduce
    Accumulator,
}
```

### New Implementation

```rust
// AFTER: compiler/ori_types/src/infer/expr/calls.rs

use ori_registry::TypeFlow;
use super::registry_bridge::tag_to_type_tag;

fn unify_higher_order_constraints(
    engine: &mut InferEngine<'_>,
    method: Name,
    ret_ty: Idx,
    arg_types: &[Idx],
) {
    // Look up the method's TypeFlow from the registry
    let method_str = match engine.lookup_name(method) {
        Some(s) => s,
        None => return,
    };

    // Determine the receiver's TypeTag for registry lookup.
    // ret_ty is the return type from resolve_builtin_method, which
    // was already resolved. We need the receiver's tag. Since the
    // receiver was already resolved before this point, we check if
    // ret_ty's tag is an iterator (common case for iterator methods)
    // or fall back to trying all relevant types.
    let resolved_ret = engine.resolve(ret_ty);
    let ret_tag = engine.pool().tag(resolved_ret);

    // Try to find the method in the registry with TypeFlow
    let type_flow = find_type_flow(ret_tag, method_str);
    let Some(flow) = type_flow else {
        return;
    };

    match flow {
        TypeFlow::ClosureOutputBecomesElement => {
            let Some(&closure_ty) = arg_types.first() else {
                return;
            };
            if !ret_tag.is_iterator() {
                return;
            }
            let elem_var = engine.pool().iterator_elem(resolved_ret);
            let resolved_closure = engine.resolve(closure_ty);
            if engine.pool().tag(resolved_closure) == Tag::Function {
                let closure_ret = engine.pool().function_return(resolved_closure);
                let _ = engine.unify().unify(elem_var, closure_ret);
            }
        }
        TypeFlow::ClosureOutputFlatElement => {
            let Some(&closure_ty) = arg_types.first() else {
                return;
            };
            if !ret_tag.is_iterator() {
                return;
            }
            let elem_var = engine.pool().iterator_elem(resolved_ret);
            let resolved_closure = engine.resolve(closure_ty);
            if engine.pool().tag(resolved_closure) == Tag::Function {
                let closure_ret = engine.pool().function_return(resolved_closure);
                let resolved_inner = engine.resolve(closure_ret);
                if engine.pool().tag(resolved_inner).is_iterator() {
                    let inner_elem = engine.pool().iterator_elem(resolved_inner);
                    let _ = engine.unify().unify(elem_var, inner_elem);
                }
            }
        }
        TypeFlow::Accumulator => {
            if let Some(&init_ty) = arg_types.first() {
                let _ = engine.unify().unify(ret_ty, init_ty);
            }
            if let Some(&closure_ty) = arg_types.get(1) {
                let resolved_closure = engine.resolve(closure_ty);
                if engine.pool().tag(resolved_closure) == Tag::Function {
                    let closure_ret = engine.pool().function_return(resolved_closure);
                    let _ = engine.unify().unify(ret_ty, closure_ret);
                }
            }
        }
    }
}

/// Look up a method's TypeFlow from the registry.
///
/// Tries the given tag first. If the tag is an iterator, looks up
/// both Iterator and DoubleEndedIterator definitions.
fn find_type_flow(ret_tag: Tag, method_str: &str) -> Option<TypeFlow> {
    // For iterator return types, the method was called on an iterator
    let candidates = match ret_tag {
        Tag::Iterator => &[TypeTag::Iterator, TypeTag::DoubleEndedIterator][..],
        Tag::DoubleEndedIterator => &[TypeTag::DoubleEndedIterator, TypeTag::Iterator][..],
        _ => {
            // For non-iterator return types (e.g., fold returns FreshVar),
            // try common higher-order method hosts
            return try_type_flow_for_accumulator(method_str);
        }
    };

    for &type_tag in candidates {
        if let Some(method_def) = ori_registry::find_method(type_tag, method_str) {
            if let Some(flow) = method_def.type_flow {
                return Some(flow);
            }
        }
    }
    None
}

/// Check if a method name is a known accumulator method.
///
/// Accumulator methods (fold, rfold, reduce) have fresh-var return types
/// that don't carry the receiver's tag. We need to recognize them by name
/// and check the registry.
fn try_type_flow_for_accumulator(method_str: &str) -> Option<TypeFlow> {
    // fold/rfold/reduce can appear on Iterator, DEI, or List
    for &type_tag in &[TypeTag::Iterator, TypeTag::DoubleEndedIterator, TypeTag::List] {
        if let Some(method_def) = ori_registry::find_method(type_tag, method_str) {
            if let Some(flow) = method_def.type_flow {
                return Some(flow);
            }
        }
    }
    None
}
```

### Methods with TypeFlow

| Type | Method | TypeFlow | Current String Match |
|------|--------|----------|---------------------|
| Iterator | `map` | `ClosureOutputBecomesElement` | `"map"` |
| Iterator | `flat_map` | `ClosureOutputFlatElement` | `"flat_map"` |
| Iterator | `fold` | `Accumulator` | `"fold"` |
| DoubleEndedIterator | `map` | `ClosureOutputBecomesElement` | `"map"` |
| DoubleEndedIterator | `flat_map` | `ClosureOutputFlatElement` | `"flat_map"` |
| DoubleEndedIterator | `fold` | `Accumulator` | `"fold"` |
| DoubleEndedIterator | `rfold` | `Accumulator` | `"rfold"` |
| List | `map` | `ClosureOutputBecomesElement` | N/A (fresh_var today) |
| List | `flat_map` | `ClosureOutputFlatElement` | N/A |
| List | `fold` | `Accumulator` | N/A |
| List | `reduce` | `Accumulator` | N/A |
| Option | `map` | `ClosureOutputBecomesElement` | N/A |
| Option | `and_then` | `ClosureOutputFlatElement` | N/A |
| Result | `map` | `ClosureOutputBecomesElement` | N/A |
| Result | `and_then` | `ClosureOutputFlatElement` | N/A |

Note: List/Option/Result HO methods currently return `fresh_var()` without TypeFlow unification. The registry provides an opportunity to add proper TypeFlow for these in the future, but this section focuses on preserving current behavior (Iterator/DEI only).

### Tasks

- [ ] Add `type_flow: Option<TypeFlow>` field to `MethodDef` in `ori_registry` (Section 01 dependency)
- [ ] Set `TypeFlow::ClosureOutputBecomesElement` on Iterator/DEI `map`
- [ ] Set `TypeFlow::ClosureOutputFlatElement` on Iterator/DEI `flat_map`
- [ ] Set `TypeFlow::Accumulator` on Iterator/DEI `fold` and DEI `rfold`
- [ ] Implement new `unify_higher_order_constraints` using registry TypeFlow
- [ ] Implement `find_type_flow()` and `try_type_flow_for_accumulator()` helpers
- [ ] Delete old string-matching implementation
- [ ] Verify `cargo st` passes (closure type inference must still work)
- [ ] Verify `cargo t -p ori_types` passes

---

## 09.7 DEI_ONLY_METHODS Migration

**File:** `compiler/ori_types/src/infer/expr/methods.rs` (line 11) and `compiler/ori_types/src/infer/expr/calls.rs` (lines 816-831)

### Current State

```rust
// compiler/ori_types/src/infer/expr/methods.rs line 11
pub const DEI_ONLY_METHODS: &[&str] = &["last", "next_back", "rev", "rfind", "rfold"];
```

Used in `resolve_receiver_and_builtin()` (calls.rs line 819) to emit a diagnostic when a DEI-only method is called on a plain Iterator:

```rust
// compiler/ori_types/src/infer/expr/calls.rs lines 816-831
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
            return ReceiverDispatch::Return(Idx::ERROR);
        }
    }
}
```

### Migration Strategy

After the registry is in place, the DEI-only check becomes a registry comparison: "method exists on DoubleEndedIterator but not on Iterator."

```rust
// AFTER: compiler/ori_types/src/infer/expr/calls.rs

// Replace DEI_ONLY_METHODS.contains(&name_str) with:
fn is_dei_only_method(method_name: &str) -> bool {
    ori_registry::find_method(TypeTag::DoubleEndedIterator, method_name).is_some()
        && ori_registry::find_method(TypeTag::Iterator, method_name).is_none()
}
```

This is structurally derived from the registry data rather than maintained as a parallel constant. Adding a new DEI-only method to the DoubleEndedIterator TypeDef automatically makes it a DEI-only method -- no manual sync required.

### Calling Site Update

```rust
// AFTER: compiler/ori_types/src/infer/expr/calls.rs
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
            return ReceiverDispatch::Return(Idx::ERROR);
        }
    }
}
```

The calling site is almost unchanged -- just the predicate source changes from array lookup to registry derivation.

### Tasks

- [ ] Implement `is_dei_only_method()` using registry lookups
- [ ] Replace `DEI_ONLY_METHODS.contains(&name_str)` call site in `calls.rs`
- [ ] Delete `DEI_ONLY_METHODS` constant from `methods.rs`
- [ ] Verify the 5 current DEI-only methods (`last`, `next_back`, `rev`, `rfind`, `rfold`) are on the DoubleEndedIterator TypeDef but NOT on the Iterator TypeDef in the registry
- [ ] Verify `cargo st tests/spec/traits/iterator/` passes (DEI rejection diagnostics unchanged)
- [ ] Verify `cargo t -p ori_types` passes

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

- [ ] Document the WellKnownNames/registry boundary in `registry_bridge.rs` module doc
- [ ] Verify no WellKnownNames code is broken by registry integration
- [ ] Verify `resolve_generic` still works (it uses Pool constructors, not registry)
- [ ] Verify `type_satisfies_trait` still works (it uses TraitSet, not registry)
- [ ] No code changes needed in `well_known/mod.rs` for this section

---

## 09.9 Validation & Regression Testing

### Pre-Migration Baseline

Before any changes, establish a test baseline:

- [ ] Run `cargo t -p ori_types` -- record pass count
- [ ] Run `cargo st` -- record pass count
- [ ] Run `./test-all.sh` -- record pass count
- [ ] Run `cargo t -p oric -- consistency` -- record pass count

### Per-Subsection Verification

After each subsection is complete, run the following:

| Subsection | Minimum Tests |
|-----------|--------------|
| 09.1 (tag_to_type_tag) | `cargo c -p ori_types` |
| 09.2 (type_tag_to_idx) | `cargo c -p ori_types`, unit tests in registry_bridge |
| 09.3 (replace dispatcher) | `cargo t -p ori_types`, `cargo st` |
| 09.4 (replace all resolvers) | `cargo t -p ori_types`, `cargo st`, `./test-all.sh` |
| 09.5 (replace TYPECK_BUILTIN_METHODS) | `cargo c -p ori_types`, `cargo t -p oric` |
| 09.6 (TypeFlow) | `cargo st tests/spec/traits/iterator/`, `cargo st` |
| 09.7 (DEI_ONLY_METHODS) | `cargo st tests/spec/traits/iterator/` |
| 09.8 (WellKnownNames) | `cargo t -p ori_types` (no changes expected) |

### Full Regression Gate

After ALL subsections are complete:

- [ ] `cargo t -p ori_types` -- pass count >= baseline
- [ ] `cargo st` -- pass count >= baseline
- [ ] `./test-all.sh` -- all green
- [ ] `cargo t -p oric -- consistency` -- passes with updated tests

### Coverage Verification

For each deleted `resolve_*_method` function, verify every match arm is covered:

| Deleted Function | Method Count | Verification |
|-----------------|-------------|-------------|
| `resolve_int_method` | 22 | All 22 in INT TypeDef |
| `resolve_float_method` | 37 | All 37 in FLOAT TypeDef |
| `resolve_bool_method` | 7 | All 7 in BOOL TypeDef |
| `resolve_byte_method` | 11 | All 11 in BYTE TypeDef |
| `resolve_char_method` | 16 | All 16 in CHAR TypeDef |
| `resolve_ordering_method` | 12 | All 12 in ORDERING TypeDef |
| `resolve_duration_method` | ~30 | All in DURATION TypeDef |
| `resolve_size_method` | ~20 | All in SIZE TypeDef |
| `resolve_tuple_method` | 5 | All 5 in TUPLE TypeDef |
| `resolve_error_method` | 8 | 7 in ERROR TypeDef + 1 computed |
| `resolve_str_method` | ~35 | ~23 in STR TypeDef + 12 computed |
| `resolve_channel_method` | 9 | 5 in CHANNEL TypeDef + 4 computed |
| `resolve_set_method` | ~13 | ~11 in SET TypeDef + 2 computed |
| `resolve_list_method` | ~35 | ~5 in LIST TypeDef + rest computed |
| `resolve_option_method` | ~13 | ~5 in OPTION TypeDef + rest computed |
| `resolve_result_method` | ~15 | ~5 in RESULT TypeDef + rest computed |
| `resolve_map_method` | ~15 | ~5 in MAP TypeDef + rest computed |
| `resolve_range_method` | 8 | 2 in RANGE TypeDef + rest computed |
| `resolve_iterator_method` | ~25 | 4 in ITERATOR TypeDef + rest computed |

### New Tests to Write

- [ ] `#[test] fn registry_bridge_all_builtin_tags()` -- every Tag with builtin methods maps to a TypeTag
- [ ] `#[test] fn registry_bridge_non_builtin_tags()` -- Named, Applied, Var, Function return None
- [ ] `#[test] fn type_tag_to_idx_primitives()` -- all primitive TypeTags map to correct Idx constants
- [ ] `#[test] fn type_tag_to_idx_self_type()` -- SelfType returns receiver_ty
- [ ] `#[test] fn type_tag_to_idx_parameterized()` -- Option/List/Iterator construct correctly in pool
- [ ] `#[test] fn dei_only_methods_derived()` -- `is_dei_only_method` returns true for exactly the 5 current methods
- [ ] `#[test] fn type_flow_from_registry()` -- `find_type_flow` returns correct TypeFlow for map/flat_map/fold/rfold
- [ ] `#[test] fn every_resolved_method_still_resolvable()` -- iterate all entries from the old `TYPECK_BUILTIN_METHODS` (captured as a test constant), verify each resolves via the new path

### Grep Verification

After full migration, these identifiers must have zero hits outside of test/doc files:

- [ ] `grep -r "resolve_int_method" compiler/ori_types/` -- 0 hits
- [ ] `grep -r "resolve_float_method" compiler/ori_types/` -- 0 hits
- [ ] `grep -r "resolve_bool_method" compiler/ori_types/` -- 0 hits
- [ ] `grep -r "resolve_byte_method" compiler/ori_types/` -- 0 hits
- [ ] `grep -r "resolve_char_method" compiler/ori_types/` -- 0 hits
- [ ] `grep -r "resolve_ordering_method" compiler/ori_types/` -- 0 hits
- [ ] `grep -r "resolve_duration_method" compiler/ori_types/` -- 0 hits
- [ ] `grep -r "resolve_size_method" compiler/ori_types/` -- 0 hits
- [ ] `grep -r "resolve_tuple_method" compiler/ori_types/` -- 0 hits
- [ ] `grep -r "resolve_error_method" compiler/ori_types/` -- 0 hits
- [ ] `grep -r "resolve_str_method" compiler/ori_types/` -- 0 hits
- [ ] `grep -r "resolve_channel_method" compiler/ori_types/` -- 0 hits
- [ ] `grep -r "resolve_set_method" compiler/ori_types/` -- 0 hits
- [ ] `grep -r "resolve_list_method" compiler/ori_types/` -- 0 hits
- [ ] `grep -r "resolve_option_method" compiler/ori_types/` -- 0 hits
- [ ] `grep -r "resolve_result_method" compiler/ori_types/` -- 0 hits
- [ ] `grep -r "resolve_map_method" compiler/ori_types/` -- 0 hits
- [ ] `grep -r "resolve_range_method" compiler/ori_types/` -- 0 hits
- [ ] `grep -r "resolve_iterator_method" compiler/ori_types/` -- 0 hits
- [ ] `grep -r "TYPECK_BUILTIN_METHODS"` (entire repo) -- 0 hits outside comments/docs
- [ ] `grep -r "DEI_ONLY_METHODS"` (entire repo) -- 0 hits outside comments/docs

---

## Implementation Tasks (Summary)

### 09.1 tag_to_type_tag() Bridge
- [ ] Create `compiler/ori_types/src/infer/expr/registry_bridge.rs`
- [ ] Implement `tag_to_type_tag()` with exhaustive match
- [ ] Add `mod registry_bridge;` in `infer/expr/mod.rs`
- [ ] Unit tests for all Tag variants
- [ ] `cargo c -p ori_types` passes

### 09.2 type_tag_to_idx() Bridge
- [ ] Implement `type_tag_to_idx()` in `registry_bridge.rs`
- [ ] Implement `extract_elem()` helper
- [ ] Unit tests for primitives, SelfType, parameterized, FreshVar
- [ ] `cargo c -p ori_types` passes

### 09.3 Replace Dispatcher
- [ ] New `resolve_builtin_method()` with registry lookup
- [ ] `resolve_computed_return()` for all computed cases
- [ ] `cargo t -p ori_types` and `cargo st` pass

### 09.4 Delete resolve_* Functions
- [ ] Phase A: 9 trivial deletions
- [ ] Phase B: 4 mostly-static deletions
- [ ] Phase C: 5 complex deletions
- [ ] Phase D: Iterator deletion
- [ ] `./test-all.sh` passes

### 09.5 Delete TYPECK_BUILTIN_METHODS
- [ ] Remove constant and pub export
- [ ] Migrate consistency tests
- [ ] `cargo c -p ori_types` and `cargo t -p oric` pass

### 09.6 TypeFlow Integration
- [ ] `type_flow` field on MethodDef (coordinate with Section 01)
- [ ] Set TypeFlow on Iterator/DEI methods in registry
- [ ] New `unify_higher_order_constraints` using registry
- [ ] `cargo st` passes

### 09.7 DEI_ONLY_METHODS
- [ ] Implement `is_dei_only_method()` from registry
- [ ] Replace calling site in `calls.rs`
- [ ] Delete constant
- [ ] `cargo st` passes

### 09.8 WellKnownNames
- [ ] Document boundary (no code changes)
- [ ] Verify no breakage

### 09.9 Validation
- [ ] Pre-migration baseline
- [ ] Per-subsection verification
- [ ] Full regression gate
- [ ] Coverage verification
- [ ] Grep verification
- [ ] New unit tests

---

## Exit Criteria

- [ ] **All 18 `resolve_*_method` functions deleted** (except `resolve_named_type_method`)
- [ ] **`TYPECK_BUILTIN_METHODS` deleted** from `methods.rs` and `lib.rs`
- [ ] **`DEI_ONLY_METHODS` deleted** from `methods.rs`
- [ ] **Single registry lookup** in `resolve_builtin_method()` + `resolve_computed_return()`
- [ ] **`unify_higher_order_constraints`** uses `TypeFlow` from registry, not string matching
- [ ] **DEI gating** uses `is_dei_only_method()` derived from registry, not constant array
- [ ] **`WellKnownNames`** unchanged and functional
- [ ] **`registry_bridge.rs`** contains `tag_to_type_tag()`, `type_tag_to_idx()`, `extract_elem()`
- [ ] **`cargo t -p ori_types`** passes (>= baseline)
- [ ] **`cargo st`** passes (>= baseline)
- [ ] **`./test-all.sh`** passes
- [ ] **Grep verification** clean: no references to deleted functions/constants outside comments
- [ ] **Net line count** in `methods.rs`: reduced by ~400+ lines (from ~878 to ~200-300)
- [ ] **Net line count** in `calls.rs`: reduced by ~30 lines (unify_higher_order_constraints simplified)
