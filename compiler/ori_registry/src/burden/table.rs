//! `BURDEN_TABLE` — pure-const burden specifications for builtin `TypeTag`s.
//!
//! # Template Categories
//!
//! - Primitives (`int`, `float`, `bool`, `char`, `byte`, `void`, `Never`,
//!   `Duration`, `Size`, `Ordering`) and `Range<T>`: empty
//!   `BuiltinBurdenSpec` (`Value` semantics; no heap, no drops).
//! - Heap collections (`Str`, `List`, `Map`, `Set`): `self_heap_alloc = true`.
//!   `List`/`Map`/`Set` carry an `element_burden` type-parameter placeholder;
//!   `Str` carries `None` (UTF-8 bytes have no recursive burden).
//! - Sum templates (`Option<T>`, `Result<T, E>`): per-variant burdens with
//!   `transfers_on_match` populated for each payload binding.
//!
//! # Composition Layer
//!
//! Other generic builtins (`Channel`, `Iterator`, `DoubleEndedIterator`,
//! `Tuple`, `Function`, `Error`) are NOT represented here — their
//! monomorphized entries are produced by the burden-composition layer that
//! consumes these templates and substitutes concrete type-parameter `TypeId`s.
//! Entries in this table are CONST TEMPLATES; the composition layer reads
//! them and produces monomorphized `UserBurdenSpec` entries (heap-backed in
//! `ori_types`) for each first-instantiation type, substituting the
//! type-parameter placeholders defined below at composition time.

use core::num::NonZeroU32;

use super::{BuiltinBurdenSpec, TransferKind, TransferRule, TypeId, VariantBurden, VariantId};
use crate::TypeTag;

// Compile-time helpers for const construction.

// Const-eval limitation: `unreachable!("msg")` invokes format_args internally,
// which is non-const. Per Rust today, only argument-less `unreachable!()` is
// permitted in const context. Callers MUST pass literal nonzero values; the
// `None` arm is unreachable by construction.
macro_rules! tid {
    ($n:expr) => {
        TypeId::new(match NonZeroU32::new($n) {
            Some(n) => n,
            None => unreachable!(),
        })
    };
}

macro_rules! vid {
    ($n:expr) => {
        VariantId::new(match NonZeroU32::new($n) {
            Some(n) => n,
            None => unreachable!(),
        })
    };
}

/// Derives a burden [`TypeId`] from a [`TypeTag`] discriminant + 1.
///
/// # INVARIANT
///
/// `TypeTag` is `#[repr(u8)]` with sequential discriminants starting at 0;
/// the burden [`TypeId`] newtype wraps `NonZeroU32` so id 0 is reserved.
/// Shifting `+1` keeps the burden id space in one-to-one correspondence
/// with the `TypeTag` discriminant while preserving the niche-optimized
/// `Option<TypeId>` layout. This const-fn is the SSOT: every
/// `pub const TYPE_ID_<NAME>` below is derived mechanically from its
/// `TypeTag::<NAME>` discriminant, so reordering `TypeTag` propagates
/// here without manual sync.
#[must_use]
pub const fn burden_type_id(tag: TypeTag) -> TypeId {
    let n = (tag as u32) + 1;
    TypeId::new(match NonZeroU32::new(n) {
        Some(nz) => nz,
        None => unreachable!(),
    })
}

// Local TypeId constants — mechanically derived from `TypeTag` discriminants.
//
// Consumers at `ori_arc::lower::burden_lookup` translate `ori_ir::TypeId`
// to this space by shifting +1.

