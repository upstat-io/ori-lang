//! RC emission from converged AIMS state map.
//!
//! Reads the [`AimsStateMap`] produced by intraprocedural analysis and emits
//! minimal `RcInc`/`RcDec` operations into the `ArcFunction`. Replaces
//! `rc_insert`, `rc_identity`, and `rc_elim` from the old pipeline.
//!
//! # Algorithm
//!
//! Forward walk per block. For each owned, non-scalar variable:
//! - **`RcInc`** before each use where a future use (or exit continuation) exists
//! - **`RcDec`** after the last use if the variable is dead at block exit
//! - **`RcDec`** at block entry for variables live at entry but unused and dead at exit
//!
//! Edge cleanup handles variables that die on specific edges (live in predecessor
//! but dead in a particular successor).
//!
//! # References
//!
//! - Perceus (Reinking et al., PLDI 2021): dup/drop = contraction/weakening
//! - Lean 4 `RC.lean`: backward liveness-driven insertion with last-use opt

pub mod arg_ownership;
pub mod cow;
pub mod drop_hints;
mod edge_cleanup;
#[cfg(test)]
mod tests;

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;
use ori_types::Pool;

use crate::aims::contract::MemoryContract;
use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::{AccessClass, Cardinality, Locality};
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, RcStrategy, ValueRepr,
};
use crate::ArcClassification;

/// Edge-specific RC decrement: variable + strategy.
type EdgeDec = (ArcVarId, RcStrategy);

/// Shared context for per-block RC emission helpers.
struct BlockCtx<'a> {
    func: &'a ArcFunction,
    blk: ArcBlockId,
    state_map: &'a AimsStateMap,
    defined_in_block: &'a FxHashSet<ArcVarId>,
    /// Variables defined by `Project` (borrowed — no independent RC management).
    borrowed_defs: &'a FxHashSet<ArcVarId>,
    use_info: &'a FxHashMap<ArcVarId, (usize, LastUse)>,
    pool: &'a Pool,
}

// RC emission result

/// Result of RC emission, including auxiliary hints.
pub struct EmitRcResult {
    /// Variables identified as candidates for local allocation (v1: hints only).
    pub local_alloc_candidates: Vec<LocalAllocCandidate>,
}

/// A variable identified as a local-allocation candidate.
pub struct LocalAllocCandidate {
    pub block: ArcBlockId,
    pub instr: usize,
    pub var: ArcVarId,
}

// Entry point

/// Emit RC operations into the function based on converged AIMS analysis.
///
/// Walks each block forward, inserting `RcInc` before each non-last use of
/// owned variables, and `RcDec` after the last use (or at block entry for
/// unused dead variables). Edge cleanup inserts `RcDec` on edges where a
/// variable is live in the predecessor but dead in the successor.
///
/// # Panics
///
/// Debug-panics if `func.var_reprs` is empty (must be populated before
/// RC emission — pipeline step 3: `compute_var_reprs`).
#[expect(clippy::implicit_hasher, reason = "FxHashMap is the canonical hasher")]
pub fn emit_rc_ops(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    _sigs: &FxHashMap<Name, MemoryContract>,
    _classifier: &dyn ArcClassification,
    pool: &Pool,
) -> EmitRcResult {
    debug_assert!(
        !func.var_reprs.is_empty(),
        "var_reprs must be populated before RC emission"
    );

    // Phase 1: per-block RC emission (body + terminator uses).
    for block_idx in 0..func.blocks.len() {
        emit_block_rc(func, block_idx, state_map, pool);
    }

    // Phase 2: inter-block edge cleanup.
    edge_cleanup::emit_edge_cleanup(func, state_map, pool);

    // Phase 3: locality hint collection (v1: hints only, no stack alloc).
    let local_alloc_candidates = collect_local_alloc_candidates(func, state_map);

    EmitRcResult {
        local_alloc_candidates,
    }
}

// Helpers

/// Convert a `usize` block index to `ArcBlockId`.
#[inline]
fn block_id(idx: usize) -> ArcBlockId {
    ArcBlockId::new(
        u32::try_from(idx).unwrap_or_else(|_| panic!("block index {idx} exceeds u32::MAX")),
    )
}

