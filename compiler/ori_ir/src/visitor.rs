//! AST Visitor Pattern
//!
//! Provides generic traversal of the AST. Based on the arena-allocated
//! structure where expressions are referenced by `ExprId` indices.
//!
//! # Design
//!
//! A single `Visitor` trait is provided for AST traversal. The visitor
//! can mutate its own state during traversal, but the AST remains immutable.
//!
//! Default implementations call `walk_*` functions that traverse children.
//! Override `visit_*` methods to add custom behavior at specific nodes.
//!
//! # Example
//!
//! ```text
//! struct CountLiterals {
//!     count: usize,
//! }
//!
//! impl<'ast> Visitor<'ast> for CountLiterals {
//!     fn visit_expr(&mut self, expr: &'ast Expr, arena: &'ast ExprArena) {
//!         match &expr.kind {
//!             ExprKind::Int(_) | ExprKind::Float(_) | ExprKind::Bool(_) => {
//!                 self.count += 1;
//!             }
//!             _ => {}
//!         }
//!         walk_expr(self, expr, arena);
//!     }
//! }
//! ```

use super::ast::{
    BindingPattern, CallArg, ConstDef, Expr, ExprKind, FieldInit, Function, FunctionExp,
    FunctionSeq, ListElement, MapElement, MapEntry, MatchArm, MatchPattern, Module, NamedExpr,
    Param, SeqBinding, Stmt, StmtKind, StructLitField, TestDef, UseDef,
};
use super::{ExprArena, ExprId};

// Visitor Trait

