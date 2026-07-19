//! ABI size and alignment computation.
//!
//! Walks `TypeInfo` (repr-aware via `ReprPlan`) to compute the byte size and
//! alignment LLVM lays out for each type. Consumed by the passing-mode
//! classification in [`super`] (`compute_param_passing` /
//! `compute_return_passing`). The sync test in `super::tests` cross-checks
//! these walkers against `TypeLayoutResolver`'s actual LLVM layouts.

use ori_repr::ReprPlan;
use ori_types::Idx;
use rustc_hash::FxHashSet;

use crate::codegen::type_info::{field_is_non_void, EnumVariantInfo, TypeInfoStore};

/// Compute the ABI size of a type in bytes.
///
/// For types where `TypeInfo::size` returns `None` (Tuple, Struct, Enum),
/// walks child types recursively via the store to compute the total size.
/// Recursive types (e.g., `type Expr = Leaf(int) | Binop(Expr, Expr)`) are
/// detected via a visiting set and treated as pointer-sized (8 bytes).
///
/// When `repr_plan` is provided, consults it for niche-encoded Option/Result
/// types — niche layouts omit the tag field, reducing ABI size.
pub fn abi_size(ty: Idx, store: &TypeInfoStore<'_>, repr_plan: Option<&ReprPlan>) -> u64 {
    let mut visiting = FxHashSet::default();
    abi_size_inner(ty, store, repr_plan, &mut visiting)
}

/// Check if a type has niche encoding in the `ReprPlan`.
fn is_niche_encoded(ty: Idx, store: &TypeInfoStore<'_>, repr_plan: Option<&ReprPlan>) -> bool {
    repr_plan
        .and_then(|plan| plan.enum_repr_with_fallback(store.pool(), ty))
        .is_some_and(|e| e.tag.is_niche())
}

/// Check if a type has tagged-pointer encoding in the `ReprPlan`.
///
/// Tagged-pointer enums are exactly 8 bytes (one i64 slot). The ABI passes
/// them as a single Direct register, identical to a regular pointer or i64.
fn is_tagged_ptr_encoded(ty: Idx, store: &TypeInfoStore<'_>, repr_plan: Option<&ReprPlan>) -> bool {
    repr_plan
        .and_then(|plan| plan.enum_repr_with_fallback(store.pool(), ty))
        .is_some_and(|e| e.tag.is_tagged_ptr())
}

/// Check if an enum type (Option/Result/user-defined) has niche or tagless
/// encoding in the `ReprPlan`. Returns `Some(size)` if an optimized layout applies,
/// `None` to fall through to the explicit tag computation.
fn niche_enum_size(
    ty: Idx,
    variants: &[EnumVariantInfo],
    store: &TypeInfoStore<'_>,
    repr_plan: Option<&ReprPlan>,
    visiting: &mut FxHashSet<Idx>,
) -> Option<u64> {
    let plan = repr_plan?;
    let enum_repr = plan.enum_repr_with_fallback(store.pool(), ty)?;

    // Tagless and niche enums lower to a struct of one variant's non-void
    // fields (resolve_enum_tagless / resolve_enum_niche) — size is the
    // alignment-padded aggregate of those fields, with an i8 placeholder
    // (1 byte) when the variant is field-less.
    let variant = if enum_repr.tag.is_tagless() {
        variants.first()?
    } else if let ori_repr::EnumTag::Niche {
        niche_variant_idx, ..
    } = &enum_repr.tag
    {
        // INVARIANT: niche payload-variant selection assumes binary enums —
        // index `usize::from(niche_variant_idx == 0)` picks "the other"
        // variant only when exactly 2 variants exist.
        debug_assert!(
            variants.len() == 2,
            "niche payload-variant selection assumes binary enums (got {} variants)",
            variants.len()
        );
        variants.get(usize::from(*niche_variant_idx == 0))?
    } else {
        return None;
    };

    let payload = aggregate_size_with_padding(
        variant
            .fields
            .iter()
            .copied()
            .filter(|&f| field_is_non_void(store.pool(), f)),
        store,
        repr_plan,
        visiting,
    );
    Some(payload.max(1))
}

