//! Helper methods for Option/Result LLVM codegen.
//!
//! Contains: niche dispatch, expect panic branches, and closure call utilities.
//! Debug formatting lives in `debug_helpers.rs`.

#[cfg(test)]
mod tests;

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::{CLOSURE_FIELD_ENV, CLOSURE_FIELD_FN};
use ori_types::Idx;

use crate::codegen::arc_emitter::tag_access::TagEncoding;
use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Niche-encoded Option dispatch (§07.2).
    ///
    /// **BUG-04-019 fix (2026-04-07)**: `unwrap`/`expect`/`unwrap_or` now mirror
    /// the explicit-tag pattern from `option_result.rs` — tag guard via
    /// `emit_unwrap_branch`/`emit_expect_branch` and `inc_value_rc` on the
    /// extracted payload to retain the inner heap data through the new
    /// owning reference.
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
                let is_some = self.compute_option_is_some(receiver, niche_idx, niche_value)?;
                Some(is_some)
            }
            "is_none" => {
                let field = self
                    .builder
                    .extract_value(receiver, niche_idx, "opt.niche")?;
                Some(self.niche_is_sentinel(field, niche_value, "is_none"))
            }
            "unwrap" => {
                let is_some = self.compute_option_is_some(receiver, niche_idx, niche_value)?;
                self.emit_unwrap_branch(
                    is_some,
                    "called `Option.unwrap()` on a `None` value",
                    "opt_unwrap_niche",
                )?;
                // After unwrap branch, guaranteed Some — retain unconditionally.
                let payload = self.builder.extract_value(receiver, 0, "opt.payload")?;
                let inner_ty = self.pool.option_inner(self.pool.resolve_fully(receiver_ty));
                self.inc_value_rc(payload, inner_ty, 1);
                Some(payload)
            }
            "unwrap_or" if arg_vals.len() >= 2 => {
                let is_some = self.compute_option_is_some(receiver, niche_idx, niche_value)?;
                let payload = self.builder.extract_value(receiver, 0, "opt.payload")?;

                // RC-retain payload when Some: the select copies payload bytes,
                // creating a second reference to inner RC data. Skip for scalar
                // payloads (no RC) to avoid extra blocks that break PHI nodes
                // when unwrap_or is inside a short-circuit `&&`/`||` branch.
                let inner_ty = self.pool.option_inner(self.pool.resolve_fully(receiver_ty));
                let needs_rc = !self.classifier.is_scalar(inner_ty);
                if needs_rc {
                    let inc_bb = self
                        .builder
                        .append_block(self.current_function, "uor.niche.inc");
                    let merge_bb = self
                        .builder
                        .append_block(self.current_function, "uor.niche.merge");
                    self.builder.cond_br(is_some, inc_bb, merge_bb);
                    self.builder.position_at_end(inc_bb);
                    self.inc_value_rc(payload, inner_ty, 1);
                    self.builder.br(merge_bb);
                    self.builder.position_at_end(merge_bb);
                }

                Some(
                    self.builder
                        .select(is_some, payload, arg_vals[1], "unwrap_or"),
                )
            }
            "expect" if arg_vals.len() >= 2 => {
                let is_some = self.compute_option_is_some(receiver, niche_idx, niche_value)?;
                self.emit_expect_branch(is_some, arg_vals[1], "expect")?;
                // After expect branch, guaranteed Some — retain unconditionally.
                let payload = self.builder.extract_value(receiver, 0, "opt.payload")?;
                let inner_ty = self.pool.option_inner(self.pool.resolve_fully(receiver_ty));
                self.inc_value_rc(payload, inner_ty, 1);
                Some(payload)
            }
            "debug" | "to_str" => {
                let is_some = self.compute_option_is_some(receiver, niche_idx, niche_value)?;
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

    /// Compute the `is_some` predicate for a niche-encoded Option.
    ///
    /// Loads the niche field, compares it against the sentinel via
    /// [`niche_is_sentinel`](Self::niche_is_sentinel) (which yields
    /// `is_none`), then inverts the polarity. Used by every Option
    /// niche helper that needs the variant predicate.
    fn compute_option_is_some(
        &mut self,
        receiver: ValueId,
        niche_idx: u32,
        niche_value: u64,
    ) -> Option<ValueId> {
        let field = self
            .builder
            .extract_value(receiver, niche_idx, "opt.niche")?;
        let is_niche = self.niche_is_sentinel(field, niche_value, "is_niche");
        let t = self.builder.const_bool(true);
        let f = self.builder.const_bool(false);
        Some(self.builder.select(is_niche, f, t, "is_some"))
    }

    /// Niche-encoded Result dispatch (§07.2).
    ///
    /// **BUG-04-019 fix (2026-04-07)**: Previously, `unwrap`/`unwrap_err`/`unwrap_or`
    /// were collapsed into a single match arm that ignored the method name and
    /// returned the raw payload without any tag guard or RC retain. The
    /// `expect`/`expect_err` arms had the same defects. This implementation now
    /// mirrors the explicit-tag pattern from `option_result.rs` — each method
    /// computes its own niche-aware variant predicate, calls
    /// `emit_unwrap_branch`/`emit_expect_branch` (panic on the wrong variant
    /// for unwrap-style) or a conditional `cond_br` (for `unwrap_or`), then
    /// extracts the payload and retains it via `inc_value_rc` so the wrapper's
    /// inner heap data is properly refcounted through the new owning reference.
    pub(super) fn emit_result_niche(
        &mut self,
        method: &str,
        receiver: ValueId,
        arg_vals: &[ValueId],
        receiver_ty: Idx,
        encoding: &TagEncoding,
    ) -> Option<ValueId> {
        let niche_idx = encoding.niche_field_index().unwrap();
        let niche_value = encoding.niche_value().unwrap();
        let niche_variant_idx = encoding.niche_variant_idx().unwrap();
        match method {
            "is_ok" => {
                self.compute_result_is_ok(receiver, niche_idx, niche_value, niche_variant_idx)
            }
            "is_err" => {
                self.compute_result_is_err(receiver, niche_idx, niche_value, niche_variant_idx)
            }
            "unwrap" => {
                let is_ok =
                    self.compute_result_is_ok(receiver, niche_idx, niche_value, niche_variant_idx)?;
                self.emit_unwrap_branch(
                    is_ok,
                    "called `Result.unwrap()` on an `Err` value",
                    "res_unwrap_niche",
                )?;
                // After unwrap branch, guaranteed Ok — retain unconditionally.
                let payload = self.builder.extract_value(receiver, 0, "res.payload")?;
                let TypeInfo::Result { ok: ok_ty, .. } = self.type_info.get(receiver_ty) else {
                    return Some(payload);
                };
                self.inc_value_rc(payload, ok_ty, 1);
                Some(payload)
            }
            "unwrap_err" => {
                let is_err = self.compute_result_is_err(
                    receiver,
                    niche_idx,
                    niche_value,
                    niche_variant_idx,
                )?;
                self.emit_unwrap_branch(
                    is_err,
                    "called `Result.unwrap_err()` on an `Ok` value",
                    "res_unwrap_err_niche",
                )?;
                // After unwrap branch, guaranteed Err — retain unconditionally.
                let payload = self.builder.extract_value(receiver, 0, "res.payload")?;
                let TypeInfo::Result { err: err_ty, .. } = self.type_info.get(receiver_ty) else {
                    return Some(payload);
                };
                self.inc_value_rc(payload, err_ty, 1);
                Some(payload)
            }
            "unwrap_or" if arg_vals.len() >= 2 => {
                let is_ok =
                    self.compute_result_is_ok(receiver, niche_idx, niche_value, niche_variant_idx)?;
                let payload = self.builder.extract_value(receiver, 0, "res.payload")?;

                // RC-retain payload when Ok: the select copies payload bytes.
                if let TypeInfo::Result { ok: ok_ty, .. } = self.type_info.get(receiver_ty) {
                    if !self.classifier.is_scalar(ok_ty) {
                        let inc_bb = self
                            .builder
                            .append_block(self.current_function, "ruor.niche.inc");
                        let merge_bb = self
                            .builder
                            .append_block(self.current_function, "ruor.niche.merge");
                        self.builder.cond_br(is_ok, inc_bb, merge_bb);
                        self.builder.position_at_end(inc_bb);
                        self.inc_value_rc(payload, ok_ty, 1);
                        self.builder.br(merge_bb);
                        self.builder.position_at_end(merge_bb);
                    }
                }

                Some(
                    self.builder
                        .select(is_ok, payload, arg_vals[1], "unwrap_or"),
                )
            }
            "expect" if arg_vals.len() >= 2 => {
                let is_ok =
                    self.compute_result_is_ok(receiver, niche_idx, niche_value, niche_variant_idx)?;
                self.emit_expect_branch(is_ok, arg_vals[1], "res_expect_niche")?;
                // After expect branch, guaranteed Ok — retain unconditionally.
                let payload = self.builder.extract_value(receiver, 0, "res.payload")?;
                let TypeInfo::Result { ok: ok_ty, .. } = self.type_info.get(receiver_ty) else {
                    return Some(payload);
                };
                self.inc_value_rc(payload, ok_ty, 1);
                Some(payload)
            }
            "expect_err" if arg_vals.len() >= 2 => {
                let is_err = self.compute_result_is_err(
                    receiver,
                    niche_idx,
                    niche_value,
                    niche_variant_idx,
                )?;
                self.emit_expect_branch(is_err, arg_vals[1], "res_expect_err_niche")?;
                // After expect branch, guaranteed Err — retain unconditionally.
                let payload = self.builder.extract_value(receiver, 0, "res.payload")?;
                let TypeInfo::Result { err: err_ty, .. } = self.type_info.get(receiver_ty) else {
                    return Some(payload);
                };
                self.inc_value_rc(payload, err_ty, 1);
                Some(payload)
            }
            "clone" => Some(receiver),
            _ => None,
        }
    }

    /// Compute the `is_ok` predicate for a niche-encoded Result.
    ///
    /// Loads the niche field, compares against the sentinel, and inverts based
    /// on whether Ok or Err is the niche variant. When `niche_variant_idx == 0`
    /// (Ok is the niche variant), `is_ok = is_niche`; otherwise `is_ok = !is_niche`.
    fn compute_result_is_ok(
        &mut self,
        receiver: ValueId,
        niche_idx: u32,
        niche_value: u64,
        niche_variant_idx: u32,
    ) -> Option<ValueId> {
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

    /// Compute the `is_err` predicate for a niche-encoded Result.
    ///
    /// Mirror of [`compute_result_is_ok`](Self::compute_result_is_ok) — when
    /// `niche_variant_idx == 1` (Err is the niche variant), `is_err = is_niche`;
    /// otherwise `is_err = !is_niche`.
    fn compute_result_is_err(
        &mut self,
        receiver: ValueId,
        niche_idx: u32,
        niche_value: u64,
        niche_variant_idx: u32,
    ) -> Option<ValueId> {
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

    /// Emit a conditional panic branch for `unwrap`/`unwrap_err`.
    ///
    /// If `is_valid` is false, calls `ori_panic_cstr` with the given static
    /// message and halts. If true, falls through to the continue block
    /// (positioned on return). The caller extracts the payload after this.
    pub(super) fn emit_unwrap_branch(
        &mut self,
        is_valid: ValueId,
        panic_msg: &str,
        label: &str,
    ) -> Option<()> {
        let ok_bb = self
            .builder
            .append_block(self.current_function, &format!("{label}.ok"));
        let panic_bb = self
            .builder
            .append_block(self.current_function, &format!("{label}.panic"));

        self.builder.cond_br(is_valid, ok_bb, panic_bb);

        // Panic block: global C string + ori_panic_cstr.
        self.builder.position_at_end(panic_bb);
        let msg_ptr = self
            .builder
            .build_global_string_ptr(panic_msg, &format!("{label}.msg"));
        let panic_fn = self.builder.runtime_fn("ori_panic_cstr");
        self.emit_rt_call(panic_fn, &[msg_ptr], "");
        self.builder.unreachable();

        // Continue block: position here for payload extraction.
        self.builder.position_at_end(ok_bb);
        Some(())
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

        let passing =
            crate::codegen::abi::compute_param_passing(arg_ty, self.type_info, self.repr_plan);
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
        let ret_is_indirect =
            crate::codegen::abi::abi_size(return_ty, self.type_info, self.repr_plan) > 16;

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
        let ret_is_indirect =
            crate::codegen::abi::abi_size(return_ty, self.type_info, self.repr_plan) > 16;

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
