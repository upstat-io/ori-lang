//! `resolve_impl_signature` — resolve a looked-up impl/trait method to a
//! concrete `ImplMethodSig` (impl-binder substitution → method-Scheme
//! instantiation → arity check). Extracted from `impl_lookup.rs` to keep that
//! module under the 500-line cap (§10.R-010); signature resolution is a
//! distinct responsibility from method lookup.

use rustc_hash::FxHashMap;

use ori_ir::{Name, Span};

use super::super::super::InferEngine;
use super::impl_lookup::{ImplMethodSig, LookupOutcome, MethodMonoData};
use crate::pool::substitute::substitute_named_in_pool;
use crate::{Idx, Tag, TypeCheckError};

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
        impl_type_params,
        optional_param_count,
    ) = match outcome {
        LookupOutcome::Found {
            sig,
            has_self,
            where_clause_metadata,
            generic_param_metadata,
            scheme_var_ids,
            impl_subst,
            impl_type_params,
            optional_param_count,
        } => (
            sig,
            has_self,
            where_clause_metadata,
            generic_param_metadata,
            scheme_var_ids,
            impl_subst,
            impl_type_params,
            optional_param_count,
        ),
        LookupOutcome::Ambiguous(trait_names) => {
            engine.push_error(TypeCheckError::ambiguous_method(span, method, trait_names));
            return Some(Err(()));
        }
        LookupOutcome::NotFound => return None,
    };

    // Build the receiver-side monomorphization carrier BEFORE consuming
    // `impl_subst` for signature substitution. Non-empty `impl_type_params`
    // marks an inherent method on a generic impl (`impl<T> Box<T>`); pair each
    // binder (declaration order) with its concrete substitution so the emission
    // hook can mint `impl_args = [int]` for `Box<int>`. Any binder missing from
    // `impl_subst` (structurally impossible for a successful match) drops the
    // carrier to `None` rather than emitting a partial instance.
    let method_mono = if impl_type_params.is_empty() {
        None
    } else {
        let impl_type_args: Vec<(Name, Idx)> = impl_type_params
            .iter()
            .filter_map(|&name| impl_subst.get(&name).map(|&concrete| (name, concrete)))
            .collect();
        if impl_type_args.len() == impl_type_params.len() {
            Some(MethodMonoData { impl_type_args })
        } else {
            None
        }
    };

    tracing::debug!(
        target: "ori_types::mono",
        method = ?method,
        impl_type_params_len = impl_type_params.len(),
        method_mono_recorded = method_mono.is_some(),
        "resolve_impl_signature method-mono carrier decision"
    );

    // Apply impl-level binder substitution (Named + RigidVar) BEFORE
    // method-level Scheme instantiation; composition order is load-bearing.
    let impl_substituted_sig =
        apply_impl_binder_substitution(engine, sig_ty, &impl_subst, &impl_type_params);

    // If the registered signature is a `Tag::Scheme` (set by
    // `build_impl_method` when the method has method-level type generics),
    // instantiate it now so each call site gets fresh unification vars in
    // place of the scheme's bound vars. This is the `GN-2` instantiation
    // pattern, mirrored from the top-level identifier path at
    // `infer/expr/identifiers.rs:16-17`. Method-level binders that would
    // otherwise fail to unify against function-type arguments (`UN-6` rigid
    // mismatch) unify cleanly because they have been replaced by fresh,
    // narrowable `Tag::Var`s.
    //
    // `instantiate_with_subst` captures the `scheme_var_id → fresh_var_idx`
    // map; downstream `check_method_inline_bounds` consumes the map to look
    // up each method-level binder's post-instantiation Var Idx and enforce
    // its inline `<T: Bound>` constraints.
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

    // BUG-04-190: a call may omit up to `optional_param_count` trailing
    // defaulted params; canon fills them. Valid arity is the inclusive range
    // [method_params.len() - optional_param_count, method_params.len()].
    // `optional_param_count == 0` reduces to strict equality (unchanged
    // behavior for no-default methods). Too-many args, or too-few past the
    // required floor, still emit E2004.
    let min_args = method_params.len().saturating_sub(optional_param_count);
    if arg_count < min_args || arg_count > method_params.len() {
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
        method_mono,
        where_clause_metadata,
        generic_param_metadata,
        scheme_var_ids,
        instantiation_subst,
    }))
}

/// Apply impl-level binder substitution to a registered method signature
/// before method-level Scheme instantiation.
///
/// Composition order is load-bearing: resolve → substitute `Tag::Named`
/// impl refs → substitute `Tag::RigidVar` impl binders to concrete receiver
/// types. `substitute_named_in_pool` rewrites only `Tag::Named`; §10.1.2 made
/// impl binders `Tag::RigidVar` (dispatch parametricity), so a `@get (self)
/// -> T` return needs the second pass to resolve `T` to the concrete receiver
/// type — without it the `RigidVar(T)` survives to mono recording and fails
/// its `is_fully_concrete` gate. SSOT scan: `build_impl_rigid_var_subst`.
fn apply_impl_binder_substitution(
    engine: &mut InferEngine<'_>,
    sig_ty: Idx,
    impl_subst: &FxHashMap<Name, Idx>,
    impl_type_params: &[Name],
) -> Idx {
    let resolved_sig = engine.resolve(sig_ty);
    let impl_substituted_sig = if impl_subst.is_empty() {
        resolved_sig
    } else {
        substitute_named_in_pool(engine.pool_mut(), resolved_sig, impl_subst)
    };

    if impl_type_params.is_empty() {
        return impl_substituted_sig;
    }
    let name_to_concrete: FxHashMap<Name, Idx> = impl_type_params
        .iter()
        .filter_map(|&n| impl_subst.get(&n).map(|&c| (n, c)))
        .collect();
    let rigid_subst =
        crate::pool::substitute::build_impl_rigid_var_subst(engine.pool(), &name_to_concrete);
    if rigid_subst.is_empty() {
        impl_substituted_sig
    } else {
        crate::pool::substitute::substitute_in_pool(
            engine.pool_mut(),
            impl_substituted_sig,
            &rigid_subst,
        )
    }
}
