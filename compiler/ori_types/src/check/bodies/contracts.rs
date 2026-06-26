//! Function `pre()` / `post()` contract validation.
//!
//! Owns the pre/post-condition contract checks invoked from `check_function`
//! (Pass 2). See `bodies/mod.rs` for the architecture docstring.

use ori_ir::{ExprArena, ExprId, ExprKind, Function};

use crate::{infer_expr, Idx, InferEngine, TypeCheckError};

/// Validate every `pre()` contract on `func`.
///
/// Spec: Clause 15 (Function-Level Contracts).
///
/// Two checks per contract, in order:
///
/// 1. **Scope (E2047)** — every `Ident` in the contract expression MUST
///    resolve in the engine's environment, which carries function parameters,
///    capability bindings, and (via parent chain) module-level signatures and
///    constants. Free names emit E2047.
/// 2. **Type (E2044)** — if scope passed, infer the condition's type and
///    require `bool`. Skipped when scope check failed to avoid cascading
///    diagnostics (ER-4: follow-on suppression).
pub(super) fn validate_pre_contracts(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    func: &Function,
) {
    for contract in &func.pre_contracts {
        let scope_errors = collect_pre_contract_scope_errors(engine, arena, contract.condition);
        if !scope_errors.is_empty() {
            for err in scope_errors {
                engine.push_error(err);
            }
            continue;
        }

        let cond_ty = infer_expr(engine, arena, contract.condition);
        let resolved = engine.resolve(cond_ty);
        if resolved != Idx::BOOL && resolved != Idx::ERROR {
            engine.push_error(TypeCheckError::pre_contract_not_bool(
                contract.span,
                resolved,
            ));
        }
    }
}

/// Validate every `post()` contract on `func`.
///
/// Spec: Clause 15 (Function-Level Contracts).
///
/// `post()` is rejected when the function returns `void` (E2046) — there is
/// no result value for the post-lambda to bind. The parser guarantees every
/// `PostContract` carries a lambda-shaped binder (one or more params before
/// `->`); see `parse_post_contract_params` in `ori_parse/src/grammar/item/
/// function/mod.rs`. Non-lambda forms like `post(true)` are rejected at parse
/// time with E1002, so no E2045 emission is reachable here.
pub(super) fn validate_post_contracts(
    engine: &mut InferEngine<'_>,
    _arena: &ExprArena,
    func: &Function,
    return_ty: Idx,
) {
    let resolved_return = engine.resolve(return_ty);
    let returns_void = resolved_return == Idx::UNIT;
    for contract in &func.post_contracts {
        if returns_void {
            engine.push_error(TypeCheckError::post_contract_void_return(contract.span));
        }
    }
}

/// Walk the pre-condition expression, collecting an E2047 error for every
/// identifier that is not bound in the engine's current environment.
///
/// The walk treats `ExprKind::Ident` as the only externally-bound reference
/// shape. `Const(_)` references resolve through a separate const map and are
/// out of scope here. Compound nodes recurse via this helper; arena lookup is
/// the only side effect.
fn collect_pre_contract_scope_errors(
    engine: &InferEngine<'_>,
    arena: &ExprArena,
    expr_id: ExprId,
) -> Vec<TypeCheckError> {
    let mut errors = Vec::new();
    walk_pre_contract_expr(engine, arena, expr_id, &mut errors);
    errors
}

fn walk_pre_contract_expr(
    engine: &InferEngine<'_>,
    arena: &ExprArena,
    expr_id: ExprId,
    errors: &mut Vec<TypeCheckError>,
) {
    let expr = arena.get_expr(expr_id);
    match expr.kind {
        ExprKind::Ident(name) => {
            if engine.env().lookup(name).is_none() {
                errors.push(TypeCheckError::pre_contract_unknown_ident(expr.span, name));
            }
        }
        ExprKind::Binary { left, right, .. } => {
            walk_pre_contract_expr(engine, arena, left, errors);
            walk_pre_contract_expr(engine, arena, right, errors);
        }
        ExprKind::Unary { operand, .. } => {
            walk_pre_contract_expr(engine, arena, operand, errors);
        }
        ExprKind::Field { receiver, .. } => {
            walk_pre_contract_expr(engine, arena, receiver, errors);
        }
        ExprKind::Index { receiver, index } => {
            walk_pre_contract_expr(engine, arena, receiver, errors);
            walk_pre_contract_expr(engine, arena, index, errors);
        }
        ExprKind::Call { func, args } => {
            walk_pre_contract_expr(engine, arena, func, errors);
            for &arg_id in arena.get_expr_list(args) {
                walk_pre_contract_expr(engine, arena, arg_id, errors);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            walk_pre_contract_expr(engine, arena, receiver, errors);
            for &arg_id in arena.get_expr_list(args) {
                walk_pre_contract_expr(engine, arena, arg_id, errors);
            }
        }
        _ => {
            // Other expression shapes (literals, struct/list/map literals,
            // control-flow forms, lambdas) either contain no free identifier
            // references or introduce their own binders. The scope check is
            // intentionally narrow — surface the unknown-ident case for the
            // common shapes (comparisons, calls, field access) and let
            // `infer_expr` handle complex forms via E2003 as a fallback.
        }
    }
}
