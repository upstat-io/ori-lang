//! Pool-based type store-size computation.
//!
//! Sums field sizes without alignment padding. Must stay in sync with
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
pub(crate) fn pool_type_store_size(ty: Idx, pool: &ori_types::Pool, depth: u32) -> i64 {
    if depth > 16 {
        return 8; // Prevent infinite recursion on recursive types
    }
    // Resolve Named/Applied/Alias types to their underlying structural type.
    let ty = pool.resolve_fully(ty);
    let tag = pool.tag(ty);
    match tag {
        Tag::Bool | Tag::Byte => 1,
        Tag::Char => 4,
        Tag::Unit => 0,
        Tag::Str | Tag::List | Tag::Set | Tag::Map => 24, // {i64, i64, ptr}
        Tag::Struct => pool
            .struct_fields(ty)
            .iter()
            .map(|(_, field_ty)| pool_type_store_size(*field_ty, pool, depth + 1))
            .sum(),
        Tag::Tuple => pool
            .tuple_elems(ty)
            .iter()
            .map(|&field_ty| pool_type_store_size(field_ty, pool, depth + 1))
            .sum(),
        Tag::Option => {
            // Option<T> = {i64 tag, T payload}
            8 + pool_type_store_size(pool.option_inner(ty), pool, depth + 1)
        }
        Tag::Result => {
            // Result<T, E> = {i64 tag, max(T, E) payload}
            let ok_ty = pool.result_ok(ty);
            let err_ty = pool.result_err(ty);
            let ok_size = pool_type_store_size(ok_ty, pool, depth + 1);
            let err_size = pool_type_store_size(err_ty, pool, depth + 1);
            8 + ok_size.max(err_size)
        }
        _ => 8, // Int, Float, Duration, Size, pointer-sized default
    }
}