/// Where a variable is last used within a block.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LastUse {
    /// Last used in a body instruction at the given index.
    Body(usize),
    /// Last used in the block terminator.
    Terminator,
}

/// Pre-scan a block to determine total use count and last-use position
/// for each variable.
fn precompute_block_uses(block: &ArcBlock) -> FxHashMap<ArcVarId, (usize, LastUse)> {
    let mut info: FxHashMap<ArcVarId, (usize, LastUse)> = FxHashMap::default();

    for (instr_idx, instr) in block.body.iter().enumerate() {
        for var in instr.used_vars() {
            let entry = info.entry(var).or_insert((0, LastUse::Body(instr_idx)));
            entry.0 += 1;
            entry.1 = LastUse::Body(instr_idx);
        }
    }

    for var in block.terminator.used_vars() {
        let entry = info.entry(var).or_insert((0, LastUse::Terminator));
        entry.0 += 1;
        entry.1 = LastUse::Terminator;
    }

    info
}

/// Compute `RcStrategy` for a variable, returning `None` for scalars.
#[inline]
fn rc_strategy(func: &ArcFunction, var: ArcVarId, pool: &Pool) -> Option<RcStrategy> {
    let repr = func.var_reprs[var.index()];
    if repr == ValueRepr::Scalar {
        return None;
    }
    Some(RcStrategy::from_var(
        repr,
        pool,
        func.var_types[var.index()],
    ))
}

/// Whether a variable is live (cardinality > Absent) at a block's exit.
#[inline]
fn is_live_at_exit(state_map: &AimsStateMap, blk: ArcBlockId, var: ArcVarId) -> bool {
    state_map.var_state_at_block_exit(blk, var).cardinality != Cardinality::Absent
}

/// Whether a variable is owned (and trackable) at block entry or definition.
///
/// For variables defined in the block whose entry AND exit states are both
/// BOTTOM (not present in the state map — common in terminal blocks like
/// Return), determines ownership from the defining instruction: `Project`
/// creates borrowed references; all other definitions (`Construct`, `Apply`,
/// `PartialApply`, etc.) create owned values that need RC management.
#[inline]
fn is_owned_at_entry(
    state_map: &AimsStateMap,
    blk: ArcBlockId,
    var: ArcVarId,
    defined_in_block: &FxHashSet<ArcVarId>,
    borrowed_defs: &FxHashSet<ArcVarId>,
) -> bool {
    if state_map.is_scalar(var) {
        return false;
    }
    let entry_state = state_map.var_state_at_block_entry(blk, var);
    if entry_state.access == AccessClass::Owned {
        return true;
    }
    // Variable defined in this block may not have entry state — check exit.
    if entry_state.cardinality == Cardinality::Absent && defined_in_block.contains(&var) {
        let exit_state = state_map.var_state_at_block_exit(blk, var);
        if exit_state.access == AccessClass::Owned {
            return true;
        }
        // Exit state is also BOTTOM — variable created and consumed entirely
        // within this block (typical in terminal blocks). Ownership comes from
        // the defining instruction: Project creates borrowed refs, everything
        // else (Construct, Apply, PartialApply, etc.) creates owned values.
        if exit_state.cardinality == Cardinality::Absent {
            return !borrowed_defs.contains(&var);
        }
    }
    false
}

// Per-block emission

/// Collect variables defined in a block (body instructions + block params).
fn collect_defined_vars(block: &ArcBlock) -> FxHashSet<ArcVarId> {
    let mut defined = FxHashSet::default();
    for instr in &block.body {
        if let Some(dst) = instr.defined_var() {
            defined.insert(dst);
        }
    }
    for &(var, _) in &block.params {
        defined.insert(var);
    }
    defined
}

/// Collect variables defined by borrowing instructions (`Project`).
///
/// These create borrowed references that do NOT need independent RC
/// management — the source variable's RC covers the borrowed ref.
fn collect_borrowed_defs(block: &ArcBlock) -> FxHashSet<ArcVarId> {
    let mut borrowed = FxHashSet::default();
    for instr in &block.body {
        if let ArcInstr::Project { dst, .. } = instr {
            borrowed.insert(*dst);
        }
    }
    borrowed
}

