//! RC insertion pass for ARC IR (Section 07.2).
//!
//! Places `RcInc` and `RcDec` instructions precisely using liveness analysis
//! results. This is the Perceus algorithm: every heap-allocated value is freed
//! exactly once at its last use, and additional uses get `RcInc`.
//!
//! # Algorithm
//!
//! For each block, walk instructions **backward** with a running `live` set
//! initialized from `live_out`:
//!
//! 1. **Terminator uses**: Variables used in the terminator that are already
//!    live get `RcInc` (they survive past the terminator). New uses join `live`.
//!
//! 2. **Instruction backward pass**: For each instruction in reverse:
//!    - **Definitions**: If the defined variable is not in `live`, it's dead
//!      immediately — emit `RcDec`. Otherwise remove from `live`.
//!    - **Uses**: If a used variable is already in `live`, emit `RcInc`
//!      (multi-use). Add to `live`.
//!
//! 3. **Block/function parameters**: Any block param (or entry-block function
//!    param) not in `live` after processing the body gets `RcDec` (unused param).
//!
//! # Borrowed Parameters
//!
//! Borrowed params (from borrow inference §06.2) and variables derived from
//! them skip all RC tracking — no Inc, no Dec. When a borrowed-derived
//! variable flows into an *owned position* (stored in `Construct`, captured
//! in `PartialApply`, etc.), it gets a single `RcInc` to transfer ownership.
//!
//! # References
//!
//! - Lean 4: `src/Lean/Compiler/IR/RC.lean`
//! - Koka: Perceus paper §3.2
//! - Swift: `lib/SILOptimizer/ARC/`

use rustc_hash::FxHashSet;

use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};
use crate::liveness::BlockLiveness;
use crate::ownership::Ownership;
use crate::ArcClassification;

/// Shared context for RC insertion within a single block.
///
/// Groups the parameters that would otherwise be threaded through every
/// helper function, keeping function signatures manageable.
struct RcContext<'a> {
    func: &'a ArcFunction,
    classifier: &'a dyn ArcClassification,
    /// Function parameters annotated as `Borrowed` — completely skip RC.
    borrowed_params: &'a FxHashSet<ArcVarId>,
    /// Variables derived from borrowed params — skip RC except at owned positions.
    borrows: &'a FxHashSet<ArcVarId>,
}

/// Insert `RcInc`/`RcDec` operations into an ARC IR function.
///
/// Modifies `func.blocks` in-place, inserting RC operations based on
/// liveness analysis. Borrowed parameters (from §06.2) and variables
/// derived from them skip RC tracking entirely.
///
/// # Arguments
///
/// * `func` — the ARC IR function to transform (mutated in place).
/// * `classifier` — type classifier for `needs_rc()` checks.
/// * `liveness` — precomputed liveness (from [`compute_liveness`](crate::compute_liveness)).
pub fn insert_rc_ops(
    func: &mut ArcFunction,
    classifier: &dyn ArcClassification,
    liveness: &BlockLiveness,
) {
    tracing::debug!(function = func.name.raw(), "inserting RC operations");

    // Collect borrowed function parameters.
    let borrowed_params: FxHashSet<ArcVarId> = func
        .params
        .iter()
        .filter(|p| p.ownership == Ownership::Borrowed)
        .map(|p| p.var)
        .collect();

    let entry_idx = func.entry.index();
    let num_blocks = func.blocks.len();

    // Precompute Invoke dst definitions for each normal successor.
    // See liveness.rs `collect_invoke_defs` — same concept: an Invoke's
    // dst is defined at the normal successor's entry, like a block param.
    let invoke_defs = crate::liveness::collect_invoke_defs(func);

    for block_idx in 0..num_blocks {
        // Compute the borrows set: variables *derived from* borrowed params
        // (e.g., via Project or Let alias). Does NOT include the borrowed
        // params themselves — those are handled separately with a complete
        // skip of all RC tracking.
        let borrows = compute_borrows(&func.blocks[block_idx], &borrowed_params);

        let ctx = RcContext {
            func,
            classifier,
            borrowed_params: &borrowed_params,
            borrows: &borrows,
        };

        let live_out = &liveness.live_out[block_idx];
        let mut live = live_out.clone();

        // We build new_body in reverse order and flip at the end.
        let mut new_body: Vec<ArcInstr> = Vec::new();
        // Track spans: None for inserted RC ops, original span for original instrs.
        let mut new_spans: Vec<Option<ori_ir::Span>> = Vec::new();

        let block = &func.blocks[block_idx];
        let old_spans = &func.spans[block_idx];

        // ── Step 1: Process terminator uses ──────────────────────
        //
        // The terminator's used_vars are "live at exit" of the instruction
        // stream. If a terminator var is already live (used later in a
        // successor), it needs an Inc. Otherwise it joins the live set.
        process_terminator_uses(
            &block.terminator,
            &mut live,
            &mut new_body,
            &mut new_spans,
            &ctx,
        );

        // ── Step 2: Backward body pass ───────────────────────────
        for (instr_idx, instr) in block.body.iter().enumerate().rev() {
            let span = if instr_idx < old_spans.len() {
                old_spans[instr_idx]
            } else {
                None
            };

            // Definition: if dst is RC'd, non-borrowed, and not live → dead def, emit Dec.
            if let Some(dst) = instr.defined_var() {
                if needs_rc_trackable(dst, &ctx) && !live.remove(&dst) {
                    // Dead definition — value is created but never used.
                    new_body.push(ArcInstr::RcDec { var: dst });
                    new_spans.push(None);
                }
            }

            // Push the instruction itself.
            new_body.push(instr.clone());
            new_spans.push(span);

            // Uses: for each used var, if it's already live → multi-use, emit Inc.
            // Use the owned-position-aware logic for borrowed-derived vars.
            process_instruction_uses(instr, &mut live, &mut new_body, &mut new_spans, &ctx);
        }

        // ── Step 3: Block parameters ─────────────────────────────
        //
        // Any block param that is RC'd, not borrowed, and not in the live
        // set at this point was never used in the block body → Dec it.
        for &(param_var, _ty) in block.params.iter().rev() {
            if needs_rc_trackable(param_var, &ctx) && !live.remove(&param_var) {
                new_body.push(ArcInstr::RcDec { var: param_var });
                new_spans.push(None);
            }
        }

        // ── Step 3.5: Invoke dst definitions ─────────────────────
        //
        // An Invoke's `dst` is defined at the normal successor's entry,
        // acting like a block parameter. If the dst is RC'd and not in
        // the live set, it was never used → Dec it.
        let block_id = func.blocks[block_idx].id;
        if let Some(dsts) = invoke_defs.get(&block_id) {
            for &dst in dsts.iter().rev() {
                if needs_rc_trackable(dst, &ctx) && !live.remove(&dst) {
                    new_body.push(ArcInstr::RcDec { var: dst });
                    new_spans.push(None);
                }
            }
        }

        // ── Step 4: Entry block function params ──────────────────
        //
        // Owned function params that are not in `live` after processing
        // the entry block body need a Dec (unused param).
        if block_idx == entry_idx {
            for param in func.params.iter().rev() {
                if param.ownership == Ownership::Owned
                    && classifier.needs_rc(param.ty)
                    && !live.remove(&param.var)
                {
                    new_body.push(ArcInstr::RcDec { var: param.var });
                    new_spans.push(None);
                }
            }
        }

        // ── Reverse to get correct order ─────────────────────────
        new_body.reverse();
        new_spans.reverse();

        func.blocks[block_idx].body = new_body;
        func.spans[block_idx] = new_spans;
    }

    // ── Step 5: Edge cleanup ────────────────────────────────────
    //
    // After per-block RC insertion, handle "stranded" variables that are
    // live at a predecessor's exit but not needed by a successor.
    // See `insert_edge_cleanup` for details.
    insert_edge_cleanup(func, classifier, liveness, &borrowed_params);
}

