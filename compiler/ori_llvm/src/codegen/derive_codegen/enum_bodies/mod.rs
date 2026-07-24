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
mod enum_format;
mod enum_hashable;

use ori_ir::{CombineOp, DerivedTrait, Name, StructBody};
use ori_types::{Idx, Pool, Tag, VariantDef, VariantFields};

use super::super::function_compiler::FunctionCompiler;
use super::super::value_id::{LLVMTypeId, ValueId};
use super::{emit_derive_return, setup_derive_function, verify_derive_function};

/// Load one payload field of a variant from a payload POINTER at `i64_offset`
/// slots, returning the loaded value and its LLVM type.
///
/// Shared by the Eq slow path (`enum_eq`), Comparable (`enum_comparable`), and
/// Hashable (`enum_hashable`) per-variant loops — each loads payload fields
/// through the same gep+resolve+register+load sequence and differs only in the
/// combine tail applied afterward. The caller advances its own `i64_offset`
/// from the returned LLVM type via
/// `TypeLayoutResolver::type_store_size(ty).div_ceil(8).max(1)`.
pub(in crate::codegen::derive_codegen) fn load_payload_slot_field<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    i64_ty: LLVMTypeId,
    payload: ValueId,
    i64_offset: u64,
    field_type: Idx,
    slot_name: &str,
    val_name: &str,
) -> (ValueId, inkwell::types::BasicTypeEnum<'a>) {
    #[expect(
        clippy::cast_possible_wrap,
        reason = "payload slot offset bounded by variant layout"
    )]
    let slot_idx = fc.builder_mut().const_i64(i64_offset as i64);
    let slot = fc
        .builder_mut()
        .gep(i64_ty, payload, &[slot_idx], slot_name);
    let field_llvm_ty = fc.resolve_type(field_type);
    let field_ty_id = fc.builder_mut().register_type(field_llvm_ty);
    let val = fc.builder_mut().load(field_ty_id, slot, val_name);
    (val, field_llvm_ty)
}

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
    mono: bool,
) {
    match *struct_body {
        StructBody::ForEachField { combine, field_op } => {
            let setup =
                setup_derive_function(fc, trait_kind, type_name, type_idx, type_name_str, mono);
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
            verify_derive_function(fc, setup.func_id, "compile_enum_match_variants");
        }
        StructBody::CloneFields => {
            // Clone on enum = identity return (value type in LLVM)
            let setup =
                setup_derive_function(fc, trait_kind, type_name, type_idx, type_name_str, mono);
            let self_val = setup.self_val.expect("Clone has self");
            emit_derive_return(fc, setup.func_id, &setup.abi, Some(self_val));
            verify_derive_function(fc, setup.func_id, "compile_enum_clone");
        }
        StructBody::FormatFields { separator, .. } => {
            enum_format::compile_enum_format(
                fc,
                trait_kind,
                type_name,
                type_idx,
                type_name_str,
                variants,
                separator,
                mono,
            );
        }
        StructBody::DefaultConstruct => {
            // Default has SumBody::NotSupported and never reaches enum dispatch.
            tracing::trace!(
                name = %type_name_str,
                derive = %trait_kind.method_name(),
                "DefaultConstruct does not apply to sum types"
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
/// Unit/Never fields are zero-sized and don't
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
