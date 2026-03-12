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
mod coalesce;
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
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, RcStrategy,
    ValueRepr,
};
use crate::ownership::Ownership;
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
    /// Function-level set of all variables defined by `Project` in any block.
    all_borrowed_defs: &'a FxHashSet<ArcVarId>,
    use_info: &'a FxHashMap<ArcVarId, (usize, LastUse)>,
    pool: &'a Pool,
    /// For each Project source variable, the latest `LastUse` of any borrowed
    /// child (direct Project destinations or their Let aliases). Used to defer
    /// parent `RcDec` until all borrowed children are dead.
    child_effective_last_use: &'a FxHashMap<ArcVarId, LastUse>,
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

    // Build function-level set of all Project-defined (borrowed) variables.
    // Used to distinguish owned vs borrowed for cross-block live variables
    // whose lattice access dimension is stuck at BOTTOM (Borrowed) because
    // backward demand propagation doesn't update access.
    let all_borrowed_defs = collect_all_borrowed_defs(func);

    // Phase 1: per-block RC emission (body + terminator uses).
    // Collect deferred parent decs (per block) for edge cleanup — these are
    // parent aggregates whose RcDec was skipped because a borrowed child
    // (from Project) is used in the block terminator.
    let mut block_deferred: FxHashMap<usize, Vec<(ArcVarId, RcStrategy)>> = FxHashMap::default();
    for block_idx in 0..func.blocks.len() {
        let deferred = emit_block_rc(func, block_idx, state_map, pool, &all_borrowed_defs);
        if !deferred.is_empty() {
            block_deferred.insert(block_idx, deferred);
        }
    }

    // Phase 1.5: dead Invoke result cleanup.
    //
    // Invoke dst variables are defined at the boundary between predecessor
    // and successor blocks. The backward analysis may never propagate demand
    // for them (e.g., if the result is unused), so they don't appear in any
    // block's entry_states. Without this sweep, such variables leak.
    emit_dead_invoke_dsts(func, state_map, pool, &all_borrowed_defs);

    // Phase 2: inter-block edge cleanup (with deferred parent decs).
    edge_cleanup::emit_edge_cleanup(func, state_map, pool, &all_borrowed_defs, &block_deferred);

    // Phase 3: RC coalescing peephole — merge adjacent RC ops per block.
    for block in &mut func.blocks {
        coalesce::coalesce_block_rc(&mut block.body);
    }

    // Phase 4: locality hint collection (v1: hints only, no stack alloc).
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
/// Three cases:
///
/// 1. **Lattice says Owned**: the interprocedural/intraprocedural analysis
///    determined this variable has owned access. Trust it.
///
/// 2. **Defined in this block** (cardinality Absent at entry): ownership is
///    determined by the defining instruction. `Project` creates borrowed
///    references (no independent RC management); all other definitions
///    (`Construct`, `Apply`, `PartialApply`, etc.) create owned values.
///
/// 3. **Cross-block live variable** (cardinality > Absent, not defined in
///    this block): the variable was defined in another block and is live at
///    this block's entry. Backward demand propagation only updates
///    `cardinality` and `consumption`, leaving `access` at its BOTTOM
///    default (`Borrowed`) — so the access dimension is unreliable for
///    these variables. Variables from `Apply`/`Invoke`/`Construct` returns
///    are always owned; only `Project` creates borrowed references.
///    We use the function-level `all_borrowed_defs` set to distinguish.
#[inline]
fn is_owned_at_entry(
    state_map: &AimsStateMap,
    blk: ArcBlockId,
    var: ArcVarId,
    defined_in_block: &FxHashSet<ArcVarId>,
    borrowed_defs: &FxHashSet<ArcVarId>,
    all_borrowed_defs: &FxHashSet<ArcVarId>,
) -> bool {
    if state_map.is_excluded(var) {
        return false;
    }
    let entry_state = state_map.var_state_at_block_entry(blk, var);

    // Case 1: lattice says Owned.
    if entry_state.access == AccessClass::Owned {
        return true;
    }

    // Case 2: variable defined in this block (entry cardinality Absent).
    // Check both block-level borrowed_defs (Project) and function-level
    // all_borrowed_defs (Project + borrowed params + Let aliases).
    if entry_state.cardinality == Cardinality::Absent && defined_in_block.contains(&var) {
        return !borrowed_defs.contains(&var) && !all_borrowed_defs.contains(&var);
    }

    // Case 3: cross-block live variable. The variable is live at this
    // block's entry but was defined elsewhere. Access is unreliable
    // (stuck at BOTTOM). Ownership comes from the defining instruction:
    // Project → borrowed, everything else → owned.
    if entry_state.cardinality != Cardinality::Absent && !defined_in_block.contains(&var) {
        return !all_borrowed_defs.contains(&var);
    }

    // Case 4: variable defined by predecessor's terminator (Invoke).
    // The Invoke result variable has cardinality Absent in the entry
    // states (it doesn't exist at block entry — it's created by the
    // Invoke that branches TO this block). But it's used in this block
    // via Let aliases. It's not in defined_in_block (which only tracks
    // body instructions and block params). Check if it's a non-scalar,
    // non-project variable — if so, it's owned.
    if entry_state.cardinality == Cardinality::Absent && !defined_in_block.contains(&var) {
        return !all_borrowed_defs.contains(&var);
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

/// Collect ALL borrowed variables across all blocks.
///
/// Includes:
/// 1. Function parameters with `Ownership::Borrowed`
/// 2. `Project` instructions (borrowed views into structs/enums)
/// 3. `Let` aliases of any borrowed variable (transitive closure)
///
/// A `Let { dst, value: Var(src) }` where `src` is borrowed creates a
/// pointer copy without incrementing the refcount. The copy must also be
/// treated as borrowed to avoid emitting spurious `RcDec`.
fn collect_all_borrowed_defs(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    let mut borrowed = FxHashSet::default();
    // Function parameters with Borrowed ownership are genuinely borrowed.
    for param in &func.params {
        if param.ownership == Ownership::Borrowed {
            borrowed.insert(param.var);
        }
    }
    // Project instructions create borrowed views into structs/enums.
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { dst, .. } = instr {
                borrowed.insert(*dst);
            }
        }
    }
    // Let aliases of borrowed variables: `let dst = borrowed_var` creates a
    // pointer copy that shares the same refcount. Compute transitive closure
    // by iterating until no new aliases are discovered.
    let mut changed = true;
    while changed {
        changed = false;
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } = instr
                {
                    if borrowed.contains(src) && borrowed.insert(*dst) {
                        changed = true;
                    }
                }
            }
        }
    }
    borrowed
}