pub const TYPE_ID_INT: TypeId = burden_type_id(TypeTag::Int);
pub const TYPE_ID_FLOAT: TypeId = burden_type_id(TypeTag::Float);
pub const TYPE_ID_BOOL: TypeId = burden_type_id(TypeTag::Bool);
pub const TYPE_ID_CHAR: TypeId = burden_type_id(TypeTag::Char);
pub const TYPE_ID_BYTE: TypeId = burden_type_id(TypeTag::Byte);
pub const TYPE_ID_UNIT: TypeId = burden_type_id(TypeTag::Unit);
pub const TYPE_ID_NEVER: TypeId = burden_type_id(TypeTag::Never);
pub const TYPE_ID_DURATION: TypeId = burden_type_id(TypeTag::Duration);
pub const TYPE_ID_SIZE: TypeId = burden_type_id(TypeTag::Size);
pub const TYPE_ID_ORDERING: TypeId = burden_type_id(TypeTag::Ordering);
pub const TYPE_ID_STR: TypeId = burden_type_id(TypeTag::Str);
pub const TYPE_ID_ERROR: TypeId = burden_type_id(TypeTag::Error);
pub const TYPE_ID_LIST: TypeId = burden_type_id(TypeTag::List);
pub const TYPE_ID_MAP: TypeId = burden_type_id(TypeTag::Map);
pub const TYPE_ID_SET: TypeId = burden_type_id(TypeTag::Set);
pub const TYPE_ID_RANGE: TypeId = burden_type_id(TypeTag::Range);
pub const TYPE_ID_TUPLE: TypeId = burden_type_id(TypeTag::Tuple);
pub const TYPE_ID_OPTION: TypeId = burden_type_id(TypeTag::Option);
pub const TYPE_ID_RESULT: TypeId = burden_type_id(TypeTag::Result);
pub const TYPE_ID_CHANNEL: TypeId = burden_type_id(TypeTag::Channel);
pub const TYPE_ID_FUNCTION: TypeId = burden_type_id(TypeTag::Function);
pub const TYPE_ID_ITERATOR: TypeId = burden_type_id(TypeTag::Iterator);
pub const TYPE_ID_DOUBLE_ENDED_ITERATOR: TypeId = burden_type_id(TypeTag::DoubleEndedIterator);

// Type-parameter placeholders.
//
// Templates for generic builtins use sentinels to mark "type parameter slot"
// positions. The composition layer substitutes concrete TypeIds when
// monomorphizing.
//
// Reserved at the top of u32 so they cannot collide with real translated
// TypeIds (real ids start at +1 and grow with TypeTag count + user-type
// pool size, never approaching u32::MAX).

pub const TYPE_PARAM_T: TypeId = tid!(u32::MAX);
pub const TYPE_PARAM_E: TypeId = tid!(u32::MAX - 1);

// Variant IDs for sum-type templates.

pub const OPTION_VARIANT_NONE: VariantId = vid!(1);
pub const OPTION_VARIANT_SOME: VariantId = vid!(2);
pub const RESULT_VARIANT_OK: VariantId = vid!(1);
pub const RESULT_VARIANT_ERR: VariantId = vid!(2);

// Per-variant transfer tables for Option/Result.

// Option<T>::Some(value) — single binding of T, moved on match.
const OPTION_SOME_TRANSFERS: &[TransferRule] = &[TransferRule {
    source_field_path: &[],
    binding_index: 0,
    field_type: TYPE_PARAM_T,
    transfer_kind: TransferKind::Move,
}];

// Result<T, E>::Ok(value) — single binding of T, moved on match.
const RESULT_OK_TRANSFERS: &[TransferRule] = &[TransferRule {
    source_field_path: &[],
    binding_index: 0,
    field_type: TYPE_PARAM_T,
    transfer_kind: TransferKind::Move,
}];

// Result<T, E>::Err(error) — single binding of E, moved on match.
const RESULT_ERR_TRANSFERS: &[TransferRule] = &[TransferRule {
    source_field_path: &[],
    binding_index: 0,
    field_type: TYPE_PARAM_E,
    transfer_kind: TransferKind::Move,
}];

const OPTION_VARIANTS: &[VariantBurden] = &[
    VariantBurden {
        variant_id: OPTION_VARIANT_NONE,
        transfers_on_match: &[],
        retained_owned: &[],
    },
    VariantBurden {
        variant_id: OPTION_VARIANT_SOME,
        transfers_on_match: OPTION_SOME_TRANSFERS,
        retained_owned: &[],
    },
];

const RESULT_VARIANTS: &[VariantBurden] = &[
    VariantBurden {
        variant_id: RESULT_VARIANT_OK,
        transfers_on_match: RESULT_OK_TRANSFERS,
        retained_owned: &[],
    },
    VariantBurden {
        variant_id: RESULT_VARIANT_ERR,
        transfers_on_match: RESULT_ERR_TRANSFERS,
        retained_owned: &[],
    },
];

