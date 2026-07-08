//! Option<T> equality thunk generator for derived trait callbacks.

use ori_types::Idx;

use crate::codegen::derive_codegen::verify_derive_function;
use crate::codegen::function_compiler::FunctionCompiler;
use crate::codegen::value_id::ValueId;

use super::thunks::get_or_create_derive_eq_thunk;

/// Generate a thunk `(ptr, ptr) -> bool` for Option<T> equality.
///
/// Loads the tag (i64 at offset 0) from each pointer, compares tags,
/// then if both Some, compares payloads at offset 8 using the inner eq thunk.
pub(super) fn get_or_create_option_eq_thunk<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    ty: Idx,
    inner_ty: Idx,
) -> Option<ValueId> {
    let inner_info = fc.type_info().get(inner_ty);
    let inner_eq_thunk = get_or_create_derive_eq_thunk(fc, inner_ty, &inner_info)?;

    let func_name = format!("_ori_eq_option_{}", ty.raw());
    let ptr_ty = fc.builder_mut().ptr_type();
    let bool_ty = fc.builder_mut().bool_type();
    let func_id = fc
        .builder_mut()
        .get_or_declare_function(&func_name, &[ptr_ty, ptr_ty], bool_ty);

    if !fc.builder_mut().function_has_body(func_id) {
        let saved_pos = fc.builder_mut().save_position();
        let saved_func = fc.builder_mut().current_function();

        fc.builder_mut().set_ccc(func_id);
        fc.builder_mut().set_internal_linkage(func_id);
        fc.builder_mut().add_nounwind_attribute(func_id);
        let entry = fc.builder_mut().append_block(func_id, "entry");
        fc.builder_mut().position_at_end(entry);
        fc.builder_mut().set_current_function(func_id);

        let a_ptr = fc.builder_mut().get_param(func_id, 0);
        let b_ptr = fc.builder_mut().get_param(func_id, 1);

        // Load tags (i64 at offset 0)
        let i64_ty_id = fc.builder_mut().i64_type();
        let a_tag = fc.builder_mut().load(i64_ty_id, a_ptr, "a_tag");
        let b_tag = fc.builder_mut().load(i64_ty_id, b_ptr, "b_tag");
        let tags_eq = fc.builder_mut().icmp_eq(a_tag, b_tag, "tags_eq");

        // None tag = 1. Both None → true. Both Some → compare payloads. The
        // payload comparison runs ONLY on the Some path: when the payload is a
        // boxed recursive back-edge the slot holds an RC box pointer that is
        // null/invalid for None — dereferencing it unconditionally would crash.
        let one = fc.builder_mut().const_i64(1);
        let is_none = fc.builder_mut().icmp_eq(a_tag, one, "is_none");
        let some_bb = fc.builder_mut().append_block(func_id, "opt.some");
        let join_bb = fc.builder_mut().append_block(func_id, "opt.join");
        let true_val = fc.builder_mut().const_bool(true);
        fc.builder_mut().cond_br(is_none, join_bb, some_bb);

        // Some path: compare payloads via the inner eq thunk.
        fc.builder_mut().position_at_end(some_bb);
        let i8_ty_id = fc.builder_mut().i8_type();
        let offset_8 = fc.builder_mut().const_i64(8);
        let mut a_payload = fc.builder_mut().gep(i8_ty_id, a_ptr, &[offset_8], "a_pay");
        let mut b_payload = fc.builder_mut().gep(i8_ty_id, b_ptr, &[offset_8], "b_pay");
        // Boxed recursive Some payload: the slot holds the RC box pointer, not
        // the inline value. Deref once so the inner thunk receives a pointer TO
        // the payload value (the box contents), not the address of the box slot.
        if crate::codegen::type_info::repr_box_oracle::payload_type_is_rc_boxed(
            fc.type_info().pool(),
            inner_ty,
        ) {
            a_payload = fc.builder_mut().load(ptr_ty, a_payload, "a_pay.box");
            b_payload = fc.builder_mut().load(ptr_ty, b_payload, "b_pay.box");
        }
        let bool_ty_id = fc.builder_mut().bool_type();
        let payload_eq = fc
            .builder_mut()
            .call_indirect(
                bool_ty_id,
                &[ptr_ty, ptr_ty],
                inner_eq_thunk,
                &[a_payload, b_payload],
                "pay_eq",
            )
            .unwrap_or_else(|| fc.builder_mut().const_bool(false));
        let some_end_bb = fc.builder_mut().current_block().unwrap_or(some_bb);
        fc.builder_mut().br(join_bb);

        // Join: same_result = is_none ? true : payload_eq.
        fc.builder_mut().position_at_end(join_bb);
        let bool_ty_phi = fc.builder_mut().bool_type();
        let same_result = fc
            .builder_mut()
            .phi_from_incoming(
                bool_ty_phi,
                &[(true_val, entry), (payload_eq, some_end_bb)],
                "same_eq",
            )
            .unwrap_or(true_val);
        let false_val = fc.builder_mut().const_bool(false);
        let result = fc
            .builder_mut()
            .select(tags_eq, same_result, false_val, "eq");
        fc.builder_mut().ret(result);

        verify_derive_function(fc, func_id, "derive_thunk");
        fc.builder_mut().restore_position(saved_pos);
        if let Some(f) = saved_func {
            fc.builder_mut().set_current_function(f);
        }
    }

    Some(fc.builder_mut().get_function_ptr(func_id))
}
