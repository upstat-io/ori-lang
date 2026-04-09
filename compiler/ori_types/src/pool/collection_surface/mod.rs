//! Collection surface discovery for public ABI protection.
//!
//! Walks a type's structure to find reachable `List` and `Set` types hidden
//! inside containers (Option, Result, Tuple, Map) and user-defined types
//! (Struct, Enum). Used by:
//!
//! - **Same-module**: `repr_setup.rs` collects `Idx` values for `pub_type_indices`
//! - **Cross-module**: type checker collects merkle hashes for export in `TypedModule`
//!
//! The walker is generic over a callback, keeping the core traversal logic in
//! one place while supporting both use cases.

use crate::pool::Pool;
use crate::{Idx, Tag};

/// Walk a type and discover all collection types (List, Set) reachable
/// through container wrappers and user-defined struct/enum fields.
///
/// Calls `on_collection(idx)` for each discovered collection type. If the
/// original type differs from the resolved type (e.g., a Named wrapper),
/// the callback is called for both indices.
///
/// Uses `resolve_fully()` to see through Named/Applied/Alias type wrappers.
/// Bounded at depth 32 to prevent pathological recursion on deeply nested types.
pub fn walk_collection_types<F>(pool: &Pool, ty: Idx, on_collection: &mut F)
where
    F: FnMut(Idx),
{
    walk_recursive(pool, ty, on_collection, 0);
}

fn walk_recursive<F>(pool: &Pool, ty: Idx, on_collection: &mut F, depth: u32)
where
    F: FnMut(Idx),
{
    if depth > 32 {
        return;
    }

    let resolved = pool.resolve_fully(ty);
    let tag = pool.tag(resolved);

    match tag {
        Tag::List | Tag::Set => {
            on_collection(resolved);
            // Also report the wrapper idx if different (Named/Applied alias).
            if ty != resolved {
                on_collection(ty);
            }
        }
        Tag::Option => {
            walk_recursive(pool, pool.option_inner(resolved), on_collection, depth + 1);
        }
        Tag::Result => {
            walk_recursive(pool, pool.result_ok(resolved), on_collection, depth + 1);
            walk_recursive(pool, pool.result_err(resolved), on_collection, depth + 1);
        }
        Tag::Tuple => {
            for elem in pool.tuple_elems(resolved) {
                walk_recursive(pool, elem, on_collection, depth + 1);
            }
        }
        Tag::Map => {
            walk_recursive(pool, pool.map_key(resolved), on_collection, depth + 1);
            walk_recursive(pool, pool.map_value(resolved), on_collection, depth + 1);
        }
        Tag::Struct => {
            for (_name, field_ty) in pool.struct_fields(resolved) {
                walk_recursive(pool, field_ty, on_collection, depth + 1);
            }
        }
        Tag::Enum => {
            for (_name, field_types) in pool.enum_variants(resolved) {
                for field_ty in field_types {
                    walk_recursive(pool, field_ty, on_collection, depth + 1);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
