//! Incremental Parsing Support
//!
//! This module provides infrastructure for incremental parsing, allowing the parser
//! to reuse unchanged AST subtrees when a document is edited.
//!
//! # Architecture
//!
//! The incremental parsing system works in phases:
//!
//! 1. **Declaration Collection** - Gather all top-level declarations with their spans
//! 2. **Cursor Navigation** - Find declarations that can be reused
//! 3. **Deep Copy** - Copy reusable declarations with span adjustment
//!
//! # Key Types
//!
//! - [`DeclKind`] - Categories of top-level declarations
//! - [`DeclRef`] - Reference to a declaration with its span
//! - [`SyntaxCursor`] - Navigator for finding reusable declarations
//! - [`IncrementalState`] - Session state tracking reuse statistics

use ori_ir::incremental::ChangeMarker;
use ori_ir::{
    ast::{BindingPattern, FunctionExp, FunctionSeq, MatchArm, MatchPattern, SeqBinding},
    CallArg, ConfigDef, DefImplDef, Expr, ExprArena, ExprId, ExprKind, ExtendDef, FieldInit,
    Function, GenericParam, ImplAssocType, ImplDef, ImplMethod, MapEntry, MatchPatternId,
    MatchPatternRange, Module, Name, NamedExpr, Param, ParsedType, ParsedTypeId, ParsedTypeRange,
    Span, Stmt, StmtKind, TestDef, TraitAssocType, TraitDef, TraitDefaultMethod, TraitItem,
    TraitMethodSig, TypeDecl, UseDef, WhereClause,
};

/// Kind of top-level declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeclKind {
    Import,
    Config,
    Function,
    Test,
    Type,
    Trait,
    Impl,
    DefImpl,
    Extend,
}

/// Reference to a declaration with its source span.
#[derive(Debug, Clone, Copy)]
pub struct DeclRef {
    pub kind: DeclKind,
    pub index: usize,
    pub span: Span,
}

/// Collect all top-level declarations from a module, sorted by start position.
pub fn collect_declarations(module: &Module) -> Vec<DeclRef> {
    let mut decls = Vec::new();

    for (i, import) in module.imports.iter().enumerate() {
        decls.push(DeclRef {
            kind: DeclKind::Import,
            index: i,
            span: import.span,
        });
    }

    for (i, config) in module.configs.iter().enumerate() {
        decls.push(DeclRef {
            kind: DeclKind::Config,
            index: i,
            span: config.span,
        });
    }

    for (i, func) in module.functions.iter().enumerate() {
        decls.push(DeclRef {
            kind: DeclKind::Function,
            index: i,
            span: func.span,
        });
    }

    for (i, test) in module.tests.iter().enumerate() {
        decls.push(DeclRef {
            kind: DeclKind::Test,
            index: i,
            span: test.span,
        });
    }

    for (i, type_decl) in module.types.iter().enumerate() {
        decls.push(DeclRef {
            kind: DeclKind::Type,
            index: i,
            span: type_decl.span,
        });
    }

    for (i, trait_def) in module.traits.iter().enumerate() {
        decls.push(DeclRef {
            kind: DeclKind::Trait,
            index: i,
            span: trait_def.span,
        });
    }

    for (i, impl_def) in module.impls.iter().enumerate() {
        decls.push(DeclRef {
            kind: DeclKind::Impl,
            index: i,
            span: impl_def.span,
        });
    }

    for (i, def_impl) in module.def_impls.iter().enumerate() {
        decls.push(DeclRef {
            kind: DeclKind::DefImpl,
            index: i,
            span: def_impl.span,
        });
    }

    for (i, extend) in module.extends.iter().enumerate() {
        decls.push(DeclRef {
            kind: DeclKind::Extend,
            index: i,
            span: extend.span,
        });
    }

    // Sort by start position for binary search
    decls.sort_by_key(|d| d.span.start);
    decls
}

