//! Clone RC increment helpers for derived `Clone` codegen.
//!
//! Extracted from `bodies.rs` to keep files under the 500-line limit.
//! These functions emit LLVM IR that RC-increments heap-allocated fields
//! during derived clone operations, handling str, list/set, map, closures,
//! Result, and composite types (struct/tuple/option).

use ori_types::{Idx, Tag};

use super::super::function_compiler::FunctionCompiler;
use super::super::{FunctionId, ValueId};

/// Emit RC increment for a single field value based on its type tag.
///
/// `func_id` is the LLVM function being compiled (needed for creating blocks).
pub(super) fn emit_clone_field_rc_inc<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    func_id: FunctionId,
    field_val: ValueId,
    tag: Tag,
    resolved: Idx,
    field_idx: usize,
) {
    match tag {
        Tag::Str => emit_clone_rc_inc_str(fc, func_id, field_val, field_idx),
        Tag::List | Tag::Set => emit_clone_rc_inc_list(fc, field_val, field_idx),
        Tag::Map => emit_clone_rc_inc_data_ptr(fc, field_val, field_idx, "map"),
        Tag::Function => emit_clone_rc_inc_closure(fc, func_id, field_val, field_idx),
        Tag::Struct | Tag::Tuple | Tag::Option => {
            emit_clone_composite_rc_inc(fc, func_id, field_val, tag, resolved, field_idx);
        }
        Tag::Result => {
            emit_clone_result_rc_inc(fc, func_id, field_val, resolved, field_idx);
        }
        _ => {} // Scalars: no RC needed
    }
}

/// Emit slice-aware `ori_str_rc_inc(data, cap)` on a str field.
///
/// Delegates SSO, null, and seamless-slice handling to the runtime function
/// (symmetric to `ori_str_rc_dec`). This correctly handles slice strings
/// from `str.split()` by reading the cap field for `SLICE_FLAG`.
fn emit_clone_rc_inc_str<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    _func_id: FunctionId,
    field_val: ValueId,
    field_idx: usize,
) {
    let Some(data_ptr) =
        fc.builder_mut()
            .extract_value(field_val, 2, &format!("clone.str.data.{field_idx}"))
    else {
        return;
    };
    let cap = fc
        .builder_mut()
        .extract_value(field_val, 1, &format!("clone.str.cap.{field_idx}"))
        .unwrap_or_else(|| fc.builder_mut().const_i64(0));

    let str_rc_inc_fn = fc.builder_mut().runtime_fn("ori_str_rc_inc");
    fc.builder_mut().call(
        str_rc_inc_fn,
        &[data_ptr, cap],
        &format!("clone.str.inc.{field_idx}"),
    );
}

/// Emit `ori_list_rc_inc(data, cap)` for a list/set field.
fn emit_clone_rc_inc_list<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    field_val: ValueId,
    field_idx: usize,
) {
    let Some(data_ptr) =
        fc.builder_mut()
            .extract_value(field_val, 2, &format!("clone.list.data.{field_idx}"))
    else {
        return;
    };
    let cap = fc
        .builder_mut()
        .extract_value(field_val, 1, &format!("clone.list.cap.{field_idx}"))
        .unwrap_or_else(|| fc.builder_mut().const_i64(0));
    let rc_inc_fn = fc.builder_mut().runtime_fn("ori_list_rc_inc");
    fc.builder_mut().call(
        rc_inc_fn,
        &[data_ptr, cap],
        &format!("clone.list.inc.{field_idx}"),
    );
}

/// Emit `ori_rc_inc` on a data pointer at field 2 (map or similar).
fn emit_clone_rc_inc_data_ptr<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    field_val: ValueId,
    field_idx: usize,
    label: &str,
) {
    let Some(data_ptr) =
        fc.builder_mut()
            .extract_value(field_val, 2, &format!("clone.{label}.data.{field_idx}"))
    else {
        return;
    };
    let rc_inc_fn = fc.builder_mut().runtime_fn("ori_rc_inc");
    fc.builder_mut().call(
        rc_inc_fn,
        &[data_ptr],
        &format!("clone.{label}.inc.{field_idx}"),
    );
}

