//! Convergence-guard + INTERSECT-semantics tests for the Phase-5
//! moved-out-fields fixpoint (`propagate_moved_out_fields`, rule IA-MF1).
//!
//! - T1 (release-active negative pin): a non-converged fixpoint fires the
//!   release-active `assert!` (run under `cargo test --release`; cap forced
//!   low via the `cap_override` parameter).
//! - T2 (positive): convergence across {DAG, cyclic} × {single-, multi-field}
//!   records `converged == true` and emits no guard.
//! - T2b (INTERSECT correctness): a diamond CFG pins RL-2 MUST-move — a field
//!   moved on only ONE predecessor is dropped at the merge; moved on ALL, kept.
//! - T3 (diagnostic quality): the guard panic names the cause + the cap.

use rustc_hash::FxHashSet;

use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcTerminator, ArcVarId};
use crate::lower::burden_lower::BurdenLowerCtx;

use super::dataflow::{derived_convergence_cap, propagate_moved_out_fields};

/// The single owned aggregate var whose fields the tests move.
fn var_a() -> ArcVarId {
    ArcVarId::new(0)
}

/// A scalar var id used for terminator operands (cond / return value); never a
/// moved-field carrier.
fn scalar() -> ArcVarId {
    ArcVarId::new(99)
}

fn fset(items: &[u32]) -> FxHashSet<u32> {
    items.iter().copied().collect()
}

/// Build an `ArcFunction` from a list of `(block_id, terminator)` pairs. Entry
/// is block 0. Bodies are empty — the moved-field bitsets are injected directly
/// into `ctx.moved_out_fields_block_local`, isolating the Pass-3 fixpoint.
fn func_from_terminators(terminators: Vec<ArcTerminator>) -> ArcFunction {
    let blocks = terminators
        .into_iter()
        .enumerate()
        .map(|(i, terminator)| {
            let id = u32::try_from(i).unwrap_or_else(|_| unreachable!("test block index fits u32"));
            ArcBlock {
                id: ArcBlockId::new(id),
                params: Vec::new(),
                body: Vec::new(),
                terminator,
            }
        })
        .collect();
    ArcFunction {
        blocks,
        entry: ArcBlockId::new(0),
        ..Default::default()
    }
}

fn jump(target: u32) -> ArcTerminator {
    ArcTerminator::Jump {
        target: ArcBlockId::new(target),
        args: Vec::new(),
    }
}

fn branch(then_block: u32, else_block: u32) -> ArcTerminator {
    ArcTerminator::Branch {
        cond: scalar(),
        then_block: ArcBlockId::new(then_block),
        else_block: ArcBlockId::new(else_block),
    }
}

fn ret() -> ArcTerminator {
    ArcTerminator::Return { value: scalar() }
}

/// Straight-line DAG: block0 -> block1(return).
fn straight_line() -> ArcFunction {
    func_from_terminators(vec![jump(1), ret()])
}

/// Diamond DAG: block0 branches to block1 / block2, both jump to block3(return).
fn diamond() -> ArcFunction {
    func_from_terminators(vec![branch(1, 2), jump(3), jump(3), ret()])
}

/// Loop (cyclic): block0 -> block1(header, branch) -> block2(body, back-edge to
/// block1) / block3(return). Block1's entry INTERSECTs the block0 forward edge
/// and the block2 back-edge, so the ⊤-seeded back-edge forces a second round.
fn loop_cfg() -> ArcFunction {
    func_from_terminators(vec![jump(1), branch(2, 3), jump(1), ret()])
}

/// Inject `field` moved out of `var_a()` in block `block_idx`'s block-local set.
fn move_field(ctx: &mut BurdenLowerCtx<'_>, block_idx: usize, field: u32) {
    ctx.moved_out_fields_block_local[block_idx]
        .entry(var_a())
        .or_default()
        .insert(field);
}