/// AST Visitor trait.
///
/// Override `visit_*` methods to add custom behavior at specific nodes.
/// Call `walk_*` functions to continue traversal into children.
///
/// Note: The visitor can mutate its own state during traversal.
/// The AST itself remains immutable.
pub trait Visitor<'ast> {
    /// Visit a module.
    fn visit_module(&mut self, module: &'ast Module, arena: &'ast ExprArena) {
        walk_module(self, module, arena);
    }

    /// Visit a function definition.
    fn visit_function(&mut self, function: &'ast Function, arena: &'ast ExprArena) {
        walk_function(self, function, arena);
    }

    /// Visit a test definition.
    fn visit_test(&mut self, test: &'ast TestDef, arena: &'ast ExprArena) {
        walk_test(self, test, arena);
    }

    /// Visit a use/import statement.
    fn visit_use(&mut self, use_def: &'ast UseDef, _arena: &'ast ExprArena) {
        // Use statements have no child expressions to walk
        let _ = use_def;
    }

    /// Visit a constant definition.
    fn visit_const(&mut self, const_def: &'ast ConstDef, arena: &'ast ExprArena) {
        // Walk the constant value expression
        self.visit_expr_id(const_def.value, arena);
    }

    /// Visit an expression.
    ///
    /// Note: Takes `&Expr` (not `&'ast Expr`) because expressions are
    /// reconstructed by value from parallel arrays.
    fn visit_expr(&mut self, expr: &Expr, arena: &'ast ExprArena) {
        walk_expr(self, expr, arena);
    }

    /// Visit an expression by ID.
    fn visit_expr_id(&mut self, id: ExprId, arena: &'ast ExprArena) {
        let expr = arena.get_expr(id);
        self.visit_expr(&expr, arena);
    }

    /// Visit a statement.
    fn visit_stmt(&mut self, stmt: &'ast Stmt, arena: &'ast ExprArena) {
        walk_stmt(self, stmt, arena);
    }

    /// Visit a parameter.
    fn visit_param(&mut self, param: &'ast Param, _arena: &'ast ExprArena) {
        // Parameters have no child expressions
        let _ = param;
    }

    /// Visit a match arm.
    fn visit_match_arm(&mut self, arm: &'ast MatchArm, arena: &'ast ExprArena) {
        walk_match_arm(self, arm, arena);
    }

    /// Visit a match pattern.
    fn visit_match_pattern(&mut self, pattern: &'ast MatchPattern, arena: &'ast ExprArena) {
        walk_match_pattern(self, pattern, arena);
    }

    /// Visit a binding pattern.
    fn visit_binding_pattern(&mut self, pattern: &'ast BindingPattern) {
        walk_binding_pattern(self, pattern);
    }

    /// Visit a map entry.
    fn visit_map_entry(&mut self, entry: &'ast MapEntry, arena: &'ast ExprArena) {
        self.visit_expr_id(entry.key, arena);
        self.visit_expr_id(entry.value, arena);
    }

    /// Visit a field initializer.
    fn visit_field_init(&mut self, init: &'ast FieldInit, arena: &'ast ExprArena) {
        if let Some(value) = init.value {
            self.visit_expr_id(value, arena);
        }
    }

    /// Visit a struct literal field (for spread syntax).
    fn visit_struct_lit_field(&mut self, field: &'ast StructLitField, arena: &'ast ExprArena) {
        match field {
            StructLitField::Field(init) => self.visit_field_init(init, arena),
            StructLitField::Spread { expr, .. } => self.visit_expr_id(*expr, arena),
        }
    }

    /// Visit a list element (for spread syntax).
    fn visit_list_element(&mut self, element: &'ast ListElement, arena: &'ast ExprArena) {
        match element {
            ListElement::Expr { expr, .. } | ListElement::Spread { expr, .. } => {
                self.visit_expr_id(*expr, arena);
            }
        }
    }

    /// Visit a map element (for spread syntax).
    fn visit_map_element(&mut self, element: &'ast MapElement, arena: &'ast ExprArena) {
        match element {
            MapElement::Entry(entry) => self.visit_map_entry(entry, arena),
            MapElement::Spread { expr, .. } => self.visit_expr_id(*expr, arena),
        }
    }

    /// Visit a sequence binding (`function_seq`).
    fn visit_seq_binding(&mut self, binding: &'ast SeqBinding, arena: &'ast ExprArena) {
        walk_seq_binding(self, binding, arena);
    }

    /// Visit a named expression (`function_exp`).
    fn visit_named_expr(&mut self, named: &'ast NamedExpr, arena: &'ast ExprArena) {
        self.visit_expr_id(named.value, arena);
    }

    /// Visit a call argument.
    fn visit_call_arg(&mut self, arg: &'ast CallArg, arena: &'ast ExprArena) {
        self.visit_expr_id(arg.value, arena);
    }

    /// Visit a `function_seq` construct.
    fn visit_function_seq(&mut self, seq: &'ast FunctionSeq, arena: &'ast ExprArena) {
        walk_function_seq(self, seq, arena);
    }

    /// Visit a `function_exp` construct.
    fn visit_function_exp(&mut self, exp: &'ast FunctionExp, arena: &'ast ExprArena) {
        walk_function_exp(self, exp, arena);
    }
}

// Walk Functions
//
// All walk functions traverse children in depth-first, left-to-right order.
// For expressions with multiple children (e.g., binary operations), the left
// child is visited before the right. For collections (lists, tuples), elements
// are visited in declaration order.

/// Walk a module's children (imports, consts, functions, tests in order).
pub fn walk_module<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    module: &'ast Module,
    arena: &'ast ExprArena,
) {
    for use_def in &module.imports {
        visitor.visit_use(use_def, arena);
    }
    for const_def in &module.consts {
        visitor.visit_const(const_def, arena);
    }
    for function in &module.functions {
        visitor.visit_function(function, arena);
    }
    for test in &module.tests {
        visitor.visit_test(test, arena);
    }
}

/// Walk a function's children.
pub fn walk_function<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    function: &'ast Function,
    arena: &'ast ExprArena,
) {
    for param in arena.get_params(function.params) {
        visitor.visit_param(param, arena);
    }
    // Visit guard clause if present
    if let Some(guard) = function.guard {
        visitor.visit_expr_id(guard, arena);
    }
    visitor.visit_expr_id(function.body, arena);
}

