//! Lambda monomorphization: resolve polymorphic lambda types to concrete types.
//!
//! Handles `BoundVar`/`Var` resolution in lambda ARC functions before LLVM emission.
//! Two strategies:
//! - **Single-instantiation**: lambda used at one concrete type → resolve directly
//! - **Multi-instantiation**: lambda used at multiple types → clone per instantiation,
//!   rewrite parent ARC IR to use specialized clones

mod type_predicates;
mod type_resolve;

#[cfg(test)]
mod tests;

use ori_ir::Name;
use ori_types::{Idx, Tag};
use type_predicates::{contains_var, has_concrete_params};
use type_resolve::{
    apply_bound_var_map, apply_call_site_types, apply_concrete_param_types, build_bound_var_map,
    fallback_bound_vars_to_int, find_all_instantiation_types, find_apply_indirect_result_type,
    find_concrete_types_from_calls, find_partial_apply_args, find_partial_apply_concrete_type,
    find_partial_apply_dst, is_concrete_function, is_polymorphic_lambda,
    resolve_lambda_return_types,
};

/// Resolve `BoundVar` types in ALL lambdas to concrete types.
///
/// Handles two cases:
/// 1. **Single-instantiation**: a lambda used at one concrete type → resolve directly
/// 2. **Multi-instantiation**: a lambda used at multiple concrete types (e.g.,
///    `let $id = x -> x; id("hello"); id(42)`) → clone per instantiation and
///    rewrite the parent's ARC IR so each use gets the correct specialization
pub(super) fn resolve_all_lambda_bound_vars(
    parent: &mut ori_arc::ArcFunction,
    lambdas: &mut Vec<ori_arc::ArcFunction>,
    pool: &ori_types::Pool,
    interner: &ori_ir::StringInterner,
    classifier: &dyn ori_arc::ArcClassification,
) {
    // Check if ANY lambda has polymorphic types or multi-instantiation.
    let any_polymorphic = lambdas.iter().any(|l| is_polymorphic_lambda(l, pool));
    let any_multi_inst = !any_polymorphic
        && lambdas
            .iter()
            .any(|l| find_all_instantiation_types(parent, l.name, pool).len() > 1);
    if !any_polymorphic && !any_multi_inst {
        return;
    }

    let orig_len = lambdas.len();
    let multi_inst_lambdas = detect_and_clone_multi_inst(parent, lambdas, pool, interner);
    let (global_map, ret_type_resolutions, concrete_fn_types) =
        build_single_inst_mappings(parent, lambdas, orig_len, &multi_inst_lambdas, pool);

    // Apply the global map + return type resolutions to ALL non-multi-inst lambdas.
    for (i, lambda) in lambdas.iter_mut().enumerate() {
        if i < orig_len && multi_inst_lambdas.contains(&i) {
            continue;
        }
        apply_bound_var_map(lambda, &global_map, pool);

        if let Some(&(schema_ret, concrete_ret)) = ret_type_resolutions.get(&i) {
            resolve_lambda_return_types(lambda, schema_ret, concrete_ret);
        }

        // Direct param substitution for container types with nested vars.
        // apply_bound_var_map only resolves top-level Var/BoundVar; this handles
        // containers like List<Var>, Option<Var> by directly substituting from
        // the concrete function type's param types.
        if let Some(&concrete_fn_ty) = concrete_fn_types.get(&i) {
            apply_concrete_param_types(lambda, concrete_fn_ty, pool);
        }
    }

    // For lambdas where no concrete function type was found via PartialApply
    // narrowing, try extracting concrete types from ApplyIndirect call sites.
    // This handles let-polymorphic lambdas (e.g., `let head = xs -> xs[0]`)
    // where type narrowing happens at the call site, not via Let copies.
    //
    // Safety: only process ORIGINAL lambdas (not multi-inst clones which are
    // appended after orig_len), and only when the lambda's params still contain
    // unresolved Generalized vars after all other resolution attempts.
    #[expect(
        clippy::needless_range_loop,
        reason = "need index for multi_inst_lambdas/concrete_fn_types lookup and mutable lambdas[i] access"
    )]
    for i in 0..orig_len {
        if multi_inst_lambdas.contains(&i) {
            continue;
        }
        if concrete_fn_types.contains_key(&i) {
            continue; // Already resolved via concrete function type.
        }
        let lambda = &lambdas[i];
        // Only try call-site extraction if params still contain nested vars.
        let has_unresolved_container_params = lambda.params.iter().any(|p| {
            !matches!(pool.tag(p.ty), Tag::BoundVar | Tag::Var) && contains_var(pool, p.ty)
        });
        if !has_unresolved_container_params {
            continue;
        }
        // Find the PartialApply dst for this lambda in the parent.
        if let Some(pa_dst) = find_partial_apply_dst(parent, lambda.name) {
            if let Some((arg_types, result_ty)) =
                find_concrete_types_from_calls(parent, pa_dst, pool)
            {
                tracing::debug!(
                    name = ?lambdas[i].name,
                    "lambda mono: resolved from ApplyIndirect call site"
                );
                apply_call_site_types(&mut lambdas[i], &arg_types, result_ty, pool);
            }
        }
    }

    // Final fallback: any remaining BoundVars/Vars → Idx::INT.
    for (i, lambda) in lambdas.iter_mut().enumerate() {
        if i < orig_len && multi_inst_lambdas.contains(&i) {
            continue;
        }
        fallback_bound_vars_to_int(lambda, pool);
    }

    remove_multi_inst_originals(lambdas, multi_inst_lambdas);

    // Recompute parent var_reprs and fix RC ops after type fixups.
    // fixup_call_result_types may have changed var_types (e.g., Var→int),
    // making existing RcInc/RcDec ops invalid (wrong strategy or RC on Scalar).
    fixup_parent_var_reprs_and_rc_ops(parent, classifier, pool);
}

