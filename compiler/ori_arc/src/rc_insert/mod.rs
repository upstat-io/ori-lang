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

mod insert;

use rustc_hash::{FxHashMap, FxHashSet};

#[cfg(test)]
pub(crate) use self::insert::insert_rc_ops;
pub use self::insert::insert_rc_ops_with_ownership;

use crate::graph::compute_predecessors;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership, RcStrategy,
};
use crate::liveness::BlockLiveness;
use crate::ownership::Ownership;
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
        //
        // Exception: `Binary(Add)` on list-typed operands is **consuming**.
        // The LLVM backend compiles list `+` to `ori_list_concat_cow`, which
        // is a COW function that takes ownership of both operands (receiver
        // and list2). The ARC pipeline must NOT emit `RcDec` for either
        // operand — the runtime handles their RC lifecycle internally.
        ArcInstr::Let {
            value: crate::ir::ArcValue::PrimOp { op, args },
            ..
        } => {
            if matches!(op, crate::ir::PrimOp::Binary(ori_ir::BinaryOp::Add)) {
                if let Some(pool) = ctx.pool {
                    if let Some(&first_arg) = args.first() {
                        let arg_ty = ctx.func.var_type(first_arg);
                        let resolved = pool.resolve_fully(arg_ty);
                        if pool.tag(resolved) == ori_types::Tag::List {
                            return false;
                        }
                    }
                }
            }
            true
        }

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
///
/// `builtins.consuming_receiver` identifies COW list methods (e.g., `push`,
/// `reverse`) where the receiver's RC is managed internally by the runtime.
/// When the receiver type is `List`, the first arg is marked `Owned` to
/// prevent the ARC pipeline from emitting a duplicate `RcDec`.
///
/// `builtins.consuming_second_arg` identifies COW list methods (e.g., `add`,
/// `concat`) where the runtime also consumes the second argument (list2).
/// When the receiver is `List` and `args.len() >= 2`, arg[1] is also `Owned`.
#[expect(clippy::implicit_hasher, reason = "FxHashMap is the canonical hasher")]
pub fn annotate_arg_ownership(
    func: &mut ArcFunction,
    sigs: &FxHashMap<ori_ir::Name, crate::ownership::AnnotatedSig>,
    interner: &ori_ir::StringInterner,
    builtins: &crate::BuiltinOwnershipSets,
    pool: &Pool,
) {
    // Split borrow: var_types is read-only, blocks are mutated.
    let var_types = &func.var_types;

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
                    compute_arg_ownership(*callee, args.len(), sigs, interner, &builtins.borrowing);
                apply_consuming_overrides(
                    *callee,
                    args,
                    arg_ownership,
                    &builtins.consuming_receiver,
                    &builtins.consuming_second_arg,
                    var_types,
                    pool,
                );
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
                compute_arg_ownership(*callee, args.len(), sigs, interner, &builtins.borrowing);
            apply_consuming_overrides(
                *callee,
                args,
                arg_ownership,
                &builtins.consuming_receiver,
                &builtins.consuming_second_arg,
                var_types,
                pool,
            );
        }
    }
}

/// Override borrowing ownership for COW list methods with consuming semantics.
///
/// Applies two overrides for list-typed receivers:
/// 1. **Receiver** (arg[0]): When `callee` is in `consuming_receiver_builtins`
///    and the receiver is `List`, marks arg[0] as `Owned`.
/// 2. **Second arg** (arg[1]): When `callee` is *also* in
///    `consuming_second_arg_builtins` and the receiver is `List`, marks
///    arg[1] as `Owned` too — the runtime consumes list2's buffer.
///
/// Type-qualified: `"add"` and `"concat"` are shared names — borrowing for
/// strings, consuming for lists. Only the list case is overridden here.
fn apply_consuming_overrides(
    callee: ori_ir::Name,
    args: &[ArcVarId],
    arg_ownership: &mut [ArgOwnership],
    consuming_receiver_builtins: &FxHashSet<ori_ir::Name>,
    consuming_second_arg_builtins: &FxHashSet<ori_ir::Name>,
    var_types: &[ori_types::Idx],
    pool: &Pool,
) {
    if args.is_empty() || arg_ownership.is_empty() {
        return;
    }

    // Check if the receiver is a list (type-qualified gate).
    let is_receiver_consuming = consuming_receiver_builtins.contains(&callee);
    if !is_receiver_consuming {
        return;
    }

    let receiver_var = args[0];
    let receiver_idx = var_types[receiver_var.index()];
    if pool.tag(receiver_idx) != ori_types::Tag::List {
        return;
    }

    // Receiver is List — mark it as Owned.
    arg_ownership[0] = ArgOwnership::Owned;

    // Also mark second arg as Owned if this method consumes list2.
    if consuming_second_arg_builtins.contains(&callee)
        && args.len() >= 2
        && arg_ownership.len() >= 2
    {
        arg_ownership[1] = ArgOwnership::Owned;
    }
}

#[cfg(test)]
mod tests;