/// Walk a test's children.
pub fn walk_test<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    test: &'ast TestDef,
    arena: &'ast ExprArena,
) {
    for param in arena.get_params(test.params) {
        visitor.visit_param(param, arena);
    }
    visitor.visit_expr_id(test.body, arena);
}

/// Walk an expression's children.
pub fn walk_expr<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    expr: &Expr,
    arena: &'ast ExprArena,
) {
    match &expr.kind {
        // Literals - no children
        ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Char(_)
        | ExprKind::Duration { .. }
        | ExprKind::Size { .. }
        | ExprKind::Unit
        | ExprKind::Ident(_)
        | ExprKind::Const(_)
        | ExprKind::SelfRef
        | ExprKind::FunctionRef(_)
        | ExprKind::HashLength
        | ExprKind::None
        | ExprKind::Error => {}

        // Single child
        ExprKind::Unary { operand, .. } => {
            visitor.visit_expr_id(*operand, arena);
        }
        ExprKind::Try(inner) | ExprKind::Await(inner) | ExprKind::Some(inner) => {
            visitor.visit_expr_id(*inner, arena);
        }
        ExprKind::Cast { expr, .. } => {
            visitor.visit_expr_id(*expr, arena);
        }
        ExprKind::Loop { body } => {
            visitor.visit_expr_id(*body, arena);
        }
        ExprKind::Break(val) | ExprKind::Continue(val) => {
            if val.is_present() {
                visitor.visit_expr_id(*val, arena);
            }
        }
        ExprKind::Ok(inner) | ExprKind::Err(inner) => {
            if inner.is_present() {
                visitor.visit_expr_id(*inner, arena);
            }
        }

        // Two children
        ExprKind::Binary { left, right, .. } => {
            visitor.visit_expr_id(*left, arena);
            visitor.visit_expr_id(*right, arena);
        }
        ExprKind::Index { receiver, index } => {
            visitor.visit_expr_id(*receiver, arena);
            visitor.visit_expr_id(*index, arena);
        }
        ExprKind::Assign { target, value } => {
            visitor.visit_expr_id(*target, arena);
            visitor.visit_expr_id(*value, arena);
        }

        // Field access
        ExprKind::Field { receiver, .. } => {
            visitor.visit_expr_id(*receiver, arena);
        }

        // Calls
        ExprKind::Call { func, args } => {
            visitor.visit_expr_id(*func, arena);
            for arg_id in arena.get_expr_list(*args).iter().copied() {
                visitor.visit_expr_id(arg_id, arena);
            }
        }
        ExprKind::CallNamed { func, args } => {
            visitor.visit_expr_id(*func, arena);
            for arg in arena.get_call_args(*args) {
                visitor.visit_call_arg(arg, arena);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            visitor.visit_expr_id(*receiver, arena);
            for arg_id in arena.get_expr_list(*args).iter().copied() {
                visitor.visit_expr_id(arg_id, arena);
            }
        }
        ExprKind::MethodCallNamed { receiver, args, .. } => {
            visitor.visit_expr_id(*receiver, arena);
            for arg in arena.get_call_args(*args) {
                visitor.visit_call_arg(arg, arena);
            }
        }

        // Control flow
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            visitor.visit_expr_id(*cond, arena);
            visitor.visit_expr_id(*then_branch, arena);
            if else_branch.is_present() {
                visitor.visit_expr_id(*else_branch, arena);
            }
        }
        ExprKind::Match { scrutinee, arms } => {
            visitor.visit_expr_id(*scrutinee, arena);
            for arm in arena.get_arms(*arms) {
                visitor.visit_match_arm(arm, arena);
            }
        }
        ExprKind::For {
            iter, guard, body, ..
        } => {
            visitor.visit_expr_id(*iter, arena);
            if guard.is_present() {
                visitor.visit_expr_id(*guard, arena);
            }
            visitor.visit_expr_id(*body, arena);
        }
        ExprKind::Block { stmts, result } => {
            for stmt in arena.get_stmt_range(*stmts) {
                visitor.visit_stmt(stmt, arena);
            }
            if result.is_present() {
                visitor.visit_expr_id(*result, arena);
            }
        }

        // Binding
        ExprKind::Let { pattern, init, .. } => {
            let pat = arena.get_binding_pattern(*pattern);
            visitor.visit_binding_pattern(pat);
            visitor.visit_expr_id(*init, arena);
        }
        ExprKind::Lambda { params, body, .. } => {
            for param in arena.get_params(*params) {
                visitor.visit_param(param, arena);
            }
            visitor.visit_expr_id(*body, arena);
        }

        // Collections
        ExprKind::List(items) | ExprKind::Tuple(items) => {
            for item_id in arena.get_expr_list(*items).iter().copied() {
                visitor.visit_expr_id(item_id, arena);
            }
        }
        ExprKind::Map(entries) => {
            for entry in arena.get_map_entries(*entries) {
                visitor.visit_map_entry(entry, arena);
            }
        }
        ExprKind::Struct { fields, .. } => {
            for init in arena.get_field_inits(*fields) {
                visitor.visit_field_init(init, arena);
            }
        }
        ExprKind::StructWithSpread { fields, .. } => {
            for field in arena.get_struct_lit_fields(*fields) {
                visitor.visit_struct_lit_field(field, arena);
            }
        }
        ExprKind::ListWithSpread(elements) => {
            for element in arena.get_list_elements(*elements) {
                visitor.visit_list_element(element, arena);
            }
        }
        ExprKind::MapWithSpread(elements) => {
            for element in arena.get_map_elements(*elements) {
                visitor.visit_map_element(element, arena);
            }
        }
        ExprKind::Range {
            start,
            end,
            step,
            inclusive: _,
        } => {
            if start.is_present() {
                visitor.visit_expr_id(*start, arena);
            }
            if end.is_present() {
                visitor.visit_expr_id(*end, arena);
            }
            if step.is_present() {
                visitor.visit_expr_id(*step, arena);
            }
        }

        // Capability provision
        ExprKind::WithCapability { provider, body, .. } => {
            visitor.visit_expr_id(*provider, arena);
            visitor.visit_expr_id(*body, arena);
        }

        // function_seq / function_exp (arena-allocated)
        ExprKind::FunctionSeq(id) => {
            let seq = arena.get_function_seq(*id);
            visitor.visit_function_seq(seq, arena);
        }
        ExprKind::FunctionExp(id) => {
            let exp = arena.get_function_exp(*id);
            visitor.visit_function_exp(exp, arena);
        }
    }
}

