//! Shared ARC processing helpers — the primary seam between the codegen
//! orchestrator and the ARC/AIMS pipeline.
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
    /// Apply borrow annotations, annotate arg ownership, and run the ARC
    /// pipeline on a function.
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
        // PC-2 contract check. Runs BEFORE run_arc_pipeline because the
        // pipeline mutates `arc_func` in place; assertion on post-pipeline
        // IR would validate the wrong structure.
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

        // Apply AIMS param ownership from pre-computed contracts.
        // Lowering defaults all params to Ownership::Owned (lower/mod.rs).
        // AIMS contracts (from compute_aims_contracts) provide the correct
        // Owned/Borrowed per param.
        debug!(name = %self.interner.lookup(name), "processing ARC function");
        self.apply_aims_param_ownership(arc_func);

        // AIMS pipeline handles arg_ownership internally (Step 4: emit_arg_ownership).
        // The reconstructed TypeRegistry surfaces collection / closure
        // UserBurdenSpec to class-ledger burden-op replacement (type_registry.burden(idx)).
        // Receiver-resolved impl-method contracts bind per function so the
        // Step-4b lookups (keyed by bare callee name) see them.
        let augmented = ori_arc::augment_contracts_with_impl_callees(
            arc_func,
            &self.aims_contracts,
            &self.impl_method_contracts,
            self.pool,
        );
        let contracts = augmented.as_ref().unwrap_or(&self.aims_contracts);
        let arc_problems = ori_arc::run_arc_pipeline(
            arc_func,
            self.arc_classifier,
            self.pool,
            self.interner,
            contracts,
            self.type_registry,
            self.verify_arc,
        );
        match arc_problems {
            Ok(problems) => {
                for problem in &problems {
                    debug!(?problem, "ARC pipeline problem");
                }
            }
            Err(verify_errors) => {
                let func_name = self.interner.lookup(name);
                for e in &verify_errors {
                    tracing::error!(function = func_name, "ARC IR verification ICE: {e}");
                }
                self.builder.record_codegen_error_with_msg(format!(
                    "ARC IR verification failed for function '{func_name}' ({} errors)",
                    verify_errors.len()
                ));
            }
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

    /// Translate AIMS `ParamContract` → `Ownership` for every param on `func`.
    ///
    /// Lowering defaults all params to `Ownership::Owned` (see
    /// `ori_arc/src/lower/mod.rs`); AIMS contracts (from `compute_aims_contracts`)
    /// carry the correct Owned/Borrowed classification per param. This helper
    /// consumes the interprocedural contract for `func.name` (when present) and
    /// writes the per-param ownership in-place. Callers: `process_arc_function`
    /// (top-level) + `declare_and_process_lambda` (lambdas); both forms share
    /// identical translation logic.
    pub(super) fn apply_aims_param_ownership(&self, func: &mut ori_arc::ArcFunction) {
        if let Some(contract) = self.aims_contracts.get(&func.name) {
            for (param, pc) in func.params.iter_mut().zip(&contract.params) {
                param.ownership = match pc.access {
                    ori_arc::aims::lattice::AccessClass::Borrowed => ori_arc::Ownership::Borrowed,
                    ori_arc::aims::lattice::AccessClass::Owned => ori_arc::Ownership::Owned,
                };
            }
        }
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
        // PC-2 contract check. Runs BEFORE run_arc_pipeline below because
        // the pipeline mutates `lambda` in place; assertion on post-pipeline
        // IR would validate the wrong structure. Mirrors the sibling check
        // in `process_arc_function`.
        let exempt: FxHashSet<u32> = FxHashSet::default();
        if let Err(err) =
            ori_arc::assert_no_unresolved_type_vars(self.pool, lambda, self.interner, &exempt)
        {
            return Err(self.report_primary_seam_violation(
                err,
                "Tag::Var in lambda ARC IR violates PC-2 contract",
            ));
        }

        // Monomorphization-resolution sibling invariant. Runs at the same
        // seam as the PC-2 check so BOTH the immediate-emit path
        // (`compile_lambda_arc`) and the two-pass prepare path
        // (`prepare_lambda`) are covered. `resolve_all_lambda_bound_vars`
        // must have substituted every `Tag::BoundVar` before this point;
        // survivors mean monomorphization did not finish. Always-on (no `debug_assert!` fail-open);
        // routes through `report_primary_seam_violation` so AOT callers see
        // the BoundVar failure through the same `codegen_errors` counter
        // they rely on for PC-2 failures.
        if let Err(err) = ori_arc::assert_no_unresolved_bound_vars_in_params(self.pool, lambda) {
            return Err(self.report_primary_seam_violation(
                err,
                "Tag::BoundVar in lambda params violates monomorphization-resolution invariant",
            ));
        }

        let is_non_capturing = lambda.num_captures == 0;

        // Apply AIMS param ownership from pre-computed contracts BEFORE the
        // name change below. The contracts map uses the original lambda name
        // (e.g., `__lambda_0` from lowering). Lambdas need correct
        // Owned/Borrowed annotations so that collect_all_borrowed_defs
        // correctly identifies borrowed params and their Let aliases.
        // Without this, edge cleanup emits spurious RcDec for
        // borrowed-param aliases (double-free on captured non-scalar
        // values like str, [T]).
        self.apply_aims_param_ownership(lambda);

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

        // ARC processing — AIMS pipeline handles arg_ownership internally.
        // Lambda path mirrors the parent path: the reconstructed TypeRegistry
        // surfaces closure-env / collection UserBurdenSpec to class-ledger
        // burden-op replacement, and receiver-resolved impl-method contracts
        // bind per function.
        let augmented = ori_arc::augment_contracts_with_impl_callees(
            lambda,
            &self.aims_contracts,
            &self.impl_method_contracts,
            self.pool,
        );
        let contracts = augmented.as_ref().unwrap_or(&self.aims_contracts);
        let arc_problems = ori_arc::run_arc_pipeline(
            lambda,
            self.arc_classifier,
            self.pool,
            self.interner,
            contracts,
            self.type_registry,
            self.verify_arc,
        );
        match arc_problems {
            Ok(problems) => {
                for problem in &problems {
                    debug!(?problem, "ARC pipeline problem (lambda)");
                }
            }
            Err(verify_errors) => {
                for e in &verify_errors {
                    tracing::error!("ARC IR verification ICE (lambda): {e}");
                }
                self.builder.record_codegen_error_with_msg(format!(
                    "ARC IR verification failed for lambda ({} errors)",
                    verify_errors.len()
                ));
            }
        }

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
