//! LLVM type size utilities.
//!
//! Approximate store size calculation for LLVM types. Extracted from
//! `layout_resolver.rs` for file-size hygiene.

use inkwell::types::BasicTypeEnum;

/// Approximate store size of an LLVM type in bytes.
///
/// **Sync point**: `ForYieldLowerer::type_store_size()` in `ori_arc` mirrors
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
            // Use LLVM's own store size which includes alignment padding.
            // The naive sum-of-fields misses inter-field and trailing padding
            // for structs with mixed-size fields (e.g., {str, bool} → 25 not 32).
            if let Some(sz) = st
                .size_of()
                .and_then(inkwell::values::IntValue::get_zero_extended_constant)
            {
                return sz;
            }
            // Fallback: sum field sizes (may undercount for padded structs).
            let mut total = 0u64;
            for i in 0..st.count_fields() {
                if let Some(field) = st.get_field_type_at_index(i) {
                    total += type_store_size_inner(field, depth + 1);
                }
            }
            total
        }
        BasicTypeEnum::ArrayType(at) => {
            let elem_size = type_store_size_inner(at.get_element_type(), depth + 1);
            elem_size * u64::from(at.len())
        }
        _ => 8,
    }
}