/// Phase 1: Detect multi-instantiation and handle it by cloning lambdas.
/// Must run before the global map build because multi-inst lambdas get
/// specialized clones that are resolved independently.
fn detect_and_clone_multi_inst(
    parent: &mut ori_arc::ArcFunction,
    lambdas: &mut Vec<ori_arc::ArcFunction>,
    pool: &ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> rustc_hash::FxHashSet<usize> {
    let orig_len = lambdas.len();
    let mut multi_inst_lambdas = rustc_hash::FxHashSet::<usize>::default();

    for i in 0..orig_len {
        let has_polymorphic = is_polymorphic_lambda(&lambdas[i], pool);
        let lambda_name = lambdas[i].name;
        let instantiations = find_all_instantiation_types(parent, lambda_name, pool);

        if instantiations.len() <= 1 && !has_polymorphic {
            continue;
        }

        if instantiations.len() > 1 {
            clone_multi_inst_lambda(
                parent,
                lambdas,
                i,
                lambda_name,
                &instantiations,
                interner,
                pool,
            );
            multi_inst_lambdas.insert(i);
        }
        // Lambdas with Scheme return types are now handled: find_all_instantiation_types
        // accepts has_concrete_params, clone_multi_inst_lambda resolves return types
        // from call sites, and rewrite_parent_for_multi_inst matches by params only.
    }

    multi_inst_lambdas
}

/// Result of `build_single_inst_mappings`: global `BoundVar` map, per-lambda
/// return type resolutions, and per-lambda concrete function types.
type SingleInstMappings = (
    rustc_hash::FxHashMap<u32, Idx>,
    rustc_hash::FxHashMap<usize, (Idx, Idx)>,
    rustc_hash::FxHashMap<usize, Idx>,
);

/// Phase 2: Build global `BoundVar` -> concrete map for single-inst lambdas.
/// Returns the global map, per-lambda return type resolutions, and per-lambda
/// concrete function types (for direct param substitution of container types).
fn build_single_inst_mappings(
    parent: &ori_arc::ArcFunction,
    lambdas: &[ori_arc::ArcFunction],
    orig_len: usize,
    multi_inst_lambdas: &rustc_hash::FxHashSet<usize>,
    pool: &ori_types::Pool,
) -> SingleInstMappings {
    let mut global_map: rustc_hash::FxHashMap<u32, Idx> = rustc_hash::FxHashMap::default();
    let mut ret_type_resolutions: rustc_hash::FxHashMap<usize, (Idx, Idx)> =
        rustc_hash::FxHashMap::default();
    let mut concrete_fn_types: rustc_hash::FxHashMap<usize, Idx> = rustc_hash::FxHashMap::default();

    for i in 0..orig_len {
        if multi_inst_lambdas.contains(&i) {
            continue;
        }
        if !is_polymorphic_lambda(&lambdas[i], pool) {
            continue;
        }

        let lambda_name = lambdas[i].name;
        let concrete_fn_ty =
            find_partial_apply_concrete_type(parent, lambdas, i, lambda_name, pool);

        if let Some(concrete_ty) = concrete_fn_ty {
            concrete_fn_types.insert(i, concrete_ty);

            build_bound_var_map(
                pool,
                concrete_ty,
                &lambdas[i].params,
                lambdas[i].return_type,
                &mut global_map,
            );

            // Track return type resolution from ApplyIndirect results (not from
            // the function type, which may still contain unresolved Vars inside
            // containers like Option<Var>, Result<Var>).
            let schema_ret = lambdas[i].return_type;
            if contains_var(pool, schema_ret) {
                if let Some(concrete_ret) =
                    find_apply_indirect_result_type(parent, lambdas[i].name, pool)
                {
                    if concrete_ret != schema_ret {
                        ret_type_resolutions.insert(i, (schema_ret, concrete_ret));
                    }
                }
            }
        }
    }

    (global_map, ret_type_resolutions, concrete_fn_types)
}

/// Remove original multi-instantiated lambdas from the vec. These have been
/// replaced by specialized clones — if left in, `emit_arc_function` compiles
/// them with unresolved type variables. Removes in reverse index order so
/// earlier indices remain valid after each removal.
fn remove_multi_inst_originals(
    lambdas: &mut Vec<ori_arc::ArcFunction>,
    multi_inst_indices: rustc_hash::FxHashSet<usize>,
) {
    if multi_inst_indices.is_empty() {
        return;
    }
    let mut to_remove: Vec<usize> = multi_inst_indices.into_iter().collect();
    to_remove.sort_unstable_by(|a, b| b.cmp(a));
    for idx in to_remove {
        lambdas.remove(idx);
    }
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
fn fixup_parent_var_reprs_and_rc_ops(
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
                *strategy = ori_arc::ir::RcStrategy::from_var(new_repr, pool, var_ty);
                true
            }
            ori_arc::ir::ArcInstr::RcDec { var, strategy } if changed_repr.contains_key(var) => {
                let new_repr = changed_repr[var];
                let var_ty = parent.var_types[var.index()];
                *strategy = ori_arc::ir::RcStrategy::from_var(new_repr, pool, var_ty);
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

/// Generate the mangled name for a specialized lambda clone.
fn specialized_lambda_name(
    interner: &ori_ir::StringInterner,
    base_name: Name,
    inst_idx: usize,
) -> Name {
    let base = interner.lookup(base_name);
    let spec_name_str = format!("{base}__mono{inst_idx}");
    interner.intern(&spec_name_str)
}

/// Clone a multi-instantiated lambda: create one clone per distinct concrete
/// instantiation, resolve each clone's types, and rewrite the parent's ARC IR.
fn clone_multi_inst_lambda(
    parent: &mut ori_arc::ArcFunction,
    lambdas: &mut Vec<ori_arc::ArcFunction>,
    orig_idx: usize,
    lambda_name: Name,
    instantiations: &[Idx],
    interner: &ori_ir::StringInterner,
    pool: &ori_types::Pool,
) {
    let pa_args = find_partial_apply_args(parent, lambda_name);
    let pa_dst = find_partial_apply_dst(parent, lambda_name);
    let schema_ret = lambdas[orig_idx].return_type;

    for (inst_idx, concrete_fn_ty) in instantiations.iter().enumerate() {
        let mut clone = lambdas[orig_idx].clone();
        clone.name = specialized_lambda_name(interner, lambda_name, inst_idx);

        // Build per-instantiation BoundVar map and apply it.
        let mut inst_map = rustc_hash::FxHashMap::<u32, Idx>::default();
        build_bound_var_map(
            pool,
            *concrete_fn_ty,
            &clone.params,
            clone.return_type,
            &mut inst_map,
        );
        apply_bound_var_map(&mut clone, &inst_map, pool);

        // Resolve concrete return type. If the function type has a concrete return,
        // use it directly. For Scheme/Var returns (let-polymorphic lambdas), extract
        // the concrete return type from the ApplyIndirect call site.
        let fn_ret = pool.function_return(*concrete_fn_ty);
        let fn_ret_resolved = pool.resolve_fully(fn_ret);
        let concrete_ret = if matches!(
            pool.tag(fn_ret_resolved),
            Tag::BoundVar | Tag::Var | Tag::Scheme
        ) {
            pa_dst
                .and_then(|dst| find_call_site_return_type(parent, dst, *concrete_fn_ty, pool))
                .unwrap_or(fn_ret_resolved)
        } else {
            fn_ret_resolved
        };
        resolve_lambda_return_types(&mut clone, schema_ret, concrete_ret);

        // Direct param substitution for container types with nested vars.
        apply_concrete_param_types(&mut clone, *concrete_fn_ty, pool);

        fallback_bound_vars_to_int(&mut clone, pool);
        lambdas.push(clone);
    }

    rewrite_parent_for_multi_inst(
        parent,
        lambda_name,
        &pa_args,
        instantiations,
        interner,
        pool,
    );
}

/// Rewrite the parent's ARC IR for a multi-instantiated lambda: replace
/// narrowing Let copies with `PartialApply` of the correct specialization.
fn rewrite_parent_for_multi_inst(
    parent: &mut ori_arc::ArcFunction,
    lambda_name: Name,
    pa_args: &[ori_arc::ir::ArcVarId],
    instantiations: &[Idx],
    interner: &ori_ir::StringInterner,
    pool: &ori_types::Pool,
) {
    let Some(pa_dst) = find_partial_apply_dst(parent, lambda_name) else {
        return;
    };

    // Replace narrowing Let copies with PartialApply of the correct clone.
    for block in &mut parent.blocks {
        for instr in &mut block.body {
            if let ori_arc::ir::ArcInstr::Let {
                dst,
                value: ori_arc::ir::ArcValue::Var(src),
                ty,
            } = instr
            {
                if *src == pa_dst {
                    let var_ty = parent.var_types[dst.index()];
                    let resolved = pool.resolve_fully(var_ty);
                    // Accept both fully-concrete and params-only-concrete copies.
                    // The latter occur for let-polymorphic lambdas with Scheme returns.
                    if is_concrete_function(pool, resolved) || has_concrete_params(pool, resolved) {
                        if let Some(spec_name) = find_matching_instantiation(
                            pool,
                            resolved,
                            instantiations,
                            interner,
                            lambda_name,
                        ) {
                            *instr = ori_arc::ir::ArcInstr::PartialApply {
                                dst: *dst,
                                ty: *ty,
                                func: spec_name,
                                args: pa_args.to_vec(),
                            };
                        }
                    }
                }
            }
        }
    }

    fixup_call_result_types(parent, pa_dst, pool);

    // Remove the original PartialApply instruction — all uses have been
    // rewritten to specialized clones, so the generic closure is dead code.
    // Also remove RcInc/RcDec on the original result variable.
    for block in &mut parent.blocks {
        block.body.retain(|instr| match instr {
            ori_arc::ir::ArcInstr::PartialApply { func, .. } if *func == lambda_name => false,
            ori_arc::ir::ArcInstr::RcInc { var, .. } | ori_arc::ir::ArcInstr::RcDec { var, .. }
                if *var == pa_dst =>
            {
                false
            }
            _ => true,
        });
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

/// Fix up parent `var_types` and instruction `ty` fields for call result variables
/// that still hold unresolved Var types from let-polymorphism.
fn fixup_call_result_types(
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

/// Find which instantiation matches a resolved function type, returning the
/// specialized lambda name if found.
///
/// Tries exact matching (params + return) first, then falls back to params-only
/// matching for let-polymorphic lambdas with Scheme return types.
fn find_matching_instantiation(
    pool: &ori_types::Pool,
    resolved: Idx,
    instantiations: &[Idx],
    interner: &ori_ir::StringInterner,
    lambda_name: Name,
) -> Option<Name> {
    let params = pool.function_params(resolved);
    let ret = pool.function_return(resolved);

    // Try exact match first (params + return).
    let full_key: Vec<Idx> = params
        .iter()
        .chain(std::iter::once(&ret))
        .map(|p| pool.resolve_fully(*p))
        .collect();
    for (idx, inst_ty) in instantiations.iter().enumerate() {
        let inst_params = pool.function_params(*inst_ty);
        let inst_ret = pool.function_return(*inst_ty);
        let inst_key: Vec<Idx> = inst_params
            .iter()
            .chain(std::iter::once(&inst_ret))
            .map(|p| pool.resolve_fully(*p))
            .collect();
        if full_key == inst_key {
            return Some(specialized_lambda_name(interner, lambda_name, idx));
        }
    }

    // Fallback: params-only match for Scheme return types.
    let params_key: Vec<Idx> = params.iter().map(|p| pool.resolve_fully(*p)).collect();
    for (idx, inst_ty) in instantiations.iter().enumerate() {
        let inst_params = pool.function_params(*inst_ty);
        let inst_key: Vec<Idx> = inst_params.iter().map(|p| pool.resolve_fully(*p)).collect();
        if params_key == inst_key {
            return Some(specialized_lambda_name(interner, lambda_name, idx));
        }
    }

    None
}

/// Find the concrete return type for a specific instantiation by following
/// the `PartialApply` → `Let` copy → `ApplyIndirect` → result chain.
///
/// For let-polymorphic lambdas where `Let` copies have concrete params but Scheme
/// return types, the concrete return is determined at the call site: the
/// `ApplyIndirect` result variable has the concrete type from the type checker.
fn find_call_site_return_type(
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
