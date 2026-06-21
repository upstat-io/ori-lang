//! End-of-body finalization helpers for [`InferEngine`].
//!
//! Two independent normalization passes invoked by each body-group pass
//! (`check::bodies`) after `InferEngine` body-checking completes:
//!
//! 1. `default_unbound_vars_*` — defaulting of genuinely unconstrained
//!    `Tag::Var` leaves reachable from empty collection-literal expression
//!    roots to `Idx::NEVER` per "End-of-body defaulting
//!    pre-pass".
//! 2. `normalize_body_generalized_to_bound_var*` — post-generalization
//!    rewrite of `Tag::Var(Generalized)` leaves in `expr_types` + sig
//!    positions to `Tag::BoundVar` per.
//!
//! Both are wrapper/core pairs — the wrapper mutates a full [`FunctionSig`]
//! and refreshes Merkle hashes; the core operates on loose `param_types` /
//! `return_type` slices.

use ori_ir::{ExprArena, ExprId, ExprKind};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::pool::substitute::substitute_in_pool;
use crate::{FunctionSig, Idx, Pool, Tag, TypeFlags, VarState};

use super::{ExprIndex, InferEngine};

#[cfg(test)]
mod tests;

impl InferEngine<'_> {
    /// Convenience wrapper: apply [`default_unbound_vars_in_scope`] to a whole
    /// [`FunctionSig`] and refresh its Merkle hashes on success.
    ///
    /// `exempt` is a pre-built set of legitimate polymorphic var ids
    /// constructed by the caller via
    /// `check::validators::build_exempt_var_ids`; passing it in avoids an
    /// `infer → check` upward import per.
    ///
    /// Callers that only have loose `param_types` / `return_type` (e.g.,
    /// `check_impl_method` which constructs its `FunctionSig` at the end via
    /// `build_method_sig`) should call
    /// [`InferEngine::default_unbound_vars_in_scope`] directly and construct
    /// the hashes themselves.
    ///
    /// [`default_unbound_vars_in_scope`]: InferEngine::default_unbound_vars_in_scope
    pub fn default_unbound_vars_from_empty_literals(
        &mut self,
        arena: &ExprArena,
        expr_types: &mut FxHashMap<ExprIndex, Idx>,
        sig: &mut FunctionSig,
        exempt: &FxHashSet<u32>,
    ) {
        debug_assert!(
            self.body_inference_complete,
            "body_inference_complete invariant violated — defaulting called before body inference finished"
        );
        let changed = self.default_unbound_vars_in_scope(
            arena,
            expr_types,
            &mut sig.param_types,
            &mut sig.return_type,
            exempt,
        );
        if changed {
            // Refresh Merkle hashes so cross-module identity
            // (`output/mod.rs:442-457`) reflects the defaulted types.
            let pool = self.pool();
            sig.param_hashes = sig.param_types.iter().map(|&idx| pool.hash(idx)).collect();
            sig.return_hash = pool.hash(sig.return_type);
        }
    }

    /// Core defaulting pass: substitute unbound vars reachable from
    /// empty-literal expr roots in `expr_types` to [`Idx::NEVER`], and
    /// propagate the substitution through `param_types` + `return_type`.
    ///
    /// Returns `true` iff any var was defaulted. Callers holding a full
    /// [`FunctionSig`] should prefer the wrapper
    /// [`InferEngine::default_unbound_vars_from_empty_literals`] which
    /// ALSO refreshes `param_hashes` + `return_hash`. Callers building a
    /// sig later (e.g., `check_impl_method` → `build_method_sig`) use this
    /// method and let `build_method_sig` compute fresh hashes from the
    /// now-defaulted types.
    pub fn default_unbound_vars_in_scope(
        &mut self,
        arena: &ExprArena,
        expr_types: &mut FxHashMap<ExprIndex, Idx>,
        param_types: &mut [Idx],
        return_type: &mut Idx,
        exempt: &FxHashSet<u32>,
    ) -> bool {
        debug_assert!(
            self.body_inference_complete,
            "body_inference_complete invariant violated — defaulting called before body inference finished"
        );
        // 1. Collect unbound var ids reachable from defaulting-root expr roots.
        //    `EmptyLiteralRoot` walks the full stored type; `IntroducerSlot`
        //    restricts the walk to the slot whose fresh var was introduced
        //    at the constructor (Ok/Err's complementary slot), preserving
        //    legitimate E2005 on unrelated payload generics.
        let mut var_subst: FxHashMap<u32, Idx> = FxHashMap::default();
        for (&expr_idx, &ty) in expr_types.iter() {
            let Ok(expr_id_raw) = u32::try_from(expr_idx) else {
                continue;
            };
            let expr_id = ExprId::new(expr_id_raw);
            let expr = arena.get_expr(expr_id);
            match is_defaulting_root(arena, &expr.kind) {
                DefaultingClassification::NotARoot => {}
                DefaultingClassification::EmptyLiteralRoot => {
                    collect_unbound_reachable_vars(self.pool(), ty, exempt, &mut var_subst);
                }
                DefaultingClassification::IntroducerSlot(slot) => {
                    let resolved = self.pool().resolve_fully(ty);
                    if self.pool().tag(resolved) != Tag::Result {
                        continue;
                    }
                    let slot_ty = match slot {
                        ResultSlot::Ok => self.pool().result_ok(resolved),
                        ResultSlot::Err => self.pool().result_err(resolved),
                    };
                    collect_unbound_reachable_vars(self.pool(), slot_ty, exempt, &mut var_subst);
                }
            }
        }
        if var_subst.is_empty() {
            return false;
        }

        // 2. Substitute through expr_types and loose sig fields.
        let pool = self.pool_mut();
        for ty in expr_types.values_mut() {
            *ty = substitute_in_pool(pool, *ty, &var_subst);
        }
        for ty in param_types.iter_mut() {
            *ty = substitute_in_pool(pool, *ty, &var_subst);
        }
        *return_type = substitute_in_pool(pool, *return_type, &var_subst);

        // 3. Defense-in-depth: link the vars in the pool too, so any raw-Idx
        //    consumer that slipped past `substitute_in_pool` resolves to
        //    `Idx::NEVER` via `resolve_fully`. No `Pool::link_var` helper on
        //    HEAD — use the canonical direct-assignment pattern from
        //    `unify/mod.rs:289`.
        for (&var_id, &target) in &var_subst {
            *pool.var_state_mut(var_id) = VarState::Link { target };
        }
        true
    }

    /// Convenience wrapper mutating a whole [`FunctionSig`] in-place and
    /// refreshing its Merkle hashes after normalization. Callers holding
    /// only loose `param_types` / `return_type` (e.g., `check_impl_method`
    /// which constructs its sig at the end via `build_method_sig`) should
    /// call [`InferEngine::normalize_body_generalized_to_bound_var`] directly
    /// and let the caller construct hashes from the normalized fields.
    pub fn normalize_body_generalized_to_bound_var_sig(
        &mut self,
        expr_types: &mut FxHashMap<ExprIndex, Idx>,
        sig: &mut FunctionSig,
    ) {
        let before_params = sig.param_types.clone();
        let before_return = sig.return_type;
        self.normalize_body_generalized_to_bound_var(
            expr_types,
            &mut sig.param_types,
            &mut sig.return_type,
            &sig.scheme_var_ids,
        );
        // If anything changed, refresh the Merkle hashes so cross-module
        // identity (`output/mod.rs:442-457`) reflects the normalized shape.
        let changed = sig.param_types != before_params || sig.return_type != before_return;
        if changed {
            let pool = self.unify.pool();
            sig.param_hashes = sig.param_types.iter().map(|&idx| pool.hash(idx)).collect();
            sig.return_hash = pool.hash(sig.return_type);
        }
    }

    /// Normalize `Tag::Var` leaves matching generalized/scheme var ids to
    /// `Tag::BoundVar` across `expr_types`, `param_types`, and `return_type`
    /// per.
    ///
    /// Scheme bodies in the pool are already rewritten to `Tag::BoundVar`
    /// leaves by [`crate::UnifyEngine::generalize`] via
    /// `rewrite_generalized_to_bound_var`, but the positions in
    /// `expr_types`, `FunctionSig.param_types`, and `FunctionSig.return_type`
    /// continue to reference the pre-generalize `Tag::Var` idxs whose
    /// `var_state` was mutated to `Generalized` in place. Without this
    /// normalization pass, those `expr_types` / sig-position idxs remain
    /// `Tag::Var(Generalized)` post-typeck, causing `validate_body_types`
    /// (PC-2) to either (a) spuriously flag them as
    /// `E2005` if the exemption arm is stripped, or (b) silently permit them
    /// to leak to downstream phases under the exemption arm — both paths are
    /// partial-migration leaks relative to the `§SC-1` target.
    ///
    /// The rewrite is driven by the union of:
    ///   1. `self.pending_generalized_vars` — drained here; these are var
    ///      ids captured by inner `let` generalization during the body.
    ///   2. `sig_scheme_var_ids` — the caller's pre-collected scheme var
    ///      ids (e.g., top-level polymorphic function sigs where
    ///      `generalize()` is never invoked on the body but `scheme_var_ids`
    ///      were populated in the signatures pass).
    ///
    /// For each var id in the union, constructs a substitution entry
    /// `{var_id → Pool::bound_var(var_id)}` and applies it via
    /// [`substitute_in_pool`] — reuses the canonical recursion skeleton used
    /// by `rewrite_generalized_to_bound_var`.
    ///
    /// Runs per-body: `pending_generalized_vars` is drained on entry and is
    /// empty on exit. Callers invoke this immediately after the end-of-body
    /// defaulting pass ([`InferEngine::default_unbound_vars_from_empty_literals`])
    /// and before `validate_body_types`. Ordering keeps defaulting's
    /// `Idx::NEVER` substitutions intact while ensuring the validator sees
    /// the `SC-1` (scheme bound-var layout) target shape.
    pub fn normalize_body_generalized_to_bound_var(
        &mut self,
        expr_types: &mut FxHashMap<ExprIndex, Idx>,
        param_types: &mut [Idx],
        return_type: &mut Idx,
        sig_scheme_var_ids: &[u32],
    ) {
        // Union of pending (from inner generalize() calls) + sig's scheme
        // var ids (from signatures pass, for top-level polymorphic fns).
        let mut all_vars: Vec<u32> = std::mem::take(&mut self.pending_generalized_vars);
        all_vars.extend(sig_scheme_var_ids.iter().copied());
        all_vars.sort_unstable();
        all_vars.dedup();

        if all_vars.is_empty() {
            return;
        }

        let pool = self.unify.pool_mut();
        let subst: FxHashMap<u32, Idx> = all_vars
            .iter()
            .map(|&id| (id, pool.bound_var(id)))
            .collect();

        for ty in expr_types.values_mut() {
            *ty = substitute_in_pool(pool, *ty, &subst);
        }
        for ty in param_types.iter_mut() {
            *ty = substitute_in_pool(pool, *ty, &subst);
        }
        *return_type = substitute_in_pool(pool, *return_type, &subst);
    }
}

