//! Tests for the burden entry-net forward dataflow (`compute_burden_entry_nets`).
//!
//! Cyclic-CFG convergence pins: a loop with a non-zero per-iteration whole-var
//! burden delta must CONVERGE (freeze-on-disagree) to a `MergeDisagree`, never
//! oscillate to the iteration cap. The `converged` flag is the observable;
//! pre-fix it does not exist (the field IS the deliverable), so these pins are
//! RED at compile time on the unfixed code.

use ori_ir::Name;
use ori_types::Idx;

use super::{
    balance_verdict, compute_burden_entry_nets, compute_var_block_deltas, BalanceViolationKind,
};
use crate::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcTerminator};
use crate::test_helpers::{b, make_func, v};

/// Single-var burden func over `v(0)`, burden-emitted.
fn make_burden_func(blocks: Vec<ArcBlock>) -> crate::ir::ArcFunction {
    let var_types = vec![Idx::from_raw(0)];
    let mut func = make_func(Vec::new(), Idx::from_raw(0), blocks, var_types);
    func.name = Name::from_raw(1);
    func.burden_emitted = vec![true];
    func
}

fn return_block(n: u32, body: Vec<ArcInstr>) -> ArcBlock {
    ArcBlock {
        id: b(n),
        params: Vec::new(),
        body,
        terminator: ArcTerminator::Return { value: v(0) },
    }
}

fn jump_block(n: u32, body: Vec<ArcInstr>, target: ArcBlockId) -> ArcBlock {
    ArcBlock {
        id: b(n),
        params: Vec::new(),
        body,
        terminator: ArcTerminator::Jump {
            target,
            args: Vec::new(),
        },
    }
}

fn branch_block(n: u32, then_b: ArcBlockId, else_b: ArcBlockId) -> ArcBlock {
    ArcBlock {
        id: b(n),
        params: Vec::new(),
        body: Vec::new(),
        terminator: ArcTerminator::Branch {
            cond: v(0),
            then_block: then_b,
            else_block: else_b,
        },
    }
}

/// Build the canonical 4-block loop with a non-zero per-iteration delta:
///   b0 entry  : Jump b1
///   b1 header : Branch b2 | b3
///   b2 body   : BurdenInc(v0); Jump b1   (back-edge)
///   b3 exit   : Return v0
fn unbalanced_loop_func() -> crate::ir::ArcFunction {
    let entry = jump_block(0, Vec::new(), b(1));
    let header = branch_block(1, b(2), b(3));
    let body = jump_block(2, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
    let exit = return_block(3, Vec::new());
    make_burden_func(vec![entry, header, body, exit])
}

/// Same CFG shape with a balanced cycle (`BurdenInc` paired by `BurdenDec` in
/// the body) — the loop net is 0, so the header predecessors agree and the
/// dataflow converges with NO disagree.
fn balanced_loop_func() -> crate::ir::ArcFunction {
    let entry = jump_block(0, Vec::new(), b(1));
    let header = branch_block(1, b(2), b(3));
    let body = jump_block(
        2,
        vec![
            ArcInstr::BurdenInc { var: v(0) },
            ArcInstr::BurdenDec { var: v(0) },
        ],
        b(1),
    );
    let exit = return_block(3, Vec::new());
    make_burden_func(vec![entry, header, body, exit])
}

// Semantic pin (RED pre-fix) — the unbalanced loop converges to MergeDisagree
// instead of oscillating to the iteration cap. The back-edge predecessor is
// ordered FIRST in preds[header] (the 100_doors-class shape).
#[test]
fn unbalanced_loop_back_edge_first_converges_to_disagree() {
    let func = unbalanced_loop_func();
    // preds[1] orders the back-edge (block 2) before the forward edge (block 0).
    let preds: Vec<Vec<usize>> = vec![Vec::new(), vec![2, 0], vec![1], vec![1]];
    let delta = compute_var_block_deltas(&func, v(0));

    let nets = compute_burden_entry_nets(&func, &preds, &delta);

    assert!(
        nets.converged,
        "unbalanced loop must converge via freeze-on-disagree, not oscillate to the cap"
    );
    assert!(
        !nets.disagree_blocks.is_empty(),
        "a non-zero per-iteration whole-var burden delta must surface as MergeDisagree"
    );
}

// Self-verifying termination clamp — convergence holds regardless of predecessor
// ordering (forward edge first). Guards against an ordering-dependent fix.
#[test]
fn unbalanced_loop_forward_edge_first_converges_to_disagree() {
    let func = unbalanced_loop_func();
    let preds: Vec<Vec<usize>> = vec![Vec::new(), vec![0, 2], vec![1], vec![1]];
    let delta = compute_var_block_deltas(&func, v(0));

    let nets = compute_burden_entry_nets(&func, &preds, &delta);

    assert!(
        nets.converged,
        "convergence must not depend on predecessor ordering"
    );
    assert!(!nets.disagree_blocks.is_empty());
}

// Positive clamp — a genuinely balanced loop converges with NO disagree. A fix
// that froze too eagerly (recording a spurious disagree on an agreeing cycle)
// breaks this cell.
#[test]
fn balanced_loop_converges_without_disagree() {
    let func = balanced_loop_func();
    let preds: Vec<Vec<usize>> = vec![Vec::new(), vec![2, 0], vec![1], vec![1]];
    let delta = compute_var_block_deltas(&func, v(0));

    let nets = compute_burden_entry_nets(&func, &preds, &delta);

    assert!(nets.converged, "a balanced cycle must converge");
    assert!(
        nets.disagree_blocks.is_empty(),
        "a balanced cycle must NOT spuriously report a merge disagreement"
    );
}

// Baseline regression — a straight-line burden func converges; the `converged`
// flag is true on the trivial acyclic case.
#[test]
fn straight_line_converges() {
    let entry = jump_block(0, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
    let exit = return_block(1, vec![ArcInstr::BurdenDec { var: v(0) }]);
    let func = make_burden_func(vec![entry, exit]);
    let preds: Vec<Vec<usize>> = vec![Vec::new(), vec![0]];
    let delta = compute_var_block_deltas(&func, v(0));

    let nets = compute_burden_entry_nets(&func, &preds, &delta);

    assert!(nets.converged);
    assert!(nets.disagree_blocks.is_empty());
}

// The verdict surface still reports MergeDisagree on the unbalanced loop after
// convergence (the disagree is the verdict, not the cap).
#[test]
fn unbalanced_loop_balance_verdict_reports_merge_disagree() {
    let func = unbalanced_loop_func();
    let preds: Vec<Vec<usize>> = vec![Vec::new(), vec![2, 0], vec![1], vec![1]];

    let verdict = balance_verdict(&func, &preds, v(0));

    let Some(violation) = verdict.violation else {
        panic!("unbalanced loop must produce a balance violation")
    };
    assert_eq!(violation.kind, BalanceViolationKind::MergeDisagree);
}
