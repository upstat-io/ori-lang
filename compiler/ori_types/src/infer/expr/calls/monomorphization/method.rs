//! Monomorphization instance recording for generic method calls.

use ori_ir::{ExprArena, ExprId, Name, ParsedType, Span};
use rustc_hash::FxHashMap;

use crate::const_eval::{EvaluatedConstExpr, GenericConstExpr};
use crate::pool::substitute::substitute_in_pool;
use crate::{
    ConcreteMethodMono, ConstGenericTerm, ConstValue, Expected, GenericArg, Idx,
    InvalidFixedListCapacityReason, MonoConstBinding, MonoInstance, Pool, Tag, TypeCheckError,
};

use crate::infer::InferEngine;

use super::super::impl_lookup::{ImplMethodSig, MethodMonoData};
use super::applied::resolve_applied_type;
use super::{build_and_register_body_type_map, collect_generic_type_params};

/// Concrete method arguments, name-keyed type substitutions, and const body bindings.
type MethodBinderArgs = (
    Vec<GenericArg>,
    Vec<(Name, Idx)>,
    FxHashMap<u32, Idx>,
    Vec<MonoConstBinding>,
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MethodConstBindingSource {
    Explicit,
    ExpectedAnnotation,
    Default,
}

#[derive(Clone, Debug)]
struct ResolvedMethodConstBinding {
    binding: MonoConstBinding,
    source: MethodConstBindingSource,
}

/// Materialize an impl argument for recordability, then retain its nominal
/// identity for dispatch.
///
/// An `Applied` carrier is interned before inference variables link, so its
/// cached flags can retain `HAS_VAR` after every argument is concrete. Empty-map
/// substitution follows those links and re-interns the concrete shell for the
/// publish gate. Method identity is then canonicalized separately so equivalent
/// pre-link carriers still name one target.
fn concrete_impl_arg(pool: &mut Pool, ty: Idx) -> Option<Idx> {
    let concrete = substitute_in_pool(pool, ty, &FxHashMap::default());
    if !pool.flags(concrete).is_recordable() {
        return None;
    }

    let semantic = pool.method_receiver_key(concrete);
    Some(if pool.tag(semantic) == Tag::Applied {
        semantic
    } else {
        pool.resolve_fully(concrete)
    })
}

/// Resolve the method's own `<T>`-style binders to concrete args.
///
/// In the SIGNATURE the binders are fresh unification vars (in
/// `instantiation_subst`, link-resolved); in the BODY they are
/// `VarState::Rigid { name }` rigid vars whose `var_id` is NOT the scheme `var_id`.
/// The signature substitution keys on `scheme_var_ids` (var-id); the body
/// substitution keys on the binder NAME via `build_impl_rigid_var_subst` — so
/// the binder name (from `generic_param_metadata`, parallel to `scheme_var_ids`
/// in declaration order, non-const entries only) is captured alongside each
/// resolved arg for `extra_named` threading (the impl-binder name-scan path).
///
/// Returns `(method_args, method_named, var_subst)`, or `None` when a binder arg
/// is not fully concrete (caller skips the recording this pass).
fn resolve_method_binder_args(
    engine: &mut InferEngine<'_>,
    sig: &ImplMethodSig,
    resolved_const_bindings: &[MonoConstBinding],
) -> Option<MethodBinderArgs> {
    let const_param_count = sig
        .generic_param_metadata
        .iter()
        .filter(|meta| meta.is_const)
        .count();
    let mut method_args = Vec::with_capacity(sig.generic_param_metadata.len());
    let mut method_named: Vec<(Name, Idx)> = Vec::with_capacity(sig.scheme_var_ids.len());
    let mut var_subst: FxHashMap<u32, Idx> = FxHashMap::default();
    let mut const_bindings = Vec::with_capacity(const_param_count);
    let mut scheme_vars = sig.scheme_var_ids.iter().copied();
    for meta in &sig.generic_param_metadata {
        if meta.is_const {
            let value = resolved_const_bindings
                .iter()
                .find(|binding| binding.name == meta.name)
                .map(|binding| &binding.value)?;
            method_args.push(GenericArg::Const(value.clone()));
            const_bindings.push(MonoConstBinding {
                name: meta.name,
                value: value.clone(),
            });
            continue;
        }

        let sv_id = scheme_vars.next()?;
        let Some(&fresh) = sig.instantiation_subst.get(&sv_id) else {
            continue;
        };
        let resolved = engine.pool().resolve_fully(fresh);
        if !is_recordable(engine, resolved) {
            return None;
        }
        var_subst.insert(sv_id, resolved);
        method_args.push(GenericArg::Type(resolved));
        method_named.push((meta.name, resolved));
    }
    Some((method_args, method_named, var_subst, const_bindings))
}

/// Resolve explicit, inferred, and default method-generic arguments, then
/// validate the fixed-list capacity expressions reached by concrete const
/// bindings.
///
/// Precedence is explicit call-site argument, surrounding result constraint,
/// then declaration default. Only capacity expressions retained on the method
/// definition are positivity-checked, so an unrelated integer const generic may
/// legitimately bind zero.
pub(in crate::infer::expr::calls) fn resolve_method_call_generic_args(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    call_expr_id: ExprId,
    sig: &ImplMethodSig,
    expected: Option<&Expected>,
    span: Span,
) -> Vec<MonoConstBinding> {
    let explicit_range = arena.method_call_type_args(call_expr_id);
    let explicit_ids = arena.get_parsed_type_list(explicit_range);
    if explicit_ids.len() > sig.generic_param_metadata.len() {
        engine.push_error(TypeCheckError::arity_mismatch(
            span,
            sig.generic_param_metadata.len(),
            explicit_ids.len(),
            crate::ArityMismatchKind::TypeArgs,
        ));
    }
    let mut expected_terms = expected.into_iter().flat_map(Expected::const_terms);
    let mut scheme_vars = sig.scheme_var_ids.iter().copied();
    let mut const_bindings = Vec::new();

    for (index, meta) in sig.generic_param_metadata.iter().enumerate() {
        let explicit = explicit_ids.get(index).map(|&id| arena.get_parsed_type(id));
        if !meta.is_const {
            let Some(scheme_var_id) = scheme_vars.next() else {
                continue;
            };
            let Some(&fresh) = sig.instantiation_subst.get(&scheme_var_id) else {
                continue;
            };
            let explicit_ty = explicit.and_then(|parsed| match parsed {
                ParsedType::ConstExpr(_) => None,
                parsed => Some(crate::resolve_parsed_type(engine, arena, parsed)),
            });
            if let Some(type_arg) = explicit_ty.or(meta.default_type) {
                let _ = engine.unify_types(fresh, type_arg);
            }
            continue;
        }

        let explicit_value = explicit.and_then(|parsed| {
            let (value, found_ty) = match parsed {
                ParsedType::ConstExpr(expr) => {
                    let value = evaluate_call_const_expr(engine, arena, *expr, &const_bindings);
                    let found_ty = match value.as_ref() {
                        Some(ConstValue::Int(_)) => Some(Idx::INT),
                        Some(ConstValue::Bool(_)) => Some(Idx::BOOL),
                        None => None,
                    };
                    (value, found_ty)
                }
                parsed => (
                    None,
                    Some(crate::resolve_parsed_type(engine, arena, parsed)),
                ),
            };
            let type_matches =
                if let (Some(expected_ty), Some(found_ty)) = (meta.const_type, found_ty) {
                    let expected = Expected::no_expectation(expected_ty);
                    engine.check_type(found_ty, &expected, span).is_ok()
                } else {
                    true
                };
            type_matches.then_some(value).flatten()
        });
        // The expectation lane is binder-ordered. Consume its slot even when
        // an explicit argument wins so a later const binder cannot inherit the
        // wrong earlier term.
        let expected_term = expected_terms.next();
        let inferred_value = if explicit.is_none() {
            expected_term.and_then(|term| match term {
                ConstGenericTerm::Value(value) => Some(value.clone()),
                ConstGenericTerm::CallerParam(_) => None,
            })
        } else {
            None
        };
        let default_value = if explicit.is_none() && inferred_value.is_none() {
            meta.const_default_value
                .as_ref()
                .and_then(|default| evaluate_owned_call_const(engine, default, &const_bindings))
        } else {
            None
        };
        let resolved = explicit_value
            .map(|value| (value, MethodConstBindingSource::Explicit))
            .or_else(|| {
                inferred_value.map(|value| (value, MethodConstBindingSource::ExpectedAnnotation))
            })
            .or_else(|| default_value.map(|value| (value, MethodConstBindingSource::Default)));
        if let Some((value, source)) = resolved {
            const_bindings.push(ResolvedMethodConstBinding {
                binding: MonoConstBinding {
                    name: meta.name,
                    value,
                },
                source,
            });
        }
    }

    validate_call_capacity_constraints(
        engine,
        &sig.fixed_list_capacity_constraints,
        &const_bindings,
        span,
    );
    const_bindings
        .into_iter()
        .map(|resolved| resolved.binding)
        .collect()
}

fn evaluate_call_const_expr(
    engine: &InferEngine<'_>,
    arena: &ExprArena,
    expr: ExprId,
    bindings: &[ResolvedMethodConstBinding],
) -> Option<ConstValue> {
    let expr = GenericConstExpr::from_arena(arena, expr).ok()?;
    evaluate_owned_call_const(engine, &expr, bindings)
}

fn evaluate_owned_call_const(
    engine: &InferEngine<'_>,
    expr: &GenericConstExpr,
    bindings: &[ResolvedMethodConstBinding],
) -> Option<ConstValue> {
    let mut resolve_name = |name| {
        bindings
            .iter()
            .find(|resolved| resolved.binding.name == name)
            .map(|resolved| resolved.binding.value.clone())
            .or_else(|| engine.const_value(name))
    };
    match expr.evaluate(&mut resolve_name).ok()? {
        EvaluatedConstExpr::Concrete(value) => Some(value),
        EvaluatedConstExpr::Symbolic => None,
    }
}

fn validate_call_capacity_constraints(
    engine: &mut InferEngine<'_>,
    constraints: &[GenericConstExpr],
    bindings: &[ResolvedMethodConstBinding],
    span: Span,
) {
    for constraint in constraints {
        let mut resolve_name = |name| {
            bindings
                .iter()
                .find(|resolved| resolved.binding.name == name)
                .map(|resolved| resolved.binding.value.clone())
                .or_else(|| engine.const_value(name))
        };
        match constraint.evaluate(&mut resolve_name) {
            Ok(EvaluatedConstExpr::Concrete(ConstValue::Int(value))) if value <= 0 => {
                // A direct `$N` result capacity inferred from a fixed-list
                // annotation was already validated at that annotation's exact
                // capacity span. Do not duplicate E2057 at the enclosing call.
                // Composite constraints (for example `$N - 1`) still need a
                // call-site error because the annotation validated only `$N`.
                if !direct_capacity_was_validated_by_expected_annotation(constraint, bindings) {
                    engine.push_error(TypeCheckError::non_positive_fixed_list_capacity(
                        span, value,
                    ));
                }
            }
            Ok(EvaluatedConstExpr::Concrete(ConstValue::Bool(_))) => {
                engine.push_error(TypeCheckError::invalid_fixed_list_capacity_expression(
                    span,
                    InvalidFixedListCapacityReason::NonInteger,
                ));
            }
            Err(reason) => engine.push_error(
                TypeCheckError::invalid_fixed_list_capacity_expression(span, reason),
            ),
            Ok(EvaluatedConstExpr::Concrete(ConstValue::Int(_)) | EvaluatedConstExpr::Symbolic) => {
            }
        }
    }
}

fn direct_capacity_was_validated_by_expected_annotation(
    constraint: &GenericConstExpr,
    bindings: &[ResolvedMethodConstBinding],
) -> bool {
    let GenericConstExpr::Name(name) = constraint else {
        return false;
    };
    bindings.iter().any(|resolved| {
        resolved.binding.name == *name
            && resolved.source == MethodConstBindingSource::ExpectedAnnotation
    })
}

/// Record a `MonoInstance` for a generic method call — either an IMPL-level
/// generic (`b.unwrap()` where `b: Box<int>` and the impl is
/// `impl<T> Box<T> { @unwrap (self) -> T }`) OR a METHOD-level generic
/// (`b.pick(item: 5)` where the impl is `impl Boxer { @pick<T> (self, item: T) -> T }`).
///
/// Fires when EITHER binder axis is present: [`ImplMethodSig::method_mono`] is
/// `Some` (impl generic over the receiver's type params) OR
/// [`ImplMethodSig::scheme_var_ids`] is non-empty (the method's own `<U>` binders,
/// present even on a concrete-receiver impl). A non-generic method on a
/// non-generic impl leaves both empty and is conservatively skipped. Emission is
/// additionally gated on the receiver (and every substituted param / return type)
/// being fully concrete; a receiver that still carries type variables — a generic
/// method resolving through another generic — is conservatively skipped this pass.
///
/// MUST be called AFTER argument type-checking has unified the method's
/// instantiation vars, so `engine.resolve` yields concrete types. `Some(id)`
/// publishes canonical call dispatch; `None` records only the concrete body
/// demand for an operator expression.
pub(in crate::infer::expr::calls) fn maybe_record_method_mono_instance(
    engine: &mut InferEngine<'_>,
    call_expr_id: Option<ExprId>,
    method_name: Name,
    receiver_ty: Idx,
    sig: &ImplMethodSig,
    resolved_const_bindings: &[MonoConstBinding],
) {
    // INVARIANT: either impl-level or method-level binders require monomorphization.
    let mono = sig.method_mono.as_ref();
    if mono.is_none() && sig.generic_param_metadata.is_empty() {
        return;
    }

    // INVARIANT: receiver substitution retains the `Applied` shell for impl matching.
    let receiver = substitute_in_pool(engine.pool_mut(), receiver_ty, &FxHashMap::default());
    let ret_resolved = engine.resolve(sig.ret);
    tracing::debug!(
        method = ?method_name,
        receiver_concrete = is_fully_concrete(engine, receiver),
        receiver_tag = ?engine.pool().tag(receiver),
        receiver_flags = ?engine.pool().flags(receiver),
        impl_args = ?mono.map(|m| &m.impl_type_args),
        ret_tag = ?engine.pool().tag(ret_resolved),
        ret_concrete = is_fully_concrete(engine, ret_resolved),
        "maybe_record_method entry gate"
    );
    if !is_recordable(engine, receiver) {
        return;
    }

    // INVARIANT: method-only generics record empty impl args and populated method args.
    let mut impl_args = Vec::with_capacity(mono.map_or(0, |m| m.impl_type_args.len()));
    if let Some(mono) = mono {
        for &(_, concrete) in &mono.impl_type_args {
            let Some(resolved) = concrete_impl_arg(engine.pool_mut(), concrete) else {
                return;
            };
            impl_args.push(GenericArg::Type(resolved));
        }
    }

    // The method's `<T>`-style binders, resolved to concrete args; `None` when
    // any binder arg still carries type vars (skip this pass).
    let Some((method_args, method_named, mut var_subst, const_bindings)) =
        resolve_method_binder_args(engine, sig, resolved_const_bindings)
    else {
        return;
    };

    // INVARIANT: substitution follows links without collapsing the `Applied` shell.
    let empty: FxHashMap<u32, Idx> = FxHashMap::default();
    let concrete_param_types: Vec<Idx> = sig
        .params
        .iter()
        .map(|&p| substitute_in_pool(engine.pool_mut(), p, &empty))
        .collect();
    let concrete_return_type = substitute_in_pool(engine.pool_mut(), sig.ret, &empty);
    if concrete_param_types
        .iter()
        .any(|&p| !is_recordable(engine, p))
        || !is_recordable(engine, concrete_return_type)
    {
        return;
    }

    let (body_type_map, extra_named) =
        build_method_body_type_map(engine, sig, mono, receiver, method_named, &mut var_subst);

    let Some(producer) = sig.producer.clone() else {
        return;
    };
    let instance = MonoInstance::new_method_with_const_bindings(
        method_name,
        producer,
        impl_args,
        method_args,
        const_bindings,
        ConcreteMethodMono {
            receiver_type: receiver,
            param_types: concrete_param_types,
            return_type: concrete_return_type,
            body_type_map,
        },
    );

    tracing::debug!(
        fn_name = ?method_name,
        receiver = ?receiver,
        impl_args = ?instance.impl_args,
        method_args = ?instance.method_args,
        extra_named = ?extra_named,
        body_type_map = ?instance.body_type_map,
        "recorded mono instance"
    );

    if let Some(call_expr_id) = call_expr_id {
        engine.record_mono_with_dispatch(call_expr_id, instance);
    } else {
        // INVARIANT: operator targets need inventory bodies despite lacking call nodes.
        engine.record_mono_instance(instance);
    }
}

/// `(body_type_map, extra_named)` returned by [`build_method_body_type_map`]:
/// the generic-body-type → concrete substitutions and the name-keyed binder
/// list threaded into the recorded-instance trace.
type MethodBodyTypeMap = (Vec<(Idx, Idx)>, Vec<(Name, Idx)>);

/// Build the body type map for a method instance and register the receiver's
/// concrete applied resolution. Returns `(body_type_map, extra_named)`; the
/// caller threads `extra_named` into the recorded-instance trace.
fn build_method_body_type_map(
    engine: &mut InferEngine<'_>,
    sig: &ImplMethodSig,
    mono: Option<&MethodMonoData>,
    receiver: Idx,
    method_named: Vec<(Name, Idx)>,
    var_subst: &mut FxHashMap<u32, Idx>,
) -> MethodBodyTypeMap {
    crate::pool::substitute::extend_var_subst_with_roots(
        engine.pool(),
        &sig.scheme_var_ids,
        var_subst,
    );
    let generic_type_params = collect_generic_type_params(engine);
    // INVARIANT: Impl and method binder names both map to body rigid variables.
    let mut extra_named: Vec<(Name, Idx)> = mono.map_or_else(Vec::new, |mono| {
        mono.impl_type_args
            .iter()
            .map(|&(name, concrete)| (name, engine.resolve(concrete)))
            .collect()
    });
    extra_named.extend(method_named);
    let pool = engine.pool_mut();
    let body_type_map =
        build_and_register_body_type_map(pool, var_subst, &extra_named, &generic_type_params);

    // INVARIANT: Register the concrete receiver separately; the body map holds binders only.
    resolve_applied_type(pool, receiver, &generic_type_params);

    (body_type_map, extra_named)
}

/// True when `ty` carries no remaining type variables / inference holes.
/// The var/infer half of `MonoInstance` recordability — `is_recordable` adds
/// the poison exclusion; the record gates use `is_recordable`, this remains a
/// debug-trace signal for the var/infer dimension alone.
fn is_fully_concrete(engine: &InferEngine<'_>, ty: Idx) -> bool {
    !engine.pool().flags(ty).has_any_var_or_infer()
}

/// True when `ty` is recordable as a `MonoInstance`: fully concrete AND not
/// poison. The poison half (`!has_errors`) is what `is_fully_concrete` omits —
/// a type-error `Idx::ERROR` substitution must never be monomorphized.
fn is_recordable(engine: &InferEngine<'_>, ty: Idx) -> bool {
    engine.pool().flags(ty).is_recordable()
}

#[cfg(test)]
mod tests {
    use ori_ir::Name;

    use crate::{Idx, Pool, TypeFlags, VarState};

    use super::concrete_impl_arg;

    fn resolved_applied(pool: &mut Pool) -> Idx {
        let owner = Name::from_raw(1);
        let field = Name::from_raw(2);
        let applied = pool.applied(owner, &[Idx::INT]);
        let body = pool.struct_type(owner, &[(field, Idx::INT)]);
        pool.set_resolution(applied, body);
        applied
    }

    #[test]
    fn impl_arg_preserves_direct_nominal_applied_identity() {
        let mut pool = Pool::new();
        let applied = resolved_applied(&mut pool);

        assert_eq!(concrete_impl_arg(&mut pool, applied), Some(applied));
    }

    #[test]
    fn impl_arg_preserves_linked_nominal_applied_identity() {
        let mut pool = Pool::new();
        let applied = resolved_applied(&mut pool);
        let linked = pool.fresh_var();
        *pool.var_state_mut(pool.data(linked)) = VarState::Link { target: applied };

        assert_eq!(concrete_impl_arg(&mut pool, linked), Some(applied));
    }

    #[test]
    fn impl_arg_uses_link_materialization_for_recordability_but_keeps_identity() {
        let mut pool = Pool::new();
        let owner = Name::from_raw(1);
        let field = Name::from_raw(2);
        let inner_var = pool.fresh_var();
        let inner = pool.applied(owner, &[inner_var]);
        let inner_body = pool.struct_type(owner, &[(field, Idx::INT)]);
        pool.set_resolution(inner, inner_body);
        let outer_var = pool.fresh_var();
        let outer = pool.applied(owner, &[outer_var]);
        let outer_body = pool.struct_type(owner, &[(field, inner)]);
        pool.set_resolution(outer, outer_body);
        *pool.var_state_mut(pool.data(inner_var)) = VarState::Link { target: Idx::INT };
        *pool.var_state_mut(pool.data(outer_var)) = VarState::Link { target: inner };

        assert!(pool.flags(outer).contains(TypeFlags::HAS_VAR));
        assert_eq!(concrete_impl_arg(&mut pool, outer), Some(outer));
    }

    #[test]
    fn impl_arg_rejects_an_unresolved_nominal_parameter() {
        let mut pool = Pool::new();
        let owner = Name::from_raw(1);
        let unresolved = pool.fresh_var();
        let applied = pool.applied(owner, &[unresolved]);
        let body = pool.struct_type(owner, &[]);
        pool.set_resolution(applied, body);

        assert!(pool.flags(applied).contains(TypeFlags::HAS_VAR));
        assert_eq!(concrete_impl_arg(&mut pool, applied), None);
    }

    #[test]
    fn impl_arg_keeps_prior_resolution_for_non_applied_types() {
        let mut pool = Pool::new();
        let alias = pool.named(Name::from_raw(3));
        let newtype_name = Name::from_raw(4);
        let newtype = pool.named(newtype_name);
        let list = pool.list(Idx::INT);
        pool.set_resolution(alias, Idx::INT);
        pool.register_newtype_ctor(newtype_name, Idx::INT);
        pool.set_resolution(newtype, Idx::INT);

        assert_eq!(concrete_impl_arg(&mut pool, alias), Some(Idx::INT));
        assert_eq!(concrete_impl_arg(&mut pool, newtype), Some(Idx::INT));
        assert_eq!(concrete_impl_arg(&mut pool, list), Some(list));
    }
}
