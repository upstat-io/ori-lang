//! Call-site type fixups during lambda specialization.
//!
//! After multi-instantiation cloning (`multi_inst`) or single-instantiation
//! mapping (`single_inst`) runs, the parent's call-result types may need the
//! concrete types selected at indirect call sites. These helpers:
//!
//! - `fixup_call_result_types` — rewrite `ApplyIndirect` / `InvokeIndirect`
//!   result types when the closure target got specialized.
//! - `find_call_site_return_type` / `resolve_call_result_type` — look up
//!   concrete return types via the `PartialApply → Let → ApplyIndirect`
//!   chain.

use ori_types::{Idx, Tag};

use super::type_predicates::contains_var;

/// Fix up parent `var_types` and instruction `ty` fields for call result variables
/// that still hold unresolved Var types from let-polymorphism.
pub(super) fn fixup_call_result_types(
    parent: &mut crate::ArcFunction,
    pa_dst: crate::ir::ArcVarId,
    pool: &ori_types::Pool,
) {
    for block_idx in 0..parent.blocks.len() {
        for instr_idx in 0..parent.blocks[block_idx].body.len() {
            if let crate::ir::ArcInstr::ApplyIndirect {
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
                        if let crate::ir::ArcInstr::ApplyIndirect { ty, .. } =
                            &mut parent.blocks[block_idx].body[instr_idx]
                        {
                            *ty = concrete;
                        }
                    }
                }
            }
        }
        // Check `InvokeIndirect` terminator.
        let needs_fixup = if let crate::ir::ArcTerminator::InvokeIndirect {
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
            if let crate::ir::ArcTerminator::InvokeIndirect { ty, .. } =
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
    parent: &crate::ArcFunction,
    closure: crate::ir::ArcVarId,
    pa_dst: crate::ir::ArcVarId,
) -> bool {
    // After rewrite, the closure var is a PartialApply dst (was a Let dst copying pa_dst).
    // Check if any PartialApply instruction has this var as dst.
    for block in &parent.blocks {
        for instr in &block.body {
            if let crate::ir::ArcInstr::PartialApply { dst, .. } = instr {
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
    parent: &crate::ArcFunction,
    pa_dst: crate::ir::ArcVarId,
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
            if let crate::ir::ArcInstr::Let {
                dst,
                value: crate::ir::ArcValue::Var(src),
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
                        if let crate::ir::ArcInstr::ApplyIndirect {
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
                    if let crate::ir::ArcTerminator::InvokeIndirect {
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
    func: &crate::ArcFunction,
    result_var: crate::ir::ArcVarId,
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
            if let crate::ir::ArcInstr::Let {
                value: crate::ir::ArcValue::Var(src),
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
