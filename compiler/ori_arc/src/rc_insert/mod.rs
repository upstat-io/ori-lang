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

use rustc_hash::{FxHashMap, FxHashSet};

use crate::graph::compute_predecessors;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership, RcStrategy,
};
use crate::liveness::BlockLiveness;
use crate::ownership::{DerivedOwnership, Ownership};
use crate::ArcClassification;
use ori_types::Pool;

/// Shared context for RC insertion within a single block.
///
/// Groups the parameters that would otherwise be threaded through every
/// helper function, keeping function signatures manageable.
struct RcContext<'a> {
    func: &'a ArcFunction,
    classifier: &'a dyn ArcClassification,
    /// Type pool for computing [`RcStrategy`] from `ValueRepr` + type tag.
    /// `None` in test-only mode (uses conservative `HeapPointer` fallback).
    pool: Option<&'a Pool>,
    /// Function parameters annotated as `Borrowed` — completely skip RC.
    borrowed_params: &'a FxHashSet<ArcVarId>,
    /// Variables derived from borrowed params — skip RC except at owned positions.
    borrows: &'a FxHashSet<ArcVarId>,
    /// Annotated signatures for closure capture analysis (Step 2.4).
    /// When `Some`, `PartialApply` captures at borrowed callee positions
    /// can skip `RcInc` for borrowed-derived vars (if the closure doesn't escape).
    sigs: Option<&'a FxHashMap<ori_ir::Name, crate::ownership::AnnotatedSig>>,
    /// Live-out set for the current block — used for closure escape checks.
    /// If a `PartialApply` dst is in `block_live_out`, the closure escapes
    /// the block and borrowed captures must be Inc'd.
    block_live_out: Option<&'a FxHashSet<ArcVarId>>,
}

/// Compute the [`RcStrategy`] for a variable in the current context.
///
/// Uses the pre-computed `var_reprs` (from Section 01.1) and the Pool to
/// distinguish fine-grained strategies (e.g., `Aggregate` → `InlineEnum`
/// vs `AggregateFields`, `FatValue` → `Closure` vs `FatPointer`).
///
/// Falls back to `HeapPointer` when either `var_reprs` or Pool is unavailable
/// (test-only `insert_rc_ops` path without Pool).
fn rc_strategy(ctx: &RcContext<'_>, var: ArcVarId) -> RcStrategy {
    let Some(repr) = ctx.func.var_repr(var) else {
        return RcStrategy::HeapPointer;
    };
    let Some(pool) = ctx.pool else {
        return RcStrategy::HeapPointer;
    };
    RcStrategy::from_var(repr, pool, ctx.func.var_type(var))
}

