//! Deferred monomorphization recording for generic-calling-generic call sites.

use ori_ir::{ExprId, Name};
use rustc_hash::FxHashMap;

use crate::Idx;

use crate::infer::InferEngine;

/// Record a deferred monomorphization call when a generic function calls another
/// generic with type variables still unresolved.
///
/// Maps each callee scheme var to either a caller scheme var position (for vars
/// that depend on the caller's type params) or a concrete type (for vars that
/// are already resolved at the call site).
pub(super) fn record_deferred_mono_call(
    engine: &mut InferEngine<'_>,
    call_expr_id: ExprId,
    callee: Name,
    callee_scheme_var_ids: &[u32],
    var_subst: &FxHashMap<u32, Idx>,
    callee_param_types: Vec<Idx>,
    callee_return_type: Idx,
) {
    let Some(caller) = engine.current_function() else {
        return;
    };

    // Get caller's signature data (borrow dance: clone to release immutable borrow).
    let caller_sig_data = engine.get_signature(caller).map(|sig| {
        (
            sig.scheme_var_ids.clone(),
            sig.param_types.clone(),
            sig.generic_param_mapping.clone(),
        )
    });
    let Some((caller_svids, caller_ptypes, caller_gpm)) = caller_sig_data else {
        return;
    };

    // Resolve each caller scheme var through the engine to find its root.
    let caller_roots: Vec<Idx> = caller_svids
        .iter()
        .enumerate()
        .map(|(pos, _)| {
            if let Some(Some(param_idx)) = caller_gpm.get(pos) {
                engine.resolve(caller_ptypes[*param_idx])
            } else {
                let sv_id = caller_svids[pos];
                let pool = engine.pool();
                let var_idx = pool.var_idx_for_id(sv_id);
                var_idx.map_or(crate::Idx::ERROR, |idx| engine.resolve(idx))
            }
        })
        .collect();

    // Map each callee var to a caller scheme var position, a concrete type, or
    // a deferred transient type.
    let mut semantic_subst: Vec<(u32, crate::DeferredVarBinding)> = Vec::new();
    for (&callee_var_id, &concrete_idx) in var_subst {
        // A type carrying `HAS_VAR` (but not `HAS_ERROR` — poison is gated
        // upstream) is transient: a bare `Tag::Var` OR a composite (`[Var]`,
        // `Option<Var>`) whose interior var resolves later in the same body
        // pass. A fully-concrete type has neither flag.
        let transient = engine.pool().flags(concrete_idx).has_vars();
        if !transient {
            semantic_subst.push((
                callee_var_id,
                crate::DeferredVarBinding::Concrete(concrete_idx),
            ));
        } else if let Some(pos) = caller_roots.iter().position(|&r| r == concrete_idx) {
            // Generic caller: a bare transient var matching a caller scheme-var
            // root binds positionally (resolved when the caller instantiates).
            semantic_subst.push((
                callee_var_id,
                crate::DeferredVarBinding::CallerSchemeVar(pos),
            ));
        } else {
            // Non-generic caller (no scheme vars to map to) OR a composite
            // transient type (`[Var]`): bind the type idx and re-resolve it at
            // the deferred phase against the fully-linked pool, rather than
            // dropping the instance (AOT missing-mono).
            semantic_subst.push((
                callee_var_id,
                crate::DeferredVarBinding::DeferredType { idx: concrete_idx },
            ));
        }
    }

    if semantic_subst.len() == callee_scheme_var_ids.len() {
        let deferred = crate::DeferredMonoCall {
            caller,
            callee,
            callee_scheme_var_ids: callee_scheme_var_ids.to_vec(),
            var_subst: semantic_subst,
            callee_param_types,
            callee_return_type,
            call_expr_id,
        };
        tracing::debug!(
            caller = ?caller,
            callee = ?callee,
            subst = ?deferred.var_subst,
            "recorded deferred mono call (generic calling generic)"
        );
        engine.record_deferred_mono_call(deferred);
    }
}