/// For each Project source variable in a block, compute the latest `LastUse`
/// of any borrowed child (direct Project destinations or their Let aliases).
///
/// A parent aggregate must not be decremented before all of its borrowed
/// children are dead. The AIMS backward analysis doesn't track this
/// relationship (it only propagates demand for direct uses), so the
/// emission phase extends parent lifetimes using this map.
fn compute_child_effective_last_use(
    block: &ArcBlock,
    use_info: &FxHashMap<ArcVarId, (usize, LastUse)>,
) -> FxHashMap<ArcVarId, LastUse> {
    // Build mapping from each borrowed child to its Project source.
    let mut child_to_parent: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    for instr in &block.body {
        if let ArcInstr::Project { dst, value, .. } = instr {
            child_to_parent.insert(*dst, *value);
        }
    }

    // Extend to Let aliases: `Let { dst, Var(src) }` where `src` borrows
    // from a parent means `dst` also borrows from the same parent.
    let mut changed = true;
    while changed {
        changed = false;
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                if let Some(&parent) = child_to_parent.get(src) {
                    if !child_to_parent.contains_key(dst) {
                        child_to_parent.insert(*dst, parent);
                        changed = true;
                    }
                }
            }
        }
    }

    // For each parent, find the latest LastUse across all its children.
    let mut result: FxHashMap<ArcVarId, LastUse> = FxHashMap::default();
    for (&child, &parent) in &child_to_parent {
        if let Some(&(_, child_last)) = use_info.get(&child) {
            let entry = result.entry(parent).or_insert(child_last);
            let is_later = match (*entry, child_last) {
                (LastUse::Body(cur), LastUse::Body(new)) => new > cur,
                (LastUse::Body(_), LastUse::Terminator) => true,
                _ => false,
            };
            if is_later {
                *entry = child_last;
            }
        }
    }

    result
}

