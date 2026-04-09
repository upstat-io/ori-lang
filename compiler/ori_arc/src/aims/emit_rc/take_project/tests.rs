//! Unit tests for take-project facts (`TakeMoveFacts`).
//!
//! These tests exercise the helper functions directly with synthetic
//! `ArcFunction` shapes, bypassing `is_take_project` (which requires a
//! `Pool` with concrete sum-type and iterator-payload types). They
//! pin the per-source bypass-safe split (TPR-07-019) at the IR level.
//!
//! End-to-end behavior with real Ori source is covered by the AOT
//! `iterator_drop` test suite in
//! `compiler/ori_llvm/tests/aot/iterator_drop.rs`.

use rustc_hash::FxHashSet;

use ori_types::Idx;

use crate::ir::{ArcBlock, ArcInstr, ArcTerminator, ArcValue, ArcVarId};
use crate::test_helpers::{b, make_func, v};

use super::{
    build_alias_graph, compute_bypass_safe_blocks, compute_bypass_safe_entries, compute_lineage,
    compute_lineage_bypass_safe_entries,
};

// ---------------------------------------------------------------------
// Helpers for building synthetic CFG shapes.
// ---------------------------------------------------------------------

/// Build a block with a `Jump` terminator and the given param list.
fn block_jump(id: u32, params: Vec<ArcVarId>, target: u32, args: Vec<ArcVarId>) -> ArcBlock {
    ArcBlock {
        id: b(id),
        params: params.into_iter().map(|var| (var, Idx::INT)).collect(),
        body: vec![],
        terminator: ArcTerminator::Jump {
            target: b(target),
            args,
        },
    }
}

/// Build a block with a `Branch` terminator (used to create
/// multi-successor topology).
fn block_branch(id: u32, cond: ArcVarId, then_target: u32, else_target: u32) -> ArcBlock {
    ArcBlock {
        id: b(id),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Branch {
            cond,
            then_block: b(then_target),
            else_block: b(else_target),
        },
    }
}

/// Build a `Return` block (no successors — terminates a CFG path).
fn block_return(id: u32, value: ArcVarId) -> ArcBlock {
    ArcBlock {
        id: b(id),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value },
    }
}

// ---------------------------------------------------------------------
// `compute_bypass_safe_blocks`
// ---------------------------------------------------------------------

/// A take-project block is NEVER bypass-safe (it is in both forward
/// and backward reachability sets — `is_ownership_transfer` covers
/// its drop at the `Project` site).
#[test]
fn bypass_safe_blocks_excludes_tp_block_itself() {
    // bb0 → bb1 (tp) → bb2 (return)
    let blocks = vec![
        block_jump(0, vec![], 1, vec![]),
        block_jump(1, vec![], 2, vec![]),
        block_return(2, v(0)),
    ];
    let func = make_func(vec![], Idx::INT, blocks, vec![Idx::INT]);
    let predecessors = crate::graph::compute_predecessors(&func);
    let bypass = compute_bypass_safe_blocks(&func, &[1], &predecessors);
    assert!(
        !bypass.contains(&1),
        "tp_block itself must not be bypass-safe"
    );
    // bb0 is backward-reachable from bb1; bb2 is forward-reachable.
    // Both are excluded.
    assert!(!bypass.contains(&0));
    assert!(!bypass.contains(&2));
}

/// On a Branch diamond, the sibling-arm block is bypass-safe — it is
/// neither forward- nor backward-reachable from the take-project arm.
#[test]
fn bypass_safe_blocks_includes_diamond_sibling() {
    // bb0 (Branch) → bb1 (tp) → bb3 (merge)
    //              → bb2 (skip) → bb3 (merge)
    let blocks = vec![
        block_branch(0, v(0), 1, 2),
        block_jump(1, vec![], 3, vec![]),
        block_jump(2, vec![], 3, vec![]),
        block_return(3, v(0)),
    ];
    let func = make_func(vec![], Idx::INT, blocks, vec![Idx::INT]);
    let predecessors = crate::graph::compute_predecessors(&func);
    let bypass = compute_bypass_safe_blocks(&func, &[1], &predecessors);
    assert!(
        bypass.contains(&2),
        "sibling skip-arm bb2 must be bypass-safe relative to tp_block bb1, got {bypass:?}"
    );
    assert!(!bypass.contains(&1), "tp_block bb1 must NOT be bypass-safe");
    assert!(
        !bypass.contains(&0),
        "branch bb0 must NOT be bypass-safe (backward-reachable)"
    );
    assert!(
        !bypass.contains(&3),
        "merge bb3 must NOT be bypass-safe (forward-reachable)"
    );
}

