//! Impl method lookup and signature resolution via `TraitRegistry`.

use ori_ir::{Name, Span};

use super::super::super::InferEngine;
use crate::{Idx, MethodLookupResult, Tag, TypeCheckError};

/// Result of looking up a method in the `TraitRegistry`.
pub(super) enum LookupOutcome {
    Found { sig: Idx, has_self: bool },
    Ambiguous(Vec<ori_ir::Name>),
    NotFound,
}

/// Successfully resolved impl method signature.
pub(super) struct ImplMethodSig {
    /// Method parameters (excluding `self`).
    pub(super) params: Vec<Idx>,
    /// Return type.
    pub(super) ret: Idx,
}

/// Perform the borrow-dance lookup for impl methods via `TraitRegistry`.
///
/// Scopes the immutable `trait_registry` borrow to extract data, so the
/// caller can use `engine` mutably afterwards.
pub(super) fn lookup_impl_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: Name,
) -> LookupOutcome {
    let trait_registry = engine.trait_registry();
    match trait_registry {
        None => LookupOutcome::NotFound,
        Some(reg) => match reg.lookup_method_checked(receiver_ty, method) {
            MethodLookupResult::Found(lookup) => LookupOutcome::Found {
                sig: lookup.method().signature,
                has_self: lookup.method().has_self,
            },
            MethodLookupResult::Ambiguous { candidates } => {
                LookupOutcome::Ambiguous(candidates.iter().map(|&(_, n)| n).collect())
            }
            MethodLookupResult::NotFound => LookupOutcome::NotFound,
        },
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
    let (sig_ty, has_self) = match outcome {
        LookupOutcome::Found { sig, has_self } => (sig, has_self),
        LookupOutcome::Ambiguous(trait_names) => {
            engine.push_error(TypeCheckError::ambiguous_method(span, method, trait_names));
            return Some(Err(()));
        }
        LookupOutcome::NotFound => return None,
    };

    let resolved_sig = engine.resolve(sig_ty);
    if engine.pool().tag(resolved_sig) != Tag::Function {
        return Some(Err(()));
    }

    let params = engine.pool().function_params(resolved_sig);
    let ret = engine.pool().function_return(resolved_sig);

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
