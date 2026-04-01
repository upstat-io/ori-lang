//! Closure-taking monadic methods for Option LLVM codegen.
//!
//! Implements `map`, `and_then`, `flat_map`, `filter`, `or`, `or_else`,
//! `ok_or` for Option. Each method emits conditional LLVM blocks with
//! phi-node merges.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_types::Idx;

use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit `Option.map(transform:)`, `.and_then(then:)`, `.flat_map(f:)`,
    /// `.filter(predicate:)`, `.or(other:)`, `.or_else(f:)`, `.ok_or(err:)`.
    pub(crate) fn emit_option_monadic(
        &mut self,
        method: &str,
        arg_vals: &[ValueId],
        receiver_ty: Idx,
        arc_args: &[ArcVarId],
        arc_func: &ArcFunction,
    ) -> Option<ValueId> {
        let receiver = arg_vals[0];

        // Niche-encoded dispatch
        if let Some(encoding) = self.get_niche_encoding(receiver_ty) {
            return self.emit_option_monadic_niche(
                method,
                receiver,
                arg_vals,
                receiver_ty,
                &encoding,
                arc_args,
                arc_func,
            );
        }

        let tag = self.builder.extract_value(receiver, 0, "opt.tag")?;
        let some_const = self
            .builder
            .const_int_matching(tag, ori_ir::OPTION_TAG_SOME as u64);
        let is_some = self.builder.icmp_eq(tag, some_const, "is_some");

        let TypeInfo::Option { inner } = self.type_info.get(receiver_ty) else {
            return None;
        };

        match method {
            "map" => self.emit_opt_map(receiver, is_some, inner, arg_vals, arc_args, arc_func),
            "and_then" | "flat_map" => {
                self.emit_opt_and_then(receiver, is_some, inner, arg_vals, arc_args, arc_func)
            }
            "filter" => self.emit_opt_filter(receiver, is_some, inner, arg_vals),
            "or" if arg_vals.len() >= 2 => {
                Some(
                    self.builder
                        .select(is_some, receiver, arg_vals[1], "opt.or"),
                )
            }
            "or_else" => self.emit_opt_or_else(receiver, is_some, arg_vals, arc_args, arc_func),
            "ok_or" if arg_vals.len() >= 2 => {
                self.emit_opt_ok_or(receiver, is_some, inner, arg_vals)
            }
            _ => None,
        }
    }

    /// `Option.map(f)`: `if Some(v) { Some(f(v)) } else { None }`
    fn emit_opt_map(
        &mut self,
        receiver: ValueId,
        is_some: ValueId,
        inner: Idx,
        arg_vals: &[ValueId],
        arc_args: &[ArcVarId],
        arc_func: &ArcFunction,
    ) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let closure = arg_vals[1];
        let return_ty = self.closure_return_ty(arc_args, arc_func)?;

        let some_bb = self.builder.append_block(self.current_function, "map.some");
        let none_bb = self.builder.append_block(self.current_function, "map.none");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "map.merge");
        self.builder.cond_br(is_some, some_bb, none_bb);

        // Some: extract payload, call closure, wrap in Some
        self.builder.position_at_end(some_bb);
        let payload = self.builder.extract_value(receiver, 1, "opt.val")?;
        let mapped = self.call_closure_single_arg(closure, payload, inner, return_ty)?;
        let some_tag = self.builder.const_i64(ori_ir::OPTION_TAG_SOME);
        let some_result = self.build_option_struct(some_tag, mapped, return_ty)?;
        let some_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        // None: build typed None
        self.builder.position_at_end(none_bb);
        let none_result = self.build_none(return_ty)?;
        let none_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        // Merge
        self.builder.position_at_end(merge_bb);
        let result_llvm = self.resolve_type_for_option(return_ty);
        let phi = self.builder.phi(result_llvm, "map.result");
        self.builder.add_phi_incoming(
            phi,
            &[(some_result, some_bb_final), (none_result, none_bb_final)],
        );
        Some(phi)
    }

    /// `Option.and_then(f)` / `Option.flat_map(f)`:
    /// `if Some(v) { f(v) } else { None }`
    fn emit_opt_and_then(
        &mut self,
        receiver: ValueId,
        is_some: ValueId,
        inner: Idx,
        arg_vals: &[ValueId],
        arc_args: &[ArcVarId],
        arc_func: &ArcFunction,
    ) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let closure = arg_vals[1];
        let return_ty = self.closure_return_ty(arc_args, arc_func)?;

        let some_bb = self.builder.append_block(self.current_function, "at.some");
        let none_bb = self.builder.append_block(self.current_function, "at.none");
        let merge_bb = self.builder.append_block(self.current_function, "at.merge");
        self.builder.cond_br(is_some, some_bb, none_bb);

        // Some: call closure (returns Option<U> directly)
        self.builder.position_at_end(some_bb);
        let payload = self.builder.extract_value(receiver, 1, "opt.val")?;
        let some_result = self.call_closure_single_arg(closure, payload, inner, return_ty)?;
        let some_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        // None: build zero value with None tag
        self.builder.position_at_end(none_bb);
        let result_llvm = self.resolve_type(return_ty);
        let zero = self.builder.const_zero_ty(result_llvm);
        let none_tag = self.builder.const_i64(ori_ir::OPTION_TAG_NONE);
        let none_result = self.builder.insert_value(zero, none_tag, 0, "at.none");
        let none_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        // Merge
        self.builder.position_at_end(merge_bb);
        let phi = self.builder.phi(result_llvm, "at.result");
        self.builder.add_phi_incoming(
            phi,
            &[(some_result, some_bb_final), (none_result, none_bb_final)],
        );
        Some(phi)
    }

    /// `Option.filter(pred)`: `if Some(v) && pred(v) { self } else { None }`
    fn emit_opt_filter(
        &mut self,
        receiver: ValueId,
        is_some: ValueId,
        inner: Idx,
        arg_vals: &[ValueId],
    ) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let closure = arg_vals[1];

        let some_bb = self.builder.append_block(self.current_function, "flt.some");
        let none_bb = self.builder.append_block(self.current_function, "flt.none");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "flt.merge");
        self.builder.cond_br(is_some, some_bb, none_bb);

        // Some: call predicate, select between self and None
        self.builder.position_at_end(some_bb);
        let payload = self.builder.extract_value(receiver, 1, "opt.val")?;
        let pred_result = self.call_closure_single_arg(closure, payload, inner, Idx::BOOL)?;
        let zero_i8 = self.builder.const_i8(0);
        let pred_i1 = self.builder.icmp_ne(pred_result, zero_i8, "flt.pred");
        let none_val = self.build_none(inner)?;
        let keep_result = self.builder.select(pred_i1, receiver, none_val, "flt.sel");
        let some_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        // None: pass through receiver
        self.builder.position_at_end(none_bb);
        let none_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        // Merge
        self.builder.position_at_end(merge_bb);
        let opt_llvm = self.resolve_type_for_option(inner);
        let phi = self.builder.phi(opt_llvm, "flt.result");
        self.builder.add_phi_incoming(
            phi,
            &[(keep_result, some_bb_final), (receiver, none_bb_final)],
        );
        Some(phi)
    }

    /// `Option.or_else(f)`: `if Some { self } else { f() }`
    fn emit_opt_or_else(
        &mut self,
        receiver: ValueId,
        is_some: ValueId,
        arg_vals: &[ValueId],
        arc_args: &[ArcVarId],
        arc_func: &ArcFunction,
    ) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let closure = arg_vals[1];
        let return_ty = self.closure_return_ty(arc_args, arc_func)?;

        let some_bb = self.builder.append_block(self.current_function, "ore.some");
        let none_bb = self.builder.append_block(self.current_function, "ore.none");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "ore.merge");
        self.builder.cond_br(is_some, some_bb, none_bb);

        self.builder.position_at_end(some_bb);
        let some_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        self.builder.position_at_end(none_bb);
        let none_result = self.call_closure_no_args(closure, return_ty)?;
        let none_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        self.builder.position_at_end(merge_bb);
        let result_llvm = self.resolve_type(return_ty);
        let phi = self.builder.phi(result_llvm, "ore.result");
        self.builder.add_phi_incoming(
            phi,
            &[(receiver, some_bb_final), (none_result, none_bb_final)],
        );
        Some(phi)
    }

    /// `Option.ok_or(err)`: `if Some(v) { Ok(v) } else { Err(err) }`
    fn emit_opt_ok_or(
        &mut self,
        receiver: ValueId,
        is_some: ValueId,
        inner: Idx,
        arg_vals: &[ValueId],
    ) -> Option<ValueId> {
        let err_val = arg_vals[1];

        let some_bb = self
            .builder
            .append_block(self.current_function, "okor.some");
        let none_bb = self
            .builder
            .append_block(self.current_function, "okor.none");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "okor.merge");
        self.builder.cond_br(is_some, some_bb, none_bb);

        // Some: Ok(payload)
        self.builder.position_at_end(some_bb);
        let payload = self.builder.extract_value(receiver, 1, "opt.val")?;
        let ok_tag = self.builder.const_i64(ori_ir::RESULT_TAG_OK);
        let ok_result = self.build_option_struct(ok_tag, payload, inner)?;
        let some_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        // None: Err(err_val)
        self.builder.position_at_end(none_bb);
        let err_tag = self.builder.const_i64(ori_ir::RESULT_TAG_ERR);
        let err_result = self.build_option_struct(err_tag, err_val, inner)?;
        let none_bb_final = self.builder.current_block().unwrap();
        self.builder.br(merge_bb);

        self.builder.position_at_end(merge_bb);
        let result_llvm = self.resolve_type_for_option(inner);
        let phi = self.builder.phi(result_llvm, "okor.result");
        self.builder.add_phi_incoming(
            phi,
            &[(ok_result, some_bb_final), (err_result, none_bb_final)],
        );
        Some(phi)
    }

    // Helpers

    /// Build a `None` value as `{i64 OPTION_TAG_NONE, zeroed payload}`.
    fn build_none(&mut self, payload_ty: Idx) -> Option<ValueId> {
        let none_tag = self.builder.const_i64(ori_ir::OPTION_TAG_NONE);
        let payload_llvm = self.resolve_type(payload_ty);
        let zero_payload = self.builder.const_zero_ty(payload_llvm);
        self.build_option_struct(none_tag, zero_payload, payload_ty)
    }

    /// Resolve the LLVM type for `Option<T>` = `{i64, T}`.
    fn resolve_type_for_option(&mut self, inner_ty: Idx) -> crate::codegen::value_id::LLVMTypeId {
        let inner_llvm = self.resolve_type(inner_ty);
        let scx = self.builder.scx();
        let i64_ty = scx.type_i64();
        let inner_raw = self.builder.raw_type(inner_llvm);
        let opt = scx.llcx.struct_type(&[i64_ty.into(), inner_raw], false);
        self.builder.register_type(opt.into())
    }

    // Niche-encoded stub

    /// Niche-encoded Option monadic dispatch.
    #[expect(
        clippy::unused_self,
        reason = "stub — will gain niche implementation later"
    )]
    fn emit_option_monadic_niche(
        &mut self,
        _method: &str,
        _receiver: ValueId,
        _arg_vals: &[ValueId],
        _receiver_ty: Idx,
        _encoding: &crate::codegen::arc_emitter::tag_access::TagEncoding,
        _arc_args: &[ArcVarId],
        _arc_func: &ArcFunction,
    ) -> Option<ValueId> {
        // Niche-encoded monadic methods fall through to runtime path.
        None
    }
}
