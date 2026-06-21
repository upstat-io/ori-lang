//! Binary-operator support rules — operator-to-trait mapping, cross-type
//! arithmetic, and trait dispatch.

use ori_ir::{BinaryOp, ExprArena, ExprId, Name, Span};
use rustc_hash::{FxHashMap, FxHashSet};

use super::super::super::InferEngine;
use super::super::registry_bridge::is_binary_op_supported;
use crate::infer::expr::calls::{
    match_self_type, pool_base_name as base_name, type_satisfies_named_trait,
};
use crate::{
    ContextKind, Expected, ExpectedOrigin, Idx, MethodLookup, MethodLookupResult, Pool, Tag,
    WhereConstraint,
};

/// Map a binary operator to its trait method name.
///
/// Delegates to `BinaryOp::trait_method_name()` — the single source of truth in `ori_ir`.
fn binary_op_to_method_name(op: BinaryOp) -> Option<&'static str> {
    op.trait_method_name()
}

/// Map a binary operator to its trait name (for error messages).
///
/// Delegates to `BinaryOp::trait_name()` — the single source of truth in `ori_ir`.
pub(super) fn binary_op_to_trait_name(op: BinaryOp) -> Option<&'static str> {
    op.trait_name()
}

/// Map a comparison operator to the trait name for error messages.
pub(super) fn comparison_trait_name(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Eq | BinaryOp::NotEq => "Eq",
        BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => "Comparable",
        _ => unreachable!("comparison_trait_name called with non-comparison op"),
    }
}

/// Check for cross-type arithmetic special cases.
///
/// Handles mixed-type arithmetic that the per-type registry can't express:
/// - `Duration * int`, `int * Duration`, `Duration / int`, `Duration div int`
/// - `Size * int`, `int * Size`, `Size / int`, `Size div int`
///
/// These are cross-type rules where the result type differs from at least one
/// operand type. The registry validates whether each individual type supports
/// the operator; this function handles the cross-type pairing.
///
/// Returns `Some(result_idx)` if a cross-type rule matched, `None` otherwise.
pub(super) fn check_cross_type_arithmetic(
    left_tag: Tag,
    right_tag: Tag,
    op: BinaryOp,
) -> Option<Idx> {
    // Validate that the operator is supported for the non-int side via the
    // registry. This prevents accepting e.g. Duration @ int (MatMul).
    let (unit_tag, unit_idx) = match (left_tag, right_tag) {
        (Tag::Duration, Tag::Int) | (Tag::Int, Tag::Duration) => (Tag::Duration, Idx::DURATION),
        (Tag::Size, Tag::Int) | (Tag::Int, Tag::Size) => (Tag::Size, Idx::SIZE),
        _ => return None,
    };

    // Only specific operators are valid for cross-type arithmetic.
    // Duration/Size: mul (both directions), div and floor_div (unit / int only).
    let unit_is_left = left_tag == unit_tag;
    match op {
        BinaryOp::Mul => Some(unit_idx),
        BinaryOp::Div | BinaryOp::FloorDiv if unit_is_left => {
            // Duration/Size supports div: check registry.
            is_binary_op_supported(unit_tag, op)
                .filter(|&supported| supported)
                .map(|_| unit_idx)
        }
        _ => None,
    }
}

