use super::*;
use crate::Span;

#[test]
fn test_expr_kind_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();

    set.insert(ExprKind::Int(42));
    set.insert(ExprKind::Int(42));
    set.insert(ExprKind::Int(43));
    set.insert(ExprKind::Bool(true));

    assert_eq!(set.len(), 3);
}

#[test]
fn test_binary_op() {
    let op = BinaryOp::Add;
    assert_eq!(op, BinaryOp::Add);
    assert_ne!(op, BinaryOp::Sub);
}

#[test]
fn test_expr_spanned() {
    use crate::Spanned;
    let expr = Expr::new(ExprKind::Int(42), Span::new(0, 2));
    assert_eq!(expr.span().start, 0);
    assert_eq!(expr.span().end, 2);
}

#[test]
fn test_module_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();

    let m1 = Module::new();
    let m2 = Module::new();

    set.insert(m1);
    set.insert(m2);

    assert_eq!(set.len(), 1);
}

#[test]
fn test_function_exp_kind() {
    assert_eq!(FunctionExpKind::Parallel, FunctionExpKind::Parallel);
    assert_ne!(FunctionExpKind::Parallel, FunctionExpKind::Spawn);
    assert_eq!(FunctionExpKind::Parallel.name(), "parallel");
}