/// Emit RC operations for a single block.
///
/// Forward walk with three phases:
/// - A: `RcDec` for variables live at entry, unused, dead at exit
/// - B: Forward walk through body with `RcInc`/`RcDec` interleaving
/// - C: Terminator uses and non-transfer `RcDec`
///
/// Returns deferred parent `RcDec` operations for variables whose borrowed
/// children are used in the block terminator. These must be placed on
/// successor edges by edge cleanup (the parent must outlive its children).
fn emit_block_rc(
    func: &mut ArcFunction,
    block_idx: usize,
    state_map: &AimsStateMap,
    pool: &Pool,
    all_borrowed_defs: &FxHashSet<ArcVarId>,
) -> Vec<(ArcVarId, RcStrategy)> {
    let blk = block_id(block_idx);
    let use_info = precompute_block_uses(&func.blocks[block_idx]);
    let defined_in_block = collect_defined_vars(&func.blocks[block_idx]);
    let borrowed_defs = collect_borrowed_defs(&func.blocks[block_idx]);
    let child_elu = compute_child_effective_last_use(&func.blocks[block_idx], &use_info);

    let old_body = std::mem::take(&mut func.blocks[block_idx].body);
    let mut new_body: Vec<ArcInstr> = Vec::with_capacity(old_body.len() * 2);

    let ctx = BlockCtx {
        func,
        blk,
        state_map,
        defined_in_block: &defined_in_block,
        borrowed_defs: &borrowed_defs,
        all_borrowed_defs,
        use_info: &use_info,
        pool,
        child_effective_last_use: &child_elu,
    };

    // Phase A: RcDec for variables live at entry, unused in block, dead at exit.
    emit_dead_at_entry_decs(&ctx, &mut new_body);

    // Phase B: forward walk through body instructions.
    let (uses_so_far, terminator_deferred) = emit_body_forward_walk(&ctx, &old_body, &mut new_body);

    // Phase C: terminator uses and cleanup.
    emit_terminator_rc(&ctx, block_idx, uses_so_far, &mut new_body);

    // For terminators without successors (Return/Resume/Unreachable),
    // emit deferred parent decs in the body. For terminators with
    // successors, return them for edge cleanup.
    let edge_deferred = match &func.blocks[block_idx].terminator {
        ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {
            for &(var, strategy) in &terminator_deferred {
                new_body.push(ArcInstr::RcDec { var, strategy });
            }
            Vec::new()
        }
        _ => terminator_deferred,
    };

    func.blocks[block_idx].body = new_body;
    edge_deferred
}