/// Check if a user nominal type implements a specific trait, identified by the
/// trait's name and its canonical method name.
///
/// Two-tier lookup, matching `infer/expr/calls/impl_lookup.rs` dispatch:
/// 1. Exact-`Idx` `lookup_method_checked` — handles non-generic `Named` types
///    and inherent-shadows-trait correctly.
/// 2. Base-name impl scan — covers generic impls (`impl<T> Pair<T>: Trait`)
///    whose `self_type = Applied(Name, [Named(T)])` does not exact-match a
///    concrete receiver `Applied(Name, [int])`.
///
/// Verifying the trait by name prevents an unrelated `trait Weird { @compare }`
/// (or `@eq`) from bypassing the operator gate.
fn type_implements_named_trait(
    engine: &InferEngine<'_>,
    ty: Idx,
    method_name_str: &str,
    trait_name_str: &str,
) -> bool {
    let Some(method_name) = engine.intern_name(method_name_str) else {
        return false;
    };
    let Some(trait_name) = engine.intern_name(trait_name_str) else {
        return false;
    };
    let pool = engine.pool();

    // Tier 1: exact-Idx checked lookup (non-generic Named + inherent shadowing).
    if let Some(trait_registry) = engine.trait_registry() {
        let is_named_trait = |trait_idx: Idx| -> bool {
            pool.tag(trait_idx) == Tag::Named && pool.named_name(trait_idx) == trait_name
        };
        if let MethodLookupResult::Found(MethodLookup::Trait { trait_idx, .. }) =
            trait_registry.lookup_method_checked(ty, method_name)
        {
            if is_named_trait(trait_idx) {
                return true;
            }
        }
    }

    // Tier 2: base-name impl scan for generic instantiations. The receiver only
    // implements the trait when a matching impl's instantiation bounds are
    // satisfied — a generic `impl<T: Eq> Pair<T>: Eq` does NOT make `Pair<NonEq>`
    // implement `Eq`. The trait-name filter is preserved so an unrelated trait's
    // same-named method cannot bypass the gate.
    let Some(base) = base_name(pool, ty) else {
        return false;
    };
    let candidates = collect_named_trait_impls(engine, trait_name, base, Some(method_name));
    let mut visited: FxHashSet<(Idx, Name)> = FxHashSet::default();
    for cand in &candidates {
        if let Some(subst) = extract_impl_subst(pool, cand.self_type, ty, &cand.type_params) {
            if impl_bounds_satisfied(
                engine,
                &cand.type_params,
                &cand.type_param_bounds,
                &cand.where_clause,
                &subst,
                &mut visited,
            ) {
                return true;
            }
        }
    }
    false
}

/// An impl matched by trait-name + base-name, with the bound data needed to
/// validate its instantiation without holding the registry borrow.
struct ImplBoundCandidate {
    self_type: Idx,
    type_params: Vec<Name>,
    type_param_bounds: Vec<Vec<Name>>,
    where_clause: Vec<WhereConstraint>,
}

/// Collect impls whose trait resolves to `trait_name` and whose `self_type`
/// shares `base`; when `method` is `Some`, also require that method to be
/// defined. Data is cloned out so the registry borrow does not outlive the call.
fn collect_named_trait_impls(
    engine: &InferEngine<'_>,
    trait_name: Name,
    base: Name,
    method: Option<Name>,
) -> Vec<ImplBoundCandidate> {
    let pool = engine.pool();
    let Some(reg) = engine.trait_registry() else {
        return Vec::new();
    };
    let is_named = |ti: Idx| pool.tag(ti) == Tag::Named && pool.named_name(ti) == trait_name;
    reg.impls_iter()
        .filter(|(_, e)| {
            e.trait_idx.is_some_and(is_named)
                && base_name(pool, e.self_type) == Some(base)
                && method.is_none_or(|m| e.methods.contains_key(&m))
        })
        .map(|(_, e)| ImplBoundCandidate {
            self_type: e.self_type,
            type_params: e.type_params.clone(),
            type_param_bounds: e.type_param_bounds.clone(),
            where_clause: e.where_clause.clone(),
        })
        .collect()
}

/// Resolve the impl-binder -> concrete-arg substitution for `target`. Manual
/// impls carry `self_type = Applied(base, [binders])` so the structural matcher
/// binds the params; derive-generated impls register `self_type = Named(base)`
/// (bare), so the params zip positionally against `target`'s applied args.
fn extract_impl_subst(
    pool: &Pool,
    self_type: Idx,
    target: Idx,
    type_params: &[Name],
) -> Option<FxHashMap<Name, Idx>> {
    if let Some(subst) = match_self_type(pool, self_type, target, type_params) {
        return Some(subst);
    }
    if pool.tag(self_type) == Tag::Named && pool.tag(target) == Tag::Applied {
        let targs = pool.applied_args(target);
        if targs.len() == type_params.len() {
            return Some(
                type_params
                    .iter()
                    .copied()
                    .zip(targs.iter().copied())
                    .collect(),
            );
        }
        // Length mismatch means const generics interleave type + const args
        // (`type_params` excludes const params, `applied_args` includes them),
        // so a positional zip would misalign. Treat the impl as applicable with
        // no bound substitution rather than over-reject a valid instantiation —
        // an empty subst leaves every inline bound unchecked, matching the
        // pre-validation base-name behavior for this narrow case.
        return Some(FxHashMap::default());
    }
    if type_params.is_empty() {
        return Some(FxHashMap::default());
    }
    None
}

