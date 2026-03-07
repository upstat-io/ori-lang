//! Method resolution for Named/Applied types (user-defined).
//!
//! All builtin type method resolution is now handled by the registry-based
//! dispatcher in [`super::resolve_builtin_method`]. This module only contains
//! [`resolve_named_type_method`] for user-defined structs, enums, and newtypes.

use crate::infer::InferEngine;
use crate::Idx;

/// Resolve methods on Named/Applied types (user-defined structs, enums, newtypes).
///
/// For newtypes, supports `.unwrap()` to extract the inner value.
pub(super) fn resolve_named_type_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method_name: &str,
) -> Option<Idx> {
    // Check type registry for newtype unwrap
    if method_name == "unwrap" || method_name == "inner" || method_name == "value" {
        if let Some(type_registry) = engine.type_registry() {
            if let Some(entry) = type_registry.get_by_idx(receiver_ty) {
                if let crate::TypeKind::Newtype { underlying } = &entry.kind {
                    return Some(*underlying);
                }
            }
        }
    }

    // Common methods on any user-defined type
    match method_name {
        "to_str" | "debug" => Some(Idx::STR),
        _ => None,
    }
}
