//! Closure-taking monadic methods for Result LLVM codegen.
//!
//! Implements `map`, `map_err`, `and_then`, `or_else` for Result.
//! Each method emits conditional LLVM blocks with phi-node merges.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_types::Idx;

use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit `Result.map(transform:)`, `.map_err(transform:)`,
    /// `.and_then(then:)`, `.or_else(f:)`.
    pub(crate) fn emit_result_monadic(
        &mut self,
        method: &str,
        arg_vals: &[ValueId],
        receiver_ty: Idx,
        arc_args: &[ArcVarId],
        arc_func: &ArcFunction,
    ) -> Option<ValueId> {
        let receiver = arg_vals[0];

        if let Some(encoding) = self.get_niche_encoding(receiver_ty) {
            return self.emit_result_monadic_niche(
                method, receiver, arg_vals, &encoding, arc_args, arc_func,
            );
        }

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

        match method {
            "map" => self.emit_res_map(
                receiver,
                receiver_ty,
                is_ok,
                ok_ty,
                err_ty,
                arg_vals,
                arc_args,
                arc_func,
            ),
            "map_err" => self.emit_res_map_err(
                receiver,
                receiver_ty,
                is_ok,
                ok_ty,
                err_ty,
                arg_vals,
                arc_args,
                arc_func,
            ),
            "and_then" => self.emit_res_and_then(
                receiver,
                receiver_ty,
                is_ok,
                ok_ty,
                err_ty,
                arg_vals,
                arc_args,
                arc_func,
            ),
            "or_else" => self.emit_res_or_else(
                receiver,
                receiver_ty,
                is_ok,
                ok_ty,
                err_ty,
                arg_vals,
                arc_args,
                arc_func,
            ),
            _ => None,
        }
    }

    /// `Result.map(f)`: `if Ok(v) { Ok(f(v)) } else { Err(e) }`
    fn emit_res_map(
        &mut self,
        receiver: ValueId,
        receiver_ty: Idx,
        is_ok: ValueId,
        ok_ty: Idx,
        err_ty: Idx,
        arg_vals: &[ValueId],
        arc_args: &[ArcVarId],
        arc_func: &ArcFunction,
    ) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let closure = arg_vals[1];
        let mapped_ok_ty = self.closure_return_ty(arc_args, arc_func)?;

        let ok_bb = self.builder.append_block(self.current_function, "rmap.ok");
        let err_bb = self.builder.append_block(self.current_function, "rmap.err");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "rmap.merge");
        self.builder.cond_br(is_ok, ok_bb, err_bb);

        // Ok: call closure, build Ok(mapped) in Result<U, E> layout
        self.builder.position_at_end(ok_bb);
        let ok_payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, ok_ty)?;
        let mapped = self.call_closure_single_arg(closure, ok_payload, ok_ty, mapped_ok_ty)?;
        let ok_result = self.build_result_struct(
            ori_ir::RESULT_TAG_OK,
            mapped,
            mapped_ok_ty,
            mapped_ok_ty,
            err_ty,
            "rmap.ok",
        )?;
        let ok_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        // Err: repackage err payload into Result<U, E>
        self.builder.position_at_end(err_bb);
        let err_payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, err_ty)?;
        let err_result = self.build_result_struct(
            ori_ir::RESULT_TAG_ERR,
            err_payload,
            err_ty,
            mapped_ok_ty,
            err_ty,
            "rmap.err",
        )?;
        let err_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        self.builder.position_at_end(merge_bb);
        let result_llvm = self.resolve_type_for_result(mapped_ok_ty, err_ty);
        let phi = self.builder.phi(result_llvm, "rmap.result");
        self.builder
            .add_phi_incoming(phi, &[(ok_result, ok_bb_final), (err_result, err_bb_final)]);
        Some(phi)
    }

    /// `Result.map_err(f)`: `if Err(e) { Err(f(e)) } else { Ok(v) }`
    fn emit_res_map_err(
        &mut self,
        receiver: ValueId,
        receiver_ty: Idx,
        is_ok: ValueId,
        ok_ty: Idx,
        err_ty: Idx,
        arg_vals: &[ValueId],
        arc_args: &[ArcVarId],
        arc_func: &ArcFunction,
    ) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let closure = arg_vals[1];
        let mapped_err_ty = self.closure_return_ty(arc_args, arc_func)?;

        let ok_bb = self.builder.append_block(self.current_function, "rme.ok");
        let err_bb = self.builder.append_block(self.current_function, "rme.err");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "rme.merge");
        self.builder.cond_br(is_ok, ok_bb, err_bb);

        // Ok: repackage ok payload into Result<T, F>
        self.builder.position_at_end(ok_bb);
        let ok_payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, ok_ty)?;
        let ok_result = self.build_result_struct(
            ori_ir::RESULT_TAG_OK,
            ok_payload,
            ok_ty,
            ok_ty,
            mapped_err_ty,
            "rme.ok",
        )?;
        let ok_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        // Err: call closure, build Err(mapped) in Result<T, F> layout
        self.builder.position_at_end(err_bb);
        let err_payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, err_ty)?;
        let mapped = self.call_closure_single_arg(closure, err_payload, err_ty, mapped_err_ty)?;
        let err_result = self.build_result_struct(
            ori_ir::RESULT_TAG_ERR,
            mapped,
            mapped_err_ty,
            ok_ty,
            mapped_err_ty,
            "rme.err",
        )?;
        let err_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        self.builder.position_at_end(merge_bb);
        let result_llvm = self.resolve_type_for_result(ok_ty, mapped_err_ty);
        let phi = self.builder.phi(result_llvm, "rme.result");
        self.builder
            .add_phi_incoming(phi, &[(ok_result, ok_bb_final), (err_result, err_bb_final)]);
        Some(phi)
    }

    /// `Result.and_then(f)`: `if Ok(v) { f(v) } else { Err(e) }`
    fn emit_res_and_then(
        &mut self,
        receiver: ValueId,
        receiver_ty: Idx,
        is_ok: ValueId,
        ok_ty: Idx,
        err_ty: Idx,
        arg_vals: &[ValueId],
        arc_args: &[ArcVarId],
        arc_func: &ArcFunction,
    ) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let closure = arg_vals[1];
        let return_ty = self.closure_return_ty(arc_args, arc_func)?;

        let ok_bb = self.builder.append_block(self.current_function, "rat.ok");
        let err_bb = self.builder.append_block(self.current_function, "rat.err");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "rat.merge");
        self.builder.cond_br(is_ok, ok_bb, err_bb);

        // Ok: call closure (returns Result directly)
        self.builder.position_at_end(ok_bb);
        let ok_payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, ok_ty)?;
        let ok_result = self.call_closure_single_arg(closure, ok_payload, ok_ty, return_ty)?;
        let ok_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        // Err: build Err in the return type's layout
        self.builder.position_at_end(err_bb);
        let err_payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, err_ty)?;
        let result_llvm_ty = self.resolve_type(return_ty);
        let zero = self.builder.const_zero_ty(result_llvm_ty);
        let err_tag = self.builder.const_i64(ori_ir::RESULT_TAG_ERR);
        let with_tag = self.builder.insert_value(zero, err_tag, 0, "rat.err");
        let alloca = self.builder.alloca(result_llvm_ty, "rat.err.a");
        self.builder.store(with_tag, alloca);
        let gep = self
            .builder
            .struct_gep(result_llvm_ty, alloca, 1, "rat.err.g");
        self.builder.store(err_payload, gep);
        let err_result = self.builder.load(result_llvm_ty, alloca, "rat.err.r");
        let err_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        self.builder.position_at_end(merge_bb);
        let phi = self.builder.phi(result_llvm_ty, "rat.result");
        self.builder
            .add_phi_incoming(phi, &[(ok_result, ok_bb_final), (err_result, err_bb_final)]);
        Some(phi)
    }

    /// `Result.or_else(f)`: `if Ok { self } else { f(err) }`
    fn emit_res_or_else(
        &mut self,
        receiver: ValueId,
        receiver_ty: Idx,
        is_ok: ValueId,
        ok_ty: Idx,
        err_ty: Idx,
        arg_vals: &[ValueId],
        arc_args: &[ArcVarId],
        arc_func: &ArcFunction,
    ) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let closure = arg_vals[1];
        let return_ty = self.closure_return_ty(arc_args, arc_func)?;

        let ok_bb = self.builder.append_block(self.current_function, "roe.ok");
        let err_bb = self.builder.append_block(self.current_function, "roe.err");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "roe.merge");
        self.builder.cond_br(is_ok, ok_bb, err_bb);

        // Ok: repackage ok payload into the return type layout
        self.builder.position_at_end(ok_bb);
        let ok_payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, ok_ty)?;
        let result_llvm_ty = self.resolve_type(return_ty);
        let zero = self.builder.const_zero_ty(result_llvm_ty);
        let ok_tag = self.builder.const_i64(ori_ir::RESULT_TAG_OK);
        let with_tag = self.builder.insert_value(zero, ok_tag, 0, "roe.ok");
        let alloca = self.builder.alloca(result_llvm_ty, "roe.ok.a");
        self.builder.store(with_tag, alloca);
        let gep = self
            .builder
            .struct_gep(result_llvm_ty, alloca, 1, "roe.ok.g");
        self.builder.store(ok_payload, gep);
        let ok_result = self.builder.load(result_llvm_ty, alloca, "roe.ok.r");
        let ok_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        // Err: call closure with err payload
        self.builder.position_at_end(err_bb);
        let err_payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, err_ty)?;
        let err_result = self.call_closure_single_arg(closure, err_payload, err_ty, return_ty)?;
        let err_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        self.builder.position_at_end(merge_bb);
        let phi = self.builder.phi(result_llvm_ty, "roe.result");
        self.builder
            .add_phi_incoming(phi, &[(ok_result, ok_bb_final), (err_result, err_bb_final)]);
        Some(phi)
    }

    // Helpers

    /// Build a Result struct `{i64, max(ok, err)}` with correct padding.
    pub(super) fn build_result_struct(
        &mut self,
        tag_val: i64,
        payload: ValueId,
        payload_ty: Idx,
        ok_ty: Idx,
        err_ty: Idx,
        label: &str,
    ) -> Option<ValueId> {
        let ok_size = crate::codegen::abi::abi_size(ok_ty, self.type_info, self.repr_plan);
        let err_size = crate::codegen::abi::abi_size(err_ty, self.type_info, self.repr_plan);
        let slot_ty_idx = if ok_size >= err_size { ok_ty } else { err_ty };

        let payload_llvm = self.resolve_type(payload_ty);
        let slot_llvm = self.resolve_type(slot_ty_idx);

        let scx = self.builder.scx();
        let i64_ty = scx.type_i64();
        let slot_raw = self.builder.raw_type(slot_llvm);
        let struct_ty = scx.llcx.struct_type(&[i64_ty.into(), slot_raw], false);
        let struct_llvm = self.builder.register_type(struct_ty.into());

        let tag = self.builder.const_i64(tag_val);
        let zero = self.builder.const_zero_ty(struct_llvm);
        let with_tag = self.builder.insert_value(zero, tag, 0, label);

        if payload_llvm == slot_llvm {
            Some(self.builder.insert_value(with_tag, payload, 1, label))
        } else {
            let alloca = self.builder.alloca(struct_llvm, label);
            self.builder.store(with_tag, alloca);
            let gep = self.builder.struct_gep(struct_llvm, alloca, 1, label);
            self.builder.store(payload, gep);
            Some(self.builder.load(struct_llvm, alloca, label))
        }
    }

    /// Resolve the LLVM type for `Result<T, E>` = `{i64, max(T, E)}`.
    pub(super) fn resolve_type_for_result(
        &mut self,
        ok_ty: Idx,
        err_ty: Idx,
    ) -> crate::codegen::value_id::LLVMTypeId {
        let ok_size = crate::codegen::abi::abi_size(ok_ty, self.type_info, self.repr_plan);
        let err_size = crate::codegen::abi::abi_size(err_ty, self.type_info, self.repr_plan);
        let slot_ty = if ok_size >= err_size { ok_ty } else { err_ty };
        let slot_llvm = self.resolve_type(slot_ty);
        let scx = self.builder.scx();
        let i64_ty = scx.type_i64();
        let slot_raw = self.builder.raw_type(slot_llvm);
        let res = scx.llcx.struct_type(&[i64_ty.into(), slot_raw], false);
        self.builder.register_type(res.into())
    }

    // Niche-encoded stub

    #[expect(
        clippy::unused_self,
        reason = "stub — will gain niche implementation later"
    )]
    fn emit_result_monadic_niche(
        &mut self,
        _method: &str,
        _receiver: ValueId,
        _arg_vals: &[ValueId],
        _encoding: &crate::codegen::arc_emitter::tag_access::TagEncoding,
        _arc_args: &[ArcVarId],
        _arc_func: &ArcFunction,
    ) -> Option<ValueId> {
        None
    }
}
