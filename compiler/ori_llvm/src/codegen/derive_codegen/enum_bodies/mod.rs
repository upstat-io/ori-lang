//! Enum body implementations for derived method codegen.
//!
//! `compile_enum_match_variants` handles `SumBody::MatchVariants`:
//! - Tag comparison for Eq (`enum_eq`), Comparable (`enum_comparable`),
//!   Hashable (`enum_hashable`)
//! - Clone is identity return (enums are value types in LLVM)
//!
//! For payload enums, per-variant field comparison is emitted via switch on
//! tag value. Each variant's payload fields are accessed through GEP into
//! the `[M x i64]` payload array.

mod enum_comparable;
mod enum_eq;
mod enum_hashable;

use ori_ir::{CombineOp, DerivedTrait, Name, StructBody};
use ori_types::{Idx, Pool, Tag, VariantDef, VariantFields};

use super::super::function_compiler::FunctionCompiler;
use super::{emit_derive_return, setup_derive_function};

/// Generate derived methods for enum types using `SumBody::MatchVariants`.
///
/// Dispatches on the `struct_body` strategy:
/// - `ForEachField`: Eq, Comparable, Hashable — with payload comparison
/// - `CloneFields`: identity return (enums are value types in LLVM)
/// - Other strategies: not yet implemented (trace warning)
pub(super) fn compile_enum_match_variants<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    trait_kind: DerivedTrait,
    type_name: Name,
    type_idx: Idx,
    type_name_str: &str,
    variants: &[VariantDef],
    struct_body: &StructBody,
) {
    match *struct_body {
        StructBody::ForEachField { combine, field_op } => {
            let setup = setup_derive_function(fc, trait_kind, type_name, type_idx, type_name_str);
            match combine {
                CombineOp::AllTrue => {
                    enum_eq::emit_enum_all_true(fc, &setup, variants, field_op);
                }
                CombineOp::Lexicographic => {
                    enum_comparable::emit_enum_lexicographic(fc, &setup, variants, field_op);
                }
                CombineOp::HashCombine => {
                    enum_hashable::emit_enum_hash_combine(fc, &setup, variants, field_op);
                }
            }
        }
        StructBody::CloneFields => {
            // Clone on enum = identity return (value type in LLVM)
            let setup = setup_derive_function(fc, trait_kind, type_name, type_idx, type_name_str);
            let self_val = setup.self_val.expect("Clone has self");
            emit_derive_return(fc, setup.func_id, &setup.abi, Some(self_val));
        }
        _ => {
            tracing::trace!(
                name = %type_name_str,
                derive = %trait_kind.method_name(),
                "enum derive strategy not yet implemented for this struct_body"
            );
        }
    }
}

/// Extract field type indices from a `VariantFields`.
pub(in crate::codegen::derive_codegen) fn variant_field_types(fields: &VariantFields) -> Vec<Idx> {
    match fields {
        VariantFields::Unit => vec![],
        VariantFields::Tuple(types) => types.clone(),
        VariantFields::Record(field_defs) => field_defs.iter().map(|f| f.ty).collect(),
    }
}

/// Extract non-void field type indices from a `VariantFields`.
///
/// BUG-04-008 / TPR-07-006: Unit/Never fields are zero-sized and don't
/// occupy payload space in the LLVM enum layout (`resolve_enum()` skips
/// them). Derive codegen must also skip them to keep offsets in sync.
pub(in crate::codegen::derive_codegen) fn variant_non_void_field_types(
    fields: &VariantFields,
    pool: &Pool,
) -> Vec<Idx> {
    variant_field_types(fields)
        .into_iter()
        .filter(|&ft| {
            let resolved = pool.resolve_fully(ft);
            !matches!(pool.tag(resolved), Tag::Unit | Tag::Never)
        })
        .collect()
}
