//! Method-dispatch diagnostics: `into`-not-implemented + the general
//! method-not-found emit — diagnostic construction, distinct from the
//! lookup/resolution paths in `impl_lookup.rs`.

use ori_ir::{Name, Span};

use super::super::super::InferEngine;
use crate::infer::tag_to_type_tag;
use crate::{Idx, Tag, TypeCheckError, UnboundVarState};

/// True iff `receiver_ty` (already union-find-resolved) is an unbound NAMED
/// `Tag::Var` that denotes a generic type parameter (`@f<T>(x: T)`).
///
/// A generic type parameter surfaces at dispatch as a named `Tag::Var`
/// (`fresh_named_var`, `Unbound { name: Some(_) }`) rather than a `RigidVar`.
/// A capability / trait-namespace receiver (`Http.get(...)` where `Http` is a
/// trait used as a capability) ALSO surfaces as a named unbound `Tag::Var` —
/// the capability/trait-associated resolution is incomplete in typeck (CP-3
/// target-only), so the name stays an unbound var. The two are separated by
/// the registry: a named var whose name is a REGISTERED TRAIT is the
/// capability/trait-namespace case (its proper resolution is the trait path,
/// so a `NotFound` must DEFER, not diagnose); only a named var whose name is
/// NOT a trait is a genuine generic parameter.
pub(super) fn is_named_generic_var(engine: &InferEngine<'_>, receiver_ty: Idx) -> bool {
    if engine.pool().tag(receiver_ty) != Tag::Var {
        return false;
    }
    let var_id = engine.pool().data(receiver_ty);
    // Extract the optional name (Copy) before borrowing the trait registry
    // (borrow-dance per `calls.rs:resolve_impl_method`).
    let var_name = match engine.pool().var_state_checked(var_id) {
        Some(crate::pool::VarState::Unbound(UnboundVarState { name: Some(n), .. })) => *n,
        _ => return false,
    };
    !engine
        .trait_registry()
        .is_some_and(|tr| tr.contains_trait(var_name))
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

/// Emit a "no method `m` on generic type parameter" diagnostic for a genuine
/// `NotFound` method lookup on a RIGID receiver (a generic type parameter with
/// no matching trait bound): bound-chain dispatch, e.g. `@f<T>(x)=x.hello()`.
///
/// Caller MUST gate this on the outcome having been `LookupOutcome::NotFound`
/// (NOT `Ambiguous` — that path already pushed `E2023`).
///
/// SCOPE — rigid receivers ONLY. A `NotFound` on a CONCRETE receiver does NOT
/// imply the method is genuinely absent: typeck's concrete-receiver dispatch is
/// incomplete (struct field-callables like `s.transform(21)`, builtin trait
/// methods like `int.default()` / `x.clone()`, builtin collection methods like
/// `list.updated(...)`), and the evaluator resolves these via its own dispatch
/// (`ori_eval` `methods/units.rs` / `numeric.rs`). Emitting on a concrete miss
/// would false-positive on every such legitimate call. The concrete-receiver
/// silent-poison case (e.g. `{str:int}.map(...)`) is therefore not diagnosed:
/// its cure depends on completing concrete-receiver typeck dispatch so a miss
/// reliably implies genuine absence. A RIGID miss is unambiguous — an unbounded
/// generic `T` has no methods anywhere (the evaluator cannot resolve them
/// either), so there is no false-positive surface.
///
/// Receiver-kind discipline:
/// - `Idx::ERROR` / `Tag::Error` receiver: skip — error-recovery monotonicity
///   (never cascade on an already-poisoned receiver).
/// - `Tag::RigidVar` / NAMED unbound `Tag::Var`: emit with an "add a trait
///   bound" suggestion (a generic parameter has no methods without a bound).
/// - every other receiver (concrete, anonymous `Tag::Var`, `Tag::Infer`,
///   `Tag::SelfType`, `Tag::BoundVar`, `Tag::Projection`, `into`): skip — either
///   a deferred placeholder, or a concrete receiver whose dispatch typeck does
///   not yet fully cover.
pub(super) fn emit_unknown_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: Name,
    span: Span,
) {
    if receiver_ty == Idx::ERROR {
        return;
    }
    // Resolve through union-find first: a function param `x: T` reaches dispatch
    // as a `Tag::Var` that links to the generic's `Tag::RigidVar`; the unresolved
    // surface tag would otherwise defer and re-introduce the silent accept.
    let receiver_ty = engine.resolve(receiver_ty);
    if receiver_ty == Idx::ERROR {
        return;
    }
    // `into` is never a method-not-found here — it has its own E2036 path.
    if engine
        .well_known()
        .is_some_and(|wk| method == wk.into_method)
    {
        return;
    }
    let tag = engine.pool().tag(receiver_ty);
    // A NAMED unbound `Tag::Var` that is a generic type parameter (`@f<T>`, name
    // not a registered trait) is diagnosable (no method on the generic; add a
    // bound). A capability/trait-namespace named var (`Http`) and an ANONYMOUS
    // `Tag::Var` are deferred — see `is_named_generic_var`.
    let treat_as_rigid = tag == Tag::RigidVar || is_named_generic_var(engine, receiver_ty);
    if treat_as_rigid {
        let method_str = engine.lookup_name(method).unwrap_or("<method>").to_owned();
        let msg = format!(
            "no method `{method_str}` on generic type parameter — add a trait \
             bound providing `{method_str}` (e.g. `<T: SomeTrait>`)"
        );
        engine.push_error(TypeCheckError::unsatisfied_bound(span, msg));
        return;
    }
    // Reject ONLY builtin-dispatchable receivers — `ori_registry` is the complete
    // method surface for these (RG-4), so a miss after builtin + trait lookup is a
    // genuine absence. Non-builtin tags DEFER (a miss is not a proven absence):
    // user `Named`/`Applied` types resolve extensions, derived/inherent impls, and
    // callable struct fields via the evaluator (typeck dispatch is incomplete for
    // them); placeholders (`Var`/`Infer`/`BoundVar`/`Projection`/`SelfType`) have
    // no determined dispatch target; `ModuleNs` is a qualified function call.
    // `tag_to_type_tag` returns `None` for every non-builtin tag. BUG-02-041 is
    // scoped to builtin receivers.
    let Some(type_tag) = tag_to_type_tag(tag) else {
        return;
    };
    // An `extend <builtin> { @m }` extension provides a method the registry lacks
    // — not a genuine unknown; the evaluator resolves it (TR-9 dispatch is
    // target-only, so typeck only avoids the false-reject here).
    if engine.builtin_extension_provides(type_tag, method) {
        return;
    }
    // Genuine absence on a builtin receiver: the registry is the SSOT (RG-4) and a
    // miss after builtin + trait + extension lookup means the method does not exist.
    engine.push_error(TypeCheckError::unknown_method(span, receiver_ty, method));
}
