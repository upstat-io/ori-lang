//! Impl method lookup and signature resolution via `TraitRegistry`.

use rustc_hash::FxHashMap;

use ori_ir::{Name, Span};

use super::super::super::InferEngine;
use crate::pool::substitute::substitute_named_in_pool;
use crate::{
    GenericParamMeta, Idx, MethodLookupResult, Pool, Tag, TypeCheckError, WhereConstraint,
};

/// Result of looking up a method in the `TraitRegistry`.
pub(super) enum LookupOutcome {
    Found {
        sig: Idx,
        has_self: bool,
        /// Method-level where-clause constraints, deep-copied owned form.
        /// Empty when the method has no `where` clause.
        where_clause_metadata: Vec<WhereConstraint>,
        /// Method-level generic parameter metadata (one entry per declared
        /// param, type AND const). Carries the inline `<T: Bound>` info that
        /// `check_method_inline_bounds` enforces at the call site.
        generic_param_metadata: Vec<GenericParamMeta>,
        /// Pool `var_id`s for the method's quantified type variables in
        /// declaration order (parallel to non-const params in
        /// `generic_param_metadata`). Used to map each binder name to the
        /// fresh Var Idx allocated during call-site `instantiate_with_subst`.
        scheme_var_ids: Vec<u32>,
        /// Impl-level binder substitution from receiver-vs-`entry.self_type`
        /// structural match. `Name → Idx` keyed on the impl's `type_params`
        /// (e.g. `{U: int}` for `impl<U> Box<U>` matched against `Box<int>`).
        /// Empty for non-generic impls (exact-Idx primary lookup); populated
        /// only by the base-name fallback path. `resolve_impl_signature`
        /// applies this via `substitute_named_in_pool` BEFORE method-level
        /// `Tag::Scheme` instantiation. BUG-01-002 §05 Phase B residual.
        impl_subst: FxHashMap<Name, Idx>,
    },
    Ambiguous(Vec<ori_ir::Name>),
    NotFound,
}

/// Successfully resolved impl method signature.
pub(super) struct ImplMethodSig {
    /// Method parameters (excluding `self`).
    pub(super) params: Vec<Idx>,
    /// Return type.
    pub(super) ret: Idx,
    /// Method-level where-clause constraints, forwarded from `LookupOutcome::Found`
    /// for call-site bound enforcement (`check_method_where_clauses`).
    pub(super) where_clause_metadata: Vec<WhereConstraint>,
    /// Method-level generic parameter metadata (inline `<T: Bound>` form).
    /// Forwarded for call-site enforcement via `check_method_inline_bounds`.
    pub(super) generic_param_metadata: Vec<GenericParamMeta>,
    /// Pool `var_id`s for the method's quantified type variables. Parallel to
    /// the non-const entries of `generic_param_metadata` in declaration order.
    /// Used by `check_method_inline_bounds` to zip param → `var_id` → subst.
    pub(super) scheme_var_ids: Vec<u32>,
    /// Substitution map produced by `instantiate_with_subst` at call site:
    /// `scheme_var_id → fresh_var_idx`. Empty when the method had no
    /// `Tag::Scheme` wrap (no method-level type generics). Lets the bound
    /// checker resolve each method-level binder to its post-instantiation
    /// concrete type via `engine.resolve(subst[scheme_var_id])`.
    pub(super) instantiation_subst: FxHashMap<u32, Idx>,
}

/// Extract the base type name from a receiver / pattern type.
///
/// `Tag::Applied(Name, [args])` and `Tag::Named(Name)` carry a nominal head
/// name extractable in O(1). Other tags (primitives, function types, vars)
/// have no nominal base — the base-name fallback path is inapplicable.
fn pool_base_name(pool: &Pool, ty: Idx) -> Option<Name> {
    match pool.tag(ty) {
        Tag::Applied => Some(pool.applied_name(ty)),
        Tag::Named => Some(pool.named_name(ty)),
        _ => None,
    }
}

