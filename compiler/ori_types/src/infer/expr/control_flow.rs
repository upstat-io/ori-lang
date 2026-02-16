//! Control flow inference — if, match, for, loop, break, continue.

use ori_ir::{ExprArena, ExprId, Name, Span};

use super::super::InferEngine;
use super::{infer_expr, lookup_struct_field_types};
use crate::{
    ContextKind, Expected, ExpectedOrigin, Idx, PatternKey, SequenceKind, Tag, TypeCheckError,
    VariantFields,
};

/// Infer the type of an if expression.
pub(crate) fn infer_if(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    cond: ExprId,
    then_branch: ExprId,
    else_branch: ExprId,
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

    if else_branch.is_present() {
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
        let else_ty = infer_expr(engine, arena, else_branch);
        let _ = engine.check_type(else_ty, &expected, arena.get_expr(else_branch).span);
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

/// Infer the type of a match expression.
///
/// Match inference follows these steps:
/// 1. Infer the scrutinee type
/// 2. For each arm: check pattern against scrutinee, check guard is bool, infer body
/// 3. Unify all arm body types
/// 4. Return the unified type (or never if no arms)
pub(crate) fn infer_match(
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
        #[expect(clippy::cast_possible_truncation, reason = "arm index fits in u32")]
        let arm_key = PatternKey::Arm(arms.start + i as u32);
        check_match_pattern(engine, arena, &arm.pattern, scrutinee_ty, arm_key, arm.span);
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
///
/// The `pattern_key` identifies this pattern for resolution lookup. For top-level
/// arm patterns it's `PatternKey::Arm(arms.start + i)`, for nested patterns it's
/// `PatternKey::Nested(match_pattern_id.raw())`.
pub(crate) fn check_match_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    pattern: &ori_ir::MatchPattern,
    expected_ty: Idx,
    pattern_key: PatternKey,
    span: Span,
) {
    use ori_ir::MatchPattern;

    match pattern {
        // Wildcard matches anything
        MatchPattern::Wildcard => {}

        // Binding: either a variable binding or an ambiguous unit variant.
        //
        // The parser can't distinguish `Pending` (unit variant) from `x` (binding)
        // without type context. We resolve this here by checking if the name is a
        // unit variant of the scrutinee's enum type.
        MatchPattern::Binding(name) => {
            let resolved = engine.resolve(expected_ty);
            let tag = engine.pool().tag(resolved);

            // Check if this name is a unit variant of the scrutinee's enum type
            let is_unit_variant = if matches!(tag, Tag::Named | Tag::Applied) {
                let scrutinee_name = if tag == Tag::Named {
                    engine.pool().named_name(resolved)
                } else {
                    engine.pool().applied_name(resolved)
                };
                engine.type_registry().and_then(|reg| {
                    let (type_entry, variant_def) = reg.lookup_variant_def(*name)?;
                    // CRITICAL: variant must belong to the scrutinee's type, not any enum
                    if type_entry.name != scrutinee_name {
                        return None;
                    }
                    if !variant_def.fields.is_unit() {
                        return None;
                    }
                    let (_, variant_idx) = reg.lookup_variant(*name)?;
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "enums are limited to 256 variants"
                    )]
                    Some((type_entry.name, variant_idx as u8))
                })
            } else {
                None
            };

            if let Some((type_name, variant_index)) = is_unit_variant {
                engine.record_pattern_resolution(
                    pattern_key,
                    crate::PatternResolution::UnitVariant {
                        type_name,
                        variant_index,
                    },
                );
                // Do NOT bind name — it's a constructor, not a variable
            } else {
                engine.env_mut().bind(*name, expected_ty);
            }
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
                    // Ok(x) or Err(e) pattern - use variant name to select inner type
                    let variant_str = engine.lookup_name(*name);
                    match variant_str {
                        Some("Err") => vec![engine.pool().result_err(resolved)],
                        _ => vec![engine.pool().result_ok(resolved)],
                    }
                }
                Tag::Named | Tag::Applied => {
                    // User-defined enum: look up variant field types from TypeRegistry,
                    // substituting any generic type parameters with concrete types from
                    // the scrutinee's type arguments.
                    let result = engine.type_registry().and_then(|reg| {
                        let (type_entry, variant_def) = reg.lookup_variant_def(*name)?;
                        let field_types: Vec<Idx> = match &variant_def.fields {
                            VariantFields::Unit => vec![],
                            VariantFields::Tuple(types) => types.clone(),
                            VariantFields::Record(fields) => fields.iter().map(|f| f.ty).collect(),
                        };
                        Some((type_entry.type_params.clone(), field_types))
                    });

                    match result {
                        Some((type_params, field_types)) if type_params.is_empty() => {
                            // Non-generic enum: field types are concrete, use directly
                            field_types
                        }
                        Some((type_params, field_types)) => {
                            // Generic enum: substitute type parameters with concrete
                            // type arguments from the scrutinee.
                            // e.g., scrutinee `MyResult<int, str>` -> T=int, E=str
                            let type_args = if tag == Tag::Applied {
                                engine.pool().applied_args(resolved)
                            } else {
                                vec![]
                            };

                            if type_args.len() == type_params.len() {
                                // Build param->arg mapping and substitute
                                let substituted: Vec<Idx> = field_types
                                    .iter()
                                    .map(|&ft| {
                                        substitute_type_params(engine, ft, &type_params, &type_args)
                                    })
                                    .collect();
                                substituted
                            } else {
                                // Mismatch between expected and actual type args — use
                                // fresh variables as fallback
                                let inner_ids = arena.get_match_pattern_list(*inner);
                                inner_ids.iter().map(|_| engine.fresh_var()).collect()
                            }
                        }
                        None => {
                            // Variant not found — fall back to fresh variables
                            let inner_ids = arena.get_match_pattern_list(*inner);
                            inner_ids.iter().map(|_| engine.fresh_var()).collect()
                        }
                    }
                }
                _ => {
                    // Unknown tag — fall back to fresh variables
                    let inner_ids = arena.get_match_pattern_list(*inner);
                    inner_ids.iter().map(|_| engine.fresh_var()).collect()
                }
            };

            // Check inner patterns
            let inner_ids = arena.get_match_pattern_list(*inner);
            for (inner_id, inner_ty) in inner_ids.iter().zip(inner_types.iter()) {
                let inner_pattern = arena.get_match_pattern(*inner_id);
                let nested_key = PatternKey::Nested(inner_id.raw());
                check_match_pattern(engine, arena, inner_pattern, *inner_ty, nested_key, span);
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
                        span,
                        elem_types.len(),
                        inner_ids.len(),
                        crate::ArityMismatchKind::Pattern,
                    ));
                    return;
                }

                // Check each element
                for (inner_id, elem_ty) in inner_ids.iter().zip(elem_types.iter()) {
                    let inner_pattern = arena.get_match_pattern(*inner_id);
                    let nested_key = PatternKey::Nested(inner_id.raw());
                    check_match_pattern(engine, arena, inner_pattern, *elem_ty, nested_key, span);
                }
            } else if resolved != Idx::ERROR {
                // Not a tuple type
                engine.push_error(TypeCheckError::mismatch(
                    span,
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
                    let nested_key = PatternKey::Nested(inner_id.raw());
                    check_match_pattern(engine, arena, inner_pattern, elem_ty, nested_key, span);
                }

                // Bind rest pattern to list type
                if let Some(rest_name) = rest {
                    engine.env_mut().bind(*rest_name, resolved);
                }
            } else if resolved != Idx::ERROR {
                // Not a list type
                engine.push_error(TypeCheckError::mismatch(
                    span,
                    expected_ty,
                    resolved,
                    vec![],
                    crate::ErrorContext::new(ContextKind::PatternMatch {
                        pattern_kind: "list",
                    }),
                ));
            }
        }

        // Struct pattern: check field types against registry
        MatchPattern::Struct { fields, .. } => {
            let resolved = engine.resolve(expected_ty);
            let field_type_map = match engine.pool().tag(resolved) {
                Tag::Named => {
                    let type_name = engine.pool().named_name(resolved);
                    lookup_struct_field_types(engine, type_name, None)
                }
                Tag::Applied => {
                    let type_name = engine.pool().applied_name(resolved);
                    let type_args = engine.pool().applied_args(resolved);
                    lookup_struct_field_types(engine, type_name, Some(&type_args))
                }
                _ => None,
            };

            for (name, inner_pattern) in fields {
                let field_ty = field_type_map
                    .as_ref()
                    .and_then(|m| m.get(name).copied())
                    .unwrap_or_else(|| engine.fresh_var());
                if let Some(inner_id) = inner_pattern {
                    let inner = arena.get_match_pattern(*inner_id);
                    let nested_key = PatternKey::Nested(inner_id.raw());
                    check_match_pattern(engine, arena, inner, field_ty, nested_key, span);
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
                let nested_key = PatternKey::Nested(alt_id.raw());
                check_match_pattern(engine, arena, alt_pattern, expected_ty, nested_key, span);
            }
        }

        // At pattern: bind name and check inner pattern
        MatchPattern::At {
            name,
            pattern: inner_id,
        } => {
            engine.env_mut().bind(*name, expected_ty);
            let inner_pattern = arena.get_match_pattern(*inner_id);
            let nested_key = PatternKey::Nested(inner_id.raw());
            check_match_pattern(engine, arena, inner_pattern, expected_ty, nested_key, span);
        }
    }
}

