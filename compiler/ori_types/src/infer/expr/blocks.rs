//! Block, let, and lambda inference.

use ori_ir::{ExprArena, ExprId, ExprKind, Name, Span};
use rustc_hash::FxHashSet;

use super::super::InferEngine;
use super::{
    bind_pattern, check_expr, infer_expr, pattern_is_irrefutable, resolve_and_check_parsed_type,
};
use crate::type_error::TypeCheckError;
use crate::{ContextKind, Expected, ExpectedOrigin, Idx};

/// Collect the set of lexically-bound outer-scope names visible to the
/// current inference frame.
///
/// Delegates to [`InferEngine::collect_lexical_outer`], which subtracts the
/// module-scope snapshot (prelude imports, same-module function signatures,
/// trait-registered names) from the current environment's full name set.
/// The result is exactly the set of names the user introduced via lexical
/// binders (function params, enclosing `let`, `for`, `match` arm) between
/// the function body's entry and the current inference point.
///
/// This is the SSOT path `should_generalize` uses to feed
/// `body_captures_outer`'s `Ident` arm — keeping prelude refs like
/// `len(collection: xs)` out of the capture set while still flagging real
/// `let outer = 1; let f = x -> outer` captures.
pub(crate) fn collect_outer_vars(engine: &InferEngine<'_>) -> FxHashSet<Name> {
    engine.collect_lexical_outer()
}

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

                // Enter a rank-only scope for let-polymorphism. The surrounding block already pushed its env scope
                // at line 38 and the let binding MUST stay visible to
                // subsequent statements — pushing another env.child() here
                // would hide later bindings from earlier `let`s. Rank-only is
                // the right knob. All three let-generalization sites
                // (`infer_block` block-stmt let, `infer_let` ExprKind::Let,
                // `infer_try_stmt` try-block let) use `enter_rank_scope` for
                // this reason.
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

                    // Value Restriction: only non-capturing lambdas may be generalized.
                    // All other initializers (list literals, map literals, struct constructions,
                    // constants) are monomorphic — their Vars must stay Unbound so the
                    // Section 02 validator can surface E2005 on empty containers.
                    // Spec: docs/ori_lang/v2026/spec/14-expressions.md:1224-1228
                    maybe_generalize(engine, arena, *init, init_ty)
                };

                // Exit rank scope (but stay in block's binding scope)
                engine.exit_rank_scope();

                // Bind pattern to the block's scope.
                // The binding is visible to subsequent statements and the result.
                if let Err(reason) = pattern_is_irrefutable(engine, pat, final_ty) {
                    let err = TypeCheckError::refutable_pattern(stmt.span, reason);
                    engine.push_error(err);
                }
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
    // Enter a rank-only scope for let-polymorphism.
    // Matches the two sibling let-generalization sites (`infer_block` block-stmt
    // let and `infer_try_stmt` try-block let): we elevate the unification rank
    // so variables introduced while inferring `init` can be generalized, but
    // we do NOT push an env.child() — the binding is installed in the outer
    // env via `bind_pattern` below (after exit), so a child env push here
    // would be dead weight.
    engine.enter_rank_scope();

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

        // Value Restriction: only non-capturing lambdas may be generalized.
        // Spec: docs/ori_lang/v2026/spec/14-expressions.md:1224-1228
        maybe_generalize(engine, arena, init, init_ty)
    };

    // Exit the rank scope (rank goes back down); see enter_rank_scope comment
    // above for why this matches the block-stmt + try-stmt siblings.
    engine.exit_rank_scope();

    // Bind the pattern to the (possibly generalized) type in the outer env.
    if let Err(reason) = pattern_is_irrefutable(engine, pattern, final_ty) {
        let err = TypeCheckError::refutable_pattern(span, reason);
        engine.push_error(err);
    }
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

/// Returns `true` iff `init` is a non-capturing lambda whose type variables
/// may be safely generalized for let-polymorphism.
///
/// Only `ExprKind::Lambda` initializers with no free outer-scope variables
/// are generalizable. All other initializers — list/map literals, struct
/// constructions, constants, function calls — are monomorphic and MUST NOT
/// generalize their inferred type variables.
///
/// `outer_vars` is the set of lexically-bound names visible at the binding
/// site (populated by `collect_outer_vars`). It lets the `Ident` check in
/// `body_captures_outer` distinguish a real capture (`outer` bound by an
/// enclosing `let`) from a module-level reference (`len`, `print`, `@fn`,
/// `$const`), which is NOT a capture.
///
/// This is the SSOT for the Value Restriction policy. Every let-binding
/// generalization site in the type checker MUST call this function rather
/// than inlining equivalent logic.
pub(crate) fn should_generalize(
    arena: &ExprArena,
    init: ExprId,
    outer_vars: &FxHashSet<Name>,
) -> bool {
    match &arena.get_expr(init).kind {
        ExprKind::Lambda { params, body, .. } => {
            let param_names: Vec<Name> = arena.get_params(*params).iter().map(|p| p.name).collect();
            !body_captures_outer(arena, *body, &param_names, outer_vars)
        }
        _ => false,
    }
}

