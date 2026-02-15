use crate::canon::{CanField, CanMapEntry, CanNode, CanParam};
use crate::{Name, Span, TypeId};

use super::*;

/// Build a simple arena with a single Int node.
fn arena_with_int(value: i64) -> (CanArena, CanId) {
    let mut arena = CanArena::new();
    let id = arena.push(CanNode::new(CanExpr::Int(value), Span::DUMMY, TypeId::INT));
    (arena, id)
}

#[test]
fn same_body_same_hash() {
    let (a1, r1) = arena_with_int(42);
    let (a2, r2) = arena_with_int(42);
    assert_eq!(
        hash_canonical_subtree(&a1, r1),
        hash_canonical_subtree(&a2, r2),
    );
}

#[test]
fn different_value_different_hash() {
    let (a1, r1) = arena_with_int(42);
    let (a2, r2) = arena_with_int(43);
    assert_ne!(
        hash_canonical_subtree(&a1, r1),
        hash_canonical_subtree(&a2, r2),
    );
}

#[test]
fn different_type_different_hash() {
    let mut a1 = CanArena::new();
    let r1 = a1.push(CanNode::new(CanExpr::Int(42), Span::DUMMY, TypeId::INT));

    let mut a2 = CanArena::new();
    let r2 = a2.push(CanNode::new(CanExpr::Int(42), Span::DUMMY, TypeId::FLOAT));

    assert_ne!(
        hash_canonical_subtree(&a1, r1),
        hash_canonical_subtree(&a2, r2),
    );
}

#[test]
fn span_does_not_affect_hash() {
    let mut a1 = CanArena::new();
    let r1 = a1.push(CanNode::new(CanExpr::Int(42), Span::new(0, 5), TypeId::INT));

    let mut a2 = CanArena::new();
    let r2 = a2.push(CanNode::new(
        CanExpr::Int(42),
        Span::new(100, 200),
        TypeId::INT,
    ));

    assert_eq!(
        hash_canonical_subtree(&a1, r1),
        hash_canonical_subtree(&a2, r2),
        "span differences should not affect the hash",
    );
}

#[test]
fn binary_expr_hash() {
    let mut arena = CanArena::new();
    let left = arena.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
    let right = arena.push(CanNode::new(CanExpr::Int(2), Span::DUMMY, TypeId::INT));
    let add = arena.push(CanNode::new(
        CanExpr::Binary {
            op: crate::BinaryOp::Add,
            left,
            right,
        },
        Span::DUMMY,
        TypeId::INT,
    ));

    let mut arena2 = CanArena::new();
    let left2 = arena2.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
    let right2 = arena2.push(CanNode::new(CanExpr::Int(2), Span::DUMMY, TypeId::INT));
    let add2 = arena2.push(CanNode::new(
        CanExpr::Binary {
            op: crate::BinaryOp::Add,
            left: left2,
            right: right2,
        },
        Span::DUMMY,
        TypeId::INT,
    ));

    assert_eq!(
        hash_canonical_subtree(&arena, add),
        hash_canonical_subtree(&arena2, add2),
    );
}

#[test]
fn different_operator_different_hash() {
    let mut a1 = CanArena::new();
    let l1 = a1.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
    let r1 = a1.push(CanNode::new(CanExpr::Int(2), Span::DUMMY, TypeId::INT));
    let add = a1.push(CanNode::new(
        CanExpr::Binary {
            op: crate::BinaryOp::Add,
            left: l1,
            right: r1,
        },
        Span::DUMMY,
        TypeId::INT,
    ));

    let mut a2 = CanArena::new();
    let l2 = a2.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
    let r2 = a2.push(CanNode::new(CanExpr::Int(2), Span::DUMMY, TypeId::INT));
    let sub = a2.push(CanNode::new(
        CanExpr::Binary {
            op: crate::BinaryOp::Sub,
            left: l2,
            right: r2,
        },
        Span::DUMMY,
        TypeId::INT,
    ));

    assert_ne!(
        hash_canonical_subtree(&a1, add),
        hash_canonical_subtree(&a2, sub),
    );
}

#[test]
fn block_with_stmts_hash() {
    let mut arena = CanArena::new();
    let s1 = arena.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
    let s2 = arena.push(CanNode::new(CanExpr::Int(2), Span::DUMMY, TypeId::INT));
    let result = arena.push(CanNode::new(CanExpr::Int(3), Span::DUMMY, TypeId::INT));
    let stmts = arena.push_expr_list(&[s1, s2]);
    let block = arena.push(CanNode::new(
        CanExpr::Block { stmts, result },
        Span::DUMMY,
        TypeId::INT,
    ));

    // Same block, different arena
    let mut arena2 = CanArena::new();
    let s1b = arena2.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
    let s2b = arena2.push(CanNode::new(CanExpr::Int(2), Span::DUMMY, TypeId::INT));
    let result2 = arena2.push(CanNode::new(CanExpr::Int(3), Span::DUMMY, TypeId::INT));
    let stmts2 = arena2.push_expr_list(&[s1b, s2b]);
    let block2 = arena2.push(CanNode::new(
        CanExpr::Block {
            stmts: stmts2,
            result: result2,
        },
        Span::DUMMY,
        TypeId::INT,
    ));

    assert_eq!(
        hash_canonical_subtree(&arena, block),
        hash_canonical_subtree(&arena2, block2),
    );
}

