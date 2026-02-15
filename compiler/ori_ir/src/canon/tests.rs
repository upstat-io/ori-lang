use std::mem;

use crate::{Name, Span, TypeId};

use super::*;

// ── Size Assertions ─────────────────────────────────────────

#[test]
fn can_expr_size() {
    assert_eq!(mem::size_of::<CanExpr>(), 24);
}

#[test]
fn can_id_size() {
    assert_eq!(mem::size_of::<CanId>(), 4);
}

#[test]
fn can_range_size() {
    assert_eq!(mem::size_of::<CanRange>(), 8);
}

#[test]
fn can_map_entry_range_size() {
    assert_eq!(mem::size_of::<CanMapEntryRange>(), 8);
}

#[test]
fn can_field_range_size() {
    assert_eq!(mem::size_of::<CanFieldRange>(), 8);
}

#[test]
fn constant_id_size() {
    assert_eq!(mem::size_of::<ConstantId>(), 4);
}

#[test]
fn decision_tree_id_size() {
    assert_eq!(mem::size_of::<DecisionTreeId>(), 4);
}

// ── CanId ───────────────────────────────────────────────────

#[test]
fn can_id_invalid() {
    assert!(!CanId::INVALID.is_valid());
    assert!(CanId::new(0).is_valid());
    assert!(CanId::new(42).is_valid());
}

#[test]
fn can_id_default_is_invalid() {
    let id: CanId = CanId::default();
    assert!(!id.is_valid());
}

#[test]
fn can_id_debug() {
    assert_eq!(format!("{:?}", CanId::INVALID), "CanId::INVALID");
    assert_eq!(format!("{:?}", CanId::new(5)), "CanId(5)");
}

// ── CanRange ────────────────────────────────────────────────

#[test]
fn can_range_empty() {
    assert!(CanRange::EMPTY.is_empty());
    assert_eq!(CanRange::EMPTY.len(), 0);
}

#[test]
fn can_range_new() {
    let r = CanRange::new(10, 5);
    assert_eq!(r.start, 10);
    assert_eq!(r.len(), 5);
    assert!(!r.is_empty());
}

#[test]
fn can_range_debug() {
    let r = CanRange::new(5, 3);
    assert_eq!(format!("{r:?}"), "CanRange(5..8)");
}

// ── CanArena ────────────────────────────────────────────────

#[test]
fn arena_push_and_get() {
    let mut arena = CanArena::new();
    let node = CanNode::new(CanExpr::Int(42), Span::DUMMY, TypeId::INT);
    let id = arena.push(node);

    assert_eq!(*arena.kind(id), CanExpr::Int(42));
    assert_eq!(arena.span(id), Span::DUMMY);
    assert_eq!(arena.ty(id), TypeId::INT);
    assert_eq!(arena.len(), 1);
}

#[test]
fn arena_multiple_nodes() {
    let mut arena = CanArena::new();
    let id1 = arena.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
    let id2 = arena.push(CanNode::new(CanExpr::Bool(true), Span::DUMMY, TypeId::BOOL));
    let id3 = arena.push(CanNode::new(CanExpr::Unit, Span::DUMMY, TypeId::UNIT));

    assert_eq!(id1.raw(), 0);
    assert_eq!(id2.raw(), 1);
    assert_eq!(id3.raw(), 2);
    assert_eq!(arena.len(), 3);

    assert_eq!(*arena.kind(id1), CanExpr::Int(1));
    assert_eq!(*arena.kind(id2), CanExpr::Bool(true));
    assert_eq!(*arena.kind(id3), CanExpr::Unit);
}

#[test]
fn arena_expr_list() {
    let mut arena = CanArena::new();
    let id1 = arena.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
    let id2 = arena.push(CanNode::new(CanExpr::Int(2), Span::DUMMY, TypeId::INT));
    let id3 = arena.push(CanNode::new(CanExpr::Int(3), Span::DUMMY, TypeId::INT));

    let range = arena.push_expr_list(&[id1, id2, id3]);
    assert_eq!(range.len(), 3);

    let ids = arena.get_expr_list(range);
    assert_eq!(ids, &[id1, id2, id3]);
}

#[test]
fn arena_empty_expr_list() {
    let mut arena = CanArena::new();
    let range = arena.push_expr_list(&[]);
    assert!(range.is_empty());
    assert_eq!(arena.get_expr_list(range), &[]);
}

