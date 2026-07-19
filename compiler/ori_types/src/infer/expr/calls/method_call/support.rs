//! Argument checking and receiver specialization for method calls.

use ori_diagnostic::Suggestion;
use ori_ir::{ExprArena, ExprId, Name, Span};

use crate::infer::expr::type_resolution::resolve_parsed_type_list;
use crate::infer::InferEngine;
use crate::{ContextKind, Expected, ExpectedOrigin, Idx, Tag};

use super::super::super::{infer_expr, lookup_struct_field_types};
use super::super::impl_lookup::ImplMethodSig;

pub(super) fn check_named_args(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    args: ori_ir::CallArgRange,
    sig: &ImplMethodSig,
    span: Span,
) {
    for (i, (arg, &param_ty)) in arena
        .get_call_args(args)
        .iter()
        .zip(sig.params.iter())
        .enumerate()
    {
        let expected = Expected {
            ty: param_ty,
            origin: ExpectedOrigin::Context {
                span,
                kind: ContextKind::FunctionArgument {
                    func_name: None,
                    arg_index: i,
                    param_name: arg.name,
                },
            },
        };
        let arg_ty = infer_expr(engine, arena, arg.value);
        let _ = engine.check_type(arg_ty, &expected, arg.span);
    }
}

/// Return the function type of a callable struct field named `method`.
pub(super) fn callable_field_fn_ty(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: Name,
) -> Option<Idx> {
    let (type_name, type_args) = match engine.pool().tag(receiver_ty) {
        Tag::Named => (engine.pool().named_name(receiver_ty), None),
        Tag::Applied => (
            engine.pool().applied_name(receiver_ty),
            Some(engine.pool().applied_args(receiver_ty)),
        ),
        _ => return None,
    };
    let fields = lookup_struct_field_types(engine, type_name, type_args.as_deref())?;
    let field_ty = engine.resolve(*fields.get(&method)?);
    (engine.pool().tag(field_ty) == Tag::Function).then_some(field_ty)
}

/// Check positional arguments against a callable field and return its result type.
pub(super) fn check_callable_field_positional(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    fn_ty: Idx,
    arg_ids: &[ExprId],
    span: Span,
    expected: Option<&Expected>,
) -> Idx {
    let params = engine.pool().function_params(fn_ty);
    let ret = engine.pool().function_return(fn_ty);
    for (i, &arg_id) in arg_ids.iter().enumerate() {
        let arg_ty = infer_expr(engine, arena, arg_id);
        if let Some(&param_ty) = params.get(i) {
            let arg_expected = Expected {
                ty: param_ty,
                origin: ExpectedOrigin::Context {
                    span,
                    kind: ContextKind::FunctionArgument {
                        func_name: None,
                        arg_index: i,
                        param_name: None,
                    },
                },
            };
            let _ = engine.check_type(arg_ty, &arg_expected, arena.get_expr(arg_id).span);
        }
    }
    if let Some(exp) = expected {
        let _ = engine.check_type(ret, exp, span);
    }
    ret
}

/// Apply explicit receiver type arguments before associated-function lookup.
pub(super) fn apply_receiver_type_args(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    resolved: Idx,
    receiver: ExprId,
    _span: Span,
) -> Idx {
    let type_args = arena.receiver_type_args(receiver);
    if type_args.is_empty() {
        return resolved;
    }
    let arg_idxs = resolve_parsed_type_list(engine, arena, type_args);
    match engine.pool().tag(resolved) {
        Tag::Applied => {
            let existing = engine.pool().applied_args(resolved);
            for (&recv_arg, &explicit) in existing.iter().zip(arg_idxs.iter()) {
                let _ = engine.unify_types(recv_arg, explicit);
            }
            resolved
        }
        Tag::Named => {
            let base_name = engine.pool().named_name(resolved);
            let resolved_wk = if let Some(well_known) = engine.well_known() {
                well_known.resolve_generic(engine.pool_mut(), base_name, &arg_idxs)
            } else {
                None
            };
            resolved_wk.unwrap_or_else(|| engine.pool_mut().applied(base_name, &arg_idxs))
        }
        _ => resolved,
    }
}

/// Type-check positional method-call arguments against the resolved signature.
pub(super) fn check_positional_args(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    arg_ids: &[ExprId],
    sig: &ImplMethodSig,
    span: Span,
) -> Idx {
    for (i, (&arg_id, &param_ty)) in arg_ids.iter().zip(sig.params.iter()).enumerate() {
        let expected = Expected {
            ty: param_ty,
            origin: ExpectedOrigin::Context {
                span,
                kind: ContextKind::FunctionArgument {
                    func_name: None,
                    arg_index: i,
                    param_name: None,
                },
            },
        };
        let arg_ty = infer_expr(engine, arena, arg_id);
        let _ = engine.check_type(arg_ty, &expected, arena.get_expr(arg_id).span);
    }
    sig.ret
}

/// Suggest the concrete iterator conversion when the result type supports it.
pub(crate) fn suggest_iterator_fix(inner_tag: Tag) -> Suggestion {
    use super::super::super::registry_bridge::tag_to_type_tag;

    let has_iter = tag_to_type_tag(inner_tag)
        .and_then(|type_tag| ori_registry::find_method(type_tag, "iter"))
        .is_some();
    let text = if has_iter {
        "this type is not an Iterator; call `.iter()` on it (e.g., `[x, x * 10].iter()`)"
    } else {
        "this type is not an Iterator; `flat_map` requires the closure to return an iterator type"
    };
    Suggestion::text(text, 1)
}
