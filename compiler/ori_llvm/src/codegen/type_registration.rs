//! Eager registration of user-defined types for V2 codegen.
//!
//! Walks the type checker's output (`TypeEntry` list) and eagerly resolves
//! each user-defined type through the `TypeLayoutResolver`. This ensures
//! LLVM named struct types exist in the module before any function body
//! compilation begins.
//!
//! Replaces `ModuleCompiler::register_struct_with_types()` and
//! `register_sum_type_from_decl()` with a single function that operates
//! on V2 infrastructure.

use ori_types::TypeEntry;

use super::type_info::TypeLayoutResolver;

/// Eagerly resolve all user-defined types, creating LLVM named struct types.
///
/// Must be called once before function compilation begins. The
/// `TypeLayoutResolver` creates named LLVM struct types for structs and
/// enums, which are then available for `struct_gep`, field access, and
/// enum tag/payload operations.
///
/// Generic types (those with non-empty `type_params`) are skipped — they
/// require monomorphization and will be resolved when concrete instances
/// are encountered.
pub fn register_user_types(resolver: &TypeLayoutResolver<'_, '_, '_>, types: &[TypeEntry]) {
    for entry in types {
        // Skip generic types — they're resolved during monomorphization
        if !entry.type_params.is_empty() {
            tracing::trace!(
                name = ?entry.name,
                "skipping generic type registration"
            );
            continue;
        }

        tracing::debug!(
            name = ?entry.name,
            idx = ?entry.idx,
            kind = ?entry.kind,
            "registering user type"
        );

        // Eagerly resolve the type to create the LLVM named struct.
        // The resolver caches the result, so subsequent calls to
        // resolver.resolve(entry.idx) return the cached type.
        resolver.resolve(entry.idx);
    }
}

#[cfg(test)]
#[allow(
    clippy::uninlined_format_args,
    clippy::doc_markdown,
    reason = "test code — style relaxed for clarity"
)]
mod tests;
