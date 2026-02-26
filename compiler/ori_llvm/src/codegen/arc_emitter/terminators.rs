//! Terminator emission for the ARC IR emitter.
//!
//! Translates [`ArcTerminator`] variants into LLVM control flow: `ret`, `br`,
//! `cond_br`, `switch`, `invoke`/`call`, `resume`, and `unreachable`.

use ori_arc::ir::{ArcFunction, ArcTerminator, ArcVarId};
use ori_ir::Name;
use rustc_hash::FxHashMap;

use super::context::{EmittedValue, InvokeMode};
use super::ArcIrEmitter;
use crate::codegen::abi::{FunctionAbi, ReturnPassing};
use crate::codegen::value_id::{BlockId, ValueId};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit an `ArcTerminator` as LLVM control flow.
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
                        let sret_ptr = self.builder.get_param(self.current_function, 0);
                        self.builder.store(val, sret_ptr);
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
                // Record phi incoming values for the target block's parameters
                let target_idx = target.index();
                debug_assert_eq!(
                    args.len(),
                    arc_func.blocks[target_idx].params.len(),
                    "Jump arg count must match target block param count (block {target_idx})"
                );
                if !args.is_empty() {
                    let Some(source_block) = self.builder.current_block() else {
                        tracing::error!("ARC jump: no current block — skipping phi incoming");
                        self.builder.record_codegen_error();
                        self.builder.br(self.block(*target));
                        return;
                    };
                    for (i, &arg) in args.iter().enumerate() {
                        let val = self.var(arg);
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
                let cond_val = self.var(*cond);
                self.builder
                    .cond_br(cond_val, self.block(*then_block), self.block(*else_block));
            }

            ArcTerminator::Switch {
                scrutinee,
                cases,
                default,
            } => {
                let scrut_val = self.var(*scrutinee);
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

            ArcTerminator::Invoke {
                dst,
                ty: _,
                func,
                args,
                arg_ownership: _,
                normal,
                unwind,
            } => self.emit_invoke(*dst, *func, args, *normal, *unwind, arc_func),

            ArcTerminator::Resume => {
                // Re-raise the caught exception using the landingpad token
                // captured at the start of this unwind block.
                if let Some(&lp_val) = landingpad_values.get(&current_block.index()) {
                    self.builder.resume(lp_val);
                } else {
                    // No landingpad for this block — should not happen if ARC IR
                    // is well-formed, but emit unreachable as a safety fallback.
                    tracing::warn!(
                        block = current_block.index(),
                        "ARC Resume without landingpad — emitting unreachable"
                    );
                    self.builder.unreachable();
                }
            }

            ArcTerminator::Unreachable => {
                self.builder.unreachable();
            }
        }
    }

    /// Emit an `Invoke` terminator (ABI-aware function call with unwind).
    ///
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
        let mode = if is_nounwind {
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
            self.builder.br(normal_block);
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
            self.builder.br(normal_block);
            self.builder.position_at_end(normal_block);
            self.def_var_repr(dst, val, arc_func);
            return;
        }

        let arg_vals: Vec<ValueId> = arc_args.iter().map(|a| self.var(*a)).collect();

        // Method dispatch chain:
        // 1. Receiver-based: use first arg's type (instance methods like eq/hash)
        // 2. Return-type-based: use dst's type (static methods like default)
        // 3. Unqualified: bare function name (free functions)
        // 4. Monomorphized generic: match arg types → mangled specialization
        // 5. Diagnostic fallback: logs warning, returns None
        let resolved = self
            .lookup_method_by_receiver(callee, arc_args, arc_func)
            .or_else(|| self.lookup_method_by_return_type(callee, dst, arc_func))
            .or_else(|| self.ctx.functions.get(&callee))
            .or_else(|| self.lookup_mono_dispatch(callee, arc_args, arc_func))
            .or_else(|| self.lookup_method_fallback(callee))
            .map(|(fid, abi)| (*fid, abi.params.clone(), abi.return_abi.clone()));

        if let Some((func_id, params, ret_abi)) = resolved {
            let passed_args = self.apply_param_passing(&arg_vals, &params);
            let result = match &ret_abi.passing {
                ReturnPassing::Sret { .. } => {
                    let ret_ty = self.resolve_type(ret_abi.ty);
                    let sret_alloca = self.builder.alloca(ret_ty, "sret.tmp");
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
        } else if let Some(val) = self.try_emit_builtin_method(callee, arc_args, arc_func) {
            // Builtin method handled inline — branch to normal block
            // (the current block needs a terminator since we skipped invoke)
            self.builder.br(normal_block);
            self.builder.position_at_end(normal_block);
            self.def_var_repr(dst, val, arc_func);
        } else if let Some(func_id) = self.builder.try_runtime_fn(func_name_str) {
            // Runtime function fallback with aggregate coercion.
            // Runtime functions take ptr params, but ARC IR passes aggregate
            // structs (Str, List, etc.) by value — coerce as needed.
            let is_list_push = func_name_str == "ori_list_push";
            let coerced_args: Vec<ValueId> = arc_args
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
            if let Some(val) = self.call_or_invoke_llvm(func_id, &coerced_args, mode, "call") {
                self.builder.position_at_end(mode.normal_block());
                self.def_var_repr(dst, val, arc_func);
            } else {
                // Void-returning runtime function: bind dst to unit constant
                self.builder.position_at_end(mode.normal_block());
                let unit = self.builder.const_i64(0);
                self.def_var(dst, EmittedValue::Immediate(unit));
            }
        } else {
            let msg =
                format!("unresolved function `{func_name_str}` in invoke — missing mono instance?");
            tracing::warn!("{msg}");
            // Emit a branch to the normal block so the IR stays well-formed
            // (every block must have a terminator).
            self.builder.br(normal_block);
            self.builder.position_at_end(normal_block);
            // Bind dst to unit constant so successor blocks don't crash
            let unit = self.builder.const_i64(0);
            self.def_var(dst, EmittedValue::Immediate(unit));
            self.builder.record_codegen_error_with_msg(msg);
        }
    }
}
