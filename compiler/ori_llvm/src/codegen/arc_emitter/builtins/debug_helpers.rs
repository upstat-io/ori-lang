//! Debug formatting helpers for Option/Result/List LLVM codegen.
//!
//! Extracted from `option_result_helpers.rs` to keep files under the 500-line
//! limit. Contains all `debug`/`to_str` emission: element formatting, wrapper
//! branching, list loops, and string literal/concat utilities.

use ori_ir::{FIELD_DATA, FIELD_LEN};
use ori_types::Idx;

use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Create an `OriStr` from a string literal via `ori_str_from_raw`.
    pub(super) fn emit_literal_ori_str(&mut self, text: &str) -> Option<ValueId> {
        let ptr = self.builder.build_global_string_ptr(text, "lit.str");
        let len = self.builder.const_i64(text.len() as i64);
        let func_id = self.builder.runtime_fn("ori_str_from_raw");
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        self.builder
            .call_with_sret(func_id, &[ptr, len], str_ty, "lit")
    }

    /// Concatenate two `OriStr` values via `ori_str_concat`.
    pub(super) fn emit_str_concat(&mut self, a: ValueId, b: ValueId) -> Option<ValueId> {
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let a_ptr = self
            .builder
            .create_entry_alloca(self.current_function, "cat.a", str_ty);
        self.builder.store(a, a_ptr);
        let b_ptr = self
            .builder
            .create_entry_alloca(self.current_function, "cat.b", str_ty);
        self.builder.store(b, b_ptr);
        let func_id = self.builder.runtime_fn("ori_str_concat");
        self.builder
            .call_with_sret(func_id, &[a_ptr, b_ptr], str_ty, "cat")
    }

    /// Dec an intermediate `OriStr` value to prevent leaks in debug loops.
    ///
    /// Extracts `{len, cap, data}` fields and calls `ori_str_rc_dec`.
    /// SSO strings (cap encodes SSO flag) are no-ops in the runtime.
    fn dec_intermediate_str(&mut self, str_val: ValueId) {
        let data = self.builder.extract_value(str_val, 2, "dbg.dec.data");
        let cap = self.builder.extract_value(str_val, 1, "dbg.dec.cap");
        if let (Some(dp), Some(cp)) = (data, cap) {
            let null_fn = self.builder.const_null_ptr();
            self.call_str_rc_dec(dp, cp, null_fn);
        }
    }

    /// Emit `to_str` for an element of any supported type.
    ///
    /// Handles primitives (int, float, bool, Duration, Size) and str.
    /// Returns None for complex types (structs, enums) -- caller should
    /// fall through to the unresolved function path.
    pub(super) fn emit_element_to_str(&mut self, val: ValueId, ty: Idx) -> Option<ValueId> {
        let type_info = self.type_info.get(ty);
        match &type_info {
            TypeInfo::Int
            | TypeInfo::Duration
            | TypeInfo::Size
            | TypeInfo::Float
            | TypeInfo::Bool => self.emit_to_str(val, &type_info),
            TypeInfo::Str => Some(val),
            _ => None,
        }
    }

    /// Emit Debug formatting for an element of any type.
    ///
    /// Unlike `emit_element_to_str` (Printable semantics), this applies
    /// Debug semantics: strings are quoted+escaped, chars are single-quoted,
    /// and compound types (Option, Result, List) are recursively formatted.
    ///
    /// Falls back to `emit_to_str` for types where Debug == Printable
    /// (int, float, bool, Duration, Size), and returns a placeholder for
    /// truly unsupported types (structs, maps, sets, closures).
    pub(super) fn emit_element_debug(&mut self, val: ValueId, ty: Idx) -> Option<ValueId> {
        let type_info = self.type_info.get(ty);
        match &type_info {
            // Primitives: Debug == Printable
            TypeInfo::Int
            | TypeInfo::Duration
            | TypeInfo::Size
            | TypeInfo::Float
            | TypeInfo::Bool => self.emit_to_str(val, &type_info),

            // Str: Debug wraps with quotes and escapes special chars
            TypeInfo::Str => {
                let str_ty = self.resolve_type(ori_types::Idx::STR);
                let s_ptr =
                    self.builder
                        .create_entry_alloca(self.current_function, "dbg.str", str_ty);
                self.builder.store(val, s_ptr);
                let func_id = self.builder.runtime_fn("ori_str_debug_format");
                self.builder
                    .call_with_sret(func_id, &[s_ptr], str_ty, "dbg.str.fmt")
            }

            // Char: Debug wraps with single quotes and escapes
            TypeInfo::Char => {
                let str_ty = self.resolve_type(ori_types::Idx::STR);
                let func_id = self.builder.runtime_fn("ori_char_debug_format");
                self.builder
                    .call_with_sret(func_id, &[val], str_ty, "dbg.char.fmt")
            }

            // Byte: Debug uses decimal (matching current evaluator/spec behavior
            // where bytes are stored as Value::Int and debug via int path).
            TypeInfo::Byte => {
                let i64_ty = self
                    .builder
                    .register_type(self.builder.scx().type_i64().into());
                let as_i64 = self.builder.sext(val, i64_ty, "dbg.byte.sext");
                self.emit_to_str(as_i64, &TypeInfo::Int)
            }

            // Option: recursive Debug
            TypeInfo::Option { inner } => {
                let inner = *inner;
                let tag = self.builder.extract_value(val, 0, "dbg.opt.tag")?;
                let some = self
                    .builder
                    .const_int_matching(tag, ori_ir::OPTION_TAG_SOME as u64);
                let is_some = self.builder.icmp_eq(tag, some, "dbg.opt.is_some");
                let payload = self.builder.extract_value(val, 1, "dbg.opt.payload")?;
                self.emit_option_debug_branch(is_some, payload, inner, true)
            }

            // Result: recursive Debug
            TypeInfo::Result {
                ok: ok_ty,
                err: err_ty,
            } => {
                let ok_ty = *ok_ty;
                let err_ty = *err_ty;
                self.emit_nested_result_debug(val, ty, ok_ty, err_ty)
            }

            // List: element-wise Debug loop
            TypeInfo::List { element } => {
                let element = *element;
                self.emit_list_debug(val, element)
            }

            // Tuple: field-wise Debug
            TypeInfo::Tuple { elements } => {
                let elements = elements.clone();
                self.emit_tuple_debug(val, &elements)
            }

            // Generic dispatch: look up the type's compiled .debug() method.
            // Handles user structs/enums with #derive(Debug), and any other
            // type whose debug method was compiled and registered.
            _ => self
                .emit_derived_debug_call(val, ty)
                .or_else(|| self.emit_literal_ori_str("<?>")),
        }
    }

    /// Emit `Option.debug()` / `Option.to_str()` with branching.
    ///
    /// - None -> "None" literal
    /// - Some(v) -> "Some(" + `v.debug()` or `v.to_str()` + ")"
    pub(super) fn emit_option_debug_branch(
        &mut self,
        is_some: ValueId,
        payload: ValueId,
        inner_ty: Idx,
        is_debug: bool,
    ) -> Option<ValueId> {
        let inner_str = if is_debug {
            self.emit_element_debug(payload, inner_ty)?
        } else {
            self.emit_element_to_str(payload, inner_ty)?
        };

        let some_bb = self.builder.append_block(self.current_function, "dbg.some");
        let none_bb = self.builder.append_block(self.current_function, "dbg.none");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "dbg.merge");

        self.builder.cond_br(is_some, some_bb, none_bb);

        // None block: produce "None"
        self.builder.position_at_end(none_bb);
        let none_str = self.emit_literal_ori_str("None")?;
        let none_bb_current = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        // Some block: produce "Some(" + inner_str + ")"
        self.builder.position_at_end(some_bb);
        let prefix = self.emit_literal_ori_str("Some(")?;
        let suffix = self.emit_literal_ori_str(")")?;
        let tmp = self.emit_str_concat(prefix, inner_str)?;
        let some_str = self.emit_str_concat(tmp, suffix)?;
        self.dec_intermediate_str(tmp);
        let some_bb_current = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        // Merge block: phi
        self.builder.position_at_end(merge_bb);
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let phi = self.builder.phi(str_ty, "dbg.result");
        self.builder.add_phi_incoming(
            phi,
            &[(none_str, none_bb_current), (some_str, some_bb_current)],
        );
        Some(phi)
    }

    /// Emit `Result.debug()`.
    ///
    /// - `Ok(v)` -> `"Ok(" + v.debug() + ")"`
    /// - `Err(e)` -> `"Err(" + e.debug() + ")"`
    ///
    /// IMPORTANT: Payload extraction and formatting MUST happen inside the
    /// respective branch blocks, not before the branch. The inactive variant's
    /// storage may contain garbage pointers that would segfault if formatted.
    pub(crate) fn emit_result_debug(
        &mut self,
        arg_vals: &[ValueId],
        receiver_ty: Idx,
    ) -> Option<ValueId> {
        let receiver = arg_vals[0];
        let tag = self.builder.extract_value(receiver, 0, "res.tag")?;
        let ok_const = self
            .builder
            .const_int_matching(tag, ori_ir::RESULT_TAG_OK as u64);
        let is_ok = self.builder.icmp_eq(tag, ok_const, "is_ok");

        let TypeInfo::Result {
            ok: ok_ty,
            err: err_ty,
        } = self.type_info.get(receiver_ty)
        else {
            return None;
        };

        let ok_bb = self.builder.append_block(self.current_function, "rdbg.ok");
        let err_bb = self.builder.append_block(self.current_function, "rdbg.err");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "rdbg.merge");

        self.builder.cond_br(is_ok, ok_bb, err_bb);

        // Ok block: extract ACTIVE payload, format "Ok(" + ok_str + ")"
        self.builder.position_at_end(ok_bb);
        let ok_payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, ok_ty)?;
        let ok_str = self.emit_element_debug(ok_payload, ok_ty)?;
        let ok_prefix = self.emit_literal_ori_str("Ok(")?;
        let ok_suffix = self.emit_literal_ori_str(")")?;
        let ok_tmp = self.emit_str_concat(ok_prefix, ok_str)?;
        let ok_result = self.emit_str_concat(ok_tmp, ok_suffix)?;
        self.dec_intermediate_str(ok_tmp);
        let ok_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        // Err block: extract ACTIVE payload, format "Err(" + err_str + ")"
        self.builder.position_at_end(err_bb);
        let err_payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, err_ty)?;
        let err_str = self.emit_element_debug(err_payload, err_ty)?;
        let err_prefix = self.emit_literal_ori_str("Err(")?;
        let err_suffix = self.emit_literal_ori_str(")")?;
        let err_tmp = self.emit_str_concat(err_prefix, err_str)?;
        let err_result = self.emit_str_concat(err_tmp, err_suffix)?;
        self.dec_intermediate_str(err_tmp);
        let err_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        // Merge
        self.builder.position_at_end(merge_bb);
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let phi = self.builder.phi(str_ty, "rdbg.result");
        self.builder
            .add_phi_incoming(phi, &[(ok_result, ok_bb_final), (err_result, err_bb_final)]);
        Some(phi)
    }

    /// Emit `Result.debug()` for a nested Result inside `emit_element_debug`.
    ///
    /// Variant of `emit_result_debug` that takes pre-resolved type indices
    /// instead of reading from `arg_vals`.
    ///
    /// IMPORTANT: Payload extraction happens inside branches, not before —
    /// inactive variant storage may contain garbage pointers.
    fn emit_nested_result_debug(
        &mut self,
        receiver: ValueId,
        receiver_ty: Idx,
        ok_ty: Idx,
        err_ty: Idx,
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

        // Ok branch — only format the ACTIVE Ok payload
        self.builder.position_at_end(ok_bb);
        let ok_payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, ok_ty)?;
        let ok_str = self.emit_element_debug(ok_payload, ok_ty)?;
        let ok_prefix = self.emit_literal_ori_str("Ok(")?;
        let ok_suffix = self.emit_literal_ori_str(")")?;
        let ok_tmp = self.emit_str_concat(ok_prefix, ok_str)?;
        let ok_result = self.emit_str_concat(ok_tmp, ok_suffix)?;
        self.dec_intermediate_str(ok_tmp);
        let ok_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        // Err branch — only format the ACTIVE Err payload
        self.builder.position_at_end(err_bb);
        let err_payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, err_ty)?;
        let err_str = self.emit_element_debug(err_payload, err_ty)?;
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

    /// Emit `[T].debug()` -- element-wise loop producing `"[e1, e2, ...]"`.
    ///
    /// Layout: `{i64 len, i64 cap, ptr data}`.
    /// For empty lists, returns `"[]"` immediately.
    fn emit_list_debug(&mut self, list: ValueId, elem_ty: Idx) -> Option<ValueId> {
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

        // Empty: return "[]"
        self.builder.position_at_end(empty_bb);
        let empty_str = self.emit_literal_ori_str("[]")?;
        let empty_bb_end = self.builder.current_block().unwrap();
        self.builder.br(done_bb);

        // First element: "[" + debug(elem[0])
        self.builder.position_at_end(first_bb);
        let open = self.emit_literal_ori_str("[")?;
        let elem_llvm_ty = self.int_element_llvm_type(elem_ty);
        let ptr0 = self.builder.gep(elem_llvm_ty, data, &[zero], "ldbg.ep0");
        let elem0 = self.builder.load(elem_llvm_ty, ptr0, "ldbg.e0");
        let elem0 = self.sext_narrowed_int_element(elem0, elem_ty, "ldbg.e0.sext");
        let elem0_str = self.emit_element_debug(elem0, elem_ty)?;
        let acc_init = self.emit_str_concat(open, elem0_str)?;
        self.dec_intermediate_str(elem0_str);
        let needs_loop = self.builder.icmp_sgt(len, one, "ldbg.needs_loop");
        let first_bb_end = self.builder.current_block().unwrap();
        self.builder.cond_br(needs_loop, loop_hdr, close_bb);

        // Loop header: check idx < len
        self.builder.position_at_end(loop_hdr);
        let i64_ty = self
            .builder
            .register_type(self.builder.scx().type_i64().into());
        let idx_phi = self.builder.phi(i64_ty, "ldbg.idx");
        let acc_phi = self.builder.phi(str_ty, "ldbg.acc");
        let has_more = self.builder.icmp_slt(idx_phi, len, "ldbg.more");
        self.builder.cond_br(has_more, loop_body, close_bb);

        // Loop body: acc = acc + ", " + debug(elem[idx])
        self.builder.position_at_end(loop_body);
        let sep = self.emit_literal_ori_str(", ")?;
        let with_sep = self.emit_str_concat(acc_phi, sep)?;
        self.dec_intermediate_str(acc_phi);
        let ptr_i = self.builder.gep(elem_llvm_ty, data, &[idx_phi], "ldbg.epi");
        let elem_i = self.builder.load(elem_llvm_ty, ptr_i, "ldbg.ei");
        let elem_i = self.sext_narrowed_int_element(elem_i, elem_ty, "ldbg.ei.sext");
        let elem_i_str = self.emit_element_debug(elem_i, elem_ty)?;
        let new_acc = self.emit_str_concat(with_sep, elem_i_str)?;
        self.dec_intermediate_str(with_sep);
        self.dec_intermediate_str(elem_i_str);
        let next_idx = self.builder.add(idx_phi, one, "ldbg.next");
        let body_end = self.builder.current_block().unwrap();
        self.builder.br(loop_hdr);

        // Wire phis: idx starts at 1, acc starts at acc_init
        self.builder
            .add_phi_incoming(idx_phi, &[(one, first_bb_end), (next_idx, body_end)]);
        self.builder
            .add_phi_incoming(acc_phi, &[(acc_init, first_bb_end), (new_acc, body_end)]);

        // Close: acc + "]"
        self.builder.position_at_end(close_bb);
        let close_acc = self.builder.phi(str_ty, "ldbg.close.acc");
        self.builder
            .add_phi_incoming(close_acc, &[(acc_init, first_bb_end), (acc_phi, loop_hdr)]);
        let suffix = self.emit_literal_ori_str("]")?;
        let result = self.emit_str_concat(close_acc, suffix)?;
        self.dec_intermediate_str(close_acc);
        let close_bb_end = self.builder.current_block().unwrap();
        self.builder.br(done_bb);

        // Done: phi between empty and formatted
        self.builder.position_at_end(done_bb);
        let final_phi = self.builder.phi(str_ty, "ldbg.final");
        self.builder.add_phi_incoming(
            final_phi,
            &[(empty_str, empty_bb_end), (result, close_bb_end)],
        );
        Some(final_phi)
    }

    /// Emit `(A, B, ...).debug()` -- field-wise formatting as `"(a, b, ...)"`.
    fn emit_tuple_debug(&mut self, tuple: ValueId, elements: &[Idx]) -> Option<ValueId> {
        if elements.is_empty() {
            return self.emit_literal_ori_str("()");
        }

        let mut acc = self.emit_literal_ori_str("(")?;
        for (i, &elem_ty) in elements.iter().enumerate() {
            if i > 0 {
                let sep = self.emit_literal_ori_str(", ")?;
                let new_acc = self.emit_str_concat(acc, sep)?;
                self.dec_intermediate_str(acc);
                acc = new_acc;
            }
            let field = self
                .builder
                .extract_value(tuple, i as u32, &format!("tdbg.f{i}"))?;
            let field_str = self.emit_element_debug(field, elem_ty)?;
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

    /// Call a compiled `.debug()` method for a type via method dispatch.
    ///
    /// Looks up the type's debug function in `method_functions`, applies
    /// ABI parameter passing (Indirect for large structs), and handles
    /// sret return (debug returns `str` which is 24 bytes → sret).
    /// Returns `None` if no compiled debug method exists for the type.
    fn emit_derived_debug_call(&mut self, val: ValueId, ty: Idx) -> Option<ValueId> {
        let type_name = *self.ctx.type_idx_to_name.get(&ty)?;
        let interned_debug = self.interner.intern("debug");
        let (func_id, abi) = {
            let (fid, abi) = self
                .ctx
                .method_functions
                .get(&(type_name, interned_debug))?;
            (*fid, abi.clone())
        };

        let raw_args = [val];
        let passed_args = self.apply_param_passing(&raw_args, &abi.params);

        // Debug returns str (24 bytes), which uses sret return convention.
        match &abi.return_abi.passing {
            crate::codegen::abi::ReturnPassing::Sret { .. } => {
                let str_ty = self.resolve_type(Idx::STR);
                self.call_with_sret(func_id, &passed_args, str_ty, "dbg.derived")
            }
            _ => self.emit_rt_call(func_id, &passed_args, "dbg.derived"),
        }
    }
}
