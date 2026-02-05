//! Expression type inference for Types V2.
//!
//! This module provides expression-level type inference using the
//! `InferEngine` infrastructure. It dispatches on `ExprKind` to
//! specialized inference functions.
//!
//! # Architecture
//!
//! Expression inference follows Hindley-Milner with bidirectional enhancements:
//!
//! - **Synthesis (infer)**: Bottom-up type derivation from expression structure
//! - **Checking (check)**: Top-down verification against expected type
//!
//! The dispatch is structured to match `ori_ir::ExprKind` variants,
//! with each category delegating to specialized modules:
//!
//! - Literals → direct primitive type
//! - Identifiers → environment lookup + instantiation
//! - Operators → operator inference (binary, unary)
//! - Calls → function/method call inference
//! - Control flow → if/match/loop inference
//! - Lambdas → lambda inference with scope management
//! - Collections → list/map/tuple inference
//!
//! # Usage
//!
//! ```ignore
//! use ori_types::infer::{InferEngine, infer_expr};
//!
//! let mut pool = Pool::new();
//! let mut engine = InferEngine::new(&mut pool);
//!
//! // Infer type of expression
//! let ty = infer_expr(&mut engine, &arena, expr_id);
//! ```

use ori_ir::{BinaryOp, ExprArena, ExprId, ExprKind, Name, Span, UnaryOp};

use super::InferEngine;
use crate::{ContextKind, Expected, ExpectedOrigin, Idx, SequenceKind, Tag, TypeCheckError};

/// Infer the type of an expression.
///
/// This is the main entry point for expression type inference.
/// It dispatches to specialized handlers based on expression kind.
pub fn infer_expr(engine: &mut InferEngine<'_>, arena: &ExprArena, expr_id: ExprId) -> Idx {
    let expr = arena.get_expr(expr_id);
    let span = expr.span;

    let ty = match &expr.kind {
        // === Literals ===
        ExprKind::Int(_) | ExprKind::HashLength => Idx::INT,
        ExprKind::Float(_) => Idx::FLOAT,
        ExprKind::Bool(_) => Idx::BOOL,
        ExprKind::String(_) => Idx::STR,
        ExprKind::Char(_) => Idx::CHAR,
        ExprKind::Duration { .. } => Idx::DURATION,
        ExprKind::Size { .. } => Idx::SIZE,
        ExprKind::Unit => Idx::UNIT,

        // === Identifiers ===
        ExprKind::Ident(name) => infer_ident(engine, *name, span),
        ExprKind::FunctionRef(name) => infer_function_ref(engine, *name, span),
        ExprKind::SelfRef => infer_self_ref(engine, span),
        ExprKind::Config(name) => infer_config(engine, *name, span),

        // === Operators ===
        ExprKind::Binary { op, left, right } => {
            infer_binary(engine, arena, *op, *left, *right, span)
        }
        ExprKind::Unary { op, operand } => infer_unary(engine, arena, *op, *operand, span),

        // === Calls ===
        ExprKind::Call { func, args } => infer_call(engine, arena, *func, *args, span),
        ExprKind::CallNamed { func, args } => infer_call_named(engine, arena, *func, *args, span),
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => infer_method_call(engine, arena, *receiver, *method, *args, span),
        ExprKind::MethodCallNamed {
            receiver,
            method,
            args,
        } => infer_method_call_named(engine, arena, *receiver, *method, *args, span),

        // === Field/Index Access ===
        ExprKind::Field { receiver, field } => infer_field(engine, arena, *receiver, *field, span),
        ExprKind::Index { receiver, index } => infer_index(engine, arena, *receiver, *index, span),

        // === Control Flow ===
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => infer_if(engine, arena, *cond, *then_branch, *else_branch, span),
        ExprKind::Match { scrutinee, arms } => infer_match(engine, arena, *scrutinee, *arms, span),
        ExprKind::For {
            binding,
            iter,
            guard,
            body,
            is_yield,
        } => infer_for(
            engine, arena, *binding, *iter, *guard, *body, *is_yield, span,
        ),
        ExprKind::Loop { body } => infer_loop(engine, arena, *body, span),

        // === Blocks and Bindings ===
        ExprKind::Block { stmts, result } => infer_block(engine, arena, *stmts, *result, span),
        ExprKind::Let {
            pattern,
            ty,
            init,
            mutable,
        } => infer_let(engine, arena, pattern, ty.as_ref(), *init, *mutable, span),

        // === Lambdas ===
        ExprKind::Lambda {
            params,
            ret_ty,
            body,
        } => infer_lambda(engine, arena, *params, ret_ty.as_ref(), *body, span),

        // === Collections ===
        ExprKind::List(elements) => infer_list(engine, arena, *elements, span),
        ExprKind::ListWithSpread(elements) => infer_list_spread(engine, arena, *elements, span),
        ExprKind::Tuple(elements) => infer_tuple(engine, arena, *elements, span),
        ExprKind::Map(entries) => infer_map_literal(engine, arena, *entries, span),
        ExprKind::MapWithSpread(elements) => infer_map_spread(engine, arena, *elements, span),
        ExprKind::Range {
            start,
            end,
            step,
            inclusive,
        } => infer_range(engine, arena, *start, *end, *step, *inclusive, span),

        // === Structs ===
        ExprKind::Struct { name, fields } => infer_struct(engine, arena, *name, *fields, span),
        ExprKind::StructWithSpread { name, fields } => {
            infer_struct_spread(engine, arena, *name, *fields, span)
        }

        // === Option/Result Constructors ===
        ExprKind::Ok(inner) => infer_ok(engine, arena, *inner, span),
        ExprKind::Err(inner) => infer_err(engine, arena, *inner, span),
        ExprKind::Some(inner) => infer_some(engine, arena, *inner, span),
        ExprKind::None => infer_none(engine),

        // === Control Flow Expressions ===
        ExprKind::Break(value) => infer_break(engine, arena, *value, span),
        ExprKind::Continue(value) => infer_continue(engine, arena, *value, span),
        ExprKind::Try(inner) => infer_try(engine, arena, *inner, span),
        ExprKind::Await(inner) => infer_await(engine, arena, *inner, span),

        // === Casts and Assignment ===
        ExprKind::Cast { expr, ty, fallible } => {
            infer_cast(engine, arena, *expr, ty, *fallible, span)
        }
        ExprKind::Assign { target, value } => infer_assign(engine, arena, *target, *value, span),

        // === Capabilities ===
        ExprKind::WithCapability {
            capability,
            provider,
            body,
        } => infer_with_capability(engine, arena, *capability, *provider, *body, span),

        // === Pattern Expressions ===
        ExprKind::FunctionSeq(func_seq) => infer_function_seq(engine, arena, func_seq, span),
        ExprKind::FunctionExp(func_exp) => infer_function_exp(engine, arena, func_exp),

        // === Error ===
        ExprKind::Error => Idx::ERROR,
    };

    // Store the inferred type
    engine.store_type(expr_id.raw() as usize, ty);
    ty
}

// ============================================================================
// Identifier Inference
// ============================================================================

/// Infer the type of an identifier reference.
fn infer_ident(engine: &mut InferEngine<'_>, name: Name, span: Span) -> Idx {
    if let Some(scheme) = engine.env().lookup(name) {
        // Instantiate the type scheme with fresh variables
        engine.instantiate(scheme)
    } else {
        // Unknown identifier - report error
        engine.push_error(TypeCheckError::undefined_identifier(name, span));
        Idx::ERROR
    }
}

/// Infer the type of a function reference (@name).
fn infer_function_ref(engine: &mut InferEngine<'_>, name: Name, span: Span) -> Idx {
    // Function references are looked up the same way as identifiers
    // but may have special handling for capability tracking
    infer_ident(engine, name, span)
}

/// Infer the type of self reference.
fn infer_self_ref(engine: &mut InferEngine<'_>, span: Span) -> Idx {
    // Self type should be tracked in scope context
    // For now, report an error if used outside impl
    engine.push_error(TypeCheckError::self_outside_impl(span));
    Idx::ERROR
}

/// Infer the type of a config reference ($name).
fn infer_config(engine: &mut InferEngine<'_>, name: Name, span: Span) -> Idx {
    // Config values should be tracked in scope context
    // For now, report an error
    engine.push_error(TypeCheckError::undefined_config(name, span));
    Idx::ERROR
}

// ============================================================================
// Operator Inference
// ============================================================================

/// Infer the type of a binary operation.
fn infer_binary(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    op: BinaryOp,
    left: ExprId,
    right: ExprId,
    span: Span,
) -> Idx {
    let left_ty = infer_expr(engine, arena, left);
    let right_ty = infer_expr(engine, arena, right);
    let op_str = op.as_symbol();

    match op {
        // Arithmetic: same type in, same type out
        BinaryOp::Add
        | BinaryOp::Sub
        | BinaryOp::Mul
        | BinaryOp::Div
        | BinaryOp::Mod
        | BinaryOp::FloorDiv => {
            // Unify left and right operands
            engine.push_context(ContextKind::BinaryOpRight { op: op_str });
            let left_span = arena.get_expr(left).span;
            let expected = Expected {
                ty: left_ty,
                origin: ExpectedOrigin::Context {
                    span: left_span,
                    kind: ContextKind::BinaryOpLeft { op: op_str },
                },
            };
            let _ = engine.check_type(right_ty, &expected, arena.get_expr(right).span);
            engine.pop_context();

            // Result type is the left operand type (after unification)
            engine.resolve(left_ty)
        }

        // Comparison: same type in, bool out
        BinaryOp::Eq
        | BinaryOp::NotEq
        | BinaryOp::Lt
        | BinaryOp::LtEq
        | BinaryOp::Gt
        | BinaryOp::GtEq => {
            // Unify left and right operands
            engine.push_context(ContextKind::ComparisonRight);
            let left_span = arena.get_expr(left).span;
            let expected = Expected {
                ty: left_ty,
                origin: ExpectedOrigin::Context {
                    span: left_span,
                    kind: ContextKind::ComparisonLeft,
                },
            };
            let _ = engine.check_type(right_ty, &expected, arena.get_expr(right).span);
            engine.pop_context();

            Idx::BOOL
        }

        // Boolean: bool in, bool out
        BinaryOp::And | BinaryOp::Or => {
            let left_span = arena.get_expr(left).span;
            let right_span = arena.get_expr(right).span;

            // Check left is bool
            engine.push_context(ContextKind::BinaryOpLeft { op: op_str });
            let bool_expected = Expected {
                ty: Idx::BOOL,
                origin: ExpectedOrigin::NoExpectation,
            };
            let _ = engine.check_type(left_ty, &bool_expected, left_span);
            engine.pop_context();

            // Check right is bool
            engine.push_context(ContextKind::BinaryOpRight { op: op_str });
            let _ = engine.check_type(right_ty, &bool_expected, right_span);
            engine.pop_context();

            Idx::BOOL
        }

        // Bitwise operations: same integer types
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr => {
            // Unify left and right operands
            engine.push_context(ContextKind::BinaryOpRight { op: op_str });
            let left_span = arena.get_expr(left).span;
            let expected = Expected {
                ty: left_ty,
                origin: ExpectedOrigin::Context {
                    span: left_span,
                    kind: ContextKind::BinaryOpLeft { op: op_str },
                },
            };
            let _ = engine.check_type(right_ty, &expected, arena.get_expr(right).span);
            engine.pop_context();

            // Result type is the left operand type
            engine.resolve(left_ty)
        }

        // Range creation
        BinaryOp::Range | BinaryOp::RangeInclusive => {
            // Both operands should be the same type (typically int)
            let left_span = arena.get_expr(left).span;
            let expected = Expected {
                ty: left_ty,
                origin: ExpectedOrigin::Context {
                    span: left_span,
                    kind: ContextKind::RangeStart,
                },
            };
            let _ = engine.check_type(right_ty, &expected, arena.get_expr(right).span);

            // Return Range<T>
            let elem_ty = engine.resolve(left_ty);
            engine.pool_mut().range(elem_ty)
        }

        // Coalesce: Option<T> ?? T -> T
        BinaryOp::Coalesce => {
            // Left should be Option<T>, right should be T
            let resolved_left = engine.resolve(left_ty);
            if engine.pool().tag(resolved_left) == Tag::Option {
                let inner = engine.pool().option_inner(resolved_left);
                let _ = engine.unify_types(inner, right_ty);
                engine.resolve(inner)
            } else {
                engine.push_error(TypeCheckError::coalesce_requires_option(span));
                Idx::ERROR
            }
        }
    }
}

