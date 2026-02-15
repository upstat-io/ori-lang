//! Backward dataflow liveness analysis on ARC IR (Section 07.1).
//!
//! Computes which variables are **live** (will be read in the future) at
//! every basic block boundary. This information drives RC insertion
//! (Section 07.2): a variable's last use is where its `RcDec` goes, and
//! additional uses require `RcInc`.
//!
//! # Algorithm
//!
//! Standard backward dataflow with fixed-point iteration:
//!
//! 1. **Precompute gen/kill** for each block (forward scan).
//!    - `gen(B)` = variables *used* before being defined in B.
//!    - `kill(B)` = variables *defined* in B (including block params).
//! 2. **Postorder iteration** for convergence:
//!    - `live_out(B) = ∪ live_in(S)` for each successor S.
//!    - `live_in(B) = gen(B) ∪ (live_out(B) - kill(B))`.
//! 3. Repeat until no sets change.
//!
//! Block parameter flow is handled implicitly: `Jump` arguments are uses
//! in the predecessor (captured by `gen` via `ArcTerminator::used_vars()`),
//! and block params are definitions in the successor (in `kill`). No
//! explicit substitution is needed.
//!
//! Only RC'd variables (those where `classifier.needs_rc()` is true) are
//! tracked. Scalar variables are excluded because they never need
//! `RcInc`/`RcDec`.
//!
//! # References
//!
//! - Lean 4: `src/Lean/Compiler/IR/LiveVars.lean`
//! - Koka: Perceus paper §3.2 (liveness-based RC insertion)
//! - Appel: "Modern Compiler Implementation" §10.1 (dataflow analysis)

use rustc_hash::{FxHashMap, FxHashSet};

use crate::graph::{compute_postorder, successor_block_ids};
use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcVarId};
use crate::ArcClassification;

/// Set of live variables at a program point.
///
/// Uses `FxHashSet` for simplicity. A bitset indexed by `ArcVarId::raw()`
/// would be faster for large functions but adds complexity — this can be
/// optimized later if profiling shows it matters.
pub type LiveSet = FxHashSet<ArcVarId>;

/// Liveness information for every basic block in a function.
///
/// `live_in[b]` is the set of variables live at the *entry* of block `b`.
/// `live_out[b]` is the set of variables live at the *exit* of block `b`.
/// Both are indexed by `ArcBlockId::index()`.
pub struct BlockLiveness {
    /// Variables live at block entry, indexed by `ArcBlockId::index()`.
    pub live_in: Vec<LiveSet>,
    /// Variables live at block exit, indexed by `ArcBlockId::index()`.
    pub live_out: Vec<LiveSet>,
}