/// Substitute generic type parameters in a field type with concrete type arguments.
///
/// Given a field type like `Named("T")` and a mapping `[T] -> [int]`, returns `int`.
/// For compound types (lists, tuples, functions, applied types), recurses into children.
/// Non-parameterized types (primitives, error, etc.) are returned unchanged.
pub(crate) fn substitute_type_params(
    engine: &mut InferEngine<'_>,
    field_ty: Idx,
    type_params: &[ori_ir::Name],
    type_args: &[Idx],
) -> Idx {
    let resolved = engine.resolve(field_ty);
    let tag = engine.pool().tag(resolved);

    match tag {
        Tag::Named => {
            // Check if this named type is one of the type parameters
            let name = engine.pool().named_name(resolved);
            for (i, &param_name) in type_params.iter().enumerate() {
                if name == param_name {
                    return type_args[i];
                }
            }
            // Not a type parameter — return as-is (concrete named type)
            resolved
        }
        Tag::Applied => {
            // Recurse into applied type arguments: e.g., List<T> -> List<int>
            let app_name = engine.pool().applied_name(resolved);
            let args = engine.pool().applied_args(resolved);
            let substituted_args: Vec<Idx> = args
                .iter()
                .map(|&arg| substitute_type_params(engine, arg, type_params, type_args))
                .collect();
            engine.pool_mut().applied(app_name, &substituted_args)
        }
        Tag::List => {
            let elem = engine.pool().list_elem(resolved);
            let sub_elem = substitute_type_params(engine, elem, type_params, type_args);
            engine.pool_mut().list(sub_elem)
        }
        Tag::Tuple => {
            let elems = engine.pool().tuple_elems(resolved);
            let sub_elems: Vec<Idx> = elems
                .iter()
                .map(|&e| substitute_type_params(engine, e, type_params, type_args))
                .collect();
            engine.pool_mut().tuple(&sub_elems)
        }
        Tag::Function => {
            let params = engine.pool().function_params(resolved);
            let ret = engine.pool().function_return(resolved);
            let sub_params: Vec<Idx> = params
                .iter()
                .map(|&p| substitute_type_params(engine, p, type_params, type_args))
                .collect();
            let sub_ret = substitute_type_params(engine, ret, type_params, type_args);
            engine.pool_mut().function(&sub_params, sub_ret)
        }
        Tag::Option => {
            let inner = engine.pool().option_inner(resolved);
            let sub_inner = substitute_type_params(engine, inner, type_params, type_args);
            engine.pool_mut().option(sub_inner)
        }
        Tag::Result => {
            let ok = engine.pool().result_ok(resolved);
            let err = engine.pool().result_err(resolved);
            let sub_ok = substitute_type_params(engine, ok, type_params, type_args);
            let sub_err = substitute_type_params(engine, err, type_params, type_args);
            engine.pool_mut().result(sub_ok, sub_err)
        }
        Tag::Map => {
            let key = engine.pool().map_key(resolved);
            let val = engine.pool().map_value(resolved);
            let sub_key = substitute_type_params(engine, key, type_params, type_args);
            let sub_val = substitute_type_params(engine, val, type_params, type_args);
            engine.pool_mut().map(sub_key, sub_val)
        }
        // Primitives and other leaf types — no substitution needed
        _ => resolved,
    }
}

