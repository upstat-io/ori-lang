//! Lambda monomorphization: resolve polymorphic lambda types to concrete types.
//!
//! Handles `BoundVar`/`Var` resolution in lambda ARC functions before LLVM emission.
//! Two strategies:
//! - **Single-instantiation**: lambda used at one concrete type → resolve directly
//! - **Multi-instantiation**: lambda used at multiple types → clone per instantiation,
//!   rewrite parent ARC IR to use specialized clones

mod type_predicates;
mod type_resolve;

use ori_ir::Name;
use ori_types::Idx;
use type_predicates::contains_var;
use type_resolve::{
    apply_bound_var_map, build_bound_var_map, fallback_bound_vars_to_int,
    find_all_instantiation_types, find_apply_indirect_result_type, find_partial_apply_args,
    find_partial_apply_concrete_type, find_partial_apply_dst, is_concrete_function,
    is_polymorphic_lambda, resolve_lambda_return_types,
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
    let (global_map, ret_type_resolutions) =
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
    }

    // Final fallback: any remaining BoundVars → Idx::INT.
    for (i, lambda) in lambdas.iter_mut().enumerate() {
        if i < orig_len && multi_inst_lambdas.contains(&i) {
            continue;
        }
        fallback_bound_vars_to_int(lambda, pool);
    }

    remove_multi_inst_originals(lambdas, multi_inst_lambdas);
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
    }

    multi_inst_lambdas
}

/// Phase 2: Build global `BoundVar` -> concrete map for single-inst lambdas.
/// Returns the global map and per-lambda return type resolutions.
fn build_single_inst_mappings(
    parent: &ori_arc::ArcFunction,
    lambdas: &[ori_arc::ArcFunction],
    orig_len: usize,
    multi_inst_lambdas: &rustc_hash::FxHashSet<usize>,
    pool: &ori_types::Pool,
) -> (
    rustc_hash::FxHashMap<u32, Idx>,
    rustc_hash::FxHashMap<usize, (Idx, Idx)>,
) {
    let mut global_map: rustc_hash::FxHashMap<u32, Idx> = rustc_hash::FxHashMap::default();
    let mut ret_type_resolutions: rustc_hash::FxHashMap<usize, (Idx, Idx)> =
        rustc_hash::FxHashMap::default();

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

    (global_map, ret_type_resolutions)
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

/// Generate the mangled name for a specialized lambda clone.
fn specialized_lambda_name(
    interner: &ori_ir::StringInterner,
    base_name: Name,
    inst_idx: usize,
) -> Name {
    let base = interner.lookup(base_name);
    let spec_name_str = format!("{base}${inst_idx}");
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

        // Set return type and matching var_types/instructions from the concrete
        // instantiation. Only exact Idx match to avoid over-replacing.
        let concrete_ret = pool.function_return(*concrete_fn_ty);
        resolve_lambda_return_types(&mut clone, schema_ret, concrete_ret);

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
                    if is_concrete_function(pool, resolved) {
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

/// Find which instantiation matches a resolved function type, returning the
/// specialized lambda name if found.
fn find_matching_instantiation(
    pool: &ori_types::Pool,
    resolved: Idx,
    instantiations: &[Idx],
    interner: &ori_ir::StringInterner,
    lambda_name: Name,
) -> Option<Name> {
    let params = pool.function_params(resolved);
    let ret = pool.function_return(resolved);
    let key: Vec<Idx> = params
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
        if key == inst_key {
            return Some(specialized_lambda_name(interner, lambda_name, idx));
        }
    }

    None
}
