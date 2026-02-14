//! Tests for AST visitor (`ori_ir::visitor`).
//!
//! These tests verify:
//! - Expression counting via visitor pattern
//! - Literal collection
//! - Identifier collection
//! - Module and function traversal
//! - Deeply nested expressions
//! - Optional children handling

use ori_ir::ast::{
    BinaryOp, Expr, ExprKind, Function, GenericParamRange, MatchArm, MatchPattern, Module, Param,
    ParamRange, Visibility,
};
use ori_ir::visitor::{walk_expr, Visitor};
use ori_ir::{ExprArena, ExprId, Name, Span};

/// Visitor that counts expressions.
struct ExprCounter {
    count: usize,
}

impl<'ast> Visitor<'ast> for ExprCounter {
    fn visit_expr(&mut self, expr: &Expr, arena: &'ast ExprArena) {
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
    fn visit_expr(&mut self, expr: &Expr, arena: &'ast ExprArena) {
        match &expr.kind {
            ExprKind::Int(_) => self.int_count += 1,
            ExprKind::Bool(_) => self.bool_count += 1,
            ExprKind::String(_) => self.string_count += 1,
            _ => {}
        }
        walk_expr(self, expr, arena);
    }
}

/// Visitor that collects identifiers.
struct IdentCollector {
    idents: Vec<u32>,
}

impl<'ast> Visitor<'ast> for IdentCollector {
    fn visit_expr(&mut self, expr: &Expr, arena: &'ast ExprArena) {
        if let ExprKind::Ident(name) = &expr.kind {
            self.idents.push(name.raw());
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
            op: BinaryOp::Add,
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
    let list_items = arena.alloc_expr_list_inline(&[int1, int2, bool1]);
    let list = arena.alloc_expr(Expr::new(ExprKind::List(list_items), Span::new(0, 11)));

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
            else_branch,
        },
        Span::new(0, 21),
    ));

    let mut counter = ExprCounter { count: 0 };
    counter.visit_expr_id(if_expr, &arena);

    assert_eq!(counter.count, 4); // if + cond + then + else
}

#[test]
fn test_visit_function() {
    let mut arena = ExprArena::new();

    let body = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(20, 22)));
    let params = arena.alloc_params([Param {
        name: Name::new(0, 1),
        pattern: None,
        ty: None,
        default: None,
        is_variadic: false,
        span: Span::new(6, 7),
    }]);

    let function = Function {
        name: Name::new(0, 0),
        generics: GenericParamRange::EMPTY,
        params,
        return_ty: None,
        capabilities: Vec::new(),
        where_clauses: Vec::new(),
        guard: None,
        body,
        span: Span::new(0, 22),
        visibility: Visibility::Private,
    };

    let mut counter = ExprCounter { count: 0 };
    counter.visit_function(&function, &arena);

    assert_eq!(counter.count, 1);
}

#[test]
fn test_visit_module() {
    let mut arena = ExprArena::new();

    let body1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));
    let body2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(10, 11)));

    let func1 = Function {
        name: Name::new(0, 0),
        generics: GenericParamRange::EMPTY,
        params: ParamRange::EMPTY,
        return_ty: None,
        capabilities: Vec::new(),
        where_clauses: Vec::new(),
        guard: None,
        body: body1,
        span: Span::new(0, 5),
        visibility: Visibility::Private,
    };

    let func2 = Function {
        name: Name::new(0, 1),
        generics: GenericParamRange::EMPTY,
        params: ParamRange::EMPTY,
        return_ty: None,
        capabilities: Vec::new(),
        where_clauses: Vec::new(),
        guard: None,
        body: body2,
        span: Span::new(10, 15),
        visibility: Visibility::Public,
    };

    let module = Module {
        file_attr: None,
        imports: vec![],
        consts: vec![],
        functions: vec![func1, func2],
        tests: vec![],
        types: vec![],
        traits: vec![],
        impls: vec![],
        extends: vec![],
        def_impls: vec![],
    };

    let mut counter = ExprCounter { count: 0 };
    counter.visit_module(&module, &arena);

    assert_eq!(counter.count, 2);
}

#[test]
fn test_visitor_collect_idents() {
    let mut arena = ExprArena::new();

    let x = arena.alloc_expr(Expr::new(ExprKind::Ident(Name::new(0, 0)), Span::new(0, 1)));
    let y = arena.alloc_expr(Expr::new(ExprKind::Ident(Name::new(0, 1)), Span::new(4, 5)));
    let binary = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Add,
            left: x,
            right: y,
        },
        Span::new(0, 5),
    ));

    let mut collector = IdentCollector { idents: vec![] };
    collector.visit_expr_id(binary, &arena);

    assert_eq!(collector.idents, vec![0, 1]);
}

#[test]
fn test_visit_empty_module() {
    let arena = ExprArena::new();
    let module = Module {
        file_attr: None,
        imports: vec![],
        consts: vec![],
        functions: vec![],
        tests: vec![],
        types: vec![],
        traits: vec![],
        impls: vec![],
        extends: vec![],
        def_impls: vec![],
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
        let list = arena.alloc_expr_list_inline(&[current]);
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

    let empty_list_items = arena.alloc_expr_list_inline(&[]);
    let empty_list = arena.alloc_expr(Expr::new(ExprKind::List(empty_list_items), Span::new(0, 2)));

    let mut counter = ExprCounter { count: 0 };
    counter.visit_expr_id(empty_list, &arena);

    assert_eq!(counter.count, 1); // Just the list itself
}

#[test]
fn test_visit_lambda_with_params() {
    let mut arena = ExprArena::new();

    let body = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(15, 17)));
    let params = arena.alloc_params([
        Param {
            name: Name::new(0, 0),
            pattern: None,
            ty: None,
            default: None,
            is_variadic: false,
            span: Span::new(1, 2),
        },
        Param {
            name: Name::new(0, 1),
            pattern: None,
            ty: None,
            default: None,
            is_variadic: false,
            span: Span::new(4, 5),
        },
    ]);

    let lambda = arena.alloc_expr(Expr::new(
        ExprKind::Lambda {
            params,
            ret_ty: ori_ir::ParsedTypeId::INVALID,
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
            else_branch: ExprId::INVALID,
        },
        Span::new(0, 14),
    ));

    let mut counter = ExprCounter { count: 0 };
    counter.visit_expr_id(if_no_else, &arena);

    // if + cond + then = 3 (no else)
    assert_eq!(counter.count, 3);
}
