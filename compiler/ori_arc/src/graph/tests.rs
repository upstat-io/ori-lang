use ori_types::Idx;

use crate::ir::{ArcBlock, ArcInstr, ArcTerminator, ArcValue};
use crate::test_helpers::{b, make_func, owned_param, v};

use super::*;

/// Single block: entry dominates itself.
#[test]
fn single_block_self_dominance() {
    let func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::INT],
    );

    let dom = DominatorTree::build(&func);
    assert!(dom.dominates(b(0), b(0)));
}

/// Linear chain: B0 → B1 → B2. B0 dominates all.
#[test]
fn linear_chain() {
    let func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(2),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            },
        ],
        vec![Idx::INT],
    );

    let dom = DominatorTree::build(&func);
    // Entry dominates everything
    assert!(dom.dominates(b(0), b(0)));
    assert!(dom.dominates(b(0), b(1)));
    assert!(dom.dominates(b(0), b(2)));
    // B1 dominates B2 but not B0
    assert!(dom.dominates(b(1), b(2)));
    assert!(!dom.dominates(b(1), b(0)));
    // B2 dominates only itself
    assert!(dom.dominates(b(2), b(2)));
    assert!(!dom.dominates(b(2), b(0)));
    assert!(!dom.dominates(b(2), b(1)));
}

/// Diamond: B0 → B1, B0 → B2, B1 → B3, B2 → B3.
/// B0 dominates all; B3 not dominated by B1 or B2.
#[test]
fn diamond() {
    let func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::BOOL,
                    value: ArcValue::Literal(crate::ir::LitValue::Bool(true)),
                }],
                terminator: ArcTerminator::Branch {
                    cond: v(1),
                    then_block: b(1),
                    else_block: b(2),
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(3),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(3),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(3),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            },
        ],
        vec![Idx::INT, Idx::BOOL],
    );

    let dom = DominatorTree::build(&func);
    assert!(dom.dominates(b(0), b(1)));
    assert!(dom.dominates(b(0), b(2)));
    assert!(dom.dominates(b(0), b(3)));
    // Neither branch dominates the merge point
    assert!(!dom.dominates(b(1), b(3)));
    assert!(!dom.dominates(b(2), b(3)));
    // Branches don't dominate each other
    assert!(!dom.dominates(b(1), b(2)));
    assert!(!dom.dominates(b(2), b(1)));
}

/// Loop: B0 → B1 → B2 → B1 (back edge), B1 → B3.
/// B0 dominates all; B1 dominates B2 (and B3).
#[test]
fn loop_cfg() {
    let func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::BOOL,
                    value: ArcValue::Literal(crate::ir::LitValue::Bool(true)),
                }],
                terminator: ArcTerminator::Branch {
                    cond: v(1),
                    then_block: b(2),
                    else_block: b(3),
                },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(3),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            },
        ],
        vec![Idx::INT, Idx::BOOL],
    );

    let dom = DominatorTree::build(&func);
    // B0 → all
    assert!(dom.dominates(b(0), b(1)));
    assert!(dom.dominates(b(0), b(2)));
    assert!(dom.dominates(b(0), b(3)));
    // Loop header dominates body and exit
    assert!(dom.dominates(b(1), b(2)));
    assert!(dom.dominates(b(1), b(3)));
    // Loop body does NOT dominate header (back edge)
    assert!(!dom.dominates(b(2), b(1)));
}

/// `dominated_preorder` returns blocks in the correct order.
#[test]
fn dominated_preorder_diamond() {
    let func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::BOOL,
                    value: ArcValue::Literal(crate::ir::LitValue::Bool(true)),
                }],
                terminator: ArcTerminator::Branch {
                    cond: v(1),
                    then_block: b(1),
                    else_block: b(2),
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(3),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(3),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(3),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            },
        ],
        vec![Idx::INT, Idx::BOOL],
    );

    let dom = DominatorTree::build(&func);
    let subtree = dom.dominated_preorder(b(0), func.blocks.len());
    // All blocks should be in the subtree rooted at entry
    assert_eq!(subtree.len(), 4);
    assert_eq!(subtree[0], b(0)); // root first

    // B1's subtree: just B1 (B3 is not dominated by B1 in a diamond)
    let b1_subtree = dom.dominated_preorder(b(1), func.blocks.len());
    assert_eq!(b1_subtree, vec![b(1)]);
}

#[test]
fn empty_function() {
    let func = make_func(vec![], Idx::UNIT, vec![], vec![]);
    let dom = DominatorTree::build(&func);
    assert!(dom.idom.is_empty());
}
