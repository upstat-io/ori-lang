//! Type-specific canonical representation helpers.
//!
//! Extracted from `canonical/mod.rs` to keep the main module under
//! the 500-line limit. Each function maps a specific compound type
//! (collection, map, function, tuple, struct, enum, option, result)
//! to its canonical `MachineRepr`.

use ori_ir::Name;
use ori_types::{Idx, Pool};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::enum_repr::{EnumRepr, EnumTag, VariantRepr};
use crate::layout::{
    compute_field_layout, compute_payload_layout, field_align, field_size, is_trivial_repr,
    round_up,
};
use crate::repr::{IntWidth, MachineRepr};
use crate::struct_repr::{ClosureRepr, FatRepr, FieldRepr, StructRepr, TupleRepr};

use super::canonical_inner;

/// Canonicalize a collection element into a fat pointer.
///
/// Returns `None` if the element type cannot be canonicalized.
pub(super) fn canonical_collection(
    pool: &Pool,
    elem_idx: Idx,
    visiting: &mut FxHashSet<Idx>,
    cache: &mut FxHashMap<Idx, MachineRepr>,
) -> Option<MachineRepr> {
    Some(MachineRepr::FatPointer(FatRepr::Collection {
        element_repr: Box::new(canonical_inner(pool, elem_idx, visiting, cache)?),
    }))
}

/// Canonicalize a map into a fat pointer with key and value reprs.
///
/// Returns `None` if either the key or value type cannot be canonicalized.
pub(super) fn canonical_map(
    pool: &Pool,
    resolved: Idx,
    visiting: &mut FxHashSet<Idx>,
    cache: &mut FxHashMap<Idx, MachineRepr>,
) -> Option<MachineRepr> {
    Some(MachineRepr::FatPointer(FatRepr::Map {
        key_repr: Box::new(canonical_inner(
            pool,
            pool.map_key(resolved),
            visiting,
            cache,
        )?),
        value_repr: Box::new(canonical_inner(
            pool,
            pool.map_value(resolved),
            visiting,
            cache,
        )?),
    }))
}

/// Canonicalize a function type into a closure representation.
///
/// Returns `None` if any parameter or the return type cannot be canonicalized.
pub(super) fn canonical_function(
    pool: &Pool,
    resolved: Idx,
    visiting: &mut FxHashSet<Idx>,
    cache: &mut FxHashMap<Idx, MachineRepr>,
) -> Option<MachineRepr> {
    let params: Option<Vec<MachineRepr>> = pool
        .function_params(resolved)
        .into_iter()
        .map(|p| canonical_inner(pool, p, visiting, cache))
        .collect();
    let ret = canonical_inner(pool, pool.function_return(resolved), visiting, cache)?;
    Some(MachineRepr::Closure(ClosureRepr {
        params: params?,
        ret: Box::new(ret),
    }))
}

/// Canonicalize a tuple into an anonymous struct with positional fields.
///
/// Returns `None` if any element type cannot be canonicalized.
pub(super) fn canonical_tuple(
    pool: &Pool,
    resolved: Idx,
    visiting: &mut FxHashSet<Idx>,
    cache: &mut FxHashMap<Idx, MachineRepr>,
) -> Option<MachineRepr> {
    let fields: Option<Vec<FieldRepr>> = pool
        .tuple_elems(resolved)
        .into_iter()
        .enumerate()
        .map(|(i, elem_idx)| {
            let repr = canonical_inner(pool, elem_idx, visiting, cache)?;
            let idx_u32 = u32::try_from(i).unwrap_or(u32::MAX);
            Some(FieldRepr {
                name: Name::new(0, idx_u32),
                original_index: idx_u32,
                offset: 0, // Set by §06 layout
                repr,
            })
        })
        .collect();
    let fields = fields?;
    let trivial = fields.iter().all(|f| is_trivial_repr(&f.repr));
    Some(TupleRepr::to_machine_repr(fields, trivial))
}

