//! Pool-based type store-size computation.
//!
//! Computes type sizes with trailing alignment padding. Must stay in sync with
//! `TypeLayoutResolver::type_store_size()` in `ori_llvm` — both compute the
//! same logical size for every type, just at different abstraction levels
//! (Pool indices here vs LLVM `BasicTypeEnum` there).
//!
//! See `compiler/ori_llvm/src/codegen/type_info/mod.rs`.

use ori_types::{Idx, Tag};

/// Recursive store-size computation from Pool type information.
///
/// TODO(type_strategy_registry/section-11): Extract shared type layout logic to `ori_ir`.
/// This function duplicates `ori_llvm::codegen::type_info::TypeLayoutResolver::type_store_size()`.
///
/// **Sync point**: Must agree with `TypeLayoutResolver::type_store_size()` in `ori_llvm`
/// (`compiler/ori_llvm/src/codegen/type_info/mod.rs`). Both compute the same logical size
/// for every type.
pub(crate) fn pool_type_store_size(ty: Idx, pool: &ori_types::Pool, depth: u32) -> i64 {
    if depth > 16 {
        return 8; // Prevent infinite recursion on recursive types
    }
    // Resolve Named/Applied/Alias types to their underlying structural type.
    let ty = pool.resolve_fully(ty);
    let tag = pool.tag(ty);
    match tag {
        // 0-byte types
        Tag::Unit | Tag::Never => 0,
        // 1-byte types (matches TypeInfo::Bool/Byte/Ordering)
        Tag::Bool | Tag::Byte | Tag::Ordering => 1,
        // 4-byte types
        Tag::Char => 4,
        // 8-byte: scalars (i64/f64/i64-encoded) and opaque pointers (heap handles)
        Tag::Int
        | Tag::Float
        | Tag::Duration
        | Tag::Size
        | Tag::Error
        | Tag::Iterator
        | Tag::DoubleEndedIterator
        | Tag::Channel
        | Tag::Range => 8,
        // 16-byte fat pointer: {fn_ptr: ptr, env_ptr: ptr}
        Tag::Function => 16,
        // 24-byte fat values: {i64, i64, ptr}
        Tag::Str | Tag::List | Tag::Set | Tag::Map => 24,
        // Composite types: walk fields with inter-field alignment padding,
        // then round up to max alignment for trailing padding (array stride).
        // Must match the ABI layout in ori_repr::layout::compute_field_layout()
        // and ori_llvm's type_store_size (which uses LLVM's size_of()).
        Tag::Struct => {
            let fields = pool.struct_fields(ty);
            aggregate_size_with_padding(fields.iter().map(|(_, field_ty)| *field_ty), pool, depth)
        }
        Tag::Tuple => {
            let elems = pool.tuple_elems(ty);
            aggregate_size_with_padding(elems.iter().copied(), pool, depth)
        }
        Tag::Option => {
            // Option<T> = {i64 tag, T payload} — include trailing alignment padding.
            // LLVM represents this as a struct; size_of includes padding to alignment.
            // The tag is i64 (align 8), so the struct alignment is at least 8.
            let payload_size = pool_type_store_size(pool.option_inner(ty), pool, depth + 1);
            round_up_i64(8 + payload_size, 8)
        }
        Tag::Result => {
            // Result<T, E> = {i64 tag, max(T, E) payload} — include trailing alignment padding.
            let ok_ty = pool.result_ok(ty);
            let err_ty = pool.result_err(ty);
            let ok_size = pool_type_store_size(ok_ty, pool, depth + 1);
            let err_size = pool_type_store_size(err_ty, pool, depth + 1);
            round_up_i64(8 + ok_size.max(err_size), 8)
        }
        Tag::Enum => {
            // §07.1: Enum tags are narrowed by variant count (i8/i16/i32/i64).
            // Enum payloads use [M x i64] array layout where each field
            // occupies ceil(field_size / 8) * 8 bytes (at least one i64 slot).
            // This differs from struct/tuple natural alignment padding.
            let variants = pool.enum_variants(ty);
            let max_payload: i64 = variants
                .iter()
                .map(|(_, fields)| enum_payload_size(fields.iter().copied(), pool, depth))
                .max()
                .unwrap_or(0);
            let tag_bytes = enum_tag_bytes(variants.len());
            if max_payload == 0 {
                // All-unit enum: just the tag, no payload.
                // LLVM: { i8 } → size 1 for ≤256 variants.
                tag_bytes
            } else {
                // Payload enum: { iN_tag, [M x i64] }.
                // The [M x i64] payload has alignment 8, so the tag is always
                // padded up to 8 regardless of tag width. Total = 8 + payload.
                8 + max_payload
            }
        }
        // Meta-types should not appear in for-yield element positions.
        // If they do, it's a compiler bug — panic rather than silently missize.
        Tag::Var
        | Tag::BoundVar
        | Tag::RigidVar
        | Tag::Scheme
        | Tag::Projection
        | Tag::ModuleNs
        | Tag::Infer
        | Tag::SelfType
        | Tag::Named
        | Tag::Applied
        | Tag::Alias
        | Tag::Borrowed => {
            debug_assert!(
                false,
                "pool_type_store_size: unexpected meta-type tag {tag:?} — \
                 this type should not appear in for-yield element positions"
            );
            8
        }
    }
}