/// Which slot of a `Result<T, E>` carries the constructor-introduced fresh
/// `Tag::Var` for [`DefaultingClassification::IntroducerSlot`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResultSlot {
    /// `Err(_)` introduces a fresh var in the `Ok` slot.
    Ok,
    /// `Ok(_)` introduces a fresh var in the `Err` slot.
    Err,
}

/// Defaulting-root classification per expression-root kind.
///
/// `NotARoot` — `kind` is not a defaulting root; skip.
///
/// `EmptyLiteralRoot` — empty collection literal (`[]`, `{}`,
/// `[...]`, `{...}`). The entire stored type is walked; every reachable
/// unbound var is defaulted to `Idx::NEVER`. This is the original
/// empty-literal defaulting behavior.
///
/// `IntroducerSlot(slot)` — polymorphic constructor (`Ok(_)`, `Err(_)`)
/// that introduces a fresh `Tag::Var` only in `slot` of the resulting
/// `Result<T, E>`. ONLY that slot is walked; the other slot carries
/// `inner_ty` (whatever `infer_expr(inner)` produced) and belongs to the
/// inner expression's own defaulting-root classification. Walking the full
/// `Result` would default unrelated payload generics and silently mask
/// legitimate `E2005`s on those payload vars — the introducer-only walk
/// is the structural invariant cure for that hazard.
///
/// `None` returns `EmptyLiteralRoot` because `infer_none` allocates a
/// fresh var as the sole inner of `Option<_>`; walking the whole
/// `Option<Var>` is equivalent to walking the single child slot, and the
/// existing collector handles it without a dedicated `IntroducerSlot`
/// variant.
///
/// `Some(_)` is EXCLUDED — `infer_some` calls `infer_expr(inner)` (no
/// fresh var introduced at the `Some` level); any reachable var belongs
/// to the inner expression's own classification. Adding `Some` here
/// would default payload vars that `check_some`'s bidirectional gate
/// is responsible for pinning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DefaultingClassification {
    NotARoot,
    EmptyLiteralRoot,
    IntroducerSlot(ResultSlot),
}

