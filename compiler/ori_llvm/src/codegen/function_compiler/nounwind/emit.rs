//! Emission phase: emit LLVM IR for prepared functions using the nounwind set.

use tracing::{debug, trace};

use super::types::{NounwindAnalyzedFunctions, PreparedLambda};
use crate::codegen::arc_emitter::ArcIrEmitter;
use crate::codegen::function_compiler::FunctionCompiler;

impl<'scx: 'ctx, 'ctx> FunctionCompiler<'_, 'scx, 'ctx, '_> {
    /// Emit LLVM IR for all nounwind-analyzed functions.
    ///
    /// For each function, emits lambdas first (so `emit_partial_apply` can
    /// reference them), then the parent function, and applies nounwind
    /// attributes from the pre-computed set.
    pub fn emit_prepared_functions(&mut self, analyzed: NounwindAnalyzedFunctions) {
        let prepared = analyzed.into_prepared();
        debug!(
            count = prepared.len(),
            "emitting prepared functions to LLVM IR"
        );

        for func in prepared {
            self.enter_debug_scope(func.func_id);
            self.builder.set_current_function(func.func_id);

            // Why: Partial applications may reference the emitted lambda functions.
            for lambda in &func.lambdas {
                self.emit_prepared_lambda(lambda);
            }

            // Why: Lambda emission leaves the builder bound to the last lambda.
            self.builder.set_current_function(func.func_id);

            self.dump_arc_if_requested(&func.arc_func, "emit path, prepared");

            let mut emitter = ArcIrEmitter::new(
                self.builder,
                self.type_info,
                self.type_resolver,
                self.interner,
                self.pool,
                self.arc_classifier as &dyn ori_arc::ArcClassification,
                func.func_id,
                &self.codegen_ctx,
                self.debug_context,
            );
            emitter.set_verify_arc(self.verify_arc);
            emitter.set_func_contract(self.aims_contracts.get(&func.arc_func.name));
            emitter.emit_function(&func.arc_func, &func.abi);

            let fn_val = self.builder.get_function_value(func.func_id);
            crate::codegen::ir_builder::cfg_simplify::simplify_cfg(fn_val);

            if self.verify_arc && !fn_val.verify(true) {
                tracing::error!(
                    name = %self.interner.lookup(func.name),
                    "LLVM IR verification failed after codegen (emit_prepared_functions)"
                );
                self.builder.record_codegen_error();
            }

            let nounwind = self.codegen_ctx.nounwind_functions.contains(&func.name);
            if nounwind {
                self.builder.add_nounwind_attribute(func.func_id);
                debug!(
                    name = %self.interner.lookup(func.name),
                    "marked nounwind"
                );
            }
            self.apply_rl31_param_attributes(func.name, func.func_id, &func.abi, nounwind, 0);
            self.apply_effect_attributes(func.name, func.func_id, &func.abi);
            self.apply_return_attributes(func.name, func.func_id, &func.abi);

            self.exit_debug_scope();
        }
    }

    /// Mark emitted functions `nounwind` when they contain no `invoke`.
    ///
    /// This covers immediate-emission paths outside the prepared-function pass
    /// and must run after every function has been emitted.
    pub fn apply_posthoc_nounwind(&mut self) {
        let mut total_added = 0u32;
        let mut newly_nounwind = Vec::new();
        // Why: Hash-map order can visit a caller before its newly nounwind callee.
        loop {
            let mut changed = false;
            for (&name, &(func_id, ref abi)) in &self.codegen_ctx.functions {
                if self.codegen_ctx.nounwind_functions.contains(&name) {
                    continue;
                }
                if !super::derived_artifact_allows_nounwind(self.interner.lookup(name)) {
                    continue;
                }
                if self.builder.function_has_no_invoke(func_id) {
                    self.builder.add_nounwind_attribute(func_id);
                    self.codegen_ctx.nounwind_functions.insert(name);
                    newly_nounwind.push((name, func_id, abi.clone()));
                    total_added = total_added.saturating_add(1);
                    changed = true;
                    trace!(
                        name = %self.interner.lookup(name),
                        "post-hoc nounwind"
                    );
                }
            }
            if !changed {
                break;
            }
        }
        for (name, func_id, abi) in newly_nounwind {
            self.apply_rl31_param_attributes(name, func_id, &abi, true, 0);
        }
        if total_added > 0 {
            debug!(total_added, "post-hoc nounwind pass complete");
        }
    }

    /// Emit LLVM IR for a prepared lambda using the pre-computed nounwind set.
    fn emit_prepared_lambda(&mut self, lambda: &PreparedLambda) {
        self.dump_arc_if_requested(&lambda.arc_func, "emit path, prepared lambda");
        self.builder.set_current_function(lambda.func_id);
        let mut emitter = ArcIrEmitter::new(
            self.builder,
            self.type_info,
            self.type_resolver,
            self.interner,
            self.pool,
            self.arc_classifier as &dyn ori_arc::ArcClassification,
            lambda.func_id,
            &self.codegen_ctx,
            self.debug_context,
        );
        emitter.set_verify_arc(self.verify_arc);
        emitter.set_func_contract(self.aims_contracts.get(&lambda.arc_func.name));
        emitter.emit_function(&lambda.arc_func, &lambda.abi);

        let fn_val = self.builder.get_function_value(lambda.func_id);
        crate::codegen::ir_builder::cfg_simplify::simplify_cfg(fn_val);

        if self.verify_arc && !fn_val.verify(true) {
            tracing::error!(
                name = %self.interner.lookup(lambda.name),
                "LLVM IR verification failed after codegen (emit_prepared_lambda)"
            );
            self.builder.record_codegen_error();
        }

        let nounwind = self.codegen_ctx.nounwind_functions.contains(&lambda.name);
        if nounwind {
            self.builder.add_nounwind_attribute(lambda.func_id);
        }
        let extra_leading_params = u32::from(
            self.codegen_ctx
                .non_capturing_lambdas
                .contains(&lambda.name),
        );
        self.apply_rl31_param_attributes(
            lambda.name,
            lambda.func_id,
            &lambda.abi,
            nounwind,
            extra_leading_params,
        );
        self.apply_effect_attributes(lambda.name, lambda.func_id, &lambda.abi);
        self.apply_return_attributes(lambda.name, lambda.func_id, &lambda.abi);
    }
}