/// Every inline + trailing-`where` bound on the impl is satisfied by the
/// instantiation `subst`, recursively for nested user generics.
fn impl_bounds_satisfied(
    engine: &InferEngine<'_>,
    type_params: &[Name],
    type_param_bounds: &[Vec<Name>],
    where_clause: &[WhereConstraint],
    subst: &FxHashMap<Name, Idx>,
    visited: &mut FxHashSet<(Idx, Name)>,
) -> bool {
    for (i, param) in type_params.iter().enumerate() {
        let Some(&concrete) = subst.get(param) else {
            continue;
        };
        for &bound_name in type_param_bounds.get(i).into_iter().flatten() {
            if !concrete_satisfies_trait(engine, concrete, bound_name, visited) {
                return false;
            }
        }
    }
    let pool = engine.pool();
    for wc in where_clause {
        // A nested-generic constrained type (`where Box<T>: Eq`) cannot be
        // substituted in this immutable gate (rebuilding `Applied(Box, [concrete])`
        // needs a `&mut Pool`); resolve_under_subst returns `None` for it, and the
        // bound is conservatively skipped rather than over-rejected.
        let Some(concrete) = resolve_under_subst(pool, wc.ty, subst) else {
            continue;
        };
        for &bound_idx in &wc.bounds {
            if pool.tag(bound_idx) != Tag::Named {
                continue;
            }
            if !concrete_satisfies_trait(engine, concrete, pool.named_name(bound_idx), visited) {
                return false;
            }
        }
    }
    true
}

/// Substitute a `where`-clause's constrained type under the impl substitution.
/// A direct binder `Named(T)` resolves to its concrete arg; an already-concrete
/// type passes through. Returns `None` when the constrained type still references
/// a substitution binder it cannot resolve in place (a nested generic like
/// `Box<T>`) — the caller skips that bound rather than over-reject.
fn resolve_under_subst(pool: &Pool, ty: Idx, subst: &FxHashMap<Name, Idx>) -> Option<Idx> {
    if pool.tag(ty) == Tag::Named {
        return Some(subst.get(&pool.named_name(ty)).copied().unwrap_or(ty));
    }
    if references_subst_binder(pool, ty, subst) {
        return None;
    }
    Some(ty)
}

/// Whether `ty` transitively references a `Named` whose name is a substitution
/// binder — i.e. a binder that would need in-place substitution the immutable
/// gate cannot perform.
fn references_subst_binder(pool: &Pool, ty: Idx, subst: &FxHashMap<Name, Idx>) -> bool {
    match pool.tag(ty) {
        Tag::Named => subst.contains_key(&pool.named_name(ty)),
        Tag::Applied => pool
            .applied_args(ty)
            .iter()
            .any(|&arg| references_subst_binder(pool, arg, subst)),
        _ => false,
    }
}

