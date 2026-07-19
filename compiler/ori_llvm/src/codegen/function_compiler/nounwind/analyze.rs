//! Nounwind fixed-point analysis.

use tracing::debug;

use super::types::{NounwindAnalyzedFunctions, PreparedFunction};
use crate::codegen::function_compiler::FunctionCompiler;

impl<'scx: 'ctx, 'ctx> FunctionCompiler<'_, 'scx, 'ctx, '_> {
    /// Build the complete nounwind function set from all prepared functions.
    ///
    /// Iterates to a fixed point so callee proofs reach their callers. Consumes
    /// the preparation buffer and returns the only state accepted by
    /// [`Self::emit_prepared_functions`].
    pub fn compute_nounwind_set(
        &mut self,
        prepared: Vec<PreparedFunction>,
    ) -> NounwindAnalyzedFunctions {
        debug!(
            functions = prepared.len(),
            "computing complete nounwind set (fixed-point)"
        );

        let mut pass = 0u32;
        let mut mono_propagated = 0u32;
        loop {
            let mut changed = false;

            for func in &prepared {
                for lambda in &func.lambdas {
                    if !self.codegen_ctx.nounwind_functions.contains(&lambda.name)
                        && self.is_arc_function_nounwind(&lambda.arc_func)
                    {
                        self.codegen_ctx.nounwind_functions.insert(lambda.name);
                        changed = true;
                    }
                }

                if !self.codegen_ctx.nounwind_functions.contains(&func.name)
                    && super::derived_artifact_allows_nounwind(self.interner.lookup(func.name))
                    && self.is_arc_function_nounwind(&func.arc_func)
                {
                    self.codegen_ctx.nounwind_functions.insert(func.name);
                    changed = true;
                }
            }

            let propagated = self.propagate_nounwind_to_generic_names();
            mono_propagated = mono_propagated.saturating_add(propagated);
            changed |= propagated > 0;

            pass = pass.saturating_add(1);
            if !changed {
                break;
            }
        }

        debug!(
            passes = pass,
            nounwind_count = self.codegen_ctx.nounwind_functions.len(),
            mono_propagated,
            "nounwind analysis complete"
        );

        NounwindAnalyzedFunctions::new(prepared)
    }

    fn propagate_nounwind_to_generic_names(&mut self) -> u32 {
        let mut propagated = 0u32;
        for (original_name, specializations) in &self.codegen_ctx.mono_dispatch {
            if self.codegen_ctx.nounwind_functions.contains(original_name) {
                continue;
            }
            let all_nounwind = specializations
                .iter()
                .all(|(_, mangled)| self.codegen_ctx.nounwind_functions.contains(mangled));
            if all_nounwind && !specializations.is_empty() {
                self.codegen_ctx.nounwind_functions.insert(*original_name);
                propagated = propagated.saturating_add(1);
            }
        }
        propagated
    }

    /// Classify one statically named call using the same dispatch order as emission.
    fn is_direct_call_nounwind(
        &self,
        callee: ori_ir::Name,
        dst: ori_arc::ir::ArcVarId,
        args: &[ori_arc::ir::ArcVarId],
        func: &ori_arc::ArcFunction,
    ) -> bool {
        use crate::codegen::arc_emitter::context::{
            intercepted_is_nounwind, is_callee_intercepted,
        };
        use crate::codegen::runtime_decl::runtime_functions::is_rt_fn_nounwind;

        let callee_name = self.interner.lookup(callee);
        match is_rt_fn_nounwind(callee_name) {
            Some(nounwind) => nounwind,
            None => {
                // Why: Interception bypasses any same-named user declaration.
                if is_callee_intercepted(
                    callee_name,
                    callee,
                    args,
                    func,
                    &self.codegen_ctx,
                    self.type_info,
                ) {
                    let receiver_tag = args
                        .first()
                        .map(|arg| self.pool.tag(self.pool.resolve_fully(func.var_type(*arg))));
                    let result_tag =
                        Some(self.pool.tag(self.pool.resolve_fully(func.var_type(dst))));
                    intercepted_is_nounwind(callee_name, receiver_tag, result_tag)
                } else {
                    self.codegen_ctx.nounwind_functions.contains(&callee)
                }
            }
        }
    }

    /// Return whether every operation in an ARC function is proven nounwind.
    ///
    /// Direct calls use runtime attributes or the fixed-point function set.
    /// Indirect calls, unwind-capable drops, and checked operations remain
    /// may-unwind so emission preserves their landing pads.
    pub(in crate::codegen::function_compiler) fn is_arc_function_nounwind(
        &self,
        func: &ori_arc::ArcFunction,
    ) -> bool {
        // Why: Same-frame catches require the checked-operation landing pad.
        if !func.catch_scoped_checked_ops.is_empty() {
            return false;
        }

        let drop_name = self.interner.intern("drop");
        let has_user_drop = |ty| self.type_has_user_drop(ty, drop_name);

        func.blocks.iter().all(|block| {
            self.arc_terminator_is_nounwind(&block.terminator, func)
                && block
                    .body
                    .iter()
                    .all(|instr| self.arc_instruction_is_nounwind(instr, func, &has_user_drop))
        })
    }

    fn type_has_user_drop(&self, ty: ori_types::Idx, drop_name: ori_ir::Name) -> bool {
        let ctx = &self.codegen_ctx;
        let resolved = self.pool.resolve_fully(ty);
        if ctx.executable_facts_bound {
            return ctx.user_drop_functions.contains_key(&ty)
                || ctx.user_drop_functions.contains_key(&resolved);
        }
        ctx.type_idx_to_name
            .get(&ty)
            .or_else(|| ctx.type_idx_to_name.get(&resolved))
            .is_some_and(|&name| ctx.method_functions.contains_key(&(name, drop_name)))
    }

    fn arc_terminator_is_nounwind(
        &self,
        terminator: &ori_arc::ir::ArcTerminator,
        func: &ori_arc::ir::ArcFunction,
    ) -> bool {
        match terminator {
            ori_arc::ir::ArcTerminator::Invoke {
                dst,
                func: callee,
                args,
                ..
            } => self.is_direct_call_nounwind(*callee, *dst, args, func),
            ori_arc::ir::ArcTerminator::InvokeIndirect { .. } => false,
            _ => true,
        }
    }

    fn arc_instruction_is_nounwind(
        &self,
        instr: &ori_arc::ir::ArcInstr,
        func: &ori_arc::ir::ArcFunction,
        has_user_drop: &impl Fn(ori_types::Idx) -> bool,
    ) -> bool {
        match instr {
            ori_arc::ir::ArcInstr::Apply {
                dst,
                func: callee,
                args,
                ..
            } => self.is_direct_call_nounwind(*callee, *dst, args, func),
            ori_arc::ir::ArcInstr::ApplyIndirect { .. } => false,
            ori_arc::ir::ArcInstr::RcDec { var, .. } => !ori_arc::type_drop_may_unwind(
                func.var_type(*var),
                self.arc_classifier as &dyn ori_arc::ArcClassification,
                self.pool,
                has_user_drop,
                &mut self.codegen_ctx.drop_unwind_memo.borrow_mut(),
            ),
            ori_arc::ir::ArcInstr::Let {
                ty,
                value: ori_arc::ir::ArcValue::PrimOp { op, .. },
                ..
            } => self.primop_is_nounwind(*ty, *op),
            _ => true,
        }
    }

    fn primop_is_nounwind(&self, ty: ori_types::Idx, op: ori_arc::ir::PrimOp) -> bool {
        let may_panic = match op {
            ori_arc::ir::PrimOp::Binary(op) => op.may_panic_on_int(),
            ori_arc::ir::PrimOp::Unary(op) => op.may_panic_on_int(),
        };
        !(may_panic
            && self
                .pool
                .tag(self.pool.resolve_fully(ty))
                .is_checked_int_arithmetic())
    }
}
