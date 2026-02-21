//! Field-level operations for derived trait codegen.
//!
//! Provides [`emit_field_operation`], a unified dispatcher that handles
//! equality (Eq), comparison (Comparable), and hash coercion (Hashable)
//! for all field types via a single `TypeInfo` match.

use ori_ir::{DerivedTrait, FieldOp};
use ori_types::Idx;
use tracing::trace;

use super::super::function_compiler::FunctionCompiler;
use super::super::type_info::TypeInfo;
use super::super::value_id::{LLVMTypeId, ValueId};
use super::emit_method_call_for_derive;

/// Emit a field-level operation for the given type.
///
/// Dispatches once on `TypeInfo`, then applies the requested [`FieldOp`].
/// For binary ops (`Equals`, `Compare`), `rhs` must be `Some`. For `Hash`
/// (unary), `rhs` should be `None`.
pub(super) fn emit_field_operation<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    op: FieldOp,
    lhs: ValueId,
    rhs: Option<ValueId>,
    field_type: Idx,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let info = fc.type_info().get(field_type);
    match &info {
        // Integer-like signed: signed compare, already i64 for hash
        TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => match op {
            FieldOp::Equals => fc.builder_mut().icmp_eq(lhs, expect_rhs(rhs), name),
            FieldOp::Compare => {
                fc.builder_mut()
                    .emit_icmp_ordering(lhs, expect_rhs(rhs), name, true)
            }
            FieldOp::Hash => lhs,
        },

        // Unsigned small: unsigned compare, zext to i64 for hash
        TypeInfo::Byte | TypeInfo::Bool => match op {
            FieldOp::Equals => fc.builder_mut().icmp_eq(lhs, expect_rhs(rhs), name),
            FieldOp::Compare => {
                fc.builder_mut()
                    .emit_icmp_ordering(lhs, expect_rhs(rhs), name, false)
            }
            FieldOp::Hash => {
                let i64_ty = fc.builder_mut().i64_type();
                fc.builder_mut().zext(lhs, i64_ty, name)
            }
        },

        // Char/Ordering: unsigned compare, sext to i64 for hash
        TypeInfo::Char | TypeInfo::Ordering => match op {
            FieldOp::Equals => fc.builder_mut().icmp_eq(lhs, expect_rhs(rhs), name),
            FieldOp::Compare => {
                fc.builder_mut()
                    .emit_icmp_ordering(lhs, expect_rhs(rhs), name, false)
            }
            FieldOp::Hash => {
                let i64_ty = fc.builder_mut().i64_type();
                fc.builder_mut().sext(lhs, i64_ty, name)
            }
        },

        TypeInfo::Float => match op {
            FieldOp::Equals => fc.builder_mut().fcmp_oeq(lhs, expect_rhs(rhs), name),
            FieldOp::Compare => fc
                .builder_mut()
                .emit_fcmp_ordering(lhs, expect_rhs(rhs), name),
            FieldOp::Hash => {
                // Normalize ±0.0 → +0.0 before bitcast to preserve hash contract:
                // (-0.0).equals(0.0) is true, so their hashes must match.
                let pos_zero = fc.builder_mut().const_f64(0.0);
                let is_zero = fc
                    .builder_mut()
                    .fcmp_oeq(lhs, pos_zero, &format!("{name}.is_zero"));
                let normalized =
                    fc.builder_mut()
                        .select(is_zero, pos_zero, lhs, &format!("{name}.normalized"));
                let i64_ty = fc.builder_mut().i64_type();
                fc.builder_mut().bitcast(normalized, i64_ty, name)
            }
        },

        TypeInfo::Str => match op {
            FieldOp::Equals => emit_str_eq_call(fc, lhs, expect_rhs(rhs), name, str_ty_id),
            FieldOp::Compare => emit_str_compare_call(fc, lhs, expect_rhs(rhs), name, str_ty_id),
            FieldOp::Hash => emit_str_hash_call(fc, lhs, name, str_ty_id),
        },

        TypeInfo::Struct { .. } => {
            let trait_kind = match op {
                FieldOp::Equals => DerivedTrait::Eq,
                FieldOp::Compare => DerivedTrait::Comparable,
                FieldOp::Hash => DerivedTrait::Hashable,
            };
            let nested_name = fc.type_idx_to_name(field_type);
            let method = fc.intern(trait_kind.method_name());
            if let Some(type_name) = nested_name {
                if let Some((fid, abi)) = fc.get_method_function(type_name, method) {
                    return match op {
                        FieldOp::Hash => emit_method_call_for_derive(fc, fid, &abi, &[lhs], name),
                        _ => emit_method_call_for_derive(
                            fc,
                            fid,
                            &abi,
                            &[lhs, expect_rhs(rhs)],
                            name,
                        ),
                    };
                }
            }
            emit_fallback(fc, op, lhs, rhs, name)
        }

        _ => {
            trace!(
                ?info,
                ?op,
                "unsupported field type for derive — using fallback"
            );
            emit_fallback(fc, op, lhs, rhs, name)
        }
    }
}

