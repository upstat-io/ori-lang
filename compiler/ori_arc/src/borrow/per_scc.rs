//! Per-SCC borrow inference (Sections 12.5–12.6).
//!
//! Decomposes borrow inference by SCC (strongly connected component).
//! Non-recursive SCCs use a single pass; recursive SCCs iterate to a
//! fixed point within the SCC only. Both accept pre-resolved external
//! callee signatures, enabling Salsa-tracked incremental queries.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::ir::ArcFunction;
use crate::ownership::{AnnotatedParam, AnnotatedSig, Ownership};
use crate::ArcClassification;

/// Initialize a single function's parameters as Borrowed (non-scalar) or Owned (scalar).
///
/// Same logic as the whole-program `initialize_all_borrowed` but for a single
/// function, returning a standalone `AnnotatedSig` rather than inserting into a
/// map.
pub fn initialize_single_borrowed(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
) -> AnnotatedSig {
    let params: Vec<AnnotatedParam> = func
        .params
        .iter()
        .map(|p| {
            let ownership = if classifier.is_scalar(p.ty) {
                Ownership::Owned
            } else {
                Ownership::Borrowed
            };
            AnnotatedParam {
                name: Name::from_raw(p.var.raw()),
                ty: p.ty,
                ownership,
            }
        })
        .collect();

    AnnotatedSig {
        params,
        return_type: func.return_type,
    }
}

/// Infer borrow annotations for a single non-recursive function (Section 12.5).
///
/// Unlike [`super::infer_borrows_scc`] which handles the full SCC decomposition,
/// this function analyzes ONE function in a single pass. Callee signatures
/// are provided via `external_sigs` (already finalized by earlier SCCs).
///
/// Suitable for non-recursive SCCs (single function, no self-call).
///
/// # Arguments
///
/// * `func` — the ARC IR function to analyze.
/// * `external_sigs` — pre-resolved callee signatures from other SCCs.
/// * `classifier` — type classifier for scalar vs ref types.
/// * `borrowing_builtins` — method names whose receiver is always borrowed.
#[expect(
    clippy::implicit_hasher,
    reason = "FxHashMap/FxHashSet are the concrete types used throughout"
)]
pub fn infer_borrow_single(
    func: &ArcFunction,
    external_sigs: &FxHashMap<Name, AnnotatedSig>,
    classifier: &dyn ArcClassification,
    borrowing_builtins: &FxHashSet<Name>,
) -> AnnotatedSig {
    let mut sig = initialize_single_borrowed(func, classifier);
    let empty_local = FxHashMap::default();
    super::update_ownership_inner(
        func,
        &mut sig,
        &empty_local,
        external_sigs,
        borrowing_builtins,
    );
    sig
}

/// Infer borrow annotations for a mutually recursive SCC (Section 12.6).
///
/// Runs fixed-point iteration over the SCC members only, using pre-resolved
/// `external_sigs` for callees outside the SCC. Convergence is guaranteed
/// in at most `sum_of_params + 1` iterations (monotonic: Borrowed → Owned).
///
/// # Arguments
///
/// * `scc_functions` — ARC IR functions in this SCC (mutually recursive group).
/// * `external_sigs` — pre-resolved callee signatures from other SCCs.
/// * `classifier` — type classifier for scalar vs ref types.
/// * `borrowing_builtins` — method names whose receiver is always borrowed.
#[expect(
    clippy::implicit_hasher,
    reason = "FxHashMap/FxHashSet are the concrete types used throughout"
)]
pub fn infer_borrow_fixed_point(
    scc_functions: &[&ArcFunction],
    external_sigs: &FxHashMap<Name, AnnotatedSig>,
    classifier: &dyn ArcClassification,
    borrowing_builtins: &FxHashSet<Name>,
) -> FxHashMap<Name, AnnotatedSig> {
    // Initialize all SCC members.
    let mut local_sigs = FxHashMap::default();
    local_sigs.reserve(scc_functions.len());
    for &func in scc_functions {
        local_sigs.insert(func.name, initialize_single_borrowed(func, classifier));
    }

    // Fixed-point iteration within the SCC.
    let mut changed = true;
    let mut iterations = 0u32;
    while changed {
        changed = false;
        for &func in scc_functions {
            let mut my_sig = local_sigs[&func.name].clone();
            if super::update_ownership_inner(
                func,
                &mut my_sig,
                &local_sigs,
                external_sigs,
                borrowing_builtins,
            ) {
                local_sigs.insert(func.name, my_sig);
                changed = true;
            }
        }
        iterations += 1;
    }

    // Convergence bound: each iteration promotes at least one param.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "param count per function bounded by u32::MAX in practice"
    )]
    let total_params: u32 = scc_functions.iter().map(|f| f.params.len() as u32).sum();
    debug_assert!(
        iterations <= total_params.saturating_add(1),
        "fixed-point exceeded convergence bound: {iterations} iterations for {total_params} params"
    );

    local_sigs
}
