//! Result<T, E> equality thunk generator for derived trait callbacks.

use ori_types::Idx;

use crate::codegen::derive_codegen::verify_derive_function;
use crate::codegen::function_compiler::FunctionCompiler;
use crate::codegen::value_id::{FunctionId, LLVMTypeId, ValueId};

use super::thunks::get_or_create_derive_eq_thunk;

/// Per-arm eq thunk pointers + boxing flags for the Result same-tag comparison.
#[derive(Clone, Copy)]
struct ResultThunkArms {
    ok_eq: ValueId,
    err_eq: ValueId,
    ok_boxed: bool,
    err_boxed: bool,
}

/// Build the same-tag payload-equality value for a Result eq thunk given the
/// offset-8 payload slot pointers and the `is_ok` discriminator.
///
/// When either arm's payload is a boxed recursive back-edge, the offset-8 slot
/// holds an RC box pointer valid ONLY on its own variant's path (the inactive
/// variant packs a different, possibly scalar, bit pattern into the same slot).
/// Branch on `is_ok` so each box pointer is loaded-through only on the matching
/// arm — mirroring the box-aware Option thunk's Some-guarded deref and the inline
/// `compare_result_boxed_payload` / `compare_result_arm_payload` branch. The arm
/// eq thunk receives a pointer TO the payload value: the loaded box pointer when
/// boxed, the slot pointer otherwise. When neither arm is boxed, the slot
/// pointers feed a branchless select (eq thunks are side-effect free, so
/// evaluating the inactive arm against the shared inline slot bytes is safe).
fn emit_result_thunk_same_tag_eq<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    func_id: FunctionId,
    arms: ResultThunkArms,
    a_payload: ValueId,
    b_payload: ValueId,
    is_ok: ValueId,
) -> ValueId {
    let ptr_ty = fc.builder_mut().ptr_type();
    let bool_ty_id = fc.builder_mut().bool_type();
    let false_val = fc.builder_mut().const_bool(false);

    if !arms.ok_boxed && !arms.err_boxed {
        let ok_result = fc
            .builder_mut()
            .call_indirect(
                bool_ty_id,
                &[ptr_ty, ptr_ty],
                arms.ok_eq,
                &[a_payload, b_payload],
                "ok_eq",
            )
            .unwrap_or_else(|| fc.builder_mut().const_bool(false));
        let err_result = fc
            .builder_mut()
            .call_indirect(
                bool_ty_id,
                &[ptr_ty, ptr_ty],
                arms.err_eq,
                &[a_payload, b_payload],
                "err_eq",
            )
            .unwrap_or_else(|| fc.builder_mut().const_bool(false));
        return fc
            .builder_mut()
            .select(is_ok, ok_result, err_result, "same_eq");
    }

    let ok_bb = fc.builder_mut().append_block(func_id, "res.ok");
    let err_bb = fc.builder_mut().append_block(func_id, "res.err");
    let join_bb = fc.builder_mut().append_block(func_id, "res.join");
    fc.builder_mut().cond_br(is_ok, ok_bb, err_bb);

    fc.builder_mut().position_at_end(ok_bb);
    let (a_ok_arg, b_ok_arg) =
        result_thunk_arm_args(fc, ptr_ty, arms.ok_boxed, a_payload, b_payload, "ok");
    let ok_result = fc
        .builder_mut()
        .call_indirect(
            bool_ty_id,
            &[ptr_ty, ptr_ty],
            arms.ok_eq,
            &[a_ok_arg, b_ok_arg],
            "ok_eq",
        )
        .unwrap_or_else(|| fc.builder_mut().const_bool(false));
    let ok_end_bb = fc.builder_mut().current_block().unwrap_or(ok_bb);
    fc.builder_mut().br(join_bb);

    fc.builder_mut().position_at_end(err_bb);
    let (a_err_arg, b_err_arg) =
        result_thunk_arm_args(fc, ptr_ty, arms.err_boxed, a_payload, b_payload, "err");
    let err_result = fc
        .builder_mut()
        .call_indirect(
            bool_ty_id,
            &[ptr_ty, ptr_ty],
            arms.err_eq,
            &[a_err_arg, b_err_arg],
            "err_eq",
        )
        .unwrap_or_else(|| fc.builder_mut().const_bool(false));
    let err_end_bb = fc.builder_mut().current_block().unwrap_or(err_bb);
    fc.builder_mut().br(join_bb);

    fc.builder_mut().position_at_end(join_bb);
    fc.builder_mut()
        .phi_from_incoming(
            bool_ty_id,
            &[(ok_result, ok_end_bb), (err_result, err_end_bb)],
            "same_eq",
        )
        .unwrap_or(false_val)
}

