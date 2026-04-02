//! Shared helper methods for the AST copier.
//!
//! Contains copy methods for patterns, statements, collection elements,
//! and other building blocks used by both expression and declaration copying.

use super::AstCopier;
use ori_ir::{
    ast::{BindingPattern, MatchArm, MatchPattern},
    CallArg, ExprArena, ExprId, ExprKind, FieldInit, MapEntry, MatchPatternId, MatchPatternRange,
    Name, Param, Stmt, StmtKind, TemplatePart, TemplatePartRange,
};

impl AstCopier<'_> {
    /// Copy a `TemplateLiteral` expression's parts.
    pub(super) fn copy_template_literal_kind(
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
    pub(super) fn copy_map_kind(
        &self,
        entries: ori_ir::MapEntryRange,
        new_arena: &mut ExprArena,
    ) -> ExprKind {
        let old_entries = self.old_arena.get_map_entries(entries);
        let new_entries: Vec<_> = old_entries
            .iter()
            .map(|e| self.copy_map_entry(e, new_arena))
            .collect();
        ExprKind::Map(new_arena.alloc_map_entries(new_entries))
    }

    /// Copy a Struct expression's name and fields.
    pub(super) fn copy_struct_kind(
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
    pub(super) fn copy_struct_with_spread_kind(
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
    pub(super) fn copy_list_with_spread_kind(
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
    pub(super) fn copy_map_with_spread_kind(
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
    pub(super) fn copy_call_named_kind(
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
    pub(super) fn copy_method_call_named_kind(
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
    pub(super) fn copy_expr_list(
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
    pub(super) fn copy_stmt(&self, stmt: &Stmt, new_arena: &mut ExprArena) -> Stmt {
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
    pub(super) fn copy_match_arm(&self, arm: &MatchArm, new_arena: &mut ExprArena) -> MatchArm {
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
    pub(super) fn copy_binding_pattern(&self, pattern: &BindingPattern) -> BindingPattern {
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
    pub(super) fn copy_map_entry(&self, entry: &MapEntry, new_arena: &mut ExprArena) -> MapEntry {
        MapEntry {
            key: self.copy_expr(entry.key, new_arena),
            value: self.copy_expr(entry.value, new_arena),
            span: self.adjust_span(entry.span),
        }
    }

    /// Copy a field initializer.
    pub(super) fn copy_field_init(
        &self,
        field: &FieldInit,
        new_arena: &mut ExprArena,
    ) -> FieldInit {
        FieldInit {
            name: field.name,
            value: field.value.map(|id| self.copy_expr(id, new_arena)),
            span: self.adjust_span(field.span),
        }
    }

    /// Copy a parameter.
    pub(super) fn copy_param(&self, param: &Param, new_arena: &mut ExprArena) -> Param {
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
}