fn abi_size_inner(
    ty: Idx,
    store: &TypeInfoStore<'_>,
    repr_plan: Option<&ReprPlan>,
    visiting: &mut FxHashSet<Idx>,
) -> u64 {
    use crate::codegen::type_info::TypeInfo;

    let info = store.get(ty);
    if let Some(size) = info.size() {
        return size;
    }

    // Cycle detection: recursive types must use heap indirection,
    // so treat them as pointer-sized when encountered again.
    if !visiting.insert(ty) {
        return 8;
    }

    // Dynamic-size types: compute recursively. Struct/tuple aggregates use
    // alignment-aware padded layout (matching LLVM's struct layout) so
    // Direct/Indirect classification agrees with the size LLVM actually
    // lays out. dereferenceable(N) remains legal either way (minimum).
    let result = match &info {
        TypeInfo::Option { inner } => {
            if is_niche_encoded(ty, store, repr_plan) {
                abi_size_inner(*inner, store, repr_plan, visiting)
            } else {
                // Explicit tag: {i64 tag, payload} — trailing padding to the
                // struct's 8-byte alignment, matching LLVM's alloc size.
                (8 + abi_size_inner(*inner, store, repr_plan, visiting)).div_ceil(8) * 8
            }
        }
        TypeInfo::Result { ok, err } => {
            let ok_size = abi_size_inner(*ok, store, repr_plan, visiting);
            let err_size = abi_size_inner(*err, store, repr_plan, visiting);
            if is_niche_encoded(ty, store, repr_plan) {
                ok_size.max(err_size)
            } else {
                // Explicit tag: {i64 tag, max(ok, err) payload} — trailing
                // padding to the struct's 8-byte alignment (LLVM alloc size).
                (8 + ok_size.max(err_size)).div_ceil(8) * 8
            }
        }
        TypeInfo::Tuple { elements } => {
            aggregate_size_with_padding(elements.iter().copied(), store, repr_plan, visiting)
        }
        TypeInfo::Struct { fields } => aggregate_size_with_padding(
            fields.iter().map(|&(_, ty)| ty),
            store,
            repr_plan,
            visiting,
        ),
        TypeInfo::Enum { variants } => {
            // Tagged-pointer enums are a single 8-byte slot
            // regardless of variant count or payload size. Check before the
            // niche/explicit-tag computation since the encoding is uniform.
            if is_tagged_ptr_encoded(ty, store, repr_plan) {
                visiting.remove(&ty);
                return 8;
            }
            if let Some(size) = niche_enum_size(ty, variants, store, repr_plan, visiting) {
                visiting.remove(&ty);
                return size;
            }
            // Enum layout: {tag, [M x i64] payload} — tag width varies
            // per enum via min_tag_width. The non-void-field slot sum is shared
            // with resolve_enum_explicit in enum_layout.rs via
            // max_variant_payload_bytes (codegen::type_info::type_size) so the
            // slot/round-up rule cannot diverge; the per-field sizer here is the
            // ABI size (no boxing oracle — this walker treats recursive fields
            // via the visiting set, not the box).
            let tag_size = u64::from(ori_repr::min_tag_width(variants.len()).size_bytes());
            let max_payload = crate::codegen::type_info::type_size::max_variant_payload_bytes(
                variants,
                store.pool(),
                |f| abi_size_inner(f, store, repr_plan, visiting),
            );
            if max_payload == 0 {
                tag_size // All-unit enum: { tag } = tag_size bytes
            } else {
                // Tag is padded to 8 due to [M x i64] payload alignment
                8 + max_payload
            }
        }
        _ => 8, // Fallback: pointer-sized
    };

    visiting.remove(&ty);
    result
}

/// Aggregate (struct/tuple) size with inter-field alignment padding plus
/// trailing padding to the max field alignment — matches LLVM's struct
/// layout so the Direct/Indirect ABI classification agrees with the size
/// LLVM lays out.
fn aggregate_size_with_padding(
    fields: impl Iterator<Item = Idx>,
    store: &TypeInfoStore<'_>,
    repr_plan: Option<&ReprPlan>,
    visiting: &mut FxHashSet<Idx>,
) -> u64 {
    let mut offset = 0u64;
    let mut max_align = 1u64;
    for field in fields {
        let align = abi_alignment(field, store, repr_plan, 0);
        if align > 1 {
            offset = offset.div_ceil(align) * align;
        }
        offset += abi_size_inner(field, store, repr_plan, visiting);
        max_align = max_align.max(align);
    }
    if max_align > 1 {
        offset = offset.div_ceil(max_align) * max_align;
    }
    offset
}

