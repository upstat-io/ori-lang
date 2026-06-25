//! Tests for the canonical-IR structural validator (`validate`).
//!
//! The validator is the sole enforcer of the Canon -> All output contract
//! (canon.md §4.3 / impl-hygiene.md PHASE-34): no `TypeId::INFER` survives,
//! every `CanId` child reference is in-bounds, every `CanExpr::Match` carries
//! an in-bounds `DecisionTreeId`, and every `ConstantId` resolves. It runs
//! under `#[cfg(debug_assertions)]` and panics on the first violation.
//!
//! Positive pins assert a well-formed result validates cleanly; negative pins
//! assert each invariant class panics, so the validator cannot be neutered to
//! `()` without a failing test (TEST-13 pseudo-tested guard).

use ori_ir::canon::tree::DecisionTree;
use ori_ir::canon::{
    CanArena, CanExpr, CanId, CanNode, CanonResult, ConstantPool, DecisionTreeId, DecisionTreePool,
};
use ori_ir::{Span, TypeId};

use super::validate;

/// Build a `CanonResult` from an arena, a root, and (optionally) populated
/// constant/decision-tree pools.
fn result_with(
    arena: CanArena,
    root: CanId,
    constants: ConstantPool,
    decision_trees: DecisionTreePool,
) -> CanonResult {
    let mut result = CanonResult::empty();
    result.arena = arena;
    result.constants = constants;
    result.decision_trees = decision_trees;
    result.root = root;
    result
}

// Positive pins — well-formed canonical IR validates without panic.

#[test]
fn valid_leaf_node_passes() {
    // A single resolved `Int` node is a well-formed canonical result.
    let mut arena = CanArena::new();
    let root = arena.push(CanNode::new(CanExpr::Int(42), Span::DUMMY, TypeId::INT));
    let result = result_with(arena, root, ConstantPool::new(), DecisionTreePool::new());
    validate(&result); // no panic
}

#[test]
fn valid_binary_with_in_bounds_children_passes() {
    // Binary node whose left/right both reference allocated, resolved nodes.
    let mut arena = CanArena::new();
    let left = arena.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
    let right = arena.push(CanNode::new(CanExpr::Int(2), Span::DUMMY, TypeId::INT));
    let root = arena.push(CanNode::new(
        CanExpr::Binary {
            op: ori_ir::BinaryOp::Add,
            left,
            right,
        },
        Span::DUMMY,
        TypeId::INT,
    ));
    let result = result_with(arena, root, ConstantPool::new(), DecisionTreePool::new());
    validate(&result);
}

#[test]
fn valid_match_with_in_bounds_decision_tree_passes() {
    // Match node carrying an in-bounds DecisionTreeId (PHASE-34: every Match
    // must reference a compiled tree).
    let mut arena = CanArena::new();
    let scrutinee = arena.push(CanNode::new(CanExpr::Int(0), Span::DUMMY, TypeId::INT));
    let mut trees = DecisionTreePool::new();
    let tree_id = trees.push(DecisionTree::Leaf {
        arm_index: 0,
        bindings: vec![],
    });
    let root = arena.push(CanNode::new(
        CanExpr::Match {
            scrutinee,
            decision_tree: tree_id,
            arms: ori_ir::canon::CanRange::EMPTY,
        },
        Span::DUMMY,
        TypeId::INT,
    ));
    let result = result_with(arena, root, ConstantPool::new(), trees);
    validate(&result);
}

#[test]
fn valid_constant_with_in_bounds_id_passes() {
    // Constant node referencing a pre-interned sentinel id (ConstantPool::new
    // pre-interns the sentinels, so ZERO is in-bounds).
    let mut arena = CanArena::new();
    let root = arena.push(CanNode::new(
        CanExpr::Constant(ConstantPool::ZERO),
        Span::DUMMY,
        TypeId::INT,
    ));
    let result = result_with(arena, root, ConstantPool::new(), DecisionTreePool::new());
    validate(&result);
}

#[test]
fn invalid_root_short_circuits_without_panic() {
    // An error-recovery result (INVALID root) validates to a no-op, never panics.
    let result = CanonResult::empty();
    validate(&result);
}

// Negative pins — each Canon -> All invariant violation panics.

#[test]
#[should_panic(expected = "unresolved type INFER")]
fn unresolved_infer_type_panics() {
    // A node left at TypeId::INFER violates the "all TypeIds resolved" contract.
    let mut arena = CanArena::new();
    let root = arena.push(CanNode::new(CanExpr::Int(7), Span::DUMMY, TypeId::INFER));
    let result = result_with(arena, root, ConstantPool::new(), DecisionTreePool::new());
    validate(&result);
}

#[test]
#[should_panic(expected = "out of bounds")]
fn out_of_bounds_root_panics() {
    // Root CanId past the arena end violates the valid-root contract.
    let mut arena = CanArena::new();
    arena.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
    let result = result_with(
        arena,
        CanId::new(99),
        ConstantPool::new(),
        DecisionTreePool::new(),
    );
    validate(&result);
}

#[test]
#[should_panic(expected = "arena has")]
fn dangling_child_can_id_panics() {
    // Binary node whose `right` references a non-existent node.
    let mut arena = CanArena::new();
    let left = arena.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
    let root = arena.push(CanNode::new(
        CanExpr::Binary {
            op: ori_ir::BinaryOp::Add,
            left,
            right: CanId::new(99), // dangling
        },
        Span::DUMMY,
        TypeId::INT,
    ));
    let result = result_with(arena, root, ConstantPool::new(), DecisionTreePool::new());
    validate(&result);
}

#[test]
#[should_panic(expected = "DecisionTreeId")]
fn out_of_bounds_decision_tree_panics() {
    // Match node whose DecisionTreeId is past the (empty) pool — PHASE-34
    // "every Match carries a compiled tree" violation.
    let mut arena = CanArena::new();
    let scrutinee = arena.push(CanNode::new(CanExpr::Int(0), Span::DUMMY, TypeId::INT));
    let root = arena.push(CanNode::new(
        CanExpr::Match {
            scrutinee,
            decision_tree: DecisionTreeId::new(0), // pool is empty
            arms: ori_ir::canon::CanRange::EMPTY,
        },
        Span::DUMMY,
        TypeId::INT,
    ));
    let result = result_with(arena, root, ConstantPool::new(), DecisionTreePool::new());
    validate(&result);
}

#[test]
#[should_panic(expected = "ConstantId")]
fn out_of_bounds_constant_panics() {
    // Constant node referencing an id past the pool — the "all ConstantId
    // references resolve" violation.
    let mut arena = CanArena::new();
    let constants = ConstantPool::new();
    let Ok(pool_len) = u32::try_from(constants.len()) else {
        panic!("constant pool len does not fit u32")
    };
    let oob = ori_ir::canon::ConstantId::new(pool_len + 10);
    let root = arena.push(CanNode::new(
        CanExpr::Constant(oob),
        Span::DUMMY,
        TypeId::INT,
    ));
    let result = result_with(arena, root, constants, DecisionTreePool::new());
    validate(&result);
}
