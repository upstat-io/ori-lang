//! Block, let, and lambda inference.

use ori_ir::{ExprArena, ExprId, ExprKind, Name, Span};

use super::super::InferEngine;
use super::{bind_pattern, check_expr, infer_expr, resolve_and_check_parsed_type};
use crate::{ContextKind, Expected, ExpectedOrigin, Idx};

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
                let pat = arena.get_binding_pattern(*pattern);

                // Track error count for closure self-capture detection
                let binding_name = pattern_first_name(pat);
                let errors_before = engine.error_count();

                // Enter rank scope for let-polymorphism (not binding scope).
                // This allows type variables in the initializer to be generalized.
                engine.enter_rank_scope();

                // Check/infer the initializer type based on presence of annotation
                let final_ty = if ty.is_valid() {
                    // With type annotation: use bidirectional checking
                    let parsed_ty = arena.get_parsed_type(*ty);
                    let expected_ty =
                        resolve_and_check_parsed_type(engine, arena, parsed_ty, stmt.span);
                    let expected = Expected {
                        ty: expected_ty,
                        origin: ExpectedOrigin::Annotation {
                            name: pattern_first_name(pat).unwrap_or(Name::EMPTY),
                            span: stmt.span,
                        },
                    };
                    let _init_ty = check_expr(engine, arena, *init, &expected, stmt.span);
                    expected_ty
                } else {
                    // No annotation: infer and generalize for let-polymorphism
                    let init_ty = infer_expr(engine, arena, *init);

                    // Detect closure self-capture: if init is a lambda and any new
                    // errors are UnknownIdent matching the binding name, rewrite them
                    // to the more helpful "closure cannot capture itself" message.
                    // Example: `{ let f = () -> f; f }` — f isn't yet in scope.
                    if let Some(name) = binding_name {
                        if matches!(arena.get_expr(*init).kind, ExprKind::Lambda { .. }) {
                            engine.rewrite_self_capture_errors(name, errors_before);
                        }
                    }

                    engine.generalize(init_ty)
                };

                // Exit rank scope (but stay in block's binding scope)
                engine.exit_rank_scope();

                // Bind pattern to the block's scope.
                // The binding is visible to subsequent statements and the result.
                bind_pattern(engine, arena, pat, final_ty);
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
    pattern: &ori_ir::BindingPattern,
    ty_annotation: Option<&ori_ir::ParsedType>,
    init: ExprId,
    // Mutability is an effect, not a type property in Ori's HM inference system.
    // Enforcement happens in the evaluator (`bind_can_pattern`) and codegen backends,
    // not here. Kept as a parameter for future "cannot assign to immutable binding"
    // diagnostics (like Rust's type checker emits).
    _mutable: ori_ir::Mutability,
    span: Span,
) -> Idx {
    // Enter scope for let-polymorphism.
    // This increases the rank so that type variables created during
    // initializer inference can be generalized.
    engine.enter_scope();

    let binding_name = pattern_first_name(pattern);
    let errors_before = engine.error_count();

    // Check/infer the initializer type based on presence of annotation
    let final_ty = if let Some(parsed_ty) = ty_annotation {
        // With type annotation: use bidirectional checking (allows literal coercion)
        let expected_ty = resolve_and_check_parsed_type(engine, arena, parsed_ty, span);
        let expected = Expected {
            ty: expected_ty,
            origin: ExpectedOrigin::Annotation {
                name: pattern_first_name(pattern).unwrap_or(Name::EMPTY),
                span,
            },
        };
        // Use check_expr for bidirectional type checking (literal coercion)
        let _init_ty = check_expr(engine, arena, init, &expected, span);
        expected_ty
    } else {
        // No annotation: infer the initializer type
        let init_ty = infer_expr(engine, arena, init);

        // Detect closure self-capture: if the init is a lambda and any new errors
        // are UnknownIdent matching the binding name, it's a self-capture attempt.
        // Example: `let f = () -> f` — the closure body references `f`, which isn't
        // yet in scope. This would create a reference cycle under ARC.
        if let Some(name) = binding_name {
            if matches!(arena.get_expr(init).kind, ExprKind::Lambda { .. }) {
                engine.rewrite_self_capture_errors(name, errors_before);
            }
        }

        // Generalize free type variables for let-polymorphism.
        // Variables created at the current (elevated) rank will be quantified.
        engine.generalize(init_ty)
    };

    // Exit scope (rank goes back down).
    // The binding will be added to the outer environment.
    engine.exit_scope();

    // Bind the pattern to the (possibly generalized) type
    bind_pattern(engine, arena, pattern, final_ty);

    // Let expression returns unit
    Idx::UNIT
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

/// Infer the type of a lambda expression.
pub(crate) fn infer_lambda(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    params: ori_ir::ParamRange,
    ret_ty: Option<&ori_ir::ParsedType>,
    body: ExprId,
    span: Span,
) -> Idx {
    // Enter a new scope for the lambda
    engine.enter_scope();

    // Create types for parameters
    let mut param_types = Vec::new();
    for param in arena.get_params(params) {
        let param_ty = if let Some(ref parsed_ty) = param.ty {
            resolve_and_check_parsed_type(engine, arena, parsed_ty, param.span)
        } else {
            engine.fresh_var()
        };
        engine.env_mut().bind(param.name, param_ty);
        param_types.push(param_ty);
    }

    // Infer body type, checking against return annotation if present
    let body_ty = if let Some(ret_parsed) = ret_ty {
        let expected_ty = resolve_and_check_parsed_type(engine, arena, ret_parsed, span);
        let inferred = infer_expr(engine, arena, body);
        let expected = Expected {
            ty: expected_ty,
            origin: ExpectedOrigin::Context {
                span,
                kind: ContextKind::FunctionReturn { func_name: None },
            },
        };
        let _ = engine.check_type(inferred, &expected, arena.get_expr(body).span);
        expected_ty
    } else {
        infer_expr(engine, arena, body)
    };

    // Exit scope
    engine.exit_scope();

    // Create function type
    engine.infer_function(&param_types, body_ty)
}
