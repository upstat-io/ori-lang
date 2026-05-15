//! AOT PC-2 hook helper.
//!
//! Extracted from `run_borrow_inference` so the inline PC-2 invariant-check
//! loops do not push the host function past the 100-line and the host file
//! past the 500-line structural limits.
//!
//! The hook walks a pre-mono or mono `arc_fn` plus every lambda extracted with
//! it, invoking `ori_arc::assert_no_unresolved_type_vars` at each. Violations
//! are logged via `tracing::error!` tagged with a site discriminator
//! (`aot_pre_mono` / `aot_pre_mono_lambda` / `aot_mono` / `aot_mono_lambda`).
//! The helper is the sole PC-2 walker at AOT secondary sites; primary seam
//! enforcement is in `ori_llvm::codegen::function_compiler::define_phase`.

#[cfg(feature = "llvm")]
use ori_ir::StringInterner;
#[cfg(feature = "llvm")]
use ori_types::Pool;
#[cfg(feature = "llvm")]
use rustc_hash::FxHashSet;

/// Run the PC-2 `Tag::Var` walker on one `arc_fn` + each of its lambdas.
///
/// `site_fn` tags diagnostics on the `arc_fn` itself (e.g. `"aot_pre_mono"`);
/// `site_lambda` tags diagnostics on each lambda (e.g. `"aot_pre_mono_lambda"`).
/// `exempt` is the scheme-var exemption set; the empty set is invariant for
/// AOT secondary sites (entries are non-generic or already monomorphized).
///
/// Emits `tracing::error!` on each violation but does not propagate — the
/// primary seam in `ori_llvm::codegen::function_compiler::define_phase` is
/// the load-bearing gate; secondary sites are diagnostic localization only.
#[cfg(feature = "llvm")]
pub(super) fn run_pc2_hook_aot(
    pool: &Pool,
    arc_fn: &ori_arc::ArcFunction,
    lambdas: &[ori_arc::ArcFunction],
    interner: &StringInterner,
    exempt: &FxHashSet<u32>,
    site_fn: &'static str,
    site_lambda: &'static str,
) {
    if let Err(err) = ori_arc::assert_no_unresolved_type_vars(pool, arc_fn, interner, exempt) {
        tracing::error!(
            contract_violation = true,
            error = ?err,
            site = site_fn,
            "Tag::Var in AOT ARC IR"
        );
    }
    for lambda in lambdas {
        if let Err(err) = ori_arc::assert_no_unresolved_type_vars(pool, lambda, interner, exempt) {
            tracing::error!(
                contract_violation = true,
                error = ?err,
                site = site_lambda,
                "Tag::Var in AOT lambda ARC IR"
            );
        }
    }
}