/// Process terminator uses for RC insertion.
///
/// Each variable used in the terminator: if it needs RC, is not borrowed,
/// and is already in the live set → emit `RcInc`. Add to live set.
///
/// For `Return`, the returned variable is treated as an owned position
/// for borrowed-derived vars (transfer to caller requires Inc).
fn process_terminator_uses(
    terminator: &ArcTerminator,
    live: &mut FxHashSet<ArcVarId>,
    new_body: &mut Vec<ArcInstr>,
    new_spans: &mut Vec<Option<ori_ir::Span>>,
    ctx: &RcContext<'_>,
) {
    // Determine which terminator positions are "owned" for borrowed-derived vars.
    let is_return = matches!(terminator, ArcTerminator::Return { .. });

    for var in terminator.used_vars() {
        if !ctx.classifier.needs_rc(ctx.func.var_type(var)) {
            continue;
        }

        // Borrowed params: completely skip all RC tracking.
        if ctx.borrowed_params.contains(&var) {
            if is_return {
                // Returning a borrowed param transfers ownership to caller.
                // Must Inc even for a borrowed param.
                new_body.push(ArcInstr::RcInc { var, count: 1 });
                new_spans.push(None);
            }
            continue;
        }

        // Borrowed-derived vars: Inc only in owned positions.
        if ctx.borrows.contains(&var) {
            if is_return {
                new_body.push(ArcInstr::RcInc { var, count: 1 });
                new_spans.push(None);
            }
            continue;
        }

        // Normal (non-borrowed) var.
        if live.contains(&var) {
            new_body.push(ArcInstr::RcInc { var, count: 1 });
            new_spans.push(None);
        }
        live.insert(var);
    }
}

/// Process uses of a single instruction for RC insertion.
///
/// For each used variable:
/// - If it's a borrowed param → skip entirely.
/// - If it's a borrowed-derived var in a non-owned position → skip.
/// - If it's a borrowed-derived var in an owned position → emit `RcInc`.
/// - If it's a normal var already in `live` → emit `RcInc` (multi-use).
/// - Add to `live` (unless borrowed).
///
/// "Owned positions" are instruction slots where the value will be stored
/// on the heap: `Construct` args, `PartialApply` args, `Apply`/`ApplyIndirect`
/// args (conservative for unknown callees).
fn process_instruction_uses(
    instr: &ArcInstr,
    live: &mut FxHashSet<ArcVarId>,
    new_body: &mut Vec<ArcInstr>,
    new_spans: &mut Vec<Option<ori_ir::Span>>,
    ctx: &RcContext<'_>,
) {
    // Collect unique vars and count occurrences to handle duplicate args.
    // For example, `Apply { args: [x, x] }` should emit exactly 1 Inc
    // (x appears twice, but one use is "free" and the second is Inc).
    let used = instr.used_vars();
    let mut seen = FxHashSet::default();

    // Determine which positions are "owned" for borrowed-derived vars.
    let owned_positions = compute_owned_positions(instr);

    for (pos, &var) in used.iter().enumerate() {
        if !ctx.classifier.needs_rc(ctx.func.var_type(var)) {
            continue;
        }

        // Borrowed params: completely skip all RC tracking.
        // No Inc, no Dec, not added to live set.
        if ctx.borrowed_params.contains(&var) {
            continue;
        }

        // Borrowed-derived vars: only emit Inc if in an owned position.
        if ctx.borrows.contains(&var) {
            if owned_positions.contains(&pos) {
                new_body.push(ArcInstr::RcInc { var, count: 1 });
                new_spans.push(None);
            }
            continue;
        }

        // Normal (non-borrowed) var.
        if !seen.insert(var) {
            // Duplicate arg in the same instruction — already handled below.
            // The first occurrence either adds to live or emits Inc.
            // The second occurrence always needs Inc.
            new_body.push(ArcInstr::RcInc { var, count: 1 });
            new_spans.push(None);
            continue;
        }

        if live.contains(&var) {
            // Already live → multi-use, emit Inc.
            new_body.push(ArcInstr::RcInc { var, count: 1 });
            new_spans.push(None);
        }
        live.insert(var);
    }
}

/// Compute which argument positions in an instruction are "owned" —
/// where a borrowed-derived variable would need an `RcInc` to transfer
/// ownership to the heap.
///
/// Returns a set of indices into `instr.used_vars()`.
fn compute_owned_positions(instr: &ArcInstr) -> FxHashSet<usize> {
    let mut owned = FxHashSet::default();
    match instr {
        // Construct, PartialApply, Apply: all args stored or consumed.
        ArcInstr::Construct { args, .. }
        | ArcInstr::PartialApply { args, .. }
        | ArcInstr::Apply { args, .. } => {
            for i in 0..args.len() {
                owned.insert(i);
            }
        }
        // ApplyIndirect: closure + all args are owned (unknown callee).
        ArcInstr::ApplyIndirect { args, .. } => {
            // Position 0 is the closure, positions 1..=args.len() are args.
            for i in 0..=args.len() {
                owned.insert(i);
            }
        }
        // Project, Let(Var), Let(Literal), Let(PrimOp), etc. — read-only.
        _ => {}
    }
    owned
}

/// Compute the "borrows" set for a block — variables *derived from*
/// borrowed parameters via projections or aliasing.
///
/// This set does NOT include the borrowed params themselves (those are
/// handled separately with a complete skip of all RC tracking). It only
/// contains vars that inherit borrowed status through `Project` or
/// `Let { value: Var(_) }`.
///
/// Follows Lean 4's `LiveVars.borrows` pattern.
fn compute_borrows(block: &ArcBlock, borrowed_params: &FxHashSet<ArcVarId>) -> FxHashSet<ArcVarId> {
    // Start with an empty set — borrowed params are NOT included.
    // We track a "source is borrowed" set that includes both borrowed params
    // and derived vars, but only derived vars go into the output.
    let mut all_borrowed = borrowed_params.clone();
    let mut derived = FxHashSet::default();

    for instr in &block.body {
        match instr {
            ArcInstr::Project { dst, value, .. } if all_borrowed.contains(value) => {
                all_borrowed.insert(*dst);
                derived.insert(*dst);
            }
            ArcInstr::Let {
                dst,
                value: ArcValue::Var(v),
                ..
            } if all_borrowed.contains(v) => {
                all_borrowed.insert(*dst);
                derived.insert(*dst);
            }
            _ => {}
        }
    }

    derived
}