/// Substitute type parameters using a pre-built map of (Name, Idx) pairs.
///
/// This is a convenience wrapper around `substitute_type_params` that accepts
/// a map representation rather than parallel arrays.
pub(crate) fn substitute_type_params_with_map(
    engine: &mut InferEngine<'_>,
    field_ty: Idx,
    subst_map: &[(Name, Idx)],
) -> Idx {
    if subst_map.is_empty() {
        return field_ty;
    }
    let type_params: Vec<Name> = subst_map.iter().map(|(n, _)| *n).collect();
    let type_args: Vec<Idx> = subst_map.iter().map(|(_, i)| *i).collect();
    substitute_type_params(engine, field_ty, &type_params, &type_args)
}

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
pub(crate) fn infer_for(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    binding: Name,
    iter: ExprId,
    guard: ExprId,
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
        Tag::Iterator => engine.pool().iterator_elem(resolved_iter),
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
        Tag::Option => {
            // Option<T> iterates as 0-or-1 element of type T
            engine.pool().option_inner(resolved_iter)
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
    if guard.is_present() {
        let guard_ty = infer_expr(engine, arena, guard);
        let expected = Expected {
            ty: Idx::BOOL,
            origin: ExpectedOrigin::Context {
                span: arena.get_expr(guard).span,
                kind: ContextKind::LoopCondition,
            },
        };
        let _ = engine.check_type(guard_ty, &expected, arena.get_expr(guard).span);
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
pub(crate) fn infer_loop(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    body: ExprId,
    _span: Span,
) -> Idx {
    // Create a fresh type variable for the loop's result (determined by break values)
    let break_ty = engine.fresh_var();
    engine.push_loop_break_type(break_ty);

    // Enter scope for loop
    engine.enter_scope();

    // Infer body type (break expressions unify their value with break_ty)
    engine.push_context(ContextKind::LoopBody);
    let _body_ty = infer_expr(engine, arena, body);
    engine.pop_context();

    // Exit loop scope
    engine.exit_scope();
    engine.pop_loop_break_type();

    // Resolve the break type — if no break was encountered, the variable
    // stays unresolved (infinite loop returns Never). If breaks exist,
    // it unifies to their value type.
    let resolved = engine.resolve(break_ty);
    if engine.pool().tag(resolved) == Tag::Var {
        // No break was encountered — this is an infinite loop (returns Never).
        // Note: `break` without a value unifies break_ty with Unit, so
        // Tag::Var here means truly no break exists in the loop body.
        Idx::NEVER
    } else {
        resolved
    }
}

/// Infer the type of a break expression.
pub(crate) fn infer_break(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    value: ExprId,
    _span: Span,
) -> Idx {
    // Infer the break value's type (unit if no value)
    let value_ty = if value.is_present() {
        infer_expr(engine, arena, value)
    } else {
        Idx::UNIT
    };

    // Unify with the enclosing loop's break type variable
    if let Some(loop_break_ty) = engine.current_loop_break_type() {
        let _ = engine.unify_types(value_ty, loop_break_ty);
    }

    // Break itself is a diverging expression (control transfers to loop exit)
    Idx::NEVER
}

/// Infer the type of a continue expression.
pub(crate) fn infer_continue(
    _engine: &mut InferEngine<'_>,
    _arena: &ExprArena,
    _value: ExprId,
    _span: Span,
) -> Idx {
    Idx::NEVER
}