/// Infer the type of a unary operation.
fn infer_unary(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    op: UnaryOp,
    operand: ExprId,
    span: Span,
) -> Idx {
    let operand_ty = infer_expr(engine, arena, operand);
    let operand_span = arena.get_expr(operand).span;

    match op {
        // Negation: numeric -> numeric
        UnaryOp::Neg => {
            // operand must be numeric
            let resolved = engine.resolve(operand_ty);
            let tag = engine.pool().tag(resolved);
            if tag == Tag::Int || tag == Tag::Float {
                resolved
            } else {
                engine.push_error(TypeCheckError::negation_requires_numeric(operand_span));
                Idx::ERROR
            }
        }

        // Logical not: bool -> bool
        UnaryOp::Not => {
            engine.push_context(ContextKind::UnaryOpOperand { op: "!" });
            let expected = Expected {
                ty: Idx::BOOL,
                origin: ExpectedOrigin::NoExpectation,
            };
            let _ = engine.check_type(operand_ty, &expected, operand_span);
            engine.pop_context();
            Idx::BOOL
        }

        // Bitwise not: int -> int
        UnaryOp::BitNot => {
            engine.push_context(ContextKind::UnaryOpOperand { op: "~" });
            let expected = Expected {
                ty: Idx::INT,
                origin: ExpectedOrigin::NoExpectation,
            };
            let _ = engine.check_type(operand_ty, &expected, operand_span);
            engine.pop_context();
            Idx::INT
        }

        // Try operator: Option<T> -> T or Result<T, E> -> T
        UnaryOp::Try => {
            let resolved = engine.resolve(operand_ty);
            let tag = engine.pool().tag(resolved);

            match tag {
                Tag::Option => engine.pool().option_inner(resolved),
                Tag::Result => engine.pool().result_ok(resolved),
                _ => {
                    engine.push_error(TypeCheckError::try_requires_option_or_result(
                        span, resolved,
                    ));
                    Idx::ERROR
                }
            }
        }
    }
}

// ============================================================================
// Control Flow Inference
// ============================================================================

/// Infer the type of an if expression.
fn infer_if(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    cond: ExprId,
    then_branch: ExprId,
    else_branch: Option<ExprId>,
    _span: Span,
) -> Idx {
    // Condition must be bool
    let cond_ty = infer_expr(engine, arena, cond);
    engine.push_context(ContextKind::IfCondition);
    let expected = Expected {
        ty: Idx::BOOL,
        origin: ExpectedOrigin::NoExpectation,
    };
    let _ = engine.check_type(cond_ty, &expected, arena.get_expr(cond).span);
    engine.pop_context();

    // Infer then branch
    engine.push_context(ContextKind::IfThenBranch);
    let then_ty = infer_expr(engine, arena, then_branch);
    engine.pop_context();

    if let Some(else_id) = else_branch {
        // Else branch must match then branch
        engine.push_context(ContextKind::IfElseBranch { branch_index: 0 });
        let then_span = arena.get_expr(then_branch).span;
        let expected = Expected {
            ty: then_ty,
            origin: ExpectedOrigin::PreviousInSequence {
                previous_span: then_span,
                current_index: 1,
                sequence_kind: SequenceKind::IfBranches,
            },
        };
        let else_ty = infer_expr(engine, arena, else_id);
        let _ = engine.check_type(else_ty, &expected, arena.get_expr(else_id).span);
        engine.pop_context();

        engine.resolve(then_ty)
    } else {
        // No else: if without else has type unit
        // (unless then_branch has type unit or never)
        let resolved_then = engine.resolve(then_ty);
        if resolved_then == Idx::UNIT || resolved_then == Idx::NEVER {
            Idx::UNIT
        } else {
            // Warning: if without else where then is not unit
            // For now, just return unit
            Idx::UNIT
        }
    }
}

// ============================================================================
// Match Expression Inference
// ============================================================================

/// Infer the type of a match expression.
///
/// Match inference follows these steps:
/// 1. Infer the scrutinee type
/// 2. For each arm: check pattern against scrutinee, check guard is bool, infer body
/// 3. Unify all arm body types
/// 4. Return the unified type (or never if no arms)
fn infer_match(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    scrutinee: ExprId,
    arms: ori_ir::ArmRange,
    span: Span,
) -> Idx {
    // Step 1: Infer scrutinee type
    engine.push_context(ContextKind::MatchScrutinee);
    let scrutinee_ty = infer_expr(engine, arena, scrutinee);
    engine.pop_context();

    let arms_slice = arena.get_arms(arms);

    // Empty match returns never (vacuously true that all branches agree)
    if arms_slice.is_empty() {
        return Idx::NEVER;
    }

    // Step 2 & 3: Process arms and unify body types
    let mut result_ty: Option<Idx> = None;
    let scrutinee_span = arena.get_expr(scrutinee).span;

    for (i, arm) in arms_slice.iter().enumerate() {
        // Check pattern against scrutinee type (and bind variables)
        engine.push_context(ContextKind::MatchArmPattern { arm_index: i });
        check_match_pattern(engine, arena, &arm.pattern, scrutinee_ty);
        engine.pop_context();

        // Check guard is bool (if present)
        if let Some(guard_id) = arm.guard {
            engine.push_context(ContextKind::MatchArmGuard { arm_index: i });
            let guard_ty = infer_expr(engine, arena, guard_id);
            let expected = Expected {
                ty: Idx::BOOL,
                origin: ExpectedOrigin::Context {
                    span: arena.get_expr(guard_id).span,
                    kind: ContextKind::MatchArmGuard { arm_index: i },
                },
            };
            let _ = engine.check_type(guard_ty, &expected, arena.get_expr(guard_id).span);
            engine.pop_context();
        }

        // Infer body type
        engine.push_context(ContextKind::MatchArm { arm_index: i });
        let body_ty = infer_expr(engine, arena, arm.body);
        engine.pop_context();

        // Unify with previous arms
        match result_ty {
            None => {
                // First arm establishes the result type
                result_ty = Some(body_ty);
            }
            Some(prev_ty) => {
                // Subsequent arms must match the first
                let expected = Expected {
                    ty: prev_ty,
                    origin: ExpectedOrigin::PreviousInSequence {
                        previous_span: scrutinee_span,
                        current_index: i,
                        sequence_kind: SequenceKind::MatchArms,
                    },
                };
                let _ = engine.check_type(body_ty, &expected, arena.get_expr(arm.body).span);
            }
        }

        // Exit pattern bindings scope (patterns introduce local bindings)
        // Note: Variables bound in patterns are only visible in that arm's body
        // This is handled by enter/exit scope around pattern checking
    }

    // Return the unified type, or error if something went wrong
    if let Some(ty) = result_ty {
        engine.resolve(ty)
    } else {
        engine.push_error(TypeCheckError::arity_mismatch(
            span,
            1,
            0,
            crate::ArityMismatchKind::Pattern,
        ));
        Idx::ERROR
    }
}

/// Check a match pattern against an expected type, binding variables in the environment.
///
/// This function validates that a pattern can match values of the given type,
/// and binds any variable names introduced by the pattern.
fn check_match_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    pattern: &ori_ir::MatchPattern,
    expected_ty: Idx,
) {
    use ori_ir::MatchPattern;

    match pattern {
        // Wildcard matches anything
        MatchPattern::Wildcard => {}

        // Binding introduces a variable with the expected type
        MatchPattern::Binding(name) => {
            engine.env_mut().bind(*name, expected_ty);
        }

        // Literal must have compatible type
        MatchPattern::Literal(expr_id) => {
            let lit_ty = infer_expr(engine, arena, *expr_id);
            let _ = engine.unify_types(lit_ty, expected_ty);
        }

        // Variant pattern: extract inner type if it's an enum/Option/Result
        MatchPattern::Variant { name, inner } => {
            let resolved = engine.resolve(expected_ty);
            let tag = engine.pool().tag(resolved);

            // Handle known container types
            let inner_types = match tag {
                Tag::Option => {
                    // Some(x) pattern - inner has one element with inner type
                    vec![engine.pool().option_inner(resolved)]
                }
                Tag::Result => {
                    // Ok(x) or Err(e) pattern - check which variant
                    // For now, assume we can infer from context
                    // TODO: Use name to determine Ok vs Err
                    let _ = name; // Will be used for variant discrimination
                    vec![engine.pool().result_ok(resolved)]
                }
                _ => {
                    // User-defined enum - needs registry lookup (Section 07)
                    // For now, create fresh variables for inner patterns
                    let inner_ids = arena.get_match_pattern_list(*inner);
                    inner_ids.iter().map(|_| engine.fresh_var()).collect()
                }
            };

            // Check inner patterns
            let inner_ids = arena.get_match_pattern_list(*inner);
            for (inner_id, inner_ty) in inner_ids.iter().zip(inner_types.iter()) {
                let inner_pattern = arena.get_match_pattern(*inner_id);
                check_match_pattern(engine, arena, inner_pattern, *inner_ty);
            }
        }

        // Tuple pattern: check each element
        MatchPattern::Tuple(inner) => {
            let resolved = engine.resolve(expected_ty);

            if engine.pool().tag(resolved) == Tag::Tuple {
                let elem_types = engine.pool().tuple_elems(resolved);
                let inner_ids = arena.get_match_pattern_list(*inner);

                // Check arity
                if inner_ids.len() != elem_types.len() {
                    engine.push_error(TypeCheckError::arity_mismatch(
                        Span::DUMMY,
                        elem_types.len(),
                        inner_ids.len(),
                        crate::ArityMismatchKind::Pattern,
                    ));
                    return;
                }

                // Check each element
                for (inner_id, elem_ty) in inner_ids.iter().zip(elem_types.iter()) {
                    let inner_pattern = arena.get_match_pattern(*inner_id);
                    check_match_pattern(engine, arena, inner_pattern, *elem_ty);
                }
            } else if resolved != Idx::ERROR {
                // Not a tuple type
                engine.push_error(TypeCheckError::mismatch(
                    Span::DUMMY,
                    expected_ty,
                    resolved,
                    vec![],
                    crate::ErrorContext::new(ContextKind::PatternMatch {
                        pattern_kind: "tuple",
                    }),
                ));
            }
        }

        // List pattern: check elements and rest
        MatchPattern::List { elements, rest } => {
            let resolved = engine.resolve(expected_ty);

            if engine.pool().tag(resolved) == Tag::List {
                let elem_ty = engine.pool().list_elem(resolved);
                let elem_ids = arena.get_match_pattern_list(*elements);

                // Check each element pattern
                for inner_id in elem_ids {
                    let inner_pattern = arena.get_match_pattern(*inner_id);
                    check_match_pattern(engine, arena, inner_pattern, elem_ty);
                }

                // Bind rest pattern to list type
                if let Some(rest_name) = rest {
                    engine.env_mut().bind(*rest_name, resolved);
                }
            } else if resolved != Idx::ERROR {
                // Not a list type
                engine.push_error(TypeCheckError::mismatch(
                    Span::DUMMY,
                    expected_ty,
                    resolved,
                    vec![],
                    crate::ErrorContext::new(ContextKind::PatternMatch {
                        pattern_kind: "list",
                    }),
                ));
            }
        }

        // Struct pattern: check field types (needs registry - Section 07)
        MatchPattern::Struct { fields } => {
            // For now, bind field names to fresh variables
            // Full implementation needs struct registry
            for (name, inner_pattern) in fields {
                let field_ty = engine.fresh_var();
                if let Some(inner_id) = inner_pattern {
                    let inner = arena.get_match_pattern(*inner_id);
                    check_match_pattern(engine, arena, inner, field_ty);
                } else {
                    // Shorthand: `{ x }` binds x to the field value
                    engine.env_mut().bind(*name, field_ty);
                }
            }
        }

        // Range pattern: check bounds
        MatchPattern::Range { start, end, .. } => {
            if let Some(start_id) = start {
                let start_ty = infer_expr(engine, arena, *start_id);
                let _ = engine.unify_types(start_ty, expected_ty);
            }
            if let Some(end_id) = end {
                let end_ty = infer_expr(engine, arena, *end_id);
                let _ = engine.unify_types(end_ty, expected_ty);
            }
        }

        // Or pattern: all alternatives must match the same type
        MatchPattern::Or(alternatives) => {
            let alt_ids = arena.get_match_pattern_list(*alternatives);
            for alt_id in alt_ids {
                let alt_pattern = arena.get_match_pattern(*alt_id);
                check_match_pattern(engine, arena, alt_pattern, expected_ty);
            }
        }

        // At pattern: bind name and check inner pattern
        MatchPattern::At {
            name,
            pattern: inner_id,
        } => {
            engine.env_mut().bind(*name, expected_ty);
            let inner_pattern = arena.get_match_pattern(*inner_id);
            check_match_pattern(engine, arena, inner_pattern, expected_ty);
        }
    }
}