#[test]
fn invalid_root_produces_consistent_hash() {
    let arena = CanArena::new();
    let h1 = hash_canonical_subtree(&arena, CanId::INVALID);
    let h2 = hash_canonical_subtree(&arena, CanId::INVALID);
    assert_eq!(h1, h2);
}

#[test]
fn call_expr_hash() {
    let mut arena = CanArena::new();
    let func = arena.push(CanNode::new(
        CanExpr::Ident(Name::from_raw(10)),
        Span::DUMMY,
        TypeId::INT,
    ));
    let arg = arena.push(CanNode::new(CanExpr::Int(42), Span::DUMMY, TypeId::INT));
    let args = arena.push_expr_list(&[arg]);
    let call = arena.push(CanNode::new(
        CanExpr::Call { func, args },
        Span::DUMMY,
        TypeId::INT,
    ));

    // Same call in a different arena
    let mut arena2 = CanArena::new();
    let func2 = arena2.push(CanNode::new(
        CanExpr::Ident(Name::from_raw(10)),
        Span::DUMMY,
        TypeId::INT,
    ));
    let arg2 = arena2.push(CanNode::new(CanExpr::Int(42), Span::DUMMY, TypeId::INT));
    let args2 = arena2.push_expr_list(&[arg2]);
    let call2 = arena2.push(CanNode::new(
        CanExpr::Call {
            func: func2,
            args: args2,
        },
        Span::DUMMY,
        TypeId::INT,
    ));

    assert_eq!(
        hash_canonical_subtree(&arena, call),
        hash_canonical_subtree(&arena2, call2),
    );
}

#[test]
fn struct_expr_hash() {
    let mut arena = CanArena::new();
    let v1 = arena.push(CanNode::new(CanExpr::Int(0), Span::DUMMY, TypeId::INT));
    let v2 = arena.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
    let fields = arena.push_fields(&[
        CanField {
            name: Name::from_raw(1),
            value: v1,
        },
        CanField {
            name: Name::from_raw(2),
            value: v2,
        },
    ]);
    let root = arena.push(CanNode::new(
        CanExpr::Struct {
            name: Name::from_raw(10),
            fields,
        },
        Span::DUMMY,
        TypeId::INT,
    ));

    // Different field values should produce different hash
    let mut arena2 = CanArena::new();
    let v1b = arena2.push(CanNode::new(CanExpr::Int(99), Span::DUMMY, TypeId::INT));
    let v2b = arena2.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
    let fields2 = arena2.push_fields(&[
        CanField {
            name: Name::from_raw(1),
            value: v1b,
        },
        CanField {
            name: Name::from_raw(2),
            value: v2b,
        },
    ]);
    let root2 = arena2.push(CanNode::new(
        CanExpr::Struct {
            name: Name::from_raw(10),
            fields: fields2,
        },
        Span::DUMMY,
        TypeId::INT,
    ));

    assert_ne!(
        hash_canonical_subtree(&arena, root),
        hash_canonical_subtree(&arena2, root2),
    );
}

#[test]
fn lambda_hash() {
    let mut arena = CanArena::new();
    let body = arena.push(CanNode::new(CanExpr::Int(42), Span::DUMMY, TypeId::INT));
    let params = arena.push_params(&[CanParam {
        name: Name::from_raw(1),
        default: CanId::INVALID,
    }]);
    let lambda = arena.push(CanNode::new(
        CanExpr::Lambda { params, body },
        Span::DUMMY,
        TypeId::INT,
    ));

    // Same lambda, different param name → different hash
    let mut arena2 = CanArena::new();
    let body2 = arena2.push(CanNode::new(CanExpr::Int(42), Span::DUMMY, TypeId::INT));
    let params2 = arena2.push_params(&[CanParam {
        name: Name::from_raw(2),
        default: CanId::INVALID,
    }]);
    let lambda2 = arena2.push(CanNode::new(
        CanExpr::Lambda {
            params: params2,
            body: body2,
        },
        Span::DUMMY,
        TypeId::INT,
    ));

    assert_ne!(
        hash_canonical_subtree(&arena, lambda),
        hash_canonical_subtree(&arena2, lambda2),
    );
}

#[test]
fn map_expr_hash() {
    let mut arena = CanArena::new();
    let k = arena.push(CanNode::new(
        CanExpr::Str(Name::from_raw(1)),
        Span::DUMMY,
        TypeId::STR,
    ));
    let v = arena.push(CanNode::new(CanExpr::Int(42), Span::DUMMY, TypeId::INT));
    let entries = arena.push_map_entries(&[CanMapEntry { key: k, value: v }]);
    let root = arena.push(CanNode::new(
        CanExpr::Map(entries),
        Span::DUMMY,
        TypeId::INT,
    ));

    // Different value in map → different hash
    let mut arena2 = CanArena::new();
    let k2 = arena2.push(CanNode::new(
        CanExpr::Str(Name::from_raw(1)),
        Span::DUMMY,
        TypeId::STR,
    ));
    let v2 = arena2.push(CanNode::new(CanExpr::Int(99), Span::DUMMY, TypeId::INT));
    let entries2 = arena2.push_map_entries(&[CanMapEntry { key: k2, value: v2 }]);
    let root2 = arena2.push(CanNode::new(
        CanExpr::Map(entries2),
        Span::DUMMY,
        TypeId::INT,
    ));

    assert_ne!(
        hash_canonical_subtree(&arena, root),
        hash_canonical_subtree(&arena2, root2),
    );
}
