//! Deep copier for AST nodes with span adjustment.

use ori_ir::incremental::ChangeMarker;
use ori_ir::{
    ast::{BindingPattern, FunctionExp, FunctionSeq, MatchArm, MatchPattern},
    CallArg, CapabilityRef, ConstDef, DefImplDef, Expr, ExprArena, ExprId, ExprKind, ExtendDef,
    ExternBlock, ExternItem, ExternParam, FieldInit, Function, GenericParam, ImplAssocType,
    ImplDef, ImplMethod, MapEntry, MatchPatternId, MatchPatternRange, Name, NamedExpr, Param,
    ParsedType, ParsedTypeId, ParsedTypeRange, Span, Stmt, StmtKind, TemplatePart,
    TemplatePartRange, TestDef, TraitAssocType, TraitDef, TraitDefaultMethod, TraitItem,
    TraitMethodSig, TypeDecl, UseDef, WhereClause,
};

/// Deep copier for AST nodes with span adjustment.
///
/// This struct handles copying expressions and declarations from an old arena
/// to a new arena while adjusting spans according to a change marker.
pub struct AstCopier<'old> {
    old_arena: &'old ExprArena,
    marker: ChangeMarker,
}

impl<'old> AstCopier<'old> {
    /// Create a new AST copier.
    pub fn new(old_arena: &'old ExprArena, marker: ChangeMarker) -> Self {
        AstCopier { old_arena, marker }
    }