/// Check if a variable needs standard RC tracking (not borrowed, needs RC).
///
/// Returns `false` for borrowed params, borrowed-derived vars, and scalars.
/// These vars are either completely skipped (borrowed params) or handled
/// with the owned-position logic (derived vars).
#[inline]
fn needs_rc_trackable(var: ArcVarId, ctx: &RcContext<'_>) -> bool {
    !ctx.borrowed_params.contains(&var)
        && !ctx.borrows.contains(&var)
        && ctx.classifier.needs_rc(ctx.func.var_type(var))
}

// ── Edge cleanup ──────────────────────────────────────────────────────
//
// After per-block RC insertion, variables that are live at a
// predecessor's exit but not live at a successor's entry need `RcDec`
// at the successor. This happens when a branch splits control flow and
// each path needs a different subset of the live variables.
//
// For example:
//   block_0: construct v0(str), v1(str); branch → b1, b2
//   block_1: return v0      ← v1 is live_out[b0] but not live_in[b1]
//   block_2: apply f(v0, v1)
//
// The gap `live_out[pred] - live_in[succ]` must be Dec'd.
//
// References:
//   - Lean 4: `addDecForDeadParams` in `src/Lean/Compiler/IR/RC.lean`
//   - Appel: "Modern Compiler Implementation" §10.2

/// Insert `RcDec` at block boundaries for variables that are live at a
/// predecessor's exit but not needed by a successor ("edge gap").
///
/// For single-predecessor blocks: inserts Decs at the block's start.
/// For multi-predecessor blocks with identical gaps: inserts at block start.
/// For multi-predecessor blocks with differing gaps: creates trampoline
/// blocks (edge splitting) to hold per-edge cleanup.
fn insert_edge_cleanup(
    func: &mut ArcFunction,
    classifier: &dyn ArcClassification,
    liveness: &BlockLiveness,
    borrowed_params: &FxHashSet<ArcVarId>,
) {
    let num_blocks = func.blocks.len();
    let predecessors = compute_predecessors(func);
    let global_borrows = compute_global_borrows(func, borrowed_params);

    // Collect edits before applying (to avoid index invalidation).
    // (block_idx, vars_to_dec) for blocks where Decs go at the start.
    let mut block_decs: Vec<(usize, Vec<ArcVarId>)> = Vec::new();
    // (pred_idx, succ_block_id, vars_to_dec) for edges that need splitting.
    let mut edge_splits: Vec<(usize, ArcBlockId, Vec<ArcVarId>)> = Vec::new();

    for (block_idx, preds) in predecessors.iter().enumerate().take(num_blocks) {
        if preds.is_empty() {
            continue;
        }

        let live_in_b = &liveness.live_in[block_idx];

        // Compute per-predecessor gaps, filtering out borrowed vars.
        let gaps: Vec<(usize, Vec<ArcVarId>)> = preds
            .iter()
            .map(|&pred_idx| {
                let mut gap: Vec<ArcVarId> = liveness.live_out[pred_idx]
                    .iter()
                    .copied()
                    .filter(|v| {
                        !live_in_b.contains(v)
                            && !borrowed_params.contains(v)
                            && !global_borrows.contains(v)
                            && classifier.needs_rc(func.var_type(*v))
                    })
                    .collect();
                gap.sort_by_key(|v| v.index()); // deterministic order
                (pred_idx, gap)
            })
            .collect();

        // Skip if all gaps are empty.
        if gaps.iter().all(|(_, g)| g.is_empty()) {
            continue;
        }

        if preds.len() == 1 {
            // Single predecessor: insert Decs at block start.
            let (_, ref gap) = gaps[0];
            if !gap.is_empty() {
                block_decs.push((block_idx, gap.clone()));
            }
        } else {
            // Multiple predecessors: check if all gaps are identical.
            let all_identical = gaps.windows(2).all(|w| w[0].1 == w[1].1);

            if all_identical {
                // All predecessors have the exact same gap → insert at block start.
                if !gaps[0].1.is_empty() {
                    block_decs.push((block_idx, gaps[0].1.clone()));
                }
            } else {
                // Different gaps per predecessor → edge split each non-empty gap.
                for &(pred_idx, ref gap) in &gaps {
                    if !gap.is_empty() {
                        edge_splits.push((pred_idx, func.blocks[block_idx].id, gap.clone()));
                    }
                }
            }
        }
    }

    // Apply block-start Decs (prepend to block body).
    for (block_idx, vars) in &block_decs {
        let decs: Vec<ArcInstr> = vars.iter().map(|&v| ArcInstr::RcDec { var: v }).collect();
        let dec_spans: Vec<Option<ori_ir::Span>> = vec![None; decs.len()];

        let mut new_body = decs;
        new_body.append(&mut func.blocks[*block_idx].body);
        func.blocks[*block_idx].body = new_body;

        let mut new_spans = dec_spans;
        new_spans.append(&mut func.spans[*block_idx]);
        func.spans[*block_idx] = new_spans;
    }

    // Apply edge splits: create trampoline blocks with Dec instructions.
    for &(pred_idx, succ_block_id, ref vars) in &edge_splits {
        let trampoline_id = func.next_block_id();
        let trampoline_body: Vec<ArcInstr> =
            vars.iter().map(|&v| ArcInstr::RcDec { var: v }).collect();

        func.push_block(ArcBlock {
            id: trampoline_id,
            params: vec![],
            body: trampoline_body,
            terminator: ArcTerminator::Jump {
                target: succ_block_id,
                args: vec![],
            },
        });

        redirect_edges(
            &mut func.blocks[pred_idx].terminator,
            succ_block_id,
            trampoline_id,
        );
    }

    if !block_decs.is_empty() || !edge_splits.is_empty() {
        tracing::debug!(
            block_decs = block_decs.len(),
            edge_splits = edge_splits.len(),
            "edge cleanup applied"
        );
    }
}

/// Compute the predecessor list for each block (deduplicated).
///
/// Returns a vector indexed by block index, where each entry is the
/// list of distinct predecessor block indices.
pub(crate) fn compute_predecessors(func: &ArcFunction) -> Vec<Vec<usize>> {
    let num_blocks = func.blocks.len();
    let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); num_blocks];

    for (block_idx, block) in func.blocks.iter().enumerate() {
        let mut seen = FxHashSet::default();
        for succ_id in successor_block_ids(&block.terminator) {
            let succ_idx = succ_id.index();
            if succ_idx < num_blocks && seen.insert(succ_idx) {
                predecessors[succ_idx].push(block_idx);
            }
        }
    }

    predecessors
}

/// Extract successor block IDs from a terminator.
pub(crate) fn successor_block_ids(terminator: &ArcTerminator) -> Vec<ArcBlockId> {
    match terminator {
        ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {
            vec![]
        }
        ArcTerminator::Jump { target, .. } => vec![*target],
        ArcTerminator::Branch {
            then_block,
            else_block,
            ..
        } => vec![*then_block, *else_block],
        ArcTerminator::Switch { cases, default, .. } => {
            let mut targets: Vec<ArcBlockId> = cases.iter().map(|&(_, b)| b).collect();
            targets.push(*default);
            targets
        }
        ArcTerminator::Invoke { normal, unwind, .. } => vec![*normal, *unwind],
    }
}