/// Canonicalize a struct type with named fields.
///
/// Returns `None` if any field type cannot be canonicalized.
pub(super) fn canonical_struct(
    pool: &Pool,
    resolved: Idx,
    visiting: &mut FxHashSet<Idx>,
    cache: &mut FxHashMap<Idx, MachineRepr>,
) -> Option<MachineRepr> {
    let fields: Option<Vec<FieldRepr>> = pool
        .struct_fields(resolved)
        .into_iter()
        .enumerate()
        .map(|(i, (name, field_idx))| {
            let repr = canonical_inner(pool, field_idx, visiting, cache)?;
            let idx_u32 = u32::try_from(i).unwrap_or(u32::MAX);
            Some(FieldRepr {
                name,
                original_index: idx_u32,
                offset: 0, // Set by §06 layout
                repr,
            })
        })
        .collect();
    let fields = fields?;
    let trivial = fields.iter().all(|f| is_trivial_repr(&f.repr));
    let (size, align) = compute_field_layout(&fields);
    Some(MachineRepr::Struct(StructRepr {
        fields,
        size,
        align,
        trivial,
    }))
}

/// Canonicalize an enum type with explicit i64 tag.
///
/// Returns `None` if any variant's field type cannot be canonicalized.
pub(super) fn canonical_enum(
    pool: &Pool,
    resolved: Idx,
    visiting: &mut FxHashSet<Idx>,
    cache: &mut FxHashMap<Idx, MachineRepr>,
) -> Option<MachineRepr> {
    let variants: Option<Vec<VariantRepr>> = pool
        .enum_variants(resolved)
        .into_iter()
        .map(|(name, field_idxs)| {
            let fields: Option<Vec<MachineRepr>> = field_idxs
                .into_iter()
                .map(|fi| canonical_inner(pool, fi, visiting, cache))
                .collect();
            let fields = fields?;
            let (size, alignment) = compute_payload_layout(&fields);
            Some(VariantRepr {
                name,
                fields,
                size,
                alignment,
            })
        })
        .collect();
    let variants = variants?;

    let max_payload = variants.iter().map(|v| v.size).max().unwrap_or(0);
    let max_align = variants
        .iter()
        .map(|v| v.alignment)
        .max()
        .unwrap_or(1)
        .max(8); // tag is i64 → 8-byte aligned
    let size = 8 + round_up(max_payload, max_align);

    Some(MachineRepr::Enum(EnumRepr {
        tag: EnumTag::Explicit {
            width: IntWidth::I64,
        },
        variants,
        size,
        align: max_align,
    }))
}

/// Build canonical `Option<T>` as a 2-variant enum: None (unit) + Some(T).
pub(super) fn canonical_option(inner_repr: MachineRepr) -> MachineRepr {
    let none_variant = VariantRepr {
        name: Name::new(0, 0), // "None" — exact interning handled at call sites
        fields: vec![],
        size: 0,
        alignment: 1,
    };
    let some_size = field_size(&inner_repr);
    let some_align = field_align(&inner_repr);
    let some_variant = VariantRepr {
        name: Name::new(0, 1), // "Some"
        fields: vec![inner_repr],
        size: some_size,
        alignment: some_align,
    };

    let max_payload = some_size;
    let align = some_align.max(8); // i64 tag
    let size = 8 + round_up(max_payload, align);

    MachineRepr::Enum(EnumRepr {
        tag: EnumTag::Explicit {
            width: IntWidth::I64,
        },
        variants: vec![none_variant, some_variant],
        size,
        align,
    })
}

/// Build canonical `Result<T, E>` as a 2-variant enum: Ok(T) + Err(E).
pub(super) fn canonical_result(ok_repr: MachineRepr, err_repr: MachineRepr) -> MachineRepr {
    let ok_size = field_size(&ok_repr);
    let ok_align = field_align(&ok_repr);
    let ok_variant = VariantRepr {
        name: Name::new(0, 0), // "Ok"
        fields: vec![ok_repr],
        size: ok_size,
        alignment: ok_align,
    };

    let err_size = field_size(&err_repr);
    let err_align = field_align(&err_repr);
    let err_variant = VariantRepr {
        name: Name::new(0, 1), // "Err"
        fields: vec![err_repr],
        size: err_size,
        alignment: err_align,
    };

    let max_payload = ok_size.max(err_size);
    let align = ok_align.max(err_align).max(8); // i64 tag
    let size = 8 + round_up(max_payload, align);

    MachineRepr::Enum(EnumRepr {
        tag: EnumTag::Explicit {
            width: IntWidth::I64,
        },
        variants: vec![ok_variant, err_variant],
        size,
        align,
    })
}
