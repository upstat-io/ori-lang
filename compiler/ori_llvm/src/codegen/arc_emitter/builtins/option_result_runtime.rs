//! Option/Result construction, checked branches, and closure calls.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::{CLOSURE_FIELD_ENV, CLOSURE_FIELD_FN};
use ori_types::Idx;

use crate::codegen::value_id::ValueId;

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
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

    /// Emit a conditional panic branch for `unwrap`/`unwrap_err` and
    /// range-checked casts.
    ///
    /// If `is_valid` is false, calls `ori_panic_cstr` with the given static
    /// message and halts. If true, falls through to the continue block
    /// (positioned on return). The caller extracts the payload after this.
    pub(in crate::codegen::arc_emitter) fn emit_unwrap_branch(
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

        self.builder.position_at_end(panic_bb);
        let msg_ptr = self
            .builder
            .build_global_string_ptr(panic_msg, &format!("{label}.msg"));
        let panic_fn = self.builder.runtime_fn("ori_panic_cstr");
        self.emit_rt_call(panic_fn, &[msg_ptr], "");
        self.builder.unreachable();

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
        // INVARIANT: ARC cannot fund this hidden owning panic call, so its borrowed message is retained here.
        self.emit_slice_aware_rc_inc(msg, ori_types::Idx::STR);
        let panic_fn = self.builder.runtime_fn("ori_panic");
        self.emit_rt_call(panic_fn, &[msg_alloca], "");
        self.builder.unreachable();

        self.builder.position_at_end(ok_bb);
        Some(())
    }

    // Closure calls

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

        let passing = crate::codegen::abi::compute_closure_param_passing(
            arg_ty,
            self.type_info,
            self.repr_plan,
            self.classifier,
        );
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
