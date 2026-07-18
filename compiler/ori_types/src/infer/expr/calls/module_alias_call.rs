//! Module-alias qualified-call resolution: `alias.func(args)`.
//!
//! A `use "path" as alias` import binds `alias` as a namespace placeholder
//! (`Tag::Named(alias)`) carrying the aliased module's public function
//! signatures (`InferEngine::module_alias_sigs`). A qualified call
//! `alias.func(args)` parses as a `MethodCall` whose receiver types to that
//! placeholder; this module resolves the call against the named function's
//! signature so it types to the function's return type instead of poisoning
//! to `Idx::ERROR` (Spec: Clause 18.3.4 Module Aliases).

use ori_ir::{ExprArena, ExprId, Name, Span};

use super::super::super::InferEngine;
use super::super::infer_expr;
use super::constraints::check_signature_capabilities;
use crate::{ContextKind, Expected, ExpectedOrigin, FunctionSig, Idx, Tag, TypeCheckError};

/// Resolve a qualified positional call `alias.func(arg, ...)` against the
/// aliased module's signature. Returns `Some(return_type)` when `receiver`
/// resolves to a registered module-alias namespace AND the namespace exports
/// `method`; `None` otherwise (the caller proceeds with ordinary dispatch).
pub(super) fn try_infer_module_alias_call(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    call_expr_id: ExprId,
    receiver: ExprId,
    method: Name,
    arg_ids: &[ExprId],
    span: Span,
) -> Option<Idx> {
    let (alias, sig) = resolve_alias_sig(engine, arena, receiver, method)?;
    let qualified = record_qualified_call(engine, call_expr_id, alias, method).unwrap_or(method);
    check_signature_capabilities(engine, call_expr_id, qualified, &sig, span);
    Some(check_positional(engine, arena, &sig, arg_ids, span))
}

/// Named-argument variant of [`try_infer_module_alias_call`]: resolves
/// `alias.func(name: value, ...)` against the aliased signature, matching
/// each named argument to its parameter by name.
pub(super) fn try_infer_module_alias_call_named(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    call_expr_id: ExprId,
    receiver: ExprId,
    method: Name,
    args: &[ori_ir::CallArg],
    span: Span,
) -> Option<Idx> {
    let (alias, sig) = resolve_alias_sig(engine, arena, receiver, method)?;
    let qualified = record_qualified_call(engine, call_expr_id, alias, method).unwrap_or(method);
    check_signature_capabilities(engine, call_expr_id, qualified, &sig, span);
    Some(check_named(engine, arena, &sig, args, span))
}

/// Record `(call_expr_id → "alias.method")` into the engine's module-alias
/// side-table so `ori_canon` rewrites this namespace `MethodCall` to a free
/// `CanExpr::Call`. The interned qualified `Name` MUST match the synthesized
/// imported-function `local_name` (`oric::imports`) so the rewritten `Call`
/// links to the declared import. No-op if the interner is unavailable.
fn record_qualified_call(
    engine: &mut InferEngine<'_>,
    call_expr_id: ExprId,
    alias: Name,
    method: Name,
) -> Option<Name> {
    let qualified = (|| {
        let alias_str = engine.lookup_name(alias)?;
        let method_str = engine.lookup_name(method)?;
        engine.intern_name(&ori_ir::qualified_alias_name(alias_str, method_str))
    })();
    if let Some(qualified) = qualified {
        engine.record_module_alias_call(call_expr_id, qualified);
    }
    qualified
}

/// Resolve the receiver to a module-alias namespace and look up `method` in
/// its exported signatures. Returns `(alias_name, FunctionSig)` (the sig
/// cloned, owned) or `None` when the receiver is not an alias namespace or the
/// function is absent. The alias name forms the qualified rewrite target.
fn resolve_alias_sig(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver: ExprId,
    method: Name,
) -> Option<(Name, FunctionSig)> {
    let receiver_ty = infer_expr(engine, arena, receiver);
    let resolved = engine.resolve(receiver_ty);
    if engine.pool().tag(resolved) != Tag::Named {
        return None;
    }
    let alias = engine.pool().named_name(resolved);
    let sigs = engine.module_alias_sigs(alias)?;
    let sig = sigs.iter().find(|s| s.name == method).cloned()?;
    Some((alias, sig))
}

/// Check positional args against the resolved signature's parameter types and
/// return its return type. Mirrors the free-function positional-call contract
/// (`infer_call`): the arg count is arity-checked against the signature
/// (`required_params <= count <= param_types.len()`, emitting `E2004` and
/// poisoning to `Idx::ERROR` on mismatch) before each argument is checked
/// against its parameter; the aliased stdlib functions are monomorphic, so no
/// instantiation is required.
fn check_positional(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    sig: &FunctionSig,
    arg_ids: &[ExprId],
    span: Span,
) -> Idx {
    let params_len = sig.param_types.len();
    if arg_ids.len() < sig.required_params || arg_ids.len() > params_len {
        engine.push_error(TypeCheckError::arity_mismatch(
            span,
            params_len,
            arg_ids.len(),
            crate::ArityMismatchKind::Function,
        ));
        return Idx::ERROR;
    }
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

/// Check named args against the resolved signature, arity-checking the arg
/// count first (`required_params <= count <= param_types.len()`, emitting
/// `E2004` and poisoning to `Idx::ERROR` on mismatch — parity with the ordinary
/// named-call path `infer_call_named`). Each argument is matched to a parameter
/// by name; when the name is absent or matches no parameter the argument falls
/// back to its positional slot, so every argument value is checked against some
/// parameter type (no value escapes checking).
fn check_named(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    sig: &FunctionSig,
    args: &[ori_ir::CallArg],
    span: Span,
) -> Idx {
    let params_len = sig.param_types.len();
    if args.len() < sig.required_params || args.len() > params_len {
        engine.push_error(TypeCheckError::arity_mismatch(
            span,
            params_len,
            args.len(),
            crate::ArityMismatchKind::Function,
        ));
        return Idx::ERROR;
    }
    for (i, arg) in args.iter().enumerate() {
        let arg_ty = infer_expr(engine, arena, arg.value);
        let param_idx = arg
            .name
            .and_then(|n| sig.param_names.iter().position(|&p| p == n))
            .unwrap_or(i);
        if let Some(&param_ty) = sig.param_types.get(param_idx) {
            let expected = Expected {
                ty: param_ty,
                origin: ExpectedOrigin::Context {
                    span,
                    kind: ContextKind::FunctionArgument {
                        func_name: Some(sig.name),
                        arg_index: param_idx,
                        param_name: sig.param_names.get(param_idx).copied(),
                    },
                },
            };
            let _ = engine.check_type(arg_ty, &expected, arena.get_expr(arg.value).span);
        }
    }
    sig.return_type
}