/// Apply the Value Restriction generalization decision to `ty` for an
/// initializer `init`. Returns `engine.generalize(ty)` iff `init` satisfies
/// [`should_generalize`] (a non-capturing lambda), otherwise returns `ty`
/// unchanged.
///
/// This is the SSOT for *applying* the decision, alongside
/// [`should_generalize`] which is the SSOT for *making* it. Every
/// let-binding generalization site in the type checker MUST call this
/// function — inlining the `if should_generalize(...) { generalize } else
/// { ty }` pattern at the call site would drift when the policy evolves.
///
/// The three live call sites are: `infer_block` (block-statement `let`),
/// `infer_let` (`ExprKind::Let` dispatch), and `infer_try_stmt` (try-block
/// `let`). Each calls this function; the policy check itself and the
/// lexical-outer collection are handled here.
///
/// **Important**: `init` is the *original* initializer expression, NOT a
/// derived type. In the try-block path, the caller unwraps the inferred
/// `Result`/`Option` type with `unwrap_result_or_option` and passes the
/// unwrapped `Idx` as `ty`, but the `init` it passes is still the user's
/// un-unwrapped expression — the unwrap changes the type, not the
/// expression kind, so the policy check still reads the source-level
/// `ExprKind`.
pub(crate) fn maybe_generalize(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    init: ExprId,
    ty: Idx,
) -> Idx {
    let outer_vars = collect_outer_vars(engine);
    if should_generalize(arena, init, &outer_vars) {
        engine.generalize(ty)
    } else {
        ty
    }
}