/// Walk a statement's children.
pub fn walk_stmt<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    stmt: &'ast Stmt,
    arena: &'ast ExprArena,
) {
    match &stmt.kind {
        StmtKind::Expr(expr_id) => {
            visitor.visit_expr_id(*expr_id, arena);
        }
        StmtKind::Let { pattern, init, .. } => {
            let pat = arena.get_binding_pattern(*pattern);
            visitor.visit_binding_pattern(pat);
            visitor.visit_expr_id(*init, arena);
        }
    }
}

/// Walk a match arm's children.
pub fn walk_match_arm<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    arm: &'ast MatchArm,
    arena: &'ast ExprArena,
) {
    visitor.visit_match_pattern(&arm.pattern, arena);
    if let Some(guard_id) = arm.guard {
        visitor.visit_expr_id(guard_id, arena);
    }
    visitor.visit_expr_id(arm.body, arena);
}

/// Walk a match pattern's children.
pub fn walk_match_pattern<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    pattern: &'ast MatchPattern,
    arena: &'ast ExprArena,
) {
    match pattern {
        MatchPattern::Wildcard | MatchPattern::Binding(_) => {}
        MatchPattern::Literal(expr_id) => {
            visitor.visit_expr_id(*expr_id, arena);
        }
        MatchPattern::Variant { inner, .. } => {
            for id in arena.get_match_pattern_list(*inner) {
                visitor.visit_match_pattern(arena.get_match_pattern(*id), arena);
            }
        }
        MatchPattern::Struct { fields, .. } => {
            for (_, sub_pattern) in fields {
                if let Some(id) = sub_pattern {
                    visitor.visit_match_pattern(arena.get_match_pattern(*id), arena);
                }
            }
        }
        MatchPattern::Tuple(patterns) | MatchPattern::Or(patterns) => {
            for id in arena.get_match_pattern_list(*patterns) {
                visitor.visit_match_pattern(arena.get_match_pattern(*id), arena);
            }
        }
        MatchPattern::List { elements, .. } => {
            for id in arena.get_match_pattern_list(*elements) {
                visitor.visit_match_pattern(arena.get_match_pattern(*id), arena);
            }
        }
        MatchPattern::Range { start, end, .. } => {
            if let Some(start_id) = start {
                visitor.visit_expr_id(*start_id, arena);
            }
            if let Some(end_id) = end {
                visitor.visit_expr_id(*end_id, arena);
            }
        }
        MatchPattern::At { pattern, .. } => {
            visitor.visit_match_pattern(arena.get_match_pattern(*pattern), arena);
        }
    }
}