/// Compute [`RcStrategy`] for a variable given direct Pool access.
///
/// Used by edge cleanup and invoke cleanup which operate outside `RcContext`.
/// Falls back to `HeapPointer` when Pool is unavailable (test-only path)
/// or `var_reprs` hasn't been computed.
fn rc_strategy_direct(func: &ArcFunction, pool: Option<&Pool>, var: ArcVarId) -> RcStrategy {
    let Some(repr) = func.var_repr(var) else {
        return RcStrategy::HeapPointer;
    };
    let Some(pool) = pool else {
        return RcStrategy::HeapPointer;
    };
    RcStrategy::from_var(repr, pool, func.var_type(var))
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
#[cfg(test)]
pub(crate) fn insert_rc_ops(
    func: &mut ArcFunction,
    classifier: &dyn ArcClassification,
    liveness: &BlockLiveness,
) {
    // Precondition: RC insertion should run on fresh IR with no existing RC ops.
    debug_assert!(
        !func
            .blocks
            .iter()
            .flat_map(|b| b.body.iter())
            .any(|i| matches!(i, ArcInstr::RcInc { .. } | ArcInstr::RcDec { .. })),
        "insert_rc_ops: IR already contains RcInc/RcDec — pipeline ordering error"
    );

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
    let invoke_defs = crate::graph::collect_invoke_defs(func);

    // Collect per-block borrow sets for reuse by insert_edge_cleanup,
    // avoiding the redundant recomputation that compute_global_borrows
    // would perform.
    let mut per_block_borrows: Vec<FxHashSet<ArcVarId>> = Vec::with_capacity(num_blocks);

    for block_idx in 0..num_blocks {
        let borrows = compute_borrows(&func.blocks[block_idx], &borrowed_params);
        per_block_borrows.push(borrows);

        let (new_body, new_spans) = {
            let ctx = RcContext {
                func,
                classifier,
                pool: None, // test-only path — no Pool available
                borrowed_params: &borrowed_params,
                borrows: &per_block_borrows[block_idx],
                sigs: None,
                block_live_out: None,
            };
            process_block_rc(
                &ctx,
                block_idx,
                &liveness.live_out[block_idx],
                &invoke_defs,
                block_idx == entry_idx,
            )
        };

        func.blocks[block_idx].body = new_body;
        func.spans[block_idx] = new_spans;
    }

    // Step 5: Edge cleanup
    //
    // After per-block RC insertion, handle "stranded" variables that are
    // live at a predecessor's exit but not needed by a successor.
    // See `insert_edge_cleanup` for details.
    //
    // Build global borrow set from pre-collected per-block sets (avoids
    // redundant recomputation via compute_global_borrows).
    let global_borrows: FxHashSet<ArcVarId> = per_block_borrows
        .into_iter()
        .flat_map(FxHashSet::into_iter)
        .collect();
    insert_edge_cleanup(
        func,
        classifier,
        liveness,
        &borrowed_params,
        &global_borrows,
        None, // test-only path — no Pool
    );
}

/// Insert `RcInc`/`RcDec` operations using global [`DerivedOwnership`] analysis.
///
/// Enhanced version of [`insert_rc_ops`] that uses the whole-function
/// `DerivedOwnership` vector (from [`infer_derived_ownership`](crate::borrow::infer_derived_ownership))
/// instead of per-block `compute_borrows`. This captures cross-block borrow
/// propagation that the per-block approach misses.
///
/// When a variable derived from a borrowed parameter flows across a block
/// boundary (e.g., defined in B0 but used in B1), the per-block approach
/// loses track and treats it as owned in B1 — potentially omitting the
/// `RcInc` needed at owned positions. The `DerivedOwnership` vector has
/// global knowledge, ensuring correct RC ops in all blocks.
///
/// With `sigs`, also performs closure capture analysis (Step 2.4):
/// `PartialApply` captures of borrowed-derived vars at `Borrowed` callee
/// positions skip `RcInc` when the closure doesn't escape the block.
#[expect(clippy::implicit_hasher, reason = "FxHashMap is the canonical hasher")]
pub fn insert_rc_ops_with_ownership(
    func: &mut ArcFunction,
    classifier: &dyn ArcClassification,
    liveness: &BlockLiveness,
    ownership: &[DerivedOwnership],
    sigs: &FxHashMap<ori_ir::Name, crate::ownership::AnnotatedSig>,
    pool: &Pool,
) {
    debug_assert!(
        !func
            .blocks
            .iter()
            .flat_map(|b| b.body.iter())
            .any(|i| matches!(i, ArcInstr::RcInc { .. } | ArcInstr::RcDec { .. })),
        "insert_rc_ops_with_ownership: IR already contains RcInc/RcDec"
    );

    tracing::debug!(
        function = func.name.raw(),
        "inserting RC operations (ownership-enhanced)"
    );

    let borrowed_params: FxHashSet<ArcVarId> = func
        .params
        .iter()
        .filter(|p| p.ownership == Ownership::Borrowed)
        .map(|p| p.var)
        .collect();

    // Global borrow set from DerivedOwnership — covers cross-block propagation.
    let global_borrows: FxHashSet<ArcVarId> = ownership
        .iter()
        .enumerate()
        .filter(|(_, o)| matches!(o, DerivedOwnership::BorrowedFrom(_)))
        .map(|(i, _)| {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "ARC IR var counts fit in u32"
            )]
            ArcVarId::new(i as u32)
        })
        .collect();

    let entry_idx = func.entry.index();
    let num_blocks = func.blocks.len();
    let invoke_defs = crate::graph::collect_invoke_defs(func);

    for block_idx in 0..num_blocks {
        let (new_body, new_spans) = {
            let ctx = RcContext {
                func,
                classifier,
                pool: Some(pool),
                borrowed_params: &borrowed_params,
                borrows: &global_borrows,
                sigs: Some(sigs),
                block_live_out: Some(&liveness.live_out[block_idx]),
            };
            process_block_rc(
                &ctx,
                block_idx,
                &liveness.live_out[block_idx],
                &invoke_defs,
                block_idx == entry_idx,
            )
        };

        func.blocks[block_idx].body = new_body;
        func.spans[block_idx] = new_spans;
    }

    insert_edge_cleanup(
        func,
        classifier,
        liveness,
        &borrowed_params,
        &global_borrows,
        Some(pool),
    );
}