// ============================================================================
// Loop Inference
// ============================================================================

/// Infer the type of a for loop.
///
/// For loops in Ori can be used in two forms:
/// - `for x in iter do body` - returns unit, iterates for side effects
/// - `for x in iter yield body` - returns a list, collects body results
///
/// The iterator must be iterable (list, range, etc.), and the binding
/// receives each element type.
// TODO(inference): Refactor with a ForLoopParams struct when implementing
#[expect(clippy::too_many_arguments, reason = "matches ExprKind::For structure")]
fn infer_for(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    binding: Name,
    iter: ExprId,
    guard: Option<ExprId>,
    body: ExprId,
    is_yield: bool,
    _span: Span,
) -> Idx {
    // Enter scope for loop binding
    engine.enter_scope();

    // Infer iterator type
    engine.push_context(ContextKind::ForIterator);
    let iter_ty = infer_expr(engine, arena, iter);
    engine.pop_context();

    // Extract element type from iterator
    let resolved_iter = engine.resolve(iter_ty);
    let tag = engine.pool().tag(resolved_iter);

    let elem_ty = match tag {
        Tag::List => engine.pool().list_elem(resolved_iter),
        Tag::Range => engine.pool().range_elem(resolved_iter),
        Tag::Map => {
            // Iterating over a map yields (key, value) tuples
            let key_ty = engine.pool().map_key(resolved_iter);
            let value_ty = engine.pool().map_value(resolved_iter);
            engine.pool_mut().tuple(&[key_ty, value_ty])
        }
        Tag::Set => {
            // Sets store elements similarly to lists (single type parameter)
            engine.pool().set_elem(resolved_iter)
        }
        _ => {
            // Not a known iterable - still allow iteration with fresh element type
            // The type checker will catch concrete type mismatches later
            engine.fresh_var()
        }
    };

    // Bind the loop variable
    engine.push_context(ContextKind::ForBinding);
    engine.env_mut().bind(binding, elem_ty);
    engine.pop_context();

    // Check guard if present (must be bool)
    if let Some(guard_id) = guard {
        let guard_ty = infer_expr(engine, arena, guard_id);
        let expected = Expected {
            ty: Idx::BOOL,
            origin: ExpectedOrigin::Context {
                span: arena.get_expr(guard_id).span,
                kind: ContextKind::LoopCondition,
            },
        };
        let _ = engine.check_type(guard_ty, &expected, arena.get_expr(guard_id).span);
    }

    // Infer body type
    engine.push_context(ContextKind::LoopBody);
    let body_ty = infer_expr(engine, arena, body);
    engine.pop_context();

    // Exit loop scope
    engine.exit_scope();

    // Return type depends on do vs yield
    if is_yield {
        // yield: collect results into a list
        let resolved_body = engine.resolve(body_ty);
        engine.pool_mut().list(resolved_body)
    } else {
        // do: iterate for side effects, return unit
        Idx::UNIT
    }
}

/// Infer the type of an infinite loop.
///
/// `loop { body }` runs the body repeatedly until a `break` is encountered.
/// The loop type is determined by break expressions within the body:
/// - If breaks have values, the loop returns that type
/// - If no breaks, the loop returns `never` (runs forever)
fn infer_loop(engine: &mut InferEngine<'_>, arena: &ExprArena, body: ExprId, _span: Span) -> Idx {
    // Enter scope for loop
    engine.enter_scope();

    // Infer body type (usually unit, break determines actual return)
    engine.push_context(ContextKind::LoopBody);
    let _body_ty = infer_expr(engine, arena, body);
    engine.pop_context();

    // Exit loop scope
    engine.exit_scope();

    // Infinite loop without break tracking returns never
    // TODO: Track break values to determine actual return type
    // For now, return unit (most common case when breaks are present)
    Idx::UNIT
}

fn infer_block(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    stmts: ori_ir::StmtRange,
    result: Option<ExprId>,
    _span: Span,
) -> Idx {
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
                mutable,
            } => {
                // Infer initializer
                let init_ty = infer_expr(engine, arena, *init);

                // TODO: Check against type annotation if present
                let _ = ty;
                let _ = mutable;

                // Bind pattern to type
                if let ori_ir::BindingPattern::Name(name) = pattern {
                    engine.env_mut().bind(*name, init_ty);
                }
                // TODO: Handle complex patterns
            }
        }
    }

    // Block type is the result expression type, or unit
    match result {
        Some(result_id) => infer_expr(engine, arena, result_id),
        None => Idx::UNIT,
    }
}

fn infer_let(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    pattern: &ori_ir::BindingPattern,
    ty_annotation: Option<&ori_ir::ParsedType>,
    init: ExprId,
    _mutable: bool,
    _span: Span,
) -> Idx {
    // Infer the initializer type
    let init_ty = infer_expr(engine, arena, init);

    // If there's a type annotation, check against it
    if let Some(_parsed_ty) = ty_annotation {
        // TODO: Convert ParsedType to Idx and check
    }

    // Bind the pattern to the type
    // For simple patterns, just bind the name
    if let ori_ir::BindingPattern::Name(name) = pattern {
        engine.env_mut().bind(*name, init_ty);
    }
    // TODO: Handle complex patterns (tuple, struct destructuring)

    // Let expression returns unit
    Idx::UNIT
}

fn infer_lambda(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    params: ori_ir::ParamRange,
    ret_ty: Option<&ori_ir::ParsedType>,
    body: ExprId,
    _span: Span,
) -> Idx {
    // Enter a new scope for the lambda
    engine.enter_scope();

    // Create types for parameters
    let mut param_types = Vec::new();
    for param in arena.get_params(params) {
        let param_ty = if param.ty.is_some() {
            // TODO: Convert ParsedType to Idx
            engine.fresh_var()
        } else {
            engine.fresh_var()
        };
        engine.env_mut().bind(param.name, param_ty);
        param_types.push(param_ty);
    }

    // Infer body type
    let body_ty = if let Some(_ret) = ret_ty {
        // TODO: Check body against return type annotation
        infer_expr(engine, arena, body)
    } else {
        infer_expr(engine, arena, body)
    };

    // Exit scope
    engine.exit_scope();

    // Create function type
    engine.infer_function(&param_types, body_ty)
}

// Collection inference stubs
fn infer_list(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    elements: ori_ir::ExprList,
    _span: Span,
) -> Idx {
    let elem_ids: Vec<_> = arena.iter_expr_list(elements).collect();

    if elem_ids.is_empty() {
        return engine.infer_empty_list();
    }

    // Infer first element
    let first_ty = infer_expr(engine, arena, elem_ids[0]);
    let first_span = arena.get_expr(elem_ids[0]).span;

    // Check remaining elements
    for (i, &elem_id) in elem_ids.iter().skip(1).enumerate() {
        let expected = Expected {
            ty: first_ty,
            origin: ExpectedOrigin::PreviousInSequence {
                previous_span: first_span,
                current_index: i + 1,
                sequence_kind: SequenceKind::ListLiteral,
            },
        };
        let elem_ty = infer_expr(engine, arena, elem_id);
        let _ = engine.check_type(elem_ty, &expected, arena.get_expr(elem_id).span);
    }

    let resolved_elem = engine.resolve(first_ty);
    engine.infer_list(resolved_elem)
}

fn infer_list_spread(
    _engine: &mut InferEngine<'_>,
    _arena: &ExprArena,
    _elements: ori_ir::ListElementRange,
    _span: Span,
) -> Idx {
    // TODO: Implement list spread inference
    Idx::ERROR
}

fn infer_tuple(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    elements: ori_ir::ExprList,
    _span: Span,
) -> Idx {
    let elem_ids: Vec<_> = arena.iter_expr_list(elements).collect();
    let elem_types: Vec<_> = elem_ids
        .iter()
        .map(|&id| infer_expr(engine, arena, id))
        .collect();
    engine.infer_tuple(&elem_types)
}

fn infer_map_literal(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    entries: ori_ir::MapEntryRange,
    _span: Span,
) -> Idx {
    let entries_slice = arena.get_map_entries(entries);

    if entries_slice.is_empty() {
        return engine.infer_empty_map();
    }

    // Infer first entry
    let first_entry = &entries_slice[0];
    let first_key_ty = infer_expr(engine, arena, first_entry.key);
    let first_val_ty = infer_expr(engine, arena, first_entry.value);

    // Check remaining entries
    for entry in entries_slice.iter().skip(1) {
        let key_ty = infer_expr(engine, arena, entry.key);
        let val_ty = infer_expr(engine, arena, entry.value);
        let _ = engine.unify_types(key_ty, first_key_ty);
        let _ = engine.unify_types(val_ty, first_val_ty);
    }

    let resolved_key = engine.resolve(first_key_ty);
    let resolved_val = engine.resolve(first_val_ty);
    engine.infer_map(resolved_key, resolved_val)
}

fn infer_map_spread(
    _engine: &mut InferEngine<'_>,
    _arena: &ExprArena,
    _elements: ori_ir::MapElementRange,
    _span: Span,
) -> Idx {
    // TODO: Implement map spread inference
    Idx::ERROR
}

fn infer_range(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    start: Option<ExprId>,
    end: Option<ExprId>,
    step: Option<ExprId>,
    _inclusive: bool,
    _span: Span,
) -> Idx {
    // Determine element type from provided bounds
    let elem_ty = if let Some(start_id) = start {
        infer_expr(engine, arena, start_id)
    } else if let Some(end_id) = end {
        infer_expr(engine, arena, end_id)
    } else {
        Idx::INT // Default to int for open ranges
    };

    // Unify all provided bounds
    if let Some(start_id) = start {
        let ty = infer_expr(engine, arena, start_id);
        let _ = engine.unify_types(ty, elem_ty);
    }
    if let Some(end_id) = end {
        let ty = infer_expr(engine, arena, end_id);
        let _ = engine.unify_types(ty, elem_ty);
    }
    if let Some(step_id) = step {
        let ty = infer_expr(engine, arena, step_id);
        let _ = engine.unify_types(ty, elem_ty);
    }

    let resolved = engine.resolve(elem_ty);
    engine.pool_mut().range(resolved)
}

// Struct inference stubs
fn infer_struct(
    _engine: &mut InferEngine<'_>,
    _arena: &ExprArena,
    _name: Name,
    _fields: ori_ir::FieldInitRange,
    _span: Span,
) -> Idx {
    // TODO: Implement struct inference
    Idx::ERROR
}

fn infer_struct_spread(
    _engine: &mut InferEngine<'_>,
    _arena: &ExprArena,
    _name: Name,
    _fields: ori_ir::StructLitFieldRange,
    _span: Span,
) -> Idx {
    // TODO: Implement struct spread inference
    Idx::ERROR
}

// Option/Result constructors
fn infer_ok(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    inner: Option<ExprId>,
    _span: Span,
) -> Idx {
    let ok_ty = match inner {
        Some(id) => infer_expr(engine, arena, id),
        None => Idx::UNIT,
    };
    let err_ty = engine.fresh_var();
    engine.infer_result(ok_ty, err_ty)
}

fn infer_err(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    inner: Option<ExprId>,
    _span: Span,
) -> Idx {
    let err_ty = match inner {
        Some(id) => infer_expr(engine, arena, id),
        None => Idx::UNIT,
    };
    let ok_ty = engine.fresh_var();
    engine.infer_result(ok_ty, err_ty)
}

fn infer_some(engine: &mut InferEngine<'_>, arena: &ExprArena, inner: ExprId, _span: Span) -> Idx {
    let inner_ty = infer_expr(engine, arena, inner);
    engine.infer_option(inner_ty)
}

fn infer_none(engine: &mut InferEngine<'_>) -> Idx {
    let inner_ty = engine.fresh_var();
    engine.infer_option(inner_ty)
}

// Control flow expression stubs
fn infer_break(
    _engine: &mut InferEngine<'_>,
    _arena: &ExprArena,
    _value: Option<ExprId>,
    _span: Span,
) -> Idx {
    Idx::NEVER
}

fn infer_continue(
    _engine: &mut InferEngine<'_>,
    _arena: &ExprArena,
    _value: Option<ExprId>,
    _span: Span,
) -> Idx {
    Idx::NEVER
}

