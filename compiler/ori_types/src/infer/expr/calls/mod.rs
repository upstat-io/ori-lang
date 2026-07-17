//! Function call and method call inference.

mod call_inference;
mod closure_unify;
mod constraints;
mod impl_lookup;
mod impl_signature;
mod infinite_iterator;
mod method_call;
mod method_diagnostics;
mod method_receiver;
mod module_alias_call;
mod monomorphization;
mod traits;

use ori_ir::{Name, Span};

use crate::{ContextKind, Expected, ExpectedOrigin, Idx};

pub(crate) use call_inference::{infer_call, infer_call_named};
pub(super) use method_call::{infer_method_call, infer_method_call_named, MethodCallSite};
pub(crate) use monomorphization::register_concrete_applied_resolutions;
pub(crate) use monomorphization::{compose_builtin_burdens_for_resolved_types, compose_for_idx};
pub use monomorphization::{compose_burden_for_idx, register_resolved_collection_burdens};

// Consumed by the operator-presence gate (`operators::dispatch`) for
// generic-impl instantiation-bound validation.
pub(crate) use impl_lookup::{match_self_type, pool_base_name};
pub(crate) use traits::type_satisfies_trait as type_satisfies_named_trait;

/// Resolve one user-defined binary operator through the ordinary impl-method
/// path and publish any concrete generic-method demand it selects.
///
/// Operators are not canonical call expressions, so the generated
/// `MonoInstance` intentionally has no expression-dispatch entry. ARC carries
/// the exact receiver-qualified method fact used to bind the realized body.
pub(crate) fn resolve_operator_method(
    engine: &mut super::super::InferEngine<'_>,
    receiver_ty: Idx,
    argument_ty: Idx,
    argument_span: Span,
    method: Name,
    op: &'static str,
    span: Span,
) -> Option<Idx> {
    let outcome = impl_lookup::lookup_impl_method(engine, receiver_ty, method);
    let Ok(sig) = impl_signature::resolve_impl_signature(engine, outcome, method, 1, span)? else {
        return Some(Idx::ERROR);
    };
    if sig.params.len() != 1 {
        return Some(Idx::ERROR);
    }

    let expected = Expected {
        ty: sig.params[0],
        origin: ExpectedOrigin::Context {
            span,
            kind: ContextKind::BinaryOpRight { op },
        },
    };
    let _ = engine.check_type(argument_ty, &expected, argument_span);
    monomorphization::maybe_record_method_mono_instance(
        engine,
        None,
        method,
        receiver_ty,
        &sig,
        None,
    );
    Some(sig.ret)
}

// Re-export for tests (accessed via `super::calls::type_satisfies_trait` etc.)
#[cfg(test)]
pub(crate) use infinite_iterator::find_infinite_source;
#[cfg(test)]
pub(crate) use method_call::suggest_iterator_fix;
#[cfg(test)]
pub(crate) use traits::type_satisfies_trait;

#[cfg(test)]
mod tests;
