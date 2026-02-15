use super::*;
use crate::{ast::ExprKind, Span};

#[test]
fn test_alloc_expr() {
    let mut arena = ExprArena::new();

    let id1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));
    let id2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(2, 3)));

    assert_eq!(id1.index(), 0);
    assert_eq!(id2.index(), 1);
    assert_eq!(arena.expr_count(), 2);

    assert!(matches!(arena.get_expr(id1).kind, ExprKind::Int(1)));
    assert!(matches!(arena.get_expr(id2).kind, ExprKind::Int(2)));
}

#[test]
fn test_alloc_expr_list() {
    let mut arena = ExprArena::new();

    let id1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));
    let id2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(2, 3)));
    let id3 = arena.alloc_expr(Expr::new(ExprKind::Int(3), Span::new(4, 5)));

    let range = arena.alloc_expr_list([id1, id2, id3]);

    assert_eq!(range.len(), 3);
    let list = arena.get_expr_list(range);
    assert_eq!(list, &[id1, id2, id3]);
}

#[test]
fn test_arena_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();

    let mut arena1 = ExprArena::new();
    arena1.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(0, 2)));

    let mut arena2 = ExprArena::new();
    arena2.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(0, 2)));

    let arena3 = ExprArena::new();

    set.insert(arena1);
    set.insert(arena2);
    set.insert(arena3);

    assert_eq!(set.len(), 2);
}

#[test]
fn test_arena_reset() {
    let mut arena = ExprArena::new();

    arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));
    assert_eq!(arena.expr_count(), 1);

    arena.reset();
    assert!(arena.is_empty());
    assert_eq!(arena.expr_count(), 0);
}

#[test]
fn test_alloc_expr_list_inline_empty() {
    let mut arena = ExprArena::new();
    let range = arena.alloc_expr_list_inline(&[]);

    assert!(range.is_empty());
    assert_eq!(range.len(), 0);

    let items = arena.get_expr_list(range);
    assert!(items.is_empty());
}

#[test]
fn test_alloc_expr_list_inline_single() {
    let mut arena = ExprArena::new();
    let id1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));

    let range = arena.alloc_expr_list_inline(&[id1]);

    assert!(!range.is_empty());
    assert_eq!(range.len(), 1);

    let items = arena.get_expr_list(range);
    assert_eq!(items, &[id1]);
}

#[test]
fn test_alloc_expr_list_inline_pair() {
    let mut arena = ExprArena::new();
    let id1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));
    let id2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(2, 3)));

    let range = arena.alloc_expr_list_inline(&[id1, id2]);

    assert_eq!(range.len(), 2);

    let items = arena.get_expr_list(range);
    assert_eq!(items, &[id1, id2]);
}

#[test]
fn test_alloc_expr_list_inline_three_items() {
    let mut arena = ExprArena::new();
    let id1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));
    let id2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(2, 3)));
    let id3 = arena.alloc_expr(Expr::new(ExprKind::Int(3), Span::new(4, 5)));

    let range = arena.alloc_expr_list_inline(&[id1, id2, id3]);

    assert_eq!(range.len(), 3);

    let items = arena.get_expr_list(range);
    assert_eq!(items, &[id1, id2, id3]);
}

#[test]
fn test_alloc_expr_list_inline_many_items() {
    let mut arena = ExprArena::new();
    let ids: Vec<_> = (0..10)
        .map(|i| arena.alloc_expr(Expr::new(ExprKind::Int(i), Span::new(0, 1))))
        .collect();

    let range = arena.alloc_expr_list_inline(&ids);

    assert_eq!(range.len(), 10);

    let items = arena.get_expr_list(range);
    assert_eq!(items, &ids[..]);
}
