//! Block and let inference.

use ori_ir::{BindingPatternId, ExprArena, ExprId, ExprKind, Name, ParsedTypeId, Span};

use super::super::InferEngine;
use super::lambdas::maybe_generalize;
use super::{
    bind_pattern, check_expr, infer_expr, pattern_is_irrefutable, resolve_and_check_parsed_type,
    unwrap_result_or_option,
};
use crate::type_error::TypeCheckError;
use crate::{Expected, ExpectedOrigin, Idx};

/// Infer the type of a block expression.
pub(crate) fn infer_block(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    stmts: ori_ir::StmtRange,
    result: ExprId,
    _span: Span,
) -> Idx {
    // Enter binding scope for the block.
    // All let bindings within this block will be isolated from parent scope.
    engine.enter_scope();

    // Process statements
    for stmt in arena.get_stmt_range(stmts) {
        match &stmt.kind {
            ori_ir::StmtKind::Expr(expr_id) => {
                let _ = infer_expr(engine, arena, *expr_id);
            }
            ori_ir::StmtKind::Let {
                pattern,
                ty,
                init,
                mutable: _,
            } => {
                let _ =
                    infer_let_binding_core(engine, arena, *pattern, *ty, *init, false, stmt.span);
            }
        }
    }

    // Block type is the result expression type, or unit
    let block_ty = if result.is_present() {
        infer_expr(engine, arena, result)
    } else {
        Idx::UNIT
    };

    // Exit block scope - bindings are no longer visible
    engine.exit_scope();

    block_ty
}

/// Infer the type of a let expression.
pub(crate) fn infer_let(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    pattern: BindingPatternId,
    ty: ParsedTypeId,
    init: ExprId,
    // Why: Mutability is an effect, not a type property in Ori's HM inference system.
    // Enforcement happens in the evaluator (`bind_can_pattern`) and codegen backends,
    // not here. Kept as a parameter for future "cannot assign to immutable binding"
    // diagnostics (like Rust's type checker emits).
    _mutable: ori_ir::Mutability,
    span: Span,
) -> Idx {
    let _ = infer_let_binding_core(engine, arena, pattern, ty, init, false, span);
    Idx::UNIT
}

/// Core let-statement checking logic shared between block and sequence let-bindings.
pub(crate) fn infer_let_binding_core(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    pattern_id: BindingPatternId,
    ty_id: ParsedTypeId,
    init: ExprId,
    unwrap_try: bool,
    span: Span,
) -> Idx {
    let pat = arena.get_binding_pattern(pattern_id);

    // Track error count for closure self-capture detection
    let binding_name = pattern_first_name(pat);
    let errors_before = engine.error_count();

    // Why: Enter a rank scope for let-polymorphism. Pushing an env scope
    // here would hide subsequent block statements from this let binding,
    // which must stay visible to the remainder of the block.
    engine.enter_rank_scope();

    // Check/infer the initializer type based on presence of annotation
    let final_ty = if ty_id.is_valid() {
        // With type annotation: use bidirectional checking
        let parsed_ty = arena.get_parsed_type(ty_id);
        let expected_ty = resolve_and_check_parsed_type(engine, arena, parsed_ty, span);
        let expected = Expected {
            ty: expected_ty,
            origin: ExpectedOrigin::Annotation {
                name: binding_name.unwrap_or(Name::EMPTY),
                span,
            },
        };
        if unwrap_try {
            let init_ty = infer_expr(engine, arena, init);
            let unwrapped = unwrap_result_or_option(engine, init_ty);
            let _ = engine.check_type(unwrapped, &expected, span);
        } else {
            let _init_ty = check_expr(engine, arena, init, &expected, span);
        }
        expected_ty
    } else {
        // No annotation: infer and generalize for let-polymorphism
        let init_ty = infer_expr(engine, arena, init);

        // Why: Detect closure self-capture: if init is a lambda and any new
        // errors are UnknownIdent matching the binding name, rewrite them
        // to the more helpful "closure cannot capture itself" message.
        // Example: `{ let f = () -> f; f }` — f isn't yet in scope.
        if let Some(name) = binding_name {
            if matches!(arena.get_expr(init).kind, ExprKind::Lambda { .. }) {
                engine.rewrite_self_capture_errors(name, errors_before);
            }
        }

        let bound_ty = if unwrap_try {
            unwrap_result_or_option(engine, init_ty)
        } else {
            init_ty
        };

        // Spec: Value Restriction: only non-capturing lambdas are generalized.
        // Other initializers are monomorphic so E2005 surfaces on empty containers.
        // Spec: Clause 14
        maybe_generalize(engine, arena, init, bound_ty)
    };

    // Exit rank scope
    engine.exit_rank_scope();

    // Bind pattern to the block's scope.
    // The binding is visible to subsequent statements and the result.
    if let Err(reason) = pattern_is_irrefutable(engine, pat, final_ty) {
        let err = TypeCheckError::refutable_pattern(span, reason);
        engine.push_error(err);
    }
    bind_pattern(engine, arena, pat, final_ty);

    final_ty
}

/// Get the first name from a binding pattern (for error messages).
pub(crate) fn pattern_first_name(pattern: &ori_ir::BindingPattern) -> Option<Name> {
    match pattern {
        ori_ir::BindingPattern::Name { name, .. } => Some(*name),
        ori_ir::BindingPattern::Tuple(pats) => pats.first().and_then(pattern_first_name),
        ori_ir::BindingPattern::Struct { fields } => fields.first().map(|field| field.name),
        ori_ir::BindingPattern::List { elements, .. } => {
            elements.first().and_then(pattern_first_name)
        }
        ori_ir::BindingPattern::Wildcard => None,
    }
}