// The table.
//
// One entry per builtin type template. Monomorphized entries (`Option<int>`,
// `Result<{str: int}, str>`, ...) are produced by the burden-composition
// layer downstream.

pub const BURDEN_TABLE: &[(TypeId, BuiltinBurdenSpec)] = &[
    // Primitives — Value semantics; no heap, no drops.
    (TYPE_ID_INT, BuiltinBurdenSpec::EMPTY),
    (TYPE_ID_FLOAT, BuiltinBurdenSpec::EMPTY),
    (TYPE_ID_BOOL, BuiltinBurdenSpec::EMPTY),
    (TYPE_ID_CHAR, BuiltinBurdenSpec::EMPTY),
    (TYPE_ID_BYTE, BuiltinBurdenSpec::EMPTY),
    (TYPE_ID_UNIT, BuiltinBurdenSpec::EMPTY),
    (TYPE_ID_NEVER, BuiltinBurdenSpec::EMPTY),
    (TYPE_ID_DURATION, BuiltinBurdenSpec::EMPTY),
    (TYPE_ID_SIZE, BuiltinBurdenSpec::EMPTY),
    (TYPE_ID_ORDERING, BuiltinBurdenSpec::EMPTY),
    // Range<T> — range bounds are inline scalars; no recursive burden.
    (TYPE_ID_RANGE, BuiltinBurdenSpec::EMPTY),
    // Str — UTF-8 bytes; heap-allocated, no recursive element burden.
    (
        TYPE_ID_STR,
        BuiltinBurdenSpec {
            self_heap_alloc: true,
            element_burden: None,
            ..BuiltinBurdenSpec::EMPTY
        },
    ),
    // [T] — list template; element burden parameterized by T.
    (
        TYPE_ID_LIST,
        BuiltinBurdenSpec {
            self_heap_alloc: true,
            element_burden: Some(TYPE_PARAM_T),
            ..BuiltinBurdenSpec::EMPTY
        },
    ),
    // {K: V} — map template; the single `element_burden` slot carries a
    // single type-parameter placeholder. Key/value separation is deferred
    // to the composition layer, which may amend the schema if separate
    // key_burden + value_burden slots turn out to be required.
    (
        TYPE_ID_MAP,
        BuiltinBurdenSpec {
            self_heap_alloc: true,
            element_burden: Some(TYPE_PARAM_T),
            ..BuiltinBurdenSpec::EMPTY
        },
    ),
    // Set<T> — set template; element burden parameterized by T.
    (
        TYPE_ID_SET,
        BuiltinBurdenSpec {
            self_heap_alloc: true,
            element_burden: Some(TYPE_PARAM_T),
            ..BuiltinBurdenSpec::EMPTY
        },
    ),
    // Option<T> — None empty; Some(T) transfers T on match.
    (
        TYPE_ID_OPTION,
        BuiltinBurdenSpec {
            variant_burdens: OPTION_VARIANTS,
            ..BuiltinBurdenSpec::EMPTY
        },
    ),
    // Result<T, E> — Ok(T) transfers T; Err(E) transfers E.
    (
        TYPE_ID_RESULT,
        BuiltinBurdenSpec {
            variant_burdens: RESULT_VARIANTS,
            ..BuiltinBurdenSpec::EMPTY
        },
    ),
];

// Registry surface.

/// Zero-sized handle for builtin burden lookups.
///
/// All state lives in `BURDEN_TABLE`; this type names the lookup entry
/// point and gives consumers (`ori_arc::lower::burden_lookup`) a single
/// import surface (`use ori_registry::burden::table::BurdenRegistry`).
pub struct BurdenRegistry;

impl BurdenRegistry {
    /// Looks up a builtin's burden spec by its local `TypeId`.
    ///
    /// Returns `None` for any `TypeId` not in `BURDEN_TABLE` — user-defined
    /// types (which live on `TypeRegistry`) and generic-builtin
    /// instantiations (monomorphized entries owned by the composition layer).
    #[must_use]
    pub fn lookup_builtin(type_id: TypeId) -> Option<&'static BuiltinBurdenSpec> {
        BURDEN_TABLE
            .iter()
            .find(|(id, _)| *id == type_id)
            .map(|(_, spec)| spec)
    }
}

#[cfg(test)]
mod tests;