/// Structurally match a registered impl's `entry.self_type` (the pattern,
/// containing `Tag::Named(binder)` references for impl-level type params)
/// against a concrete receiver type (the target).
///
/// Returns `Some(Name → Idx)` mapping each impl-level binder to the
/// corresponding concrete sub-tree of the target when the structures align.
/// Returns `None` when the structures diverge.
///
/// `type_params` lists the impl's binder names — only `Tag::Named(name)`
/// references whose name is in this set are treated as binders; other
/// `Tag::Named` references are nominal type lookups requiring exact-Idx match.
///
/// BUG-01-002 §05 Phase B residual: the engine half of the dispatch fix per
/// `typeck.md §EN-2` (engine owns inference state) and `types.md §RG-2`
/// (registry stays a frozen-after-registration data store).
fn match_self_type(
    pool: &Pool,
    pattern: Idx,
    target: Idx,
    type_params: &[Name],
) -> Option<FxHashMap<Name, Idx>> {
    let mut subst: FxHashMap<Name, Idx> = FxHashMap::default();
    if match_self_type_inner(pool, pattern, target, type_params, &mut subst) {
        Some(subst)
    } else {
        None
    }
}

fn match_self_type_inner(
    pool: &Pool,
    pattern: Idx,
    target: Idx,
    type_params: &[Name],
    subst: &mut FxHashMap<Name, Idx>,
) -> bool {
    if pattern == target {
        return true;
    }
    match (pool.tag(pattern), pool.tag(target)) {
        (Tag::Named, _) => {
            let name = pool.named_name(pattern);
            if !type_params.contains(&name) {
                // Nominal type ref, not a binder — require exact-Idx match.
                return false;
            }
            if let Some(&existing) = subst.get(&name) {
                existing == target
            } else {
                subst.insert(name, target);
                true
            }
        }
        (Tag::Applied, Tag::Applied) => {
            if pool.applied_name(pattern) != pool.applied_name(target) {
                return false;
            }
            let pargs = pool.applied_args(pattern);
            let targs = pool.applied_args(target);
            if pargs.len() != targs.len() {
                return false;
            }
            for (&p, &t) in pargs.iter().zip(targs.iter()) {
                if !match_self_type_inner(pool, p, t, type_params, subst) {
                    return false;
                }
            }
            true
        }
        _ => false,
    }
}

/// Result of the base-name fallback search — single match, ambiguous, or none.
enum FallbackResult {
    Single {
        sig: Idx,
        has_self: bool,
        where_clause_metadata: Vec<WhereConstraint>,
        generic_param_metadata: Vec<GenericParamMeta>,
        scheme_var_ids: Vec<u32>,
        impl_subst: FxHashMap<Name, Idx>,
    },
    Ambiguous(Vec<Name>),
    None,
}

/// Base-name fallback: iterate registered impls, structurally match
/// `entry.self_type` against the receiver, and return the resolved candidate.
///
/// Inherent impls (`trait_idx == None`) win over trait impls per
/// `typeck.md §EX-4` (builtin → inherent → trait). Within each tier, ties
/// across distinct trait impls return `Ambiguous` so the caller can emit
/// `E2023`. BUG-01-002 §05 Phase B residual.
fn lookup_method_by_base_match(
    engine: &InferEngine<'_>,
    receiver_ty: Idx,
    method: Name,
) -> FallbackResult {
    let pool = engine.pool();
    let Some(base_name) = pool_base_name(pool, receiver_ty) else {
        return FallbackResult::None;
    };
    let Some(reg) = engine.trait_registry() else {
        return FallbackResult::None;
    };

    let mut inherent_matches: Vec<(&crate::ImplMethodDef, FxHashMap<Name, Idx>)> = Vec::new();
    let mut trait_matches: Vec<(&crate::ImplMethodDef, FxHashMap<Name, Idx>, Name)> = Vec::new();

    for (_, entry) in reg.impls_iter() {
        let Some(entry_base) = pool_base_name(pool, entry.self_type) else {
            continue;
        };
        if entry_base != base_name {
            continue;
        }
        let Some(method_def) = entry.methods.get(&method) else {
            continue;
        };
        let Some(impl_subst) =
            match_self_type(pool, entry.self_type, receiver_ty, &entry.type_params)
        else {
            continue;
        };
        match entry.trait_idx {
            None => inherent_matches.push((method_def, impl_subst)),
            Some(trait_idx) => {
                let trait_name = reg.get_trait_by_idx(trait_idx).map_or(method, |t| t.name);
                trait_matches.push((method_def, impl_subst, trait_name));
            }
        }
    }

    // Inherent wins; ambiguity within inherent is a registration error caught
    // earlier (coherence check `TR-5`), so first hit suffices.
    if let Some((method_def, impl_subst)) = inherent_matches.into_iter().next() {
        return FallbackResult::Single {
            sig: method_def.signature,
            has_self: method_def.has_self,
            where_clause_metadata: method_def.where_clause_metadata.clone(),
            generic_param_metadata: method_def.generic_param_metadata.clone(),
            scheme_var_ids: method_def.scheme_var_ids.clone(),
            impl_subst,
        };
    }

    if trait_matches.len() > 1 {
        let trait_names: Vec<Name> = trait_matches.iter().map(|(_, _, n)| *n).collect();
        return FallbackResult::Ambiguous(trait_names);
    }
    if let Some((method_def, impl_subst, _)) = trait_matches.into_iter().next() {
        FallbackResult::Single {
            sig: method_def.signature,
            has_self: method_def.has_self,
            where_clause_metadata: method_def.where_clause_metadata.clone(),
            generic_param_metadata: method_def.generic_param_metadata.clone(),
            scheme_var_ids: method_def.scheme_var_ids.clone(),
            impl_subst,
        }
    } else {
        FallbackResult::None
    }
}