/// Compute the global set of borrow-derived variables across all blocks.
///
/// A variable is borrow-derived if it was created by `Project` or `Let(Var)`
/// from a borrowed parameter (or transitively from another borrow-derived var)
/// in any block. These variables must not receive `RcDec` in edge cleanup
/// because they were never `RcInc`'d.
fn compute_global_borrows(
    func: &ArcFunction,
    borrowed_params: &FxHashSet<ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let mut global = FxHashSet::default();
    for block in &func.blocks {
        let block_borrows = compute_borrows(block, borrowed_params);
        global.extend(block_borrows);
    }
    global
}

/// Redirect all edges in a terminator from `old_target` to `new_target`.
fn redirect_edges(terminator: &mut ArcTerminator, old_target: ArcBlockId, new_target: ArcBlockId) {
    match terminator {
        ArcTerminator::Branch {
            then_block,
            else_block,
            ..
        } => {
            if *then_block == old_target {
                *then_block = new_target;
            }
            if *else_block == old_target {
                *else_block = new_target;
            }
        }
        ArcTerminator::Switch { cases, default, .. } => {
            for (_, target) in cases.iter_mut() {
                if *target == old_target {
                    *target = new_target;
                }
            }
            if *default == old_target {
                *default = new_target;
            }
        }
        ArcTerminator::Jump { target, .. } => {
            if *target == old_target {
                *target = new_target;
            }
        }
        ArcTerminator::Invoke { normal, unwind, .. } => {
            if *normal == old_target {
                *normal = new_target;
            }
            if *unwind == old_target {
                *unwind = new_target;
            }
        }
        ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {}
    }
}

#[cfg(test)]
mod tests {
    use ori_ir::Name;
    use ori_types::{Idx, Pool};

    use crate::ir::{
        ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
        CtorKind, LitValue,
    };
    use crate::liveness::compute_liveness;
    use crate::ownership::Ownership;
    use crate::ArcClassifier;

    use super::insert_rc_ops;

    // ── Helpers ─────────────────────────────────────────────────

    fn make_func(
        params: Vec<ArcParam>,
        return_type: Idx,
        blocks: Vec<ArcBlock>,
        var_types: Vec<Idx>,
    ) -> ArcFunction {
        let span_vecs: Vec<Vec<Option<ori_ir::Span>>> =
            blocks.iter().map(|b| vec![None; b.body.len()]).collect();
        ArcFunction {
            name: Name::from_raw(1),
            params,
            return_type,
            blocks,
            entry: ArcBlockId::new(0),
            var_types,
            spans: span_vecs,
        }
    }

    fn owned_param(var: u32, ty: Idx) -> ArcParam {
        ArcParam {
            var: ArcVarId::new(var),
            ty,
            ownership: Ownership::Owned,
        }
    }

    fn borrowed_param(var: u32, ty: Idx) -> ArcParam {
        ArcParam {
            var: ArcVarId::new(var),
            ty,
            ownership: Ownership::Borrowed,
        }
    }

    fn v(n: u32) -> ArcVarId {
        ArcVarId::new(n)
    }

    fn b(n: u32) -> ArcBlockId {
        ArcBlockId::new(n)
    }

    /// Run RC insertion on a function, returning the transformed function.
    fn run_rc_insert(mut func: ArcFunction) -> ArcFunction {
        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let liveness = compute_liveness(&func, &classifier);
        insert_rc_ops(&mut func, &classifier, &liveness);
        func
    }

    /// Count occurrences of `RcInc` for a specific var in a block.
    fn count_inc(func: &ArcFunction, block_idx: usize, var: ArcVarId) -> usize {
        func.blocks[block_idx]
            .body
            .iter()
            .filter(|i| matches!(i, ArcInstr::RcInc { var: v, .. } if *v == var))
            .count()
    }

    /// Count occurrences of `RcDec` for a specific var in a block.
    fn count_dec(func: &ArcFunction, block_idx: usize, var: ArcVarId) -> usize {
        func.blocks[block_idx]
            .body
            .iter()
            .filter(|i| matches!(i, ArcInstr::RcDec { var: v } if *v == var))
            .count()
    }

    /// Count total RC ops (Inc + Dec) in a block.
    fn count_rc_ops(func: &ArcFunction, block_idx: usize) -> usize {
        func.blocks[block_idx]
            .body
            .iter()
            .filter(|i| matches!(i, ArcInstr::RcInc { .. } | ArcInstr::RcDec { .. }))
            .count()
    }

    // ── Tests ───────────────────────────────────────────────────