    /// Adjust a span from old positions to new positions.
    fn adjust_span(&self, span: Span) -> Span {
        self.marker.adjust_span(span).unwrap_or(span)
    }

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
                binding,
                iter,
                guard,
                body,
                is_yield,
            } => ExprKind::For {
                label: *label,
                binding: *binding,
                iter: self.copy_expr(*iter, new_arena),
                guard: if guard.is_present() {
                    self.copy_expr(*guard, new_arena)
                } else {
                    ExprId::INVALID
                },
                body: self.copy_expr(*body, new_arena),
                is_yield: *is_yield,
            },
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
        ret_ty: ParsedTypeId,
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

    /// Copy a `TemplateLiteral` expression's parts.
    fn copy_template_literal_kind(
        &self,
        head: Name,
        parts: TemplatePartRange,
        new_arena: &mut ExprArena,
    ) -> ExprKind {
        let old_parts = self.old_arena.get_template_parts(parts);
        let new_parts: Vec<_> = old_parts
            .iter()
            .map(|p| TemplatePart {
                expr: self.copy_expr(p.expr, new_arena),
                format_spec: p.format_spec,
                text_after: p.text_after,
            })
            .collect();
        ExprKind::TemplateLiteral {
            head,
            parts: new_arena.alloc_template_parts(new_parts),
        }
    }

    /// Copy a Map expression's entries.
    fn copy_map_kind(&self, entries: ori_ir::MapEntryRange, new_arena: &mut ExprArena) -> ExprKind {
        let old_entries = self.old_arena.get_map_entries(entries);
        let new_entries: Vec<_> = old_entries
            .iter()
            .map(|e| self.copy_map_entry(e, new_arena))
            .collect();
        ExprKind::Map(new_arena.alloc_map_entries(new_entries))
    }

    /// Copy a Struct expression's name and fields.
    fn copy_struct_kind(
        &self,
        name: Name,
        fields: ori_ir::FieldInitRange,
        new_arena: &mut ExprArena,
    ) -> ExprKind {
        let old_fields = self.old_arena.get_field_inits(fields);
        let new_fields: Vec<_> = old_fields
            .iter()
            .map(|f| self.copy_field_init(f, new_arena))
            .collect();
        ExprKind::Struct {
            name,
            fields: new_arena.alloc_field_inits(new_fields),
        }
    }

    /// Copy a `StructWithSpread` expression's name and fields.
    fn copy_struct_with_spread_kind(
        &self,
        name: Name,
        fields: ori_ir::StructLitFieldRange,
        new_arena: &mut ExprArena,
    ) -> ExprKind {
        let old_fields = self.old_arena.get_struct_lit_fields(fields);
        let new_fields: Vec<_> = old_fields
            .iter()
            .map(|f| self.copy_struct_lit_field(f, new_arena))
            .collect();
        ExprKind::StructWithSpread {
            name,
            fields: new_arena.alloc_struct_lit_fields(new_fields),
        }
    }

    /// Copy a struct literal field (either regular field or spread).
    fn copy_struct_lit_field(
        &self,
        field: &ori_ir::StructLitField,
        new_arena: &mut ExprArena,
    ) -> ori_ir::StructLitField {
        match field {
            ori_ir::StructLitField::Field(init) => {
                ori_ir::StructLitField::Field(self.copy_field_init(init, new_arena))
            }
            ori_ir::StructLitField::Spread { expr, span } => ori_ir::StructLitField::Spread {
                expr: self.copy_expr(*expr, new_arena),
                span: self.adjust_span(*span),
            },
        }
    }

    /// Copy a `ListWithSpread` expression's elements.
    fn copy_list_with_spread_kind(
        &self,
        elements: ori_ir::ListElementRange,
        new_arena: &mut ExprArena,
    ) -> ExprKind {
        let old_elements = self.old_arena.get_list_elements(elements);
        let new_elements: Vec<_> = old_elements
            .iter()
            .map(|e| self.copy_list_element(e, new_arena))
            .collect();
        ExprKind::ListWithSpread(new_arena.alloc_list_elements(new_elements))
    }

    /// Copy a list element (either regular value or spread).
    fn copy_list_element(
        &self,
        element: &ori_ir::ListElement,
        new_arena: &mut ExprArena,
    ) -> ori_ir::ListElement {
        match element {
            ori_ir::ListElement::Expr { expr, span } => ori_ir::ListElement::Expr {
                expr: self.copy_expr(*expr, new_arena),
                span: self.adjust_span(*span),
            },
            ori_ir::ListElement::Spread { expr, span } => ori_ir::ListElement::Spread {
                expr: self.copy_expr(*expr, new_arena),
                span: self.adjust_span(*span),
            },
        }
    }

    /// Copy a `MapWithSpread` expression's elements.
    fn copy_map_with_spread_kind(
        &self,
        elements: ori_ir::MapElementRange,
        new_arena: &mut ExprArena,
    ) -> ExprKind {
        let old_elements = self.old_arena.get_map_elements(elements);
        let new_elements: Vec<_> = old_elements
            .iter()
            .map(|e| self.copy_map_element(e, new_arena))
            .collect();
        ExprKind::MapWithSpread(new_arena.alloc_map_elements(new_elements))
    }

    /// Copy a map element (either entry or spread).
    fn copy_map_element(
        &self,
        element: &ori_ir::MapElement,
        new_arena: &mut ExprArena,
    ) -> ori_ir::MapElement {
        match element {
            ori_ir::MapElement::Entry(entry) => {
                ori_ir::MapElement::Entry(self.copy_map_entry(entry, new_arena))
            }
            ori_ir::MapElement::Spread { expr, span } => ori_ir::MapElement::Spread {
                expr: self.copy_expr(*expr, new_arena),
                span: self.adjust_span(*span),
            },
        }
    }

    /// Copy a named call's function and arguments.
    fn copy_call_named_kind(
        &self,
        func: ExprId,
        args: ori_ir::CallArgRange,
        new_arena: &mut ExprArena,
    ) -> ExprKind {
        let new_func = self.copy_expr(func, new_arena);
        let old_args = self.old_arena.get_call_args(args);
        let new_args: Vec<_> = old_args
            .iter()
            .map(|arg| self.copy_call_arg(arg, new_arena))
            .collect();
        ExprKind::CallNamed {
            func: new_func,
            args: new_arena.alloc_call_args(new_args),
        }
    }

    /// Copy a named method call's receiver, method, and arguments.
    fn copy_method_call_named_kind(
        &self,
        receiver: ExprId,
        method: Name,
        args: ori_ir::CallArgRange,
        new_arena: &mut ExprArena,
    ) -> ExprKind {
        let new_receiver = self.copy_expr(receiver, new_arena);
        let old_args = self.old_arena.get_call_args(args);
        let new_args: Vec<_> = old_args
            .iter()
            .map(|arg| self.copy_call_arg(arg, new_arena))
            .collect();
        ExprKind::MethodCallNamed {
            receiver: new_receiver,
            method,
            args: new_arena.alloc_call_args(new_args),
        }
    }

    /// Copy an `ExprRange` (expression list stored in arena).
    fn copy_expr_list(
        &self,
        range: ori_ir::ExprRange,
        new_arena: &mut ExprArena,
    ) -> ori_ir::ExprRange {
        let items: Vec<ExprId> = self
            .old_arena
            .get_expr_list(range)
            .iter()
            .copied()
            .map(|id| self.copy_expr(id, new_arena))
            .collect();
        new_arena.alloc_expr_list_inline(&items)
    }

    /// Copy a statement.
    fn copy_stmt(&self, stmt: &Stmt, new_arena: &mut ExprArena) -> Stmt {
        let new_span = self.adjust_span(stmt.span);
        let new_kind = match &stmt.kind {
            StmtKind::Expr(id) => StmtKind::Expr(self.copy_expr(*id, new_arena)),
            StmtKind::Let {
                pattern,
                ty,
                init,
                mutable,
            } => {
                let old_pattern = self.old_arena.get_binding_pattern(*pattern);
                let copied_pattern = self.copy_binding_pattern(old_pattern);
                let new_pattern_id = new_arena.alloc_binding_pattern(copied_pattern);
                StmtKind::Let {
                    pattern: new_pattern_id,
                    ty: self.copy_optional_parsed_type_id(*ty, new_arena),
                    init: self.copy_expr(*init, new_arena),
                    mutable: *mutable,
                }
            }
        };
        Stmt::new(new_kind, new_span)
    }

    /// Copy a call argument.
    fn copy_call_arg(&self, arg: &CallArg, new_arena: &mut ExprArena) -> CallArg {
        CallArg {
            name: arg.name,
            value: self.copy_expr(arg.value, new_arena),
            is_spread: arg.is_spread,
            span: self.adjust_span(arg.span),
        }
    }

    /// Copy a match arm.
    fn copy_match_arm(&self, arm: &MatchArm, new_arena: &mut ExprArena) -> MatchArm {
        MatchArm {
            pattern: self.copy_match_pattern(&arm.pattern, new_arena),
            guard: arm.guard.map(|g| self.copy_expr(g, new_arena)),
            body: self.copy_expr(arm.body, new_arena),
            span: self.adjust_span(arm.span),
        }
    }

    /// Copy a match pattern.
    fn copy_match_pattern(
        &self,
        pattern: &MatchPattern,
        new_arena: &mut ExprArena,
    ) -> MatchPattern {
        match pattern {
            MatchPattern::Wildcard => MatchPattern::Wildcard,
            MatchPattern::Binding(name) => MatchPattern::Binding(*name),
            MatchPattern::Literal(id) => MatchPattern::Literal(self.copy_expr(*id, new_arena)),
            MatchPattern::Variant { name, inner } => {
                let new_inner = self.copy_match_pattern_range(*inner, new_arena);
                MatchPattern::Variant {
                    name: *name,
                    inner: new_inner,
                }
            }
            MatchPattern::Struct { fields, rest } => {
                let new_fields: Vec<_> = fields
                    .iter()
                    .map(|(name, opt_pattern)| {
                        let new_opt =
                            opt_pattern.map(|pid| self.copy_match_pattern_id(pid, new_arena));
                        (*name, new_opt)
                    })
                    .collect();
                MatchPattern::Struct {
                    fields: new_fields,
                    rest: *rest,
                }
            }
            MatchPattern::Tuple(patterns) => {
                let new_patterns = self.copy_match_pattern_range(*patterns, new_arena);
                MatchPattern::Tuple(new_patterns)
            }
            MatchPattern::List { elements, rest } => {
                let new_elements = self.copy_match_pattern_range(*elements, new_arena);
                MatchPattern::List {
                    elements: new_elements,
                    rest: *rest,
                }
            }
            MatchPattern::Range {
                start,
                end,
                inclusive,
            } => MatchPattern::Range {
                start: start.map(|s| self.copy_expr(s, new_arena)),
                end: end.map(|e| self.copy_expr(e, new_arena)),
                inclusive: *inclusive,
            },
            MatchPattern::Or(patterns) => {
                let new_patterns = self.copy_match_pattern_range(*patterns, new_arena);
                MatchPattern::Or(new_patterns)
            }
            MatchPattern::At { name, pattern } => MatchPattern::At {
                name: *name,
                pattern: self.copy_match_pattern_id(*pattern, new_arena),
            },
        }
    }

    /// Copy a match pattern by ID, allocating in the new arena.
    fn copy_match_pattern_id(
        &self,
        old_id: MatchPatternId,
        new_arena: &mut ExprArena,
    ) -> MatchPatternId {
        let old_pattern = self.old_arena.get_match_pattern(old_id);
        let new_pattern = self.copy_match_pattern(old_pattern, new_arena);
        new_arena.alloc_match_pattern(new_pattern)
    }

    /// Copy a match pattern range, allocating in the new arena.
    fn copy_match_pattern_range(
        &self,
        range: MatchPatternRange,
        new_arena: &mut ExprArena,
    ) -> MatchPatternRange {
        let old_ids = self.old_arena.get_match_pattern_list(range);
        let new_ids: Vec<_> = old_ids
            .iter()
            .map(|id| self.copy_match_pattern_id(*id, new_arena))
            .collect();
        new_arena.alloc_match_pattern_list(new_ids)
    }

    /// Copy a binding pattern.
    #[allow(
        clippy::self_only_used_in_recursion,
        reason = "recursive copy pattern requires &self for method consistency"
    )]
    fn copy_binding_pattern(&self, pattern: &BindingPattern) -> BindingPattern {
        match pattern {
            BindingPattern::Name { name, mutable } => BindingPattern::Name {
                name: *name,
                mutable: *mutable,
            },
            BindingPattern::Wildcard => BindingPattern::Wildcard,
            BindingPattern::Tuple(patterns) => {
                let new_patterns: Vec<_> = patterns
                    .iter()
                    .map(|p| self.copy_binding_pattern(p))
                    .collect();
                BindingPattern::Tuple(new_patterns)
            }
            BindingPattern::Struct { fields } => {
                let new_fields: Vec<_> = fields
                    .iter()
                    .map(|field| ori_ir::FieldBinding {
                        name: field.name,
                        mutable: field.mutable,
                        pattern: field.pattern.as_ref().map(|p| self.copy_binding_pattern(p)),
                    })
                    .collect();
                BindingPattern::Struct { fields: new_fields }
            }
            BindingPattern::List { elements, rest } => {
                let new_elements: Vec<_> = elements
                    .iter()
                    .map(|p| self.copy_binding_pattern(p))
                    .collect();
                BindingPattern::List {
                    elements: new_elements,
                    rest: *rest,
                }
            }
        }
    }

    /// Copy a map entry.
    fn copy_map_entry(&self, entry: &MapEntry, new_arena: &mut ExprArena) -> MapEntry {
        MapEntry {
            key: self.copy_expr(entry.key, new_arena),
            value: self.copy_expr(entry.value, new_arena),
            span: self.adjust_span(entry.span),
        }
    }

    /// Copy a field initializer.
    fn copy_field_init(&self, field: &FieldInit, new_arena: &mut ExprArena) -> FieldInit {
        FieldInit {
            name: field.name,
            value: field.value.map(|id| self.copy_expr(id, new_arena)),
            span: self.adjust_span(field.span),
        }
    }

    /// Copy a parameter.
    fn copy_param(&self, param: &Param, new_arena: &mut ExprArena) -> Param {
        Param {
            name: param.name,
            pattern: param
                .pattern
                .as_ref()
                .map(|p| self.copy_match_pattern(p, new_arena)),
            ty: param
                .ty
                .as_ref()
                .map(|t| self.copy_parsed_type(t, new_arena)),
            default: param.default.map(|e| self.copy_expr(e, new_arena)),
            is_variadic: param.is_variadic,
            span: self.adjust_span(param.span),
        }
    }

    /// Copy a parsed type, allocating nested types in the new arena.
    fn copy_parsed_type(&self, ty: &ParsedType, new_arena: &mut ExprArena) -> ParsedType {
        match ty {
            ParsedType::Primitive(id) => ParsedType::Primitive(*id),
            ParsedType::Named { name, type_args } => {
                let new_type_args = self.copy_parsed_type_range(*type_args, new_arena);
                ParsedType::Named {
                    name: *name,
                    type_args: new_type_args,
                }
            }
            ParsedType::List(elem_id) => {
                let new_elem_id = self.copy_parsed_type_id(*elem_id, new_arena);
                ParsedType::List(new_elem_id)
            }
            ParsedType::FixedList { elem, capacity } => {
                let new_elem = self.copy_parsed_type_id(*elem, new_arena);
                let new_capacity = self.copy_expr(*capacity, new_arena);
                ParsedType::FixedList {
                    elem: new_elem,
                    capacity: new_capacity,
                }
            }
            ParsedType::Tuple(elems) => {
                let new_elems = self.copy_parsed_type_range(*elems, new_arena);
                ParsedType::Tuple(new_elems)
            }
            ParsedType::Function { params, ret } => {
                let new_params = self.copy_parsed_type_range(*params, new_arena);
                let new_ret = self.copy_parsed_type_id(*ret, new_arena);
                ParsedType::Function {
                    params: new_params,
                    ret: new_ret,
                }
            }
            ParsedType::Map { key, value } => {
                let new_key = self.copy_parsed_type_id(*key, new_arena);
                let new_value = self.copy_parsed_type_id(*value, new_arena);
                ParsedType::Map {
                    key: new_key,
                    value: new_value,
                }
            }
            ParsedType::Infer => ParsedType::Infer,
            ParsedType::SelfType => ParsedType::SelfType,
            ParsedType::AssociatedType { base, assoc_name } => {
                let new_base = self.copy_parsed_type_id(*base, new_arena);
                ParsedType::AssociatedType {
                    base: new_base,
                    assoc_name: *assoc_name,
                }
            }
            ParsedType::ConstExpr(expr_id) => {
                let new_expr = self.copy_expr(*expr_id, new_arena);
                ParsedType::ConstExpr(new_expr)
            }
            ParsedType::TraitBounds(bounds) => {
                let new_bounds = self.copy_parsed_type_range(*bounds, new_arena);
                ParsedType::TraitBounds(new_bounds)
            }
        }
    }

    /// Copy a parsed type by ID, allocating in the new arena.
    fn copy_parsed_type_id(&self, old_id: ParsedTypeId, new_arena: &mut ExprArena) -> ParsedTypeId {
        let old_ty = self.old_arena.get_parsed_type(old_id);
        let new_ty = self.copy_parsed_type(old_ty, new_arena);
        new_arena.alloc_parsed_type(new_ty)
    }

    /// Copy an optional parsed type ID (INVALID sentinel = no type annotation).
    fn copy_optional_parsed_type_id(
        &self,
        id: ParsedTypeId,
        new_arena: &mut ExprArena,
    ) -> ParsedTypeId {
        if id.is_valid() {
            self.copy_parsed_type_id(id, new_arena)
        } else {
            ParsedTypeId::INVALID
        }
    }

    /// Copy a parsed type range, allocating in the new arena.
    fn copy_parsed_type_range(
        &self,
        range: ParsedTypeRange,
        new_arena: &mut ExprArena,
    ) -> ParsedTypeRange {
        let old_ids = self.old_arena.get_parsed_type_list(range);
        let new_ids: Vec<_> = old_ids
            .iter()
            .map(|id| self.copy_parsed_type_id(*id, new_arena))
            .collect();
        new_arena.alloc_parsed_type_list(new_ids)
    }

    /// Copy a `FunctionSeq`.
    fn copy_function_seq(&self, seq: &FunctionSeq, new_arena: &mut ExprArena) -> FunctionSeq {
        match seq {
            FunctionSeq::Try {
                stmts,
                result,
                span,
            } => {
                let old_stmts = self.old_arena.get_stmt_range(*stmts);
                let new_stmts: Vec<_> = old_stmts
                    .iter()
                    .map(|s| self.copy_stmt(s, new_arena))
                    .collect();
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
                FunctionSeq::Try {
                    stmts: new_arena.alloc_stmt_range(start_id, new_stmts.len()),
                    result: self.copy_expr(*result, new_arena),
                    span: self.adjust_span(*span),
                }
            }
            FunctionSeq::Match {
                scrutinee,
                arms,
                span,
            } => {
                let old_arms = self.old_arena.get_arms(*arms);
                let new_arms: Vec<_> = old_arms
                    .iter()
                    .map(|arm| self.copy_match_arm(arm, new_arena))
                    .collect();
                FunctionSeq::Match {
                    scrutinee: self.copy_expr(*scrutinee, new_arena),
                    arms: new_arena.alloc_arms(new_arms),
                    span: self.adjust_span(*span),
                }
            }
            FunctionSeq::ForPattern {
                over,
                map,
                arm,
                default,
                span,
            } => FunctionSeq::ForPattern {
                over: self.copy_expr(*over, new_arena),
                map: map.map(|m| self.copy_expr(m, new_arena)),
                arm: self.copy_match_arm(arm, new_arena),
                default: self.copy_expr(*default, new_arena),
                span: self.adjust_span(*span),
            },
        }
    }

    /// Copy a `FunctionExp`.
    fn copy_function_exp(&self, exp: &FunctionExp, new_arena: &mut ExprArena) -> FunctionExp {
        let old_props = self.old_arena.get_named_exprs(exp.props);
        let new_props: Vec<_> = old_props
            .iter()
            .map(|p| self.copy_named_expr(p, new_arena))
            .collect();
        FunctionExp {
            kind: exp.kind,
            props: new_arena.alloc_named_exprs(new_props),
            type_args: self.copy_parsed_type_range(exp.type_args, new_arena),
            span: self.adjust_span(exp.span),
        }
    }

    /// Copy a named expression.
    fn copy_named_expr(&self, expr: &NamedExpr, new_arena: &mut ExprArena) -> NamedExpr {
        NamedExpr {
            name: expr.name,
            value: self.copy_expr(expr.value, new_arena),
            span: self.adjust_span(expr.span),
        }
    }

    // Declaration Copying

    /// Copy a function declaration.
    pub fn copy_function(&self, func: &Function, new_arena: &mut ExprArena) -> Function {
        let old_generics = self.old_arena.get_generic_params(func.generics);
        let new_generics: Vec<_> = old_generics
            .iter()
            .map(|g| self.copy_generic_param(g, new_arena))
            .collect();

        let old_params = self.old_arena.get_params(func.params);
        let new_params: Vec<_> = old_params
            .iter()
            .map(|p| self.copy_param(p, new_arena))
            .collect();

        let new_where_clauses: Vec<_> = func
            .where_clauses
            .iter()
            .map(|w| self.copy_where_clause(w, new_arena))
            .collect();

        Function {
            name: func.name,
            generics: new_arena.alloc_generic_params(new_generics),
            params: new_arena.alloc_params(new_params),
            return_ty: func
                .return_ty
                .as_ref()
                .map(|t| self.copy_parsed_type(t, new_arena)),
            capabilities: func
                .capabilities
                .iter()
                .map(|c| CapabilityRef {
                    name: c.name,
                    span: self.adjust_span(c.span),
                })
                .collect(),
            where_clauses: new_where_clauses,
            guard: func.guard.map(|g| self.copy_expr(g, new_arena)),
            body: self.copy_expr(func.body, new_arena),
            span: self.adjust_span(func.span),
            visibility: func.visibility,
        }
    }

    /// Copy a test definition.
    pub fn copy_test(&self, test: &TestDef, new_arena: &mut ExprArena) -> TestDef {
        let old_params = self.old_arena.get_params(test.params);
        let new_params: Vec<_> = old_params
            .iter()
            .map(|p| self.copy_param(p, new_arena))
            .collect();

        TestDef {
            name: test.name,
            targets: test.targets.clone(),
            params: new_arena.alloc_params(new_params),
            return_ty: test
                .return_ty
                .as_ref()
                .map(|t| self.copy_parsed_type(t, new_arena)),
            body: self.copy_expr(test.body, new_arena),
            span: self.adjust_span(test.span),
            skip_reason: test.skip_reason,
            expected_errors: test.expected_errors.clone(),
            fail_expected: test.fail_expected,
        }
    }

    /// Copy a type declaration.
    pub fn copy_type_decl(&self, decl: &TypeDecl, new_arena: &mut ExprArena) -> TypeDecl {
        let old_generics = self.old_arena.get_generic_params(decl.generics);
        let new_generics: Vec<_> = old_generics
            .iter()
            .map(|g| self.copy_generic_param(g, new_arena))
            .collect();

        let new_where_clauses: Vec<_> = decl
            .where_clauses
            .iter()
            .map(|w| self.copy_where_clause(w, new_arena))
            .collect();

        TypeDecl {
            name: decl.name,
            generics: new_arena.alloc_generic_params(new_generics),
            where_clauses: new_where_clauses,
            kind: self.copy_type_decl_kind(&decl.kind, new_arena),
            span: self.adjust_span(decl.span),
            visibility: decl.visibility,
            derives: decl.derives.clone(),
        }
    }

    /// Copy a type declaration kind.
    fn copy_type_decl_kind(
        &self,
        kind: &ori_ir::TypeDeclKind,
        new_arena: &mut ExprArena,
    ) -> ori_ir::TypeDeclKind {
        match kind {
            ori_ir::TypeDeclKind::Struct(fields) => {
                let new_fields: Vec<_> = fields
                    .iter()
                    .map(|f| ori_ir::StructField {
                        name: f.name,
                        ty: self.copy_parsed_type(&f.ty, new_arena),
                        span: self.adjust_span(f.span),
                    })
                    .collect();
                ori_ir::TypeDeclKind::Struct(new_fields)
            }
            ori_ir::TypeDeclKind::Sum(variants) => {
                let new_variants: Vec<_> = variants
                    .iter()
                    .map(|v| ori_ir::Variant {
                        name: v.name,
                        fields: v
                            .fields
                            .iter()
                            .map(|f| ori_ir::VariantField {
                                name: f.name,
                                ty: self.copy_parsed_type(&f.ty, new_arena),
                                span: self.adjust_span(f.span),
                            })
                            .collect(),
                        span: self.adjust_span(v.span),
                    })
                    .collect();
                ori_ir::TypeDeclKind::Sum(new_variants)
            }
            ori_ir::TypeDeclKind::Newtype(ty) => {
                ori_ir::TypeDeclKind::Newtype(self.copy_parsed_type(ty, new_arena))
            }
        }
    }

    /// Copy a trait definition.
    pub fn copy_trait(&self, trait_def: &TraitDef, new_arena: &mut ExprArena) -> TraitDef {
        let old_generics = self.old_arena.get_generic_params(trait_def.generics);
        let new_generics: Vec<_> = old_generics
            .iter()
            .map(|g| self.copy_generic_param(g, new_arena))
            .collect();

        let new_items: Vec<_> = trait_def
            .items
            .iter()
            .map(|item| self.copy_trait_item(item, new_arena))
            .collect();

        TraitDef {
            name: trait_def.name,
            generics: new_arena.alloc_generic_params(new_generics),
            super_traits: trait_def
                .super_traits
                .iter()
                .map(|t| self.copy_trait_bound(t))
                .collect(),
            items: new_items,
            span: self.adjust_span(trait_def.span),
            visibility: trait_def.visibility,
        }
    }

    /// Copy a trait item.
    fn copy_trait_item(&self, item: &TraitItem, new_arena: &mut ExprArena) -> TraitItem {
        match item {
            TraitItem::MethodSig(sig) => {
                TraitItem::MethodSig(self.copy_trait_method_sig(sig, new_arena))
            }
            TraitItem::DefaultMethod(method) => {
                TraitItem::DefaultMethod(self.copy_trait_default_method(method, new_arena))
            }
            TraitItem::AssocType(assoc) => {
                TraitItem::AssocType(self.copy_trait_assoc_type(assoc, new_arena))
            }
        }
    }

    /// Copy a trait method signature.
    fn copy_trait_method_sig(
        &self,
        sig: &TraitMethodSig,
        new_arena: &mut ExprArena,
    ) -> TraitMethodSig {
        let old_params = self.old_arena.get_params(sig.params);
        let new_params: Vec<_> = old_params
            .iter()
            .map(|p| self.copy_param(p, new_arena))
            .collect();

        TraitMethodSig {
            name: sig.name,
            params: new_arena.alloc_params(new_params),
            return_ty: self.copy_parsed_type(&sig.return_ty, new_arena),
            span: self.adjust_span(sig.span),
        }
    }

    /// Copy a trait default method.
    fn copy_trait_default_method(
        &self,
        method: &TraitDefaultMethod,
        new_arena: &mut ExprArena,
    ) -> TraitDefaultMethod {
        let old_params = self.old_arena.get_params(method.params);
        let new_params: Vec<_> = old_params
            .iter()
            .map(|p| self.copy_param(p, new_arena))
            .collect();

        TraitDefaultMethod {
            name: method.name,
            params: new_arena.alloc_params(new_params),
            return_ty: self.copy_parsed_type(&method.return_ty, new_arena),
            body: self.copy_expr(method.body, new_arena),
            span: self.adjust_span(method.span),
        }
    }

    /// Copy a trait associated type.
    fn copy_trait_assoc_type(
        &self,
        assoc: &TraitAssocType,
        new_arena: &mut ExprArena,
    ) -> TraitAssocType {
        TraitAssocType {
            name: assoc.name,
            default_type: assoc
                .default_type
                .as_ref()
                .map(|t| self.copy_parsed_type(t, new_arena)),
            span: self.adjust_span(assoc.span),
        }
    }

    /// Copy an impl definition.
    pub fn copy_impl(&self, impl_def: &ImplDef, new_arena: &mut ExprArena) -> ImplDef {
        let old_generics = self.old_arena.get_generic_params(impl_def.generics);
        let new_generics: Vec<_> = old_generics
            .iter()
            .map(|g| self.copy_generic_param(g, new_arena))
            .collect();

        let old_trait_type_args = self
            .old_arena
            .get_parsed_type_list(impl_def.trait_type_args);
        let new_trait_type_args: Vec<_> = old_trait_type_args
            .iter()
            .map(|id| self.copy_parsed_type_id(*id, new_arena))
            .collect();

        let new_where_clauses: Vec<_> = impl_def
            .where_clauses
            .iter()
            .map(|w| self.copy_where_clause(w, new_arena))
            .collect();

        let new_methods: Vec<_> = impl_def
            .methods
            .iter()
            .map(|m| self.copy_impl_method(m, new_arena))
            .collect();

        let new_assoc_types: Vec<_> = impl_def
            .assoc_types
            .iter()
            .map(|a| self.copy_impl_assoc_type(a, new_arena))
            .collect();

        ImplDef {
            generics: new_arena.alloc_generic_params(new_generics),
            trait_path: impl_def.trait_path.clone(),
            trait_type_args: new_arena.alloc_parsed_type_list(new_trait_type_args),
            self_path: impl_def.self_path.clone(),
            self_ty: self.copy_parsed_type(&impl_def.self_ty, new_arena),
            where_clauses: new_where_clauses,
            methods: new_methods,
            assoc_types: new_assoc_types,
            span: self.adjust_span(impl_def.span),
        }
    }

    /// Copy an impl method.
    fn copy_impl_method(&self, method: &ImplMethod, new_arena: &mut ExprArena) -> ImplMethod {
        let old_params = self.old_arena.get_params(method.params);
        let new_params: Vec<_> = old_params
            .iter()
            .map(|p| self.copy_param(p, new_arena))
            .collect();

        ImplMethod {
            name: method.name,
            params: new_arena.alloc_params(new_params),
            return_ty: self.copy_parsed_type(&method.return_ty, new_arena),
            body: self.copy_expr(method.body, new_arena),
            span: self.adjust_span(method.span),
        }
    }

    /// Copy an impl associated type.
    fn copy_impl_assoc_type(
        &self,
        assoc: &ImplAssocType,
        new_arena: &mut ExprArena,
    ) -> ImplAssocType {
        ImplAssocType {
            name: assoc.name,
            ty: self.copy_parsed_type(&assoc.ty, new_arena),
            span: self.adjust_span(assoc.span),
        }
    }

    /// Copy a def impl definition.
    pub fn copy_def_impl(&self, def_impl: &DefImplDef, new_arena: &mut ExprArena) -> DefImplDef {
        let new_methods: Vec<_> = def_impl
            .methods
            .iter()
            .map(|m| self.copy_impl_method(m, new_arena))
            .collect();

        DefImplDef {
            trait_name: def_impl.trait_name,
            methods: new_methods,
            span: self.adjust_span(def_impl.span),
            visibility: def_impl.visibility,
        }
    }

    /// Copy an extend definition.
    pub fn copy_extend(&self, extend: &ExtendDef, new_arena: &mut ExprArena) -> ExtendDef {
        let old_generics = self.old_arena.get_generic_params(extend.generics);
        let new_generics: Vec<_> = old_generics
            .iter()
            .map(|g| self.copy_generic_param(g, new_arena))
            .collect();

        let new_where_clauses: Vec<_> = extend
            .where_clauses
            .iter()
            .map(|w| self.copy_where_clause(w, new_arena))
            .collect();

        let new_methods: Vec<_> = extend
            .methods
            .iter()
            .map(|m| self.copy_impl_method(m, new_arena))
            .collect();

        ExtendDef {
            generics: new_arena.alloc_generic_params(new_generics),
            target_ty: self.copy_parsed_type(&extend.target_ty, new_arena),
            target_type_name: extend.target_type_name,
            where_clauses: new_where_clauses,
            methods: new_methods,
            span: self.adjust_span(extend.span),
        }
    }

    /// Copy a constant definition.
    pub fn copy_const(&self, const_def: &ConstDef, new_arena: &mut ExprArena) -> ConstDef {
        ConstDef {
            name: const_def.name,
            ty: const_def
                .ty
                .as_ref()
                .map(|t| self.copy_parsed_type(t, new_arena)),
            value: self.copy_expr(const_def.value, new_arena),
            span: self.adjust_span(const_def.span),
            visibility: const_def.visibility,
        }
    }

    /// Copy an extension import definition.
    ///
    /// Extension imports are pure data (no `ExprId` children), so only spans
    /// need adjustment; the rest is cloned directly.
    pub fn copy_extension_import(
        &self,
        ext_import: &ori_ir::ExtensionImport,
    ) -> ori_ir::ExtensionImport {
        ori_ir::ExtensionImport {
            path: ext_import.path.clone(),
            items: ext_import
                .items
                .iter()
                .map(|item| ori_ir::ExtensionImportItem {
                    type_name: item.type_name,
                    method_name: item.method_name,
                    span: self.adjust_span(item.span),
                })
                .collect(),
            visibility: ext_import.visibility,
            span: self.adjust_span(ext_import.span),
        }
    }

    /// Copy an extern block, adjusting spans and deep-copying parsed types.
    ///
    /// Extern blocks have no `ExprId` children but do contain `ParsedType`
    /// fields that reference arena-allocated compound types (e.g., `[float]`,
    /// `Option<CPtr>`). These must be deep-copied to avoid dangling references
    /// in the new arena.
    pub fn copy_extern_block(&self, block: &ExternBlock, new_arena: &mut ExprArena) -> ExternBlock {
        ExternBlock {
            convention: block.convention,
            library: block.library,
            items: block
                .items
                .iter()
                .map(|item| self.copy_extern_item(item, new_arena))
                .collect(),
            visibility: block.visibility,
            span: self.adjust_span(block.span),
        }
    }

    /// Copy an extern item (function declaration), adjusting spans and types.
    fn copy_extern_item(&self, item: &ExternItem, new_arena: &mut ExprArena) -> ExternItem {
        ExternItem {
            name: item.name,
            params: item
                .params
                .iter()
                .map(|p| self.copy_extern_param(p, new_arena))
                .collect(),
            return_ty: self.copy_parsed_type(&item.return_ty, new_arena),
            alias: item.alias,
            is_c_variadic: item.is_c_variadic,
            span: self.adjust_span(item.span),
        }
    }

    /// Copy an extern parameter, adjusting span and deep-copying its type.
    fn copy_extern_param(&self, param: &ExternParam, new_arena: &mut ExprArena) -> ExternParam {
        ExternParam {
            name: param.name,
            ty: self.copy_parsed_type(&param.ty, new_arena),
            span: self.adjust_span(param.span),
        }
    }

    /// Copy a use definition (import).
    pub fn copy_use(&self, use_def: &UseDef) -> UseDef {
        UseDef {
            path: use_def.path.clone(),
            items: use_def.items.clone(),
            module_alias: use_def.module_alias,
            visibility: use_def.visibility,
            span: self.adjust_span(use_def.span),
        }
    }

    // Helper Methods

    /// Copy a generic parameter.
    fn copy_generic_param(&self, param: &GenericParam, new_arena: &mut ExprArena) -> GenericParam {
        GenericParam {
            name: param.name,
            bounds: param
                .bounds
                .iter()
                .map(|b| self.copy_trait_bound(b))
                .collect(),
            default_type: param
                .default_type
                .as_ref()
                .map(|t| self.copy_parsed_type(t, new_arena)),
            is_const: param.is_const,
            const_type: param
                .const_type
                .as_ref()
                .map(|t| self.copy_parsed_type(t, new_arena)),
            default_value: param.default_value.map(|e| self.copy_expr(e, new_arena)),
            span: self.adjust_span(param.span),
        }
    }

    /// Copy a where clause.
    fn copy_where_clause(&self, clause: &WhereClause, new_arena: &mut ExprArena) -> WhereClause {
        match clause {
            WhereClause::TypeBound {
                param,
                projection,
                bounds,
                span,
            } => WhereClause::TypeBound {
                param: *param,
                projection: *projection,
                bounds: bounds.iter().map(|b| self.copy_trait_bound(b)).collect(),
                span: self.adjust_span(*span),
            },
            WhereClause::ConstBound { expr, span } => WhereClause::ConstBound {
                expr: self.copy_expr(*expr, new_arena),
                span: self.adjust_span(*span),
            },
        }
    }

    /// Copy a trait bound.
    fn copy_trait_bound(&self, bound: &ori_ir::TraitBound) -> ori_ir::TraitBound {
        ori_ir::TraitBound {
            first: bound.first,
            rest: bound.rest.clone(),
            span: self.adjust_span(bound.span),
        }
    }
}
