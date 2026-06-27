//! Module-alias qualified-call resolution: `alias.func(args)`.
//!
//! A `use "path" as alias` import binds `alias` as a namespace placeholder
//! (`Tag::Named(alias)`) carrying the aliased module's public function
//! signatures (`InferEngine::module_alias_sigs`). A qualified call
//! `alias.func(args)` parses as a `MethodCall` whose receiver types to that
//! placeholder; this module resolves the call against the named function's
//! signature so it types to the function's return type instead of poisoning
//! to `Idx::ERROR` (Spec: Clause 12 Module Alias).

use ori_ir::{ExprArena, ExprId, Name, Span};

use super::super::super::InferEngine;
use super::super::infer_expr;
use crate::{ContextKind, Expected, ExpectedOrigin, FunctionSig, Idx, Tag};

/// Resolve a qualified positional call `alias.func(arg, ...)` against the
/// aliased module's signature. Returns `Some(return_type)` when `receiver`
/// resolves to a registered module-alias namespace AND the namespace exports
/// `method`; `None` otherwise (the caller proceeds with ordinary dispatch).
pub(super) fn try_infer_module_alias_call(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver: ExprId,
    method: Name,
    arg_ids: &[ExprId],
    span: Span,
) -> Option<Idx> {
    let sig = resolve_alias_sig(engine, arena, receiver, method)?;
    Some(check_positional(engine, arena, &sig, arg_ids, span))
}

/// Named-argument variant of [`try_infer_module_alias_call`]: resolves
/// `alias.func(name: value, ...)` against the aliased signature, matching
/// each named argument to its parameter by name.
pub(super) fn try_infer_module_alias_call_named(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver: ExprId,
    method: Name,
    args: &[ori_ir::CallArg],
    span: Span,
) -> Option<Idx> {
    let sig = resolve_alias_sig(engine, arena, receiver, method)?;
    Some(check_named(engine, arena, &sig, args, span))
}

/// Resolve the receiver to a module-alias namespace and look up `method` in
/// its exported signatures. Returns the matching `FunctionSig` (cloned, owned)
/// or `None` when the receiver is not an alias namespace or the function is
/// absent.
fn resolve_alias_sig(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver: ExprId,
    method: Name,
) -> Option<FunctionSig> {
    let receiver_ty = infer_expr(engine, arena, receiver);
    let resolved = engine.resolve(receiver_ty);
    if engine.pool().tag(resolved) != Tag::Named {
        return None;
    }
    let alias = engine.pool().named_name(resolved);
    let sigs = engine.module_alias_sigs(alias)?;
    sigs.iter().find(|s| s.name == method).cloned()
}

/// Check positional args against the resolved signature's parameter types and
/// return its return type. Mirrors the free-function positional-call contract:
/// each argument is checked against its parameter; the aliased stdlib functions
/// are monomorphic, so no instantiation is required.
fn check_positional(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    sig: &FunctionSig,
    arg_ids: &[ExprId],
    span: Span,
) -> Idx {
    for (i, &arg_id) in arg_ids.iter().enumerate() {
        let arg_ty = infer_expr(engine, arena, arg_id);
        if let Some(&param_ty) = sig.param_types.get(i) {
            let expected = Expected {
                ty: param_ty,
                origin: ExpectedOrigin::Context {
                    span,
                    kind: ContextKind::FunctionArgument {
                        func_name: Some(sig.name),
                        arg_index: i,
                        param_name: sig.param_names.get(i).copied(),
                    },
                },
            };
            let _ = engine.check_type(arg_ty, &expected, arena.get_expr(arg_id).span);
        }
    }
    sig.return_type
}

/// Check named args against the resolved signature, matching each `name: value`
/// to its parameter by name. An unmatched argument name is left to surface via
/// the ordinary argument-checking path's diagnostics; here a missing match
/// simply skips the per-arg constraint (the value is still inferred for side
/// effects).
fn check_named(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    sig: &FunctionSig,
    args: &[ori_ir::CallArg],
    span: Span,
) -> Idx {
    for arg in args {
        let arg_ty = infer_expr(engine, arena, arg.value);
        let param_idx = arg
            .name
            .and_then(|n| sig.param_names.iter().position(|&p| p == n));
        if let Some(idx) = param_idx {
            if let Some(&param_ty) = sig.param_types.get(idx) {
                let expected = Expected {
                    ty: param_ty,
                    origin: ExpectedOrigin::Context {
                        span,
                        kind: ContextKind::FunctionArgument {
                            func_name: Some(sig.name),
                            arg_index: idx,
                            param_name: arg.name,
                        },
                    },
                };
                let _ = engine.check_type(arg_ty, &expected, arena.get_expr(arg.value).span);
            }
        }
    }
    sig.return_type
}