/// Emit RC operations for a single block.
///
/// Forward walk with three phases:
/// - A: `RcDec` for variables live at entry, unused, dead at exit
/// - B: Forward walk through body with `RcInc`/`RcDec` interleaving
/// - C: Terminator uses and non-transfer `RcDec`
fn emit_block_rc(func: &mut ArcFunction, block_idx: usize, state_map: &AimsStateMap, pool: &Pool) {
    let blk = block_id(block_idx);
    let use_info = precompute_block_uses(&func.blocks[block_idx]);
    let defined_in_block = collect_defined_vars(&func.blocks[block_idx]);
    let borrowed_defs = collect_borrowed_defs(&func.blocks[block_idx]);

    let old_body = std::mem::take(&mut func.blocks[block_idx].body);
    let mut new_body: Vec<ArcInstr> = Vec::with_capacity(old_body.len() * 2);

    let ctx = BlockCtx {
        func,
        blk,
        state_map,
        defined_in_block: &defined_in_block,
        borrowed_defs: &borrowed_defs,
        use_info: &use_info,
        pool,
    };

    // Phase A: RcDec for variables live at entry, unused in block, dead at exit.
    emit_dead_at_entry_decs(&ctx, &mut new_body);

    // Phase B: forward walk through body instructions.
    let uses_so_far = emit_body_forward_walk(&ctx, &old_body, &mut new_body);

    // Phase C: terminator uses and cleanup.
    emit_terminator_rc(&ctx, block_idx, uses_so_far, &mut new_body);

    func.blocks[block_idx].body = new_body;
}

/// Phase A: `RcDec` for variables live at entry, unused in block, dead at exit.
fn emit_dead_at_entry_decs(ctx: &BlockCtx<'_>, new_body: &mut Vec<ArcInstr>) {
    let Some(entry_states) = ctx.state_map.block_entry_states(ctx.blk) else {
        return;
    };
    for (&var, &state) in entry_states {
        if state.is_scalar() || state.access != AccessClass::Owned {
            continue;
        }
        if state.cardinality == Cardinality::Absent {
            continue;
        }
        if ctx.use_info.contains_key(&var) || is_live_at_exit(ctx.state_map, ctx.blk, var) {
            continue;
        }
        if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
            new_body.push(ArcInstr::RcDec { var, strategy });
        }
    }
}

/// Phase B: forward walk through body, emitting `RcInc`/`RcDec` around each
/// instruction. Returns the accumulated use counts for Phase C.
fn emit_body_forward_walk(
    ctx: &BlockCtx<'_>,
    old_body: &[ArcInstr],
    new_body: &mut Vec<ArcInstr>,
) -> FxHashMap<ArcVarId, usize> {
    let mut uses_so_far: FxHashMap<ArcVarId, usize> = FxHashMap::default();

    for (instr_idx, instr) in old_body.iter().enumerate() {
        emit_pre_instr_incs(ctx, instr, instr_idx, &mut uses_so_far, new_body);
        new_body.push(instr.clone());
        emit_post_instr_decs(ctx, instr, instr_idx, new_body);
    }

    uses_so_far
}

/// Emit `RcInc` before each use in an instruction where a future use exists.
fn emit_pre_instr_incs(
    ctx: &BlockCtx<'_>,
    instr: &ArcInstr,
    instr_idx: usize,
    uses_so_far: &mut FxHashMap<ArcVarId, usize>,
    new_body: &mut Vec<ArcInstr>,
) {
    for var in instr.used_vars() {
        if !is_owned_at_entry(
            ctx.state_map,
            ctx.blk,
            var,
            ctx.defined_in_block,
            ctx.borrowed_defs,
        ) {
            continue;
        }

        let count = uses_so_far.entry(var).or_insert(0);
        *count += 1;

        let has_future_use = if let Some(&(total_uses, last_use)) = ctx.use_info.get(&var) {
            let remaining_in_block = total_uses - *count;
            remaining_in_block > 0
                || (matches!(last_use, LastUse::Terminator) && LastUse::Body(instr_idx) != last_use)
                || is_live_at_exit(ctx.state_map, ctx.blk, var)
        } else {
            false
        };

        if has_future_use {
            if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                new_body.push(ArcInstr::RcInc {
                    var,
                    count: 1,
                    strategy,
                });
            }
        }
    }
}

