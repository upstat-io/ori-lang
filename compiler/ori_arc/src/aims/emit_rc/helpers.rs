//! Per-block helper types and precomputation for RC emission.
//!
//! Contains the [`BlockCtx`] context bundle, the [`LastUse`] position enum,
//! and all functions that scan a block to collect use information, determine
//! ownership, and extend parent lifetimes past borrowed children.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_types::Pool;

use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::{AccessClass, Cardinality};
use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcValue, ArcVarId, ValueRepr};

/// Shared context for per-block RC emission helpers.
pub(crate) struct BlockCtx<'a> {
    pub(crate) func: &'a ArcFunction,
    pub(crate) blk: ArcBlockId,
    pub(crate) state_map: &'a AimsStateMap,
    pub(crate) defined_in_block: &'a FxHashSet<ArcVarId>,
    /// Variables defined by `Project` (borrowed — no independent RC management).
    pub(crate) borrowed_defs: &'a FxHashSet<ArcVarId>,
    /// Function-level set of all variables defined by `Project` in any block.
    pub(crate) all_borrowed_defs: &'a FxHashSet<ArcVarId>,
    /// Function-level set of Project-borrowed variables only (excludes function
    /// params). Used to allow `RcInc` for borrowed
    /// params that need it (e.g., for-loop collection cleanup).
    pub(crate) project_borrowed_defs: &'a FxHashSet<ArcVarId>,
    /// Variables that are projections of `__iter_next` results (field index 1).
    /// These elements are borrowed from the collection buffer — their cleanup
    /// is handled by the collection's `elem_dec_fn`, not by caller-side `RcDec`.
    pub(crate) iter_element_defs: &'a FxHashSet<ArcVarId>,
    /// Variables projected from inline-enum sources (`Option`, `Result`, `Enum`).
    /// Their RC is managed by the parent inline-enum's `RcDec`/`RcInc` — no
    /// separate per-field `RcDec` is needed.
    pub(crate) inline_enum_projected_defs: &'a FxHashSet<ArcVarId>,
    pub(crate) use_info: &'a FxHashMap<ArcVarId, (usize, LastUse)>,
    pub(crate) pool: &'a Pool,
    /// For each Project source variable, the latest `LastUse` of any borrowed
    /// child (direct Project destinations or their Let aliases). Used to defer
    /// parent `RcDec` until all borrowed children are dead.
    pub(crate) child_effective_last_use: &'a FxHashMap<ArcVarId, LastUse>,
}

/// Where a variable is last used within a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LastUse {
    /// Last used in a body instruction at the given index.
    Body(usize),
    /// Last used in the block terminator.
    Terminator,
}