/// Process a single block for RC insertion, returning the new body and spans.
///
/// Shared inner implementation for [`insert_rc_ops`] and
/// [`insert_rc_ops_with_ownership`]. Performs the backward walk over
/// instructions, inserting `RcInc`/`RcDec` based on liveness and borrowing.
fn process_block_rc(
    ctx: &RcContext<'_>,
    block_idx: usize,
    live_out: &FxHashSet<ArcVarId>,
    invoke_defs: &FxHashMap<ArcBlockId, Vec<ArcVarId>>,
    is_entry: bool,
) -> (Vec<ArcInstr>, Vec<Option<ori_ir::Span>>) {
    let mut live = live_out.clone();
    let mut new_body: Vec<ArcInstr> = Vec::new();
    let mut new_spans: Vec<Option<ori_ir::Span>> = Vec::new();

    let block = &ctx.func.blocks[block_idx];
    let old_spans = &ctx.func.spans[block_idx];

    // Step 1: Process terminator uses
    process_terminator_uses(
        &block.terminator,
        &mut live,
        &mut new_body,
        &mut new_spans,
        ctx,
    );

    // Step 2: Backward body pass
    for (instr_idx, instr) in block.body.iter().enumerate().rev() {
        let span = if instr_idx < old_spans.len() {
            old_spans[instr_idx]
        } else {
            None
        };

        // Definition: if dst is RC'd, non-borrowed, and not live → dead def, emit Dec.
        if let Some(dst) = instr.defined_var() {
            if needs_rc_trackable(dst, ctx) && !live.remove(&dst) {
                new_body.push(ArcInstr::RcDec {
                    var: dst,
                    strategy: rc_strategy(ctx, dst),
                });
                new_spans.push(None);
            }
        }

        // Borrowing uses: emit Dec for last-use RC-typed args.
        //
        // PrimOps and external calls BORROW their args (read without consuming).
        // In the Perceus model, only consuming uses (Invoke, Apply, Construct,
        // PartialApply) transfer ownership. Borrowing uses leave the caller
        // responsible for freeing the value.
        //
        // At this point in the backward walk, `live` reflects variables needed
        // by later instructions (in forward order). Args NOT in `live` are at
        // their last use. In the reversed body, these Decs appear BEFORE the
        // instruction → AFTER the borrowing use in forward order.
        //
        // Ref: Lean 4 `src/Lean/Compiler/IR/RC.lean` — primitive ops are
        // non-consuming; Dec inserted at last borrowing use.
        if is_borrowing_instr(instr, ctx) {
            for &arg in &instr.used_vars() {
                if needs_rc_trackable(arg, ctx) && !live.contains(&arg) {
                    new_body.push(ArcInstr::RcDec {
                        var: arg,
                        strategy: rc_strategy(ctx, arg),
                    });
                    new_spans.push(None);
                }
            }
        }

        new_body.push(instr.clone());
        new_spans.push(span);

        process_instruction_uses(instr, &mut live, &mut new_body, &mut new_spans, ctx);
    }

    // Step 3: Block parameters
    for &(param_var, _ty) in block.params.iter().rev() {
        if needs_rc_trackable(param_var, ctx) && !live.remove(&param_var) {
            new_body.push(ArcInstr::RcDec {
                var: param_var,
                strategy: rc_strategy(ctx, param_var),
            });
            new_spans.push(None);
        }
    }

    // Step 3.5: Invoke dst definitions
    let block_id = ctx.func.blocks[block_idx].id;
    if let Some(dsts) = invoke_defs.get(&block_id) {
        for &dst in dsts.iter().rev() {
            if needs_rc_trackable(dst, ctx) && !live.remove(&dst) {
                new_body.push(ArcInstr::RcDec {
                    var: dst,
                    strategy: rc_strategy(ctx, dst),
                });
                new_spans.push(None);
            }
        }
    }

    // Step 4: Entry block function params
    if is_entry {
        for param in ctx.func.params.iter().rev() {
            if param.ownership == Ownership::Owned
                && ctx.classifier.needs_rc(param.ty)
                && !live.remove(&param.var)
            {
                new_body.push(ArcInstr::RcDec {
                    var: param.var,
                    strategy: rc_strategy(ctx, param.var),
                });
                new_spans.push(None);
            }
        }
    }

    new_body.reverse();
    new_spans.reverse();

    (new_body, new_spans)
}