/// Navigator for finding reusable declarations in an old AST.
pub struct SyntaxCursor<'old> {
    module: &'old Module,
    arena: &'old ExprArena,
    marker: ChangeMarker,
    declarations: Vec<DeclRef>,
    current_index: usize,
}

impl<'old> SyntaxCursor<'old> {
    /// Create a new cursor for navigating the old AST.
    pub fn new(module: &'old Module, arena: &'old ExprArena, marker: ChangeMarker) -> Self {
        let declarations = collect_declarations(module);
        SyntaxCursor {
            module,
            arena,
            marker,
            declarations,
            current_index: 0,
        }
    }

    /// Get the change marker.
    pub fn marker(&self) -> &ChangeMarker {
        &self.marker
    }

    /// Get reference to the old module.
    pub fn module(&self) -> &'old Module {
        self.module
    }

    /// Get reference to the old arena.
    pub fn arena(&self) -> &'old ExprArena {
        self.arena
    }

    /// Find a reusable declaration at or after the given position.
    ///
    /// Returns `Some(decl_ref)` if a declaration exists that:
    /// 1. Starts at or after `pos`
    /// 2. Does not intersect the affected region
    ///
    /// Returns `None` if no suitable declaration is found.
    pub fn find_at(&mut self, pos: u32) -> Option<DeclRef> {
        // Advance past declarations that end before pos
        while self.current_index < self.declarations.len() {
            let decl = self.declarations[self.current_index];
            if decl.span.end > pos {
                break;
            }
            self.current_index += 1;
        }

        if self.current_index >= self.declarations.len() {
            return None;
        }

        let decl = self.declarations[self.current_index];

        // Check if the declaration can be reused (doesn't intersect affected region)
        if self.marker.intersects(decl.span) {
            None
        } else {
            Some(decl)
        }
    }

    /// Advance the cursor past a declaration (after reusing it).
    pub fn advance(&mut self) {
        if self.current_index < self.declarations.len() {
            self.current_index += 1;
        }
    }

    /// Check if all declarations have been processed.
    pub fn is_exhausted(&self) -> bool {
        self.current_index >= self.declarations.len()
    }
}

/// Statistics for incremental parsing.
#[derive(Clone, Debug, Default)]
pub struct IncrementalStats {
    /// Number of declarations reused from the old tree.
    pub reused_count: usize,
    /// Number of declarations that were reparsed.
    pub reparsed_count: usize,
}

impl IncrementalStats {
    /// Calculate reuse rate as a percentage.
    #[allow(clippy::cast_precision_loss)] // Acceptable for percentage display - counts won't approach 2^52
    pub fn reuse_rate(&self) -> f64 {
        let total = self.reused_count + self.reparsed_count;
        if total == 0 {
            0.0
        } else {
            (self.reused_count as f64 / total as f64) * 100.0
        }
    }
}

/// State for an incremental parsing session.
pub struct IncrementalState<'old> {
    pub cursor: SyntaxCursor<'old>,
    pub stats: IncrementalStats,
}

impl<'old> IncrementalState<'old> {
    /// Create a new incremental parsing state.
    pub fn new(cursor: SyntaxCursor<'old>) -> Self {
        IncrementalState {
            cursor,
            stats: IncrementalStats::default(),
        }
    }
}

