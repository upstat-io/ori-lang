//! Take-project alias class computation (TPR-07-011 / TPR-07-016).
//!
//! A **take-project** is a `Project` instruction whose source is a sum
//! type (`Enum`/`Option`/`Result`) and whose projected payload is a
//! unique-owned Box (`Tag::Iterator` or `Tag::DoubleEndedIterator`).
//! The predicate lives in [`super::borrowed_defs::is_take_project`].
//! Once a take-project executes, the source enum has logically given
//! up its payload — any subsequent scope-exit `RcDec` that walks the
//! source's representation would re-free a pointer the consumer
//! already freed (TPR-07-011 double-free).
//!
//! # The fix surface
//!
//! TPR-07-011 used a function-global suppression: if ANY take-project
//! reached a variable in the alias chain, every cleanup site skipped
//! that variable. That was too coarse — TPR-07-016 showed that on
//! runtime paths which bypass the projection (e.g. the `else` arm of
//! an `if` that guards the `match`), the source enum was NEVER
//! dropped and its Box-allocated payload leaked.
//!
//! The fix: **drop the global suppression and rely on the existing
//! per-instruction `is_ownership_transfer` check at the take-project
//! `Project` site itself.** Natural scope-exit drops in non-projecting
//! predecessors (e.g., the `else` branch's `RcDec %src`) handle the
//! cleanup on bypass paths. The take-project's `is_ownership_transfer
//! ` classification suppresses the source enum's last-use drop at the
//! `Project` site, preventing the double-free that TPR-07-011 was
//! originally chasing.
//!
//! # What this module still does
//!
//! Two cleanup sites need to know whether a variable belongs to a
//! take-project alias class so they can skip emitting a redundant
//! second drop:
//!
//! 1. [`dead_cleanup::emit_dead_at_entry_decs`] source 1: variables
//!    that are unused-and-dead-at-exit and would otherwise get a
//!    block-local drop. Skipping the in-class case prevents an
//!    unconditional drop on paths that already saw the take-project.
//! 2. [`dead_cleanup::emit_dead_block_param_decs`] (source 2): block
//!    params that are SSA aliases of the take-project source via
//!    Jump-arg propagation. Routing them to predecessors (the
//!    natural mechanism for merge-block dead params) would emit a
//!    drop using a name that has no SSA definition reachable from
//!    the predecessor — the LLVM emitter resolves the param ID to
//!    the merge block's phi node, producing a phi-dominance
//!    violation. Skipping the in-class case avoids the spurious
//!    drop entirely; the source enum's natural drops cover cleanup.
//!
//! Both call sites only need the membership question
//! `is_in_class(var)`. No dataflow is required — the alias class is
//! the closure of:
//!
//! - bidirectional `Let { dst, value: Var(src) }` (`dst ↔ src`)
//! - forward `Jump arg → block param` (passing the var to a merge
//!   block param propagates the class to the param)
//!
//! seeded by every take-project source.
//!
//! # Reference
//!
//! Codex iteration 6 TPR review finding (TPR-07-016). Discussion in
//! `plans/repr-opt/section-07-enum-repr.md` §07.R TPR-07-016.

use rustc_hash::FxHashSet;

use ori_types::Pool;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

use super::borrowed_defs::is_take_project;

/// Per-function take-project facts queried by RC cleanup sites.
///
/// Two pieces of information:
///
/// - **Alias-class membership** ([`Self::is_in_class`]): the set of
///   `ArcVarId`s that transitively reach a take-project source via
///   Let aliases or Jump-arg → block-param propagation. These vars
///   share storage with the take-project source enum (or its
///   moved-out payload).
///
/// - **Bypass-safe blocks** ([`Self::is_bypass_safe_block`]): the
///   set of block indices where the take-project is *unreachable*
///   in BOTH directions — neither forward (the take-project hasn't
///   happened yet) nor backward (the take-project has happened and
///   the payload is consumed). A bypass-safe block is a place where
///   the source enum is still live AND the take-project will never
///   consume it on any reachable path → it's safe to emit a
///   scope-exit drop without conflicting with the take-project's
///   ownership transfer or with a post-projection double-free.
///
/// Built by [`analyze`].
pub(crate) struct TakeMoveFacts {
    in_class: FxHashSet<ArcVarId>,
    bypass_safe_blocks: FxHashSet<usize>,
}

impl TakeMoveFacts {
    /// Empty facts — used when the function has no take-projects at
    /// all. Avoids allocating the membership set for the common case.
    pub(crate) fn empty() -> Self {
        Self {
            in_class: FxHashSet::default(),
            bypass_safe_blocks: FxHashSet::default(),
        }
    }

    /// Whether `var` participates in any take-project alias class.
    /// Variables outside every class are cleaned up normally.
    pub(crate) fn is_in_class(&self, var: ArcVarId) -> bool {
        self.in_class.contains(&var)
    }

    /// Whether `block_idx` is a "bypass-safe" block — neither
    /// forward- nor backward-reachable from any take-project. RC
    /// cleanup may emit a scope-exit drop for an in-class var here
    /// without colliding with the take-project's ownership transfer
    /// (TPR-07-011) or double-freeing a post-projection payload.
    pub(crate) fn is_bypass_safe_block(&self, block_idx: usize) -> bool {
        self.bypass_safe_blocks.contains(&block_idx)
    }
}