/// Emit null-checked `ori_rc_inc` on a closure env pointer (field 1).
fn emit_clone_rc_inc_closure<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    func_id: FunctionId,
    field_val: ValueId,
    field_idx: usize,
) {
    let Some(env_ptr) =
        fc.builder_mut()
            .extract_value(field_val, 1, &format!("clone.closure.env.{field_idx}"))
    else {
        return;
    };

    if fc.builder_mut().is_const_null_ptr(env_ptr) {
        return;
    }

    let do_inc = fc
        .builder_mut()
        .append_block(func_id, &format!("clone.env.inc.{field_idx}"));
    let skip = fc
        .builder_mut()
        .append_block(func_id, &format!("clone.env.skip.{field_idx}"));

    let i64_ty = fc.builder_mut().i64_type();
    let p2i = fc
        .builder_mut()
        .ptr_to_int(env_ptr, i64_ty, &format!("clone.env.p2i.{field_idx}"));
    let zero = fc.builder_mut().const_i64(0);
    let is_null = fc
        .builder_mut()
        .icmp_eq(p2i, zero, &format!("clone.env.null.{field_idx}"));
    fc.builder_mut().cond_br(is_null, skip, do_inc);

    fc.builder_mut().position_at_end(do_inc);
    let rc_inc_fn = fc.builder_mut().runtime_fn("ori_rc_inc");
    fc.builder_mut().call(
        rc_inc_fn,
        &[env_ptr],
        &format!("clone.env.call.{field_idx}"),
    );
    fc.builder_mut().br(skip);

    fc.builder_mut().position_at_end(skip);
}

/// Check if a type tag represents an RC-tracked type that needs clone increment.
fn tag_needs_clone_rc(tag: Tag) -> bool {
    matches!(
        tag,
        Tag::Str
            | Tag::List
            | Tag::Set
            | Tag::Map
            | Tag::Function
            | Tag::Struct
            | Tag::Tuple
            | Tag::Option
            | Tag::Result
    )
}

/// Emit tag-conditional RC increment for a `Result<T, E>` field.
///
/// Result has two variants (Ok/Err) with potentially different payload types.
/// Unlike Option (one inner type, unconditional RC-inc), Result must branch
/// on the tag to RC-inc the correct variant's payload.
///
/// Uses alloca + GEP to safely load the payload with each variant's LLVM type,
/// since Ok and Err may have different struct layouts.
fn emit_clone_result_rc_inc<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    func_id: FunctionId,
    field_val: ValueId,
    resolved: Idx,
    field_idx: usize,
) {
    let pool = fc.type_info().pool();
    let ok_ty_idx = pool.result_ok(resolved);
    let err_ty_idx = pool.result_err(resolved);
    let ok_resolved = pool.resolve_fully(ok_ty_idx);
    let err_resolved = pool.resolve_fully(err_ty_idx);
    let ok_tag = pool.tag(ok_resolved);
    let err_tag = pool.tag(err_resolved);

    let ok_needs_rc = tag_needs_clone_rc(ok_tag);
    let err_needs_rc = tag_needs_clone_rc(err_tag);

    if !ok_needs_rc && !err_needs_rc {
        return; // Both variants are scalar — no RC needed
    }

    // Resolve LLVM types for Result struct and each variant payload
    let result_inkwell_ty = fc.resolve_type(resolved);
    let result_llvm_ty = fc.builder_mut().register_type(result_inkwell_ty);

    // Extract tag (field 0) — 0 = Ok, 1 = Err
    let Some(tag_val) =
        fc.builder_mut()
            .extract_value(field_val, 0, &format!("clone.result.tag.{field_idx}"))
    else {
        return;
    };

    // Store the Result value to an alloca for type-safe payload extraction.
    // GEP to field 1 gives a pointer to the payload bytes, which we can load
    // as either Ok or Err's LLVM type (safe with opaque pointers).
    let alloca = fc.builder_mut().create_entry_alloca(
        func_id,
        &format!("clone.result.tmp.{field_idx}"),
        result_llvm_ty,
    );
    fc.builder_mut().store(field_val, alloca);
    let payload_ptr = fc.builder_mut().struct_gep(
        result_llvm_ty,
        alloca,
        1,
        &format!("clone.result.payload.{field_idx}"),
    );

    let zero = fc.builder_mut().const_i64(0);
    let is_ok = fc
        .builder_mut()
        .icmp_eq(tag_val, zero, &format!("clone.result.is_ok.{field_idx}"));

    let ok_block = fc
        .builder_mut()
        .append_block(func_id, &format!("clone.result.ok.{field_idx}"));
    let err_block = fc
        .builder_mut()
        .append_block(func_id, &format!("clone.result.err.{field_idx}"));
    let merge_block = fc
        .builder_mut()
        .append_block(func_id, &format!("clone.result.merge.{field_idx}"));

    fc.builder_mut().cond_br(is_ok, ok_block, err_block);

    // Ok branch
    fc.builder_mut().position_at_end(ok_block);
    if ok_needs_rc {
        let ok_inkwell_ty = fc.resolve_type(ok_resolved);
        let ok_llvm_ty = fc.builder_mut().register_type(ok_inkwell_ty);
        let ok_val = fc.builder_mut().load(
            ok_llvm_ty,
            payload_ptr,
            &format!("clone.result.ok_val.{field_idx}"),
        );
        emit_clone_field_rc_inc(
            fc,
            func_id,
            ok_val,
            ok_tag,
            ok_resolved,
            field_idx * 100 + 1000,
        );
    }
    fc.builder_mut().br(merge_block);

    // Err branch
    fc.builder_mut().position_at_end(err_block);
    if err_needs_rc {
        let err_inkwell_ty = fc.resolve_type(err_resolved);
        let err_llvm_ty = fc.builder_mut().register_type(err_inkwell_ty);
        let err_val = fc.builder_mut().load(
            err_llvm_ty,
            payload_ptr,
            &format!("clone.result.err_val.{field_idx}"),
        );
        emit_clone_field_rc_inc(
            fc,
            func_id,
            err_val,
            err_tag,
            err_resolved,
            field_idx * 100 + 2000,
        );
    }
    fc.builder_mut().br(merge_block);

    fc.builder_mut().position_at_end(merge_block);
}