// ---------------------------------------------------------------------
// `compute_bypass_safe_entries`
// ---------------------------------------------------------------------

/// A bypass-safe block whose only predecessor is also bypass-safe is
/// NOT an entry — its drop is inherited from the upstream entry.
#[test]
fn bypass_safe_entries_excludes_inherited_blocks() {
    // bb0 (Branch) → bb1 (tp) → bb4 (return)
    //              → bb2 (skip-A) → bb3 (skip-B) → bb4 (return)
    let blocks = vec![
        block_branch(0, v(0), 1, 2),
        block_jump(1, vec![], 4, vec![]),
        block_jump(2, vec![], 3, vec![]),
        block_jump(3, vec![], 4, vec![]),
        block_return(4, v(0)),
    ];
    let func = make_func(vec![], Idx::INT, blocks, vec![Idx::INT]);
    let predecessors = crate::graph::compute_predecessors(&func);
    let bypass = compute_bypass_safe_blocks(&func, &[1], &predecessors);
    let entries = compute_bypass_safe_entries(0, &bypass, &predecessors);
    // bb2 (skip-A) is the entry — its pred bb0 is NOT bypass-safe.
    assert!(
        entries.contains(&2),
        "bb2 must be a bypass-safe entry, got {entries:?}"
    );
    // bb3 (skip-B) is bypass-safe but its only pred bb2 IS bypass-safe
    // → bb3 inherits the dec, must NOT be an entry.
    assert!(
        !entries.contains(&3),
        "bb3 must NOT be a bypass-safe entry, got {entries:?}"
    );
}

/// regression: the function entry block is treated as
/// having an implicit "outside caller" predecessor. A loop-headed
/// function whose entry is also a bypass-safe loop body block must
/// still qualify as an entry edge.
#[test]
fn bypass_safe_entries_treats_function_entry_specially() {
    // entry block: bb0 (loop header)
    // bb0 → bb1 (Branch on cond)
    // bb1 → bb2 (latch jumps back to bb0) | bb3 (exit returns)
    // bb2 → bb0 (back-edge)
    // bb3 (return)
    // tp_block: bb4 (unreachable from this loop, on a separate path)
    // The loop bb0/bb1/bb2 form a bypass-safe region for tp_block bb4.
    let blocks = vec![
        block_jump(0, vec![], 1, vec![]),
        block_branch(1, v(0), 2, 3),
        block_jump(2, vec![], 0, vec![]),
        block_return(3, v(0)),
        block_return(4, v(0)),
    ];
    let func = make_func(vec![], Idx::INT, blocks, vec![Idx::INT]);
    let predecessors = crate::graph::compute_predecessors(&func);
    // tp_block bb4 is unreachable from bb0/bb1/bb2 → those are bypass-safe.
    let bypass = compute_bypass_safe_blocks(&func, &[4], &predecessors);
    assert!(bypass.contains(&0), "loop header bb0 must be bypass-safe");
    let entries = compute_bypass_safe_entries(0, &bypass, &predecessors);
    // bb0 is the function entry block AND has only a bypass-safe
    // predecessor (bb2 latch). With the function-entry special case,
    // it qualifies as an entry edge.
    assert!(
        entries.contains(&0),
        "function entry block bb0 must qualify as a bypass-safe entry even when its only \
         CFG pred is also bypass-safe, got {entries:?}"
    );
}

// ---------------------------------------------------------------------
// `compute_lineage`
// ---------------------------------------------------------------------

/// A take-project source has lineage `{i}` — its own index — and any
/// `Let dst = Var(src)` chain inherits the same lineage.
#[test]
fn lineage_let_alias_chain_inherits_singleton() {
    // bb0:
    //   Let %1 = Var(%0)
    //   Let %2 = Var(%1)
    //   Return %2
    let blocks = vec![ArcBlock {
        id: b(0),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: v(1),
                ty: Idx::INT,
                value: ArcValue::Var(v(0)),
            },
            ArcInstr::Let {
                dst: v(2),
                ty: Idx::INT,
                value: ArcValue::Var(v(1)),
            },
        ],
        terminator: ArcTerminator::Return { value: v(2) },
    }];
    let func = make_func(vec![], Idx::INT, blocks, vec![Idx::INT; 3]);
    // tp_source 0 = (bb0, %0)
    let lineage = compute_lineage(&func, &[(0, v(0))]);
    assert_eq!(lineage.get(&v(0)), Some(&vec![0]));
    assert_eq!(lineage.get(&v(1)), Some(&vec![0]));
    assert_eq!(lineage.get(&v(2)), Some(&vec![0]));
}