/// Phase A: `RcDec` for variables live at entry, unused in block, dead at exit.
fn emit_dead_at_entry_decs(ctx: &BlockCtx<'_>, new_body: &mut Vec<ArcInstr>) {
    let Some(entry_states) = ctx.state_map.block_entry_states(ctx.blk) else {
        return;
    };
    for (&var, &state) in entry_states {
        if state.is_scalar() {
            continue;
        }
        if state.cardinality == Cardinality::Absent {
            continue;
        }
        // Use is_owned_at_entry to handle cross-block variables whose
        // access dimension is stuck at BOTTOM (Borrowed) due to backward
        // demand propagation not updating access.
        if !is_owned_at_entry(
            ctx.state_map,
            ctx.blk,
            var,
            ctx.defined_in_block,
            ctx.borrowed_defs,
            ctx.all_borrowed_defs,
        ) {
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

/// Sweep for dead Invoke result variables across all blocks.
///
/// An Invoke terminator's `dst` variable is "born" at the edge between the
/// predecessor (where the Invoke lives) and the successor blocks. The backward
/// analysis may never see demand for this variable (e.g., if the result is
/// unused or only used in the defining block), causing it to be absent from
/// all entry/exit state maps. Phases A–C only process variables they find in
/// `entry_states` or `use_info`, so they miss these orphaned definitions entirely.
///
/// This function scans every Invoke terminator and checks whether its `dst`
/// variable has an `RcDec` in the normal successor block. If not, and the
/// variable is an `RcPointer`, it prepends one. The same check is done for
/// the unwind successor.
fn emit_dead_invoke_dsts(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
    all_borrowed_defs: &FxHashSet<ArcVarId>,
) {
    // Collect (successor_block_idx, var, strategy) tuples to avoid borrowing
    // func mutably while iterating.
    let mut pending_decs: Vec<(usize, ArcVarId, RcStrategy)> = Vec::new();

    for block in &func.blocks {
        let ArcTerminator::Invoke { dst, normal, .. } = &block.terminator else {
            continue;
        };

        // Skip scalars and borrowed defs.
        if state_map.is_excluded(*dst) || all_borrowed_defs.contains(dst) {
            continue;
        }

        let Some(strategy) = rc_strategy(func, *dst, pool) else {
            continue;
        };

        // Only check the NORMAL successor. On the unwind path, the Invoke's
        // dst variable is never populated (the call threw), so decrementing it
        // would be a use-after-free of uninitialized memory.
        let succ_idx = normal.index();
        if succ_idx >= func.blocks.len() {
            continue;
        }

        let has_dec = func.blocks[succ_idx]
            .body
            .iter()
            .any(|instr| matches!(instr, ArcInstr::RcDec { var, .. } if *var == *dst));
        if has_dec {
            continue;
        }

        // Skip if the variable is used in the successor (Phase B/C
        // will have emitted appropriate RcDec already).
        let used_in_succ = func.blocks[succ_idx]
            .body
            .iter()
            .any(|instr| instr.used_vars().contains(dst))
            || func.blocks[succ_idx].terminator.used_vars().contains(dst);
        if used_in_succ {
            continue;
        }

        // Skip if the variable is live at the exit of the successor block
        // (it's still needed in downstream blocks).
        let succ_blk = block_id(succ_idx);
        if is_live_at_exit(state_map, succ_blk, *dst) {
            continue;
        }

        pending_decs.push((succ_idx, *dst, strategy));
    }

    // Prepend RcDec at the start of each successor block.
    for (succ_idx, var, strategy) in pending_decs {
        func.blocks[succ_idx]
            .body
            .insert(0, ArcInstr::RcDec { var, strategy });
    }
}

/// Phase B: forward walk through body, emitting `RcInc`/`RcDec` around each
/// instruction. Returns the accumulated use counts for Phase C and any
/// deferred parent `RcDec` operations whose borrowed children are used in
/// the block terminator.
fn emit_body_forward_walk(
    ctx: &BlockCtx<'_>,
    old_body: &[ArcInstr],
    new_body: &mut Vec<ArcInstr>,
) -> (FxHashMap<ArcVarId, usize>, Vec<(ArcVarId, RcStrategy)>) {
    let mut uses_so_far: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    // Deferred parent decs: parent vars whose RcDec was skipped because
    // a borrowed child has a later use. (var, strategy, effective_last_use).
    let mut deferred: Vec<(ArcVarId, RcStrategy, LastUse)> = Vec::new();

    for (instr_idx, instr) in old_body.iter().enumerate() {
        emit_pre_instr_incs(ctx, instr, instr_idx, &mut uses_so_far, new_body);
        new_body.push(instr.clone());
        emit_post_instr_decs(ctx, instr, instr_idx, new_body, &mut deferred);

        // Emit deferred parent decs whose children's last use is this instruction.
        deferred.retain(|&(var, strategy, effective_last)| {
            if effective_last == LastUse::Body(instr_idx)
                && !is_live_at_exit(ctx.state_map, ctx.blk, var)
            {
                new_body.push(ArcInstr::RcDec { var, strategy });
                return false;
            }
            true
        });
    }

    // Remaining deferred parents: children used in the terminator.
    let terminator_deferred = deferred
        .into_iter()
        .map(|(var, strategy, _)| (var, strategy))
        .collect();

    (uses_so_far, terminator_deferred)
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
        let owned = is_owned_at_entry(
            ctx.state_map,
            ctx.blk,
            var,
            ctx.defined_in_block,
            ctx.borrowed_defs,
            ctx.all_borrowed_defs,
        );
        if !owned {
            continue;
        }

        let count = uses_so_far.entry(var).or_insert(0);
        *count += 1;

        let has_future_use = if let Some(&(total_uses, last_use)) = ctx.use_info.get(&var) {
            let remaining_in_block = total_uses - *count;
            let live = is_live_at_exit(ctx.state_map, ctx.blk, var);
            remaining_in_block > 0
                || (matches!(last_use, LastUse::Terminator) && LastUse::Body(instr_idx) != last_use)
                || live
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
///
/// Parent variables with borrowed children that have later uses are deferred
/// rather than decremented immediately (the parent must outlive its children).
fn emit_post_instr_decs(
    ctx: &BlockCtx<'_>,
    instr: &ArcInstr,
    instr_idx: usize,
    new_body: &mut Vec<ArcInstr>,
    deferred: &mut Vec<(ArcVarId, RcStrategy, LastUse)>,
) {
    // RcDec for defined-but-dead variables.
    if let Some(dst) = instr.defined_var() {
        if !ctx.state_map.is_excluded(dst)
            && ctx.func.var_reprs[dst.index()] != ValueRepr::Scalar
            && !ctx.use_info.contains_key(&dst)
            && !is_live_at_exit(ctx.state_map, ctx.blk, dst)
        {
            if let Some(strategy) = rc_strategy(ctx.func, dst, ctx.pool) {
                new_body.push(ArcInstr::RcDec { var: dst, strategy });
            }
        }
    }

    // Skip operand drops for consuming PrimOp instructions (e.g., list/string
    // concat). These produce RcPtr results via COW runtime functions that
    // handle operand RC internally (realloc or copy+dec). Emitting separate
    // RcDec here would double-free.
    if is_consuming_primop(instr, ctx.func) {
        return;
    }

    // Skip last-use RcDec for alias assignments (`Let { dst, Var(src) }`).
    // Ownership transfers from `src` to `dst` — `dst` will have its own
    // RcDec when it dies. Emitting RcDec for `src` here would double-dec
    // the same refcount.
    if is_ownership_transfer(instr, ctx.func) {
        return;
    }

    // RcDec for variables whose last use was this instruction.
    for (pos, var) in instr.used_vars().into_iter().enumerate() {
        if !is_owned_at_entry(
            ctx.state_map,
            ctx.blk,
            var,
            ctx.defined_in_block,
            ctx.borrowed_defs,
            ctx.all_borrowed_defs,
        ) {
            continue;
        }
        // Skip RcDec for variables at owned positions in call instructions.
        // Ownership transfers to the callee — the callee handles the dec.
        if instr.is_owned_position(pos) {
            continue;
        }
        if let Some(&(_total, last_use)) = ctx.use_info.get(&var) {
            if last_use == LastUse::Body(instr_idx) && !is_live_at_exit(ctx.state_map, ctx.blk, var)
            {
                // Defer RcDec if this variable has borrowed children with
                // later uses. The parent must outlive all its children.
                if let Some(&child_last) = ctx.child_effective_last_use.get(&var) {
                    let child_is_later = match child_last {
                        LastUse::Body(c) => c > instr_idx,
                        LastUse::Terminator => true,
                    };
                    if child_is_later {
                        if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                            deferred.push((var, strategy, child_last));
                        }
                        continue;
                    }
                }

                if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                    new_body.push(ArcInstr::RcDec { var, strategy });
                }
            }
        }
    }
}

/// Whether an instruction is a consuming `PrimOp` — a `Let` with a `PrimOp`
/// value that produces an `RcPtr` result.
///
/// `RcPtr`-producing `PrimOp`s are collection operations (list concat, string
/// concat, etc.) compiled to COW runtime functions that consume their
/// operands internally. The ARC pipeline must NOT emit separate `RcDec`
/// for the operands.
#[inline]
fn is_consuming_primop(instr: &ArcInstr, func: &ArcFunction) -> bool {
    if let ArcInstr::Let {
        dst,
        value: crate::ir::ArcValue::PrimOp { .. },
        ..
    } = instr
    {
        func.var_reprs[dst.index()] != ValueRepr::Scalar
    } else {
        false
    }
}

/// Whether an instruction transfers ownership from used variables into a
/// destination value that will manage their lifetimes.
///
/// In Perceus terms, the last use of a source variable in an ownership-
/// transferring instruction is NOT a drop point — the destination now
/// owns the value (or its fields). The `dst` variable will have its own
/// `RcDec`/drop when it dies, which recursively cleans up children.
///
/// Covered cases:
/// - `Let { dst, Var(src) }` — alias: `dst` is another name for `src`
/// - `Construct { dst, args }` — struct/enum build: args become fields of `dst`
/// - `PartialApply { dst, args }` — closure capture: args are captured by `dst`
#[inline]
fn is_ownership_transfer(instr: &ArcInstr, func: &ArcFunction) -> bool {
    match instr {
        ArcInstr::Let {
            dst,
            value: crate::ir::ArcValue::Var(_),
            ..
        }
        | ArcInstr::Construct { dst, .. }
        | ArcInstr::PartialApply { dst, .. } => func.var_reprs[dst.index()] != ValueRepr::Scalar,
        _ => false,
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
            ctx.all_borrowed_defs,
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
            if !ctx.state_map.is_excluded(*cond) && !is_live_at_exit(ctx.state_map, ctx.blk, *cond)
            {
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
                if state_map.is_excluded(dst) {
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

// Shared RC-incremented variable tracking

/// Collect all variables whose underlying object had an `RcInc` emitted.
///
/// Returns a set containing:
/// - Every variable `v` that has an `RcInc { var: v }` instruction
/// - Every variable `d` defined as `Let { dst: d, value: Var(s) }` where
///   `s` is in the set (transitive through alias chains)
///
/// Used by both COW annotations and drop hints: after RC emission, any
/// variable in this set shares a heap object whose physical refcount was
/// bumped above 1. The pre-emission AIMS uniqueness state (`Unique`) no
/// longer reflects the physical reality for these variables.
pub(crate) fn collect_rc_incremented_vars(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    let mut incremented = FxHashSet::default();
    let mut alias_edges: Vec<Option<ArcVarId>> = vec![None; func.var_types.len()];

    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::RcInc { var, .. } => {
                    incremented.insert(*var);
                }
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } => {
                    alias_edges[dst.index()] = Some(*src);
                }
                _ => {}
            }
        }
    }

    // Propagate: for each alias edge dst → src, if src is incremented,
    // dst is also incremented (shares the same object).
    // Fixed-point loop handles chains like %11 = %8 = %6 (where %6 has RcInc).
    loop {
        let mut changed = false;
        for (i, alias) in alias_edges.iter().enumerate() {
            if let Some(src) = alias {
                if incremented.contains(src) {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "ARC IR var counts fit in u32"
                    )]
                    let dst = ArcVarId::new(i as u32);
                    if incremented.insert(dst) {
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }

    incremented
}
