//! Nounwind analysis: fixed-point iteration + purity/readonly detection.

use tracing::debug;

use super::types::PreparedFunction;
use crate::codegen::function_compiler::purity_analysis::{
    has_only_pure_arc_instructions, is_abi_memory_free,
};
use crate::codegen::function_compiler::FunctionCompiler;

impl<'scx: 'ctx, 'ctx> FunctionCompiler<'_, 'scx, 'ctx, '_> {
    /// Build the complete nounwind function set from all prepared functions.
    ///
    /// Uses fixed-point iteration: each pass analyzes all functions and their
    /// lambdas, adding newly-proven-nounwind functions to the set. Iteration
    /// continues until no new functions are added. This correctly handles
    /// call chains like A→B→C where C must be proven nounwind before B,
    /// and B before A.
    ///
    /// Must be called after all `prepare_*` methods and before
    /// [`Self::emit_prepared_functions`].
    pub fn compute_nounwind_set(&mut self, prepared: &[PreparedFunction]) {
        debug!(
            functions = prepared.len(),
            "computing complete nounwind set (fixed-point)"
        );

        let mut pass = 0u32;
        let mut mono_propagated = 0u32;
        loop {
            let mut changed = false;

            for func in prepared {
                // Check lambdas first (they may be callees of the parent)
                for lambda in &func.lambdas {
                    if !self.codegen_ctx.nounwind_functions.contains(&lambda.name)
                        && self.is_arc_function_nounwind(&lambda.arc_func)
                    {
                        self.codegen_ctx.nounwind_functions.insert(lambda.name);
                        changed = true;
                    }
                }

                // Check parent function
                if !self.codegen_ctx.nounwind_functions.contains(&func.name)
                    && self.is_arc_function_nounwind(&func.arc_func)
                {
                    self.codegen_ctx.nounwind_functions.insert(func.name);
                    changed = true;
                }
            }

            // Propagate nounwind from mangled monomorphized names to their
            // original generic names. ARC IR `Invoke` terminators use the
            // original name (e.g., `"identity"`), while `nounwind_functions`
            // contains mangled names (e.g., `"identity$m$int"`). If ALL
            // specializations of a generic are nounwind, the original name
            // is also safe to call without landing pads.
            //
            // This must be INSIDE the fixed-point loop so that callers of
            // the original generic name (e.g., `main` calling `identity`)
            // are re-analyzed after the original name is added to the set.
            for (original_name, specializations) in &self.codegen_ctx.mono_dispatch {
                if self.codegen_ctx.nounwind_functions.contains(original_name) {
                    continue; // Already marked (e.g., non-generic with same name)
                }
                let all_nounwind = specializations
                    .iter()
                    .all(|(_, mangled)| self.codegen_ctx.nounwind_functions.contains(mangled));
                if all_nounwind && !specializations.is_empty() {
                    self.codegen_ctx.nounwind_functions.insert(*original_name);
                    mono_propagated = mono_propagated.saturating_add(1);
                    changed = true;
                }
            }

            pass = pass.saturating_add(1);
            if !changed {
                break;
            }
        }

        // Purity analysis: functions with no memory effects get `memory(none)`.
        // Requires: (1) all ARC instructions are `Let` only, (2) all params
        // are Direct-passing (no pointer loads), (3) return is Direct or Void
        // (no sret store). Single-pass — pure functions have no calls.
        let mut pure_count = 0u32;
        for func in prepared {
            for lambda in &func.lambdas {
                if has_only_pure_arc_instructions(&lambda.arc_func)
                    && is_abi_memory_free(&lambda.abi)
                {
                    self.codegen_ctx.pure_functions.insert(lambda.name);
                    pure_count = pure_count.saturating_add(1);
                }
            }
            if has_only_pure_arc_instructions(&func.arc_func) && is_abi_memory_free(&func.abi) {
                self.codegen_ctx.pure_functions.insert(func.name);
                pure_count = pure_count.saturating_add(1);
            }
        }

        // Read-only analysis: functions that read memory but don't write.
        // These have `Let`/`Select`/`Project` instructions (Project reads via
        // pointer) but no calls, RC ops, or mutations. Strictly weaker than
        // pure — pure functions get `memory(none)`, readonly gets `memory(read)`.
        // CONSTRAINT: Sret returns WRITE to the sret pointer, so functions with
        // Sret return passing cannot be marked memory(read).
        let mut readonly_count = 0u32;
        for func in prepared {
            for lambda in &func.lambdas {
                if !self.codegen_ctx.pure_functions.contains(&lambda.name)
                    && has_only_pure_arc_instructions(&lambda.arc_func)
                    && !matches!(
                        lambda.abi.return_abi.passing,
                        crate::codegen::abi::ReturnPassing::Sret { .. }
                    )
                {
                    self.codegen_ctx.readonly_functions.insert(lambda.name);
                    readonly_count = readonly_count.saturating_add(1);
                }
            }
            if !self.codegen_ctx.pure_functions.contains(&func.name)
                && has_only_pure_arc_instructions(&func.arc_func)
                && !matches!(
                    func.abi.return_abi.passing,
                    crate::codegen::abi::ReturnPassing::Sret { .. }
                )
            {
                self.codegen_ctx.readonly_functions.insert(func.name);
                readonly_count = readonly_count.saturating_add(1);
            }
        }

        debug!(
            passes = pass,
            nounwind_count = self.codegen_ctx.nounwind_functions.len(),
            mono_propagated,
            pure_count,
            readonly_count,
            "nounwind + memory analysis complete"
        );
    }

    /// Check if an ARC function is nounwind (cannot unwind/panic).
    ///
    /// A function is nounwind if:
    /// 1. All `Invoke` callees are already known-nounwind (in the set), AND
    /// 2. No `Apply` calls a may-unwind function, AND
    /// 3. No `ApplyIndirect` instructions exist (indirect calls through
    ///    closures/function pointers are conservatively may-unwind).
    ///
    /// For `Apply` callees, three cases:
    /// - **Runtime function with `Nounwind` attr**: safe (cannot unwind).
    /// - **Runtime function WITHOUT `Nounwind` attr**: unsafe — may call
    ///   `ori_panic` internally (e.g., `ori_list_get` on OOB, `ori_assert`
    ///   on failure, allocating functions on OOM).
    /// - **User-defined function**: check `nounwind_functions` set.
    ///
    /// Indirect calls (`ApplyIndirect`) cannot be statically resolved to a
    /// known callee, so we must conservatively assume they may unwind. This
    /// prevents UB when a closure target panics inside a `nounwind` function.
    pub(in crate::codegen::function_compiler) fn is_arc_function_nounwind(
        &self,
        func: &ori_arc::ArcFunction,
    ) -> bool {
        use crate::codegen::arc_emitter::context::{
            intercepted_is_nounwind, is_callee_intercepted,
        };
        use crate::codegen::runtime_decl::runtime_functions::is_rt_fn_nounwind;

        // Local "does this exact type carry a user `@drop`" check — the
        // codegen SSOT (`CodegenContext.method_functions`, mirror of
        // `ArcIrEmitter::user_drop_method`). Supplied to
        // `ori_arc::type_drop_may_unwind`, which recurses the drop tree.
        let drop_name = self.interner.intern("drop");
        let ctx = &self.codegen_ctx;
        let pool = self.pool;
        let has_user_drop = |ty: ori_types::Idx| -> bool {
            let type_name = ctx
                .type_idx_to_name
                .get(&ty)
                .copied()
                .or_else(|| ctx.type_idx_to_name.get(&pool.resolve_fully(ty)).copied());
            match type_name {
                Some(n) => ctx.method_functions.contains_key(&(n, drop_name)),
                None => false,
            }
        };

        func.blocks.iter().all(|block| {
            let term_ok = match &block.terminator {
                ori_arc::ir::ArcTerminator::Invoke {
                    func: callee, args, ..
                } => {
                    // Same checks as Apply: nounwind set, runtime fns, then
                    // intercepted builtins (protocols, prelude, methods).
                    if self.codegen_ctx.nounwind_functions.contains(callee) {
                        true
                    } else {
                        let s = self.interner.lookup(*callee);
                        match is_rt_fn_nounwind(s) {
                            Some(true) => true,
                            Some(false) => false,
                            None => {
                                self.codegen_ctx.nounwind_functions.contains(callee)
                                    || (is_callee_intercepted(
                                        s,
                                        *callee,
                                        args,
                                        func,
                                        &self.codegen_ctx,
                                        self.type_info,
                                    ) && intercepted_is_nounwind(s))
                            }
                        }
                    }
                }
                // Indirect calls through closures cannot be statically
                // resolved — conservatively assume they may unwind.
                ori_arc::ir::ArcTerminator::InvokeIndirect { .. } => false,
                _ => true,
            };
            let instrs_ok = block.body.iter().all(|instr| match instr {
                ori_arc::ir::ArcInstr::Apply {
                    func: callee, args, ..
                } => {
                    let s = self.interner.lookup(*callee);
                    match is_rt_fn_nounwind(s) {
                        // Known runtime function with Nounwind attribute
                        Some(true) => true,
                        // Known runtime function WITHOUT Nounwind — may unwind
                        Some(false) => false,
                        // Not a runtime function — check user function set,
                        // then check if the callee will be intercepted by
                        // builtin handlers (which always emit `call`).
                        // Intercepted builtins are nounwind by default, with
                        // a small exception set for builtin methods that
                        // emit inline panics (Option.expect etc.) — those
                        // are filtered out here via intercepted_is_nounwind.
                        None => {
                            self.codegen_ctx.nounwind_functions.contains(callee)
                                || (is_callee_intercepted(
                                    s,
                                    *callee,
                                    args,
                                    func,
                                    &self.codegen_ctx,
                                    self.type_info,
                                ) && intercepted_is_nounwind(s))
                        }
                    }
                }
                // Indirect calls through closures/function pointers are
                // conservatively treated as may-unwind — we cannot know
                // the callee's unwind behavior at compile time.
                //
                // Conservative decision (document limitation).
                // Interprocedural proof (tracking all possible callees for
                // every closure variable) is a significant analysis investment
                // for a LOW-severity finding. The pessimistic result (using
                // invoke instead of call) is always safe.
                ori_arc::ir::ArcInstr::ApplyIndirect { .. } => false,
                // A scope-exit `RcDec` whose drop tree transitively reaches a
                // user `@drop` may raise a foreign Itanium exception, so the
                // function is may-unwind: it needs an `invoke` + cleanup pad
                // (emitted at the dec site) to thread the exception toward the
                // `@main` catch-all. Treating it as nounwind (the prior catch-
                // all below) elides the landing pad → abort on the recoverable
                // path. `ori_rc_inc`/scalar/`Trivial`-drop decs stay nounwind.
                ori_arc::ir::ArcInstr::RcDec { var, .. } => !ori_arc::type_drop_may_unwind(
                    func.var_type(*var),
                    self.arc_classifier as &dyn ori_arc::ArcClassification,
                    self.pool,
                    &has_user_drop,
                    &mut self.codegen_ctx.drop_unwind_memo.borrow_mut(),
                ),
                _ => true,
            });
            term_ok && instrs_ok
        })
    }
}