/// Forward-only lineage: if `%a` is a take-project source and `%b` is
/// ALSO a take-project source unrelated to `%a`, BUT they both flow
/// into the same phi-style block param `%p`, then `%p` has mixed
/// lineage `{0, 1}` while `%a` and `%b` retain singleton lineages
/// `{0}` and `{1}` respectively. They do NOT inherit each other's
/// lineage via the phi.
#[test]
fn lineage_forward_only_phi_does_not_back_propagate() {
    // bb0 (Branch on cond) → bb1 (Jump merge(%a)) → bb3 (param: %p)
    //                      → bb2 (Jump merge(%b)) → bb3
    // bb3: Return %p
    let blocks = vec![
        block_branch(0, v(2), 1, 2),
        block_jump(1, vec![], 3, vec![v(0)]),
        block_jump(2, vec![], 3, vec![v(1)]),
        block_return(3, v(3)),
    ];
    // Manually set bb3.params = [%3] (the phi param).
    let mut blocks = blocks;
    blocks[3].params = vec![(v(3), Idx::INT)];
    let func = make_func(vec![], Idx::INT, blocks, vec![Idx::INT; 4]);
    // Two take-project sources: %0 in bb1, %1 in bb2.
    let lineage = compute_lineage(&func, &[(1, v(0)), (2, v(1))]);
    assert_eq!(
        lineage.get(&v(0)),
        Some(&vec![0]),
        "%0 must have singleton lineage {{0}}, not inherit from %1 via phi"
    );
    assert_eq!(
        lineage.get(&v(1)),
        Some(&vec![1]),
        "%1 must have singleton lineage {{1}}, not inherit from %0 via phi"
    );
    assert_eq!(
        lineage.get(&v(3)),
        Some(&vec![0, 1]),
        "phi param %3 must have mixed lineage {{0, 1}} from both incoming args"
    );
}

/// `build_alias_graph` is asymmetric: Let edges are bidirectional
/// (SSA aliases) but Jump-arg → block-param edges are forward-only
/// (phi merges are choices, not shared storage).
#[test]
fn alias_graph_let_bidirectional_jump_forward_only() {
    let blocks = vec![
        ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: v(1),
                ty: Idx::INT,
                value: ArcValue::Var(v(0)),
            }],
            terminator: ArcTerminator::Jump {
                target: b(1),
                args: vec![v(1)],
            },
        },
        ArcBlock {
            id: b(1),
            params: vec![(v(2), Idx::INT)],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(2) },
        },
    ];
    let func = make_func(vec![], Idx::INT, blocks, vec![Idx::INT; 3]);
    let graph = build_alias_graph(&func);
    // Let is bidirectional: %0 ↔ %1
    assert!(
        graph.get(&v(0)).is_some_and(|s| s.contains(&v(1))),
        "Let {{ dst: %1, src: %0 }} must produce edge %0 → %1"
    );
    assert!(
        graph.get(&v(1)).is_some_and(|s| s.contains(&v(0))),
        "Let {{ dst: %1, src: %0 }} must ALSO produce reverse edge %1 → %0 \
         (Let aliases are SSA-equivalent — they name the same value)"
    );
    // Jump arg is forward-only: %1 → %2 (param)
    assert!(
        graph.get(&v(1)).is_some_and(|s| s.contains(&v(2))),
        "Jump arg %1 → block-param %2 must produce edge %1 → %2"
    );
    // No reverse edge: %2 should NOT have %1 in its successors via Jump.
    assert!(
        !graph
            .get(&v(2))
            .is_some_and(|s: &Vec<ArcVarId>| s.contains(&v(1))),
        "Jump arg %1 → block-param %2 must NOT produce reverse edge %2 → %1 \
         (phi params are CFG choices, not shared storage)"
    );
}

// ---------------------------------------------------------------------
// `compute_lineage_bypass_safe_entries`
// ---------------------------------------------------------------------