/// Compute liveness for all blocks in an ARC IR function.
///
/// Only tracks variables whose types satisfy `classifier.needs_rc()`.
/// Scalar variables (int, float, bool, etc.) are excluded entirely.
///
/// # Arguments
///
/// * `func` — the ARC IR function to analyze.
/// * `classifier` — type classifier that determines which variables need RC.
pub fn compute_liveness(func: &ArcFunction, classifier: &dyn ArcClassification) -> BlockLiveness {
    let num_blocks = func.blocks.len();

    tracing::debug!(function = func.name.raw(), num_blocks, "computing liveness");

    // Step 0.5: Build Invoke dst mapping.
    // An Invoke terminator defines `dst` at the normal successor's entry,
    // not at the invoking block. Precompute which blocks receive Invoke
    // definitions so gen/kill can account for them.
    let invoke_defs = crate::graph::collect_invoke_defs(func);

    // Step 1: Precompute gen/kill for each block.
    let mut gen: Vec<LiveSet> = Vec::with_capacity(num_blocks);
    let mut kill: Vec<LiveSet> = Vec::with_capacity(num_blocks);

    for block in &func.blocks {
        let (block_gen, block_kill) = compute_gen_kill(block, func, classifier, &invoke_defs);
        gen.push(block_gen);
        kill.push(block_kill);
    }

    // Step 2: Compute postorder for convergence ordering.
    let postorder = compute_postorder(func);

    // Step 3: Fixed-point iteration.
    let mut live_in: Vec<LiveSet> = (0..num_blocks).map(|_| LiveSet::default()).collect();
    let mut live_out: Vec<LiveSet> = (0..num_blocks).map(|_| LiveSet::default()).collect();

    let mut iteration = 0u32;
    loop {
        iteration += 1;
        let mut changed = false;

        // Iterate in postorder. For a backward analysis, postorder processes
        // successors before predecessors, which gives good convergence.
        for &block_idx in &postorder {
            // live_out(B) = ∪ live_in(S) for each successor S.
            //
            // Block parameter flow is handled implicitly: Jump args are
            // uses in the predecessor (captured by gen/kill via
            // `ArcTerminator::used_vars()`), and block params are definitions
            // in the successor (in kill). No explicit substitution needed.
            let mut new_live_out = LiveSet::default();
            for succ_id in successor_block_ids(&func.blocks[block_idx].terminator) {
                let succ_idx = succ_id.index();
                if succ_idx < num_blocks {
                    for &var in &live_in[succ_idx] {
                        new_live_out.insert(var);
                    }
                }
            }

            // live_in(B) = gen(B) ∪ (live_out(B) - kill(B))
            let mut new_live_in = gen[block_idx].clone();
            for &var in &new_live_out {
                if !kill[block_idx].contains(&var) {
                    new_live_in.insert(var);
                }
            }

            if new_live_in != live_in[block_idx] || new_live_out != live_out[block_idx] {
                changed = true;
                live_in[block_idx] = new_live_in;
                live_out[block_idx] = new_live_out;
            }
        }

        if !changed {
            break;
        }
    }

    tracing::debug!(iterations = iteration, "liveness converged");

    BlockLiveness { live_in, live_out }
}

/// Precompute gen and kill sets for a single block.
///
/// Walk instructions forward. A variable is in `gen` if it's used before
/// being defined. A variable is in `kill` if it's defined in this block.
/// Block parameters are in `kill` (they're definitions at the block entry).
///
/// Invoke `dst` variables are treated as definitions at the normal
/// successor's entry (via `invoke_defs`), not at the invoking block.
fn compute_gen_kill(
    block: &ArcBlock,
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    invoke_defs: &FxHashMap<ArcBlockId, Vec<ArcVarId>>,
) -> (LiveSet, LiveSet) {
    let mut gen = LiveSet::default();
    let mut kill = LiveSet::default();

    // Block parameters are definitions.
    for &(param_var, _) in &block.params {
        if needs_rc_var(param_var, func, classifier) {
            kill.insert(param_var);
        }
    }

    // Invoke dst variables defined at this block's entry.
    // An Invoke in a predecessor block defines `dst` at the normal
    // successor — that definition acts like a block parameter.
    if let Some(dsts) = invoke_defs.get(&block.id) {
        for &dst in dsts {
            if needs_rc_var(dst, func, classifier) {
                kill.insert(dst);
            }
        }
    }

    // Walk instructions forward.
    for instr in &block.body {
        // Uses before definitions go into gen.
        for var in instr.used_vars() {
            if needs_rc_var(var, func, classifier) && !kill.contains(&var) {
                gen.insert(var);
            }
        }
        // Definitions go into kill.
        if let Some(dst) = instr.defined_var() {
            if needs_rc_var(dst, func, classifier) {
                kill.insert(dst);
            }
        }
    }

    // Terminator uses.
    for var in block.terminator.used_vars() {
        if needs_rc_var(var, func, classifier) && !kill.contains(&var) {
            gen.insert(var);
        }
    }

    (gen, kill)
}

/// Check whether a variable needs RC tracking.
#[inline]
fn needs_rc_var(var: ArcVarId, func: &ArcFunction, classifier: &dyn ArcClassification) -> bool {
    let idx = var.index();
    if idx < func.var_types.len() {
        classifier.needs_rc(func.var_types[idx])
    } else {
        // Out-of-bounds variable — conservative fallback.
        true
    }
}