/// Alignment of a Pool type in bytes.
///
/// Recursively computes alignment for composite types (struct/tuple) as the
/// max field alignment, matching `type_alignment()` in `ori_llvm`. Without
/// recursion, nested aggregates of small fields (e.g., `(char, char)`) would
/// incorrectly default to alignment 8 instead of their actual max alignment.
fn pool_type_alignment(ty: Idx, pool: &ori_types::Pool) -> i64 {
    pool_type_alignment_inner(ty, pool, 0)
}

fn pool_type_alignment_inner(ty: Idx, pool: &ori_types::Pool, depth: u32) -> i64 {
    if depth > 16 {
        return 8;
    }
    let resolved = pool.resolve_fully(ty);
    let tag = pool.tag(resolved);
    match tag {
        Tag::Unit | Tag::Never | Tag::Bool | Tag::Byte | Tag::Ordering => 1,
        Tag::Char => 4,
        Tag::Struct => {
            let fields = pool.struct_fields(resolved);
            fields
                .iter()
                .map(|(_, field_ty)| pool_type_alignment_inner(*field_ty, pool, depth + 1))
                .max()
                .unwrap_or(1)
        }
        Tag::Tuple => {
            let elems = pool.tuple_elems(resolved);
            elems
                .iter()
                .map(|elem_ty| pool_type_alignment_inner(*elem_ty, pool, depth + 1))
                .max()
                .unwrap_or(1)
        }
        Tag::Enum => {
            // §07.1: all-unit enums have narrowed tag alignment.
            // BUG-04-008: Check actual payload size, not just field presence.
            // Variants with only void/unit fields have zero payload size.
            let variants = pool.enum_variants(resolved);
            let has_payload = variants
                .iter()
                .any(|(_, fields)| enum_payload_size(fields.iter().copied(), pool, depth) > 0);
            if has_payload {
                8 // Payload enum: [M x i64] array dominates alignment.
            } else {
                // All-unit enum: alignment = tag width.
                enum_tag_bytes(variants.len())
            }
        }
        _ => 8,
    }
}

/// Compute total size of an aggregate (struct/tuple/enum-variant payload)
/// with proper inter-field alignment padding.
///
/// Each field is aligned to its natural alignment before placement, matching
/// the ABI layout in `ori_repr::layout::compute_field_layout()` and LLVM's
/// own struct size computation. The result includes trailing padding to the
/// max field alignment (correct array element stride).
fn aggregate_size_with_padding(
    fields: impl Iterator<Item = Idx>,
    pool: &ori_types::Pool,
    depth: u32,
) -> i64 {
    let mut offset = 0i64;
    let mut max_align = 1i64;
    for field_ty in fields {
        let fa = pool_type_alignment(field_ty, pool);
        offset = round_up_i64(offset, fa);
        offset += pool_type_store_size(field_ty, pool, depth + 1);
        max_align = max_align.max(fa);
    }
    round_up_i64(offset, max_align)
}

/// Compute total payload size for an enum variant's fields using i64-slot layout.
///
/// Enum payloads are stored as `[M x i64]` arrays in LLVM. Each field occupies
/// at least one full i64 slot (8 bytes), regardless of its natural alignment.
/// This matches `compute_variant_field_offsets()` in the LLVM emitter.
fn enum_payload_size(fields: impl Iterator<Item = Idx>, pool: &ori_types::Pool, depth: u32) -> i64 {
    fields
        .map(|field_ty| {
            let field_size = pool_type_store_size(field_ty, pool, depth + 1);
            round_up_i64(field_size, 8)
        })
        .sum()
}

/// Enum tag size in bytes based on variant count.
///
/// Must stay in sync with `ori_repr::min_tag_width()`. Inlined here to avoid
/// a circular dependency (`ori_repr` depends on `ori_arc`).
///
/// §07.1: all enums with ≤256 variants use i8 tags (1 byte).
fn enum_tag_bytes(variant_count: usize) -> i64 {
    match variant_count {
        0 | 1 => 1,
        n => {
            let bits_needed = usize::BITS - (n - 1).leading_zeros();
            match bits_needed {
                0..=8 => 1,
                9..=16 => 2,
                17..=32 => 4,
                _ => 8,
            }
        }
    }
}

/// Round `value` up to the next multiple of `align`.
fn round_up_i64(value: i64, align: i64) -> i64 {
    if align <= 1 {
        return value;
    }
    (value + align - 1) / align * align
}