/// For a singleton lineage, the result equals the entries of that
/// single source's bypass-safe set — same as TPR-07-017 behavior.
#[test]
fn lineage_bypass_safe_entries_singleton_matches_per_source() {
    // bb0 (Branch) → bb1 (tp) → bb3 (return)
    //              → bb2 (bypass) → bb3 (return)
    let blocks = vec![
        block_branch(0, v(0), 1, 2),
        block_jump(1, vec![], 3, vec![]),
        block_jump(2, vec![], 3, vec![]),
        block_return(3, v(0)),
    ];
    let func = make_func(vec![], Idx::INT, blocks, vec![Idx::INT]);
    let predecessors = crate::graph::compute_predecessors(&func);
    let source_bypass = vec![compute_bypass_safe_blocks(&func, &[1], &predecessors)];
    let entries = compute_lineage_bypass_safe_entries(&[0], &source_bypass, 0, &predecessors);
    // bb2 should be the entry — pred bb0 is not bypass-safe.
    assert!(entries.contains(&2));
    assert_eq!(entries.len(), 1);
}

/// TPR-07-019 semantic pin: for a mixed lineage merging two unrelated
/// sources, the per-source intersection produces a STRICTLY TIGHTER
/// bypass-safe set than either source alone. Pre-fix code (TPR-07-017)
/// computed bypass-safe from the merged `tp_blocks` set, which is the
/// SAME mathematical operation as the intersection of per-source sets
/// — both produce the SAME small set. The TPR-07-019 fix's value is
/// in the SINGLETON lineages: `compute_lineage` correctly assigns
/// singleton `{0}` and `{1}` to the source vars themselves (not the
/// over-approximating `{0, 1}` that the union-find membership would
/// give them). This test pins the intersection semantics for the phi-
/// merge case alongside the singleton case.
#[test]
fn lineage_bypass_safe_entries_phi_merged_uses_intersection() {
    // Two-tp shape:
    //   bb0 (Branch flag1) → bb1 (tp_a) → bb6 (return)
    //                      → bb2 (Branch flag2)
    //                              → bb3 (tp_b) → bb6
    //                              → bb4 (skip both) → bb5 (skip) → bb6
    let blocks = vec![
        block_branch(0, v(0), 1, 2),
        block_jump(1, vec![], 6, vec![]),
        block_branch(2, v(1), 3, 4),
        block_jump(3, vec![], 6, vec![]),
        block_jump(4, vec![], 5, vec![]),
        block_jump(5, vec![], 6, vec![]),
        block_return(6, v(0)),
    ];
    let func = make_func(vec![], Idx::INT, blocks, vec![Idx::INT; 2]);
    let predecessors = crate::graph::compute_predecessors(&func);
    // tp_a in bb1, tp_b in bb3.
    let source_bypass = vec![
        compute_bypass_safe_blocks(&func, &[1], &predecessors),
        compute_bypass_safe_blocks(&func, &[3], &predecessors),
    ];
    // Singleton lineage {0} (tp_a only): includes bb3 + every block
    // in bb3's reachability that is not in bb1's reachability.
    let entries_a = compute_lineage_bypass_safe_entries(&[0], &source_bypass, 0, &predecessors);
    assert!(
        !entries_a.is_empty(),
        "singleton lineage {{0}} should have non-empty entries: bb3 is bypass-safe for tp_a, \
         got {entries_a:?}"
    );
    // Mixed lineage {0, 1}: intersection — strictly tighter or equal
    // to each singleton. bb3 is in source_bypass[0] (bypass-safe for
    // tp_a) but NOT in source_bypass[1] (it IS tp_b's block) → bb3
    // is NOT in the mixed-lineage entries.
    let entries_mixed =
        compute_lineage_bypass_safe_entries(&[0, 1], &source_bypass, 0, &predecessors);
    assert!(
        !entries_mixed.contains(&3),
        "bb3 (tp_b's block) must NOT be in the mixed-lineage entries — it's contaminated by \
         source 1, got {entries_mixed:?}"
    );
    // Each singleton lineage is strictly more permissive than the
    // mixed lineage on at least one block.
    let extra_in_a: FxHashSet<usize> = entries_a.difference(&entries_mixed).copied().collect();
    assert!(
        !extra_in_a.is_empty(),
        "singleton lineage {{0}} must include at least one block that the mixed lineage \
         excludes (proves the per-source split is strictly tighter than the merged set), \
         got entries_a={entries_a:?}, entries_mixed={entries_mixed:?}"
    );
}

/// An empty lineage (no sources) yields no bypass-safe entries.
/// Defensive guard against future callers passing degenerate input.
#[test]
fn lineage_bypass_safe_entries_empty_lineage() {
    let blocks = vec![block_return(0, v(0))];
    let func = make_func(vec![], Idx::INT, blocks, vec![Idx::INT]);
    let predecessors = crate::graph::compute_predecessors(&func);
    let entries = compute_lineage_bypass_safe_entries(&[], &[], 0, &predecessors);
    assert!(entries.is_empty());
}