/// Recursive ABI alignment for a type — max field alignment for
/// struct/tuple aggregates, repr-aware payload alignment for niche-encoded
/// Option/Result and tagless/niche enums (mirroring `TypeLayoutResolver`'s
/// lowered layouts), `TypeInfo::alignment` for everything else.
pub(crate) fn abi_alignment(
    ty: Idx,
    store: &TypeInfoStore<'_>,
    repr_plan: Option<&ReprPlan>,
    depth: u32,
) -> u64 {
    use crate::codegen::type_info::TypeInfo;

    if depth > 16 {
        return 8;
    }
    match &store.get(ty) {
        TypeInfo::Tuple { elements } => elements
            .iter()
            .map(|&e| abi_alignment(e, store, repr_plan, depth + 1))
            .max()
            .unwrap_or(1),
        TypeInfo::Struct { fields } => fields
            .iter()
            .map(|&(_, t)| abi_alignment(t, store, repr_plan, depth + 1))
            .max()
            .unwrap_or(1),
        // Niche-encoded Option lowers to its payload type directly (no i64
        // tag slot) — alignment is the payload's, not 8.
        TypeInfo::Option { inner } if is_niche_encoded(ty, store, repr_plan) => {
            abi_alignment(*inner, store, repr_plan, depth + 1)
        }
        // Niche-encoded Result uses the selected payload arm's alignment.
        // Equal-size integer arms have equal alignment, so Ok is a valid tie.
        TypeInfo::Result { ok, err } if is_niche_encoded(ty, store, repr_plan) => {
            let mut visiting = FxHashSet::default();
            let ok_size = abi_size_inner(*ok, store, repr_plan, &mut visiting);
            let err_size = abi_size_inner(*err, store, repr_plan, &mut visiting);
            let arm = if ok_size >= err_size { *ok } else { *err };
            abi_alignment(arm, store, repr_plan, depth + 1)
        }
        TypeInfo::Enum { variants } => {
            enum_payload_alignment(ty, variants, store, repr_plan, depth)
        }
        info => u64::from(info.alignment()),
    }
}

/// Alignment of an enum's lowered LLVM struct. Tagless/niche enums lower to
/// one variant's non-void fields (i8 placeholder when field-less) — max field
/// alignment. Tagged-pointer enums: 8 (one i64 slot). Explicit-tag enums:
/// 8 when a payload exists ({tag, [M x i64]}), else the tag's own alignment
/// (all-unit enums lower to a bare tag struct).
fn enum_payload_alignment(
    ty: Idx,
    variants: &[EnumVariantInfo],
    store: &TypeInfoStore<'_>,
    repr_plan: Option<&ReprPlan>,
    depth: u32,
) -> u64 {
    let Some(enum_repr) = repr_plan.and_then(|p| p.enum_repr_with_fallback(store.pool(), ty))
    else {
        return explicit_enum_alignment(variants, store);
    };
    let variant = if enum_repr.tag.is_tagless() {
        variants.first()
    } else if let ori_repr::EnumTag::Niche {
        niche_variant_idx, ..
    } = &enum_repr.tag
    {
        // INVARIANT: niche payload-variant selection assumes binary enums
        // (same invariant as niche_enum_size above).
        debug_assert!(
            variants.len() == 2,
            "niche payload-variant selection assumes binary enums (got {} variants)",
            variants.len()
        );
        variants.get(usize::from(*niche_variant_idx == 0))
    } else if enum_repr.tag.is_tagged_ptr() {
        return 8;
    } else {
        return explicit_enum_alignment(variants, store);
    };
    variant.map_or(1, |v| {
        v.fields
            .iter()
            .filter(|&&f| field_is_non_void(store.pool(), f))
            .map(|&f| abi_alignment(f, store, repr_plan, depth + 1))
            .max()
            .unwrap_or(1)
    })
}

/// Alignment of an explicit-tag enum: 8 when any variant carries a non-void
/// payload field (the `[M x i64]` payload array forces 8-byte alignment),
/// else the tag's own width (all-unit enums lower to `{ tag }`).
fn explicit_enum_alignment(variants: &[EnumVariantInfo], store: &TypeInfoStore<'_>) -> u64 {
    let has_payload = variants
        .iter()
        .any(|v| v.fields.iter().any(|&f| field_is_non_void(store.pool(), f)));
    if has_payload {
        8
    } else {
        // Tag widths are 1/2/4/8 bytes — LLVM int alignment equals size.
        u64::from(ori_repr::min_tag_width(variants.len()).size_bytes())
    }
}

/// Repr-aware pointer-passing alignment for Indirect/Sret, clamped to u32.
pub(super) fn indirect_alignment(
    ty: Idx,
    store: &TypeInfoStore<'_>,
    repr_plan: Option<&ReprPlan>,
) -> u32 {
    // ABI alignment is a max over primitive alignments (<= 8) — a value
    // above u32::MAX is impossible; fail loud rather than fabricate 8.
    u32::try_from(abi_alignment(ty, store, repr_plan, 0)).expect("ABI alignment fits u32")
}
