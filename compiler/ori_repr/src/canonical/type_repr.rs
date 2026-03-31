//! Type-specific canonical representation helpers.
//!
//! Extracted from `canonical/mod.rs` to keep the main module under
//! the 500-line limit. Each function maps a specific compound type
//! (collection, map, function, tuple, struct, enum, option, result)
//! to its canonical `MachineRepr`.

use ori_ir::Name;
use ori_types::{Idx, Pool};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::enum_repr::{min_tag_width, EnumRepr, EnumTag, VariantRepr};
use crate::layout::{compute_enum_payload_layout, compute_field_layout, is_trivial_repr, round_up};
use crate::repr::MachineRepr;
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
            let (size, alignment) = compute_enum_payload_layout(&fields);
            Some(VariantRepr {
                name,
                fields,
                size,
                alignment,
            })
        })
        .collect();
    let variants = variants?;

    // §07.2: Single-variant enum (newtype erasure) — no tag needed.
    if variants.len() == 1 {
        let variant = &variants[0];
        let (size, align) = if variant.size == 0 {
            (1, 1) // Unit newtype: minimal size for LLVM struct
        } else {
            (variant.size, variant.alignment.max(1))
        };
        return Some(MachineRepr::Enum(EnumRepr {
            tag: EnumTag::None,
            variants,
            size,
            align,
        }));
    }

    let tag_width = min_tag_width(variants.len());
    let tag_size = tag_width.size_bytes();
    let max_payload = variants.iter().map(|v| v.size).max().unwrap_or(0);
    let max_variant_align = variants.iter().map(|v| v.alignment).max().unwrap_or(1);
    // Non-unit enums: LLVM uses [M x i64] payload with alignment 8, so the
    // tag is always padded to 8 bytes regardless of tag width. All-unit enums
    // have no payload, so the tag determines the entire size and alignment.
    let (size, align) = if max_payload == 0 {
        (tag_size, tag_size) // All-unit enum: just the narrowed tag
    } else {
        // Payload alignment is at least 8 because of LLVM's [M x i64] convention
        let payload_align = max_variant_align.max(8);
        let align = payload_align;
        let size = 8 + round_up(max_payload, align); // 8 = padded tag slot
        (size, align)
    };

    Some(MachineRepr::Enum(EnumRepr {
        tag: EnumTag::Explicit { width: tag_width },
        variants,
        size,
        align,
    }))
}

/// Build canonical `Option<T>` as a 2-variant enum: None (unit) + Some(T).
///
/// Attempts niche optimization first (§07.2): if the inner type has a niche,
/// `None` is encoded via the niche value, eliminating the explicit tag field.
/// Falls back to `I64` tag for types without niches (e.g., `int`, `[T]`).
pub(super) fn canonical_option(inner_repr: &MachineRepr) -> MachineRepr {
    crate::layout::niche::optimize_option_repr(inner_repr)
}

/// Build canonical `Result<T, E>` as a 2-variant enum: Ok(T) + Err(E).
///
/// Attempts niche optimization first (§07.2): if either payload has a niche,
/// the other variant is encoded via that niche value.
/// Falls back to `I64` tag for types without niches.
pub(super) fn canonical_result(ok_repr: &MachineRepr, err_repr: &MachineRepr) -> MachineRepr {
    crate::layout::niche::optimize_result_repr(ok_repr, err_repr)
}
