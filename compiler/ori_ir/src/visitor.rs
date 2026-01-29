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
    BindingPattern, CallArg, ConfigDef, Expr, ExprKind, FieldInit, Function, FunctionExp,
    FunctionSeq, MapEntry, MatchArm, MatchPattern, Module, NamedExpr, Param, SeqBinding, Stmt,
    StmtKind, TestDef, UseDef,
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

    /// Visit a config variable definition.
    fn visit_config(&mut self, config: &'ast ConfigDef, arena: &'ast ExprArena) {
        // Walk the config value expression
        self.visit_expr_id(config.value, arena);
    }

    /// Visit an expression.
    fn visit_expr(&mut self, expr: &'ast Expr, arena: &'ast ExprArena) {
        walk_expr(self, expr, arena);
    }

    /// Visit an expression by ID.
    fn visit_expr_id(&mut self, id: ExprId, arena: &'ast ExprArena) {
        self.visit_expr(arena.get_expr(id), arena);
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

/// Walk a module's children (imports, configs, functions, tests in order).
pub fn walk_module<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    module: &'ast Module,
    arena: &'ast ExprArena,
) {
    for use_def in &module.imports {
        visitor.visit_use(use_def, arena);
    }
    for config in &module.configs {
        visitor.visit_config(config, arena);
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
    expr: &'ast Expr,
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
        | ExprKind::Config(_)
        | ExprKind::SelfRef
        | ExprKind::FunctionRef(_)
        | ExprKind::HashLength
        | ExprKind::None
        | ExprKind::Continue
        | ExprKind::Error => {}

        // Single child
        ExprKind::Unary { operand, .. } => {
            visitor.visit_expr_id(*operand, arena);
        }
        ExprKind::Try(inner) | ExprKind::Await(inner) | ExprKind::Some(inner) => {
            visitor.visit_expr_id(*inner, arena);
        }
        ExprKind::Loop { body } => {
            visitor.visit_expr_id(*body, arena);
        }
        ExprKind::Return(val) | ExprKind::Break(val) => {
            if let Some(id) = val {
                visitor.visit_expr_id(*id, arena);
            }
        }
        ExprKind::Ok(inner) | ExprKind::Err(inner) => {
            if let Some(id) = inner {
                visitor.visit_expr_id(*id, arena);
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
            for &arg_id in arena.get_expr_list(*args) {
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
            for &arg_id in arena.get_expr_list(*args) {
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
            if let Some(else_id) = else_branch {
                visitor.visit_expr_id(*else_id, arena);
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
            if let Some(guard_id) = guard {
                visitor.visit_expr_id(*guard_id, arena);
            }
            visitor.visit_expr_id(*body, arena);
        }
        ExprKind::Block { stmts, result } => {
            for stmt in arena.get_stmt_range(*stmts) {
                visitor.visit_stmt(stmt, arena);
            }
            if let Some(result_id) = result {
                visitor.visit_expr_id(*result_id, arena);
            }
        }

        // Binding
        ExprKind::Let { pattern, init, .. } => {
            visitor.visit_binding_pattern(pattern);
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
            for &item_id in arena.get_expr_list(*items) {
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
        ExprKind::Range {
            start,
            end,
            inclusive: _,
        } => {
            if let Some(start_id) = start {
                visitor.visit_expr_id(*start_id, arena);
            }
            if let Some(end_id) = end {
                visitor.visit_expr_id(*end_id, arena);
            }
        }

        // Capability provision
        ExprKind::WithCapability { provider, body, .. } => {
            visitor.visit_expr_id(*provider, arena);
            visitor.visit_expr_id(*body, arena);
        }

        // function_seq / function_exp
        ExprKind::FunctionSeq(seq) => {
            visitor.visit_function_seq(seq, arena);
        }
        ExprKind::FunctionExp(exp) => {
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
            visitor.visit_binding_pattern(pattern);
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
            visitor.visit_binding_pattern(pattern);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ast::ExprKind, Span};

    /// Visitor that counts expressions.
    struct ExprCounter {
        count: usize,
    }

    impl<'ast> Visitor<'ast> for ExprCounter {
        fn visit_expr(&mut self, expr: &'ast Expr, arena: &'ast ExprArena) {
            self.count += 1;
            walk_expr(self, expr, arena);
        }
    }

    /// Visitor that counts literals.
    #[expect(
        clippy::struct_field_names,
        reason = "fields represent distinct literal type counts, _count suffix is intentional"
    )]
    struct LiteralCounter {
        int_count: usize,
        bool_count: usize,
        string_count: usize,
    }

    impl<'ast> Visitor<'ast> for LiteralCounter {
        fn visit_expr(&mut self, expr: &'ast Expr, arena: &'ast ExprArena) {
            match &expr.kind {
                ExprKind::Int(_) => self.int_count += 1,
                ExprKind::Bool(_) => self.bool_count += 1,
                ExprKind::String(_) => self.string_count += 1,
                _ => {}
            }
            walk_expr(self, expr, arena);
        }
    }

    #[test]
    fn test_visit_single_expr() {
        let mut arena = ExprArena::new();
        let expr_id = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(0, 2)));

        let mut counter = ExprCounter { count: 0 };
        counter.visit_expr_id(expr_id, &arena);

        assert_eq!(counter.count, 1);
    }

    #[test]
    fn test_visit_binary_expr() {
        let mut arena = ExprArena::new();

        let left = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));
        let right = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(4, 5)));
        let binary = arena.alloc_expr(Expr::new(
            ExprKind::Binary {
                op: crate::ast::BinaryOp::Add,
                left,
                right,
            },
            Span::new(0, 5),
        ));

        let mut counter = ExprCounter { count: 0 };
        counter.visit_expr_id(binary, &arena);

        assert_eq!(counter.count, 3); // binary + left + right
    }

    #[test]
    fn test_visit_literals() {
        let mut arena = ExprArena::new();

        let int1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));
        let int2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(3, 4)));
        let bool1 = arena.alloc_expr(Expr::new(ExprKind::Bool(true), Span::new(6, 10)));
        let list_range = arena.alloc_expr_list([int1, int2, bool1]);
        let list = arena.alloc_expr(Expr::new(ExprKind::List(list_range), Span::new(0, 11)));

        let mut counter = LiteralCounter {
            int_count: 0,
            bool_count: 0,
            string_count: 0,
        };
        counter.visit_expr_id(list, &arena);

        assert_eq!(counter.int_count, 2);
        assert_eq!(counter.bool_count, 1);
        assert_eq!(counter.string_count, 0);
    }

    #[test]
    fn test_visit_if_expr() {
        let mut arena = ExprArena::new();

        let cond = arena.alloc_expr(Expr::new(ExprKind::Bool(true), Span::new(3, 7)));
        let then_branch = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(13, 14)));
        let else_branch = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(20, 21)));
        let if_expr = arena.alloc_expr(Expr::new(
            ExprKind::If {
                cond,
                then_branch,
                else_branch: Some(else_branch),
            },
            Span::new(0, 21),
        ));

        let mut counter = ExprCounter { count: 0 };
        counter.visit_expr_id(if_expr, &arena);

        assert_eq!(counter.count, 4); // if + cond + then + else
    }

    #[test]
    fn test_visit_function() {
        use crate::ast::{Function, Param};
        use crate::Name;

        let mut arena = ExprArena::new();

        let body = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(20, 22)));
        let params = arena.alloc_params([Param {
            name: Name::new(0, 1),
            ty: None,
            span: Span::new(6, 7),
        }]);

        let function = Function {
            name: Name::new(0, 0),
            generics: crate::ast::GenericParamRange::EMPTY,
            params,
            return_ty: None,
            capabilities: Vec::new(),
            where_clauses: Vec::new(),
            body,
            span: Span::new(0, 22),
            is_public: false,
        };

        let mut counter = ExprCounter { count: 0 };
        counter.visit_function(&function, &arena);

        assert_eq!(counter.count, 1);
    }

    #[test]
    fn test_visit_module() {
        use crate::ast::{Function, Module};
        use crate::Name;

        let mut arena = ExprArena::new();

        let body1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));
        let body2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(10, 11)));

        let func1 = Function {
            name: Name::new(0, 0),
            generics: crate::ast::GenericParamRange::EMPTY,
            params: crate::ast::ParamRange::EMPTY,
            return_ty: None,
            capabilities: Vec::new(),
            where_clauses: Vec::new(),
            body: body1,
            span: Span::new(0, 5),
            is_public: false,
        };

        let func2 = Function {
            name: Name::new(0, 1),
            generics: crate::ast::GenericParamRange::EMPTY,
            params: crate::ast::ParamRange::EMPTY,
            return_ty: None,
            capabilities: Vec::new(),
            where_clauses: Vec::new(),
            body: body2,
            span: Span::new(10, 15),
            is_public: true,
        };

        let module = Module {
            imports: vec![],
            configs: vec![],
            functions: vec![func1, func2],
            tests: vec![],
            types: vec![],
            traits: vec![],
            impls: vec![],
            extends: vec![],
        };

        let mut counter = ExprCounter { count: 0 };
        counter.visit_module(&module, &arena);

        assert_eq!(counter.count, 2);
    }

    /// Test Visitor trait with state accumulation.
    struct IdentCollector {
        idents: Vec<u32>,
    }

    impl<'ast> Visitor<'ast> for IdentCollector {
        fn visit_expr(&mut self, expr: &'ast Expr, arena: &'ast ExprArena) {
            if let ExprKind::Ident(name) = &expr.kind {
                self.idents.push(name.raw());
            }
            walk_expr(self, expr, arena);
        }
    }

    #[test]
    fn test_visitor_collect_idents() {
        use crate::Name;

        let mut arena = ExprArena::new();

        let x = arena.alloc_expr(Expr::new(ExprKind::Ident(Name::new(0, 0)), Span::new(0, 1)));
        let y = arena.alloc_expr(Expr::new(ExprKind::Ident(Name::new(0, 1)), Span::new(4, 5)));
        let binary = arena.alloc_expr(Expr::new(
            ExprKind::Binary {
                op: crate::ast::BinaryOp::Add,
                left: x,
                right: y,
            },
            Span::new(0, 5),
        ));

        let mut collector = IdentCollector { idents: vec![] };
        collector.visit_expr_id(binary, &arena);

        assert_eq!(collector.idents, vec![0, 1]);
    }

    // Edge case tests

    #[test]
    fn test_visit_empty_module() {
        use crate::ast::Module;

        let arena = ExprArena::new();
        let module = Module {
            imports: vec![],
            configs: vec![],
            functions: vec![],
            tests: vec![],
            types: vec![],
            traits: vec![],
            impls: vec![],
            extends: vec![],
        };

        let mut counter = ExprCounter { count: 0 };
        counter.visit_module(&module, &arena);

        assert_eq!(counter.count, 0);
    }

    #[test]
    fn test_visit_deeply_nested_expressions() {
        let mut arena = ExprArena::new();

        // Create a deeply nested expression: ((((1))))
        let mut current = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(4, 5)));

        // Wrap in 10 levels of tuple nesting
        for depth in 0..10 {
            let list = arena.alloc_expr_list([current]);
            current = arena.alloc_expr(Expr::new(
                ExprKind::Tuple(list),
                Span::new(0, (depth + 1) * 2),
            ));
        }

        let mut counter = ExprCounter { count: 0 };
        counter.visit_expr_id(current, &arena);

        // 10 tuple wrappers + 1 inner int = 11 expressions
        assert_eq!(counter.count, 11);
    }

    #[test]
    fn test_visit_empty_list() {
        let mut arena = ExprArena::new();

        let empty_list_range = arena.alloc_expr_list([]);
        let empty_list = arena.alloc_expr(Expr::new(
            ExprKind::List(empty_list_range),
            Span::new(0, 2),
        ));

        let mut counter = ExprCounter { count: 0 };
        counter.visit_expr_id(empty_list, &arena);

        assert_eq!(counter.count, 1); // Just the list itself
    }

    #[test]
    fn test_visit_lambda_with_params() {
        use crate::ast::Param;
        use crate::Name;

        let mut arena = ExprArena::new();

        let body = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(15, 17)));
        let params = arena.alloc_params([
            Param {
                name: Name::new(0, 0),
                ty: None,
                span: Span::new(1, 2),
            },
            Param {
                name: Name::new(0, 1),
                ty: None,
                span: Span::new(4, 5),
            },
        ]);

        let lambda = arena.alloc_expr(Expr::new(
            ExprKind::Lambda {
                params,
                ret_ty: None,
                body,
            },
            Span::new(0, 17),
        ));

        let mut counter = ExprCounter { count: 0 };
        counter.visit_expr_id(lambda, &arena);

        // Lambda + body = 2 expressions
        assert_eq!(counter.count, 2);
    }

    #[test]
    fn test_visit_match_with_guard() {
        use crate::ast::{MatchArm, MatchPattern};
        use crate::Name;

        let mut arena = ExprArena::new();

        let scrutinee = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(6, 8)));
        let guard = arena.alloc_expr(Expr::new(ExprKind::Bool(true), Span::new(15, 19)));
        let body = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(25, 26)));

        let arm = MatchArm {
            pattern: MatchPattern::Binding(Name::new(0, 0)),
            guard: Some(guard),
            body,
            span: Span::new(10, 26),
        };

        let arms = arena.alloc_arms([arm]);
        let match_expr = arena.alloc_expr(Expr::new(
            ExprKind::Match { scrutinee, arms },
            Span::new(0, 27),
        ));

        let mut counter = ExprCounter { count: 0 };
        counter.visit_expr_id(match_expr, &arena);

        // match + scrutinee + guard + body = 4
        assert_eq!(counter.count, 4);
    }

    #[test]
    fn test_visit_optional_children() {
        let mut arena = ExprArena::new();

        // If without else
        let cond = arena.alloc_expr(Expr::new(ExprKind::Bool(true), Span::new(3, 7)));
        let then_branch = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(13, 14)));
        let if_no_else = arena.alloc_expr(Expr::new(
            ExprKind::If {
                cond,
                then_branch,
                else_branch: None,
            },
            Span::new(0, 14),
        ));

        let mut counter = ExprCounter { count: 0 };
        counter.visit_expr_id(if_no_else, &arena);

        // if + cond + then = 3 (no else)
        assert_eq!(counter.count, 3);
    }
}
