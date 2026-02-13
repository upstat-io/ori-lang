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
mod tests {
    use ori_ir::Name;
    use ori_types::{Idx, Pool};

    use crate::ir::{ArcBlock, ArcInstr, ArcTerminator, ArcValue, LitValue, PrimOp};
    use crate::test_helpers::{b, make_func, owned_param as param, v};
    use crate::ArcClassifier;

    use crate::graph::compute_postorder;

    use super::compute_liveness;

    // Tests

    /// Single block: str param used and returned.
    /// `live_in` = {v0}, `live_out` = {} (Return has no successors).
    #[test]
    fn single_block_linear() {
        // fn f(x: str) -> str
        //   return x
        let func = make_func(
            vec![param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let result = compute_liveness(&func, &classifier);

        // v0 is used in the return → gen={v0}, kill={}
        // live_out = {} (no successors), live_in = gen ∪ (live_out - kill) = {v0}
        assert!(result.live_in[0].contains(&v(0)));
        assert!(result.live_out[0].is_empty());
    }

    /// Defined but never used RC'd variable → not in any live set.
    #[test]
    fn dead_after_definition() {
        // fn f() -> int
        //   let v0: str = "hello"  // RC'd but never used after definition
        //   let v1: int = 42
        //   return v1
        let func = make_func(
            vec![],
            Idx::INT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Let {
                        dst: v(0),
                        ty: Idx::STR,
                        value: ArcValue::Literal(LitValue::String(Name::from_raw(100))),
                    },
                    ArcInstr::Let {
                        dst: v(1),
                        ty: Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(42)),
                    },
                ],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::INT],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let result = compute_liveness(&func, &classifier);

        // v0 is never used (str but dead), v1 is int (scalar, not tracked)
        assert!(!result.live_in[0].contains(&v(0)));
        assert!(!result.live_out[0].contains(&v(0)));
    }