/// Classify `kind` for end-of-body defaulting. See
/// [`DefaultingClassification`] for the per-variant semantics.
fn is_defaulting_root(arena: &ExprArena, kind: &ExprKind) -> DefaultingClassification {
    match kind {
        ExprKind::List(range) => {
            if arena.get_expr_list(*range).is_empty() {
                DefaultingClassification::EmptyLiteralRoot
            } else {
                DefaultingClassification::NotARoot
            }
        }
        ExprKind::ListWithSpread(range) => {
            if arena.get_list_elements(*range).is_empty() {
                DefaultingClassification::EmptyLiteralRoot
            } else {
                DefaultingClassification::NotARoot
            }
        }
        ExprKind::Map(range) => {
            if arena.get_map_entries(*range).is_empty() {
                DefaultingClassification::EmptyLiteralRoot
            } else {
                DefaultingClassification::NotARoot
            }
        }
        ExprKind::MapWithSpread(range) => {
            if arena.get_map_elements(*range).is_empty() {
                DefaultingClassification::EmptyLiteralRoot
            } else {
                DefaultingClassification::NotARoot
            }
        }
        // `infer_none` allocates `Option<fresh_var>`; the existing full-tree
        // walk over the single-child `Option<Var>` collects the introduced
        // var without needing an `IntroducerSlot` arm.
        ExprKind::None => DefaultingClassification::EmptyLiteralRoot,
        // `infer_ok` allocates `Result<inner_ty, fresh_var>` — the err slot
        // is the introducer; payload vars in `inner_ty` belong to the inner
        // expression's classification.
        ExprKind::Ok(_) => DefaultingClassification::IntroducerSlot(ResultSlot::Err),
        // `infer_err` allocates `Result<fresh_var, inner_ty>` — the ok slot
        // is the introducer; payload vars in `inner_ty` belong to the inner
        // expression's classification.
        ExprKind::Err(_) => DefaultingClassification::IntroducerSlot(ResultSlot::Ok),
        _ => DefaultingClassification::NotARoot,
    }
}

/// Walk the compound type rooted at `ty`, adding every
/// `VarState::Unbound` var id (not in `exempt`) to `var_subst` with target
/// [`Idx::NEVER`]. Mirrors the traversal in
/// `check::validators::collect_first_unbound_var` — no visited-set needed
/// because occurs-check prevents cyclic types from
/// reaching this code path.
fn collect_unbound_reachable_vars(
    pool: &Pool,
    ty: Idx,
    exempt: &FxHashSet<u32>,
    var_subst: &mut FxHashMap<u32, Idx>,
) {
    let resolved = pool.resolve_fully(ty);
    if !pool.flags(resolved).contains(TypeFlags::HAS_VAR) {
        return;
    }
    match pool.tag(resolved) {
        Tag::Var => {
            let var_id = pool.data(resolved);
            if let VarState::Unbound { .. } = pool.var_state(var_id) {
                if !exempt.contains(&var_id) {
                    var_subst.insert(var_id, Idx::NEVER);
                }
            }
        }
        Tag::BoundVar => { /* scheme-quantified; skip */ }
        _ => {
            pool.visit_children(resolved, |child| {
                collect_unbound_reachable_vars(pool, child, exempt, var_subst);
            });
        }
    }
}
