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

mod annotate;
mod block_rc;
mod edge_cleanup;
mod insert;

use rustc_hash::{FxHashMap, FxHashSet};

pub use self::annotate::annotate_arg_ownership;
pub use self::edge_cleanup::insert_external_invoke_cleanup;
#[cfg(test)]
pub(crate) use self::insert::insert_rc_ops;
pub use self::insert::insert_rc_ops_with_ownership;

use crate::ir::{ArcFunction, ArcInstr, ArcVarId, ArgOwnership, RcStrategy};

#[cfg(test)]
use crate::ir::ArcBlock;
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

#[cfg(test)]
mod tests;
