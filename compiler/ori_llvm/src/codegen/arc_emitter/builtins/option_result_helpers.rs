//! Helper methods for Option/Result LLVM codegen.
//!
//! Contains: niche dispatch, expect panic branches, and closure call utilities.
//! Debug formatting lives in `debug_helpers.rs`.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::{CLOSURE_FIELD_ENV, CLOSURE_FIELD_FN};
use ori_types::Idx;

use crate::codegen::arc_emitter::tag_access::TagEncoding;
use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Niche-encoded Option dispatch (§07.2).
    pub(super) fn emit_option_niche(
        &mut self,
        method: &str,
        receiver: ValueId,
        arg_vals: &[ValueId],
        receiver_ty: Idx,
        encoding: &TagEncoding,
    ) -> Option<ValueId> {
        let niche_idx = encoding.niche_field_index().unwrap();
        let niche_value = encoding.niche_value().unwrap();
        match method {
            "is_some" => {
                let field = self
                    .builder
                    .extract_value(receiver, niche_idx, "opt.niche")?;
                let is_niche = self.niche_is_sentinel(field, niche_value, "is_niche");
                let t = self.builder.const_bool(true);
                let f = self.builder.const_bool(false);
                Some(self.builder.select(is_niche, f, t, "is_some"))
            }
            "is_none" => {
                let field = self
                    .builder
                    .extract_value(receiver, niche_idx, "opt.niche")?;
                Some(self.niche_is_sentinel(field, niche_value, "is_none"))
            }
            "unwrap" => self.builder.extract_value(receiver, 0, "opt.payload"),
            "unwrap_or" if arg_vals.len() >= 2 => {
                let field = self
                    .builder
                    .extract_value(receiver, niche_idx, "opt.niche")?;
                let is_niche = self.niche_is_sentinel(field, niche_value, "is_niche");
                let payload = self.builder.extract_value(receiver, 0, "opt.payload")?;
                Some(
                    self.builder
                        .select(is_niche, arg_vals[1], payload, "unwrap_or"),
                )
            }
            "expect" if arg_vals.len() >= 2 => {
                let field = self
                    .builder
                    .extract_value(receiver, niche_idx, "opt.niche")?;
                let is_niche = self.niche_is_sentinel(field, niche_value, "is_niche");
                let t = self.builder.const_bool(true);
                let f = self.builder.const_bool(false);
                let is_some = self.builder.select(is_niche, f, t, "is_some");
                self.emit_expect_branch(is_some, arg_vals[1], "expect")?;
                self.builder.extract_value(receiver, 0, "opt.payload")
            }
            "debug" | "to_str" => {
                let field = self
                    .builder
                    .extract_value(receiver, niche_idx, "opt.niche")?;
                let is_niche = self.niche_is_sentinel(field, niche_value, "is_niche");
                let t = self.builder.const_bool(true);
                let f = self.builder.const_bool(false);
                let is_some = self.builder.select(is_niche, f, t, "is_some");
                let payload = self.builder.extract_value(receiver, 0, "opt.payload")?;
                let TypeInfo::Option { inner } = self.type_info.get(receiver_ty) else {
                    return None;
                };
                self.emit_option_debug_branch(is_some, payload, inner, method == "debug")
            }
            "clone" => Some(receiver),
            _ => None,
        }
    }

    /// Niche-encoded Result dispatch (§07.2).
    pub(super) fn emit_result_niche(
        &mut self,
        method: &str,
        receiver: ValueId,
        arg_vals: &[ValueId],
        encoding: &TagEncoding,
    ) -> Option<ValueId> {
        let niche_idx = encoding.niche_field_index().unwrap();
        let niche_value = encoding.niche_value().unwrap();
        let niche_variant_idx = encoding.niche_variant_idx().unwrap();
        match method {
            "is_ok" => {
                let field = self
                    .builder
                    .extract_value(receiver, niche_idx, "res.niche")?;
                let is_niche = self.niche_is_sentinel(field, niche_value, "res.is_niche");
                if niche_variant_idx == 0 {
                    Some(is_niche)
                } else {
                    let t = self.builder.const_bool(true);
                    let f = self.builder.const_bool(false);
                    Some(self.builder.select(is_niche, f, t, "is_ok"))
                }
            }
            "is_err" => {
                let field = self
                    .builder
                    .extract_value(receiver, niche_idx, "res.niche")?;
                let is_niche = self.niche_is_sentinel(field, niche_value, "res.is_niche");
                if niche_variant_idx == 1 {
                    Some(is_niche)
                } else {
                    let t = self.builder.const_bool(true);
                    let f = self.builder.const_bool(false);
                    Some(self.builder.select(is_niche, f, t, "is_err"))
                }
            }
            "unwrap" | "unwrap_err" | "unwrap_or" => {
                self.builder.extract_value(receiver, 0, "res.payload")
            }
            "expect" if arg_vals.len() >= 2 => {
                // Niche path: no tag check needed for expect (niche layout guarantee)
                self.builder.extract_value(receiver, 0, "res.payload")
            }
            "expect_err" if arg_vals.len() >= 2 => {
                self.builder.extract_value(receiver, 0, "res.payload")
            }
            "clone" => Some(receiver),
            _ => None,
        }
    }

    /// Build an `Option<T>` struct `{i64 tag, T payload}` from tag and payload values.
    pub(super) fn build_option_struct(
        &mut self,
        tag: ValueId,
        payload: ValueId,
        payload_ty: Idx,
    ) -> Option<ValueId> {
        let payload_llvm = self.resolve_type(payload_ty);
        let scx = self.builder.scx();
        let i64_ty = scx.type_i64();
        let payload_raw = self.builder.raw_type(payload_llvm);
        let opt_struct = scx.llcx.struct_type(&[i64_ty.into(), payload_raw], false);
        let opt_ty = self.builder.register_type(opt_struct.into());
        let zero = self.builder.const_zero_ty(opt_ty);
        let with_tag = self.builder.insert_value(zero, tag, 0, "opt");
        Some(self.builder.insert_value(with_tag, payload, 1, "opt.val"))
    }

    /// Emit a conditional panic branch for `expect`/`expect_err`.
    ///
    /// If `is_valid` is false, calls `ori_panic` with the message string and
    /// halts. If true, falls through to the continue block (positioned on
    /// return). The caller extracts the payload after this returns.
    pub(super) fn emit_expect_branch(
        &mut self,
        is_valid: ValueId,
        msg: ValueId,
        label: &str,
    ) -> Option<()> {
        let ok_bb = self
            .builder
            .append_block(self.current_function, &format!("{label}.ok"));
        let panic_bb = self
            .builder
            .append_block(self.current_function, &format!("{label}.panic"));

        self.builder.cond_br(is_valid, ok_bb, panic_bb);

        // Panic block: alloca the str struct, store msg, call ori_panic.
        self.builder.position_at_end(panic_bb);
        let scx = self.builder.scx();
        let i64_ty = scx.type_i64();
        let ptr_ty = scx.type_ptr();
        let str_struct_ty = scx
            .llcx
            .struct_type(&[i64_ty.into(), i64_ty.into(), ptr_ty.into()], false);
        let str_ty_id = self.builder.register_type(str_struct_ty.into());
        let msg_alloca = self.builder.alloca(str_ty_id, &format!("{label}.msg"));
        self.builder.store(msg, msg_alloca);
        let panic_fn = self.builder.runtime_fn("ori_panic");
        self.emit_rt_call(panic_fn, &[msg_alloca], "");
        self.builder.unreachable();

        // Continue block: position here for payload extraction.
        self.builder.position_at_end(ok_bb);
        Some(())
    }

    // Closure call helpers (shared by option_monadic.rs and result_monadic.rs)

    /// Call a closure `{fn_ptr, env_ptr}` with a single argument.
    ///
    /// Handles ABI for both the argument (direct vs indirect) and the return
    /// value (direct vs sret for types >16 bytes).
    pub(super) fn call_closure_single_arg(
        &mut self,
        closure: ValueId,
        arg: ValueId,
        arg_ty: Idx,
        return_ty: Idx,
    ) -> Option<ValueId> {
        let fn_ptr = self
            .builder
            .extract_value(closure, CLOSURE_FIELD_FN, "clos.fn")?;
        let env_ptr = self
            .builder
            .extract_value(closure, CLOSURE_FIELD_ENV, "clos.env")?;

        let ptr_ty = self.builder.ptr_type();
        let mut args = vec![env_ptr];
        let mut params = vec![ptr_ty];

        let passing = crate::codegen::abi::compute_param_passing(arg_ty, self.type_info);
        match passing {
            crate::codegen::abi::ParamPassing::Indirect { .. }
            | crate::codegen::abi::ParamPassing::Reference => {
                let llvm_ty = self.resolve_type(arg_ty);
                let alloca = self.builder.alloca(llvm_ty, "clos.arg");
                self.builder.store(arg, alloca);
                args.push(alloca);
                params.push(ptr_ty);
            }
            crate::codegen::abi::ParamPassing::Void => {}
            crate::codegen::abi::ParamPassing::Direct => {
                args.push(arg);
                params.push(self.resolve_type(arg_ty));
            }
        }

        let ret_ty = self.resolve_type(return_ty);
        let ret_is_indirect = crate::codegen::abi::abi_size(return_ty, self.type_info) > 16;

        if ret_is_indirect {
            let sret = self.builder.alloca(ret_ty, "clos.sret");
            self.builder
                .call_indirect_with_sret(ret_ty, &params, fn_ptr, sret, &args);
            Some(self.builder.load(ret_ty, sret, "clos.ret"))
        } else {
            self.builder
                .call_indirect(ret_ty, &params, fn_ptr, &args, "clos.ret")
        }
    }

    /// Call a closure `{fn_ptr, env_ptr}` with no user arguments.
    pub(super) fn call_closure_no_args(
        &mut self,
        closure: ValueId,
        return_ty: Idx,
    ) -> Option<ValueId> {
        let fn_ptr = self
            .builder
            .extract_value(closure, CLOSURE_FIELD_FN, "clos.fn")?;
        let env_ptr = self
            .builder
            .extract_value(closure, CLOSURE_FIELD_ENV, "clos.env")?;

        let ptr_ty = self.builder.ptr_type();
        let ret_ty = self.resolve_type(return_ty);
        let ret_is_indirect = crate::codegen::abi::abi_size(return_ty, self.type_info) > 16;

        if ret_is_indirect {
            let sret = self.builder.alloca(ret_ty, "clos.sret");
            self.builder
                .call_indirect_with_sret(ret_ty, &[ptr_ty], fn_ptr, sret, &[env_ptr]);
            Some(self.builder.load(ret_ty, sret, "clos.ret"))
        } else {
            self.builder
                .call_indirect(ret_ty, &[ptr_ty], fn_ptr, &[env_ptr], "clos.ret")
        }
    }

    /// Extract the closure's return type from the pool.
    pub(super) fn closure_return_ty(
        &self,
        arc_args: &[ArcVarId],
        arc_func: &ArcFunction,
    ) -> Option<Idx> {
        if arc_args.len() < 2 {
            return None;
        }
        let closure_ty = arc_func.var_type(arc_args[1]);
        if self.pool.tag(closure_ty) == ori_types::Tag::Function {
            Some(self.pool.function_return(closure_ty))
        } else {
            None
        }
    }
}
