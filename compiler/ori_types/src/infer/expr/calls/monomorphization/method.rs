//! Monomorphization instance recording for generic method calls.
//!
//! Diagnostics share the `ori_types::mono` target with free-function
//! monomorphization so one filter observes the complete realization request.

use ori_ir::{ExprId, Name};
use rustc_hash::FxHashMap;

use crate::pool::substitute::substitute_in_pool;
use crate::{
    ConcreteMethodMono, ConstGenericTerm, Expected, GenericArg, Idx, MonoConstBinding, MonoInstance,
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
    expected: Option<&Expected>,
) -> Option<MethodBinderArgs> {
    let const_param_count = sig
        .generic_param_metadata
        .iter()
        .filter(|meta| meta.is_const)
        .count();
    let mut const_terms = expected.into_iter().flat_map(Expected::const_terms);
    let mut method_args = Vec::with_capacity(sig.generic_param_metadata.len());
    let mut method_named: Vec<(Name, Idx)> = Vec::with_capacity(sig.scheme_var_ids.len());
    let mut var_subst: FxHashMap<u32, Idx> = FxHashMap::default();
    let mut const_bindings = Vec::with_capacity(const_param_count);
    let mut scheme_vars = sig.scheme_var_ids.iter().copied();
    for meta in &sig.generic_param_metadata {
        if meta.is_const {
            let Some(ConstGenericTerm::Value(value)) = const_terms.next() else {
                return None;
            };
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
    expected: Option<&Expected>,
) {
    // Entry gate keys on EITHER binder axis: `method_mono` (impl-level, set when
    // the impl is generic over the receiver — `impl<T> Box<T>`) OR
    // `scheme_var_ids` (method-level, the method's own `<U>` binders, present
    // even on a concrete-receiver impl — `impl Boxer { @pick<T> }`). Keying on
    // `method_mono` alone conflates "impl is generic over the receiver" with
    // "method needs monomorphization"; a method-level-only generic then records
    // no MonoInstance and its `Tag::RigidVar` survives to codegen (PC-2 break).
    let mono = sig.method_mono.as_ref();
    if mono.is_none() && sig.generic_param_metadata.is_empty() {
        return;
    }

    // Receiver carrier: the FULL concrete receiver Idx (e.g. `Box<int>`, NOT
    // `substitute_in_pool` follows linked children while retaining the `Applied`
    // shell required to match an impl method's rigid self parameter.
    let receiver = substitute_in_pool(engine.pool_mut(), receiver_ty, &FxHashMap::default());
    let ret_resolved = engine.resolve(sig.ret);
    tracing::debug!(
        target: "ori_types::mono",
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

    // Impl-level + method-level concrete arguments. `impl_type_args` is the
    // receiver-side substitution in declaration order (`[(T, int)]`); method-
    // level args come from the call-site instantiation of `<U>`-style binders.
    // A method-level-only generic on a concrete-receiver impl carries no impl
    // binders (`mono` is `None`) — `impl_args` is empty, and `MonoInstance::
    // new_method` accepts empty `impl_args` with populated `method_args`.
    let mut impl_args = Vec::with_capacity(mono.map_or(0, |m| m.impl_type_args.len()));
    if let Some(mono) = mono {
        for &(_, concrete) in &mono.impl_type_args {
            let resolved = engine.pool().resolve_fully(concrete);
            if !is_recordable(engine, resolved) {
                return;
            }
            impl_args.push(GenericArg::Type(resolved));
        }
    }

    // The method's `<T>`-style binders, resolved to concrete args; `None` when
    // any binder arg still carries type vars (skip this pass).
    let Some((method_args, method_named, mut var_subst, const_bindings)) =
        resolve_method_binder_args(engine, sig, expected)
    else {
        return;
    };

    // Concrete param / return types via `substitute_in_pool` (empty map), which
    // follows each child Var's `VarState::Link` while PRESERVING the `Applied`
    // shape — `Applied(Pair, [Var->str, Var->int])` deep-resolves to the
    // concrete `Applied(Pair, [str, int])`. `resolve_fully` is wrong for this step: its
    // matching-args fallback collapses `Pair<str, int>` to the generic `Pair`
    // struct, so codegen would emit the wrong layout (`{i64, i64}`) and fail IR
    // verification on a permuted-generic return like `swap (self) -> Pair<B, A>`.
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
        target: "ori_types::mono",
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
        // Operator expressions have no canonical call node to rewrite, but
        // their selected generic method still needs a concrete body in the
        // executable inventory. The ARC operator-target pass binds that body
        // through its receiver-qualified method fact.
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
    // `body_type_map` maps each generic body type to its concrete form: the
    // method-level scheme vars via the canonical SSOT helper, plus the impl-
    // level `Tag::Named(binder)` entries the var-keyed helper cannot reach.
    crate::pool::substitute::extend_var_subst_with_roots(
        engine.pool(),
        &sig.scheme_var_ids,
        var_subst,
    );
    let generic_type_params = collect_generic_type_params(engine);
    // Name-keyed binder entries the var-keyed helper cannot reach — both the
    // impl-level `Tag::Named(binder)` binders (when `mono` is `Some`) AND the
    // method-level `<T>` binders (the body's `VarState::Rigid { name }` rigid
    // vars). `build_and_register_body_type_map` runs `build_impl_rigid_var_subst`
    // over `extra_named` (a pool scan mapping each binder NAME to its body rigid
    // var_id) so a `[T]`-returning method body re-interns to `[int]`. Without the
    // method binders, the signature monomorphizes but the body's rigid leaf
    // survives to codegen (the `Tag::rigid_var` symptom).
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

    // The receiver's own concrete Applied type (e.g. `Box<str>`) is the `self`
    // type — never a value in `body_type_map`, which carries only binder
    // substitutions (`Named("T") -> str`). Register its concrete struct
    // resolution directly so the LLVM `TypeInfoStore` resolves the receiver
    // type AND its structurally-interned construction-site `Idx` with the
    // substituted field layout instead of the generic `Box<T>` fallback. The
    // helper recurses into nested generic fields (e.g. `Wrapper<Box<int>>`).
    // The mono-dispatch shell key derives from the receiver's `Tag::Applied`
    // structure directly (`Pool::generic_shell`), so this resolution does not
    // collapse the `(method, Box<_>)` dispatch key.
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
