//! Terminator emission for the ARC IR emitter.
//!
//! Translates [`ArcTerminator`] variants into LLVM control flow: `ret`, `br`,
//! `cond_br`, `switch`, `invoke`/`call`, `resume`, and `unreachable`.

use ori_arc::ir::{ArcFunction, ArcTerminator, ArcVarId};
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
    #[expect(
        clippy::too_many_lines,
        reason = "terminator dispatch over Return/Jump/Branch/Switch/Invoke"
    )]
    pub(super) fn emit_terminator(
        &mut self,
        term: &ArcTerminator,
        current_block: ori_arc::ir::ArcBlockId,
        _phi_nodes: &[Vec<(ArcVarId, ValueId)>],
        abi: &FunctionAbi,
        landingpad_values: &FxHashMap<usize, ValueId>,
        arc_func: &ArcFunction,
    ) {
        tracing::trace!(?term, block = current_block.index(), "emit_terminator");
        match term {
            ArcTerminator::Return { value } => {
                let val = self.var(*value);
                match &abi.return_abi.passing {
                    ReturnPassing::Sret { .. } => {
                        // Skip identity store when the return value was written
                        // directly to the sret pointer via sret forwarding.
                        let is_identity = self.sret_forwarded_result.is_some_and(|fwd| fwd == val);
                        if !is_identity {
                            let sret_ptr = self.builder.get_param(self.current_function, 0);
                            self.builder.store(val, sret_ptr);
                        }
                        self.builder.ret_void();
                    }
                    ReturnPassing::Direct => {
                        self.builder.ret(val);
                    }
                    ReturnPassing::Void => {
                        self.builder.ret_void();
                    }
                }
            }

            ArcTerminator::Jump { target, args } => {
                let target_idx = target.index();
                debug_assert_eq!(
                    args.len(),
                    arc_func.blocks[target_idx].params.len(),
                    "Jump arg count must match target block param count (block {target_idx})"
                );

                // If inside a SEH funclet (catch-unwind block), exit via catchret
                // to a trampoline block before branching to the target.
                // Same pattern as br_exiting_catchpad — kept inline for phi interleaving
                // (source_block must be read between catchret and br).
                if let Some((pad, kind)) = self.current_funclet_pad.take() {
                    match kind {
                        super::FuncletPadKind::Catch => {
                            let continue_bb = self
                                .builder
                                .append_block(self.current_function, "seh.continue");
                            self.builder.catchret(pad, continue_bb);
                            self.builder.position_at_end(continue_bb);
                        }
                        super::FuncletPadKind::Cleanup => {
                            self.builder.record_codegen_error_with_msg(
                                "Jump terminator inside cleanuppad — \
                                 cleanup pads must exit via cleanupret (Resume terminator)",
                            );
                            self.builder.unreachable();
                            return;
                        }
                    }
                }

                // Record phi incoming values AFTER potential catchret insertion,
                // so source_block is the block that will actually br to target.
                if !args.is_empty() {
                    let Some(source_block) = self.builder.current_block() else {
                        tracing::error!("ARC jump: no current block — skipping phi incoming");
                        self.builder.record_codegen_error();
                        self.builder.br(self.block(*target));
                        return;
                    };
                    for (i, &arg) in args.iter().enumerate() {
                        let val = self.var(arg);
                        // Trunc i64 → narrow for narrowed phi targets
                        let val = {
                            let target_var = arc_func.blocks[target_idx].params[i].0;
                            if let Some(&width) = self.narrowed_vars.get(&target_var) {
                                let narrow_ty = self.llvm_type_for_int_width(width);
                                self.builder.trunc(val, narrow_ty, "phi.trunc")
                            } else {
                                val
                            }
                        };
                        self.phi_incoming.push((target_idx, i, val, source_block));
                    }
                }
                self.builder.br(self.block(*target));
            }

            ArcTerminator::Branch {
                cond,
                then_block,
                else_block,
            } => {
                debug_assert!(
                    self.current_funclet_pad.is_none(),
                    "Branch terminator inside SEH funclet — \
                     funclets must exit via catchret/cleanupret, not cond_br"
                );
                let cond_val = self.var(*cond);
                self.builder
                    .cond_br(cond_val, self.block(*then_block), self.block(*else_block));
            }

            ArcTerminator::Switch {
                scrutinee,
                cases,
                default,
            } => {
                debug_assert!(
                    self.current_funclet_pad.is_none(),
                    "Switch terminator inside SEH funclet — \
                     funclets must exit via catchret/cleanupret, not switch"
                );
                let scrut_val = self.var(*scrutinee);

                // §07.2: Niche-encoded enum — emit icmp + cond_br instead of switch.
                let handled_niche = self
                    .niche_scrutinees
                    .get(scrutinee)
                    .copied()
                    .and_then(|enum_ty| self.get_niche_encoding(enum_ty));
                if let Some(encoding) = handled_niche {
                    if encoding.is_tagless() {
                        let target = cases
                            .first()
                            .map_or_else(|| self.block(*default), |(_, b)| self.block(*b));
                        self.builder.br(target);
                    } else {
                        self.emit_niche_switch(scrut_val, &encoding, cases, *default);
                    }
                } else {
                    let llvm_cases: Vec<(ValueId, BlockId)> = cases
                        .iter()
                        .map(|&(tag, block_id)| {
                            let tag_val = self.builder.const_int_matching(scrut_val, tag);
                            (tag_val, self.block(block_id))
                        })
                        .collect();
                    self.builder
                        .switch(scrut_val, self.block(*default), &llvm_cases);
                }
            }

            ArcTerminator::Invoke {
                dst,
                ty: _,
                func,
                args,
                arg_ownership: _,
                normal,
                unwind,
            } => {
                // On SEH, catch-type invokes use the ori_try_call trampoline
                // instead of LLVM catchpad (which triggers "Rust panics must
                // be rethrown" on MSVC).
                let is_nounwind = self.ctx.nounwind_functions.contains(func);
                let eh_model = self.builder.eh_model();
                let is_seh_catch = !is_nounwind
                    && eh_model == EhModel::Seh
                    && !matches!(
                        arc_func.blocks[unwind.index()].terminator,
                        ArcTerminator::Resume
                    );

                if is_seh_catch {
                    self.emit_seh_catch_invoke(*dst, *func, args, *normal, *unwind, arc_func);
                } else {
                    self.emit_invoke(*dst, *func, args, *normal, *unwind, arc_func);
                }
            }

            ArcTerminator::InvokeIndirect {
                dst,
                ty,
                closure,
                args,
                normal,
                unwind,
            } => {
                self.emit_invoke_indirect(*dst, *ty, *closure, args, *normal, *unwind, arc_func);
            }

            ArcTerminator::Resume => {
                match self.builder.eh_model() {
                    EhModel::Itanium => {
                        // Re-raise the caught exception using the landingpad token
                        // captured at the start of this unwind block.
                        if let Some(&lp_val) = landingpad_values.get(&current_block.index()) {
                            self.builder.resume(lp_val);
                        } else {
                            tracing::warn!(
                                block = current_block.index(),
                                "ARC Resume without landingpad — emitting unreachable"
                            );
                            self.builder.unreachable();
                        }
                    }
                    EhModel::Seh => {
                        // SEH: cleanupret re-raises the exception.
                        // The cleanuppad token was stored in current_funclet_pad
                        // during the unwind block prelude.
                        if let Some((pad, _kind)) = self.current_funclet_pad.take() {
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

            ArcTerminator::Unreachable => {
                self.builder.unreachable();
            }
        }
    }

    /// Emit an `Invoke` terminator (ABI-aware function call with unwind).
    ///
    /// §07.2: Emit a niche-aware switch as `icmp eq` + `cond_br`.
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
    fn emit_invoke(
        &mut self,
        dst: ArcVarId,
        callee: Name,
        arc_args: &[ArcVarId],
        normal: ori_arc::ir::ArcBlockId,
        unwind: ori_arc::ir::ArcBlockId,
        arc_func: &ArcFunction,
    ) {
        let func_name_str = self.interner.lookup(callee);
        let normal_block = self.block(normal);
        let is_nounwind = self.ctx.nounwind_functions.contains(&callee);
        // An unwind block with no effective cleanup (empty body or only
        // no-op RcDecs on non-capturing closures + Resume terminator) has
        // no LLVM basic block — emit_function() marks it dead.
        // Using `call` instead of `invoke` is safe because there's nothing
        // to unwind through.
        let unwind_is_empty_cleanup =
            !super::dead_unwind::has_effective_cleanup(&arc_func.blocks[unwind.index()], arc_func);
        // Builtin handlers (format calls, prelude functions, builtin methods)
        // always emit `call`, not `invoke`. Their unwind blocks are dead —
        // emit_function() already skipped creating LLVM blocks for them.
        let callee_intercepted = self.callee_will_be_intercepted(callee, arc_args, arc_func);
        let mode = if is_nounwind || unwind_is_empty_cleanup || callee_intercepted {
            InvokeMode::Call {
                normal: normal_block,
            }
        } else {
            // Only resolve unwind block when actually needed — dead unwind
            // blocks have no LLVM basic block and would panic in block().
            let unwind_block = self.block(unwind);
            InvokeMode::Invoke {
                normal: normal_block,
                unwind: unwind_block,
            }
        };

        // Intercept ori_format_* calls: decompose string struct arg into (ptr, len).
        if let Some(val) = self.try_emit_format_call(func_name_str, arc_args, arc_func) {
            self.br_exiting_catchpad(normal_block);
            self.builder.position_at_end(normal_block);
            self.def_var_repr(dst, val, arc_func);
            return;
        }

        // Prelude builtin functions (str, int, float, byte, hash_combine, etc.)
        if let Some(val) = super::builtins::prelude::try_emit_prelude_function(
            self,
            func_name_str,
            arc_args,
            arc_func,
        ) {
            self.br_exiting_catchpad(normal_block);
            self.builder.position_at_end(normal_block);
            self.def_var_repr(dst, val, arc_func);
            return;
        }

        let arg_vals: Vec<ValueId> = arc_args.iter().map(|a| self.var(*a)).collect();

        let resolved = self.resolve_invoke_callee(callee, arc_args, dst, arc_func);

        if let Some((func_id, params, ret_passing, ret_ty_idx)) = resolved {
            self.emit_abi_resolved_call(
                dst,
                func_id,
                &params,
                ret_passing,
                ret_ty_idx,
                &arg_vals,
                arc_args,
                mode,
                arc_func,
            );
        } else if let Some(val) = self.try_emit_builtin_method(callee, arc_args, arc_func) {
            // Builtin method handled inline — branch to normal block
            // (the current block needs a terminator since we skipped invoke)
            self.br_exiting_catchpad(normal_block);
            self.builder.position_at_end(normal_block);
            self.def_var_repr(dst, val, arc_func);
        } else if let Some(func_id) = self.builder.try_runtime_fn(func_name_str) {
            self.emit_runtime_fn_call(
                dst,
                func_id,
                func_name_str,
                arc_args,
                &arg_vals,
                mode,
                arc_func,
            );
        } else {
            let msg =
                format!("unresolved function `{func_name_str}` in invoke — missing mono instance?");
            tracing::warn!("{msg}");
            // Emit a branch to the normal block so the IR stays well-formed
            // (every block must have a terminator).
            self.br_exiting_catchpad(normal_block);
            self.builder.position_at_end(normal_block);
            // Bind dst to unit constant so successor blocks don't crash
            let unit = self.builder.const_i64(0);
            self.def_var(dst, EmittedValue::Immediate(unit));
            self.builder.record_codegen_error_with_msg(msg);
        }
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

        let ptr_ty = self.builder.ptr_type();
        let mut arg_vals = Vec::with_capacity(1 + args.len());
        let mut param_types = Vec::with_capacity(1 + args.len());
        arg_vals.push(env_ptr);
        param_types.push(ptr_ty);

        for &a in args {
            let arg_ty = arc_func.var_type(a);
            let passing =
                crate::codegen::abi::compute_param_passing(arg_ty, self.type_info, self.repr_plan);
            match passing {
                crate::codegen::abi::ParamPassing::Indirect { .. }
                | crate::codegen::abi::ParamPassing::Reference => {
                    let llvm_ty = self.resolve_type(arg_ty);
                    let alloca = self.builder.alloca(llvm_ty, "icall.arg.tmp");
                    self.builder.store(self.var(a), alloca);
                    arg_vals.push(alloca);
                    param_types.push(ptr_ty);
                }
                crate::codegen::abi::ParamPassing::Void => {}
                crate::codegen::abi::ParamPassing::Direct => {
                    arg_vals.push(self.var(a));
                    param_types.push(self.resolve_type(arg_ty));
                }
            }
        }

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
            self.br_exiting_catchpad(normal_block);
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
            if let Some((pad, _kind)) = self.current_funclet_pad {
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
        } else if let Some((pad, _kind)) = self.current_funclet_pad {
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
    /// For sret returns, falls back to `call` + `br` since `invoke_indirect`
    /// doesn't directly support sret convention. For direct returns, uses a
    /// real LLVM `invoke` instruction.
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
            // Sret: emit call + br (invoke_indirect doesn't support sret
            // convention directly). The unwind edge is sacrificed for
            // correctness of the ABI — closure sret invoke is rare.
            self.emit_invoke_indirect_call(
                dst,
                ret_ty,
                ret_is_indirect,
                fn_ptr,
                param_types,
                arg_vals,
                arc_func,
            );
            self.br_exiting_catchpad(normal_block);
            self.builder.position_at_end(normal_block);
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

    /// Resolve the callee for an Invoke terminator via the 5-step dispatch chain.
    ///
    /// Tries: receiver-based → return-type-based → unqualified → monomorphized → fallback.
    fn resolve_invoke_callee(
        &self,
        callee: Name,
        arc_args: &[ArcVarId],
        dst: ArcVarId,
        arc_func: &ArcFunction,
    ) -> Option<(FunctionId, Vec<ParamAbi>, ReturnPassing, Idx)> {
        self.lookup_method_by_receiver(callee, arc_args, arc_func)
            .or_else(|| self.lookup_method_by_return_type(callee, dst, arc_func))
            .or_else(|| self.ctx.functions.get(&callee))
            .or_else(|| self.lookup_mono_dispatch(callee, arc_args, arc_func))
            .or_else(|| self.lookup_method_fallback(callee))
            .map(|(fid, abi)| {
                (
                    *fid,
                    abi.params.clone(),
                    abi.return_abi.passing,
                    abi.return_abi.ty,
                )
            })
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
        let passed_args = self.apply_param_passing_with_forwarding(arg_vals, arc_vars, params);
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
        func_name_str: &str,
        arc_args: &[ArcVarId],
        arg_vals: &[ValueId],
        mode: InvokeMode,
        arc_func: &ArcFunction,
    ) {
        let is_list_push = func_name_str == "ori_list_push";
        let is_list_new = func_name_str == "ori_list_new";
        let mut coerced_args: Vec<ValueId> = arc_args
            .iter()
            .zip(arg_vals.iter())
            .enumerate()
            .map(|(i, (arc_var, &val))| {
                let arg_ty = arc_func.var_type(*arc_var);
                if is_list_push && i == 1 {
                    self.coerce_any_to_ptr(val, arg_ty)
                } else {
                    self.coerce_aggregate_to_ptr(val, arg_ty)
                }
            })
            .collect();

        // Replace elem_size for int-element for-yield lists.
        // Only override when the pre-scan identified this elem_size_var as
        // belonging to an int-element for-yield (prevents corrupting non-int
        // accumulators like [str] when narrowed [int] exists in the program).
        if let Some(width) = self.narrowed_int_collection_element_width() {
            let narrowed_size = self.builder.const_i64(i64::from(width.size_bytes()));
            if is_list_new
                && arc_args.len() == 2
                && self.for_yield_int_elem_sizes.contains(&arc_args[1])
            {
                coerced_args[1] = narrowed_size;
            } else if is_list_push
                && arc_args.len() == 3
                && self.for_yield_int_elem_sizes.contains(&arc_args[2])
            {
                coerced_args[2] = narrowed_size;
            }
        }

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
