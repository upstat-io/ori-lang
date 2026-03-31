//! LLVM type size utilities.
//!
//! Store size calculation for LLVM types including alignment padding.
//! Must stay in sync with `pool_type_store_size()` in `ori_arc`.

use inkwell::types::BasicTypeEnum;

/// Store size of an LLVM type in bytes, including trailing alignment padding.
///
/// **Sync point**: `pool_type_store_size()` in `ori_arc` mirrors
/// this logic at the Pool level. Both must agree on sizes for all types.
pub(crate) fn type_store_size(ty: BasicTypeEnum<'_>) -> u64 {
    type_store_size_inner(ty, 0)
}

/// Inner implementation with depth tracking for recursive struct types.
fn type_store_size_inner(ty: BasicTypeEnum<'_>, depth: u32) -> u64 {
    if depth > 16 {
        return 8;
    }
    match ty {
        BasicTypeEnum::IntType(t) => {
            let bits = t.get_bit_width();
            u64::from(bits).div_ceil(8)
        }
        BasicTypeEnum::StructType(st) => {
            if st.is_opaque() {
                return 8;
            }
            // Try LLVM's own size computation (includes all padding).
            if let Some(sz) = st
                .size_of()
                .and_then(inkwell::values::IntValue::get_zero_extended_constant)
            {
                return sz;
            }
            // Fallback: compute with alignment padding.
            // Sum fields with inter-field alignment, then round to max alignment.
            let mut offset = 0u64;
            let mut max_align = 1u64;
            for i in 0..st.count_fields() {
                if let Some(field) = st.get_field_type_at_index(i) {
                    let sz = type_store_size_inner(field, depth + 1);
                    let align = type_alignment(field, depth + 1);
                    if align > 0 {
                        offset = offset.div_ceil(align) * align;
                    }
                    offset += sz;
                    max_align = max_align.max(align);
                }
            }
            // Trailing padding to struct alignment.
            if max_align > 1 {
                offset = offset.div_ceil(max_align) * max_align;
            }
            offset
        }
        BasicTypeEnum::ArrayType(at) => {
            let elem_size = type_store_size_inner(at.get_element_type(), depth + 1);
            elem_size * u64::from(at.len())
        }
        _ => 8,
    }
}

/// Alignment of an LLVM type in bytes.
fn type_alignment(ty: BasicTypeEnum<'_>, depth: u32) -> u64 {
    if depth > 16 {
        return 8;
    }
    match ty {
        BasicTypeEnum::IntType(t) => {
            let bytes = u64::from(t.get_bit_width()).div_ceil(8);
            // Standard alignment: size rounded up to next power of 2, capped at 8
            bytes.next_power_of_two().min(8)
        }
        BasicTypeEnum::StructType(st) => {
            if st.is_opaque() {
                return 8;
            }
            let mut max_align = 1u64;
            for i in 0..st.count_fields() {
                if let Some(field) = st.get_field_type_at_index(i) {
                    max_align = max_align.max(type_alignment(field, depth + 1));
                }
            }
            max_align
        }
        BasicTypeEnum::ArrayType(at) => type_alignment(at.get_element_type(), depth + 1),
        // Pointers, floats: 8 bytes
        _ => 8,
    }
}