/// Process terminator uses for RC insertion.
///
/// Each variable used in the terminator: if it needs RC, is not borrowed,
/// and is already in the live set → emit `RcInc`. Add to live set.
///
/// For `Return`, the returned variable is treated as an owned position
/// for borrowed-derived vars (transfer to caller requires Inc).
///
/// **Borrowing Invoke args**: An Invoke arg is treated as borrowing (not
/// consuming) when:
/// - The callee is external (C runtime `ori_*` — borrows all args), OR
/// - The callee's `AnnotatedSig` marks the corresponding param as `Borrowed`
///
/// Borrowing args: add to `live` (keep alive through call) but no `RcInc`.
/// The companion post-pass [`insert_external_invoke_cleanup`] handles the
/// `RcDec` for args whose last use is a borrowing Invoke position.
///
/// Ref: Lean 4 `src/Lean/Compiler/IR/RC.lean` — caller checks callee param
/// ownership to decide Inc/Dec at call sites.
fn process_terminator_uses(
    terminator: &ArcTerminator,
    live: &mut FxHashSet<ArcVarId>,
    new_body: &mut Vec<ArcInstr>,
    new_spans: &mut Vec<Option<ori_ir::Span>>,
    ctx: &RcContext<'_>,
) {
    // Determine which terminator positions are "owned" for borrowed-derived vars.
    let is_return = matches!(terminator, ArcTerminator::Return { .. });

    // Read per-arg borrowing info from the Invoke's pre-computed `arg_ownership`.
    // Populated by `annotate_arg_ownership` before RC insertion.
    let invoke_arg_ownership = match terminator {
        ArcTerminator::Invoke { arg_ownership, .. } => arg_ownership.as_slice(),
        _ => &[],
    };

    for (pos, var) in terminator.used_vars().into_iter().enumerate() {
        if !ctx.classifier.needs_rc(ctx.func.var_type(var)) {
            continue;
        }

        // Borrowed params: completely skip all RC tracking.
        if ctx.borrowed_params.contains(&var) {
            if is_return {
                // Returning a borrowed param transfers ownership to caller.
                // Must Inc even for a borrowed param.
                new_body.push(ArcInstr::RcInc {
                    var,
                    count: 1,
                    strategy: rc_strategy(ctx, var),
                });
                new_spans.push(None);
            }
            continue;
        }

        // Borrowed-derived vars: Inc only in owned positions.
        if ctx.borrows.contains(&var) {
            if is_return {
                new_body.push(ArcInstr::RcInc {
                    var,
                    count: 1,
                    strategy: rc_strategy(ctx, var),
                });
                new_spans.push(None);
            }
            continue;
        }

        // Borrowing Invoke arg: add to live (keeps value alive through the
        // call) but never Inc (callee borrows, doesn't consume).
        // The post-pass inserts Dec for last-use args.
        if invoke_arg_ownership.get(pos).copied() == Some(ArgOwnership::Borrowed) {
            live.insert(var);
            continue;
        }

        // Normal (non-borrowed) var — standard Perceus ownership transfer.
        if live.contains(&var) {
            new_body.push(ArcInstr::RcInc {
                var,
                count: 1,
                strategy: rc_strategy(ctx, var),
            });
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
///
/// **External Apply handling**: Like external Invoke, C runtime `Apply` calls
/// borrow their args. Args are added to `live` (kept alive through the call)
/// but never get `RcInc` (external callee doesn't consume/Dec).
fn process_instruction_uses(
    instr: &ArcInstr,
    live: &mut FxHashSet<ArcVarId>,
    new_body: &mut Vec<ArcInstr>,
    new_spans: &mut Vec<Option<ori_ir::Span>>,
    ctx: &RcContext<'_>,
) {
    // Borrowing uses: PrimOps and external calls don't consume their args.
    let is_borrowing = is_borrowing_instr(instr, ctx);

    // Collect unique vars and count occurrences to handle duplicate args.
    // For example, `Apply { args: [x, x] }` should emit exactly 1 Inc
    // (x appears twice, but one use is "free" and the second is Inc).
    let used = instr.used_vars();
    let mut seen = FxHashSet::default();

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
            if !is_borrowing
                && instr.is_owned_position(pos)
                && !is_borrowed_capture(instr, pos, ctx)
            {
                new_body.push(ArcInstr::RcInc {
                    var,
                    count: 1,
                    strategy: rc_strategy(ctx, var),
                });
                new_spans.push(None);
            }
            continue;
        }

        // Borrowing uses (PrimOp, external Apply): add to live (keeps value
        // alive through the operation) but never Inc (operation borrows,
        // doesn't consume). The backward walk emits Dec for last-use args
        // (see process_block_rc's `is_borrowing_instr` check).
        if is_borrowing {
            live.insert(var);
            continue;
        }

        // Normal (non-borrowed) var.
        if !seen.insert(var) {
            // Duplicate arg in the same instruction — already handled below.
            // The first occurrence either adds to live or emits Inc.
            // The second occurrence always needs Inc.
            new_body.push(ArcInstr::RcInc {
                var,
                count: 1,
                strategy: rc_strategy(ctx, var),
            });
            new_spans.push(None);
            continue;
        }

        if live.contains(&var) {
            // Already live → multi-use, emit Inc.
            new_body.push(ArcInstr::RcInc {
                var,
                count: 1,
                strategy: rc_strategy(ctx, var),
            });
            new_spans.push(None);
        }
        live.insert(var);
    }
}

/// Check if a `PartialApply` capture position is a borrowed callee parameter
/// and the closure doesn't escape the block.
///
/// When capturing a borrowed-derived variable into a closure, we normally need
/// `RcInc` because the closure stores the value. But if:
/// 1. The callee expects this parameter as `Borrowed` (won't store/escape it)
/// 2. The closure doesn't escape the current block (consumed locally)
///
/// ...then the Inc can be safely skipped. The captured value remains alive
/// through its borrow root (a function parameter with lifetime spanning the
/// entire function).
///
/// Follows Lean 4's `Borrow.lean` pattern for closure captures.
#[inline]
fn is_borrowed_capture(instr: &ArcInstr, pos: usize, ctx: &RcContext<'_>) -> bool {
    let (Some(sigs), Some(live_out)) = (ctx.sigs, ctx.block_live_out) else {
        return false;
    };

    let ArcInstr::PartialApply {
        dst, func: callee, ..
    } = instr
    else {
        return false;
    };

    // Closure escapes the block → must Inc for safety.
    if live_out.contains(dst) {
        return false;
    }

    // Callee's parameter at this position is Borrowed → skip Inc.
    sigs.get(callee)
        .and_then(|sig| sig.params.get(pos))
        .is_some_and(|p| p.ownership == Ownership::Borrowed)
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
#[cfg(test)]
fn compute_borrows(block: &ArcBlock, borrowed_params: &FxHashSet<ArcVarId>) -> FxHashSet<ArcVarId> {
    use crate::ir::ArcValue;

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

// Edge cleanup
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
    global_borrows: &FxHashSet<ArcVarId>,
    pool: Option<&Pool>,
) {
    let num_blocks = func.blocks.len();
    let predecessors = compute_predecessors(func);

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
        let decs: Vec<ArcInstr> = vars
            .iter()
            .map(|&v| ArcInstr::RcDec {
                var: v,
                strategy: rc_strategy_direct(func, pool, v),
            })
            .collect();
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
        let trampoline_body: Vec<ArcInstr> = vars
            .iter()
            .map(|&v| ArcInstr::RcDec {
                var: v,
                strategy: rc_strategy_direct(func, pool, v),
            })
            .collect();

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

/// Insert `RcDec` for last-use args passed to borrowing `Invoke` positions.
///
/// Companion to the per-arg borrowing detection in [`process_terminator_uses`].
/// Together they implement correct RC at Invoke call sites:
///
/// 1. `process_terminator_uses` identifies per-arg borrowing:
///    - ALL args to external C runtime functions (`ori_*`)
///    - Args to Ori functions where the callee param is `Borrowed`
///
///    These args are added to `live` (kept alive) but skip `RcInc`.
///
/// 2. This post-pass inserts `RcDec` for borrowing args whose **last use**
///    is the Invoke — i.e., args NOT in `live_out` of the invoking block.
///    Args that ARE in `live_out` are still needed later and will be Dec'd
///    at their actual last-use point by the normal Perceus logic.
///
/// Uses the **original** (pre-RC-insertion) liveness data, which correctly
/// reflects user-code liveness without RC op interference.
///
/// Ref: Lean 4 `src/Lean/Compiler/IR/RC.lean` — caller inserts Dec for
/// args passed to borrowed positions at call sites.
pub fn insert_external_invoke_cleanup(
    func: &mut ArcFunction,
    classifier: &dyn ArcClassification,
    liveness: &BlockLiveness,
    pool: &Pool,
) {
    // Collect: normal_block → [last-use RC-typed borrowing args from Invoke]
    let mut cleanups: FxHashMap<ArcBlockId, Vec<ArcVarId>> = FxHashMap::default();

    for block in &func.blocks {
        if let ArcTerminator::Invoke {
            args,
            arg_ownership,
            normal,
            ..
        } = &block.terminator
        {
            let block_live_out = &liveness.live_out[block.id.index()];

            // Read per-arg ownership from the IR (populated by annotate_arg_ownership).
            let last_use_args: Vec<ArcVarId> = args
                .iter()
                .enumerate()
                .filter(|&(i, &arg)| {
                    arg_ownership.get(i).copied() == Some(ArgOwnership::Borrowed)
                        && classifier.needs_rc(func.var_type(arg))
                        && !block_live_out.contains(&arg)
                })
                .map(|(_, &arg)| arg)
                .collect();

            if !last_use_args.is_empty() {
                cleanups.entry(*normal).or_default().extend(last_use_args);
            }
        }
    }

    if cleanups.is_empty() {
        return;
    }

    tracing::debug!(
        function = func.name.raw(),
        count = cleanups.values().map(Vec::len).sum::<usize>(),
        "inserting invoke borrowed-arg cleanup RcDecs"
    );

    // Prepend RcDec at the START of each normal successor block.
    for (block_id, vars) in cleanups {
        let block_idx = block_id.index();
        for (i, var) in vars.into_iter().enumerate() {
            let strategy = rc_strategy_direct(func, Some(pool), var);
            func.blocks[block_idx]
                .body
                .insert(i, ArcInstr::RcDec { var, strategy });
            func.spans[block_idx].insert(i, None);
        }
    }
}

/// Check if an instruction has BORROWING semantics (reads args without consuming).
///
/// In the Perceus ownership model, consuming operations (function calls,
/// constructors, partial application) transfer ownership of their args to the
/// callee/constructor. Borrowing operations (primitive ops, external runtime
/// calls) just read their args — the caller retains ownership and must
/// free the value when it's no longer needed.
///
/// This distinction drives two behaviors in RC insertion:
/// 1. **No `RcInc`** for borrowing uses even at multi-use points (the operation
///    doesn't hold a reference, so no extra RC needed).
/// 2. **`RcDec` after** the borrowing operation if the arg is at its last use
///    (the caller must free since the operation didn't consume).
///
/// Ref: Lean 4 `src/Lean/Compiler/IR/RC.lean` — projections and primitives
/// are non-consuming; Koka Perceus §3.2 notes same distinction.
fn is_borrowing_instr(instr: &ArcInstr, ctx: &RcContext<'_>) -> bool {
    match instr {
        // PrimOp: arithmetic, comparison, logical, string ops — all borrow.
        ArcInstr::Let {
            value: crate::ir::ArcValue::PrimOp { .. },
            ..
        } => true,

        // Project with scalar result: the parent is borrowed, not consumed.
        //
        // When extracting a scalar field (e.g., tag from Result), the parent
        // still owns its RC fields and must be Dec'd separately. In contrast,
        // projecting an RC field transfers ownership from parent to field
        // (consuming semantics) — no separate Dec needed for the parent.
        //
        // Follows Lean 4 `src/Lean/Compiler/IR/RC.lean`:
        // `proj i x` borrows x; if the result is an object, Inc it.
        ArcInstr::Project { dst, .. } => ctx.classifier.is_scalar(ctx.func.var_type(*dst)),

        // Apply with all-borrowed args: external C runtime functions borrow without
        // Perceus ownership. Detected via the pre-computed `arg_ownership` field.
        ArcInstr::Apply { arg_ownership, .. } => {
            !arg_ownership.is_empty() && arg_ownership.iter().all(|o| *o == ArgOwnership::Borrowed)
        }

        _ => false,
    }
}

/// Compute per-argument ownership for a single call.
///
/// Determines whether each argument is borrowed (callee reads without
/// consuming) or owned (ownership transfers to callee). Uses the callee's
/// `AnnotatedSig` when available, falls back to all-owned for unknown callees.
///
/// External callees (C runtime `ori_*` functions not in the `sigs` map) get
/// all-borrowed — they don't participate in Perceus ownership.
///
/// Builtin methods (e.g., `len`, `is_empty`) that are known to borrow their
/// receiver also get all-borrowed — they're compiled inline by the LLVM
/// emitter and don't consume their arguments.
fn compute_arg_ownership(
    callee: ori_ir::Name,
    arg_count: usize,
    sigs: &FxHashMap<ori_ir::Name, crate::ownership::AnnotatedSig>,
    interner: &ori_ir::StringInterner,
    borrowing_builtins: &FxHashSet<ori_ir::Name>,
) -> Vec<ArgOwnership> {
    // External C runtime: not in sigs, name starts with `ori_`.
    if !sigs.contains_key(&callee) {
        if interner
            .try_lookup(callee)
            .is_some_and(|name_str| name_str.starts_with("ori_"))
        {
            return vec![ArgOwnership::Borrowed; arg_count];
        }
        // Builtin method with borrowing receiver (e.g., len, is_empty).
        if borrowing_builtins.contains(&callee) {
            return vec![ArgOwnership::Borrowed; arg_count];
        }
        // Unknown non-external: conservative owned.
        return vec![ArgOwnership::Owned; arg_count];
    }

    // Ori function with known signature: per-param ownership.
    if let Some(sig) = sigs.get(&callee) {
        return (0..arg_count)
            .map(|i| {
                if sig
                    .params
                    .get(i)
                    .is_some_and(|p| p.ownership == Ownership::Borrowed)
                {
                    ArgOwnership::Borrowed
                } else {
                    ArgOwnership::Owned
                }
            })
            .collect();
    }

    vec![ArgOwnership::Owned; arg_count]
}

/// Populate `arg_ownership` on all `Apply` and `Invoke` instructions.
///
/// Must be called **before** [`insert_rc_ops_with_ownership`] so that the
/// RC insertion pass can read ownership from the IR instead of re-deriving
/// it from sigs + interner.
///
/// This is the single point where external-callee detection and per-param
/// ownership lookup happen. All downstream passes read from the field.
///
/// `borrowing_builtins` identifies builtin method names (e.g., `len`,
/// `is_empty`) whose receiver is always borrowed. These are compiled inline
/// by the LLVM emitter — their args must be marked Borrowed so that the
/// caller retains ownership and inserts `RcDec` at the arg's last use.
#[expect(clippy::implicit_hasher, reason = "FxHashMap is the canonical hasher")]
pub fn annotate_arg_ownership(
    func: &mut ArcFunction,
    sigs: &FxHashMap<ori_ir::Name, crate::ownership::AnnotatedSig>,
    interner: &ori_ir::StringInterner,
    borrowing_builtins: &FxHashSet<ori_ir::Name>,
) {
    for block in &mut func.blocks {
        // Annotate body instructions.
        for instr in &mut block.body {
            if let ArcInstr::Apply {
                func: callee,
                args,
                arg_ownership,
                ..
            } = instr
            {
                *arg_ownership =
                    compute_arg_ownership(*callee, args.len(), sigs, interner, borrowing_builtins);
            }
        }

        // Annotate terminator.
        if let ArcTerminator::Invoke {
            func: callee,
            args,
            arg_ownership,
            ..
        } = &mut block.terminator
        {
            *arg_ownership =
                compute_arg_ownership(*callee, args.len(), sigs, interner, borrowing_builtins);
        }
    }
}

#[cfg(test)]
mod tests;
