//! Call-site fixups after lambda monomorphization.
//!
//! After multi-instantiation cloning (`multi_inst`) or single-instantiation
//! mapping (`single_inst`) runs, the parent `ArcFunction`'s `var_types`,
//! `var_reprs`, and RC operations may be stale. These helpers:
//!
//! - `fixup_call_result_types` — rewrite `ApplyIndirect` / `InvokeIndirect`
//!   result types when the closure target got specialized.
//! - `fixup_parent_var_reprs_and_rc_ops` — recompute `var_reprs` and drop
//!   now-invalid RC operations.
//! - `find_call_site_return_type` / `resolve_call_result_type` — look up
//!   concrete return types via the `PartialApply → Let → ApplyIndirect`
//!   chain.

use ori_types::{Idx, Tag};

use super::type_predicates::contains_var;

/// Fix up parent `var_types` and instruction `ty` fields for call result variables
/// that still hold unresolved Var types from let-polymorphism.
pub(super) fn fixup_call_result_types(
    parent: &mut ori_arc::ArcFunction,
    pa_dst: ori_arc::ir::ArcVarId,
    pool: &ori_types::Pool,
) {
    for block_idx in 0..parent.blocks.len() {
        for instr_idx in 0..parent.blocks[block_idx].body.len() {
            if let ori_arc::ir::ArcInstr::ApplyIndirect {
                dst: result_dst,
                closure,
                ty,
                ..
            } = &parent.blocks[block_idx].body[instr_idx]
            {
                let result_dst = *result_dst;
                let closure = *closure;
                let result_ty = pool.resolve_fully(*ty);
                if matches!(pool.tag(result_ty), Tag::Var | Tag::Scheme)
                    && is_rewritten_closure(parent, closure, pa_dst)
                {
                    if let Some(concrete) = resolve_call_result_type(parent, result_dst, pool) {
                        parent.var_types[result_dst.index()] = concrete;
                        if let ori_arc::ir::ArcInstr::ApplyIndirect { ty, .. } =
                            &mut parent.blocks[block_idx].body[instr_idx]
                        {
                            *ty = concrete;
                        }
                    }
                }
            }
        }
        // Check `InvokeIndirect` terminator.
        let needs_fixup = if let ori_arc::ir::ArcTerminator::InvokeIndirect {
            dst: result_dst,
            closure,
            ty,
            ..
        } = &parent.blocks[block_idx].terminator
        {
            let result_ty = pool.resolve_fully(*ty);
            if matches!(pool.tag(result_ty), Tag::Var | Tag::Scheme)
                && is_rewritten_closure(parent, *closure, pa_dst)
            {
                resolve_call_result_type(parent, *result_dst, pool)
                    .map(|concrete| (*result_dst, concrete))
            } else {
                None
            }
        } else {
            None
        };
        if let Some((result_dst, concrete)) = needs_fixup {
            parent.var_types[result_dst.index()] = concrete;
            if let ori_arc::ir::ArcTerminator::InvokeIndirect { ty, .. } =
                &mut parent.blocks[block_idx].terminator
            {
                *ty = concrete;
            }
        }
    }
}

/// Check if a closure variable was rewritten from a Let copy of `pa_dst`.
/// After rewriting, the `Let` instruction is replaced with `PartialApply`, but
/// the variable ID is preserved.
fn is_rewritten_closure(
    parent: &ori_arc::ArcFunction,
    closure: ori_arc::ir::ArcVarId,
    pa_dst: ori_arc::ir::ArcVarId,
) -> bool {
    // After rewrite, the closure var is a PartialApply dst (was a Let dst copying pa_dst).
    // Check if any PartialApply instruction has this var as dst.
    for block in &parent.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::PartialApply { dst, .. } = instr {
                if *dst == closure {
                    return true;
                }
            }
        }
    }
    // Also match the original pa_dst in case it wasn't rewritten.
    closure == pa_dst
}