#[test]
fn arena_incremental_expr_list() {
    let mut arena = CanArena::new();
    let id1 = arena.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
    let id2 = arena.push(CanNode::new(CanExpr::Int(2), Span::DUMMY, TypeId::INT));

    let start = arena.start_expr_list();
    arena.push_expr_list_item(id1);
    arena.push_expr_list_item(id2);
    let range = arena.finish_expr_list(start);

    assert_eq!(range.len(), 2);
    assert_eq!(arena.get_expr_list(range), &[id1, id2]);
}

#[test]
fn arena_map_entries() {
    let mut arena = CanArena::new();
    let k = arena.push(CanNode::new(
        CanExpr::Str(Name::EMPTY),
        Span::DUMMY,
        TypeId::STR,
    ));
    let v = arena.push(CanNode::new(CanExpr::Int(42), Span::DUMMY, TypeId::INT));

    let range = arena.push_map_entries(&[CanMapEntry { key: k, value: v }]);
    assert_eq!(range.len(), 1);

    let entries = arena.get_map_entries(range);
    assert_eq!(entries[0].key, k);
    assert_eq!(entries[0].value, v);
}

#[test]
fn arena_fields() {
    let mut arena = CanArena::new();
    let v = arena.push(CanNode::new(CanExpr::Int(0), Span::DUMMY, TypeId::INT));

    let range = arena.push_fields(&[CanField {
        name: Name::from_raw(1),
        value: v,
    }]);
    assert_eq!(range.len(), 1);

    let fields = arena.get_fields(range);
    assert_eq!(fields[0].name, Name::from_raw(1));
    assert_eq!(fields[0].value, v);
}

// ── ConstantPool ────────────────────────────────────────────

#[test]
fn constant_pool_sentinels() {
    let pool = ConstantPool::new();
    assert_eq!(*pool.get(ConstantPool::UNIT), ConstValue::Unit);
    assert_eq!(*pool.get(ConstantPool::TRUE), ConstValue::Bool(true));
    assert_eq!(*pool.get(ConstantPool::FALSE), ConstValue::Bool(false));
    assert_eq!(*pool.get(ConstantPool::ZERO), ConstValue::Int(0));
    assert_eq!(*pool.get(ConstantPool::ONE), ConstValue::Int(1));
    assert_eq!(
        *pool.get(ConstantPool::EMPTY_STR),
        ConstValue::Str(Name::EMPTY)
    );
}

#[test]
fn constant_pool_intern_dedup() {
    let mut pool = ConstantPool::new();
    let id1 = pool.intern(ConstValue::Int(42));
    let id2 = pool.intern(ConstValue::Int(42));
    assert_eq!(id1, id2); // same constant → same ID
}

#[test]
fn constant_pool_intern_distinct() {
    let mut pool = ConstantPool::new();
    let id1 = pool.intern(ConstValue::Int(42));
    let id2 = pool.intern(ConstValue::Int(43));
    assert_ne!(id1, id2); // different constants → different IDs
}

#[test]
fn constant_pool_sentinel_dedup() {
    let mut pool = ConstantPool::new();
    // Interning a sentinel value should return the pre-interned ID.
    let id = pool.intern(ConstValue::Bool(true));
    assert_eq!(id, ConstantPool::TRUE);
}

// ── DecisionTreePool ────────────────────────────────────────

#[test]
fn decision_tree_pool_push_and_get() {
    let mut pool = DecisionTreePool::new();
    let tree = DecisionTree::Leaf {
        arm_index: 0,
        bindings: vec![],
    };
    let id = pool.push(tree.clone());
    assert_eq!(*pool.get(id), tree);
    assert_eq!(pool.len(), 1);
}

// ── CanonResult ─────────────────────────────────────────────

#[test]
fn canon_result_empty() {
    let result = CanonResult::empty();
    assert!(!result.root.is_valid());
    assert!(result.arena.is_empty());
}

// ── CanExpr equality / hashing ──────────────────────────────

#[test]
fn can_expr_eq() {
    assert_eq!(CanExpr::Int(42), CanExpr::Int(42));
    assert_ne!(CanExpr::Int(42), CanExpr::Int(43));
    assert_ne!(CanExpr::Int(42), CanExpr::Float(42));
}

#[test]
fn can_expr_hash_consistency() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(CanExpr::Int(42));
    set.insert(CanExpr::Int(42)); // duplicate
    set.insert(CanExpr::Bool(true));
    assert_eq!(set.len(), 2);
}
