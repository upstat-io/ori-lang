//! Compound-aggregate `debug`/`to_str` LLVM emission: Option, Result, List,
//! Tuple.

use super::{super::ArcIrEmitter, RenderStyle};
use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;
use ori_ir::{FIELD_DATA, FIELD_LEN};
use ori_types::Idx;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit `Option.debug()` / `Option.to_str()` with branching.
    ///
    /// - None -> "None" literal
    /// - Some(v) -> "Some(" + `v.debug()` or `v.to_str()` + ")"
    #[must_use = "the absence of a value must be handled"]
    pub(super) fn emit_option_debug_branch(
        &mut self,
        is_some: ValueId,
        payload: ValueId,
        inner_ty: Idx,
        style: RenderStyle,
    ) -> Option<ValueId> {
        let some_bb = self.builder.append_block(self.current_function, "dbg.some");
        let none_bb = self.builder.append_block(self.current_function, "dbg.none");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "dbg.merge");

        self.builder.cond_br(is_some, some_bb, none_bb);

        self.builder.position_at_end(none_bb);
        let none_str = self.emit_literal_ori_str("None")?;
        let none_bb_current = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        self.builder.position_at_end(some_bb);
        let payload = if crate::codegen::type_info::repr_box_oracle::payload_type_is_rc_boxed(
            self.pool, inner_ty,
        ) {
            let inner_llvm_ty = self.resolve_type(inner_ty);
            self.builder
                .load(inner_llvm_ty, payload, "dbg.opt.payload.boxed")
        } else {
            payload
        };

        let inner_str = if style.is_debug() {
            self.emit_element_debug(payload, inner_ty)?
        } else {
            self.emit_element_to_str(payload, inner_ty)?
        };
        let inner_str_is_borrowed = style == RenderStyle::Printable
            && matches!(self.type_info.get(inner_ty), TypeInfo::Str);
        let prefix = self.emit_literal_ori_str("Some(")?;
        let suffix = self.emit_literal_ori_str(")")?;
        let tmp = self.emit_str_concat(prefix, inner_str)?;
        if !inner_str_is_borrowed {
            self.dec_intermediate_str(inner_str);
        }
        let some_str = self.emit_str_concat(tmp, suffix)?;
        self.dec_intermediate_str(tmp);
        let some_bb_current = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        self.builder.position_at_end(merge_bb);
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let phi = self.builder.phi(str_ty, "dbg.result");
        self.builder.add_phi_incoming(
            phi,
            &[(none_str, none_bb_current), (some_str, some_bb_current)],
        );
        Some(phi)
    }

    /// Render the active `Result` payload in Debug or Printable style.
    ///
    /// Payload extraction stays inside its branch because inactive variant
    /// storage may contain invalid pointers.
    #[must_use = "the absence of a value must be handled"]
    pub(super) fn emit_result_debug(
        &mut self,
        arg_vals: &[ValueId],
        receiver_ty: Idx,
        style: RenderStyle,
    ) -> Option<ValueId> {
        let receiver = arg_vals[0];
        let TypeInfo::Result {
            ok: ok_ty,
            err: err_ty,
        } = self.type_info.get(receiver_ty)
        else {
            return None;
        };
        self.emit_nested_result_render(receiver, receiver_ty, ok_ty, err_ty, style)
    }

    /// Render a nested `Result` from pre-resolved payload types.
    ///
    /// Payload extraction stays inside its branch because inactive variant
    /// storage may contain invalid pointers.
    #[must_use = "the absence of a value must be handled"]
    pub(super) fn emit_nested_result_render(
        &mut self,
        receiver: ValueId,
        receiver_ty: Idx,
        ok_ty: Idx,
        err_ty: Idx,
        style: RenderStyle,
    ) -> Option<ValueId> {
        let tag = self.builder.extract_value(receiver, 0, "rdbg.n.tag")?;
        let ok_const = self
            .builder
            .const_int_matching(tag, ori_ir::RESULT_TAG_OK as u64);
        let is_ok = self.builder.icmp_eq(tag, ok_const, "rdbg.n.is_ok");

        let ok_bb = self
            .builder
            .append_block(self.current_function, "rdbg.n.ok");

        let err_bb = self
            .builder
            .append_block(self.current_function, "rdbg.n.err");

        let merge_bb = self
            .builder
            .append_block(self.current_function, "rdbg.n.merge");

        self.builder.cond_br(is_ok, ok_bb, err_bb);

        self.builder.position_at_end(ok_bb);
        let ok_payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, ok_ty)?;
        let ok_str = if style.is_debug() {
            self.emit_element_debug(ok_payload, ok_ty)?
        } else {
            self.emit_element_to_str(ok_payload, ok_ty)?
        };
        let ok_prefix = self.emit_literal_ori_str("Ok(")?;
        let ok_suffix = self.emit_literal_ori_str(")")?;
        let ok_tmp = self.emit_str_concat(ok_prefix, ok_str)?;
        let ok_result = self.emit_str_concat(ok_tmp, ok_suffix)?;
        self.dec_intermediate_str(ok_tmp);
        let ok_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        self.builder.position_at_end(err_bb);
        let err_payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, err_ty)?;
        let err_str = if style.is_debug() {
            self.emit_element_debug(err_payload, err_ty)?
        } else {
            self.emit_element_to_str(err_payload, err_ty)?
        };
        let err_prefix = self.emit_literal_ori_str("Err(")?;
        let err_suffix = self.emit_literal_ori_str(")")?;
        let err_tmp = self.emit_str_concat(err_prefix, err_str)?;
        let err_result = self.emit_str_concat(err_tmp, err_suffix)?;
        self.dec_intermediate_str(err_tmp);
        let err_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        self.builder.position_at_end(merge_bb);
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let phi = self.builder.phi(str_ty, "rdbg.n.result");
        self.builder
            .add_phi_incoming(phi, &[(ok_result, ok_bb_final), (err_result, err_bb_final)]);
        Some(phi)
    }

    /// Emit `[T].debug()` / `[T].to_str()` -- element-wise loop producing
    /// `"[e1, e2, ...]"`. `style` selects Debug vs Printable element render;
    /// the bracket/separator literals are identical for both.
    ///
    /// Layout: `{i64 len, i64 cap, ptr data}`.
    /// For empty lists, returns `"[]"` immediately.
    #[must_use = "the absence of a value must be handled"]
    pub(super) fn emit_list_debug(
        &mut self,
        list: ValueId,
        list_ty: Idx,
        elem_ty: Idx,
        style: RenderStyle,
    ) -> Option<ValueId> {
        let len = self.builder.extract_value(list, FIELD_LEN, "ldbg.len")?;
        let data = self.builder.extract_value(list, FIELD_DATA, "ldbg.data")?;

        let zero = self.builder.const_i64(0);
        let one = self.builder.const_i64(1);
        let is_empty = self.builder.icmp_eq(len, zero, "ldbg.empty");

        let func = self.current_function;
        let str_ty = self.resolve_type(ori_types::Idx::STR);

        let empty_bb = self.builder.append_block(func, "ldbg.empty");
        let first_bb = self.builder.append_block(func, "ldbg.first");
        let loop_hdr = self.builder.append_block(func, "ldbg.hdr");
        let loop_body = self.builder.append_block(func, "ldbg.body");
        let close_bb = self.builder.append_block(func, "ldbg.close");
        let done_bb = self.builder.append_block(func, "ldbg.done");

        self.builder.cond_br(is_empty, empty_bb, first_bb);

        self.builder.position_at_end(empty_bb);
        let empty_str = self.emit_literal_ori_str("[]")?;
        let empty_bb_end = self.builder.current_block().unwrap();
        self.builder.br(done_bb);

        self.builder.position_at_end(first_bb);
        let open = self.emit_literal_ori_str("[")?;
        let elem_llvm_ty = self.int_element_llvm_type(list_ty, elem_ty);
        let ptr0 = self.builder.gep(elem_llvm_ty, data, &[zero], "ldbg.ep0");
        let elem0 = self.builder.load(elem_llvm_ty, ptr0, "ldbg.e0");
        let elem0 = self.sext_narrowed_int_element(elem0, list_ty, elem_ty, "ldbg.e0.sext");
        let elem0_str = if style.is_debug() {
            self.emit_element_debug(elem0, elem_ty)?
        } else {
            self.emit_element_to_str(elem0, elem_ty)?
        };
        let acc_init = self.emit_str_concat(open, elem0_str)?;
        self.dec_intermediate_str(elem0_str);
        let needs_loop = self.builder.icmp_sgt(len, one, "ldbg.needs_loop");
        let first_bb_end = self.builder.current_block().unwrap();
        self.builder.cond_br(needs_loop, loop_hdr, close_bb);

        self.builder.position_at_end(loop_hdr);
        let i64_ty = self
            .builder
            .register_type(self.builder.scx().type_i64().into());
        let idx_phi = self.builder.phi(i64_ty, "ldbg.idx");
        let acc_phi = self.builder.phi(str_ty, "ldbg.acc");
        let has_more = self.builder.icmp_slt(idx_phi, len, "ldbg.more");
        self.builder.cond_br(has_more, loop_body, close_bb);

        self.builder.position_at_end(loop_body);
        let sep = self.emit_literal_ori_str(", ")?;
        let with_sep = self.emit_str_concat(acc_phi, sep)?;
        self.dec_intermediate_str(acc_phi);
        let ptr_i = self.builder.gep(elem_llvm_ty, data, &[idx_phi], "ldbg.epi");
        let elem_i = self.builder.load(elem_llvm_ty, ptr_i, "ldbg.ei");
        let elem_i = self.sext_narrowed_int_element(elem_i, list_ty, elem_ty, "ldbg.ei.sext");
        let elem_i_str = if style.is_debug() {
            self.emit_element_debug(elem_i, elem_ty)?
        } else {
            self.emit_element_to_str(elem_i, elem_ty)?
        };
        let new_acc = self.emit_str_concat(with_sep, elem_i_str)?;
        self.dec_intermediate_str(with_sep);
        self.dec_intermediate_str(elem_i_str);
        let next_idx = self.builder.add(idx_phi, one, "ldbg.next");
        let body_end = self.builder.current_block().unwrap();
        self.builder.br(loop_hdr);

        self.builder
            .add_phi_incoming(idx_phi, &[(one, first_bb_end), (next_idx, body_end)]);
        self.builder
            .add_phi_incoming(acc_phi, &[(acc_init, first_bb_end), (new_acc, body_end)]);

        self.builder.position_at_end(close_bb);
        let close_acc = self.builder.phi(str_ty, "ldbg.close.acc");
        self.builder
            .add_phi_incoming(close_acc, &[(acc_init, first_bb_end), (acc_phi, loop_hdr)]);
        let suffix = self.emit_literal_ori_str("]")?;
        let result = self.emit_str_concat(close_acc, suffix)?;
        self.dec_intermediate_str(close_acc);
        let close_bb_end = self.builder.current_block().unwrap();
        self.builder.br(done_bb);

        self.builder.position_at_end(done_bb);
        let final_phi = self.builder.phi(str_ty, "ldbg.final");
        self.builder.add_phi_incoming(
            final_phi,
            &[(empty_str, empty_bb_end), (result, close_bb_end)],
        );
        Some(final_phi)
    }

    /// Emit `(A, B, ...).debug()` / `.to_str()` -- field-wise formatting as
    /// `"(a, b, ...)"`. `style` selects Debug vs Printable per-field render;
    /// the parens/separator literals are identical for both.
    #[must_use = "the absence of a value must be handled"]
    pub(super) fn emit_tuple_debug(
        &mut self,
        tuple: ValueId,
        elements: &[Idx],
        tuple_ty: Idx,
        style: RenderStyle,
    ) -> Option<ValueId> {
        if elements.is_empty() {
            return self.emit_literal_ori_str("()");
        }

        let mut acc = self.emit_literal_ori_str("(")?;
        for (i, &elem_ty) in elements.iter().enumerate() {
            let field_index = u32::try_from(i).ok()?;
            if i > 0 {
                let sep = self.emit_literal_ori_str(", ")?;
                let new_acc = self.emit_str_concat(acc, sep)?;
                self.dec_intermediate_str(acc);
                acc = new_acc;
            }
            let memory_field = self.remap_struct_field(tuple_ty, field_index);
            let field = self
                .builder
                .extract_value(tuple, memory_field, &format!("tdbg.f{i}"))?;

            let field_str = if style.is_debug() {
                self.emit_element_debug(field, elem_ty)?
            } else {
                self.emit_element_to_str(field, elem_ty)?
            };
            let new_acc = self.emit_str_concat(acc, field_str)?;
            self.dec_intermediate_str(acc);
            self.dec_intermediate_str(field_str);
            acc = new_acc;
        }
        let suffix = self.emit_literal_ori_str(")")?;
        let result = self.emit_str_concat(acc, suffix)?;
        self.dec_intermediate_str(acc);
        Some(result)
    }
}