fn infer_try(engine: &mut InferEngine<'_>, arena: &ExprArena, inner: ExprId, span: Span) -> Idx {
    let inner_ty = infer_expr(engine, arena, inner);
    let resolved = engine.resolve(inner_ty);
    let tag = engine.pool().tag(resolved);

    match tag {
        Tag::Option => {
            // Option<T>? -> T (propagates None)
            engine.pool().option_inner(resolved)
        }
        Tag::Result => {
            // Result<T, E>? -> T (propagates Err)
            engine.pool().result_ok(resolved)
        }
        _ => {
            engine.push_error(TypeCheckError::try_requires_option_or_result(
                span, resolved,
            ));
            Idx::ERROR
        }
    }
}

fn infer_await(
    _engine: &mut InferEngine<'_>,
    _arena: &ExprArena,
    _inner: ExprId,
    _span: Span,
) -> Idx {
    // TODO: Implement await inference
    Idx::ERROR
}

fn infer_cast(
    _engine: &mut InferEngine<'_>,
    _arena: &ExprArena,
    _expr: ExprId,
    _ty: &ori_ir::ParsedType,
    _fallible: bool,
    _span: Span,
) -> Idx {
    // TODO: Implement cast inference
    Idx::ERROR
}

fn infer_assign(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    target: ExprId,
    value: ExprId,
    _span: Span,
) -> Idx {
    let target_ty = infer_expr(engine, arena, target);
    let value_ty = infer_expr(engine, arena, value);

    let expected = Expected {
        ty: target_ty,
        origin: ExpectedOrigin::Context {
            span: arena.get_expr(target).span,
            kind: ContextKind::Assignment,
        },
    };
    let _ = engine.check_type(value_ty, &expected, arena.get_expr(value).span);

    Idx::UNIT
}

fn infer_with_capability(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    _capability: Name,
    provider: ExprId,
    body: ExprId,
    _span: Span,
) -> Idx {
    // Infer provider type (for validation)
    let _ = infer_expr(engine, arena, provider);

    // TODO: Track capability provision for propagation checking

    // Expression type is the body type
    infer_expr(engine, arena, body)
}

// Call inference stubs
fn infer_call(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    func: ExprId,
    args: ori_ir::ExprList,
    span: Span,
) -> Idx {
    let func_ty = infer_expr(engine, arena, func);
    let resolved = engine.resolve(func_ty);

    if engine.pool().tag(resolved) != Tag::Function {
        if resolved != Idx::ERROR {
            engine.push_error(TypeCheckError::not_callable(span, resolved));
        }
        return Idx::ERROR;
    }

    let params = engine.pool().function_params(resolved);
    let ret = engine.pool().function_return(resolved);

    let arg_ids: Vec<_> = arena.iter_expr_list(args).collect();

    // Check arity
    if arg_ids.len() != params.len() {
        engine.push_error(TypeCheckError::arity_mismatch(
            span,
            params.len(),
            arg_ids.len(),
            crate::ArityMismatchKind::Function,
        ));
        return Idx::ERROR;
    }

    // Check each argument
    for (i, (&arg_id, &param_ty)) in arg_ids.iter().zip(params.iter()).enumerate() {
        let expected = Expected {
            ty: param_ty,
            origin: ExpectedOrigin::Context {
                span: arena.get_expr(func).span,
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

    ret
}

fn infer_call_named(
    _engine: &mut InferEngine<'_>,
    _arena: &ExprArena,
    _func: ExprId,
    _args: ori_ir::CallArgRange,
    _span: Span,
) -> Idx {
    // TODO: Implement named call inference
    Idx::ERROR
}

fn infer_method_call(
    _engine: &mut InferEngine<'_>,
    _arena: &ExprArena,
    _receiver: ExprId,
    _method: Name,
    _args: ori_ir::ExprList,
    _span: Span,
) -> Idx {
    // TODO: Implement method call inference
    Idx::ERROR
}

fn infer_method_call_named(
    _engine: &mut InferEngine<'_>,
    _arena: &ExprArena,
    _receiver: ExprId,
    _method: Name,
    _args: ori_ir::CallArgRange,
    _span: Span,
) -> Idx {
    // TODO: Implement named method call inference
    Idx::ERROR
}

fn infer_field(
    _engine: &mut InferEngine<'_>,
    _arena: &ExprArena,
    _receiver: ExprId,
    _field: Name,
    _span: Span,
) -> Idx {
    // TODO: Implement field access inference
    Idx::ERROR
}

fn infer_index(
    _engine: &mut InferEngine<'_>,
    _arena: &ExprArena,
    _receiver: ExprId,
    _index: ExprId,
    _span: Span,
) -> Idx {
    // TODO: Implement index access inference
    Idx::ERROR
}

/// Infer type for a `function_seq` expression (run, try, match, for).
///
/// `FunctionSeq` represents sequential expressions where order matters:
/// - **Run**: `run(let x = a, let y = b, result)` - sequential bindings
/// - **Try**: `try(let x = fallible()?, result)` - auto-unwrap `Result`/`Option`
/// - **Match**: `match(scrutinee, Pattern -> expr, ...)` - pattern matching
/// - **`ForPattern`**: `for(over: items, match: Pattern -> expr, default: fallback)`
fn infer_function_seq(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    func_seq: &ori_ir::FunctionSeq,
    span: Span,
) -> Idx {
    use ori_ir::FunctionSeq;

    match func_seq {
        FunctionSeq::Run {
            bindings, result, ..
        } => infer_run_seq(engine, arena, *bindings, *result),

        FunctionSeq::Try {
            bindings, result, ..
        } => infer_try_seq(engine, arena, *bindings, *result, span),

        FunctionSeq::Match {
            scrutinee,
            arms,
            span: match_span,
        } => {
            // Delegate to existing match inference
            infer_match(engine, arena, *scrutinee, *arms, *match_span)
        }

        FunctionSeq::ForPattern {
            over,
            map,
            arm,
            default,
            ..
        } => infer_for_pattern(engine, arena, *over, *map, arm, *default, span),
    }
}

/// Infer type for `run(let x = a, let y = b, result)`.
///
/// Creates a new scope, processes bindings sequentially, and returns the result type.
fn infer_run_seq(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    bindings: ori_ir::SeqBindingRange,
    result: ExprId,
) -> Idx {
    // Enter a new scope for the run block
    engine.enter_scope();

    // Process each binding in sequence
    let seq_bindings = arena.get_seq_bindings(bindings);
    for binding in seq_bindings {
        infer_seq_binding(engine, arena, binding, false);
    }

    // Infer the result expression
    let result_ty = infer_expr(engine, arena, result);

    // Exit scope
    engine.exit_scope();

    result_ty
}

/// Infer type for `try(let x = fallible()?, result)`.
///
/// Like run, but auto-unwraps Result/Option types in let bindings.
/// The entire expression returns a Result or Option wrapping the result.
fn infer_try_seq(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    bindings: ori_ir::SeqBindingRange,
    result: ExprId,
    span: Span,
) -> Idx {
    // Enter a new scope for the try block
    engine.enter_scope();

    // Track the error type for Result propagation
    let mut error_ty: Option<Idx> = None;

    // Process each binding in sequence (with unwrapping)
    let seq_bindings = arena.get_seq_bindings(bindings);
    for binding in seq_bindings {
        if let ori_ir::SeqBinding::Let { value, .. } = binding {
            // Infer the value type first
            let value_ty = infer_expr(engine, arena, *value);
            let resolved = engine.resolve(value_ty);
            let tag = engine.pool().tag(resolved);

            // Track error type from Result
            if tag == Tag::Result && error_ty.is_none() {
                error_ty = Some(engine.pool().result_err(resolved));
            }
        }
        // Process binding with try-unwrapping enabled
        infer_seq_binding(engine, arena, binding, true);
    }

    // Infer the result expression
    let result_ty = infer_expr(engine, arena, result);

    // Exit scope
    engine.exit_scope();

    // The result type depends on what was in the bindings
    // If we saw Results, wrap the result in Result<T, E>
    // If we saw Options, wrap in Option<T>
    // Otherwise, return as-is (though this shouldn't happen in valid try blocks)
    if let Some(err_ty) = error_ty {
        engine.pool_mut().result(result_ty, err_ty)
    } else {
        // Check if result is already wrapped
        let resolved = engine.resolve(result_ty);
        let tag = engine.pool().tag(resolved);
        if tag == Tag::Result || tag == Tag::Option {
            result_ty
        } else {
            // Default to Result with a fresh error type for proper try semantics
            let _ = span; // Available for future error reporting
            let err_var = engine.fresh_var();
            engine.pool_mut().result(result_ty, err_var)
        }
    }
}

/// Infer type for `for(over: items, [map: transform,] match: Pattern -> expr, default: fallback)`.
///
/// Iterates over a collection, applies optional map, finds first matching pattern,
/// or returns default.
fn infer_for_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    over: ExprId,
    map: Option<ExprId>,
    arm: &ori_ir::MatchArm,
    default: ExprId,
    _span: Span,
) -> Idx {
    // Infer the iterable type
    let over_ty = infer_expr(engine, arena, over);
    let resolved_over = engine.resolve(over_ty);

    // Extract element type from collection
    let elem_ty = match engine.pool().tag(resolved_over) {
        Tag::List => engine.pool().list_elem(resolved_over),
        Tag::Set => engine.pool().set_elem(resolved_over),
        Tag::Range => engine.pool().range_elem(resolved_over),
        Tag::Map => engine.pool().map_key(resolved_over),
        _ => engine.fresh_var(), // Unknown iterable, create type var
    };

    // Apply optional map function
    let scrutinee_ty = if let Some(map_fn) = map {
        let map_fn_ty = infer_expr(engine, arena, map_fn);
        let resolved_map = engine.resolve(map_fn_ty);

        if engine.pool().tag(resolved_map) == Tag::Function {
            // Map function return type becomes the new element type
            engine.pool().function_return(resolved_map)
        } else {
            // Not a function, just use elem_ty
            elem_ty
        }
    } else {
        elem_ty
    };

    // Enter scope for pattern bindings
    engine.enter_scope();

    // Check pattern against scrutinee type
    check_match_pattern(engine, arena, &arm.pattern, scrutinee_ty);

    // Check guard if present
    if let Some(guard_id) = arm.guard {
        engine.push_context(ContextKind::MatchArmGuard { arm_index: 0 });
        let guard_ty = infer_expr(engine, arena, guard_id);
        let _ = engine.unify_types(guard_ty, Idx::BOOL);
        engine.pop_context();
    }

    // Infer arm body
    let arm_ty = infer_expr(engine, arena, arm.body);

    // Exit scope
    engine.exit_scope();

    // Infer default expression
    let default_ty = infer_expr(engine, arena, default);

    // Arm and default must have same type
    let _ = engine.unify_types(arm_ty, default_ty);

    arm_ty
}

/// Process a sequential binding (let or statement).
///
/// If `try_unwrap` is true, auto-unwrap Result/Option in let bindings.
fn infer_seq_binding(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    binding: &ori_ir::SeqBinding,
    try_unwrap: bool,
) {
    use ori_ir::SeqBinding;

    match binding {
        SeqBinding::Let {
            pattern, ty, value, ..
        } => {
            // Infer the initializer type
            let init_ty = infer_expr(engine, arena, *value);

            // For try blocks, unwrap Result/Option
            let bound_ty = if try_unwrap {
                unwrap_result_or_option(engine, init_ty)
            } else {
                init_ty
            };

            // Handle type annotation if present
            let final_ty = if let Some(parsed_ty) = ty {
                // TODO: Convert ParsedType to Idx
                let _ = parsed_ty;
                bound_ty
            } else {
                bound_ty
            };

            // Bind pattern to type
            bind_pattern(engine, arena, pattern, final_ty);
        }

        SeqBinding::Stmt { expr, .. } => {
            // Statement expression - evaluate for side effects
            infer_expr(engine, arena, *expr);
        }
    }
}

/// Unwrap Result<T, E> → T or Option<T> → T.
fn unwrap_result_or_option(engine: &mut InferEngine<'_>, ty: Idx) -> Idx {
    let resolved = engine.resolve(ty);
    let tag = engine.pool().tag(resolved);

    match tag {
        Tag::Result => engine.pool().result_ok(resolved),
        Tag::Option => engine.pool().option_inner(resolved),
        _ => ty, // Not wrapped, return as-is
    }
}

/// Bind a binding pattern to a type, introducing variables into scope.
#[expect(
    clippy::only_used_in_recursion,
    reason = "Arena will be used for struct field lookup in Section 07"
)]
fn bind_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    pattern: &ori_ir::BindingPattern,
    ty: Idx,
) {
    use ori_ir::BindingPattern;

    match pattern {
        BindingPattern::Name(name) => {
            engine.env_mut().bind(*name, ty);
        }

        BindingPattern::Tuple(patterns) => {
            let resolved = engine.resolve(ty);
            if engine.pool().tag(resolved) == Tag::Tuple {
                let elem_types = engine.pool().tuple_elems(resolved);
                for (pat, elem_ty) in patterns.iter().zip(elem_types.iter()) {
                    bind_pattern(engine, arena, pat, *elem_ty);
                }
            } else {
                // Type mismatch - bind each to fresh var
                for pat in patterns {
                    let var = engine.fresh_var();
                    bind_pattern(engine, arena, pat, var);
                }
            }
        }

        BindingPattern::Struct { fields } => {
            // TODO: Need registry to look up struct field types (Section 07)
            // For now, bind each field to a fresh variable
            for (name, sub_pattern) in fields {
                let field_ty = engine.fresh_var();
                if let Some(sub_pat) = sub_pattern {
                    bind_pattern(engine, arena, sub_pat, field_ty);
                } else {
                    // Shorthand: { x } means { x: x }
                    engine.env_mut().bind(*name, field_ty);
                }
            }
        }

        BindingPattern::List { elements, rest } => {
            let resolved = engine.resolve(ty);
            if engine.pool().tag(resolved) == Tag::List {
                let elem_ty = engine.pool().list_elem(resolved);
                for pat in elements {
                    bind_pattern(engine, arena, pat, elem_ty);
                }
                if let Some(rest_name) = rest {
                    // Rest binding gets the full list type
                    engine.env_mut().bind(*rest_name, ty);
                }
            } else {
                // Type mismatch - bind each to fresh var
                for pat in elements {
                    let var = engine.fresh_var();
                    bind_pattern(engine, arena, pat, var);
                }
                if let Some(rest_name) = rest {
                    engine.env_mut().bind(*rest_name, ty);
                }
            }
        }

        BindingPattern::Wildcard => {
            // Wildcard binds nothing
        }
    }
}