/// Compute take-project facts for `func`.
///
/// Returns empty facts when the function has no take-projects,
/// avoiding both the alias closure and the reachability sweep.
pub(crate) fn analyze(func: &ArcFunction, pool: &Pool) -> TakeMoveFacts {
    let take_project_blocks = collect_take_project_blocks(func, pool);
    if take_project_blocks.is_empty() {
        return TakeMoveFacts::empty();
    }
    let take_projects = collect_take_projects(func, pool);
    let in_class = compute_alias_class_set(func, &take_projects);
    let bypass_safe_blocks = compute_bypass_safe_blocks(func, &take_project_blocks);
    TakeMoveFacts {
        in_class,
        bypass_safe_blocks,
    }
}

/// Collect the indices of every block that contains at least one
/// take-project `Project` instruction.
fn collect_take_project_blocks(func: &ArcFunction, pool: &Pool) -> Vec<usize> {
    let mut blocks = Vec::new();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        if block
            .body
            .iter()
            .any(|instr| is_take_project(instr, func, pool))
        {
            blocks.push(block_idx);
        }
    }
    blocks
}

/// Compute the set of block indices that are NEITHER forward- NOR
/// backward-reachable from any take-project block.
///
/// A block is "bypass-safe" if no CFG path through it touches a
/// take-project. The source enum is still live AND will never be
/// consumed by the take-project on any reachable path, so a
/// scope-exit `RcDec` here is the canonical place to drop it without
/// conflicting with the take-project's ownership transfer.
///
/// The take-project blocks themselves are NOT bypass-safe — the
/// take-project's `is_ownership_transfer` mechanism handles the drop
/// at the `Project` site (TPR-07-011), and emitting an entry-time
/// dec would walk the tagged-pointer encoding before the projection
/// reads its payload.
fn compute_bypass_safe_blocks(func: &ArcFunction, tp_blocks: &[usize]) -> FxHashSet<usize> {
    let predecessors = crate::graph::compute_predecessors(func);
    // Forward-reachable from any take-project block (including the
    // block itself). These are the "post-move" blocks where the
    // payload has already been consumed.
    let mut forward_reachable: FxHashSet<usize> = FxHashSet::default();
    let mut work: Vec<usize> = tp_blocks.to_vec();
    while let Some(b) = work.pop() {
        if !forward_reachable.insert(b) {
            continue;
        }
        for succ in crate::graph::successor_block_ids(&func.blocks[b].terminator) {
            work.push(succ.index());
        }
    }
    // Backward-reachable from any take-project block (including the
    // block itself). These are the "pre-move" blocks where the
    // payload might still be needed by the take-project.
    let mut backward_reachable: FxHashSet<usize> = FxHashSet::default();
    let mut work: Vec<usize> = tp_blocks.to_vec();
    while let Some(b) = work.pop() {
        if !backward_reachable.insert(b) {
            continue;
        }
        if let Some(preds) = predecessors.get(b) {
            for &pred in preds {
                work.push(pred);
            }
        }
    }
    // Bypass-safe = complement of (forward ∪ backward).
    (0..func.blocks.len())
        .filter(|b| !forward_reachable.contains(b) && !backward_reachable.contains(b))
        .collect()
}

/// Scan the function for every take-project `Project` instruction and
/// return the list of source variables. Used as the seed for alias
/// class computation and as a fast "no take-projects → empty facts"
/// check.
fn collect_take_projects(func: &ArcFunction, pool: &Pool) -> Vec<ArcVarId> {
    let mut sources = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            if is_take_project(instr, func, pool) {
                if let ArcInstr::Project { value, .. } = instr {
                    sources.push(*value);
                }
            }
        }
    }
    sources
}

/// Compute the union of every take-project source's alias class.
///
/// Returns the set of `ArcVarId`s that transitively reach any
/// take-project source via:
/// - bidirectional `Let { dst, value: Var(src) }` (dst ↔ src)
/// - forward `Jump arg → block param` (arg in class → param in class)
///
/// All take-project sources in the function land in the SAME union.
/// This is a conservative over-approximation that is sufficient for
/// the membership-only consumers in `dead_cleanup`. If a future
/// per-class consumer ever needs distinct tracking (e.g. two
/// iterators moved on different branches), this function should be
/// partitioned into per-seed connected components.
fn compute_alias_class_set(func: &ArcFunction, seeds: &[ArcVarId]) -> FxHashSet<ArcVarId> {
    let mut class: FxHashSet<ArcVarId> = seeds.iter().copied().collect();
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
                    if class.contains(dst) && class.insert(*src) {
                        changed = true;
                    }
                    if class.contains(src) && class.insert(*dst) {
                        changed = true;
                    }
                }
            }
            if let ArcTerminator::Jump { target, args } = &block.terminator {
                let target_idx = target.index();
                if target_idx < func.blocks.len() {
                    for (i, &arg) in args.iter().enumerate() {
                        if !class.contains(&arg) {
                            continue;
                        }
                        if let Some(&(param_var, _)) = func.blocks[target_idx].params.get(i) {
                            if class.insert(param_var) {
                                changed = true;
                            }
                        }
                    }
                }
            }
        }
    }
    class
}