/// Find the concrete return type for a specific instantiation by following
/// the `PartialApply` → `Let` copy → `ApplyIndirect` → result chain.
///
/// For let-polymorphic lambdas where `Let` copies have concrete params but Scheme
/// return types, the concrete return is determined at the call site: the
/// `ApplyIndirect` result variable has the concrete type from the type checker.
pub(super) fn find_call_site_return_type(
    parent: &ori_arc::ArcFunction,
    pa_dst: ori_arc::ir::ArcVarId,
    concrete_fn_ty: Idx,
    pool: &ori_types::Pool,
) -> Option<Idx> {
    let target_params = pool.function_params(concrete_fn_ty);
    let target_key: Vec<Idx> = target_params
        .iter()
        .map(|p| pool.resolve_fully(*p))
        .collect();

    // Find the Let copy whose resolved param types match this instantiation.
    for block in &parent.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::Let {
                dst,
                value: ori_arc::ir::ArcValue::Var(src),
                ..
            } = instr
            {
                if *src != pa_dst {
                    continue;
                }
                let ty = parent.var_type(*dst);
                let resolved = pool.resolve_fully(ty);
                if pool.tag(resolved) != Tag::Function {
                    continue;
                }
                let params = pool.function_params(resolved);
                let params_key: Vec<Idx> = params.iter().map(|p| pool.resolve_fully(*p)).collect();
                if params_key != target_key {
                    continue;
                }
                // Found matching Let copy. Follow to ApplyIndirect or
                // InvokeIndirect (terminator with unwind) to get the result type.
                let let_dst = *dst;
                for b in &parent.blocks {
                    // Check instructions for ApplyIndirect.
                    for i in &b.body {
                        if let ori_arc::ir::ArcInstr::ApplyIndirect {
                            dst: result_dst,
                            closure,
                            ..
                        } = i
                        {
                            if *closure == let_dst {
                                if let Some(ret) =
                                    resolve_call_result_type(parent, *result_dst, pool)
                                {
                                    return Some(ret);
                                }
                            }
                        }
                    }
                    // Check terminator for InvokeIndirect (calls with unwind).
                    if let ori_arc::ir::ArcTerminator::InvokeIndirect {
                        dst: result_dst,
                        closure,
                        ..
                    } = &b.terminator
                    {
                        if *closure == let_dst {
                            if let Some(ret) = resolve_call_result_type(parent, *result_dst, pool) {
                                return Some(ret);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Resolve the concrete type of a call result variable.
///
/// If the `var_type` is already concrete, returns it directly. If it's still a
/// Var/Scheme (common for let-polymorphic return types), looks for a downstream
/// narrowing Let copy that assigns the concrete type.
fn resolve_call_result_type(
    func: &ori_arc::ArcFunction,
    result_var: ori_arc::ir::ArcVarId,
    pool: &ori_types::Pool,
) -> Option<Idx> {
    let result_ty = func.var_type(result_var);
    let resolved = pool.resolve_fully(result_ty);
    if !matches!(pool.tag(resolved), Tag::BoundVar | Tag::Var | Tag::Scheme) {
        return Some(resolved);
    }
    // Result is still Var/Scheme — look for a downstream narrowing Let copy.
    for block in &func.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::Let {
                value: ori_arc::ir::ArcValue::Var(src),
                ty,
                ..
            } = instr
            {
                if *src == result_var {
                    let narrowed = pool.resolve_fully(*ty);
                    if !matches!(pool.tag(narrowed), Tag::BoundVar | Tag::Var | Tag::Scheme)
                        && !contains_var(pool, narrowed)
                    {
                        return Some(narrowed);
                    }
                }
            }
        }
    }
    None
}

/// Recompute `var_reprs` and fix stale RC operations after type fixups.
///
/// After `fixup_call_result_types` changes `parent.var_types` (e.g., from
/// unresolved `Var` to concrete `int`), the existing `var_reprs` and RC
/// instruction strategies become stale:
/// - A var that was `RcPointer` (from unresolved `Var`) may now be `Scalar`
///   → its `RcInc`/`RcDec` must be removed (RC on scalar = crash)
/// - A var that was `RcPointer` may now be `FatValue` or `Aggregate`
///   → its RC strategy must be updated to match the new repr
pub(super) fn fixup_parent_var_reprs_and_rc_ops(
    parent: &mut ori_arc::ArcFunction,
    classifier: &dyn ori_arc::ArcClassification,
    pool: &ori_types::Pool,
) {
    if parent.var_reprs.is_empty() {
        // var_reprs not yet computed (pre-pipeline) — nothing to fix.
        return;
    }

    let old_reprs = parent.var_reprs.clone();
    let new_reprs = ori_arc::compute_var_reprs(parent, classifier, pool);

    // Check if anything actually changed.
    if old_reprs == new_reprs {
        parent.var_reprs = new_reprs;
        return;
    }

    // Build set of vars whose repr changed.
    let changed_to_scalar: rustc_hash::FxHashSet<ori_arc::ir::ArcVarId> = old_reprs
        .iter()
        .zip(new_reprs.iter())
        .enumerate()
        .filter(|(_, (old, new))| *old != *new && **new == ori_arc::ir::ValueRepr::Scalar)
        .map(|(i, _)| ori_arc::ir::ArcVarId::new(i as u32))
        .collect();

    let changed_repr: rustc_hash::FxHashMap<ori_arc::ir::ArcVarId, ori_arc::ir::ValueRepr> =
        old_reprs
            .iter()
            .zip(new_reprs.iter())
            .enumerate()
            .filter(|(_, (old, new))| *old != *new && **new != ori_arc::ir::ValueRepr::Scalar)
            .map(|(i, (_, new))| (ori_arc::ir::ArcVarId::new(i as u32), *new))
            .collect();

    if !changed_to_scalar.is_empty() || !changed_repr.is_empty() {
        tracing::debug!(
            scalars = changed_to_scalar.len(),
            repr_changes = changed_repr.len(),
            "fixup_parent_var_reprs_and_rc_ops: fixing stale RC ops"
        );
    }

    // Walk all blocks and fix RC instructions.
    for block in &mut parent.blocks {
        block.body.retain_mut(|instr| match instr {
            // Remove RcInc/RcDec on vars that became Scalar.
            ori_arc::ir::ArcInstr::RcInc { var, .. } | ori_arc::ir::ArcInstr::RcDec { var, .. }
                if changed_to_scalar.contains(var) =>
            {
                tracing::trace!(var = var.raw(), "removing RC op on var that became Scalar");
                false
            }
            // Update strategy on vars whose repr changed between ref types.
            ori_arc::ir::ArcInstr::RcInc { var, strategy, .. }
                if changed_repr.contains_key(var) =>
            {
                let new_repr = changed_repr[var];
                let var_ty = parent.var_types[var.index()];
                *strategy = ori_arc::ir::RcStrategy::from_repr(new_repr, pool, var_ty);
                true
            }
            ori_arc::ir::ArcInstr::RcDec { var, strategy, .. }
                if changed_repr.contains_key(var) =>
            {
                let new_repr = changed_repr[var];
                let var_ty = parent.var_types[var.index()];
                *strategy = ori_arc::ir::RcStrategy::from_repr(new_repr, pool, var_ty);
                true
            }
            _ => true,
        });
    }

    parent.var_reprs = new_reprs;

    // Debug assertion: no RC ops should target Scalar vars after fixup.
    #[cfg(debug_assertions)]
    for block in &parent.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::RcInc { var, strategy, .. }
            | ori_arc::ir::ArcInstr::RcDec { var, strategy, .. } = instr
            {
                let repr = parent.var_reprs[var.index()];
                debug_assert!(
                    repr != ori_arc::ir::ValueRepr::Scalar,
                    "RC op on Scalar var v{} after fixup (strategy={strategy:?})",
                    var.raw(),
                );
            }
        }
    }
}
