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

    // `?` in a lambda returns from that lambda, not from a `try {}` enclosing
    // the lambda expression. Nested try blocks pushed while inferring the body
    // still collect their own propagation observations above this barrier.
    engine.push_try_boundary_barrier();
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
    engine.pop_try_boundary_barrier();

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
/// **Important**: `init` is the original initializer expression, not a
/// derived type. Generalization policy is based on source-level expression
/// shape even when inference has resolved or unified `ty`.
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

/// Returns whether a lambda body references a lexically bound outer name.
///
/// Unknown shapes conservatively count as captures to preserve type soundness.
fn body_captures_outer(
    arena: &ExprArena,
    id: ExprId,
    param_names: &[Name],
    outer_vars: &FxHashSet<Name>,
) -> bool {
    CaptureContext {
        arena,
        param_names,
        outer_vars,
    }
    .captures(id)
}

struct CaptureContext<'a> {
    arena: &'a ExprArena,
    param_names: &'a [Name],
    outer_vars: &'a FxHashSet<Name>,
}

impl CaptureContext<'_> {
    fn captures(&self, id: ExprId) -> bool {
        if id == ExprId::INVALID {
            return false;
        }

        let kind = &self.arena.get_expr(id).kind;
        if is_capture_free_leaf(kind) {
            return false;
        }

        self.captures_direct_child(kind)
            .or_else(|| self.captures_call_or_collection(kind))
            .or_else(|| self.captures_structured(kind))
            .or_else(|| self.captures_spread(kind))
            .unwrap_or(true)
    }

    fn captures_direct_child(&self, kind: &ExprKind) -> Option<bool> {
        let captured = match kind {
            ExprKind::Ident(name) => {
                !self.param_names.contains(name) && self.outer_vars.contains(name)
            }
            ExprKind::SelfRef => true,
            ExprKind::Lambda { params, body, .. } => {
                let mut all_params = self.param_names.to_vec();
                all_params.extend(self.arena.get_params(*params).iter().map(|p| p.name));
                CaptureContext {
                    arena: self.arena,
                    param_names: &all_params,
                    outer_vars: self.outer_vars,
                }
                .captures(*body)
            }
            ExprKind::Unary { operand: child, .. }
            | ExprKind::Await(child)
            | ExprKind::Try(child)
            | ExprKind::Unsafe(child)
            | ExprKind::Ok(child)
            | ExprKind::Err(child)
            | ExprKind::Some(child)
            | ExprKind::Cast { expr: child, .. }
            | ExprKind::Break { value: child, .. }
            | ExprKind::Continue { value: child, .. }
            | ExprKind::Let { init: child, .. }
            | ExprKind::Loop { body: child, .. }
            | ExprKind::Field {
                receiver: child, ..
            } => self.captures(*child),
            ExprKind::Binary {
                left: first,
                right: second,
                ..
            }
            | ExprKind::Index {
                receiver: first,
                index: second,
            }
            | ExprKind::Assign {
                target: first,
                value: second,
            }
            | ExprKind::WithCapability {
                provider: first,
                body: second,
                ..
            }
            | ExprKind::While {
                cond: first,
                body: second,
                ..
            } => self.captures_all([*first, *second]),
            ExprKind::If {
                cond: first,
                then_branch: second,
                else_branch: third,
            }
            | ExprKind::Range {
                start: first,
                end: second,
                step: third,
                ..
            }
            | ExprKind::For {
                iter: first,
                guard: second,
                body: third,
                ..
            } => self.captures_all([*first, *second, *third]),
            _ => return None,
        };
        Some(captured)
    }

    fn captures_call_or_collection(&self, kind: &ExprKind) -> Option<bool> {
        let captured = match kind {
            ExprKind::Call { func: head, args }
            | ExprKind::MethodCall {
                receiver: head,
                args,
                ..
            } => self.captures(*head) || self.captures_exprs(self.arena.get_expr_list(*args)),
            ExprKind::List(range) | ExprKind::Tuple(range) => {
                self.captures_exprs(self.arena.get_expr_list(*range))
            }
            ExprKind::Map(range) => self
                .arena
                .get_map_entries(*range)
                .iter()
                .any(|entry| self.captures(entry.key) || self.captures(entry.value)),
            ExprKind::Struct { fields, .. } => self
                .arena
                .get_field_inits(*fields)
                .iter()
                .any(|field| field.value.is_some_and(|value| self.captures(value))),
            _ => return None,
        };
        Some(captured)
    }

    fn captures_structured(&self, kind: &ExprKind) -> Option<bool> {
        let captured = match kind {
            ExprKind::Match { scrutinee, arms } => {
                self.captures(*scrutinee)
                    || self.arena.get_arms(*arms).iter().any(|arm| {
                        arm.guard.is_some_and(|guard| self.captures(guard))
                            || self.captures(arm.body)
                    })
            }
            ExprKind::Block { stmts, result } => {
                self.captures(*result)
                    || self
                        .arena
                        .get_stmt_range(*stmts)
                        .iter()
                        .any(|stmt| match &stmt.kind {
                            ori_ir::StmtKind::Expr(expr) => self.captures(*expr),
                            ori_ir::StmtKind::Let { init, .. } => self.captures(*init),
                        })
            }
            ExprKind::CallNamed { func: head, args }
            | ExprKind::MethodCallNamed {
                receiver: head,
                args,
                ..
            } => {
                self.captures(*head)
                    || self
                        .arena
                        .get_call_args(*args)
                        .iter()
                        .any(|arg| self.captures(arg.value))
            }
            _ => return None,
        };
        Some(captured)
    }

    fn captures_spread(&self, kind: &ExprKind) -> Option<bool> {
        let captured = match kind {
            ExprKind::ListWithSpread(range) => {
                self.arena
                    .get_list_elements(*range)
                    .iter()
                    .any(|element| match element {
                        ori_ir::ast::ListElement::Expr { expr, .. }
                        | ori_ir::ast::ListElement::Spread { expr, .. } => self.captures(*expr),
                    })
            }
            ExprKind::MapWithSpread(range) => {
                self.arena
                    .get_map_elements(*range)
                    .iter()
                    .any(|element| match element {
                        ori_ir::ast::MapElement::Entry(entry) => {
                            self.captures(entry.key) || self.captures(entry.value)
                        }
                        ori_ir::ast::MapElement::Spread { expr, .. } => self.captures(*expr),
                    })
            }
            ExprKind::StructWithSpread { fields, .. } => self
                .arena
                .get_struct_lit_fields(*fields)
                .iter()
                .any(|field| match field {
                    ori_ir::ast::StructLitField::Field(init) => {
                        init.value.is_some_and(|value| self.captures(value))
                    }
                    ori_ir::ast::StructLitField::Spread { expr, .. } => self.captures(*expr),
                }),
            ExprKind::TemplateLiteral { parts, .. } => self
                .arena
                .get_template_parts(*parts)
                .iter()
                .any(|part| self.captures(part.expr)),
            _ => return None,
        };
        Some(captured)
    }

    fn captures_all<const N: usize>(&self, ids: [ExprId; N]) -> bool {
        ids.into_iter().any(|id| self.captures(id))
    }

    fn captures_exprs(&self, ids: &[ExprId]) -> bool {
        ids.iter().any(|id| self.captures(*id))
    }
}

fn is_capture_free_leaf(kind: &ExprKind) -> bool {
    matches!(
        kind,
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
            | ExprKind::Error
    )
}