// T1 — release-active negative pin. Run under `cargo test --release`.
#[test]
fn propagate_moved_out_fields_nonconvergence_fires_release_active() {
    // A genuinely multi-round CFG (the loop needs >=2 rounds: block0 moves A.0,
    // block2 moves A.1) with the cap forced to 1 leaves `changed == true` at the
    // cap. The release-active guard MUST fire (panic). In a release build a
    // release-stripped `debug_assert!` would let the non-converged state proceed
    // silently, over-approximating the moved-set and suppressing an owed release.
    let func = loop_cfg();
    let mut ctx = BurdenLowerCtx::new(&func);
    move_field(&mut ctx, 0, 0);
    move_field(&mut ctx, 2, 1);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        propagate_moved_out_fields(&mut ctx, &func, Some(1));
    }));
    assert!(
        result.is_err(),
        "release-active convergence guard must fire (panic) when the moved-out-fields \
         fixpoint has not converged at the iteration cap",
    );
}

// T3 — diagnostic quality: the guard names the cause + the cap (actionable).
#[test]
fn moved_out_fields_nonconvergence_diagnostic_actionable() {
    let func = loop_cfg();
    let mut ctx = BurdenLowerCtx::new(&func);
    move_field(&mut ctx, 0, 0);
    move_field(&mut ctx, 2, 1);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        propagate_moved_out_fields(&mut ctx, &func, Some(1));
    }));
    let Err(payload) = result else {
        panic!("expected the convergence guard to fire");
    };
    let msg = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("");
    assert!(
        msg.contains("failed to converge"),
        "guard diagnostic must name the cause (non-convergence); got: {msg}",
    );
    assert!(
        msg.contains("rounds") && msg.contains("IA-MF1"),
        "guard diagnostic must name the iteration cap + governing rule; got: {msg}",
    );
}

// T2 — positive convergence across {DAG, cyclic} × {single, multi}-field.
#[test]
fn propagate_moved_out_fields_normal_convergence_classifies() {
    let mut count = 0usize;

    // 1. DAG single-field.
    {
        let func = straight_line();
        let mut ctx = BurdenLowerCtx::new(&func);
        move_field(&mut ctx, 0, 0);
        propagate_moved_out_fields(&mut ctx, &func, None);
        assert!(
            ctx.moved_fields_convergence
                .is_some_and(|c| c.converged && c.rounds <= c.iteration_cap),
            "DAG single-field must converge within the derived cap",
        );
        // A.0 moved on the single path is definitely-moved at the exit block.
        assert_eq!(
            ctx.moved_out_fields_block_exit[1].get(&var_a()),
            Some(&fset(&[0]))
        );
        count += 1;
    }
    // 2. DAG multi-field.
    {
        let func = straight_line();
        let mut ctx = BurdenLowerCtx::new(&func);
        move_field(&mut ctx, 0, 0);
        move_field(&mut ctx, 1, 1);
        propagate_moved_out_fields(&mut ctx, &func, None);
        assert!(
            ctx.moved_fields_convergence.is_some_and(|c| c.converged),
            "DAG multi-field must converge",
        );
        assert_eq!(
            ctx.moved_out_fields_block_exit[1].get(&var_a()),
            Some(&fset(&[0, 1])),
            "both fields moved along the single path accumulate at the exit",
        );
        count += 1;
    }
    // 3. cyclic single-field. The moved field predates the loop header (moved
    // in block0, which dominates every other block), so it stays
    // definitely-moved across the back edge once the ⊤-seeded block2 exit is
    // replaced by its real (round-1) value — pins that the back edge does not
    // spuriously widen or narrow a field moved before the loop.
    {
        let func = loop_cfg();
        let mut ctx = BurdenLowerCtx::new(&func);
        move_field(&mut ctx, 0, 0);
        propagate_moved_out_fields(&mut ctx, &func, None);
        let convergence = ctx.moved_fields_convergence;
        assert!(
            convergence.is_some_and(|c| c.converged),
            "cyclic single-field must converge (no guard fire)",
        );
        assert!(
            convergence.is_some_and(|c| c.rounds >= 2),
            "loop_cfg's back edge (block2 -> block1) requires >=2 rounds to \
             stabilize the ⊤-seeded non-entry exits — a 1-round result would mean \
             the back edge never actually re-fed block1's entry; got {convergence:?}",
        );
        for block_idx in 0..4 {
            assert_eq!(
                ctx.moved_out_fields_block_exit[block_idx].get(&var_a()),
                Some(&fset(&[0])),
                "field 0 (moved in block0, which dominates every block) must stay \
                 definitely-moved at every block's exit, including across the \
                 block2 -> block1 back edge; block {block_idx}",
            );
        }
        count += 1;
    }
    // 4. cyclic multi-field. Field 0 moves before the loop header (block0);
    // field 1 moves only INSIDE the loop body (block2), on the path that
    // returns to the header via the back edge, never on the block0 -> block1
    // forward edge. INTERSECT semantics (RL-2 MUST-move) require field 1 to
    // stay OUT of the header's definitely-moved set even after the back edge
    // stabilizes — pins that a loop-body-only move does not leak past the
    // header via the back edge.
    {
        let func = loop_cfg();
        let mut ctx = BurdenLowerCtx::new(&func);
        move_field(&mut ctx, 0, 0);
        move_field(&mut ctx, 2, 1);
        propagate_moved_out_fields(&mut ctx, &func, None);
        let convergence = ctx.moved_fields_convergence;
        assert!(
            convergence.is_some_and(|c| c.converged),
            "cyclic multi-field must converge (no guard fire)",
        );
        assert!(
            convergence.is_some_and(|c| c.rounds >= 2),
            "loop_cfg's back edge (block2 -> block1) requires >=2 rounds to \
             stabilize the ⊤-seeded non-entry exits; got {convergence:?}",
        );
        assert_eq!(
            ctx.moved_out_fields_block_exit[2].get(&var_a()),
            Some(&fset(&[0, 1])),
            "block2 (the loop body, which locally moves field 1) accumulates \
             both fields at its own exit",
        );
        for block_idx in [0, 1, 3] {
            assert_eq!(
                ctx.moved_out_fields_block_exit[block_idx].get(&var_a()),
                Some(&fset(&[0])),
                "field 1 (moved only inside the loop body, block2) must NOT be \
                 definitely-moved at the header (block1), its forward predecessor \
                 (block0), or the loop exit (block3) — INTERSECT drops a field \
                 not moved on the block0 -> block1 forward edge; block {block_idx}",
            );
        }
        count += 1;
    }

    assert_eq!(
        count, 4,
        "matrix must visit all {{DAG, cyclic}} x {{single, multi}} cells"
    );
}