/// Does `concrete` implement the named trait — primitives + registry-bridge
/// first, then user impls with their own bounds recursively validated. The
/// `(Idx, Name)` DFS-path guard terminates on cyclic/self-referential impls.
fn concrete_satisfies_trait(
    engine: &InferEngine<'_>,
    concrete: Idx,
    trait_name: Name,
    visited: &mut FxHashSet<(Idx, Name)>,
) -> bool {
    let pool = engine.pool();
    // Follow unification-var links to the bound type, KEEPING the nominal head
    // intact (NOT `resolve_fully`, which would collapse `Applied(Box, [int])` /
    // `Named(NoEq)` to a struct and lose the base-name impl scan).
    let concrete = pool.chase_var_links(concrete);
    // A still-unbound var or a parametric rigid var carries its bound elsewhere
    // (instantiation-site TR-4 checking); the gate must not over-reject it — only
    // a concrete type that demonstrably lacks the trait is rejected.
    if pool.tag(concrete).is_type_variable() {
        return true;
    }
    let Some(trait_str) = engine.lookup_name(trait_name) else {
        return false;
    };
    let trait_str = trait_str.to_string();
    if type_satisfies_named_trait(concrete, &trait_str, pool) {
        return true;
    }
    if !visited.insert((concrete, trait_name)) {
        return false;
    }
    let result = match base_name(pool, concrete) {
        Some(base) => {
            let candidates = collect_named_trait_impls(engine, trait_name, base, None);
            let mut satisfied = false;
            for cand in &candidates {
                if let Some(subst) =
                    extract_impl_subst(pool, cand.self_type, concrete, &cand.type_params)
                {
                    if impl_bounds_satisfied(
                        engine,
                        &cand.type_params,
                        &cand.type_param_bounds,
                        &cand.where_clause,
                        &subst,
                        visited,
                    ) {
                        satisfied = true;
                        break;
                    }
                }
            }
            satisfied
        }
        None => false,
    };
    visited.remove(&(concrete, trait_name));
    result
}

/// Check if a user-defined type implements the Comparable trait specifically.
/// Recognizes non-generic and generic (`impl<T> T: Comparable`) impls.
pub(crate) fn has_comparable_trait(engine: &InferEngine<'_>, ty: Idx) -> bool {
    type_implements_named_trait(engine, ty, "compare", "Comparable")
}

/// Check if a user-defined type implements the Eq trait specifically.
///
/// Recognizes `#derive(Eq)`, manual `impl T: Eq`, and generic
/// (`impl<T: Eq> Pair<T>: Eq`) impls. Verifying the trait by name prevents an
/// unrelated `eq` method from bypassing the equality-operator gate.
/// Spec: Clause 14 "Equality" — operands shall implement `Eq`.
pub(crate) fn has_eq_trait(engine: &InferEngine<'_>, ty: Idx) -> bool {
    type_implements_named_trait(engine, ty, "eq", "Eq")
}

/// Try to resolve a binary operator via trait dispatch.
///
/// Looks up the operator's method name in the `TraitRegistry` for the left
/// operand's type. If found, checks the right operand against the method's
/// parameter type and returns the method's return type.
pub(super) fn resolve_binary_op_via_trait(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver_ty: Idx,
    right_ty: Idx,
    right: ExprId,
    op: BinaryOp,
    span: Span,
) -> Option<Idx> {
    let method_name = binary_op_to_method_name(op)?;
    let op_str = op.as_symbol();
    let name = engine.intern_name(method_name)?;

    // Scoped borrow: extract signature and self-ness, then release the registry borrow.
    let (sig_ty, has_self) = {
        let trait_registry = engine.trait_registry()?;
        let lookup = trait_registry.lookup_method(receiver_ty, name)?;
        (lookup.method().signature, lookup.method().has_self)
    };

    let resolved_sig = engine.resolve(sig_ty);
    if engine.pool().tag(resolved_sig) != Tag::Function {
        return Some(Idx::ERROR);
    }

    let params = engine.pool().function_params(resolved_sig);
    let ret = engine.pool().function_return(resolved_sig);

    // Skip `self` parameter for instance methods
    let skip = usize::from(has_self);
    let method_params = &params[skip..];

    // Binary operators expect exactly one non-self parameter
    if method_params.len() != 1 {
        return Some(Idx::ERROR);
    }

    // Check right operand against the method's parameter type
    let expected = Expected {
        ty: method_params[0],
        origin: ExpectedOrigin::Context {
            span,
            kind: ContextKind::BinaryOpRight { op: op_str },
        },
    };
    let _ = engine.check_type(right_ty, &expected, arena.get_expr(right).span);

    Some(ret)
}

#[cfg(test)]
#[expect(clippy::expect_used, reason = "Test code uses expect for clarity")]
mod tests;
