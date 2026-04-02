//! Expression copying for the AST copier.
//!
//! Contains the main `copy_expr` dispatch and closely-related block/lambda/match
//! expression helpers.

use super::AstCopier;
use ori_ir::{Expr, ExprArena, ExprId, ExprKind};

impl AstCopier<'_> {
    /// Copy an expression tree recursively, allocating in the new arena.
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive ExprKind copy dispatch for incremental reparsing"
    )]
    pub fn copy_expr(&self, old_id: ExprId, new_arena: &mut ExprArena) -> ExprId {
        let old_expr = self.old_arena.get_expr(old_id);
        let new_span = self.adjust_span(old_expr.span);

        let new_kind = match &old_expr.kind {
            // Leaf nodes - just clone
            ExprKind::Int(n) => ExprKind::Int(*n),
            ExprKind::Float(bits) => ExprKind::Float(*bits),
            ExprKind::Bool(b) => ExprKind::Bool(*b),
            ExprKind::String(name) => ExprKind::String(*name),
            ExprKind::Char(c) => ExprKind::Char(*c),
            ExprKind::Duration { value, unit } => ExprKind::Duration {
                value: *value,
                unit: *unit,
            },
            ExprKind::Size { value, unit } => ExprKind::Size {
                value: *value,
                unit: *unit,
            },
            ExprKind::Unit => ExprKind::Unit,
            ExprKind::Ident(name) => ExprKind::Ident(*name),
            ExprKind::Const(name) => ExprKind::Const(*name),
            ExprKind::SelfRef => ExprKind::SelfRef,
            ExprKind::FunctionRef(name) => ExprKind::FunctionRef(*name),
            ExprKind::HashLength => ExprKind::HashLength,
            ExprKind::None => ExprKind::None,
            ExprKind::TemplateFull(name) => ExprKind::TemplateFull(*name),
            ExprKind::Error => ExprKind::Error,

            // Binary and unary operations
            ExprKind::Binary { op, left, right } => ExprKind::Binary {
                op: *op,
                left: self.copy_expr(*left, new_arena),
                right: self.copy_expr(*right, new_arena),
            },
            ExprKind::Unary { op, operand } => ExprKind::Unary {
                op: *op,
                operand: self.copy_expr(*operand, new_arena),
            },

            // Call expressions
            ExprKind::Call { func, args } => {
                let new_func = self.copy_expr(*func, new_arena);
                let new_args = self.copy_expr_list(*args, new_arena);
                ExprKind::Call {
                    func: new_func,
                    args: new_args,
                }
            }
            ExprKind::CallNamed { func, args } => {
                self.copy_call_named_kind(*func, *args, new_arena)
            }
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => {
                let new_receiver = self.copy_expr(*receiver, new_arena);
                let new_args = self.copy_expr_list(*args, new_arena);
                ExprKind::MethodCall {
                    receiver: new_receiver,
                    method: *method,
                    args: new_args,
                }
            }
            ExprKind::MethodCallNamed {
                receiver,
                method,
                args,
            } => self.copy_method_call_named_kind(*receiver, *method, *args, new_arena),

            // Field and index access
            ExprKind::Field { receiver, field } => ExprKind::Field {
                receiver: self.copy_expr(*receiver, new_arena),
                field: *field,
            },
            ExprKind::Index { receiver, index } => ExprKind::Index {
                receiver: self.copy_expr(*receiver, new_arena),
                index: self.copy_expr(*index, new_arena),
            },

            // Control flow
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => ExprKind::If {
                cond: self.copy_expr(*cond, new_arena),
                then_branch: self.copy_expr(*then_branch, new_arena),
                else_branch: if else_branch.is_present() {
                    self.copy_expr(*else_branch, new_arena)
                } else {
                    ExprId::INVALID
                },
            },
            ExprKind::Match { scrutinee, arms } => {
                self.copy_match_kind(*scrutinee, *arms, new_arena)
            }
            ExprKind::For {
                label,
                pattern,
                iter,
                guard,
                body,
                is_yield,
            } => {
                let old_pattern = self.old_arena.get_binding_pattern(*pattern);
                let copied_pattern = self.copy_binding_pattern(old_pattern);
                let new_pattern_id = new_arena.alloc_binding_pattern(copied_pattern);
                ExprKind::For {
                    label: *label,
                    pattern: new_pattern_id,
                    iter: self.copy_expr(*iter, new_arena),
                    guard: if guard.is_present() {
                        self.copy_expr(*guard, new_arena)
                    } else {
                        ExprId::INVALID
                    },
                    body: self.copy_expr(*body, new_arena),
                    is_yield: *is_yield,
                }
            }
            ExprKind::Loop { label, body } => ExprKind::Loop {
                label: *label,
                body: self.copy_expr(*body, new_arena),
            },
            ExprKind::Block { stmts, result } => self.copy_block_kind(*stmts, *result, new_arena),

            // Bindings
            ExprKind::Let {
                pattern,
                ty,
                init,
                mutable,
            } => {
                let old_pattern = self.old_arena.get_binding_pattern(*pattern);
                let copied_pattern = self.copy_binding_pattern(old_pattern);
                let new_pattern_id = new_arena.alloc_binding_pattern(copied_pattern);
                ExprKind::Let {
                    pattern: new_pattern_id,
                    ty: self.copy_optional_parsed_type_id(*ty, new_arena),
                    init: self.copy_expr(*init, new_arena),
                    mutable: *mutable,
                }
            }
            ExprKind::Lambda {
                params,
                ret_ty,
                body,
            } => self.copy_lambda_kind(*params, *ret_ty, *body, new_arena),

            // Collections
            ExprKind::List(exprs) => {
                let new_exprs = self.copy_expr_list(*exprs, new_arena);
                ExprKind::List(new_exprs)
            }
            ExprKind::ListWithSpread(elements) => {
                self.copy_list_with_spread_kind(*elements, new_arena)
            }
            ExprKind::Map(entries) => self.copy_map_kind(*entries, new_arena),
            ExprKind::MapWithSpread(elements) => {
                self.copy_map_with_spread_kind(*elements, new_arena)
            }
            ExprKind::Struct { name, fields } => self.copy_struct_kind(*name, *fields, new_arena),
            ExprKind::StructWithSpread { name, fields } => {
                self.copy_struct_with_spread_kind(*name, *fields, new_arena)
            }
            ExprKind::Tuple(exprs) => {
                let new_exprs = self.copy_expr_list(*exprs, new_arena);
                ExprKind::Tuple(new_exprs)
            }
            ExprKind::TemplateLiteral { head, parts } => {
                self.copy_template_literal_kind(*head, *parts, new_arena)
            }
            ExprKind::Range {
                start,
                end,
                step,
                inclusive,
            } => ExprKind::Range {
                start: if start.is_present() {
                    self.copy_expr(*start, new_arena)
                } else {
                    ExprId::INVALID
                },
                end: if end.is_present() {
                    self.copy_expr(*end, new_arena)
                } else {
                    ExprId::INVALID
                },
                step: if step.is_present() {
                    self.copy_expr(*step, new_arena)
                } else {
                    ExprId::INVALID
                },
                inclusive: *inclusive,
            },

            // Result/Option constructors
            ExprKind::Ok(inner) => ExprKind::Ok(if inner.is_present() {
                self.copy_expr(*inner, new_arena)
            } else {
                ExprId::INVALID
            }),
            ExprKind::Err(inner) => ExprKind::Err(if inner.is_present() {
                self.copy_expr(*inner, new_arena)
            } else {
                ExprId::INVALID
            }),
            ExprKind::Some(inner) => ExprKind::Some(self.copy_expr(*inner, new_arena)),

            // Control
            ExprKind::Break { label, value } => ExprKind::Break {
                label: *label,
                value: if value.is_present() {
                    self.copy_expr(*value, new_arena)
                } else {
                    ExprId::INVALID
                },
            },
            ExprKind::Continue { label, value } => ExprKind::Continue {
                label: *label,
                value: if value.is_present() {
                    self.copy_expr(*value, new_arena)
                } else {
                    ExprId::INVALID
                },
            },
            ExprKind::Unsafe(inner) => ExprKind::Unsafe(self.copy_expr(*inner, new_arena)),
            ExprKind::Await(inner) => ExprKind::Await(self.copy_expr(*inner, new_arena)),
            ExprKind::Try(inner) => ExprKind::Try(self.copy_expr(*inner, new_arena)),
            ExprKind::Cast { expr, ty, fallible } => ExprKind::Cast {
                expr: self.copy_expr(*expr, new_arena),
                ty: self.copy_parsed_type_id(*ty, new_arena),
                fallible: *fallible,
            },
            ExprKind::Assign { target, value } => ExprKind::Assign {
                target: self.copy_expr(*target, new_arena),
                value: self.copy_expr(*value, new_arena),
            },

            // Capability
            ExprKind::WithCapability {
                capability,
                provider,
                body,
            } => ExprKind::WithCapability {
                capability: *capability,
                provider: self.copy_expr(*provider, new_arena),
                body: self.copy_expr(*body, new_arena),
            },

            // Function constructs
            ExprKind::FunctionSeq(seq_id) => {
                let seq = self.old_arena.get_function_seq(*seq_id);
                let new_seq = self.copy_function_seq(seq, new_arena);
                let new_id = new_arena.alloc_function_seq(new_seq);
                ExprKind::FunctionSeq(new_id)
            }
            ExprKind::FunctionExp(exp_id) => {
                let exp = self.old_arena.get_function_exp(*exp_id);
                let new_exp = self.copy_function_exp(exp, new_arena);
                let new_id = new_arena.alloc_function_exp(new_exp);
                ExprKind::FunctionExp(new_id)
            }
        };

        new_arena.alloc_expr(Expr::new(new_kind, new_span))
    }

    /// Copy a Block expression's statements and result.
    fn copy_block_kind(
        &self,
        stmts: ori_ir::StmtRange,
        result: ExprId,
        new_arena: &mut ExprArena,
    ) -> ExprKind {
        let old_stmts = self.old_arena.get_stmt_range(stmts);
        let mut new_stmts = Vec::with_capacity(old_stmts.len());
        for stmt in old_stmts {
            new_stmts.push(self.copy_stmt(stmt, new_arena));
        }
        // Allocate statements sequentially
        #[allow(
            clippy::cast_possible_truncation,
            reason = "statement indices won't exceed u32::MAX in practice"
        )]
        let start_id = if new_stmts.is_empty() {
            0
        } else {
            let first_id = new_arena.alloc_stmt(new_stmts[0].clone());
            for stmt in new_stmts.iter().skip(1) {
                new_arena.alloc_stmt(stmt.clone());
            }
            first_id.index() as u32
        };
        ExprKind::Block {
            stmts: new_arena.alloc_stmt_range(start_id, new_stmts.len()),
            result: if result.is_present() {
                self.copy_expr(result, new_arena)
            } else {
                ExprId::INVALID
            },
        }
    }

    /// Copy a Lambda expression's parameters and body.
    fn copy_lambda_kind(
        &self,
        params: ori_ir::ParamRange,
        ret_ty: ori_ir::ParsedTypeId,
        body: ExprId,
        new_arena: &mut ExprArena,
    ) -> ExprKind {
        let old_params = self.old_arena.get_params(params);
        let new_params: Vec<_> = old_params
            .iter()
            .map(|p| self.copy_param(p, new_arena))
            .collect();
        ExprKind::Lambda {
            params: new_arena.alloc_params(new_params),
            ret_ty: self.copy_optional_parsed_type_id(ret_ty, new_arena),
            body: self.copy_expr(body, new_arena),
        }
    }

    /// Copy a Match expression's scrutinee and arms.
    fn copy_match_kind(
        &self,
        scrutinee: ExprId,
        arms: ori_ir::ArmRange,
        new_arena: &mut ExprArena,
    ) -> ExprKind {
        let new_scrutinee = self.copy_expr(scrutinee, new_arena);
        let old_arms = self.old_arena.get_arms(arms);
        let new_arms: Vec<_> = old_arms
            .iter()
            .map(|arm| self.copy_match_arm(arm, new_arena))
            .collect();
        ExprKind::Match {
            scrutinee: new_scrutinee,
            arms: new_arena.alloc_arms(new_arms),
        }
    }
}