/// Resolve the (a, b) pointer arguments for one Result arm's eq thunk. When the
/// arm's payload is boxed, load the box pointer out of each offset-8 slot so the
/// thunk receives a pointer to the box contents; otherwise pass the slot pointer.
fn result_thunk_arm_args<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    ptr_ty: LLVMTypeId,
    boxed: bool,
    a_payload: ValueId,
    b_payload: ValueId,
    arm: &str,
) -> (ValueId, ValueId) {
    if boxed {
        let a_box = fc
            .builder_mut()
            .load(ptr_ty, a_payload, &format!("a_{arm}.box"));
        let b_box = fc
            .builder_mut()
            .load(ptr_ty, b_payload, &format!("b_{arm}.box"));
        (a_box, b_box)
    } else {
        (a_payload, b_payload)
    }
}

/// Generate a thunk `(ptr, ptr) -> bool` for Result<T, E> equality.
///
/// Loads the tag (i64 at offset 0) from each pointer, compares tags,
/// then if same variant, compares payloads using Ok or Err eq thunk.
pub(super) fn get_or_create_result_eq_thunk<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    ty: Idx,
    ok_ty: Idx,
    err_ty: Idx,
) -> Option<ValueId> {
    let ok_info = fc.type_info().get(ok_ty);
    let err_info = fc.type_info().get(err_ty);
    let ok_eq = get_or_create_derive_eq_thunk(fc, ok_ty, &ok_info)?;
    let err_eq = get_or_create_derive_eq_thunk(fc, err_ty, &err_info)?;

    let ok_boxed = crate::codegen::type_info::repr_box_oracle::payload_type_is_rc_boxed(
        fc.type_info().pool(),
        ok_ty,
    );
    let err_boxed = crate::codegen::type_info::repr_box_oracle::payload_type_is_rc_boxed(
        fc.type_info().pool(),
        err_ty,
    );

    let func_name = format!("_ori_eq_result_{}", ty.raw());
    let ptr_ty = fc.builder_mut().ptr_type();
    let bool_ty = fc.builder_mut().bool_type();
    let func_id = fc
        .builder_mut()
        .get_or_declare_function(&func_name, &[ptr_ty, ptr_ty], bool_ty);

    if !fc.builder_mut().function_has_body(func_id) {
        let saved_pos = fc.builder_mut().save_position();
        let saved_func = fc.builder_mut().current_function();

        fc.builder_mut().set_ccc(func_id);
        fc.builder_mut().add_nounwind_attribute(func_id);
        let entry = fc.builder_mut().append_block(func_id, "entry");
        fc.builder_mut().position_at_end(entry);
        fc.builder_mut().set_current_function(func_id);

        let a_ptr = fc.builder_mut().get_param(func_id, 0);
        let b_ptr = fc.builder_mut().get_param(func_id, 1);

        // Load tags
        let i64_ty_id = fc.builder_mut().i64_type();
        let a_tag = fc.builder_mut().load(i64_ty_id, a_ptr, "a_tag");
        let b_tag = fc.builder_mut().load(i64_ty_id, b_ptr, "b_tag");
        let tags_eq = fc.builder_mut().icmp_eq(a_tag, b_tag, "tags_eq");

        // Payload pointers at byte offset 8
        let i8_ty_id = fc.builder_mut().i8_type();
        let offset_8 = fc.builder_mut().const_i64(8);
        let a_payload = fc.builder_mut().gep(i8_ty_id, a_ptr, &[offset_8], "a_pay");
        let b_payload = fc.builder_mut().gep(i8_ty_id, b_ptr, &[offset_8], "b_pay");

        // Ok tag = 0, Err tag = 1
        let zero = fc.builder_mut().const_i64(0);
        let is_ok = fc.builder_mut().icmp_eq(a_tag, zero, "is_ok");
        let same_eq = emit_result_thunk_same_tag_eq(
            fc,
            func_id,
            ResultThunkArms {
                ok_eq,
                err_eq,
                ok_boxed,
                err_boxed,
            },
            a_payload,
            b_payload,
            is_ok,
        );

        let false_val = fc.builder_mut().const_bool(false);
        let result = fc.builder_mut().select(tags_eq, same_eq, false_val, "eq");
        fc.builder_mut().ret(result);

        verify_derive_function(fc, func_id, "derive_thunk");
        fc.builder_mut().restore_position(saved_pos);
        if let Some(f) = saved_func {
            fc.builder_mut().set_current_function(f);
        }
    }

    Some(fc.builder_mut().get_function_ptr(func_id))
}
