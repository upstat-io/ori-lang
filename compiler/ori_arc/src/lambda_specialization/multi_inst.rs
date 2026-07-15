//! Multi-instantiation lambda monomorphization.
//!
//! Handles lambdas that are used at 2+ concrete instantiations (e.g.,
//! `let $id = x -> x; id("hello"); id(42)`). Each distinct instantiation
//! gets a specialized clone; the parent's ARC IR is rewritten to dispatch
//! to the correct clone at each call site.

use ori_ir::Name;
use ori_types::{Idx, Tag};
use rustc_hash::FxHashSet;

use super::call_site::{find_call_site_return_type, fixup_call_result_types};
use super::type_predicates::has_concrete_params;
use super::type_resolve::{
    apply_bound_var_map, apply_concrete_param_types, build_bound_var_map,
    find_all_instantiation_types, find_partial_apply_args, find_partial_apply_dst,
    is_concrete_function, is_polymorphic_lambda, resolve_lambda_return_types,
};

/// Phase 1: Detect multi-instantiation and handle it by cloning lambdas.
/// Must run before the global map build because multi-inst lambdas get
/// specialized clones that are resolved independently.
pub(super) fn detect_and_clone_multi_inst(
    parent: &mut crate::ArcFunction,
    lambdas: &mut Vec<crate::ArcFunction>,
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<usize> {
    let orig_len = lambdas.len();
    let mut multi_inst_lambdas = FxHashSet::<usize>::default();

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

/// Remove original multi-instantiated lambdas from the vec. These have been
/// replaced by specialized clones — if left in, `emit_arc_function` compiles
/// them with unresolved type variables. Removes in reverse index order so
/// earlier indices remain valid after each removal.
pub(super) fn remove_multi_inst_originals(
    lambdas: &mut Vec<crate::ArcFunction>,
    multi_inst_indices: FxHashSet<usize>,
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

/// Generate the mangled name for a specialized lambda clone.
pub(super) fn specialized_lambda_name(
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
    parent: &mut crate::ArcFunction,
    lambdas: &mut Vec<crate::ArcFunction>,
    orig_idx: usize,
    lambda_name: Name,
    instantiations: &[Idx],
    interner: &ori_ir::StringInterner,
    pool: &mut ori_types::Pool,
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
    parent: &mut crate::ArcFunction,
    lambda_name: Name,
    pa_args: &[crate::ir::ArcVarId],
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
            if let crate::ir::ArcInstr::Let {
                dst,
                value: crate::ir::ArcValue::Var(src),
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
                            *instr = crate::ir::ArcInstr::PartialApply {
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
            crate::ir::ArcInstr::PartialApply { func, .. } if *func == lambda_name => false,
            crate::ir::ArcInstr::RcInc { var, .. } | crate::ir::ArcInstr::RcDec { var, .. }
                if *var == pa_dst =>
            {
                false
            }
            _ => true,
        });
    }

    // The original closure register has no value after every construction and
    // use is rewritten to an exact specialization. Keep the stable SSA index
    // table dense, but replace this provably detached slot with the uninhabited
    // type so a generic schema cannot leak into the closed artifact. If any
    // reference survives, leave the schema intact: the shared closure gate
    // will then reject the incomplete rewrite instead of hiding it.
    let is_detached = !parent
        .params
        .iter()
        .any(|parameter| parameter.var == pa_dst)
        && parent.blocks.iter().all(|block| {
            !block.defines_var(pa_dst)
                && !block
                    .body
                    .iter()
                    .any(|instruction| instruction.uses_var(pa_dst))
                && !block.terminator.uses_var(pa_dst)
        });
    if is_detached {
        parent.var_types[pa_dst.index()] = Idx::NEVER;
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