/// Unwrap `rhs` for binary operations (Eq, Compare).
fn expect_rhs(rhs: Option<ValueId>) -> ValueId {
    rhs.expect("binary field op (Equals/Compare) requires rhs")
}

/// Fallback values when a type doesn't support the operation.
fn emit_fallback<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    op: FieldOp,
    lhs: ValueId,
    rhs: Option<ValueId>,
    name: &str,
) -> ValueId {
    match op {
        FieldOp::Equals => fc.builder_mut().icmp_eq(lhs, expect_rhs(rhs), name),
        FieldOp::Compare => fc.builder_mut().const_i8(1), // Equal
        FieldOp::Hash => fc.builder_mut().const_i64(0),
    }
}

// ---------------------------------------------------------------------------
// String runtime helpers (alloca+store+call pattern)
// ---------------------------------------------------------------------------

/// Call `ori_str_eq(a: ptr, b: ptr) -> bool` via alloca+store pattern.
fn emit_str_eq_call<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let ptr_ty = fc.builder_mut().ptr_type();
    let bool_ty = fc.builder_mut().bool_type();

    let lhs_alloca = fc.entry_alloca(str_ty_id, "lhs_str");
    fc.builder_mut().store(lhs, lhs_alloca);
    let rhs_alloca = fc.entry_alloca(str_ty_id, "rhs_str");
    fc.builder_mut().store(rhs, rhs_alloca);

    let eq_fn = fc
        .builder_mut()
        .get_or_declare_function("ori_str_eq", &[ptr_ty, ptr_ty], bool_ty);
    fc.builder_mut()
        .call(eq_fn, &[lhs_alloca, rhs_alloca], name)
        .unwrap_or_else(|| fc.builder_mut().const_bool(false))
}

/// Call `ori_str_compare(a: ptr, b: ptr) -> i8` via alloca+store pattern.
fn emit_str_compare_call<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let ptr_ty = fc.builder_mut().ptr_type();
    let i8_ty = fc.builder_mut().i8_type();

    let lhs_alloca = fc.entry_alloca(str_ty_id, "cmp_lhs_str");
    fc.builder_mut().store(lhs, lhs_alloca);
    let rhs_alloca = fc.entry_alloca(str_ty_id, "cmp_rhs_str");
    fc.builder_mut().store(rhs, rhs_alloca);

    let cmp_fn =
        fc.builder_mut()
            .get_or_declare_function("ori_str_compare", &[ptr_ty, ptr_ty], i8_ty);
    fc.builder_mut()
        .call(cmp_fn, &[lhs_alloca, rhs_alloca], name)
        .unwrap_or_else(|| fc.builder_mut().const_i8(1)) // Equal fallback
}

/// Call `ori_str_hash(s: ptr) -> i64` via alloca+store pattern.
fn emit_str_hash_call<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    val: ValueId,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let ptr_ty = fc.builder_mut().ptr_type();
    let i64_ty = fc.builder_mut().i64_type();

    let val_alloca = fc.entry_alloca(str_ty_id, &format!("{name}.str"));
    fc.builder_mut().store(val, val_alloca);

    let hash_fn = fc
        .builder_mut()
        .get_or_declare_function("ori_str_hash", &[ptr_ty], i64_ty);
    fc.builder_mut()
        .call(hash_fn, &[val_alloca], name)
        .unwrap_or_else(|| fc.builder_mut().const_i64(0))
}