/// Check if a lambda body captures outer variables by scanning for
/// `Ident` nodes that are not in the parameter list.
///
/// **Soundness direction: conservative in the direction of rejecting
/// generalization.** Unknown or unhandled expression shapes are treated as
/// capturing — a false positive costs one missed polymorphic generalization;
/// a false negative silently generalizes a capturing lambda, whose bound
/// type variables then include outer captures, and downstream instantiation
/// produces wrong types (a type-soundness violation, not a detectable
/// codegen failure).
///
/// Leaf arms returning `false` cover only verified non-capturing shapes:
/// literals, `Unit`, `None`, module-level references (`Const`, `FunctionRef`),
/// and the `Error` poison placeholder. Every other shape either descends
/// into its children or, for shapes this function does not yet walk, is
/// absorbed by the `_ => true` wildcard.
#[expect(
    clippy::too_many_lines,
    reason = "exhaustive ExprKind dispatch — every shape with child \
              expressions must be walked for capture-analysis soundness; \
              splitting the match would require a parallel second match \
              that could drift out of sync with this one"
)]
#[expect(
    clippy::match_same_arms,
    reason = "`ExprKind::SelfRef` intentionally stays as its own arm: it \
              is a verified-true case (lambdas that reference `self` \
              genuinely capture the enclosing method's receiver), distinct \
              from the conservative `_ => true` default for unknown shapes. \
              Collapsing loses the semantic distinction and makes future \
              maintainers re-derive why `self` ends up captured"
)]
fn body_captures_outer(
    arena: &ExprArena,
    id: ExprId,
    param_names: &[Name],
    outer_vars: &FxHashSet<Name>,
) -> bool {
    if id == ExprId::INVALID {
        return false;
    }
    match &arena.get_expr(id).kind {
        // Leaves with no child expressions — provably cannot capture.
        ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Char(_)
        | ExprKind::Duration { .. }
        | ExprKind::Size { .. }
        | ExprKind::Unit
        | ExprKind::None
        | ExprKind::HashLength
        | ExprKind::Const(_)
        | ExprKind::FunctionRef(_)
        | ExprKind::TemplateFull(_)
        | ExprKind::Error => false,

        // A bare identifier is a capture iff it is not one of the lambda's
        // own params AND it is a lexically-bound outer name. Names that are
        // neither (prelude free functions, built-in constructors, type names
        // resolved via `ori_registry`) are module-level and do NOT count as
        // captures — their presence in a lambda body is orthogonal to
        // let-polymorphism soundness.
        ExprKind::Ident(n) => !param_names.contains(n) && outer_vars.contains(n),

        // `self` inside a lambda body references the enclosing method's
        // receiver — that is a capture of outer scope.
        ExprKind::SelfRef => true,

        // Nested lambdas: descend with the inner params added to the visible set.
        ExprKind::Lambda { params, body, .. } => {
            let mut all_params: Vec<Name> = param_names.to_vec();
            for p in arena.get_params(*params) {
                all_params.push(p.name);
            }
            body_captures_outer(arena, *body, &all_params, outer_vars)
        }

        // Single-child wrappers.
        ExprKind::Unary { operand, .. } => {
            body_captures_outer(arena, *operand, param_names, outer_vars)
        }
        ExprKind::Await(child)
        | ExprKind::Try(child)
        | ExprKind::Unsafe(child)
        | ExprKind::Ok(child)
        | ExprKind::Err(child)
        | ExprKind::Some(child) => body_captures_outer(arena, *child, param_names, outer_vars),
        ExprKind::Cast { expr, .. } => body_captures_outer(arena, *expr, param_names, outer_vars),
        ExprKind::Break { value, .. } | ExprKind::Continue { value, .. } => {
            body_captures_outer(arena, *value, param_names, outer_vars)
        }
        ExprKind::Let { init, .. } => body_captures_outer(arena, *init, param_names, outer_vars),
        ExprKind::Loop { body, .. } => body_captures_outer(arena, *body, param_names, outer_vars),
        ExprKind::While { cond, body, .. } => {
            body_captures_outer(arena, *cond, param_names, outer_vars)
                || body_captures_outer(arena, *body, param_names, outer_vars)
        }
        ExprKind::Field { receiver, .. } => {
            body_captures_outer(arena, *receiver, param_names, outer_vars)
        }

        // Two-child shapes.
        ExprKind::Binary { left, right, .. } => {
            body_captures_outer(arena, *left, param_names, outer_vars)
                || body_captures_outer(arena, *right, param_names, outer_vars)
        }
        ExprKind::Index { receiver, index } => {
            body_captures_outer(arena, *receiver, param_names, outer_vars)
                || body_captures_outer(arena, *index, param_names, outer_vars)
        }
        ExprKind::Assign { target, value } => {
            body_captures_outer(arena, *target, param_names, outer_vars)
                || body_captures_outer(arena, *value, param_names, outer_vars)
        }
        ExprKind::WithCapability { provider, body, .. } => {
            body_captures_outer(arena, *provider, param_names, outer_vars)
                || body_captures_outer(arena, *body, param_names, outer_vars)
        }

        // Three-child shapes.
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            body_captures_outer(arena, *cond, param_names, outer_vars)
                || body_captures_outer(arena, *then_branch, param_names, outer_vars)
                || body_captures_outer(arena, *else_branch, param_names, outer_vars)
        }
        ExprKind::Range {
            start, end, step, ..
        } => {
            body_captures_outer(arena, *start, param_names, outer_vars)
                || body_captures_outer(arena, *end, param_names, outer_vars)
                || body_captures_outer(arena, *step, param_names, outer_vars)
        }
        ExprKind::For {
            iter, guard, body, ..
        } => {
            body_captures_outer(arena, *iter, param_names, outer_vars)
                || body_captures_outer(arena, *guard, param_names, outer_vars)
                || body_captures_outer(arena, *body, param_names, outer_vars)
        }

        // Call: must walk BOTH func and args. The previous code checked only
        // `func`, leaving captures inside `args` undetected (F1+F7 finding).
        ExprKind::Call { func, args } => {
            body_captures_outer(arena, *func, param_names, outer_vars)
                || arena
                    .get_expr_list(*args)
                    .iter()
                    .any(|e| body_captures_outer(arena, *e, param_names, outer_vars))
        }

        // MethodCall: must walk BOTH receiver and args (same F1+F7 finding
        // applied to the method-dispatch path).
        ExprKind::MethodCall { receiver, args, .. } => {
            body_captures_outer(arena, *receiver, param_names, outer_vars)
                || arena
                    .get_expr_list(*args)
                    .iter()
                    .any(|e| body_captures_outer(arena, *e, param_names, outer_vars))
        }

        // List / Tuple literals: walk every element.
        ExprKind::List(range) | ExprKind::Tuple(range) => arena
            .get_expr_list(*range)
            .iter()
            .any(|e| body_captures_outer(arena, *e, param_names, outer_vars)),

        // Map literal: walk every key and value.
        ExprKind::Map(range) => arena.get_map_entries(*range).iter().any(|entry| {
            body_captures_outer(arena, entry.key, param_names, outer_vars)
                || body_captures_outer(arena, entry.value, param_names, outer_vars)
        }),

        // Struct literal: walk every supplied field value.
        ExprKind::Struct { fields, .. } => arena.get_field_inits(*fields).iter().any(|fi| {
            fi.value
                .is_some_and(|v| body_captures_outer(arena, v, param_names, outer_vars))
        }),

        // Match: scrutinee + each arm's guard and body. Pattern bindings are
        // not added to the visible set here — false positives (flagging an
        // arm-body reference to a pattern-bound name as capture) are
        // acceptable under the conservative-direction rule; false negatives
        // are not.
        ExprKind::Match { scrutinee, arms } => {
            if body_captures_outer(arena, *scrutinee, param_names, outer_vars) {
                return true;
            }
            arena.get_arms(*arms).iter().any(|arm| {
                arm.guard
                    .is_some_and(|g| body_captures_outer(arena, g, param_names, outer_vars))
                    || body_captures_outer(arena, arm.body, param_names, outer_vars)
            })
        }

        // Block: walk every statement's carried expression plus the result.
        ExprKind::Block { stmts, result } => {
            if body_captures_outer(arena, *result, param_names, outer_vars) {
                return true;
            }
            arena
                .get_stmt_range(*stmts)
                .iter()
                .any(|stmt| match &stmt.kind {
                    ori_ir::StmtKind::Expr(e) => {
                        body_captures_outer(arena, *e, param_names, outer_vars)
                    }
                    ori_ir::StmtKind::Let { init, .. } => {
                        body_captures_outer(arena, *init, param_names, outer_vars)
                    }
                })
        }

        // Named-argument call forms (sugar eliminated by canon but present
        // during type checking — when `body_captures_outer` actually runs).
        // Must walk func/receiver AND every named arg value.
        ExprKind::CallNamed { func, args } => {
            body_captures_outer(arena, *func, param_names, outer_vars)
                || arena
                    .get_call_args(*args)
                    .iter()
                    .any(|a| body_captures_outer(arena, a.value, param_names, outer_vars))
        }
        ExprKind::MethodCallNamed { receiver, args, .. } => {
            body_captures_outer(arena, *receiver, param_names, outer_vars)
                || arena
                    .get_call_args(*args)
                    .iter()
                    .any(|a| body_captures_outer(arena, a.value, param_names, outer_vars))
        }

        // Spread-aware literal forms: each element/entry either carries a
        // plain expression or a `...expr` spread; both contribute captures.
        ExprKind::ListWithSpread(range) => {
            arena.get_list_elements(*range).iter().any(|el| match el {
                ori_ir::ast::ListElement::Expr { expr, .. }
                | ori_ir::ast::ListElement::Spread { expr, .. } => {
                    body_captures_outer(arena, *expr, param_names, outer_vars)
                }
            })
        }
        ExprKind::MapWithSpread(range) => {
            arena.get_map_elements(*range).iter().any(|el| match el {
                ori_ir::ast::MapElement::Entry(entry) => {
                    body_captures_outer(arena, entry.key, param_names, outer_vars)
                        || body_captures_outer(arena, entry.value, param_names, outer_vars)
                }
                ori_ir::ast::MapElement::Spread { expr, .. } => {
                    body_captures_outer(arena, *expr, param_names, outer_vars)
                }
            })
        }
        ExprKind::StructWithSpread { fields, .. } => arena
            .get_struct_lit_fields(*fields)
            .iter()
            .any(|field| match field {
                ori_ir::ast::StructLitField::Field(fi) => fi
                    .value
                    .is_some_and(|v| body_captures_outer(arena, v, param_names, outer_vars)),
                ori_ir::ast::StructLitField::Spread { expr, .. } => {
                    body_captures_outer(arena, *expr, param_names, outer_vars)
                }
            }),

        // Template literals with interpolations: walk every part's expr.
        ExprKind::TemplateLiteral { parts, .. } => arena
            .get_template_parts(*parts)
            .iter()
            .any(|part| body_captures_outer(arena, part.expr, param_names, outer_vars)),

        // Remaining arena-indexed shapes this function does not yet decode
        // (`FunctionSeq`, `FunctionExp` — the recurse/parallel/nursery/etc.
        // combinator families). These are conservatively treated as
        // capturing. A false positive here costs one missed polymorphic
        // generalization on an uncommon shape; a false negative would
        // silently generalize a capturing lambda — a type-soundness bug.
        _ => true,
    }
}