/// Recurse into composite types (struct/tuple/option) for clone RC increments.
fn emit_clone_composite_rc_inc<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    func_id: FunctionId,
    field_val: ValueId,
    tag: Tag,
    resolved: Idx,
    field_idx: usize,
) {
    let pool = fc.type_info().pool();
    let sub_fields: Vec<(u32, Idx)> = match tag {
        Tag::Struct => {
            let fields = pool.struct_fields(resolved);
            #[expect(
                clippy::cast_possible_truncation,
                reason = "field count bounded by struct definition"
            )]
            fields
                .into_iter()
                .enumerate()
                .map(|(i, (_, ty))| (i as u32, ty))
                .collect()
        }
        Tag::Tuple => {
            let elems = pool.tuple_elems(resolved);
            #[expect(
                clippy::cast_possible_truncation,
                reason = "element count bounded by tuple arity"
            )]
            elems
                .into_iter()
                .enumerate()
                .map(|(i, ty)| (i as u32, ty))
                .collect()
        }
        Tag::Option => {
            let inner = pool.option_inner(resolved);
            vec![(1, inner)]
        }
        _ => return,
    };

    for (idx, inner_ty) in sub_fields {
        let pool = fc.type_info().pool();
        let inner_resolved = pool.resolve_fully(inner_ty);
        let inner_tag = pool.tag(inner_resolved);
        if !matches!(
            inner_tag,
            Tag::Str
                | Tag::List
                | Tag::Set
                | Tag::Map
                | Tag::Function
                | Tag::Struct
                | Tag::Tuple
                | Tag::Option
                | Tag::Result
        ) {
            continue;
        }
        // Remap declaration-order index to memory order.
        let mem_idx = fc
            .repr_plan()
            .and_then(|plan| {
                let repr = plan.get_repr(resolved)?;
                let fields = match repr {
                    ori_repr::MachineRepr::Struct(s) => &s.fields[..],
                    ori_repr::MachineRepr::Tuple(t) => &t.elements[..],
                    _ => return None,
                };
                fields
                    .iter()
                    .position(|f| f.original_index == idx)
                    .map(|p| p as u32)
            })
            .unwrap_or(idx);
        let inner_fv = fc.builder_mut().extract_value(
            field_val,
            mem_idx,
            &format!("clone.sub.{field_idx}.{idx}"),
        );
        if let Some(ifv) = inner_fv {
            emit_clone_field_rc_inc(
                fc,
                func_id,
                ifv,
                inner_tag,
                inner_resolved,
                field_idx * 100 + idx as usize,
            );
        }
    }
}