// T2b — INTERSECT (MUST-move) correctness: only fields moved on ALL predecessor
// paths are definitely-moved at a merge.
#[test]
fn propagate_moved_out_fields_intersect_drops_one_sided_move() {
    // Disjoint fields: block1 moves A.0, block2 moves A.1. Neither field is
    // moved on BOTH paths, so A carries NO definitely-moved field at the merge.
    let func = diamond();
    let mut ctx = BurdenLowerCtx::new(&func);
    move_field(&mut ctx, 1, 0);
    move_field(&mut ctx, 2, 1);
    propagate_moved_out_fields(&mut ctx, &func, None);
    assert!(ctx.moved_fields_convergence.is_some_and(|c| c.converged));
    let merge = &ctx.moved_out_fields_block_exit[3];
    assert!(
        merge.get(&var_a()).is_none_or(FxHashSet::is_empty),
        "a field moved on only ONE predecessor path must NOT be definitely-moved \
         at the merge (INTERSECT semantics, RL-2) — else an owed release is skipped",
    );

    // Same field on both paths: A.0 IS definitely-moved at the merge.
    let func = diamond();
    let mut ctx = BurdenLowerCtx::new(&func);
    move_field(&mut ctx, 1, 0);
    move_field(&mut ctx, 2, 0);
    propagate_moved_out_fields(&mut ctx, &func, None);
    assert_eq!(
        ctx.moved_out_fields_block_exit[3].get(&var_a()),
        Some(&fset(&[0])),
        "a field moved on ALL predecessor paths IS definitely-moved at the merge",
    );
}

// Derived-cap regression: the cap is n_blocks * universe_pair_count + 2 (rule
// IA-MF1) — a derived lattice-height bound, not a heuristic.
#[test]
fn derived_convergence_cap_matches_lattice_height_plus_margin() {
    assert_eq!(derived_convergence_cap(4, 2), 4 * 2 + 2);
    assert_eq!(derived_convergence_cap(0, 0), 2);
    assert_eq!(derived_convergence_cap(10, 5), 52);
    // saturating: no overflow panic on pathological inputs.
    assert_eq!(derived_convergence_cap(usize::MAX, usize::MAX), usize::MAX);
}