/// Perform the borrow-dance lookup for impl methods via `TraitRegistry`.
///
/// Scopes the immutable `trait_registry` borrow to extract data, so the
/// caller can use `engine` mutably afterwards.
///
/// Two-phase lookup per BUG-01-002 §05 Phase B residual:
/// 1. Exact-`Idx` primary lookup via `lookup_method_checked` — fast path,
///    matches concrete impls (`impl Box<int>`) and impls registered against
///    the receiver's exact pool index.
/// 2. Base-name fallback via `lookup_method_by_base_match` — fires only on
///    primary-lookup miss. Iterates registered impls, structurally matches
///    each `entry.self_type` against the receiver, and returns the resolved
///    candidate with its impl-level substitution map. This is what makes
///    `b: Box<int>` dispatch to `impl<U> Box<U> { @m<T> ... }` work despite
///    `Applied(Box, [Named(U)]) ≠ Applied(Box, [Int])` per `types.md §TI-2`.
pub(super) fn lookup_impl_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: Name,
) -> LookupOutcome {
    let primary = {
        let Some(reg) = engine.trait_registry() else {
            return LookupOutcome::NotFound;
        };
        reg.lookup_method_checked(receiver_ty, method)
    };
    match primary {
        MethodLookupResult::Found(lookup) => {
            let m = lookup.method();
            return LookupOutcome::Found {
                sig: m.signature,
                has_self: m.has_self,
                where_clause_metadata: m.where_clause_metadata.clone(),
                generic_param_metadata: m.generic_param_metadata.clone(),
                scheme_var_ids: m.scheme_var_ids.clone(),
                impl_subst: FxHashMap::default(),
            };
        }
        MethodLookupResult::Ambiguous { candidates } => {
            return LookupOutcome::Ambiguous(candidates.iter().map(|&(_, n)| n).collect());
        }
        MethodLookupResult::NotFound => {
            // Fall through to base-name fallback below.
        }
    }

    match lookup_method_by_base_match(engine, receiver_ty, method) {
        FallbackResult::Single {
            sig,
            has_self,
            where_clause_metadata,
            generic_param_metadata,
            scheme_var_ids,
            impl_subst,
        } => LookupOutcome::Found {
            sig,
            has_self,
            where_clause_metadata,
            generic_param_metadata,
            scheme_var_ids,
            impl_subst,
        },
        FallbackResult::Ambiguous(trait_names) => LookupOutcome::Ambiguous(trait_names),
        FallbackResult::None => LookupOutcome::NotFound,
    }
}