/// Infer type for a `function_exp` expression (recurse, parallel, print, etc.).
///
/// `FunctionExp` represents named property expressions:
/// - **Print**: `print(value: expr)` → unit
/// - **Panic**: `panic(message: expr)` → never
/// - **Todo/Unreachable**: `todo(message?: expr)` → never
/// - **Catch**: `catch(try: expr, catch: expr)` → T
/// - **Recurse**: `recurse(condition: expr, base: expr, step: expr)` → T
/// - **Parallel/Spawn/Timeout/Cache/With**: Concurrency patterns
fn infer_function_exp(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    func_exp: &ori_ir::FunctionExp,
) -> Idx {
    use ori_ir::FunctionExpKind;

    let props = arena.get_named_exprs(func_exp.props);

    match func_exp.kind {
        // === Simple built-ins ===
        FunctionExpKind::Print => {
            // print(value: expr) → unit
            // Evaluate the value (if present) for type checking
            for prop in props {
                infer_expr(engine, arena, prop.value);
            }
            Idx::UNIT
        }

        FunctionExpKind::Panic => {
            // panic(message: expr) → never
            // Evaluate message for type checking
            for prop in props {
                infer_expr(engine, arena, prop.value);
            }
            Idx::NEVER
        }

        FunctionExpKind::Todo => {
            // todo(message?: expr) → never
            // Optional message
            for prop in props {
                infer_expr(engine, arena, prop.value);
            }
            Idx::NEVER
        }

        FunctionExpKind::Unreachable => {
            // unreachable(message?: expr) → never
            for prop in props {
                infer_expr(engine, arena, prop.value);
            }
            Idx::NEVER
        }

        // === Error handling ===
        FunctionExpKind::Catch => {
            // catch(try: expr, catch: expr) → T
            // Both try and catch must produce the same type
            infer_catch(engine, arena, props)
        }

        // === Recursion ===
        FunctionExpKind::Recurse => {
            // recurse(condition: expr, base: expr, step: expr)
            // Complex: step can reference `self` (the recursive function)
            infer_recurse(engine, arena, props)
        }

        // === Concurrency patterns ===
        FunctionExpKind::Parallel => {
            // parallel(tasks: [expr]) → [T]
            // Returns list of results from parallel execution
            infer_parallel(engine, arena, props)
        }

        FunctionExpKind::Spawn => {
            // spawn(task: expr) → Task<T>
            // Returns a handle to the spawned task
            infer_spawn(engine, arena, props)
        }

        FunctionExpKind::Timeout => {
            // timeout(duration: Duration, task: expr) → Option<T>
            // Returns Some(result) or None if timeout
            infer_timeout(engine, arena, props)
        }

        FunctionExpKind::Cache => {
            // cache(key: expr, compute: expr) → T
            // Returns cached or computed value
            infer_cache(engine, arena, props)
        }

        FunctionExpKind::With => {
            // with(resource: expr, body: expr) → T
            // Resource management pattern
            infer_with(engine, arena, props)
        }
    }
}

/// Infer type for `catch(try: expr, catch: expr)`.
fn infer_catch(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    props: &[ori_ir::NamedExpr],
) -> Idx {
    let mut try_ty = None;
    let mut catch_ty = None;

    for prop in props {
        let ty = infer_expr(engine, arena, prop.value);
        // Note: We're comparing raw name indices. In real code, we'd intern "try" and "catch"
        // For now, assume: try = first prop, catch = second prop
        let _ = prop.name; // Will be used for proper property dispatch later
        if try_ty.is_none() {
            try_ty = Some(ty);
        } else if catch_ty.is_none() {
            catch_ty = Some(ty);
        }
    }

    match (try_ty, catch_ty) {
        (Some(t), Some(c)) => {
            // Both must produce same type
            let _ = engine.unify_types(t, c);
            t
        }
        (Some(t), None) => t,
        _ => engine.fresh_var(),
    }
}

/// Infer type for `recurse(condition: expr, base: expr, step: expr)`.
fn infer_recurse(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    props: &[ori_ir::NamedExpr],
) -> Idx {
    // The step expression needs access to `self` (the recursive function)
    // For now, we'll infer base and use that as the result type
    // Full implementation needs Section 07 (scoped bindings)

    let mut condition_ty = None;
    let mut base_ty = None;
    let mut step_ty = None;

    for prop in props {
        let ty = infer_expr(engine, arena, prop.value);
        if condition_ty.is_none() {
            // condition should be bool
            condition_ty = Some(ty);
        } else if base_ty.is_none() {
            base_ty = Some(ty);
        } else if step_ty.is_none() {
            step_ty = Some(ty);
        }
    }

    // Condition must be bool
    if let Some(cond) = condition_ty {
        let _ = engine.unify_types(cond, Idx::BOOL);
    }

    // Base and step must have same type
    if let (Some(b), Some(s)) = (base_ty, step_ty) {
        let _ = engine.unify_types(b, s);
    }

    base_ty.unwrap_or_else(|| engine.fresh_var())
}

/// Infer type for `parallel(tasks: [expr])`.
fn infer_parallel(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    props: &[ori_ir::NamedExpr],
) -> Idx {
    // Parallel takes a list of tasks and returns a list of results
    // For now, return [?a] with fresh variable
    for prop in props {
        let ty = infer_expr(engine, arena, prop.value);
        // If it's a list, extract element type and wrap result
        let resolved = engine.resolve(ty);
        if engine.pool().tag(resolved) == Tag::List {
            let elem_ty = engine.pool().list_elem(resolved);
            return engine.pool_mut().list(elem_ty);
        }
    }

    let result_ty = engine.fresh_var();
    engine.pool_mut().list(result_ty)
}

/// Infer type for `spawn(task: expr)`.
fn infer_spawn(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    props: &[ori_ir::NamedExpr],
) -> Idx {
    // Spawn returns a handle to the task
    // For now, return the task's result type wrapped in a fresh type
    // (Would need a Task<T> type in the pool)
    for prop in props {
        let _ = infer_expr(engine, arena, prop.value);
    }
    // TODO: Return proper Task<T> type when Task is added
    engine.fresh_var()
}

/// Infer type for `timeout(duration: Duration, task: expr)`.
fn infer_timeout(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    props: &[ori_ir::NamedExpr],
) -> Idx {
    // Returns Option<T> where T is the task result
    let mut task_ty = None;

    for prop in props {
        let ty = infer_expr(engine, arena, prop.value);
        // Skip duration, capture task type (first non-duration property)
        if task_ty.is_none() {
            let resolved = engine.resolve(ty);
            if engine.pool().tag(resolved) != Tag::Duration {
                task_ty = Some(ty);
            }
        }
        // If we already have a task type, just evaluate for type checking
    }

    let inner = task_ty.unwrap_or_else(|| engine.fresh_var());
    engine.pool_mut().option(inner)
}

/// Infer type for `cache(key: expr, compute: expr)`.
fn infer_cache(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    props: &[ori_ir::NamedExpr],
) -> Idx {
    // Returns the compute expression's type
    let mut compute_ty = None;

    for prop in props {
        let ty = infer_expr(engine, arena, prop.value);
        // Second property is typically the compute function
        if compute_ty.is_some() {
            compute_ty = Some(ty);
        } else {
            // Skip key
            compute_ty = Some(ty);
        }
    }

    compute_ty.unwrap_or_else(|| engine.fresh_var())
}

