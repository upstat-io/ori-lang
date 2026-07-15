//! Shared executable-artifact projection helpers.
//!
//! Both the immediate-emit path ([`FunctionCompiler::emit_arc_function`] in
//! [`super::define_phase`]) and the two-pass prepare path
//! ([`FunctionCompiler::prepare_arc_function`] in [`super::nounwind::prepare`])
//! route through these helpers, keeping the shared-seam surface in one place.
//!
//! Callers (grep-verifiable):
//! - `define_phase.rs`: `emit_arc_function_inner` → `process_arc_function`;
//!   `compile_lambda_arc` → `declare_and_process_lambda`.
//! - `nounwind/prepare.rs`: `prepare_arc_function` / `prepare_lambda` →
//!   same two entry points.
//! - `impls.rs`: `compile_impls` / test compilation go through
//!   `emit_arc_function` above (no direct call to these helpers today).

use ori_arc::verify::VerifyError;
use ori_ir::Name;
use rustc_hash::FxHashSet;
use tracing::debug;

use super::FunctionCompiler;
use crate::codegen::abi::{select_call_conv, CallConvSite, FunctionAbi};
use crate::codegen::value_id::FunctionId;

impl<'scx: 'ctx, 'ctx> FunctionCompiler<'_, 'scx, 'ctx, '_> {
    /// Replace a lowering shape-driver with its closed realized body.
    ///
    /// Shared by both the immediate-emit path ([`Self::emit_arc_function`]) and
    /// the two-pass prepare path ([`Self::prepare_arc_function`]).
    ///
    /// Returns `Err(VerifyError::UnresolvedTypeVar(_))` when the PC-2
    /// cross-phase contract check detects `Tag::Var` or `Tag::Projection`
    /// in the ARC IR. This is ALWAYS-ON contract enforcement, not gated by
    /// `self.verify_arc` which controls additional downstream verification
    /// (per-function LLVM IR verification under `ORI_VERIFY_ARC=1`).
    pub(super) fn process_arc_function(
        &mut self,
        name: Name,
        arc_func: &mut ori_arc::ArcFunction,
    ) -> Result<(), VerifyError> {
        if let Some(program) = self.executable_program {
            let Some(function) = program.function_id(name) else {
                self.builder.record_codegen_error_with_msg(format!(
                    "validated executable has no realized body for {}",
                    self.interner.lookup(name)
                ));
                return Ok(());
            };
            *arc_func = program.functions()[function.index()].clone();
        } else {
            #[cfg(not(test))]
            {
                self.builder.record_codegen_error_with_msg(format!(
                    "physical LLVM projection requires a closed executable artifact for {}",
                    self.interner.lookup(name)
                ));
                return Err(VerifyError::VariableMetadataUnrealized);
            }
            #[cfg(test)]
            debug!(
                name = %self.interner.lookup(name),
                "projecting an explicitly supplied low-level test fixture"
            );
        }

        // PC-2 contract check guards the exact body consumed by physical
        // projection. Checking a caller-supplied lowering shape instead would
        // validate the wrong structure.
        //
        // Empty exempt set: generic bodies reach this seam only post-
        // monomorphization; non-generic bodies have no scheme_var_ids.
        let exempt: FxHashSet<u32> = FxHashSet::default();
        if let Err(err) =
            ori_arc::assert_no_unresolved_type_vars(self.pool, arc_func, self.interner, &exempt)
        {
            return Err(self
                .report_primary_seam_violation(err, "Tag::Var in ARC IR violates PC-2 contract"));
        }

        if self.executable_program.is_some() {
            debug!(
                name = %self.interner.lookup(name),
                "consuming closed post-AIMS function body"
            );
        }

        Ok(())
    }

    /// Report a contract-violation at a primary PC-2 / monomorphization-resolution
    /// seam: emit structured `contract_violation=true` tracing event, increment
    /// the codegen-error counter (so AOT callers see the failure through
    /// `builder.codegen_error_count`), and convert the typed error to the
    /// shared `VerifyError` variant via `Into`. Returns the converted error so
    /// the caller can `return Err(...)` directly.
    ///
    /// Callers: `process_arc_function` (PC-2 on top-level functions),
    /// `declare_and_process_lambda` (PC-2 + `BoundVar` on lambdas). Secondary-site
    /// hooks (`pc2_hooks::run_pc2_hook_aot` AOT path, `evaluator/compile.rs` JIT
    /// path) do NOT use this helper — they emit `tracing::error!` only per the
    /// secondary-site contract (no `record_codegen_error`, no `return Err`).
    pub(super) fn report_primary_seam_violation<E>(
        &mut self,
        err: E,
        msg: &'static str,
    ) -> VerifyError
    where
        E: std::fmt::Debug + Into<VerifyError>,
    {
        tracing::error!(
            contract_violation = true,
            error = ?err,
            "{}", msg,
        );
        self.builder.record_codegen_error();
        err.into()
    }

    /// Declare a lambda LLVM function, register it in `codegen_ctx`, and run
    /// the ARC pipeline.
    ///
    /// Shared by both the immediate-emit path ([`Self::compile_lambda_arc`]) and
    /// the two-pass prepare path ([`Self::prepare_lambda`]).
    ///
    /// Returns `(lambda_name, func_id, abi)` for the caller to either emit
    /// LLVM IR immediately or buffer as a [`PreparedLambda`].
    ///
    /// **Non-capturing optimization**: When `lambda.num_captures == 0`, the
    /// LLVM function is declared with `ccc` + a phantom `ptr %_env` leading
    /// parameter, making it directly callable as a closure without generating
    /// a `_ori_partial_N` trampoline wrapper. The emission ABI (stored in
    /// `codegen_ctx.functions`) does NOT include the phantom param -- it stays
    /// unchanged so `emit_function` body emission works correctly.
    pub(super) fn declare_and_process_lambda(
        &mut self,
        lambda: &mut ori_arc::ArcFunction,
    ) -> Result<(Name, FunctionId, FunctionAbi), VerifyError> {
        if let Some(program) = self.executable_program {
            let Some(function) = program.function_id(lambda.name) else {
                self.builder.record_codegen_error_with_msg(format!(
                    "validated executable has no realized lambda body for {}",
                    self.interner.lookup(lambda.name)
                ));
                return Err(VerifyError::VariableMetadataUnrealized);
            };
            *lambda = program.functions()[function.index()].clone();
        } else {
            #[cfg(not(test))]
            {
                self.builder.record_codegen_error_with_msg(format!(
                    "physical LLVM projection requires a closed executable artifact for lambda {}",
                    self.interner.lookup(lambda.name)
                ));
                return Err(VerifyError::VariableMetadataUnrealized);
            }
            #[cfg(test)]
            debug!(
                name = %self.interner.lookup(lambda.name),
                "projecting an explicitly supplied low-level lambda fixture"
            );
        }

        // PC-2 contract check guards the exact realized lambda body consumed
        // by physical projection. Mirrors the sibling check in
        // `process_arc_function`.
        let exempt: FxHashSet<u32> = FxHashSet::default();
        if let Err(err) =
            ori_arc::assert_no_unresolved_type_vars(self.pool, lambda, self.interner, &exempt)
        {
            return Err(self.report_primary_seam_violation(
                err,
                "Tag::Var in lambda ARC IR violates PC-2 contract",
            ));
        }

        // Shared specialization sibling invariant. Runs at the same
        // seam as the PC-2 check so BOTH the immediate-emit path
        // (`compile_lambda_arc`) and the two-pass prepare path
        // (`prepare_lambda`) are covered. `specialize_polymorphic_lambdas`
        // must have substituted every `Tag::BoundVar` before this point;
        // survivors mean monomorphization did not finish. Always-on (no `debug_assert!` fail-open);
        // routes through `report_primary_seam_violation` so AOT callers see
        // the BoundVar failure through the same `codegen_errors` counter
        // they rely on for PC-2 failures.
        if let Err(err) = ori_arc::assert_no_unresolved_bound_vars(self.pool, lambda) {
            return Err(self.report_primary_seam_violation(
                err,
                "Tag::BoundVar in shared lambda ARC violates specialization invariant",
            ));
        }

        let is_non_capturing = lambda.num_captures == 0;

        let mut abi = self.compute_arc_function_abi(lambda);

        // Non-capturing lambdas use `ccc` so they match the closure calling
        // convention directly: `(ptr %env, user_args...) -> ret`.
        if is_non_capturing {
            abi.call_conv = select_call_conv(CallConvSite::NonCapturingLambda);
        }

        // Lambda names are globally unique from lowering (include parent function
        // name: `__lambda_{parent}_{idx}`). No renaming needed — the AIMS contract
        // map uses the same names, so ownership lookup succeeds.
        let unique_name = lambda.name;

        let lambda_name_str = self.interner.lookup(unique_name);
        let symbol = self
            .mangler
            .mangle_function(self.module_path, lambda_name_str);

        debug!(
            name = %self.interner.lookup(unique_name),
            symbol,
            params = abi.params.len(),
            non_capturing = is_non_capturing,
            "declaring lambda"
        );

        // Declare with phantom env param for non-capturing lambdas.
        // The emission ABI (registered below) does NOT include the phantom
        // param -- emit_function adjusts llvm_param_idx to skip it.
        let func_id = if is_non_capturing {
            let ptr_ty = self.builder.ptr_type();
            self.declare_function_llvm_with_extra_params(&symbol, &abi, &[ptr_ty])
        } else {
            self.declare_function_llvm(&symbol, &abi)
        };

        if is_non_capturing {
            self.codegen_ctx.non_capturing_lambdas.insert(unique_name);
        }

        self.codegen_ctx
            .functions
            .insert(unique_name, (func_id, abi.clone()));

        // Store capture param ownership so emit_partial_apply can generate
        // correct wrapper functions: borrowed captures skip RcInc (body borrows
        // from env). Env drop RcDec's ALL captures regardless.
        if lambda.num_captures > 0 {
            let capture_ownership: Vec<ori_arc::Ownership> = lambda
                .params
                .iter()
                .take(lambda.num_captures)
                .map(|p| p.ownership)
                .collect();
            self.codegen_ctx
                .lambda_capture_ownership
                .insert(unique_name, capture_ownership);
        }

        Ok((unique_name, func_id, abi))
    }
}