/// Pre-scan a block to determine total use count and last-use position
/// for each variable.
pub(crate) fn precompute_block_uses(block: &ArcBlock) -> FxHashMap<ArcVarId, (usize, LastUse)> {
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

/// Whether a variable is live (cardinality > Absent) at a block's exit.
#[inline]
pub(crate) fn is_live_at_exit(state_map: &AimsStateMap, blk: ArcBlockId, var: ArcVarId) -> bool {
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
pub(crate) fn is_owned_at_entry(
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
    // TPR-07-011: take-project source chains are "moved" — the source
    // enum has logically given up its payload. Even if the AIMS
    // lattice still reports `access == Owned` (the backward analysis
    // doesn't model unique-owned moves), treat the variable as not
    // owned so no scope-exit `RcDec` is emitted. Without this,
    // `emit_dead_at_entry_decs` would walk the tagged-pointer encoding
    // and call `ori_iter_drop` on a payload that was already consumed
    // by the projected variable's consumer, double-freeing the Box.
    if all_borrowed_defs.contains(&var) {
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

/// Collect variables defined in a block (body instructions + block params).
pub(crate) fn collect_defined_vars(block: &ArcBlock) -> FxHashSet<ArcVarId> {
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

/// Compute a function-level mapping from Project destinations to their
/// source variables, across all blocks.
///
/// Needed by [`compute_child_effective_last_use`] to detect cross-block
/// Project relationships: a variable defined by `Project` in block A may
/// be used (via `Let` aliases) in block B, and block B needs to know that
/// those aliases borrow from the Project source.
pub(crate) fn compute_function_project_sources(
    func: &ArcFunction,
) -> FxHashMap<ArcVarId, ArcVarId> {
    let mut sources: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { dst, value, .. } = instr {
                sources.insert(*dst, *value);
            }
        }
    }
    sources
}

/// For each Project source variable in a block, compute the latest `LastUse`
/// of any borrowed child (direct Project destinations or their Let aliases).
///
/// A parent aggregate must not be decremented before all of its borrowed
/// children are dead. The AIMS backward analysis doesn't track this
/// relationship (it only propagates demand for direct uses), so the
/// emission phase extends parent lifetimes using this map.
///
/// The `func_project_sources` parameter provides a function-level mapping
/// from Project destinations to their source variables, enabling cross-block
/// Project detection.
pub(crate) fn compute_child_effective_last_use(
    block: &ArcBlock,
    use_info: &FxHashMap<ArcVarId, (usize, LastUse)>,
    func_project_sources: &FxHashMap<ArcVarId, ArcVarId>,
) -> FxHashMap<ArcVarId, LastUse> {
    // Build mapping from each borrowed child to its Project source.
    // Start with block-local Projects.
    let mut child_to_parent: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    for instr in &block.body {
        if let ArcInstr::Project { dst, value, .. } = instr {
            child_to_parent.insert(*dst, *value);
        }
    }

    // Add cross-block Project results: any variable USED in this block that
    // was defined by a Project in another block.
    for &var in use_info.keys() {
        if let Some(&parent) = func_project_sources.get(&var) {
            child_to_parent.entry(var).or_insert(parent);
        }
    }

    // Extend to Let aliases: `Let { dst, Var(src) }` where `src` borrows
    // from a parent means `dst` also borrows from the same parent.
    // Also handles cross-block: if `src` was a Project result from another
    // block (present in func_project_sources), `dst` inherits that parent.
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
                let parent = child_to_parent
                    .get(src)
                    .copied()
                    .or_else(|| func_project_sources.get(src).copied());
                if let Some(parent) = parent {
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

/// Whether an instruction is a consuming `PrimOp` — a `Let` with a `PrimOp`
/// value that produces an `RcPointer` result.
///
/// Only `RcPointer`-producing `PrimOp`s (list concat, map merge, set union)
/// consume their operands internally via COW runtime functions. The ARC
/// pipeline must NOT emit separate `RcDec` for those operands.
///
/// `FatValue`-producing `PrimOp`s (string concat `+`) do NOT consume
/// operands — `ori_str_concat` borrows both inputs and returns a new
/// value. The caller is responsible for dropping the old operands.
#[inline]
pub(crate) fn is_consuming_primop(instr: &ArcInstr, func: &ArcFunction) -> bool {
    if let ArcInstr::Let {
        dst,
        value: ArcValue::PrimOp { .. },
        ..
    } = instr
    {
        func.var_reprs[dst.index()] == ValueRepr::RcPointer
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
/// - `Project { dst, value }` where the projection is a *take* (see
///   `is_take_project` in `borrowed_defs.rs`) — the projected payload
///   moves ownership out of the source enum, which logically no longer
///   contains anything to drop. TPR-07-011.
#[inline]
pub(crate) fn is_ownership_transfer(instr: &ArcInstr, func: &ArcFunction, pool: &Pool) -> bool {
    match instr {
        ArcInstr::Let {
            dst,
            value: ArcValue::Var(_),
            ..
        }
        | ArcInstr::Construct { dst, .. }
        | ArcInstr::PartialApply { dst, .. } => func.var_reprs[dst.index()] != ValueRepr::Scalar,
        ArcInstr::Project { .. } => super::borrowed_defs::is_take_project(instr, func, pool),
        _ => false,
    }
}
