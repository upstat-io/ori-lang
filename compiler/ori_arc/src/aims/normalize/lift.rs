//! TRMC lifting pre-pass: constructor argument A-normal form verification.
//!
//! The TRMC 4-equation rewrite (Leijen & Lorenzen, JFP 2025) requires all
//! constructor arguments to be in A-normal form — each argument must be a
//! simple variable reference, not an embedded expression.
//!
//! In ARC IR, this invariant is **enforced by the type system**: `Construct`
//! arguments are `Vec<ArcVarId>`, so nested expressions are impossible by
//! construction. The lowering pass (`lower/`) evaluates each sub-expression
//! into a variable before building the `Construct` instruction.
//!
//! This module provides [`lift_constructor_args`], which verifies the
//! invariant via `debug_assert!` rather than performing any transformation.
//! It exists as documentation and as a safety net against future lowering
//! changes that might violate the invariant.
//!
//! # References
//!
//! - Invariant I4 (literature review §04.2): lifting precedes detection.
//! - Leijen & Lorenzen, JFP 2025 — constructor argument normalization.

use crate::ir::{ArcFunction, ArcInstr};

/// Verify that all `Construct` arguments are in A-normal form.
///
/// In ARC IR, `Construct.args` is `Vec<ArcVarId>`, so A-normal form is
/// guaranteed by the type system — this function is a no-op. It exists as:
/// 1. A `debug_assert!` safety net against future lowering changes.
/// 2. Documentation that the TRMC pipeline depends on this invariant.
/// 3. The hook point for an actual lifting transformation if ARC IR ever
///    permits non-variable constructor arguments.
///
/// # Pipeline position
///
/// Called BEFORE [`detect_context_regions`](super::detect::detect_context_regions)
/// in [`normalize_function`](super::normalize_function). Invariant I4:
/// lifting must precede detection so that context region metadata
/// references variables, not embedded expressions.
pub(super) fn lift_constructor_args(func: &ArcFunction) {
    // ARC IR enforces A-normal form by construction: `Construct.args` is
    // `Vec<ArcVarId>`, so embedded expressions are impossible. This pass
    // verifies the structural invariant rather than performing any
    // transformation.
    if cfg!(debug_assertions) {
        verify_construct_args_in_bounds(func);
    }
}

/// Debug-only verification that all `Construct` variables (dst and args)
/// reference valid entries in the function's `var_types` table.
fn verify_construct_args_in_bounds(func: &ArcFunction) {
    let var_count = func.var_types.len();

    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Construct { dst, args, .. } = instr {
                debug_assert!(
                    (dst.raw() as usize) < var_count,
                    "Construct dst v{} out of bounds (var_types len {})",
                    dst.raw(),
                    var_count,
                );
                for (i, arg) in args.iter().enumerate() {
                    debug_assert!(
                        (arg.raw() as usize) < var_count,
                        "Construct field {i} arg v{} out of bounds (var_types len {})",
                        arg.raw(),
                        var_count,
                    );
                }
            }
        }
    }
}