/// Infer type for `with(resource: expr, body: expr)`.
fn infer_with(engine: &mut InferEngine<'_>, arena: &ExprArena, props: &[ori_ir::NamedExpr]) -> Idx {
    // Returns the body expression's type
    let mut body_ty = None;

    for prop in props {
        let ty = infer_expr(engine, arena, prop.value);
        body_ty = Some(ty);
    }

    body_ty.unwrap_or_else(|| engine.fresh_var())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Pool;
    use ori_ir::{
        ast::{Expr, ExprKind, MapEntry, MatchArm, MatchPattern, Param, Stmt, StmtKind},
        BindingPattern, ExprArena, ExprId, Name, Span,
    };

    // ========================================================================
    // Test Helpers
    // ========================================================================

    /// Create a Name from a raw u32 for testing.
    fn name(n: u32) -> Name {
        Name::from_raw(n)
    }

    /// Create a dummy span for test expressions.
    fn span() -> Span {
        Span::DUMMY
    }

    /// Helper to build an expression and get its ID.
    fn alloc(arena: &mut ExprArena, kind: ExprKind) -> ExprId {
        arena.alloc_expr(Expr::new(kind, span()))
    }

    // ========================================================================
    // Literal Inference Tests
    // ========================================================================

    #[test]
    fn test_infer_literal_int() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let expr_id = alloc(&mut arena, ExprKind::Int(42));
        let ty = infer_expr(&mut engine, &arena, expr_id);

        assert_eq!(ty, Idx::INT);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_literal_float() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let expr_id = alloc(&mut arena, ExprKind::Float(3_14_f64.to_bits()));
        let ty = infer_expr(&mut engine, &arena, expr_id);

        assert_eq!(ty, Idx::FLOAT);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_literal_bool() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let true_id = alloc(&mut arena, ExprKind::Bool(true));
        let false_id = alloc(&mut arena, ExprKind::Bool(false));

        assert_eq!(infer_expr(&mut engine, &arena, true_id), Idx::BOOL);
        assert_eq!(infer_expr(&mut engine, &arena, false_id), Idx::BOOL);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_literal_str() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let expr_id = alloc(&mut arena, ExprKind::String(name(1)));
        let ty = infer_expr(&mut engine, &arena, expr_id);

        assert_eq!(ty, Idx::STR);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_literal_char() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let expr_id = alloc(&mut arena, ExprKind::Char('a'));
        let ty = infer_expr(&mut engine, &arena, expr_id);

        assert_eq!(ty, Idx::CHAR);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_literal_unit() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let expr_id = alloc(&mut arena, ExprKind::Unit);
        let ty = infer_expr(&mut engine, &arena, expr_id);

        assert_eq!(ty, Idx::UNIT);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_literal_duration() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let expr_id = alloc(
            &mut arena,
            ExprKind::Duration {
                value: 100,
                unit: ori_ir::DurationUnit::Milliseconds,
            },
        );
        let ty = infer_expr(&mut engine, &arena, expr_id);

        assert_eq!(ty, Idx::DURATION);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_literal_size() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let expr_id = alloc(
            &mut arena,
            ExprKind::Size {
                value: 1024,
                unit: ori_ir::SizeUnit::Kilobytes,
            },
        );
        let ty = infer_expr(&mut engine, &arena, expr_id);

        assert_eq!(ty, Idx::SIZE);
        assert!(!engine.has_errors());
    }

    // ========================================================================
    // Binary Operator Tests
    // ========================================================================

    #[test]
    fn test_infer_binary_arithmetic_int() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let left = alloc(&mut arena, ExprKind::Int(10));
        let right = alloc(&mut arena, ExprKind::Int(5));
        let add = alloc(
            &mut arena,
            ExprKind::Binary {
                op: BinaryOp::Add,
                left,
                right,
            },
        );

        let ty = infer_expr(&mut engine, &arena, add);

        assert_eq!(ty, Idx::INT);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_binary_arithmetic_float() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let left = alloc(&mut arena, ExprKind::Float(1_5_f64.to_bits()));
        let right = alloc(&mut arena, ExprKind::Float(2_5_f64.to_bits()));
        let mul = alloc(
            &mut arena,
            ExprKind::Binary {
                op: BinaryOp::Mul,
                left,
                right,
            },
        );

        let ty = infer_expr(&mut engine, &arena, mul);

        assert_eq!(ty, Idx::FLOAT);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_binary_comparison() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let left = alloc(&mut arena, ExprKind::Int(10));
        let right = alloc(&mut arena, ExprKind::Int(5));
        let lt = alloc(
            &mut arena,
            ExprKind::Binary {
                op: BinaryOp::Lt,
                left,
                right,
            },
        );

        let ty = infer_expr(&mut engine, &arena, lt);

        assert_eq!(ty, Idx::BOOL);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_binary_equality() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let left = alloc(&mut arena, ExprKind::String(name(1)));
        let right = alloc(&mut arena, ExprKind::String(name(2)));
        let eq = alloc(
            &mut arena,
            ExprKind::Binary {
                op: BinaryOp::Eq,
                left,
                right,
            },
        );

        let ty = infer_expr(&mut engine, &arena, eq);

        assert_eq!(ty, Idx::BOOL);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_binary_boolean_and() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let left = alloc(&mut arena, ExprKind::Bool(true));
        let right = alloc(&mut arena, ExprKind::Bool(false));
        let and_op = alloc(
            &mut arena,
            ExprKind::Binary {
                op: BinaryOp::And,
                left,
                right,
            },
        );

        let ty = infer_expr(&mut engine, &arena, and_op);

        assert_eq!(ty, Idx::BOOL);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_binary_boolean_or() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let left = alloc(&mut arena, ExprKind::Bool(true));
        let right = alloc(&mut arena, ExprKind::Bool(false));
        let or_op = alloc(
            &mut arena,
            ExprKind::Binary {
                op: BinaryOp::Or,
                left,
                right,
            },
        );

        let ty = infer_expr(&mut engine, &arena, or_op);

        assert_eq!(ty, Idx::BOOL);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_binary_bitwise() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let left = alloc(&mut arena, ExprKind::Int(0xFF));
        let right = alloc(&mut arena, ExprKind::Int(0x0F));
        let bitand = alloc(
            &mut arena,
            ExprKind::Binary {
                op: BinaryOp::BitAnd,
                left,
                right,
            },
        );

        let ty = infer_expr(&mut engine, &arena, bitand);

        assert_eq!(ty, Idx::INT);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_binary_range() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let left = alloc(&mut arena, ExprKind::Int(1));
        let right = alloc(&mut arena, ExprKind::Int(10));
        let range = alloc(
            &mut arena,
            ExprKind::Binary {
                op: BinaryOp::Range,
                left,
                right,
            },
        );

        let ty = infer_expr(&mut engine, &arena, range);

        assert_eq!(engine.pool().tag(ty), Tag::Range);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_binary_type_mismatch_reports_error() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let left = alloc(&mut arena, ExprKind::Int(10));
        let right = alloc(&mut arena, ExprKind::String(name(1)));
        let add = alloc(
            &mut arena,
            ExprKind::Binary {
                op: BinaryOp::Add,
                left,
                right,
            },
        );

        let _ = infer_expr(&mut engine, &arena, add);

        assert!(engine.has_errors(), "Should report type mismatch error");
    }

    // ========================================================================
    // Unary Operator Tests
    // ========================================================================

    #[test]
    fn test_infer_unary_neg_int() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let operand = alloc(&mut arena, ExprKind::Int(42));
        let neg = alloc(
            &mut arena,
            ExprKind::Unary {
                op: UnaryOp::Neg,
                operand,
            },
        );

        let ty = infer_expr(&mut engine, &arena, neg);

        assert_eq!(ty, Idx::INT);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_unary_neg_float() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let operand = alloc(&mut arena, ExprKind::Float(3_14_f64.to_bits()));
        let neg = alloc(
            &mut arena,
            ExprKind::Unary {
                op: UnaryOp::Neg,
                operand,
            },
        );

        let ty = infer_expr(&mut engine, &arena, neg);

        assert_eq!(ty, Idx::FLOAT);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_unary_not() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let operand = alloc(&mut arena, ExprKind::Bool(true));
        let not = alloc(
            &mut arena,
            ExprKind::Unary {
                op: UnaryOp::Not,
                operand,
            },
        );

        let ty = infer_expr(&mut engine, &arena, not);

        assert_eq!(ty, Idx::BOOL);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_unary_bitnot() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let operand = alloc(&mut arena, ExprKind::Int(0xFF));
        let bitnot = alloc(
            &mut arena,
            ExprKind::Unary {
                op: UnaryOp::BitNot,
                operand,
            },
        );

        let ty = infer_expr(&mut engine, &arena, bitnot);

        assert_eq!(ty, Idx::INT);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_unary_try_option() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // Bind 'opt' to Option<int>
        let opt_ty = engine.infer_option(Idx::INT);
        engine.env_mut().bind(name(1), opt_ty);

        let operand = alloc(&mut arena, ExprKind::Ident(name(1)));
        let try_op = alloc(
            &mut arena,
            ExprKind::Unary {
                op: UnaryOp::Try,
                operand,
            },
        );

        let ty = infer_expr(&mut engine, &arena, try_op);

        assert_eq!(ty, Idx::INT, "Try on Option<int> should yield int");
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_unary_try_result() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // Bind 'res' to Result<str, int>
        let res_ty = engine.infer_result(Idx::STR, Idx::INT);
        engine.env_mut().bind(name(1), res_ty);

        let operand = alloc(&mut arena, ExprKind::Ident(name(1)));
        let try_op = alloc(
            &mut arena,
            ExprKind::Unary {
                op: UnaryOp::Try,
                operand,
            },
        );

        let ty = infer_expr(&mut engine, &arena, try_op);

        assert_eq!(ty, Idx::STR, "Try on Result<str, _> should yield str");
        assert!(!engine.has_errors());
    }

    // ========================================================================
    // Collection Inference Tests
    // ========================================================================

    #[test]
    fn test_infer_empty_list() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let list = arena.alloc_expr_list_inline(&[]);
        let expr_id = alloc(&mut arena, ExprKind::List(list));

        let ty = infer_expr(&mut engine, &arena, expr_id);

        assert_eq!(engine.pool().tag(ty), Tag::List);
        // Element type should be a fresh variable
        let elem_ty = engine.pool().list_elem(ty);
        assert_eq!(engine.pool().tag(elem_ty), Tag::Var);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_list_homogeneous() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let e1 = alloc(&mut arena, ExprKind::Int(1));
        let e2 = alloc(&mut arena, ExprKind::Int(2));
        let e3 = alloc(&mut arena, ExprKind::Int(3));
        let list = arena.alloc_expr_list_inline(&[e1, e2, e3]);
        let expr_id = alloc(&mut arena, ExprKind::List(list));

        let ty = infer_expr(&mut engine, &arena, expr_id);

        assert_eq!(engine.pool().tag(ty), Tag::List);
        let elem_ty = engine.resolve(engine.pool().list_elem(ty));
        assert_eq!(elem_ty, Idx::INT);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_list_heterogeneous_error() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let e1 = alloc(&mut arena, ExprKind::Int(1));
        let e2 = alloc(&mut arena, ExprKind::String(name(1)));
        let list = arena.alloc_expr_list_inline(&[e1, e2]);
        let expr_id = alloc(&mut arena, ExprKind::List(list));

        let _ = infer_expr(&mut engine, &arena, expr_id);

        assert!(
            engine.has_errors(),
            "Mixed int/str in list should report error"
        );
    }

    #[test]
    fn test_infer_tuple() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let e1 = alloc(&mut arena, ExprKind::Int(42));
        let e2 = alloc(&mut arena, ExprKind::String(name(1)));
        let e3 = alloc(&mut arena, ExprKind::Bool(true));
        let tuple = arena.alloc_expr_list_inline(&[e1, e2, e3]);
        let expr_id = alloc(&mut arena, ExprKind::Tuple(tuple));

        let ty = infer_expr(&mut engine, &arena, expr_id);

        assert_eq!(engine.pool().tag(ty), Tag::Tuple);
        let elems = engine.pool().tuple_elems(ty);
        assert_eq!(elems.len(), 3);
        assert_eq!(elems[0], Idx::INT);
        assert_eq!(elems[1], Idx::STR);
        assert_eq!(elems[2], Idx::BOOL);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_empty_map() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let entries = arena.alloc_map_entries(std::iter::empty());
        let expr_id = alloc(&mut arena, ExprKind::Map(entries));

        let ty = infer_expr(&mut engine, &arena, expr_id);

        assert_eq!(engine.pool().tag(ty), Tag::Map);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_map_with_entries() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let k1 = alloc(&mut arena, ExprKind::String(name(1)));
        let v1 = alloc(&mut arena, ExprKind::Int(100));
        let k2 = alloc(&mut arena, ExprKind::String(name(2)));
        let v2 = alloc(&mut arena, ExprKind::Int(200));

        let entries = arena.alloc_map_entries([
            MapEntry {
                key: k1,
                value: v1,
                span: span(),
            },
            MapEntry {
                key: k2,
                value: v2,
                span: span(),
            },
        ]);
        let expr_id = alloc(&mut arena, ExprKind::Map(entries));

        let ty = infer_expr(&mut engine, &arena, expr_id);

        assert_eq!(engine.pool().tag(ty), Tag::Map);
        let key_ty = engine.resolve(engine.pool().map_key(ty));
        let val_ty = engine.resolve(engine.pool().map_value(ty));
        assert_eq!(key_ty, Idx::STR);
        assert_eq!(val_ty, Idx::INT);
        assert!(!engine.has_errors());
    }

    // ========================================================================
    // If/Else Inference Tests
    // ========================================================================

    #[test]
    fn test_infer_if_with_else() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let cond = alloc(&mut arena, ExprKind::Bool(true));
        let then_branch = alloc(&mut arena, ExprKind::Int(1));
        let else_branch = alloc(&mut arena, ExprKind::Int(2));

        let if_expr = alloc(
            &mut arena,
            ExprKind::If {
                cond,
                then_branch,
                else_branch: Some(else_branch),
            },
        );

        let ty = infer_expr(&mut engine, &arena, if_expr);

        assert_eq!(ty, Idx::INT);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_if_without_else() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let cond = alloc(&mut arena, ExprKind::Bool(true));
        let then_branch = alloc(&mut arena, ExprKind::Unit);

        let if_expr = alloc(
            &mut arena,
            ExprKind::If {
                cond,
                then_branch,
                else_branch: None,
            },
        );

        let ty = infer_expr(&mut engine, &arena, if_expr);

        assert_eq!(ty, Idx::UNIT);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_if_branch_mismatch() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let cond = alloc(&mut arena, ExprKind::Bool(true));
        let then_branch = alloc(&mut arena, ExprKind::Int(1));
        let else_branch = alloc(&mut arena, ExprKind::String(name(1)));

        let if_expr = alloc(
            &mut arena,
            ExprKind::If {
                cond,
                then_branch,
                else_branch: Some(else_branch),
            },
        );

        let _ = infer_expr(&mut engine, &arena, if_expr);

        assert!(
            engine.has_errors(),
            "Mismatched branches should report error"
        );
    }

    #[test]
    fn test_infer_if_non_bool_condition() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let cond = alloc(&mut arena, ExprKind::Int(1)); // Not a bool!
        let then_branch = alloc(&mut arena, ExprKind::Unit);

        let if_expr = alloc(
            &mut arena,
            ExprKind::If {
                cond,
                then_branch,
                else_branch: None,
            },
        );

        let _ = infer_expr(&mut engine, &arena, if_expr);

        assert!(
            engine.has_errors(),
            "Non-bool condition should report error"
        );
    }

    // ========================================================================
    // Match Expression Tests
    // ========================================================================

    #[test]
    fn test_infer_match_simple() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let scrutinee = alloc(&mut arena, ExprKind::Int(42));
        let body1 = alloc(&mut arena, ExprKind::String(name(1)));
        let body2 = alloc(&mut arena, ExprKind::String(name(2)));

        // Pattern: _
        let pattern1 = arena.alloc_match_pattern(MatchPattern::Wildcard);
        let pattern2 = arena.alloc_match_pattern(MatchPattern::Wildcard);

        let arms = arena.alloc_arms([
            MatchArm {
                pattern: arena.get_match_pattern(pattern1).clone(),
                guard: None,
                body: body1,
                span: span(),
            },
            MatchArm {
                pattern: arena.get_match_pattern(pattern2).clone(),
                guard: None,
                body: body2,
                span: span(),
            },
        ]);

        let match_expr = alloc(&mut arena, ExprKind::Match { scrutinee, arms });

        let ty = infer_expr(&mut engine, &arena, match_expr);

        assert_eq!(ty, Idx::STR, "Match should return string type");
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_match_with_binding() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let scrutinee = alloc(&mut arena, ExprKind::Int(42));

        // Use the bound variable 'x' in the body
        let x_ref = alloc(&mut arena, ExprKind::Ident(name(1)));

        // Pattern: x (binding)
        let pattern = arena.alloc_match_pattern(MatchPattern::Binding(name(1)));

        let arms = arena.alloc_arms([MatchArm {
            pattern: arena.get_match_pattern(pattern).clone(),
            guard: None,
            body: x_ref,
            span: span(),
        }]);

        let match_expr = alloc(&mut arena, ExprKind::Match { scrutinee, arms });

        let ty = infer_expr(&mut engine, &arena, match_expr);

        assert_eq!(
            ty,
            Idx::INT,
            "Binding 'x' should have int type from scrutinee"
        );
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_match_arm_type_mismatch() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let scrutinee = alloc(&mut arena, ExprKind::Int(42));
        let body1 = alloc(&mut arena, ExprKind::Int(1));
        let body2 = alloc(&mut arena, ExprKind::String(name(1))); // Type mismatch!

        let pattern1 = arena.alloc_match_pattern(MatchPattern::Wildcard);
        let pattern2 = arena.alloc_match_pattern(MatchPattern::Wildcard);

        let arms = arena.alloc_arms([
            MatchArm {
                pattern: arena.get_match_pattern(pattern1).clone(),
                guard: None,
                body: body1,
                span: span(),
            },
            MatchArm {
                pattern: arena.get_match_pattern(pattern2).clone(),
                guard: None,
                body: body2,
                span: span(),
            },
        ]);

        let match_expr = alloc(&mut arena, ExprKind::Match { scrutinee, arms });
        let _ = infer_expr(&mut engine, &arena, match_expr);

        assert!(
            engine.has_errors(),
            "Mismatched arm types should report error"
        );
    }

    // ========================================================================
    // For Loop Tests
    // ========================================================================

    #[test]
    fn test_infer_for_do() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // Bind 'list' to [int]
        let list_ty = engine.infer_list(Idx::INT);
        engine.env_mut().bind(name(1), list_ty);

        let iter = alloc(&mut arena, ExprKind::Ident(name(1)));
        let body = alloc(&mut arena, ExprKind::Unit);

        let for_expr = alloc(
            &mut arena,
            ExprKind::For {
                binding: name(2), // 'x'
                iter,
                guard: None,
                body,
                is_yield: false,
            },
        );

        let ty = infer_expr(&mut engine, &arena, for_expr);

        assert_eq!(ty, Idx::UNIT, "For-do should return unit");
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_for_yield() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // Bind 'list' to [int]
        let list_ty = engine.infer_list(Idx::INT);
        engine.env_mut().bind(name(1), list_ty);

        let iter = alloc(&mut arena, ExprKind::Ident(name(1)));
        // Use x (the bound element) in body
        let x_ref = alloc(&mut arena, ExprKind::Ident(name(2)));

        let for_expr = alloc(
            &mut arena,
            ExprKind::For {
                binding: name(2), // 'x'
                iter,
                guard: None,
                body: x_ref,
                is_yield: true,
            },
        );

        let ty = infer_expr(&mut engine, &arena, for_expr);

        assert_eq!(
            engine.pool().tag(ty),
            Tag::List,
            "For-yield should return list"
        );
        let elem_ty = engine.resolve(engine.pool().list_elem(ty));
        assert_eq!(elem_ty, Idx::INT, "Yielded elements should be int");
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_for_with_guard() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // Bind 'list' to [int]
        let list_ty = engine.infer_list(Idx::INT);
        engine.env_mut().bind(name(1), list_ty);

        let iter = alloc(&mut arena, ExprKind::Ident(name(1)));
        let guard = alloc(&mut arena, ExprKind::Bool(true));
        let body = alloc(&mut arena, ExprKind::Unit);

        let for_expr = alloc(
            &mut arena,
            ExprKind::For {
                binding: name(2),
                iter,
                guard: Some(guard),
                body,
                is_yield: false,
            },
        );

        let ty = infer_expr(&mut engine, &arena, for_expr);

        assert_eq!(ty, Idx::UNIT);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_for_guard_not_bool() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let list_ty = engine.infer_list(Idx::INT);
        engine.env_mut().bind(name(1), list_ty);

        let iter = alloc(&mut arena, ExprKind::Ident(name(1)));
        let guard = alloc(&mut arena, ExprKind::Int(1)); // Not bool!
        let body = alloc(&mut arena, ExprKind::Unit);

        let for_expr = alloc(
            &mut arena,
            ExprKind::For {
                binding: name(2),
                iter,
                guard: Some(guard),
                body,
                is_yield: false,
            },
        );

        let _ = infer_expr(&mut engine, &arena, for_expr);

        assert!(engine.has_errors(), "Non-bool guard should report error");
    }

    // ========================================================================
    // Loop (Infinite) Tests
    // ========================================================================

    #[test]
    fn test_infer_infinite_loop() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let body = alloc(&mut arena, ExprKind::Unit);
        let loop_expr = alloc(&mut arena, ExprKind::Loop { body });

        let ty = infer_expr(&mut engine, &arena, loop_expr);

        // Currently returns UNIT (break value tracking not yet implemented)
        assert_eq!(ty, Idx::UNIT);
        assert!(!engine.has_errors());
    }

    // ========================================================================
    // Identifier and Environment Tests
    // ========================================================================

    #[test]
    fn test_infer_ident_bound() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        engine.env_mut().bind(name(1), Idx::INT);

        let expr_id = alloc(&mut arena, ExprKind::Ident(name(1)));
        let ty = infer_expr(&mut engine, &arena, expr_id);

        assert_eq!(ty, Idx::INT);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_ident_unbound() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let expr_id = alloc(&mut arena, ExprKind::Ident(name(999)));
        let ty = infer_expr(&mut engine, &arena, expr_id);

        assert_eq!(ty, Idx::ERROR);
        assert!(
            engine.has_errors(),
            "Unbound identifier should report error"
        );
    }

    // ========================================================================
    // Function Call Tests
    // ========================================================================

    #[test]
    fn test_infer_call_simple() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // Bind 'f' to (int) -> str
        let fn_ty = engine.infer_function(&[Idx::INT], Idx::STR);
        engine.env_mut().bind(name(1), fn_ty);

        let func = alloc(&mut arena, ExprKind::Ident(name(1)));
        let arg = alloc(&mut arena, ExprKind::Int(42));
        let args = arena.alloc_expr_list_inline(&[arg]);

        let call = alloc(&mut arena, ExprKind::Call { func, args });

        let ty = infer_expr(&mut engine, &arena, call);

        assert_eq!(ty, Idx::STR);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_call_arity_mismatch() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // Bind 'f' to (int, int) -> str (expects 2 args)
        let fn_ty = engine.infer_function(&[Idx::INT, Idx::INT], Idx::STR);
        engine.env_mut().bind(name(1), fn_ty);

        let func = alloc(&mut arena, ExprKind::Ident(name(1)));
        let arg = alloc(&mut arena, ExprKind::Int(42));
        let args = arena.alloc_expr_list_inline(&[arg]); // Only 1 arg

        let call = alloc(&mut arena, ExprKind::Call { func, args });
        let ty = infer_expr(&mut engine, &arena, call);

        assert_eq!(ty, Idx::ERROR);
        assert!(engine.has_errors(), "Arity mismatch should report error");
    }

    #[test]
    fn test_infer_call_arg_type_mismatch() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // Bind 'f' to (int) -> str
        let fn_ty = engine.infer_function(&[Idx::INT], Idx::STR);
        engine.env_mut().bind(name(1), fn_ty);

        let func = alloc(&mut arena, ExprKind::Ident(name(1)));
        let arg = alloc(&mut arena, ExprKind::String(name(2))); // str, not int
        let args = arena.alloc_expr_list_inline(&[arg]);

        let call = alloc(&mut arena, ExprKind::Call { func, args });
        let _ = infer_expr(&mut engine, &arena, call);

        assert!(
            engine.has_errors(),
            "Argument type mismatch should report error"
        );
    }

    #[test]
    fn test_infer_call_not_callable() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // Bind 'x' to int (not callable)
        engine.env_mut().bind(name(1), Idx::INT);

        let func = alloc(&mut arena, ExprKind::Ident(name(1)));
        let args = arena.alloc_expr_list_inline(&[]);

        let call = alloc(&mut arena, ExprKind::Call { func, args });
        let ty = infer_expr(&mut engine, &arena, call);

        assert_eq!(ty, Idx::ERROR);
        assert!(
            engine.has_errors(),
            "Calling non-function should report error"
        );
    }

    // ========================================================================
    // Lambda Tests
    // ========================================================================

    #[test]
    fn test_infer_lambda_simple() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // |x| x (identity function)
        let body = alloc(&mut arena, ExprKind::Ident(name(1)));
        let params = arena.alloc_params([Param {
            name: name(1),
            pattern: None,
            ty: None,
            default: None,
            is_variadic: false,
            span: span(),
        }]);

        let lambda = alloc(
            &mut arena,
            ExprKind::Lambda {
                params,
                ret_ty: None,
                body,
            },
        );

        let ty = infer_expr(&mut engine, &arena, lambda);

        assert_eq!(engine.pool().tag(ty), Tag::Function);
        let params_ty = engine.pool().function_params(ty);
        assert_eq!(params_ty.len(), 1);
        // Parameter type is a fresh variable
        assert_eq!(engine.pool().tag(params_ty[0]), Tag::Var);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_lambda_with_body_int() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // |x| 42 (constant function returning int)
        let body = alloc(&mut arena, ExprKind::Int(42));
        let params = arena.alloc_params([Param {
            name: name(1),
            pattern: None,
            ty: None,
            default: None,
            is_variadic: false,
            span: span(),
        }]);

        let lambda = alloc(
            &mut arena,
            ExprKind::Lambda {
                params,
                ret_ty: None,
                body,
            },
        );

        let ty = infer_expr(&mut engine, &arena, lambda);

        assert_eq!(engine.pool().tag(ty), Tag::Function);
        let ret_ty = engine.pool().function_return(ty);
        assert_eq!(ret_ty, Idx::INT);
        assert!(!engine.has_errors());
    }

    // ========================================================================
    // Block Tests
    // ========================================================================

    #[test]
    fn test_infer_block_empty() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let stmts = arena.alloc_stmt_range(0, 0);
        let block = alloc(
            &mut arena,
            ExprKind::Block {
                stmts,
                result: None,
            },
        );

        let ty = infer_expr(&mut engine, &arena, block);

        assert_eq!(ty, Idx::UNIT);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_block_with_result() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let result_expr = alloc(&mut arena, ExprKind::Int(42));
        let stmts = arena.alloc_stmt_range(0, 0);
        let block = alloc(
            &mut arena,
            ExprKind::Block {
                stmts,
                result: Some(result_expr),
            },
        );

        let ty = infer_expr(&mut engine, &arena, block);

        assert_eq!(ty, Idx::INT);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_block_with_let() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // { let x = 42; x }
        let init = alloc(&mut arena, ExprKind::Int(42));
        let _stmt = arena.alloc_stmt(Stmt {
            kind: StmtKind::Let {
                pattern: BindingPattern::Name(name(1)),
                ty: None,
                init,
                mutable: false,
            },
            span: span(),
        });

        let result_expr = alloc(&mut arena, ExprKind::Ident(name(1)));
        let stmts = arena.alloc_stmt_range(0, 1);
        let block = alloc(
            &mut arena,
            ExprKind::Block {
                stmts,
                result: Some(result_expr),
            },
        );

        let ty = infer_expr(&mut engine, &arena, block);

        assert_eq!(ty, Idx::INT, "Block should resolve x to int");
        assert!(!engine.has_errors());
    }

    // ========================================================================
    // Option/Result Constructor Tests
    // ========================================================================

    #[test]
    fn test_infer_some() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let inner = alloc(&mut arena, ExprKind::Int(42));
        let some = alloc(&mut arena, ExprKind::Some(inner));

        let ty = infer_expr(&mut engine, &arena, some);

        assert_eq!(engine.pool().tag(ty), Tag::Option);
        let inner_ty = engine.pool().option_inner(ty);
        assert_eq!(inner_ty, Idx::INT);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_none() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let none = alloc(&mut arena, ExprKind::None);

        let ty = infer_expr(&mut engine, &arena, none);

        assert_eq!(engine.pool().tag(ty), Tag::Option);
        // Inner type is a fresh variable
        let inner_ty = engine.pool().option_inner(ty);
        assert_eq!(engine.pool().tag(inner_ty), Tag::Var);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_ok() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let inner = alloc(&mut arena, ExprKind::String(name(1)));
        let ok = alloc(&mut arena, ExprKind::Ok(Some(inner)));

        let ty = infer_expr(&mut engine, &arena, ok);

        assert_eq!(engine.pool().tag(ty), Tag::Result);
        let ok_ty = engine.pool().result_ok(ty);
        assert_eq!(ok_ty, Idx::STR);
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_err() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let inner = alloc(&mut arena, ExprKind::String(name(1)));
        let err = alloc(&mut arena, ExprKind::Err(Some(inner)));

        let ty = infer_expr(&mut engine, &arena, err);

        assert_eq!(engine.pool().tag(ty), Tag::Result);
        let err_ty = engine.pool().result_err(ty);
        assert_eq!(err_ty, Idx::STR);
        assert!(!engine.has_errors());
    }

    // ========================================================================
    // Range Expression Tests
    // ========================================================================

    #[test]
    fn test_infer_range_explicit() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let start = alloc(&mut arena, ExprKind::Int(1));
        let end = alloc(&mut arena, ExprKind::Int(10));

        let range = alloc(
            &mut arena,
            ExprKind::Range {
                start: Some(start),
                end: Some(end),
                step: None,
                inclusive: false,
            },
        );

        let ty = infer_expr(&mut engine, &arena, range);

        assert_eq!(engine.pool().tag(ty), Tag::Range);
        let elem_ty = engine.resolve(engine.pool().range_elem(ty));
        assert_eq!(elem_ty, Idx::INT);
        assert!(!engine.has_errors());
    }

    // ========================================================================
    // Assignment Tests
    // ========================================================================

    #[test]
    fn test_infer_assign() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // Bind 'x' to int
        engine.env_mut().bind(name(1), Idx::INT);

        let target = alloc(&mut arena, ExprKind::Ident(name(1)));
        let value = alloc(&mut arena, ExprKind::Int(42));
        let assign = alloc(&mut arena, ExprKind::Assign { target, value });

        let ty = infer_expr(&mut engine, &arena, assign);

        assert_eq!(ty, Idx::UNIT, "Assignment returns unit");
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_assign_type_mismatch() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        engine.env_mut().bind(name(1), Idx::INT);

        let target = alloc(&mut arena, ExprKind::Ident(name(1)));
        let value = alloc(&mut arena, ExprKind::String(name(2))); // str, not int
        let assign = alloc(&mut arena, ExprKind::Assign { target, value });

        let _ = infer_expr(&mut engine, &arena, assign);

        assert!(
            engine.has_errors(),
            "Assigning wrong type should report error"
        );
    }

    // ========================================================================
    // Break/Continue Tests
    // ========================================================================

    #[test]
    fn test_infer_break() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let break_expr = alloc(&mut arena, ExprKind::Break(None));
        let ty = infer_expr(&mut engine, &arena, break_expr);

        assert_eq!(ty, Idx::NEVER, "Break returns never type");
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_continue() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let continue_expr = alloc(&mut arena, ExprKind::Continue(None));
        let ty = infer_expr(&mut engine, &arena, continue_expr);

        assert_eq!(ty, Idx::NEVER, "Continue returns never type");
        assert!(!engine.has_errors());
    }

    // ========================================================================
    // Error Expression Test
    // ========================================================================

    #[test]
    fn test_infer_error_expr() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        let error = alloc(&mut arena, ExprKind::Error);
        let ty = infer_expr(&mut engine, &arena, error);

        assert_eq!(ty, Idx::ERROR);
        assert!(!engine.has_errors(), "Error expr itself doesn't add errors");
    }

    // ========================================================================
    // Coalesce Operator Tests
    // ========================================================================

    #[test]
    fn test_infer_coalesce() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // Bind 'opt' to Option<int>
        let opt_ty = engine.infer_option(Idx::INT);
        engine.env_mut().bind(name(1), opt_ty);

        let left = alloc(&mut arena, ExprKind::Ident(name(1)));
        let right = alloc(&mut arena, ExprKind::Int(0));
        let coalesce = alloc(
            &mut arena,
            ExprKind::Binary {
                op: BinaryOp::Coalesce,
                left,
                right,
            },
        );

        let ty = infer_expr(&mut engine, &arena, coalesce);

        assert_eq!(ty, Idx::INT, "Option<int> ?? int = int");
        assert!(!engine.has_errors());
    }

    // ========================================================================
    // Pattern Expression Tests (FunctionSeq)
    // ========================================================================

    #[test]
    fn test_infer_function_seq_run() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // run(let x = 42, x + 1)
        let init = alloc(&mut arena, ExprKind::Int(42));
        let bindings = arena.alloc_seq_bindings([ori_ir::SeqBinding::Let {
            pattern: BindingPattern::Name(name(1)),
            ty: None,
            value: init,
            mutable: false,
            span: Span::DUMMY,
        }]);

        // x + 1 where x is name(1)
        let x_ref = alloc(&mut arena, ExprKind::Ident(name(1)));
        let one = alloc(&mut arena, ExprKind::Int(1));
        let result = alloc(
            &mut arena,
            ExprKind::Binary {
                op: BinaryOp::Add,
                left: x_ref,
                right: one,
            },
        );

        let func_seq = ori_ir::FunctionSeq::Run {
            bindings,
            result,
            span: Span::DUMMY,
        };
        let expr_id = alloc(&mut arena, ExprKind::FunctionSeq(func_seq));

        let ty = infer_expr(&mut engine, &arena, expr_id);

        assert_eq!(ty, Idx::INT, "run should return result type");
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_function_seq_run_multiple_bindings() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // run(let x = 1, let y = "hello", y)
        let x_init = alloc(&mut arena, ExprKind::Int(1));
        let y_init = alloc(&mut arena, ExprKind::String(ori_ir::Name::from_raw(100)));

        let bindings = arena.alloc_seq_bindings([
            ori_ir::SeqBinding::Let {
                pattern: BindingPattern::Name(name(1)),
                ty: None,
                value: x_init,
                mutable: false,
                span: Span::DUMMY,
            },
            ori_ir::SeqBinding::Let {
                pattern: BindingPattern::Name(name(2)),
                ty: None,
                value: y_init,
                mutable: false,
                span: Span::DUMMY,
            },
        ]);

        let y_ref = alloc(&mut arena, ExprKind::Ident(name(2)));

        let func_seq = ori_ir::FunctionSeq::Run {
            bindings,
            result: y_ref,
            span: Span::DUMMY,
        };
        let expr_id = alloc(&mut arena, ExprKind::FunctionSeq(func_seq));

        let ty = infer_expr(&mut engine, &arena, expr_id);

        assert_eq!(ty, Idx::STR, "run should return str from y");
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_function_exp_print() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // print(value: "hello")
        let value = alloc(&mut arena, ExprKind::String(ori_ir::Name::from_raw(100)));
        let props = arena.alloc_named_exprs([ori_ir::NamedExpr {
            name: name(1), // "value"
            value,
            span: Span::DUMMY,
        }]);

        let func_exp = ori_ir::FunctionExp {
            kind: ori_ir::FunctionExpKind::Print,
            props,
            span: Span::DUMMY,
        };
        let expr_id = alloc(&mut arena, ExprKind::FunctionExp(func_exp));

        let ty = infer_expr(&mut engine, &arena, expr_id);

        assert_eq!(ty, Idx::UNIT, "print should return unit");
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_function_exp_panic() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // panic(message: "oops")
        let message = alloc(&mut arena, ExprKind::String(ori_ir::Name::from_raw(100)));
        let props = arena.alloc_named_exprs([ori_ir::NamedExpr {
            name: name(1),
            value: message,
            span: Span::DUMMY,
        }]);

        let func_exp = ori_ir::FunctionExp {
            kind: ori_ir::FunctionExpKind::Panic,
            props,
            span: Span::DUMMY,
        };
        let expr_id = alloc(&mut arena, ExprKind::FunctionExp(func_exp));

        let ty = infer_expr(&mut engine, &arena, expr_id);

        assert_eq!(ty, Idx::NEVER, "panic should return never");
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_function_exp_todo() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // todo() - no properties required
        let props = arena.alloc_named_exprs(std::iter::empty());

        let func_exp = ori_ir::FunctionExp {
            kind: ori_ir::FunctionExpKind::Todo,
            props,
            span: Span::DUMMY,
        };
        let expr_id = alloc(&mut arena, ExprKind::FunctionExp(func_exp));

        let ty = infer_expr(&mut engine, &arena, expr_id);

        assert_eq!(ty, Idx::NEVER, "todo should return never");
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_function_exp_unreachable() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // unreachable()
        let props = arena.alloc_named_exprs(std::iter::empty());

        let func_exp = ori_ir::FunctionExp {
            kind: ori_ir::FunctionExpKind::Unreachable,
            props,
            span: Span::DUMMY,
        };
        let expr_id = alloc(&mut arena, ExprKind::FunctionExp(func_exp));

        let ty = infer_expr(&mut engine, &arena, expr_id);

        assert_eq!(ty, Idx::NEVER, "unreachable should return never");
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_function_exp_catch() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // catch(try: 42, catch: 0)
        let try_expr = alloc(&mut arena, ExprKind::Int(42));
        let catch_expr = alloc(&mut arena, ExprKind::Int(0));

        let props = arena.alloc_named_exprs([
            ori_ir::NamedExpr {
                name: name(1), // "try"
                value: try_expr,
                span: Span::DUMMY,
            },
            ori_ir::NamedExpr {
                name: name(2), // "catch"
                value: catch_expr,
                span: Span::DUMMY,
            },
        ]);

        let func_exp = ori_ir::FunctionExp {
            kind: ori_ir::FunctionExpKind::Catch,
            props,
            span: Span::DUMMY,
        };
        let expr_id = alloc(&mut arena, ExprKind::FunctionExp(func_exp));

        let ty = infer_expr(&mut engine, &arena, expr_id);

        assert_eq!(ty, Idx::INT, "catch should return int (unified type)");
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_infer_function_exp_timeout() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        let mut arena = ExprArena::new();

        // timeout(duration: ..., task: 42)
        let duration = alloc(&mut arena, ExprKind::Int(1000)); // milliseconds
        let task = alloc(&mut arena, ExprKind::Int(42));

        let props = arena.alloc_named_exprs([
            ori_ir::NamedExpr {
                name: name(1),
                value: duration,
                span: Span::DUMMY,
            },
            ori_ir::NamedExpr {
                name: name(2),
                value: task,
                span: Span::DUMMY,
            },
        ]);

        let func_exp = ori_ir::FunctionExp {
            kind: ori_ir::FunctionExpKind::Timeout,
            props,
            span: Span::DUMMY,
        };
        let expr_id = alloc(&mut arena, ExprKind::FunctionExp(func_exp));

        let ty = infer_expr(&mut engine, &arena, expr_id);

        // timeout returns Option<T>
        assert_eq!(
            engine.pool().tag(ty),
            Tag::Option,
            "timeout should return Option"
        );
        assert!(!engine.has_errors());
    }
}