/// Walk a binding pattern's children.
pub fn walk_binding_pattern<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    pattern: &'ast BindingPattern,
) {
    match pattern {
        BindingPattern::Name(_) | BindingPattern::Wildcard => {}
        BindingPattern::Tuple(patterns) => {
            for p in patterns {
                visitor.visit_binding_pattern(p);
            }
        }
        BindingPattern::Struct { fields, .. } => {
            for (_, sub_pattern) in fields {
                if let Some(p) = sub_pattern {
                    visitor.visit_binding_pattern(p);
                }
            }
        }
        BindingPattern::List { elements, .. } => {
            for p in elements {
                visitor.visit_binding_pattern(p);
            }
        }
    }
}

/// Walk a sequence binding's children.
pub fn walk_seq_binding<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    binding: &'ast SeqBinding,
    arena: &'ast ExprArena,
) {
    match binding {
        SeqBinding::Let { pattern, value, .. } => {
            let pat = arena.get_binding_pattern(*pattern);
            visitor.visit_binding_pattern(pat);
            visitor.visit_expr_id(*value, arena);
        }
        SeqBinding::Stmt { expr, .. } => {
            visitor.visit_expr_id(*expr, arena);
        }
    }
}

/// Walk a `function_seq`'s children.
pub fn walk_function_seq<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    seq: &'ast FunctionSeq,
    arena: &'ast ExprArena,
) {
    match seq {
        FunctionSeq::Run {
            bindings, result, ..
        }
        | FunctionSeq::Try {
            bindings, result, ..
        } => {
            for binding in arena.get_seq_bindings(*bindings) {
                visitor.visit_seq_binding(binding, arena);
            }
            visitor.visit_expr_id(*result, arena);
        }
        FunctionSeq::Match {
            scrutinee, arms, ..
        } => {
            visitor.visit_expr_id(*scrutinee, arena);
            for arm in arena.get_arms(*arms) {
                visitor.visit_match_arm(arm, arena);
            }
        }
        FunctionSeq::ForPattern {
            over,
            map,
            arm,
            default,
            ..
        } => {
            visitor.visit_expr_id(*over, arena);
            if let Some(map_expr) = map {
                visitor.visit_expr_id(*map_expr, arena);
            }
            visitor.visit_match_arm(arm, arena);
            visitor.visit_expr_id(*default, arena);
        }
    }
}

/// Walk a `function_exp`'s children.
pub fn walk_function_exp<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    exp: &'ast FunctionExp,
    arena: &'ast ExprArena,
) {
    for named in arena.get_named_exprs(exp.props) {
        visitor.visit_named_expr(named, arena);
    }
}
