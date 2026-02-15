//! Function sequence and check desugaring — `FunctionSeq`, seq bindings, pre/post checks.
//!
//! Handles lowering of `FunctionSeq` variants (Run, Try, Match, `ForPattern`)
//! and the desugaring of pre-check and post-check assertions into conditional
//! panic expressions.

use ori_ir::canon::{CanBindingPattern, CanExpr, CanId, CanNamedExpr, CanRange};
use ori_ir::{FunctionExpKind, Name, Span, TypeId, UnaryOp};

use super::Lowerer;

impl Lowerer<'_> {
    // FunctionSeq Desugaring

    /// Lower a `FunctionSeq` (`ExprArena` side-table) into primitive `CanExpr` nodes.
    ///
    /// Each `FunctionSeq` variant is desugared:
    /// - `Run { bindings, result }` → `Block { stmts, result }`
    /// - `Try { bindings, result }` → `Block` with Try-wrapped bindings
    /// - `Match { scrutinee, arms }` → `Match` with decision tree
    /// - `ForPattern { over, map, arm, default }` → `For` with match body
    pub(super) fn lower_function_seq(
        &mut self,
        seq_id: ori_ir::FunctionSeqId,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let seq = self.src.get_function_seq(seq_id).clone();
        match seq {
            ori_ir::FunctionSeq::Run {
                pre_checks,
                bindings,
                result,
                post_checks,
                ..
            } => self.lower_run_seq(pre_checks, bindings, result, post_checks, span, ty),
            ori_ir::FunctionSeq::Try {
                bindings, result, ..
            } => {
                let stmts = self.lower_seq_bindings_try(bindings);
                let result = self.lower_expr(result);
                self.push(CanExpr::Block { stmts, result }, span, ty)
            }
            ori_ir::FunctionSeq::Match {
                scrutinee, arms, ..
            } => self.lower_match(scrutinee, arms, span, ty),
            ori_ir::FunctionSeq::ForPattern {
                over,
                map,
                arm,
                default: _,
                ..
            } => {
                // Desugar ForPattern into a For expression.
                // The arm's pattern becomes a match inside the for body.
                let iter = self.lower_expr(over);

                // If there's a map transform, emit Error — ForPattern map
                // semantics are not fully specified yet. Emitting a MethodCall
                // with a wrong method name (e.g., "concat") would cause
                // backends to dispatch incorrectly.
                let iter = if map.is_some() {
                    self.push(CanExpr::Error, span, ty)
                } else {
                    iter
                };

                // Lower the arm body directly.
                // Note: we do NOT lower the default expression — ForPattern
                // default handling is deferred. Lowering it would allocate
                // orphaned nodes in the arena.
                let body = self.lower_expr(arm.body);

                // Use a simple for with the binding from the arm pattern.
                let binding = match &arm.pattern {
                    ori_ir::MatchPattern::Binding(name) => *name,
                    // Wildcard and complex patterns: simplified for now
                    _ => Name::EMPTY,
                };

                let guard = arm.guard.map_or(CanId::INVALID, |g| self.lower_expr(g));

                self.push(
                    CanExpr::For {
                        label: Name::EMPTY,
                        binding,
                        iter,
                        guard,
                        body,
                        is_yield: true,
                    },
                    span,
                    ty,
                )
            }
        }
    }

    // Seq Binding Lowering

    /// Lower seq bindings (Run variant) to block statements.
    pub(super) fn lower_seq_bindings(&mut self, range: ori_ir::SeqBindingRange) -> CanRange {
        let bindings = self.src.get_seq_bindings(range);
        if bindings.is_empty() {
            return CanRange::EMPTY;
        }

        let bindings: Vec<_> = bindings.to_vec();

        // Lower all bindings before building the expr list to avoid
        // interleaving with nested start/push/finish cycles.
        let lowered: Vec<CanId> = bindings
            .iter()
            .map(|binding| match binding {
                ori_ir::SeqBinding::Let {
                    pattern,
                    ty: _,
                    value,
                    mutable,
                    span,
                } => {
                    let init = self.lower_expr(*value);
                    let pattern = self.lower_binding_pattern(*pattern);
                    self.push(
                        CanExpr::Let {
                            pattern,
                            init,
                            mutable: *mutable,
                        },
                        *span,
                        TypeId::UNIT,
                    )
                }
                ori_ir::SeqBinding::Stmt { expr, .. } => self.lower_expr(*expr),
            })
            .collect();

        let start = self.arena.start_expr_list();
        for can_id in lowered {
            self.arena.push_expr_list_item(can_id);
        }
        self.arena.finish_expr_list(start)
    }

    /// Lower seq bindings (Try variant) — each binding wrapped in Try.
    fn lower_seq_bindings_try(&mut self, range: ori_ir::SeqBindingRange) -> CanRange {
        let bindings = self.src.get_seq_bindings(range);
        if bindings.is_empty() {
            return CanRange::EMPTY;
        }

        let bindings: Vec<_> = bindings.to_vec();

        // Lower all bindings before building the expr list.
        let lowered: Vec<CanId> = bindings
            .iter()
            .map(|binding| match binding {
                ori_ir::SeqBinding::Let {
                    pattern,
                    ty: _,
                    value,
                    mutable,
                    span,
                } => {
                    let value = self.lower_expr(*value);
                    let tried_value = self.push(CanExpr::Try(value), *span, TypeId::ERROR);
                    let pattern = self.lower_binding_pattern(*pattern);
                    self.push(
                        CanExpr::Let {
                            pattern,
                            init: tried_value,
                            mutable: *mutable,
                        },
                        *span,
                        TypeId::UNIT,
                    )
                }
                ori_ir::SeqBinding::Stmt { expr, span } => {
                    let value = self.lower_expr(*expr);
                    self.push(CanExpr::Try(value), *span, TypeId::ERROR)
                }
            })
            .collect();

        let start = self.arena.start_expr_list();
        for can_id in lowered {
            self.arena.push_expr_list_item(can_id);
        }
        self.arena.finish_expr_list(start)
    }

    // Pre/Post Check Desugaring

    /// Lower a `Run` sequence with pre/post checks.
    ///
    /// Desugars per the checks proposal:
    /// - Pre-checks → `if !cond then panic(msg: "...") else ()`
    /// - Post-checks → bind result, assert via lambda call, return result
    fn lower_run_seq(
        &mut self,
        pre_checks: ori_ir::CheckRange,
        bindings: ori_ir::SeqBindingRange,
        result: ori_ir::ExprId,
        post_checks: ori_ir::CheckRange,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let pre_check_list = self.src.get_checks(pre_checks).to_vec();
        let post_check_list = self.src.get_checks(post_checks).to_vec();
        let has_post_checks = !post_check_list.is_empty();

        // Phase 1: Lower all pre-check assertions
        let pre_stmts: Vec<CanId> = pre_check_list
            .iter()
            .map(|check| self.lower_check_assertion(check, self.name_pre_check_failed, span))
            .collect();

        // Phase 2: Lower bindings
        let binding_stmts = self.lower_seq_bindings(bindings);

        // Phase 3: Lower result and post-checks
        if has_post_checks {
            // Bind result to a temporary, run post-check assertions, return temporary.
            let result_id = self.lower_expr(result);
            let result_name = self.name_check_result;
            let pattern = self.arena.push_binding_pattern(CanBindingPattern::Name {
                name: result_name,
                mutable: false,
            });
            let let_result = self.push(
                CanExpr::Let {
                    pattern,
                    init: result_id,
                    mutable: false,
                },
                span,
                TypeId::UNIT,
            );

            // Reference to the bound result
            let result_ref = self.push(CanExpr::Ident(result_name), span, ty);

            // Lower post-check assertions: if !lambda(result) then panic(msg: "...")
            let post_stmts: Vec<CanId> = post_check_list
                .iter()
                .map(|check| self.lower_post_check_assertion(check, result_ref, span, ty))
                .collect();

            // Final result: reference to the bound result
            let final_result = self.push(CanExpr::Ident(result_name), span, ty);

            // Assemble block: [pre_stmts, bindings, let_result, post_stmts]
            let start = self.arena.start_expr_list();
            for &s in &pre_stmts {
                self.arena.push_expr_list_item(s);
            }
            self.arena.extend_expr_list(binding_stmts);
            self.arena.push_expr_list_item(let_result);
            for &s in &post_stmts {
                self.arena.push_expr_list_item(s);
            }
            let stmts = self.arena.finish_expr_list(start);

            self.push(
                CanExpr::Block {
                    stmts,
                    result: final_result,
                },
                span,
                ty,
            )
        } else if pre_stmts.is_empty() {
            // No checks at all — original fast path
            let binding_range = binding_stmts;
            let result = self.lower_expr(result);
            self.push(
                CanExpr::Block {
                    stmts: binding_range,
                    result,
                },
                span,
                ty,
            )
        } else {
            // Pre-checks only, no post-checks
            let result = self.lower_expr(result);
            let start = self.arena.start_expr_list();
            for &s in &pre_stmts {
                self.arena.push_expr_list_item(s);
            }
            self.arena.extend_expr_list(binding_stmts);
            let stmts = self.arena.finish_expr_list(start);
            self.push(CanExpr::Block { stmts, result }, span, ty)
        }
    }

    /// Lower a pre-check into `if !condition then panic(msg: "...") else ()`.
    fn lower_check_assertion(
        &mut self,
        check: &ori_ir::CheckExpr,
        default_msg: Name,
        span: Span,
    ) -> CanId {
        let cond = self.lower_expr(check.expr);
        let negated = self.push(
            CanExpr::Unary {
                op: UnaryOp::Not,
                operand: cond,
            },
            check.span,
            TypeId::BOOL,
        );

        let panic_node = self.lower_check_panic(check, default_msg, span);
        let unit = self.push(CanExpr::Unit, span, TypeId::UNIT);

        self.push(
            CanExpr::If {
                cond: negated,
                then_branch: panic_node,
                else_branch: unit,
            },
            check.span,
            TypeId::UNIT,
        )
    }

    /// Lower a post-check into `if !lambda(result) then panic(msg: "...") else ()`.
    fn lower_post_check_assertion(
        &mut self,
        check: &ori_ir::CheckExpr,
        result_ref: CanId,
        span: Span,
        _result_ty: TypeId,
    ) -> CanId {
        // Lower the lambda expression
        let lambda = self.lower_expr(check.expr);

        // Call the lambda with the result: lambda(result)
        let args = self.arena.push_expr_list(&[result_ref]);
        let call = self.push(
            CanExpr::Call { func: lambda, args },
            check.span,
            TypeId::BOOL,
        );

        // Negate: !lambda(result)
        let negated = self.push(
            CanExpr::Unary {
                op: UnaryOp::Not,
                operand: call,
            },
            check.span,
            TypeId::BOOL,
        );

        let panic_node = self.lower_check_panic(check, self.name_post_check_failed, span);
        let unit = self.push(CanExpr::Unit, span, TypeId::UNIT);

        self.push(
            CanExpr::If {
                cond: negated,
                then_branch: panic_node,
                else_branch: unit,
            },
            check.span,
            TypeId::UNIT,
        )
    }

    /// Construct a `panic(msg: "...")` node for check failure.
    fn lower_check_panic(
        &mut self,
        check: &ori_ir::CheckExpr,
        default_msg: Name,
        span: Span,
    ) -> CanId {
        // Use custom message if provided, otherwise use pre-interned default
        let msg_id = if let Some(msg_expr) = check.message {
            self.lower_expr(msg_expr)
        } else {
            self.push(CanExpr::Str(default_msg), span, TypeId::STR)
        };

        let props = self.arena.push_named_exprs(&[CanNamedExpr {
            name: self.name_msg,
            value: msg_id,
        }]);

        self.push(
            CanExpr::FunctionExp {
                kind: FunctionExpKind::Panic,
                props,
            },
            span,
            TypeId::NEVER,
        )
    }
}
