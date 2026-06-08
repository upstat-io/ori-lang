//! Backward-liveness dataflow feeding `emit_burden_ops`: per-block live-out
//! sets restricted to `owned_vars_needing_rc`, plus the per-block gen/kill
//! inputs the fixpoint consumes.

use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcVarId};

/// Compute per-block live-out sets restricted to `owned_vars_needing_rc`.
///
/// Standard backward liveness (`live_out(B) = ∪ live_in(S)` over successors `S`;
/// `live_in(B) = gen(B) ∪ (live_out(B) − kill(B))`), filtered to vars that carry
/// an owned-heap burden. Mirrors `crate::liveness::compute_liveness`'s gen/kill
/// shape (an `Invoke` `dst` is a definition at its `normal` successor's entry,
/// like a block param) but is keyed on the burden walk's own
/// `owned_vars_needing_rc` set rather than the `ArcClassification` `needs_rc`
/// predicate — no parallel ownership tracker (AIMS Invariant 5): the set is the
/// burden walk's existing owned-RC classification.
///
/// Consumed by `emit_last_use_decs` + `emit_terminator_burden_decs` to suppress
/// the in-block last-use `BurdenDec` for a var live-out of the block per
/// `Spec: Annex E §AIMS RL-4` (the dec belongs on the dying CFG edge / at the
/// dead-out block, not unconditionally in a block the value outlives).
pub(in crate::lower::burden_lower) fn compute_live_out_owned(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> Vec<FxHashSet<ArcVarId>> {
    let n = func.blocks.len();
    let (gen, kill) = compute_owned_gen_kill(func, owned_vars_needing_rc);

    // Fixed-point backward dataflow: `live_out(B) = ∪ live_in(S)`,
    // `live_in(B) = gen(B) ∪ (live_out(B) − kill(B))`.
    let mut live_in: Vec<FxHashSet<ArcVarId>> = vec![FxHashSet::default(); n];
    let mut live_out: Vec<FxHashSet<ArcVarId>> = vec![FxHashSet::default(); n];
    let mut changed = true;
    while changed {
        changed = false;
        for b in (0..n).rev() {
            let mut new_out: FxHashSet<ArcVarId> = FxHashSet::default();
            for succ in crate::graph::successor_block_ids(&func.blocks[b].terminator) {
                let si = succ.index();
                if si < n {
                    new_out.extend(live_in[si].iter().copied());
                }
            }
            let mut new_in = gen[b].clone();
            for &var in &new_out {
                if !kill[b].contains(&var) {
                    new_in.insert(var);
                }
            }
            if new_in != live_in[b] || new_out != live_out[b] {
                changed = true;
                live_in[b] = new_in;
                live_out[b] = new_out;
            }
        }
    }
    live_out
}

/// Per-block `(gen, kill)` sets for `compute_live_out_owned`, restricted to
/// `owned_vars_needing_rc`. `gen` = vars used before any definition in the
/// block; `kill` = vars defined in the block (incl. block params + the
/// `Invoke` `dst` bound at the normal-successor entry).
fn compute_owned_gen_kill(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> (Vec<FxHashSet<ArcVarId>>, Vec<FxHashSet<ArcVarId>>) {
    let n = func.blocks.len();
    let invoke_defs = crate::graph::collect_invoke_defs(func);
    let mut gen: Vec<FxHashSet<ArcVarId>> = vec![FxHashSet::default(); n];
    let mut kill: Vec<FxHashSet<ArcVarId>> = vec![FxHashSet::default(); n];
    for (b, block) in func.blocks.iter().enumerate() {
        let g = &mut gen[b];
        let k = &mut kill[b];
        for &(param_var, _) in &block.params {
            if owned_vars_needing_rc.contains(&param_var) {
                k.insert(param_var);
            }
        }
        if let Some(dsts) = invoke_defs.get(&block.id) {
            for &dst in dsts {
                if owned_vars_needing_rc.contains(&dst) {
                    k.insert(dst);
                }
            }
        }
        for instr in &block.body {
            for var in instr.used_vars() {
                if owned_vars_needing_rc.contains(&var) && !k.contains(&var) {
                    g.insert(var);
                }
            }
            if let Some(dst) = instr.defined_var() {
                if owned_vars_needing_rc.contains(&dst) {
                    k.insert(dst);
                }
            }
        }
        for var in block.terminator.used_vars() {
            if owned_vars_needing_rc.contains(&var) && !k.contains(&var) {
                g.insert(var);
            }
        }
    }
    (gen, kill)
}
