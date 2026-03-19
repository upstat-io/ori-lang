//! Emission phase: emit LLVM IR for prepared functions using the nounwind set.

use tracing::{debug, trace};

use super::types::{PreparedFunction, PreparedLambda};
use crate::codegen::arc_emitter::ArcIrEmitter;
use crate::codegen::function_compiler::FunctionCompiler;

impl<'scx: 'ctx, 'ctx> FunctionCompiler<'_, 'scx, 'ctx, '_> {
    /// Emit LLVM IR for all prepared functions using the complete nounwind set.
    ///
    /// Must be called after [`Self::compute_nounwind_set`]. For each function,
    /// emits lambdas first (so `emit_partial_apply` can reference them), then
    /// the parent function, and applies nounwind attributes from the
    /// pre-computed set.
    pub fn emit_prepared_functions(&mut self, prepared: Vec<PreparedFunction>) {
        debug!(
            count = prepared.len(),
            "emitting prepared functions to LLVM IR"
        );

        for func in prepared {
            self.enter_debug_scope(func.func_id);
            self.builder.set_current_function(func.func_id);

            // Emit lambdas first
            for lambda in &func.lambdas {
                self.emit_prepared_lambda(lambda);
            }

            // Lambda compilation changes builder.current_function to the last
            // lambda's FunctionId. Reset it to the parent so entry-block allocas
            // (sret temporaries, indirect param storage) land in the right function.
            self.builder.set_current_function(func.func_id);

            // Emit parent function
            let mut emitter = ArcIrEmitter::new(
                self.builder,
                self.type_info,
                self.type_resolver,
                self.interner,
                self.pool,
                self.arc_classifier as &dyn ori_arc::ArcClassification,
                func.func_id,
                &self.codegen_ctx,
            );
            emitter.emit_function(&func.arc_func, &func.abi);

            // Post-emission CFG simplification
            let fn_val = self.builder.get_function_value(func.func_id);
            crate::codegen::ir_builder::cfg_simplify::simplify_cfg(fn_val);

            if self.codegen_ctx.nounwind_functions.contains(&func.name) {
                self.builder.add_nounwind_attribute(func.func_id);
                debug!(
                    name = %self.interner.lookup(func.name),
                    "marked nounwind"
                );
            }
            if self.codegen_ctx.pure_functions.contains(&func.name) {
                self.builder.add_memory_none_attribute(func.func_id);
                debug!(
                    name = %self.interner.lookup(func.name),
                    "marked memory(none)"
                );
            } else if self.codegen_ctx.readonly_functions.contains(&func.name) {
                self.builder.add_memory_read_attribute(func.func_id);
                debug!(
                    name = %self.interner.lookup(func.name),
                    "marked memory(read)"
                );
            }

            self.exit_debug_scope();
        }
    }

    /// Post-hoc nounwind pass: walk all emitted LLVM functions and add `nounwind`
    /// to any function that contains no `invoke` instructions.
    ///
    /// This catches impl methods and test wrappers that were compiled via the
    /// immediate-emit path (before the two-pass nounwind analysis). If all their
    /// call sites used `call` (not `invoke`), the function is provably nounwind.
    ///
    /// Must be called after ALL functions are emitted (impls, two-pass batch,
    /// derives, tests, main wrapper).
    pub fn apply_posthoc_nounwind(&mut self) {
        let mut total_added = 0u32;
        // Fixed-point: newly-marked nounwind functions update their LLVM
        // attribute, which lets callers pass the `function_has_no_invoke`
        // check in subsequent iterations. HashMap iteration order is
        // non-deterministic, so a single pass can miss call chains.
        loop {
            let mut changed = false;
            for (&name, &(func_id, _)) in &self.codegen_ctx.functions {
                if self.codegen_ctx.nounwind_functions.contains(&name) {
                    continue;
                }
                if self.builder.function_has_no_invoke(func_id) {
                    self.builder.add_nounwind_attribute(func_id);
                    self.codegen_ctx.nounwind_functions.insert(name);
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
        if total_added > 0 {
            debug!(total_added, "post-hoc nounwind pass complete");
        }
    }

    /// Emit LLVM IR for a prepared lambda using the pre-computed nounwind set.
    fn emit_prepared_lambda(&mut self, lambda: &PreparedLambda) {
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
        );
        emitter.emit_function(&lambda.arc_func, &lambda.abi);

        // Post-emission CFG simplification
        let fn_val = self.builder.get_function_value(lambda.func_id);
        crate::codegen::ir_builder::cfg_simplify::simplify_cfg(fn_val);

        if self.codegen_ctx.nounwind_functions.contains(&lambda.name) {
            self.builder.add_nounwind_attribute(lambda.func_id);
        }
        if self.codegen_ctx.pure_functions.contains(&lambda.name) {
            self.builder.add_memory_none_attribute(lambda.func_id);
        } else if self.codegen_ctx.readonly_functions.contains(&lambda.name) {
            self.builder.add_memory_read_attribute(lambda.func_id);
        }
    }
}
