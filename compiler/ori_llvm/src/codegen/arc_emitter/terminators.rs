//! Terminator emission for the ARC IR emitter.
//!
//! Translates [`ArcTerminator`] variants into LLVM control flow: `ret`, `br`,
//! `cond_br`, `switch`, `invoke`/`call`, `resume`, and `unreachable`.

use ori_arc::ir::{ArcBlockId, ArcFunction, ArcTerminator, ArcVarId};
use ori_ir::{Name, CLOSURE_FIELD_ENV, CLOSURE_FIELD_FN};
use ori_types::Idx;
use rustc_hash::FxHashMap;

use super::context::{EmittedValue, InvokeMode};
use super::ArcIrEmitter;
use crate::codegen::abi::{FunctionAbi, ParamAbi, ReturnPassing};
use crate::codegen::eh_model::EhModel;
use crate::codegen::value_id::{BlockId, FunctionId, ValueId};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit an `ArcTerminator` as LLVM control flow.
    pub(super) fn emit_terminator(
        &mut self,
        term: &ArcTerminator,
        current_block: ArcBlockId,
        _phi_nodes: &[Vec<(ArcVarId, ValueId)>],
        abi: &FunctionAbi,
        landingpad_values: &FxHashMap<usize, ValueId>,
        arc_func: &ArcFunction,
    ) {
        tracing::trace!(?term, block = current_block.index(), "emit_terminator");
        match term {
            ArcTerminator::Return { value } => self.emit_return_terminator(*value, abi),
            ArcTerminator::Jump { target, args } => {
                self.emit_jump_terminator(*target, args, arc_func);
            }
            ArcTerminator::Branch {
                cond,
                then_block,
                else_block,
            } => {
                debug_assert!(
                    self.current_cleanup_pad.is_none(),
                    "Branch terminator inside SEH funclet"
                );
                let condition = self.var(*cond);
                self.builder
                    .cond_br(condition, self.block(*then_block), self.block(*else_block));
            }
            ArcTerminator::Switch {
                scrutinee,
                cases,
                default,
            } => self.emit_switch_terminator(*scrutinee, cases, *default),
            ArcTerminator::Invoke { .. } => {
                self.emit_direct_invoke_terminator(term, arc_func);
            }
            ArcTerminator::InvokeIndirect {
                dst,
                ty,
                closure,
                args,
                normal,
                unwind,
                ..
            } => {
                self.emit_invoke_indirect(*dst, *ty, *closure, args, *normal, *unwind, arc_func);
            }
            ArcTerminator::Resume => {
                self.emit_resume_terminator(current_block, landingpad_values);
            }
            ArcTerminator::Unreachable => self.builder.unreachable(),
        }
    }

    fn emit_return_terminator(&mut self, value: ArcVarId, abi: &FunctionAbi) {
        let value = self.var(value);
        match abi.return_abi.passing {
            ReturnPassing::Sret { .. } => {
                if self.sret_forwarded_result != Some(value) {
                    let output = self.builder.get_param(self.current_function, 0);
                    self.builder.store(value, output);
                }
                self.builder.ret_void();
            }
            ReturnPassing::Direct => self.builder.ret(value),
            ReturnPassing::Void => self.builder.ret_void(),
        }
    }

    fn emit_jump_terminator(
        &mut self,
        target: ArcBlockId,
        args: &[ArcVarId],
        arc_func: &ArcFunction,
    ) {
        let target_index = target.index();
        debug_assert_eq!(
            args.len(),
            arc_func.blocks[target_index].params.len(),
            "Jump arg count must match target block param count (block {target_index})"
        );
        if self.current_cleanup_pad.take().is_some() {
            self.builder.record_codegen_error_with_msg(
                "Jump terminator inside cleanuppad; cleanup pads must exit with Resume",
            );
            self.builder.unreachable();
            return;
        }
        if args.is_empty() {
            self.builder.br(self.block(target));
            return;
        }

        let Some(source_block) = self.builder.current_block() else {
            tracing::error!("ARC jump: no current block — skipping phi incoming");
            self.builder.record_codegen_error();
            self.builder.br(self.block(target));
            return;
        };
        for (param_index, &arg) in args.iter().enumerate() {
            let value = self.var(arg);
            let target_var = arc_func.blocks[target_index].params[param_index].0;
            let value = if let Some(&width) = self.narrowed_vars.get(&target_var) {
                let narrow_ty = self.llvm_type_for_int_width(width);
                self.builder.trunc(value, narrow_ty, "phi.trunc")
            } else {
                value
            };
            self.phi_incoming
                .push((target_index, param_index, value, source_block));
        }
        self.builder.br(self.block(target));
    }

    fn emit_switch_terminator(
        &mut self,
        scrutinee: ArcVarId,
        cases: &[(u64, ArcBlockId)],
        default: ArcBlockId,
    ) {
        debug_assert!(
            self.current_cleanup_pad.is_none(),
            "Switch terminator inside SEH funclet"
        );
        let value = self.var(scrutinee);
        let niche_encoding = self
            .niche_scrutinees
            .get(&scrutinee)
            .copied()
            .and_then(|enum_ty| self.get_niche_encoding(enum_ty));
        if let Some(encoding) = niche_encoding {
            self.emit_niche_switch(value, &encoding, cases, default);
            return;
        }

        let llvm_cases = cases
            .iter()
            .map(|&(tag, block)| {
                let tag_value = self.builder.const_int_matching(value, tag);
                (tag_value, self.block(block))
            })
            .collect::<Vec<_>>();
        self.builder.switch(value, self.block(default), &llvm_cases);
    }

    fn emit_direct_invoke_terminator(&mut self, term: &ArcTerminator, arc_func: &ArcFunction) {
        let ArcTerminator::Invoke {
            dst,
            func,
            args,
            mono_instance_id,
            normal,
            unwind,
            ..
        } = term
        else {
            unreachable!("direct invoke helper requires Invoke")
        };

        let is_nounwind = self.ctx.nounwind_functions.contains(func);
        let is_seh_catch = !is_nounwind
            && self.builder.eh_model() == EhModel::Seh
            && !matches!(
                arc_func.blocks[unwind.index()].terminator,
                ArcTerminator::Resume
            );
        if is_seh_catch {
            self.emit_seh_catch_invoke(
                *dst,
                *func,
                args,
                *normal,
                *unwind,
                arc_func,
                *mono_instance_id,
            );
        } else {
            self.emit_invoke(
                *dst,
                *func,
                args,
                *normal,
                *unwind,
                arc_func,
                *mono_instance_id,
            );
        }
    }

    fn emit_resume_terminator(
        &mut self,
        current_block: ArcBlockId,
        landingpad_values: &FxHashMap<usize, ValueId>,
    ) {
        match self.builder.eh_model() {
            EhModel::Itanium => {
                if let Some(&landingpad) = landingpad_values.get(&current_block.index()) {
                    self.builder.resume(landingpad);
                } else {
                    tracing::warn!(
                        block = current_block.index(),
                        "ARC Resume without landingpad — emitting unreachable"
                    );
                    self.builder.unreachable();
                }
            }
            EhModel::Seh => {
                if let Some(pad) = self.current_cleanup_pad.take() {
                    self.builder.cleanupret(pad, None);
                } else {
                    tracing::warn!(
                        block = current_block.index(),
                        "ARC Resume without cleanuppad — emitting unreachable"
                    );
                    self.builder.unreachable();
                }
            }
        }
    }

    /// Emit a niche-aware switch as `icmp eq` + `cond_br`.
    ///
    /// For 2-variant niche enums (Option, Result): compare the raw niche field
    /// value against the niche sentinel, then branch to the niche variant block
    /// or the data variant block.
    fn emit_niche_switch(
        &mut self,
        scrut_val: ValueId,
        encoding: &super::tag_access::TagEncoding,
        cases: &[(u64, ori_arc::ir::ArcBlockId)],
        default: ori_arc::ir::ArcBlockId,
    ) {
        let niche_value = encoding.niche_value().unwrap();
        let niche_variant_idx = encoding.niche_variant_idx().unwrap();

        // Find blocks for each logical variant.
        let niche_block = cases
            .iter()
            .find(|(tag, _)| *tag == u64::from(niche_variant_idx))
            .map(|(_, b)| self.block(*b));
        let data_block = cases
            .iter()
            .find(|(tag, _)| *tag != u64::from(niche_variant_idx))
            .map_or_else(|| self.block(default), |(_, b)| self.block(*b));

        let is_niche = self.niche_is_sentinel(scrut_val, niche_value, "is.niche");
        if let Some(nb) = niche_block {
            self.builder.cond_br(is_niche, nb, data_block);
        } else {
            // No case for the niche variant — branch directly to data.
            self.builder.br(data_block);
        }
    }

    /// When the callee is in [`nounwind_functions`], emits `call` + `br` instead
    /// of `invoke`, eliminating the unwind edge and its associated landing pad.
    #[expect(
        clippy::too_many_arguments,
        reason = "Invoke emission threads dst/callee/args/edges/mono_id"
    )]
    fn emit_invoke(
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
        // An unwind block with no effective cleanup (empty body or only
        // no-op RcDecs on non-capturing closures + Resume terminator) has
        // no LLVM basic block — emit_function marks it dead.
        // Using `call` instead of `invoke` is safe because there's nothing
        // to unwind through.
        let unwind_is_empty_cleanup =
            !super::dead_unwind::has_effective_cleanup(&arc_func.blocks[unwind.index()], arc_func);
        // Builtin handlers (format calls, prelude functions, builtin methods)
        // always emit `call`, not `invoke`. Their unwind blocks are dead —
        // emit_function already skipped creating LLVM blocks for them.
        let callee_intercepted = self.callee_will_be_intercepted(callee, arc_args, arc_func);
        let mode = if is_nounwind || unwind_is_empty_cleanup || callee_intercepted {
            InvokeMode::Call {
                normal: normal_block,
            }
        } else {
            // Only resolve unwind block when actually needed — dead unwind
            // blocks have no LLVM basic block and would panic in block.
            let unwind_block = self.block(unwind);
            InvokeMode::Invoke {
                normal: normal_block,
                unwind: unwind_block,
            }
        };

        // Arm intercepted unwind routing before every interception tier,
        // including prelude functions. Checked `int`/`byte` conversions live
        // in that early tier and must retain their cleanup/catch edge.
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

        // Arm the intercepted-unwind route: when the intercepted emission
        // calls a panicking runtime function (list `updated` / `__index`
        // OOB, Option/Result `unwrap` / `expect` wrong-variant), its
        // `emit_rt_call` targets the live ARC unwind block via `invoke`
        // so cleanup decs run on the panic path and the panic lands in an
        // enclosing `catch(expr:)` handler. Same predicate as
        // `detect_dead_unwind_blocks` (the block is live iff this fires).
        // Internal protocol intercepts: `__index` arrives as an Invoke when
        // the receiver can panic (list OOB → `ori_list_get`); the armed
        // route above sends that runtime call through `invoke`.
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
            // Builtin method handled inline — branch to normal block
            // (the current block needs a terminator since we skipped invoke)
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
            // Emit a branch to the normal block so the IR stays well-formed
            // (every block must have a terminator).
            self.br_outside_cleanup_pad(normal_block);
            self.builder.position_at_end(normal_block);
            // Bind dst to unit constant so successor blocks don't crash
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

        // Format, prelude, and traceless accessors are ordered to match Apply
        // emission and must run before a same-named declaration is resolved.
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

    /// Emit an `InvokeIndirect` terminator — indirect call through a closure
    /// fat pointer that may unwind.
    ///
    /// Mirrors `emit_apply_indirect` for the call mechanics (extract `fn_ptr` +
    /// `env_ptr`, build param types, handle sret) but uses `invoke` when the
    /// unwind block has effective cleanup, and `call` + `br` otherwise.
    #[expect(
        clippy::too_many_arguments,
        reason = "terminator emission requires all parameters"
    )]
    fn emit_invoke_indirect(
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
            // No effective cleanup — emit `call` + `br` (same as ApplyIndirect).
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
            // Real unwind path — emit LLVM `invoke`.
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

    /// Inner helper: emit indirect call (no unwind edge) and define `dst`.
    #[expect(
        clippy::too_many_arguments,
        reason = "indirect call emission requires all parameters"
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

    /// Inner helper: emit indirect invoke (with unwind edge) and define `dst`.
    ///
    /// Both return shapes use a real LLVM `invoke` instruction: direct
    /// returns via `invoke_indirect`, sret returns via
    /// `invoke_indirect_with_sret` (sret attribute on the first parameter,
    /// result loaded on the normal edge).
    #[expect(
        clippy::too_many_arguments,
        reason = "indirect invoke emission requires all parameters"
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
            // Sret: real LLVM `invoke` with the sret attribute on the first
            // parameter — preserves the unwind edge (required by
            // `catch(expr:)` over closures returning >16-byte types).
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
            // The sret slot is only valid on the normal edge — load it there.
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
        reason = "ABI dispatch requires all parameters"
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
                self.call_or_invoke_llvm(func_id, &full_args, mode, "call");
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
            // Void-returning call: ARC IR still expects dst to be defined
            // (uniform SSA — every Invoke produces a variable). Bind to a
            // unit constant so successor blocks can reference it.
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
        reason = "runtime call dispatch requires all parameters"
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
            // Void-returning runtime function: bind dst to unit constant
            self.builder.position_at_end(mode.normal_block());
            let unit = self.builder.const_i64(0);
            self.def_var(dst, EmittedValue::Immediate(unit));
        }
    }
}