    /// Passthrough — `fn(x: str) -> str { x }`.
    /// Ownership transfers through return, no RC ops needed.
    #[test]
    fn passthrough_no_ops() {
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR],
        );

        let result = run_rc_insert(func);

        // x is owned, used exactly once in return → no Inc, no Dec.
        assert_eq!(count_rc_ops(&result, 0), 0);
    }

    /// Dead definition — `fn() { let s = "hello"; unit }`.
    /// String is created but not used → `RcDec`.
    #[test]
    fn dead_definition_gets_dec() {
        let func = make_func(
            vec![],
            Idx::UNIT,
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
                        ty: Idx::UNIT,
                        value: ArcValue::Literal(LitValue::Unit),
                    },
                ],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::UNIT],
        );

        let result = run_rc_insert(func);

        // v0 (str) is defined but never used → Dec.
        assert_eq!(count_dec(&result, 0, v(0)), 1);
        // v1 (unit) is scalar → no RC ops.
        assert_eq!(count_inc(&result, 0, v(1)), 0);
        assert_eq!(count_dec(&result, 0, v(1)), 0);
    }

    /// Multiple uses — `fn(x: str) { g(x, x) }`.
    /// x is used twice in the same Apply → 1 `RcInc`.
    #[test]
    fn multiple_uses_get_inc() {
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::UNIT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(1),
                    ty: Idx::UNIT,
                    func: Name::from_raw(99),
                    args: vec![v(0), v(0)],
                }],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::UNIT],
        );

        let result = run_rc_insert(func);

        // x used twice in Apply → 1 Inc (second occurrence).
        assert_eq!(count_inc(&result, 0, v(0)), 1);
    }

    /// Borrowed param — `fn(@borrow x: str) -> int { len(x) }`.
    /// Borrowed parameter: zero RC ops.
    #[test]
    fn borrowed_param_no_ops() {
        let func = make_func(
            vec![borrowed_param(0, Idx::STR)],
            Idx::INT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(1),
                    ty: Idx::INT,
                    func: Name::from_raw(99),
                    args: vec![v(0)],
                }],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::INT],
        );

        let result = run_rc_insert(func);

        // Borrowed param: no Inc, no Dec.
        assert_eq!(count_rc_ops(&result, 0), 0);
    }

    /// Borrowed param returned — `fn(@borrow x: str) -> str { x }`.
    /// Borrowed param being returned needs Inc (transfer ownership to caller).
    #[test]
    fn borrowed_returned_gets_inc() {
        let func = make_func(
            vec![borrowed_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR],
        );

        let result = run_rc_insert(func);

        // Borrowed x returned → Inc (transfer to caller).
        assert_eq!(count_inc(&result, 0, v(0)), 1);
        // No Dec (borrowed params are never Dec'd).
        assert_eq!(count_dec(&result, 0, v(0)), 0);
    }

    /// Project from borrowed — `fn(@borrow p: T) { use p.field }`.
    /// Projected field from borrowed param: no RC ops (borrows set).
    #[test]
    fn project_from_borrowed_no_ops() {
        // Project an int field from a borrowed param → scalar, no RC ops.
        let func = make_func(
            vec![borrowed_param(0, Idx::STR)],
            Idx::INT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Project {
                        dst: v(1),
                        ty: Idx::INT,
                        value: v(0),
                        field: 0,
                    },
                    ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::INT,
                        value: ArcValue::PrimOp {
                            op: crate::ir::PrimOp::Binary(ori_ir::BinaryOp::Add),
                            args: vec![v(1), v(1)],
                        },
                    },
                ],
                terminator: ArcTerminator::Return { value: v(2) },
            }],
            vec![Idx::STR, Idx::INT, Idx::INT],
        );

        let result = run_rc_insert(func);

        // v1 is int (scalar) → no RC. v0 is borrowed → no RC. Zero ops.
        assert_eq!(count_rc_ops(&result, 0), 0);
    }

    /// Project from borrowed stored — `fn(@borrow p: T) { Construct(p.field) }`.
    /// Projected RC field from borrowed, stored in Construct → Inc.
    #[test]
    fn project_from_borrowed_stored() {
        // Project str field from borrowed → store in Construct → owned position.
        let func = make_func(
            vec![borrowed_param(0, Idx::STR)],
            Idx::UNIT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Project {
                        dst: v(1),
                        ty: Idx::STR,
                        value: v(0),
                        field: 0,
                    },
                    ArcInstr::Construct {
                        dst: v(2),
                        ty: Idx::UNIT,
                        ctor: CtorKind::Tuple,
                        args: vec![v(1)],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(2) },
            }],
            vec![Idx::STR, Idx::STR, Idx::UNIT],
        );

        let result = run_rc_insert(func);

        // field (v1) is borrowed-derived but stored in Construct → Inc.
        assert_eq!(count_inc(&result, 0, v(1)), 1);
        // p (v0) is borrowed → no RC ops.
        assert_eq!(count_inc(&result, 0, v(0)), 0);
        assert_eq!(count_dec(&result, 0, v(0)), 0);
    }

    /// Unused owned param — `fn(x: str, y: str) -> str { x }`.
    /// y is never used → Dec at entry.
    #[test]
    fn unused_owned_param_dec() {
        let func = make_func(
            vec![owned_param(0, Idx::STR), owned_param(1, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR, Idx::STR],
        );

        let result = run_rc_insert(func);

        // x (v0) used in return → no Inc/Dec (single use, ownership transfers).
        assert_eq!(count_inc(&result, 0, v(0)), 0);
        assert_eq!(count_dec(&result, 0, v(0)), 0);
        // y (v1) never used → Dec.
        assert_eq!(count_dec(&result, 0, v(1)), 1);
    }

    /// Diamond branch — if/else both using a str var.
    /// Each branch path should have correct RC balance.
    #[test]
    fn diamond_branch() {
        // Block 0: branch on v1 (bool) → b1 or b2
        // Block 1: let v2 = apply f(v0); jump to b3 with v2
        // Block 2: jump to b3 with v0
        // Block 3: param v3: str; return v3
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
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
                    body: vec![ArcInstr::Apply {
                        dst: v(2),
                        ty: Idx::STR,
                        func: Name::from_raw(99),
                        args: vec![v(0)],
                    }],
                    terminator: ArcTerminator::Jump {
                        target: b(3),
                        args: vec![v(2)],
                    },
                },
                ArcBlock {
                    id: b(2),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Jump {
                        target: b(3),
                        args: vec![v(0)],
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

        let result = run_rc_insert(func);

        // v0 in block 0: just live, no inc/dec.
        assert_eq!(count_rc_ops(&result, 0), 0);
        // Block 3: v3 returned, single use.
        assert_eq!(count_rc_ops(&result, 3), 0);
    }

    /// Loop variable — variable live across loop iterations.
    #[test]
    fn loop_variable() {
        // Block 0: jump to b1 with v0 (str param)
        // Block 1: param v1: str; branch on v2 (bool) → b2 or b3
        // Block 2: let v3 = apply f(v1); jump to b1 with v3
        // Block 3: return v1
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
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

        let result = run_rc_insert(func);

        // No Inc for v1 in b2 — it's a single use (consumed by Apply).
        assert_eq!(count_inc(&result, 2, v(1)), 0);
        // Block 3: v1 used in Return. Single use, transfers ownership.
        assert_eq!(count_rc_ops(&result, 3), 0);
    }

    /// Unused block param — block param never used in block body.
    #[test]
    fn unused_block_param_dec() {
        // Block 0: jump to b1 with v0 (str)
        // Block 1: param v1: str; let v2 = "other"; return v2
        //
        // v1 is a block param but never used → Dec.
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![
                ArcBlock {
                    id: b(0),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Jump {
                        target: b(1),
                        args: vec![v(0)],
                    },
                },
                ArcBlock {
                    id: b(1),
                    params: vec![(v(1), Idx::STR)],
                    body: vec![ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::STR,
                        value: ArcValue::Literal(LitValue::String(Name::from_raw(100))),
                    }],
                    terminator: ArcTerminator::Return { value: v(2) },
                },
            ],
            vec![Idx::STR, Idx::STR, Idx::STR],
        );

        let result = run_rc_insert(func);

        // v1 (block param) unused in b1 → Dec.
        assert_eq!(count_dec(&result, 1, v(1)), 1);
        // v2 used in return → no extra ops.
        assert_eq!(count_dec(&result, 1, v(2)), 0);
    }

    /// All-int function — zero RC ops.
    #[test]
    fn scalars_untouched() {
        let func = make_func(
            vec![owned_param(0, Idx::INT), owned_param(1, Idx::INT)],
            Idx::INT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::INT,
                    value: ArcValue::PrimOp {
                        op: crate::ir::PrimOp::Binary(ori_ir::BinaryOp::Add),
                        args: vec![v(0), v(1)],
                    },
                }],
                terminator: ArcTerminator::Return { value: v(2) },
            }],
            vec![Idx::INT, Idx::INT, Idx::INT],
        );

        let result = run_rc_insert(func);

        assert_eq!(count_rc_ops(&result, 0), 0);
    }

    /// Early exit cleanup: one branch returns early, the other continues.
    /// The early-exit branch must Dec all live RC'd variables that it
    /// doesn't return. This demonstrates that the liveness-based RC
    /// insertion naturally handles break/continue/early-return patterns
    /// (Section 07.5).
    ///
    /// ```text
    /// block_0:
    ///   %s1 = construct str  // live in both branches
    ///   %s2 = construct str  // live in both branches
    ///   %cond = ...
    ///   branch %cond → b1, b2
    ///
    /// block_1 (early exit):  // returns s1, must Dec s2
    ///   return s1
    ///
    /// block_2 (continues):   // uses both, consumes both
    ///   %r = apply f(s1, s2)
    ///   return r
    /// ```
    #[test]
    fn early_exit_cleanup() {
        let func = make_func(
            vec![],
            Idx::STR,
            vec![
                ArcBlock {
                    id: b(0),
                    params: vec![],
                    body: vec![
                        ArcInstr::Construct {
                            dst: v(0),
                            ty: Idx::STR,
                            ctor: CtorKind::Struct(Name::from_raw(50)),
                            args: vec![],
                        },
                        ArcInstr::Construct {
                            dst: v(1),
                            ty: Idx::STR,
                            ctor: CtorKind::Struct(Name::from_raw(51)),
                            args: vec![],
                        },
                        ArcInstr::Let {
                            dst: v(2),
                            ty: Idx::BOOL,
                            value: ArcValue::Literal(LitValue::Bool(true)),
                        },
                    ],
                    terminator: ArcTerminator::Branch {
                        cond: v(2),
                        then_block: b(1),
                        else_block: b(2),
                    },
                },
                // Early exit: returns s1, does NOT use s2.
                ArcBlock {
                    id: b(1),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Return { value: v(0) },
                },
                // Normal path: uses both s1 and s2.
                ArcBlock {
                    id: b(2),
                    params: vec![],
                    body: vec![ArcInstr::Apply {
                        dst: v(3),
                        ty: Idx::STR,
                        func: Name::from_raw(99),
                        args: vec![v(0), v(1)],
                    }],
                    terminator: ArcTerminator::Return { value: v(3) },
                },
            ],
            vec![Idx::STR, Idx::STR, Idx::BOOL, Idx::STR],
        );

        let result = run_rc_insert(func);

        // Block 1 (early exit): s2 (v1) is live but not used → must be Dec'd.
        assert_eq!(count_dec(&result, 1, v(1)), 1);
        // Block 1: s1 (v0) is returned → no Dec.
        assert_eq!(count_dec(&result, 1, v(0)), 0);

        // Block 2 (normal): both s1 and s2 consumed by Apply → no extra Dec.
        assert_eq!(count_dec(&result, 2, v(0)), 0);
        assert_eq!(count_dec(&result, 2, v(1)), 0);
    }

    /// Early exit in a loop (break pattern): loop body uses s1, but the
    /// break branch exits while s1 is still live. Must Dec s1 on exit.
    ///
    /// ```text
    /// block_0: let s1 = "hello"; jump to b1
    /// block_1: branch cond → b2 (break), b3 (body)
    /// block_2 (break exit): return unit  // must Dec s1
    /// block_3 (body): apply f(s1); jump to b1
    /// ```
    #[test]
    fn break_from_loop_cleanup() {
        let func = make_func(
            vec![],
            Idx::UNIT,
            vec![
                ArcBlock {
                    id: b(0),
                    params: vec![],
                    body: vec![
                        ArcInstr::Construct {
                            dst: v(0),
                            ty: Idx::STR,
                            ctor: CtorKind::Struct(Name::from_raw(50)),
                            args: vec![],
                        },
                        ArcInstr::Let {
                            dst: v(1),
                            ty: Idx::BOOL,
                            value: ArcValue::Literal(LitValue::Bool(true)),
                        },
                    ],
                    terminator: ArcTerminator::Jump {
                        target: b(1),
                        args: vec![],
                    },
                },
                // Loop header: branch to break or body.
                ArcBlock {
                    id: b(1),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Branch {
                        cond: v(1),
                        then_block: b(2),
                        else_block: b(3),
                    },
                },
                // Break exit: return unit. s1 is live here → must Dec.
                ArcBlock {
                    id: b(2),
                    params: vec![],
                    body: vec![ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::UNIT,
                        value: ArcValue::Literal(LitValue::Unit),
                    }],
                    terminator: ArcTerminator::Return { value: v(2) },
                },
                // Loop body: uses s1, then loops back.
                ArcBlock {
                    id: b(3),
                    params: vec![],
                    body: vec![ArcInstr::Apply {
                        dst: v(3),
                        ty: Idx::UNIT,
                        func: Name::from_raw(99),
                        args: vec![v(0)],
                    }],
                    terminator: ArcTerminator::Jump {
                        target: b(1),
                        args: vec![],
                    },
                },
            ],
            vec![Idx::STR, Idx::BOOL, Idx::UNIT, Idx::UNIT],
        );

        let result = run_rc_insert(func);

        // Block 2 (break): s1 (v0) is live but not used → Dec.
        assert_eq!(count_dec(&result, 2, v(0)), 1);
        // Block 3 (body): s1 used in Apply. It's also live out (loops back
        // to b1 where s1 is live), so it gets an Inc before Apply.
        assert_eq!(count_inc(&result, 3, v(0)), 1);
    }

    /// Duplicate var in single instruction — `Apply { args: [x, x] }`.
    /// Should produce exactly 1 Inc.
    #[test]
    fn duplicate_in_single_instr() {
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(1),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![v(0), v(0)],
                }],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::STR],
        );

        let result = run_rc_insert(func);

        // x appears twice → 1 Inc.
        assert_eq!(count_inc(&result, 0, v(0)), 1);
        // No Dec for x (it's used).
        assert_eq!(count_dec(&result, 0, v(0)), 0);
    }

    /// Switch with asymmetric edge cleanup: three branches where only
    /// some paths use each variable.
    ///
    /// ```text
    /// block_0: v0(str), v1(str), v2(int); switch v2 → b1(0), b2(1), b3(default)
    /// block_1: return v0         // must Dec v1
    /// block_2: return v1         // must Dec v0
    /// block_3: apply f(v0, v1)   // uses both
    /// ```
    #[test]
    fn switch_asymmetric_cleanup() {
        let func = make_func(
            vec![],
            Idx::STR,
            vec![
                ArcBlock {
                    id: b(0),
                    params: vec![],
                    body: vec![
                        ArcInstr::Construct {
                            dst: v(0),
                            ty: Idx::STR,
                            ctor: CtorKind::Struct(Name::from_raw(50)),
                            args: vec![],
                        },
                        ArcInstr::Construct {
                            dst: v(1),
                            ty: Idx::STR,
                            ctor: CtorKind::Struct(Name::from_raw(51)),
                            args: vec![],
                        },
                        ArcInstr::Let {
                            dst: v(2),
                            ty: Idx::INT,
                            value: ArcValue::Literal(LitValue::Int(0)),
                        },
                    ],
                    terminator: ArcTerminator::Switch {
                        scrutinee: v(2),
                        cases: vec![(0, b(1)), (1, b(2))],
                        default: b(3),
                    },
                },
                // Case 0: returns v0, doesn't use v1.
                ArcBlock {
                    id: b(1),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Return { value: v(0) },
                },
                // Case 1: returns v1, doesn't use v0.
                ArcBlock {
                    id: b(2),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Return { value: v(1) },
                },
                // Default: uses both.
                ArcBlock {
                    id: b(3),
                    params: vec![],
                    body: vec![ArcInstr::Apply {
                        dst: v(3),
                        ty: Idx::STR,
                        func: Name::from_raw(99),
                        args: vec![v(0), v(1)],
                    }],
                    terminator: ArcTerminator::Return { value: v(3) },
                },
            ],
            vec![Idx::STR, Idx::STR, Idx::INT, Idx::STR],
        );

        let result = run_rc_insert(func);

        // Block 1: v1 not used → Dec v1 (edge cleanup).
        assert_eq!(count_dec(&result, 1, v(1)), 1);
        assert_eq!(count_dec(&result, 1, v(0)), 0);

        // Block 2: v0 not used → Dec v0 (edge cleanup).
        assert_eq!(count_dec(&result, 2, v(0)), 1);
        assert_eq!(count_dec(&result, 2, v(1)), 0);

        // Block 3: both consumed by Apply → no extra Dec.
        assert_eq!(count_dec(&result, 3, v(0)), 0);
        assert_eq!(count_dec(&result, 3, v(1)), 0);
    }

    /// Multiple RC'd vars in edge cleanup: early exit must Dec ALL
    /// stranded variables, not just one.
    ///
    /// ```text
    /// block_0: v0(str), v1(str), v2(str); branch → b1, b2
    /// block_1: return unit       // must Dec v0, v1, v2
    /// block_2: apply f(v0, v1, v2)
    /// ```
    #[test]
    fn edge_cleanup_multiple_vars() {
        let func = make_func(
            vec![],
            Idx::UNIT,
            vec![
                ArcBlock {
                    id: b(0),
                    params: vec![],
                    body: vec![
                        ArcInstr::Construct {
                            dst: v(0),
                            ty: Idx::STR,
                            ctor: CtorKind::Struct(Name::from_raw(50)),
                            args: vec![],
                        },
                        ArcInstr::Construct {
                            dst: v(1),
                            ty: Idx::STR,
                            ctor: CtorKind::Struct(Name::from_raw(51)),
                            args: vec![],
                        },
                        ArcInstr::Construct {
                            dst: v(2),
                            ty: Idx::STR,
                            ctor: CtorKind::Struct(Name::from_raw(52)),
                            args: vec![],
                        },
                        ArcInstr::Let {
                            dst: v(3),
                            ty: Idx::BOOL,
                            value: ArcValue::Literal(LitValue::Bool(true)),
                        },
                    ],
                    terminator: ArcTerminator::Branch {
                        cond: v(3),
                        then_block: b(1),
                        else_block: b(2),
                    },
                },
                // Early exit: no RC'd vars used.
                ArcBlock {
                    id: b(1),
                    params: vec![],
                    body: vec![ArcInstr::Let {
                        dst: v(4),
                        ty: Idx::UNIT,
                        value: ArcValue::Literal(LitValue::Unit),
                    }],
                    terminator: ArcTerminator::Return { value: v(4) },
                },
                // Normal: uses all three.
                ArcBlock {
                    id: b(2),
                    params: vec![],
                    body: vec![ArcInstr::Apply {
                        dst: v(5),
                        ty: Idx::UNIT,
                        func: Name::from_raw(99),
                        args: vec![v(0), v(1), v(2)],
                    }],
                    terminator: ArcTerminator::Return { value: v(5) },
                },
            ],
            vec![
                Idx::STR,
                Idx::STR,
                Idx::STR,
                Idx::BOOL,
                Idx::UNIT,
                Idx::UNIT,
            ],
        );

        let result = run_rc_insert(func);

        // Block 1: all three str vars need Dec.
        assert_eq!(count_dec(&result, 1, v(0)), 1);
        assert_eq!(count_dec(&result, 1, v(1)), 1);
        assert_eq!(count_dec(&result, 1, v(2)), 1);
    }

    /// Edge cleanup with multi-predecessor same gap: merge block b3
    /// reached by two branches (b1 and b2) that both have the same
    /// stranded variable v1. Both b1 and b2 also branch to blocks
    /// that DO use v1, keeping it in their `live_out`.
    ///
    /// ```text
    /// block_0: v0(str), v1(str), v2(bool), v3(bool)
    ///          branch v2 → b1, b2
    /// block_1: branch v3 → b3, b4
    /// block_2: branch v3 → b3, b5
    /// block_3: return v0           // v1 stranded from BOTH b1 and b2
    /// block_4: return v1           // uses v1 (keeps it live in b1)
    /// block_5: return v1           // uses v1 (keeps it live in b2)
    /// ```
    ///
    /// Here v1 is in `live_out[b1]` and `live_out[b2]` because b4/b5 need
    /// it, but b3 only uses v0. So gap(b1→b3) = gap(b2→b3) = {v1}.
    /// Since b3 has two predecessors with the same gap, edge cleanup
    /// inserts Dec v1 at b3's start.
    #[test]
    fn multi_pred_same_gap() {
        let func = make_func(
            vec![],
            Idx::STR,
            vec![
                ArcBlock {
                    id: b(0),
                    params: vec![],
                    body: vec![
                        ArcInstr::Construct {
                            dst: v(0),
                            ty: Idx::STR,
                            ctor: CtorKind::Struct(Name::from_raw(50)),
                            args: vec![],
                        },
                        ArcInstr::Construct {
                            dst: v(1),
                            ty: Idx::STR,
                            ctor: CtorKind::Struct(Name::from_raw(51)),
                            args: vec![],
                        },
                        ArcInstr::Let {
                            dst: v(2),
                            ty: Idx::BOOL,
                            value: ArcValue::Literal(LitValue::Bool(true)),
                        },
                        ArcInstr::Let {
                            dst: v(3),
                            ty: Idx::BOOL,
                            value: ArcValue::Literal(LitValue::Bool(false)),
                        },
                    ],
                    terminator: ArcTerminator::Branch {
                        cond: v(2),
                        then_block: b(1),
                        else_block: b(2),
                    },
                },
                // b1: branches to b3 (uses v0, not v1) and b4 (uses v1)
                ArcBlock {
                    id: b(1),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Branch {
                        cond: v(3),
                        then_block: b(3),
                        else_block: b(4),
                    },
                },
                // b2: branches to b3 (uses v0, not v1) and b5 (uses v1)
                ArcBlock {
                    id: b(2),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Branch {
                        cond: v(3),
                        then_block: b(3),
                        else_block: b(5),
                    },
                },
                // b3: uses v0, v1 is stranded (from both b1 and b2)
                ArcBlock {
                    id: b(3),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Return { value: v(0) },
                },
                // b4: uses v1
                ArcBlock {
                    id: b(4),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Return { value: v(1) },
                },
                // b5: uses v1
                ArcBlock {
                    id: b(5),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Return { value: v(1) },
                },
            ],
            vec![Idx::STR, Idx::STR, Idx::BOOL, Idx::BOOL],
        );

        let result = run_rc_insert(func);

        // b3 has two predecessors (b1, b2). Both have gap = {v1}.
        // Since gaps are identical, edge cleanup inserts Dec v1 at b3's start.
        assert_eq!(count_dec(&result, 3, v(1)), 1);
        // v0 is returned in b3 → no Dec for v0.
        assert_eq!(count_dec(&result, 3, v(0)), 0);

        // b4: v0 is stranded (b1's live_out has v0, b4 only uses v1).
        assert_eq!(count_dec(&result, 4, v(0)), 1);
        // b5: v0 is stranded similarly.
        assert_eq!(count_dec(&result, 5, v(0)), 1);
    }

    /// Invoke: live str variable gets `RcDec` in unwind block.
    ///
    /// When an Invoke's unwind block is reached, all RC'd variables that
    /// were live at the invoke point (but NOT the invoke's dst) must be
    /// Dec'd for cleanup.
    ///
    /// ```text
    /// block_0:
    ///   %s = construct str "hello"
    ///   invoke f(%s) → dst=%r, normal=b1, unwind=b2
    ///
    /// block_1 (normal):
    ///   return %r
    ///
    /// block_2 (unwind):
    ///   resume   // edge cleanup must insert RcDec(%s) here
    /// ```
    #[test]
    fn invoke_unwind_cleanup() {
        let func = make_func(
            vec![],
            Idx::STR,
            vec![
                ArcBlock {
                    id: b(0),
                    params: vec![],
                    body: vec![ArcInstr::Construct {
                        dst: v(0),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(50)),
                        args: vec![],
                    }],
                    terminator: ArcTerminator::Invoke {
                        dst: v(1),
                        ty: Idx::STR,
                        func: Name::from_raw(99),
                        args: vec![v(0)],
                        normal: b(1),
                        unwind: b(2),
                    },
                },
                // Normal continuation: return the invoke result.
                ArcBlock {
                    id: b(1),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Return { value: v(1) },
                },
                // Unwind block: initially just Resume.
                ArcBlock {
                    id: b(2),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Resume,
                },
            ],
            vec![Idx::STR, Idx::STR],
        );

        let result = run_rc_insert(func);

        // v0 (str) is consumed by the Invoke call (it's an arg),
        // so it's NOT stranded — no RcDec needed in unwind.
        // But if v0 had survived (e.g., used after invoke in normal block),
        // it would need cleanup. Let's verify no spurious Decs.

        // v1 (invoke dst) should NOT be Dec'd in unwind — it's never
        // produced on the unwind path.
        // Check that the unwind block's body is handled properly.
        let unwind_idx = 2;
        assert_eq!(
            count_dec(&result, unwind_idx, v(1)),
            0,
            "invoke dst must NOT be Dec'd in unwind block"
        );
    }

    /// Invoke with multiple live variables: ALL stranded vars get cleanup.
    ///
    /// ```text
    /// block_0:
    ///   %s1 = construct str
    ///   %s2 = construct str
    ///   invoke f() → dst=%r, normal=b1, unwind=b2
    ///
    /// block_1 (normal):
    ///   apply g(%s1, %s2)
    ///   return %r
    ///
    /// block_2 (unwind):
    ///   resume   // must insert RcDec(%s1), RcDec(%s2)
    /// ```
    #[test]
    fn invoke_unwind_cleanup_multiple_vars() {
        let func = make_func(
            vec![],
            Idx::STR,
            vec![
                ArcBlock {
                    id: b(0),
                    params: vec![],
                    body: vec![
                        ArcInstr::Construct {
                            dst: v(0),
                            ty: Idx::STR,
                            ctor: CtorKind::Struct(Name::from_raw(50)),
                            args: vec![],
                        },
                        ArcInstr::Construct {
                            dst: v(1),
                            ty: Idx::STR,
                            ctor: CtorKind::Struct(Name::from_raw(51)),
                            args: vec![],
                        },
                    ],
                    // Invoke with NO args (doesn't consume s1 or s2).
                    terminator: ArcTerminator::Invoke {
                        dst: v(2),
                        ty: Idx::STR,
                        func: Name::from_raw(99),
                        args: vec![],
                        normal: b(1),
                        unwind: b(2),
                    },
                },
                // Normal: uses s1, s2, and the invoke result.
                ArcBlock {
                    id: b(1),
                    params: vec![],
                    body: vec![ArcInstr::Apply {
                        dst: v(3),
                        ty: Idx::UNIT,
                        func: Name::from_raw(98),
                        args: vec![v(0), v(1)],
                    }],
                    terminator: ArcTerminator::Return { value: v(2) },
                },
                // Unwind: just Resume.
                ArcBlock {
                    id: b(2),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Resume,
                },
            ],
            vec![Idx::STR, Idx::STR, Idx::STR, Idx::UNIT],
        );

        let result = run_rc_insert(func);

        // The unwind block may have been replaced by an edge-split trampoline.
        // Find the block that predecessors b0 and has Resume terminator.
        // The edge cleanup may create a trampoline block that Decs and jumps
        // to the unwind block, OR it may insert Decs directly in the unwind
        // block if it's single-predecessor.
        //
        // With single predecessor (b0 is the only pred of unwind b2),
        // Decs are inserted at the start of b2.
        let unwind_idx = 2;
        assert_eq!(
            count_dec(&result, unwind_idx, v(0)),
            1,
            "s1 must be Dec'd in unwind block"
        );
        assert_eq!(
            count_dec(&result, unwind_idx, v(1)),
            1,
            "s2 must be Dec'd in unwind block"
        );
        // Invoke dst must NOT be Dec'd in unwind.
        assert_eq!(
            count_dec(&result, unwind_idx, v(2)),
            0,
            "invoke dst must NOT be Dec'd in unwind block"
        );
    }

    /// Invoke where dst is unused in normal block → gets Dec'd there.
    #[test]
    fn invoke_unused_dst_gets_dec_in_normal() {
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
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
                        args: vec![],
                        normal: b(1),
                        unwind: b(2),
                    },
                },
                // Normal: returns v0 (param), ignores v1 (invoke dst).
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
                    terminator: ArcTerminator::Resume,
                },
            ],
            vec![Idx::STR, Idx::STR],
        );

        let result = run_rc_insert(func);

        // v1 (invoke dst) is unused in normal block → Dec it there.
        let normal_idx = 1;
        assert_eq!(
            count_dec(&result, normal_idx, v(1)),
            1,
            "unused invoke dst should be Dec'd in normal block"
        );
    }

    /// No edge cleanup needed when all paths use the same variables.
    /// Diamond where both branches consume all live vars.
    #[test]
    fn no_edge_cleanup_symmetric() {
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
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
                // Both branches use v0.
                ArcBlock {
                    id: b(1),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Return { value: v(0) },
                },
                ArcBlock {
                    id: b(2),
                    params: vec![],
                    body: vec![ArcInstr::Apply {
                        dst: v(2),
                        ty: Idx::STR,
                        func: Name::from_raw(99),
                        args: vec![v(0)],
                    }],
                    terminator: ArcTerminator::Return { value: v(2) },
                },
            ],
            vec![Idx::STR, Idx::BOOL, Idx::STR],
        );

        let result = run_rc_insert(func);

        // No edge cleanup needed — v0 is used in both branches.
        assert_eq!(count_dec(&result, 1, v(0)), 0);
        assert_eq!(count_dec(&result, 2, v(0)), 0);
    }
}
