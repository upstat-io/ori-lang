//! Lambda inference, capture analysis, and Value Restriction policy.

use ori_ir::{ExprArena, ExprId, ExprKind, Name, Span};
use rustc_hash::FxHashSet;

use crate::registry::burden_compose::closure::compose_closure_burden_spec;
use crate::{ContextKind, Expected, ExpectedOrigin, Idx};

use super::super::InferEngine;
use super::{infer_expr, resolve_and_check_parsed_type};

/// Collect the set of lexically-bound outer-scope names visible to the
/// current inference frame.
///
/// Delegates to [`InferEngine::collect_lexical_outer`], which subtracts the
/// module-scope snapshot (prelude imports, same-module function signatures,
/// trait-registered names) from the current environment's full name set.
/// The result is exactly the set of names the user introduced via lexical
/// binders (function params, enclosing `let`, `for`, `match` arm) between
/// the function body's entry and the current inference point.
fn collect_outer_vars(engine: &InferEngine<'_>) -> FxHashSet<Name> {
    engine.collect_lexical_outer()
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
    engine.enter_scope();

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

    engine.exit_scope();

    let closure_idx = engine.infer_function(&param_types, body_ty);

    // Register a conservative logical callable identity so the AIMS burden
    // walk accounts for closure ownership on every execution path. Exact
    // capture topology is frozen later from realized closure sites; a physical
    // projection may erase a non-capturing environment only from that proof.
    // This registration selects no storage, header, or counter mechanism.
    let closure_burden = compose_closure_burden_spec(closure_idx, &[], &[]);
    engine.record_composed_burden(closure_idx, closure_burden);

    closure_idx
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
/// generalization site must call this function rather than inlining
/// equivalent logic.
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
/// initializer `init`. Returns `engine.generalize(ty)` if `init` satisfies
/// [`should_generalize`] (a non-capturing lambda), otherwise returns `ty`
/// unchanged.
///
/// This is the SSOT for applying the decision, alongside [`should_generalize`]
/// which is the SSOT for making it. Generalization sites call this function
/// to ensure policy consistency when the policy evolves.
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
/// type variables then include outer captures, and subsequent instantiation
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

        ExprKind::Call { func, args } => {
            body_captures_outer(arena, *func, param_names, outer_vars)
                || arena
                    .get_expr_list(*args)
                    .iter()
                    .any(|e| body_captures_outer(arena, *e, param_names, outer_vars))
        }

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

        // Why: Pattern bindings are not added to the visible set; conservative false positives are acceptable.
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
