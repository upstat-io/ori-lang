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

// Post-dominator tree tests

/// Linear chain: B0 → B1 → B2 (Return). C post-dominates B0 and B1.
#[test]
fn post_dom_linear() {
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

    let pdom = PostDominatorTree::build(&func);

    // B2 (exit) post-dominates everything.
    assert!(pdom.post_dominates(b(2), b(0)));
    assert!(pdom.post_dominates(b(2), b(1)));
    assert!(pdom.post_dominates(b(2), b(2)));

    // B1 post-dominates B0 (every path from B0 goes through B1).
    assert!(pdom.post_dominates(b(1), b(0)));

    // B0 does NOT post-dominate B1 or B2.
    assert!(!pdom.post_dominates(b(0), b(1)));
    assert!(!pdom.post_dominates(b(0), b(2)));
}

/// Diamond: B0 → B1/B2, both → B3 (Return).
/// B3 post-dominates everything. B1 does NOT post-dominate B0.
#[test]
fn post_dom_diamond() {
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

    let pdom = PostDominatorTree::build(&func);

    // B3 (return block) post-dominates all blocks.
    assert!(pdom.post_dominates(b(3), b(0)));
    assert!(pdom.post_dominates(b(3), b(1)));
    assert!(pdom.post_dominates(b(3), b(2)));
    assert!(pdom.post_dominates(b(3), b(3)));

    // B1 does NOT post-dominate B0 (B0 can reach exit via B2).
    assert!(!pdom.post_dominates(b(1), b(0)));
    // B2 does NOT post-dominate B0 (B0 can reach exit via B1).
    assert!(!pdom.post_dominates(b(2), b(0)));
    // Branches don't post-dominate each other.
    assert!(!pdom.post_dominates(b(1), b(2)));
    assert!(!pdom.post_dominates(b(2), b(1)));
}

/// Multiple exits: B0 → B1 (Return), B0 → B2 (Return).
/// Neither B1 nor B2 post-dominates B0 (both are independent exits).
#[test]
fn post_dom_multiple_exits() {
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
                terminator: ArcTerminator::Return { value: v(0) },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            },
        ],
        vec![Idx::INT, Idx::BOOL],
    );

    let pdom = PostDominatorTree::build(&func);

    // Neither branch post-dominates B0 — both are exits.
    assert!(!pdom.post_dominates(b(1), b(0)));
    assert!(!pdom.post_dominates(b(2), b(0)));

    // Each post-dominates itself.
    assert!(pdom.post_dominates(b(0), b(0)));
    assert!(pdom.post_dominates(b(1), b(1)));
    assert!(pdom.post_dominates(b(2), b(2)));
}

/// Loop: B0 → B1 (header), B1 → B2 (body), B2 → B1 (back edge), B1 → B3 (exit, Return).
/// B3 (exit) post-dominates the loop header B1.
#[test]
fn post_dom_loop() {
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

    let pdom = PostDominatorTree::build(&func);

    // B3 (exit) post-dominates the loop header B1 and entry B0.
    assert!(pdom.post_dominates(b(3), b(0)));
    assert!(pdom.post_dominates(b(3), b(1)));

    // B1 post-dominates B0 (only path from B0 goes through B1).
    assert!(pdom.post_dominates(b(1), b(0)));

    // B2 (loop body) does NOT post-dominate B1 (B1 can exit to B3).
    assert!(!pdom.post_dominates(b(2), b(1)));
}

/// Empty function produces empty `PostDominatorTree`.
#[test]
fn post_dom_empty() {
    let func = make_func(vec![], Idx::UNIT, vec![], vec![]);
    let pdom = PostDominatorTree::build(&func);
    assert!(pdom.ipdom.is_empty());
}