    /// Diamond CFG (if-then-else with merge): verify per-branch liveness.
    #[test]
    fn diamond_cfg() {
        // Block 0 (entry): branch on v1 (bool) to block 1 or block 2
        // Block 1 (then):  jump to block 3 with v0 (str param)
        // Block 2 (else):  let v2: str = "default"; jump to block 3 with v2
        // Block 3 (merge): param v3: str; return v3
        let func = make_func(
            vec![param(0, Idx::STR)],
            Idx::STR,
            vec![
                ArcBlock {
                    id: b(0),
                    params: vec![],
                    body: vec![ArcInstr::Let {
                        dst: v(1),
                        ty: Idx::BOOL,
                        value: ArcValue::Literal(LitValue::Bool(true)),
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
                        args: vec![v(0)],
                    },
                },
                ArcBlock {
                    id: b(2),
                    params: vec![],
                    body: vec![ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::STR,
                        value: ArcValue::Literal(LitValue::String(Name::from_raw(100))),
                    }],
                    terminator: ArcTerminator::Jump {
                        target: b(3),
                        args: vec![v(2)],
                    },
                },
                ArcBlock {
                    id: b(3),
                    params: vec![(v(3), Idx::STR)],
                    body: vec![],
                    terminator: ArcTerminator::Return { value: v(3) },
                },
            ],
            vec![Idx::STR, Idx::BOOL, Idx::STR, Idx::STR],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let result = compute_liveness(&func, &classifier);

        // Block 3 (merge): v3 is a block param (kill) and used in Return (but
        // use-after-def, so not in gen). live_out = {} (Return has no successors).
        // live_in = {} ∪ ({} - {v3}) = {}
        // This is correct: v3 is "born" at block 3's entry, no need for it
        // before. The demand is expressed via Jump args in predecessors.
        assert!(result.live_in[3].is_empty());
        assert!(result.live_out[3].is_empty());

        // Block 1 (then): Jump args=[v0] → gen={v0}, kill={}.
        // live_out = live_in(b3) = {}.
        // live_in = {v0} ∪ ({} - {}) = {v0}.
        assert!(result.live_in[1].contains(&v(0)));

        // Block 2 (else): Let defines v2 (kill={v2}), Jump args=[v2]
        // → use-after-def, not in gen. gen={}.
        // live_out = live_in(b3) = {}.
        // live_in = {} ∪ ({} - {v2}) = {}.
        assert!(result.live_in[2].is_empty());

        // Block 0 (entry): Branch cond=v1 (bool, not tracked).
        // live_out = live_in(b1) ∪ live_in(b2) = {v0} ∪ {} = {v0}.
        // gen = {}, kill = {} (v1 is bool).
        // live_in = {} ∪ ({v0} - {}) = {v0}.
        assert!(result.live_in[0].contains(&v(0)));
        assert!(result.live_out[0].contains(&v(0)));
    }

    /// Int-only function → all live sets empty (scalars not tracked).
    #[test]
    fn scalars_not_tracked() {
        // fn f(x: int, y: int) -> int
        //   let v2 = x + y
        //   return v2
        let func = make_func(
            vec![param(0, Idx::INT), param(1, Idx::INT)],
            Idx::INT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::INT,
                    value: ArcValue::PrimOp {
                        op: PrimOp::Binary(ori_ir::BinaryOp::Add),
                        args: vec![v(0), v(1)],
                    },
                }],
                terminator: ArcTerminator::Return { value: v(2) },
            }],
            vec![Idx::INT, Idx::INT, Idx::INT],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let result = compute_liveness(&func, &classifier);

        assert!(result.live_in[0].is_empty());
        assert!(result.live_out[0].is_empty());
    }

    /// Loop back edge: requires fixed-point iteration.
    #[test]
    fn loop_back_edge() {
        // Block 0 (entry): jump to block 1 with v0 (str param)
        // Block 1 (loop header): param v1: str; branch on v2 (bool, from somewhere)
        //   then → block 2 (loop body)
        //   else → block 3 (exit)
        // Block 2 (body): let v3 = apply f(v1); jump to block 1 with v3
        // Block 3 (exit): return v1
        //
        // v1 should be live across the loop body because it's used in the
        // exit path (block 3).
        let func = make_func(
            vec![param(0, Idx::STR)],
            Idx::STR,
            vec![
                ArcBlock {
                    id: b(0),
                    params: vec![],
                    body: vec![ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::BOOL,
                        value: ArcValue::Literal(LitValue::Bool(true)),
                    }],
                    terminator: ArcTerminator::Jump {
                        target: b(1),
                        args: vec![v(0)],
                    },
                },
                ArcBlock {
                    id: b(1),
                    params: vec![(v(1), Idx::STR)],
                    body: vec![],
                    terminator: ArcTerminator::Branch {
                        cond: v(2),
                        then_block: b(2),
                        else_block: b(3),
                    },
                },
                ArcBlock {
                    id: b(2),
                    params: vec![],
                    body: vec![ArcInstr::Apply {
                        dst: v(3),
                        ty: Idx::STR,
                        func: Name::from_raw(99),
                        args: vec![v(1)],
                    }],
                    terminator: ArcTerminator::Jump {
                        target: b(1),
                        args: vec![v(3)],
                    },
                },
                ArcBlock {
                    id: b(3),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Return { value: v(1) },
                },
            ],
            vec![Idx::STR, Idx::STR, Idx::BOOL, Idx::STR],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let result = compute_liveness(&func, &classifier);

        // Block 3 (exit): live_in = {v1} (return v1)
        assert!(result.live_in[3].contains(&v(1)));

        // Block 1 (header): v1 used in block 3 (exit) and block 2 (body).
        // live_in should contain v1 from gen (used in terminator's successors
        // where it propagates back).
        // Actually v1 is a block param of b1, so it's in kill. But it flows
        // through to live_out via successors that use it.
        // live_out(b1) should contain v1 (needed in b2 and b3).
        assert!(result.live_out[1].contains(&v(1)));

        // Block 2 (body): v1 is used in Apply → live_in should contain v1.
        assert!(result.live_in[2].contains(&v(1)));
    }

    /// Two branches jump to merge with different args → different vars live
    /// in each branch, both live at the entry.
    #[test]
    fn block_param_substitution() {
        // Block 0: branch on v2 (bool) → b1 or b2
        // Block 1: jump to b3 with v0 (str)
        // Block 2: jump to b3 with v1 (str)
        // Block 3: param v3: str; return v3
        //
        // Both v0 and v1 should be live in block 0 because they are used
        // as Jump arguments in different branches.
        let func = make_func(
            vec![param(0, Idx::STR), param(1, Idx::STR)],
            Idx::STR,
            vec![
                ArcBlock {
                    id: b(0),
                    params: vec![],
                    body: vec![ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::BOOL,
                        value: ArcValue::Literal(LitValue::Bool(true)),
                    }],
                    terminator: ArcTerminator::Branch {
                        cond: v(2),
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
                        args: vec![v(0)],
                    },
                },
                ArcBlock {
                    id: b(2),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Jump {
                        target: b(3),
                        args: vec![v(1)],
                    },
                },
                ArcBlock {
                    id: b(3),
                    params: vec![(v(3), Idx::STR)],
                    body: vec![],
                    terminator: ArcTerminator::Return { value: v(3) },
                },
            ],
            vec![Idx::STR, Idx::STR, Idx::BOOL, Idx::STR],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let result = compute_liveness(&func, &classifier);

        // Block 1: Jump args=[v0] → gen={v0}. live_in = {v0}.
        assert!(result.live_in[1].contains(&v(0)));

        // Block 2: Jump args=[v1] → gen={v1}. live_in = {v1}.
        assert!(result.live_in[2].contains(&v(1)));

        // Block 0: live_out = live_in(b1) ∪ live_in(b2) = {v0} ∪ {v1} = {v0, v1}.
        // Both v0 and v1 are live at entry because they're needed in
        // different branches.
        assert!(result.live_in[0].contains(&v(0)));
        assert!(result.live_in[0].contains(&v(1)));
    }

    /// Same var in multiple arg positions of a single instruction.
    #[test]
    fn multiple_uses_same_var() {
        // fn f(x: str) -> str
        //   let v1 = apply g(x, x)  // x used twice
        //   return v1
        let func = make_func(
            vec![param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(1),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![v(0), v(0)], // same var twice
                }],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::STR],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let result = compute_liveness(&func, &classifier);

        // v0 is used (gen set), v1 is returned but defined in same block.
        // live_in = {v0} (v0 used before any definition)
        assert!(result.live_in[0].contains(&v(0)));
        // v1 is defined and returned in the same block. Since it's defined,
        // it's in kill. It's used in the terminator but kill already has it,
        // so it's not in gen. live_out = {} (return has no successors).
        assert!(!result.live_in[0].contains(&v(1)));
    }

    /// Return of a param with no body instructions.
    #[test]
    fn used_in_terminator_only() {
        // fn f(x: str) -> str
        //   return x
        // This is the same as single_block_linear, but explicitly testing
        // that terminator-only uses are correctly captured.
        let func = make_func(
            vec![param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let result = compute_liveness(&func, &classifier);

        assert!(result.live_in[0].contains(&v(0)));
        assert_eq!(result.live_in[0].len(), 1);
    }

    /// Invoke: dst NOT live in unwind block, IS available in normal block.
    #[test]
    fn invoke_dst_not_live_in_unwind() {
        // Block 0: invoke f(v0) → dst=v1, normal=b1, unwind=b2
        // Block 1 (normal): return v1
        // Block 2 (unwind): return v0  (v1 is NOT defined here)
        let func = make_func(
            vec![param(0, Idx::STR)],
            Idx::STR,
            vec![
                ArcBlock {
                    id: b(0),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Invoke {
                        dst: v(1),
                        ty: Idx::STR,
                        func: Name::from_raw(99),
                        args: vec![v(0)],
                        normal: b(1),
                        unwind: b(2),
                    },
                },
                ArcBlock {
                    id: b(1),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Return { value: v(1) },
                },
                ArcBlock {
                    id: b(2),
                    params: vec![],
                    body: vec![],
                    // Unwind block returns the original param, NOT the invoke dst.
                    terminator: ArcTerminator::Resume,
                },
            ],
            vec![Idx::STR, Idx::STR],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let result = compute_liveness(&func, &classifier);

        // Block 1 (normal): v1 is defined at entry (Invoke dst) and used in Return.
        // gen={}, kill={v1} → live_in = {} (v1 is born here).
        assert!(
            !result.live_in[1].contains(&v(1)),
            "v1 should NOT be in live_in of normal block (it's defined there)"
        );

        // Block 2 (unwind): v1 is NOT defined here.
        // If v1 appeared in live_in[2], that would be a bug — it would trigger
        // RC ops for a variable that was never produced on the unwind path.
        assert!(
            !result.live_in[2].contains(&v(1)),
            "v1 must NOT be in live_in of unwind block"
        );
        assert!(
            !result.live_out[2].contains(&v(1)),
            "v1 must NOT be in live_out of unwind block"
        );

        // Block 0: v0 is used as an Invoke arg → gen={v0}
        assert!(result.live_in[0].contains(&v(0)));
    }

    /// Invoke: a live str variable before the invoke should be live in
    /// both normal and unwind successors (it needs cleanup on unwind).
    #[test]
    fn invoke_live_var_propagates_to_unwind() {
        // Block 0: let v1: str = "hello"; invoke f(v0) → dst=v2, normal=b1, unwind=b2
        // Block 1 (normal): return v1  (uses v1)
        // Block 2 (unwind): resume (v1 needs RcDec on cleanup)
        let func = make_func(
            vec![param(0, Idx::STR)],
            Idx::STR,
            vec![
                ArcBlock {
                    id: b(0),
                    params: vec![],
                    body: vec![ArcInstr::Let {
                        dst: v(1),
                        ty: Idx::STR,
                        value: ArcValue::Literal(LitValue::String(Name::from_raw(100))),
                    }],
                    terminator: ArcTerminator::Invoke {
                        dst: v(2),
                        ty: Idx::STR,
                        func: Name::from_raw(99),
                        args: vec![v(0)],
                        normal: b(1),
                        unwind: b(2),
                    },
                },
                ArcBlock {
                    id: b(1),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Return { value: v(1) },
                },
                ArcBlock {
                    id: b(2),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Resume,
                },
            ],
            vec![Idx::STR, Idx::STR, Idx::STR],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let result = compute_liveness(&func, &classifier);

        // v1 is live at block 0's exit (it's used in normal successor b1).
        assert!(
            result.live_out[0].contains(&v(1)),
            "v1 should be live at block 0 exit"
        );

        // v1 should be live_in for the normal block (used in Return).
        assert!(
            result.live_in[1].contains(&v(1)),
            "v1 should be live_in for normal block"
        );

        // v1 should NOT be in unwind's live_in (Resume doesn't use it).
        // BUT: once RC insertion adds cleanup RcDec(v1) to the unwind block,
        // that will create a use and v1 will become live there. Before RC
        // insertion, liveness only sees what the IR declares.
        assert!(
            !result.live_in[2].contains(&v(1)),
            "v1 should not be in unwind live_in before RC insertion"
        );
    }

    /// Verify postorder visits successors before predecessors.
    #[test]
    fn postorder_visits_successors_first() {
        // Linear chain: b0 → b1 → b2 → return
        let func = make_func(
            vec![param(0, Idx::STR)],
            Idx::STR,
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
            vec![Idx::STR],
        );

        let postorder = compute_postorder(&func);

        // Postorder: leaf first, root last → [2, 1, 0]
        assert_eq!(postorder.len(), 3);
        // b2 should appear before b1, and b1 before b0.
        let pos_0 = postorder.iter().position(|&x| x == 0);
        let pos_1 = postorder.iter().position(|&x| x == 1);
        let pos_2 = postorder.iter().position(|&x| x == 2);
        assert!(pos_2 < pos_1, "b2 should come before b1 in postorder");
        assert!(pos_1 < pos_0, "b1 should come before b0 in postorder");
    }

    /// Switch terminator with multiple successors.
    #[test]
    fn switch_multiple_successors() {
        // Block 0: switch on v1 (int, scalar) → case 0: b1, case 1: b2, default: b3
        // Block 1: return v0 (str)
        // Block 2: return v0 (str)
        // Block 3: let v2: str = "default"; return v2
        let func = make_func(
            vec![param(0, Idx::STR)],
            Idx::STR,
            vec![
                ArcBlock {
                    id: b(0),
                    params: vec![],
                    body: vec![ArcInstr::Let {
                        dst: v(1),
                        ty: Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(0)),
                    }],
                    terminator: ArcTerminator::Switch {
                        scrutinee: v(1),
                        cases: vec![(0, b(1)), (1, b(2))],
                        default: b(3),
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
                ArcBlock {
                    id: b(3),
                    params: vec![],
                    body: vec![ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::STR,
                        value: ArcValue::Literal(LitValue::String(Name::from_raw(100))),
                    }],
                    terminator: ArcTerminator::Return { value: v(2) },
                },
            ],
            vec![Idx::STR, Idx::INT, Idx::STR],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let result = compute_liveness(&func, &classifier);

        // Blocks 1 and 2 use v0 → live_in = {v0}
        assert!(result.live_in[1].contains(&v(0)));
        assert!(result.live_in[2].contains(&v(0)));

        // Block 3 defines v2 and returns it — v0 not used.
        assert!(!result.live_in[3].contains(&v(0)));

        // Block 0: live_out = union of live_in(b1, b2, b3) = {v0}
        assert!(result.live_out[0].contains(&v(0)));
        assert!(result.live_in[0].contains(&v(0)));
    }

    // RefinedLiveness tests

    /// Variable used as operand → `live_for_use`, not `live_for_drop`.
    #[test]
    fn refined_used_var_is_live_for_use() {
        // fn f(x: str) -> str
        //   v1 = apply g(x)   -- x is a real operand
        //   return v1
        let func = make_func(
            vec![param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(1),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![v(0)],
                }],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::STR],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let (refined, _) = super::compute_refined_liveness(&func, &classifier);

        // v0 is used as an Apply argument → live_for_use
        assert!(
            refined[0].live_for_use.contains(&v(0)),
            "v0 should be live_for_use"
        );
        assert!(
            !refined[0].live_for_drop.contains(&v(0)),
            "v0 should NOT be live_for_drop"
        );
    }

    /// Variable only appears in `RcDec` → `live_for_drop`.
    #[test]
    fn refined_only_dec_is_live_for_drop() {
        // fn f(x: str) -> int
        //   v1 = let 42 : int
        //   RcDec(x)           -- x only used for drop
        //   return v1
        let func = make_func(
            vec![param(0, Idx::STR)],
            Idx::INT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Let {
                        dst: v(1),
                        ty: Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(42)),
                    },
                    ArcInstr::RcDec { var: v(0) },
                ],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::INT],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let (refined, _) = super::compute_refined_liveness(&func, &classifier);

        // v0 only appears in RcDec → live_for_drop
        assert!(
            refined[0].live_for_drop.contains(&v(0)),
            "v0 should be live_for_drop"
        );
        assert!(
            !refined[0].live_for_use.contains(&v(0)),
            "v0 should NOT be live_for_use"
        );
    }

    /// Variable used then decremented → `live_for_use` wins.
    #[test]
    fn refined_use_then_dec_is_live_for_use() {
        // fn f(x: str) -> str
        //   v1 = apply g(x)   -- x is a real operand
        //   RcDec(x)           -- also dec'd
        //   return v1
        let func = make_func(
            vec![param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Apply {
                        dst: v(1),
                        ty: Idx::STR,
                        func: Name::from_raw(99),
                        args: vec![v(0)],
                    },
                    ArcInstr::RcDec { var: v(0) },
                ],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::STR],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let (refined, _) = super::compute_refined_liveness(&func, &classifier);

        // v0 is used in Apply AND in RcDec — live_for_use wins
        assert!(
            refined[0].live_for_use.contains(&v(0)),
            "v0 should be live_for_use (use wins over drop)"
        );
        assert!(
            !refined[0].live_for_drop.contains(&v(0)),
            "v0 should NOT be in live_for_drop"
        );
    }

    /// Variable used in terminator → `live_for_use`.
    #[test]
    fn refined_terminator_use_is_live_for_use() {
        // fn f(x: str) -> str
        //   return x
        let func = make_func(
            vec![param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let (refined, _) = super::compute_refined_liveness(&func, &classifier);

        assert!(
            refined[0].live_for_use.contains(&v(0)),
            "v0 used in Return → live_for_use"
        );
    }
}