/// After an impl method lookup, resolve the signature and validate arity.
///
/// Returns `Some(Ok(sig))` on success with params (excluding `self`) and
/// return type. Returns `Some(Err(()))` for errors (ambiguous, bad
/// signature, arity mismatch -- diagnostic already pushed). Returns `None`
/// if the method was not found.
pub(super) fn resolve_impl_signature(
    engine: &mut InferEngine<'_>,
    outcome: LookupOutcome,
    method: Name,
    arg_count: usize,
    span: Span,
) -> Option<Result<ImplMethodSig, ()>> {
    let (
        sig_ty,
        has_self,
        where_clause_metadata,
        generic_param_metadata,
        scheme_var_ids,
        impl_subst,
    ) = match outcome {
        LookupOutcome::Found {
            sig,
            has_self,
            where_clause_metadata,
            generic_param_metadata,
            scheme_var_ids,
            impl_subst,
        } => (
            sig,
            has_self,
            where_clause_metadata,
            generic_param_metadata,
            scheme_var_ids,
            impl_subst,
        ),
        LookupOutcome::Ambiguous(trait_names) => {
            engine.push_error(TypeCheckError::ambiguous_method(span, method, trait_names));
            return Some(Err(()));
        }
        LookupOutcome::NotFound => return None,
    };

    // Phase B residual (BUG-01-002): apply impl-level binder substitution
    // BEFORE method-level Scheme instantiation. The composition order is
    // load-bearing — receiver-bind → impl-Name-substitute → method-level
    // Scheme instantiate — so a registered signature on `impl<U> Box<U>
    // { @m<T> ... }` with `Tag::Named(U)` impl-level refs and a
    // `Tag::Scheme([T_var_id], ...)` method-level wrap fully resolves to
    // a concrete function type after both layers run.
    let resolved_sig = engine.resolve(sig_ty);
    let impl_substituted_sig = if impl_subst.is_empty() {
        resolved_sig
    } else {
        substitute_named_in_pool(engine.pool_mut(), resolved_sig, &impl_subst)
    };

    // Phase B Step 5b (BUG-01-002): if the registered signature is a
    // `Tag::Scheme` (set by `build_impl_method` when the method has
    // method-level type generics), instantiate it now so each call site gets
    // fresh unification vars in place of the scheme's bound vars. This is the
    // `GN-2` (`typeck.md §GN-2`) instantiation pattern, mirrored from the
    // top-level identifier path at `infer/expr/identifiers.rs:16-17`.
    // Method-level binders that previously failed to unify against function-
    // type arguments (`UN-6` rigid mismatch) now unify cleanly because they
    // have been replaced by fresh, narrowable `Tag::Var`s.
    //
    // Phase B residual (BUG-01-002): use `instantiate_with_subst` to capture
    // the `scheme_var_id → fresh_var_idx` map; downstream
    // `check_method_inline_bounds` consumes the map to look up each
    // method-level binder's post-instantiation Var Idx and enforce its
    // inline `<T: Bound>` constraints.
    let (instantiated_sig, instantiation_subst) =
        if engine.pool().tag(impl_substituted_sig) == Tag::Scheme {
            engine.instantiate_with_subst(impl_substituted_sig)
        } else {
            (impl_substituted_sig, FxHashMap::default())
        };
    if engine.pool().tag(instantiated_sig) != Tag::Function {
        return Some(Err(()));
    }

    let params = engine.pool().function_params(instantiated_sig);
    let ret = engine.pool().function_return(instantiated_sig);

    // For instance methods (has_self), skip the first `self` param
    let skip = usize::from(has_self);
    let method_params = params[skip..].to_vec();

    if arg_count != method_params.len() {
        engine.push_error(TypeCheckError::arity_mismatch(
            span,
            method_params.len(),
            arg_count,
            crate::ArityMismatchKind::Function,
        ));
        return Some(Err(()));
    }

    Some(Ok(ImplMethodSig {
        params: method_params,
        ret,
        where_clause_metadata,
        generic_param_metadata,
        scheme_var_ids,
        instantiation_subst,
    }))
}

/// Emit E2036 when `.into()` is called on a type with no Into implementation.
///
/// Only fires when the method name matches the well-known `into` name.
/// Non-into methods fall through silently (handled by other error paths).
pub(super) fn emit_into_not_implemented(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: Name,
    span: Span,
) {
    let is_into = engine
        .well_known()
        .is_some_and(|wk| method == wk.into_method);
    if is_into {
        engine.push_error(TypeCheckError::into_not_implemented(
            span,
            receiver_ty,
            None,
        ));
    }
}