// =============================================================================
// Deep Copy Infrastructure
// =============================================================================

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
            ExprKind::Config(name) => ExprKind::Config(*name),
            ExprKind::SelfRef => ExprKind::SelfRef,
            ExprKind::FunctionRef(name) => ExprKind::FunctionRef(*name),
            ExprKind::HashLength => ExprKind::HashLength,
            ExprKind::None => ExprKind::None,
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
                else_branch: else_branch.map(|e| self.copy_expr(e, new_arena)),
            },
            ExprKind::Match { scrutinee, arms } => {
                self.copy_match_kind(*scrutinee, *arms, new_arena)
            }
            ExprKind::For {
                binding,
                iter,
                guard,
                body,
                is_yield,
            } => ExprKind::For {
                binding: *binding,
                iter: self.copy_expr(*iter, new_arena),
                guard: guard.map(|g| self.copy_expr(g, new_arena)),
                body: self.copy_expr(*body, new_arena),
                is_yield: *is_yield,
            },
            ExprKind::Loop { body } => ExprKind::Loop {
                body: self.copy_expr(*body, new_arena),
            },
            ExprKind::Block { stmts, result } => self.copy_block_kind(*stmts, *result, new_arena),

            // Bindings
            ExprKind::Let {
                pattern,
                ty,
                init,
                mutable,
            } => ExprKind::Let {
                pattern: self.copy_binding_pattern(pattern),
                ty: ty.as_ref().map(|t| self.copy_parsed_type(t, new_arena)),
                init: self.copy_expr(*init, new_arena),
                mutable: *mutable,
            },
            ExprKind::Lambda {
                params,
                ret_ty,
                body,
            } => self.copy_lambda_kind(*params, ret_ty.as_ref(), *body, new_arena),

            // Collections
            ExprKind::List(exprs) => {
                let new_exprs = self.copy_expr_list(*exprs, new_arena);
                ExprKind::List(new_exprs)
            }
            ExprKind::Map(entries) => self.copy_map_kind(*entries, new_arena),
            ExprKind::Struct { name, fields } => self.copy_struct_kind(*name, *fields, new_arena),
            ExprKind::Tuple(exprs) => {
                let new_exprs = self.copy_expr_list(*exprs, new_arena);
                ExprKind::Tuple(new_exprs)
            }
            ExprKind::Range {
                start,
                end,
                step,
                inclusive,
            } => ExprKind::Range {
                start: start.map(|s| self.copy_expr(s, new_arena)),
                end: end.map(|e| self.copy_expr(e, new_arena)),
                step: step.map(|s| self.copy_expr(s, new_arena)),
                inclusive: *inclusive,
            },

            // Result/Option constructors
            ExprKind::Ok(inner) => ExprKind::Ok(inner.map(|i| self.copy_expr(i, new_arena))),
            ExprKind::Err(inner) => ExprKind::Err(inner.map(|i| self.copy_expr(i, new_arena))),
            ExprKind::Some(inner) => ExprKind::Some(self.copy_expr(*inner, new_arena)),

            // Control
            ExprKind::Break(val) => ExprKind::Break(val.map(|v| self.copy_expr(v, new_arena))),
            ExprKind::Continue(val) => {
                ExprKind::Continue(val.map(|v| self.copy_expr(v, new_arena)))
            }
            ExprKind::Await(inner) => ExprKind::Await(self.copy_expr(*inner, new_arena)),
            ExprKind::Try(inner) => ExprKind::Try(self.copy_expr(*inner, new_arena)),
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
            ExprKind::FunctionSeq(seq) => {
                ExprKind::FunctionSeq(self.copy_function_seq(seq, new_arena))
            }
            ExprKind::FunctionExp(exp) => {
                ExprKind::FunctionExp(self.copy_function_exp(exp, new_arena))
            }
        };

        new_arena.alloc_expr(Expr::new(new_kind, new_span))
    }

    /// Copy a Block expression's statements and result.
    fn copy_block_kind(
        &self,
        stmts: ori_ir::StmtRange,
        result: Option<ExprId>,
        new_arena: &mut ExprArena,
    ) -> ExprKind {
        let old_stmts = self.old_arena.get_stmt_range(stmts);
        let mut new_stmts = Vec::with_capacity(old_stmts.len());
        for stmt in old_stmts {
            new_stmts.push(self.copy_stmt(stmt, new_arena));
        }
        // Allocate statements sequentially
        #[allow(clippy::cast_possible_truncation)]
        // Statement indices won't exceed u32::MAX in practice
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
            result: result.map(|r| self.copy_expr(r, new_arena)),
        }
    }

    /// Copy a Lambda expression's parameters and body.
    fn copy_lambda_kind(
        &self,
        params: ori_ir::ParamRange,
        ret_ty: Option<&ParsedType>,
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
            ret_ty: ret_ty.map(|t| self.copy_parsed_type(t, new_arena)),
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

    /// Copy an `ExprList` (inline or overflow).
    fn copy_expr_list(
        &self,
        list: ori_ir::ExprList,
        new_arena: &mut ExprArena,
    ) -> ori_ir::ExprList {
        let items: Vec<ExprId> = self
            .old_arena
            .iter_expr_list(list)
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
            } => StmtKind::Let {
                pattern: self.copy_binding_pattern(pattern),
                ty: *ty, // Option<TypeId> is Copy, no adjustment needed
                init: self.copy_expr(*init, new_arena),
                mutable: *mutable,
            },
        };
        Stmt::new(new_kind, new_span)
    }

    /// Copy a call argument.
    fn copy_call_arg(&self, arg: &CallArg, new_arena: &mut ExprArena) -> CallArg {
        CallArg {
            name: arg.name,
            value: self.copy_expr(arg.value, new_arena),
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
            MatchPattern::Struct { fields } => {
                let new_fields: Vec<_> = fields
                    .iter()
                    .map(|(name, opt_pattern)| {
                        let new_opt =
                            opt_pattern.map(|pid| self.copy_match_pattern_id(pid, new_arena));
                        (*name, new_opt)
                    })
                    .collect();
                MatchPattern::Struct { fields: new_fields }
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
    #[allow(clippy::self_only_used_in_recursion)] // Recursive copy pattern requires &self for consistency
    fn copy_binding_pattern(&self, pattern: &BindingPattern) -> BindingPattern {
        match pattern {
            BindingPattern::Name(name) => BindingPattern::Name(*name),
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
                    .map(|(name, opt_pat)| {
                        (
                            *name,
                            opt_pat.as_ref().map(|p| self.copy_binding_pattern(p)),
                        )
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
            ty: param
                .ty
                .as_ref()
                .map(|t| self.copy_parsed_type(t, new_arena)),
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
        }
    }

    /// Copy a parsed type by ID, allocating in the new arena.
    fn copy_parsed_type_id(&self, old_id: ParsedTypeId, new_arena: &mut ExprArena) -> ParsedTypeId {
        let old_ty = self.old_arena.get_parsed_type(old_id);
        let new_ty = self.copy_parsed_type(old_ty, new_arena);
        new_arena.alloc_parsed_type(new_ty)
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
            FunctionSeq::Run {
                bindings,
                result,
                span,
            } => {
                let new_bindings = self.copy_seq_binding_range(*bindings, new_arena);
                FunctionSeq::Run {
                    bindings: new_bindings,
                    result: self.copy_expr(*result, new_arena),
                    span: self.adjust_span(*span),
                }
            }
            FunctionSeq::Try {
                bindings,
                result,
                span,
            } => {
                let new_bindings = self.copy_seq_binding_range(*bindings, new_arena);
                FunctionSeq::Try {
                    bindings: new_bindings,
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

    /// Copy a sequence binding range.
    fn copy_seq_binding_range(
        &self,
        range: ori_ir::SeqBindingRange,
        new_arena: &mut ExprArena,
    ) -> ori_ir::SeqBindingRange {
        let old_bindings = self.old_arena.get_seq_bindings(range);
        let new_bindings: Vec<_> = old_bindings
            .iter()
            .map(|b| self.copy_seq_binding(b, new_arena))
            .collect();
        new_arena.alloc_seq_bindings(new_bindings)
    }

    /// Copy a sequence binding.
    fn copy_seq_binding(&self, binding: &SeqBinding, new_arena: &mut ExprArena) -> SeqBinding {
        match binding {
            SeqBinding::Let {
                pattern,
                ty,
                value,
                mutable,
                span,
            } => SeqBinding::Let {
                pattern: self.copy_binding_pattern(pattern),
                ty: ty.as_ref().map(|t| self.copy_parsed_type(t, new_arena)),
                value: self.copy_expr(*value, new_arena),
                mutable: *mutable,
                span: self.adjust_span(*span),
            },
            SeqBinding::Stmt { expr, span } => SeqBinding::Stmt {
                expr: self.copy_expr(*expr, new_arena),
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

    // =========================================================================
    // Declaration Copying
    // =========================================================================

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
            .map(|w| self.copy_where_clause(w))
            .collect();

        Function {
            name: func.name,
            generics: new_arena.alloc_generic_params(new_generics),
            params: new_arena.alloc_params(new_params),
            return_ty: func
                .return_ty
                .as_ref()
                .map(|t| self.copy_parsed_type(t, new_arena)),
            capabilities: func.capabilities.clone(),
            where_clauses: new_where_clauses,
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
            .map(|w| self.copy_where_clause(w))
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
            .map(|w| self.copy_where_clause(w))
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
            .map(|w| self.copy_where_clause(w))
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

    /// Copy a config definition.
    pub fn copy_config(&self, config: &ConfigDef, new_arena: &mut ExprArena) -> ConfigDef {
        ConfigDef {
            name: config.name,
            value: self.copy_expr(config.value, new_arena),
            span: self.adjust_span(config.span),
            visibility: config.visibility,
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

    // =========================================================================
    // Helper Methods
    // =========================================================================

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
            span: self.adjust_span(param.span),
        }
    }

    /// Copy a where clause.
    fn copy_where_clause(&self, clause: &WhereClause) -> WhereClause {
        WhereClause {
            param: clause.param,
            projection: clause.projection,
            bounds: clause
                .bounds
                .iter()
                .map(|b| self.copy_trait_bound(b))
                .collect(),
            span: self.adjust_span(clause.span),
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

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::incremental::TextChange;

    #[test]
    fn test_collect_declarations_empty() {
        let module = Module::new();
        let decls = collect_declarations(&module);
        assert!(decls.is_empty());
    }

    #[test]
    fn test_collect_declarations_sorted() {
        // Create a module with declarations in various orders
        let mut module = Module::new();

        // Add configs, functions in non-sorted order
        module.configs.push(ConfigDef {
            name: ori_ir::Name::EMPTY,
            value: ExprId::INVALID,
            span: Span::new(100, 150),
            visibility: ori_ir::Visibility::Private,
        });

        module.functions.push(Function {
            name: ori_ir::Name::EMPTY,
            generics: ori_ir::GenericParamRange::EMPTY,
            params: ori_ir::ParamRange::EMPTY,
            return_ty: None,
            capabilities: Vec::new(),
            where_clauses: Vec::new(),
            body: ExprId::INVALID,
            span: Span::new(50, 80),
            visibility: ori_ir::Visibility::Private,
        });

        let decls = collect_declarations(&module);
        assert_eq!(decls.len(), 2);
        // Should be sorted by start position
        assert_eq!(decls[0].span.start, 50);
        assert_eq!(decls[1].span.start, 100);
    }

    #[test]
    fn test_syntax_cursor_find_at() {
        let mut module = Module::new();

        module.functions.push(Function {
            name: ori_ir::Name::EMPTY,
            generics: ori_ir::GenericParamRange::EMPTY,
            params: ori_ir::ParamRange::EMPTY,
            return_ty: None,
            capabilities: Vec::new(),
            where_clauses: Vec::new(),
            body: ExprId::INVALID,
            span: Span::new(0, 50),
            visibility: ori_ir::Visibility::Private,
        });

        module.functions.push(Function {
            name: ori_ir::Name::EMPTY,
            generics: ori_ir::GenericParamRange::EMPTY,
            params: ori_ir::ParamRange::EMPTY,
            return_ty: None,
            capabilities: Vec::new(),
            where_clauses: Vec::new(),
            body: ExprId::INVALID,
            span: Span::new(100, 150),
            visibility: ori_ir::Visibility::Private,
        });

        let arena = ExprArena::new();
        // Change affects positions 60-80, so first function is reusable, second might be
        let change = TextChange::new(60, 80, 30);
        let marker = ChangeMarker::from_change(&change, 55);
        let mut cursor = SyntaxCursor::new(&module, &arena, marker);

        // First function (0-50) doesn't intersect the change (60-80)
        let Some(first) = cursor.find_at(0) else {
            panic!("should find first declaration");
        };
        assert_eq!(first.kind, DeclKind::Function);
        assert_eq!(first.span.start, 0);
    }

    #[test]
    fn test_incremental_stats() {
        let mut stats = IncrementalStats::default();
        #[allow(clippy::float_cmp)] // Exact zero comparison is intentional
        {
            assert_eq!(stats.reuse_rate(), 0.0);
        }

        stats.reused_count = 8;
        stats.reparsed_count = 2;
        assert!((stats.reuse_rate() - 80.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_incremental_basic() {
        use crate::{parse, parse_incremental};
        use ori_ir::StringInterner;

        let interner = StringInterner::new();

        // Original source with two functions
        let source = "@first () -> int = 42\n\n@second () -> int = 100";
        let tokens = ori_lexer::lex(source, &interner);
        let old_result = parse(&tokens, &interner);

        assert!(!old_result.has_errors());
        assert_eq!(old_result.module.functions.len(), 2);

        // Now modify the first function: change 42 to 99
        // The source is: "@first () -> int = 42\n\n@second () -> int = 100"
        //                 ^^^^^^^^^^^^^^^^^^^^ position 19-21 is "42"
        let new_source = "@first () -> int = 99\n\n@second () -> int = 100";
        let new_tokens = ori_lexer::lex(new_source, &interner);

        // Create a change: replace "42" (2 chars at position 19) with "99" (2 chars)
        let change = TextChange::new(19, 21, 2);

        let new_result = parse_incremental(&new_tokens, &interner, &old_result, change);

        assert!(!new_result.has_errors());
        assert_eq!(new_result.module.functions.len(), 2);
    }

    #[test]
    fn test_parse_incremental_insert() {
        use crate::{parse, parse_incremental};
        use ori_ir::StringInterner;

        let interner = StringInterner::new();

        // Original source
        let source = "@add (x: int) -> int = x + 1";
        let tokens = ori_lexer::lex(source, &interner);
        let old_result = parse(&tokens, &interner);

        assert!(!old_result.has_errors());
        assert_eq!(old_result.module.functions.len(), 1);

        // Insert a newline and new function at the end
        let new_source = "@add (x: int) -> int = x + 1\n\n@sub (x: int) -> int = x - 1";
        let new_tokens = ori_lexer::lex(new_source, &interner);

        // Insert at position 28 (after original source)
        let change = TextChange::insert(28, 30); // "\n\n@sub (x: int) -> int = x - 1" is 30 chars

        let new_result = parse_incremental(&new_tokens, &interner, &old_result, change);

        assert!(!new_result.has_errors());
        assert_eq!(new_result.module.functions.len(), 2);
    }

    #[test]
    fn test_parse_incremental_fresh_parse_on_overlap() {
        use crate::{parse, parse_incremental};
        use ori_ir::StringInterner;

        let interner = StringInterner::new();

        // Original source with one function
        let source = "@compute (x: int, y: int) -> int = x + y";
        let tokens = ori_lexer::lex(source, &interner);
        let old_result = parse(&tokens, &interner);

        assert!(!old_result.has_errors());
        assert_eq!(old_result.module.functions.len(), 1);

        // Modify the function signature (change "y: int" to "y: float")
        // Position of "y: int" is approximately at position 14-20
        let new_source = "@compute (x: int, y: float) -> int = x + y";
        let new_tokens = ori_lexer::lex(new_source, &interner);

        // Change: "int" (3 chars) to "float" (5 chars) at position ~18-21
        let change = TextChange::new(18, 21, 5);

        let new_result = parse_incremental(&new_tokens, &interner, &old_result, change);

        assert!(!new_result.has_errors());
        assert_eq!(new_result.module.functions.len(), 1);
    }
}