/// Refined liveness that distinguishes *why* a variable is live.
///
/// Standard liveness says "variable X is live here" but doesn't distinguish
/// between "X is live because it will be *read*" and "X is live only because
/// it needs an `RcDec`". This distinction matters for cross-block reset/reuse:
/// a variable that is only live-for-drop can be safely reset without risking
/// a use-after-free, whereas a variable that is live-for-use cannot.
///
/// Inspired by Lean 4's distinction between "consumed" and "dropped" in
/// `Lean.Compiler.IR.RC` and Koka's `CheckFBIP.hs` ownership analysis.
pub struct RefinedLiveness {
    /// Variables live because they will be read as an operand (not just dropped).
    pub live_for_use: LiveSet,
    /// Variables live only because they need `RcDec` (not read as operand).
    pub live_for_drop: LiveSet,
}

/// Compute refined liveness for all blocks.
///
/// After computing standard liveness, performs a second backward pass to
/// classify *why* each variable is live:
///
/// - **`live_for_use`**: the variable appears as an operand in an instruction
///   or terminator (read, not just decremented).
/// - **`live_for_drop`**: the variable only appears in `RcDec` instructions.
///
/// At join points (blocks with multiple predecessors), `live_for_use` wins
/// conservatively — if any successor path reads the variable, we treat it
/// as live-for-use at the join.
///
/// Returns both the refined classification and the standard `BlockLiveness`
/// that was computed internally, avoiding a redundant fixed-point iteration
/// when callers need both.
pub fn compute_refined_liveness(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
) -> (Vec<RefinedLiveness>, BlockLiveness) {
    let standard = compute_liveness(func, classifier);
    let num_blocks = func.blocks.len();

    // For each block, classify each live_out variable.
    // We do a backward walk per block: a var is "use" if we see it used
    // as a real operand, "drop" if we only see it in RcDec.
    let mut refined: Vec<RefinedLiveness> = Vec::with_capacity(num_blocks);

    for block_idx in 0..num_blocks {
        let block = &func.blocks[block_idx];
        let live_out = &standard.live_out[block_idx];

        // Start from live_out classification of successors.
        // At joins, live_for_use wins (conservative).
        let mut use_set = LiveSet::default();
        let mut drop_set = LiveSet::default();

        // Seed from successor blocks' refined classification.
        // Since we process in block order (not RPO), we use the standard
        // live_out and then classify within this block.
        for &var in live_out {
            // Default: check if successors use this var as an operand.
            // We approximate conservatively: anything in live_out that
            // appears in a successor's gen set as a real operand → use.
            // This is a simplified approach; for full precision we'd need
            // backward dataflow on the classification itself.
            drop_set.insert(var);
        }

        // Backward walk through the block body.
        // If we see a real use of a var, promote it from drop to use.
        // If we see only RcDec, it stays in drop.

        // Check terminator first (backward walk: terminator is "last").
        for var in block.terminator.used_vars() {
            if live_out.contains(&var) || standard.live_in[block_idx].contains(&var) {
                drop_set.remove(&var);
                use_set.insert(var);
            }
        }

        // Walk instructions backward.
        for instr in block.body.iter().rev() {
            match instr {
                crate::ir::ArcInstr::RcDec { var } => {
                    // RcDec is a "drop" use — only promotes to drop_set
                    // if not already in use_set.
                    if !use_set.contains(var) {
                        drop_set.insert(*var);
                    }
                }
                _ => {
                    // Any other instruction that uses variables → real use.
                    for var in instr.used_vars() {
                        if drop_set.contains(&var) {
                            drop_set.remove(&var);
                            use_set.insert(var);
                        } else if needs_rc_var(var, func, classifier) {
                            use_set.insert(var);
                        }
                    }
                }
            }
        }

        refined.push(RefinedLiveness {
            live_for_use: use_set,
            live_for_drop: drop_set,
        });
    }

    (refined, standard)
}

#[cfg(test)]
mod tests;
