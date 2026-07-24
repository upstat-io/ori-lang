//! Direct and indirect invoke emission for ARC terminators.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::{Name, CLOSURE_FIELD_ENV, CLOSURE_FIELD_FN};
use ori_types::Idx;

use crate::codegen::abi::{ParamAbi, ReturnPassing};
use crate::codegen::value_id::{BlockId, FunctionId, ValueId};

use super::context::{EmittedValue, InvokeMode};
use super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// When the callee is in [`nounwind_functions`], emits `call` + `br` instead
    /// of `invoke`, eliminating the unwind edge and its associated landing pad.
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors ArcTerminator::Invoke fields plus the owning ArcFunction context"
    )]
    pub(super) fn emit_invoke(
        &mut self,
        dst: ArcVarId,
        callee: Name,
        arc_args: &[ArcVarId],
        normal: ori_arc::ir::ArcBlockId,
        unwind: ori_arc::ir::ArcBlockId,
        arc_func: &ArcFunction,
        mono_instance_id: Option<ori_ir::canon::MonoInstanceId>,
    ) {
        let func_name_str = self.interner.lookup(callee);
        let normal_block = self.block(normal);
        let is_nounwind = self.ctx.nounwind_functions.contains(&callee);
        // Why: An ARC unwind block with no effective cleanup has no emitted LLVM block.
        let unwind_is_empty_cleanup =
            !super::dead_unwind::has_effective_cleanup(&arc_func.blocks[unwind.index()], arc_func);
        let callee_intercepted = self.callee_will_be_intercepted(callee, arc_args, arc_func);
        let mode = if is_nounwind || unwind_is_empty_cleanup || callee_intercepted {
            InvokeMode::Call {
                normal: normal_block,
            }
        } else {
            // Why: Resolving a dead unwind block would address an LLVM block that was never emitted.
            let unwind_block = self.block(unwind);
            InvokeMode::Invoke {
                normal: normal_block,
                unwind: unwind_block,
            }
        };

        // INVARIANT: Intercepted calls retain the live ARC cleanup edge across every interception tier.
        self.intercepted_unwind = (callee_intercepted
            && !unwind_is_empty_cleanup
            && self.current_cleanup_pad.is_none()
            && self.intercept_routes_unwind(callee, dst, arc_args, arc_func))
        .then(|| self.block(unwind));

        let runtime_projection_allowed = self.runtime_projection_allowed(arc_func, dst);
        if self.try_emit_invoke_runtime_projection(
            dst,
            callee,
            arc_args,
            normal_block,
            arc_func,
            runtime_projection_allowed,
        ) {
            self.intercepted_unwind = None;
            return;
        }

        let arg_vals: Vec<ValueId> = arc_args.iter().map(|a| self.var(*a)).collect();

        let resolved = self.resolve_callee(callee, arc_args, dst, arc_func, mono_instance_id);

        if let Some((func_id, params, ret_abi)) = resolved {
            self.emit_abi_resolved_call(
                dst,
                func_id,
                &params,
                ret_abi.passing,
                ret_abi.ty,
                &arg_vals,
                arc_args,
                mode,
                arc_func,
            );
            return;
        }

        if runtime_projection_allowed && self.try_emit_protocol(dst, callee, arc_args, arc_func) {
            self.intercepted_unwind = None;
            self.br_outside_cleanup_pad(normal_block);
            self.builder.position_at_end(normal_block);
            return;
        }
        let builtin_val = runtime_projection_allowed.then(|| {
            self.try_emit_builtin_method(callee, arc_args, arc_func, arc_func.var_type(dst))
                .or_else(|| {
                    self.try_emit_builtin_associated(callee, arc_args, arc_func.var_type(dst))
                })
        });
        let builtin_val = builtin_val.flatten();
        self.intercepted_unwind = None;

        if let Some(val) = builtin_val {
            self.br_outside_cleanup_pad(normal_block);
            self.builder.position_at_end(normal_block);
            self.def_var_repr(dst, val, arc_func);
        } else if let Some(func_id) = runtime_projection_allowed
            .then(|| self.builder.try_runtime_fn(func_name_str))
            .flatten()
        {
            self.emit_runtime_fn_call(dst, func_id, callee, arc_args, &arg_vals, mode, arc_func);
        } else {
            let msg = self.unresolved_direct_call_message(arc_func, dst, func_name_str, "invoke");
            tracing::warn!("{msg}");
            self.br_outside_cleanup_pad(normal_block);
            self.builder.position_at_end(normal_block);
            let unit = self.builder.const_i64(0);
            self.def_var(dst, EmittedValue::Immediate(unit));
            self.builder.record_codegen_error_with_msg(msg);
        }
    }

    /// Emit runtime-backed direct-call projections before closed target resolution.
    fn try_emit_invoke_runtime_projection(
        &mut self,
        dst: ArcVarId,
        callee: Name,
        arc_args: &[ArcVarId],
        normal_block: BlockId,
        arc_func: &ArcFunction,
        runtime_projection_allowed: bool,
    ) -> bool {
        if !runtime_projection_allowed {
            return false;
        }

        // INVARIANT: Format, prelude, and traceless projections precede same-named declarations.
        let callee_name = self.interner.lookup(callee);
        let value = self
            .try_emit_format_call(callee, arc_args, arc_func)
            .or_else(|| {
                super::builtins::prelude::try_emit_prelude_function(
                    self,
                    callee_name,
                    arc_args,
                    arc_func,
                )
            })
            .or_else(|| {
                self.try_emit_traceless_traceable(
                    callee,
                    arc_args,
                    arc_func,
                    arc_func.var_type(dst),
                )
            });

        let Some(value) = value else {
            return false;
        };
        self.br_outside_cleanup_pad(normal_block);
        self.builder.position_at_end(normal_block);
        self.def_var_repr(dst, value, arc_func);
        true
    }

    /// Emit an `InvokeIndirect` terminator through a closure fat pointer.
    ///
    /// The emitter extracts the function/environment pointers, follows the
    /// closure return ABI, and selects `invoke` only when the unwind block has
    /// live cleanup.
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors ArcTerminator::InvokeIndirect fields plus the owning ArcFunction context"
    )]
    pub(super) fn emit_invoke_indirect(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        closure: ArcVarId,
        args: &[ArcVarId],
        normal: ori_arc::ir::ArcBlockId,
        unwind: ori_arc::ir::ArcBlockId,
        arc_func: &ArcFunction,
    ) {
        let closure_val = self.var(closure);
        let normal_block = self.block(normal);
        let unwind_is_empty_cleanup =
            !super::dead_unwind::has_effective_cleanup(&arc_func.blocks[unwind.index()], arc_func);

        let fn_ptr = self
            .builder
            .extract_value(closure_val, CLOSURE_FIELD_FN, "icall.fn_ptr");
        let env_ptr = self
            .builder
            .extract_value(closure_val, CLOSURE_FIELD_ENV, "icall.env_ptr");

        let (Some(fn_ptr), Some(env_ptr)) = (fn_ptr, env_ptr) else {
            tracing::error!(
                closure_var = closure.raw(),
                "emit_invoke_indirect: extract_value failed"
            );
            self.builder.record_codegen_error();
            self.builder.br(normal_block);
            self.builder.position_at_end(normal_block);
            let unit = self.builder.const_i64(0);
            self.def_var(dst, EmittedValue::Immediate(unit));
            return;
        };

        let (arg_vals, param_types) = self.marshal_indirect_call_args(env_ptr, args, arc_func);

        let ret_ty = self.resolve_type(ty);
        let ret_is_indirect =
            crate::codegen::abi::abi_size(ty, self.type_info, self.repr_plan) > 16;

        if unwind_is_empty_cleanup {
            self.emit_invoke_indirect_call(
                dst,
                ret_ty,
                ret_is_indirect,
                fn_ptr,
                &param_types,
                &arg_vals,
                arc_func,
            );
            self.br_outside_cleanup_pad(normal_block);
            self.builder.position_at_end(normal_block);
        } else {
            let unwind_block = self.block(unwind);
            self.emit_invoke_indirect_invoke(
                dst,
                ret_ty,
                ret_is_indirect,
                fn_ptr,
                &param_types,
                &arg_vals,
                normal_block,
                unwind_block,
                arc_func,
            );
        }
    }

    /// Emit an indirect call without an unwind edge and define `dst`.
    #[expect(
        clippy::too_many_arguments,
        reason = "matches LLVM indirect-call operands plus ARC destination and function context"
    )]
    fn emit_invoke_indirect_call(
        &mut self,
        dst: ArcVarId,
        ret_ty: crate::codegen::value_id::LLVMTypeId,
        ret_is_indirect: bool,
        fn_ptr: ValueId,
        param_types: &[crate::codegen::value_id::LLVMTypeId],
        arg_vals: &[ValueId],
        arc_func: &ArcFunction,
    ) {
        if ret_is_indirect {
            let sret_alloca = self.builder.alloca(ret_ty, "icall.sret");
            if let Some(pad) = self.current_cleanup_pad {
                self.builder.call_indirect_with_sret_and_funclet(
                    ret_ty,
                    param_types,
                    fn_ptr,
                    sret_alloca,
                    arg_vals,
                    pad,
                );
            } else {
                self.builder.call_indirect_with_sret(
                    ret_ty,
                    param_types,
                    fn_ptr,
                    sret_alloca,
                    arg_vals,
                );
            }
            let loaded = self.builder.load(ret_ty, sret_alloca, "icall.sret.load");
            self.def_var_repr(dst, loaded, arc_func);
        } else if let Some(pad) = self.current_cleanup_pad {
            let result = self.builder.call_indirect_with_funclet(
                ret_ty,
                param_types,
                fn_ptr,
                arg_vals,
                pad,
                "icall",
            );
            if let Some(val) = result {
                self.def_var_repr(dst, val, arc_func);
            }
        } else {
            let result = self
                .builder
                .call_indirect(ret_ty, param_types, fn_ptr, arg_vals, "icall");
            if let Some(val) = result {
                self.def_var_repr(dst, val, arc_func);
            }
        }
    }

    /// Emit an indirect invoke with a live unwind edge and define `dst`.
    ///
    /// Both return shapes use a real LLVM `invoke` instruction: direct
    /// returns via `invoke_indirect`, sret returns via
    /// `invoke_indirect_with_sret` (sret attribute on the first parameter,
    /// result loaded on the normal edge).
    #[expect(
        clippy::too_many_arguments,
        reason = "matches LLVM indirect-invoke operands, both control-flow edges, and ARC context"
    )]
    fn emit_invoke_indirect_invoke(
        &mut self,
        dst: ArcVarId,
        ret_ty: crate::codegen::value_id::LLVMTypeId,
        ret_is_indirect: bool,
        fn_ptr: ValueId,
        param_types: &[crate::codegen::value_id::LLVMTypeId],
        arg_vals: &[ValueId],
        normal_block: BlockId,
        unwind_block: BlockId,
        arc_func: &ArcFunction,
    ) {
        if ret_is_indirect {
            // INVARIANT: Sret calls retain a real unwind edge for enclosing catch handlers.
            let sret_alloca = self.builder.alloca(ret_ty, "icall.sret");
            self.builder.invoke_indirect_with_sret(
                ret_ty,
                param_types,
                fn_ptr,
                sret_alloca,
                arg_vals,
                normal_block,
                unwind_block,
            );
            self.builder.position_at_end(normal_block);
            let loaded = self.builder.load(ret_ty, sret_alloca, "icall.sret.load");
            self.def_var_repr(dst, loaded, arc_func);
        } else {
            let result = self.builder.invoke_indirect(
                ret_ty,
                param_types,
                fn_ptr,
                arg_vals,
                normal_block,
                unwind_block,
                "icall",
            );
            self.builder.position_at_end(normal_block);
            if let Some(val) = result {
                self.def_var_repr(dst, val, arc_func);
            }
        }
    }

    /// Emit an ABI-aware call/invoke for a resolved callee.
    ///
    /// Handles sret return passing (stack-allocated return slot), direct returns,
    /// and void returns. Defines the destination variable with the appropriate
    /// representation.
    #[expect(
        clippy::too_many_arguments,
        reason = "matches the resolved parameter/return ABI, invoke mode, and ARC call-site context"
    )]
    fn emit_abi_resolved_call(
        &mut self,
        dst: ArcVarId,
        func_id: FunctionId,
        params: &[ParamAbi],
        ret_passing: ReturnPassing,
        ret_ty_idx: Idx,
        arg_vals: &[ValueId],
        arc_vars: &[ArcVarId],
        mode: InvokeMode,
        arc_func: &ArcFunction,
    ) {
        let passed_args = self.apply_param_passing(arg_vals, Some(arc_vars), params);
        let result = match &ret_passing {
            ReturnPassing::Sret { .. } => {
                let ret_ty = self.resolve_type(ret_ty_idx);
                let sret_alloca =
                    self.builder
                        .create_entry_alloca(self.current_function, "sret.tmp", ret_ty);
                let mut full_args = vec![sret_alloca];
                full_args.extend_from_slice(&passed_args);
                let call_result = self.call_or_invoke_llvm(func_id, &full_args, mode, "call");
                assert!(
                    call_result.is_none(),
                    "an sret call must not produce a direct LLVM return value"
                );
                self.builder.position_at_end(mode.normal_block());
                Some(self.builder.load(ret_ty, sret_alloca, "sret.load"))
            }
            ReturnPassing::Direct | ReturnPassing::Void => {
                let result = self.call_or_invoke_llvm(func_id, &passed_args, mode, "call");
                self.builder.position_at_end(mode.normal_block());
                result
            }
        };
        if let Some(val) = result {
            self.def_var_repr(dst, val, arc_func);
        } else {
            // INVARIANT: Every ARC invoke defines its SSA destination, including void callees.
            let unit = self.builder.const_i64(0);
            self.def_var(dst, EmittedValue::Immediate(unit));
        }
    }

    /// Emit a runtime function call with aggregate-to-pointer coercion.
    ///
    /// Runtime functions (`ori_*`) take `ptr` params, but ARC IR passes aggregate
    /// structs (Str, List, etc.) by value — this helper coerces each arg as needed.
    #[expect(
        clippy::too_many_arguments,
        reason = "matches runtime callee identity, raw/coerced arguments, invoke mode, and ARC context"
    )]
    fn emit_runtime_fn_call(
        &mut self,
        dst: ArcVarId,
        func_id: FunctionId,
        callee: Name,
        arc_args: &[ArcVarId],
        arg_vals: &[ValueId],
        mode: InvokeMode,
        arc_func: &ArcFunction,
    ) {
        let coerced_args = self.coerce_runtime_fn_args(callee, arc_args, arg_vals, arc_func);

        if let Some(val) = self.call_or_invoke_llvm(func_id, &coerced_args, mode, "call") {
            self.builder.position_at_end(mode.normal_block());
            self.def_var_repr(dst, val, arc_func);
        } else {
            self.builder.position_at_end(mode.normal_block());
            let unit = self.builder.const_i64(0);
            self.def_var(dst, EmittedValue::Immediate(unit));
        }
    }
}