/// Emit `RcDec` after an instruction for defined-but-dead variables and
/// variables whose last use was this instruction.
fn emit_post_instr_decs(
    ctx: &BlockCtx<'_>,
    instr: &ArcInstr,
    instr_idx: usize,
    new_body: &mut Vec<ArcInstr>,
) {
    // RcDec for defined-but-dead variables.
    if let Some(dst) = instr.defined_var() {
        if !ctx.state_map.is_scalar(dst)
            && ctx.func.var_reprs[dst.index()] != ValueRepr::Scalar
            && !ctx.use_info.contains_key(&dst)
            && !is_live_at_exit(ctx.state_map, ctx.blk, dst)
        {
            if let Some(strategy) = rc_strategy(ctx.func, dst, ctx.pool) {
                new_body.push(ArcInstr::RcDec { var: dst, strategy });
            }
        }
    }

    // RcDec for variables whose last use was this instruction.
    for var in instr.used_vars() {
        if !is_owned_at_entry(
            ctx.state_map,
            ctx.blk,
            var,
            ctx.defined_in_block,
            ctx.borrowed_defs,
        ) {
            continue;
        }
        if let Some(&(_total, last_use)) = ctx.use_info.get(&var) {
            if last_use == LastUse::Body(instr_idx) && !is_live_at_exit(ctx.state_map, ctx.blk, var)
            {
                if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                    new_body.push(ArcInstr::RcDec { var, strategy });
                }
            }
        }
    }
}

/// Phase C: handle terminator uses and non-transfer `RcDec`.
fn emit_terminator_rc(
    ctx: &BlockCtx<'_>,
    block_idx: usize,
    mut uses_so_far: FxHashMap<ArcVarId, usize>,
    new_body: &mut Vec<ArcInstr>,
) {
    // RcInc for terminator uses with future (exit) liveness.
    for var in ctx.func.blocks[block_idx].terminator.used_vars() {
        if !is_owned_at_entry(
            ctx.state_map,
            ctx.blk,
            var,
            ctx.defined_in_block,
            ctx.borrowed_defs,
        ) {
            continue;
        }
        *uses_so_far.entry(var).or_insert(0) += 1;

        if is_live_at_exit(ctx.state_map, ctx.blk, var) {
            if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                new_body.push(ArcInstr::RcInc {
                    var,
                    count: 1,
                    strategy,
                });
            }
        }
    }

    // RcDec for Branch/Switch scrutinee — read but not ownership-transferred.
    // Return/Jump/Invoke transfer ownership; Resume/Unreachable have nothing.
    match &ctx.func.blocks[block_idx].terminator {
        ArcTerminator::Branch { cond, .. }
        | ArcTerminator::Switch {
            scrutinee: cond, ..
        } => {
            if !ctx.state_map.is_scalar(*cond) && !is_live_at_exit(ctx.state_map, ctx.blk, *cond) {
                if let Some(strategy) = rc_strategy(ctx.func, *cond, ctx.pool) {
                    new_body.push(ArcInstr::RcDec {
                        var: *cond,
                        strategy,
                    });
                }
            }
        }
        _ => {}
    }
}

// Locality hint collection

/// Collect local-allocation candidates from the state map (v1: hints only).
///
/// Scans for `Construct` instructions where the defined variable has
/// `Locality::FunctionLocal` or `BlockLocal`, indicating potential for
/// stack allocation in a future optimization pass.
fn collect_local_alloc_candidates(
    func: &ArcFunction,
    state_map: &AimsStateMap,
) -> Vec<LocalAllocCandidate> {
    let mut candidates = Vec::new();

    for (block_idx, block) in func.blocks.iter().enumerate() {
        let blk = block_id(block_idx);
        for (instr_idx, instr) in block.body.iter().enumerate() {
            if let Some(dst) = instr.defined_var() {
                if state_map.is_scalar(dst) {
                    continue;
                }
                let exit_state = state_map.var_state_at_block_exit(blk, dst);
                if matches!(
                    exit_state.locality,
                    Locality::FunctionLocal | Locality::BlockLocal
                ) {
                    candidates.push(LocalAllocCandidate {
                        block: blk,
                        instr: instr_idx,
                        var: dst,
                    });
                }
            }
        }
    }

    candidates
}
