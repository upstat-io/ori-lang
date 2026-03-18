//! Per-block helper types and precomputation for RC emission.
//!
//! Contains the [`BlockCtx`] context bundle, the [`LastUse`] position enum,
//! and all functions that scan a block to collect use information, determine
//! ownership, and extend parent lifetimes past borrowed children.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_types::Pool;

use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::{AccessClass, Cardinality};
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr,
};
use crate::ownership::Ownership;

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

/// Collect variables defined by borrowing instructions (`Project`).
///
/// These create borrowed references that do NOT need independent RC
/// management — the source variable's RC covers the borrowed ref.
pub(crate) fn collect_borrowed_defs(block: &ArcBlock) -> FxHashSet<ArcVarId> {
    let mut borrowed = FxHashSet::default();
    for instr in &block.body {
        if let ArcInstr::Project { dst, .. } = instr {
            borrowed.insert(*dst);
        }
    }
    borrowed
}

/// Collect variables that are direct element projections from `__iter_next`
/// results, plus their Let aliases (transitive closure).
///
/// These variables are borrowed from the collection buffer. Their cleanup
/// is handled by `elem_dec_fn` when the collection is freed, so the AIMS
/// pipeline should NOT emit independent `RcDec` for them.
///
/// Specifically targets: `Project { dst, src, field: 1 }` where `src` is
/// defined by `Apply { func: __iter_next, .. }`.
pub(crate) fn collect_iter_element_defs(
    func: &ArcFunction,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    let iter_next_name =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::IterNext.name());

    // Phase 1: find all Apply @__iter_next calls. Collect:
    // - dst variables (the __iter_next result)
    // - args[1] (the elem_ty_marker phantom — a zero-valued type marker
    //   that must NOT be RC-decremented; its LLVM repr is `i64 0`, not a
    //   real struct, so RcDec on it operates on garbage memory)
    let mut iter_next_dsts: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut iter_elems = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst, func: f, args, ..
            } = instr
            {
                if *f == iter_next_name {
                    iter_next_dsts.insert(*dst);
                    // args[1] is the elem_ty_marker phantom — suppress RcDec
                    if args.len() > 1 {
                        iter_elems.insert(args[1]);
                    }
                }
            }
        }
    }

    // Phase 2: find Project at field index 1 from __iter_next results.
    // This is the yielded element (field 0 is the Option tag).
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project {
                dst,
                value,
                field: 1,
                ..
            } = instr
            {
                if iter_next_dsts.contains(value) {
                    iter_elems.insert(*dst);
                }
            }
        }
    }

    // Phase 2.5: propagate through transitive Project chains.
    // Map iteration yields `(key, val)` tuples — destructuring produces
    // Project chains: `%tuple = Project __iter_next.1`, then
    // `%key = Project %tuple.0`, `%val = Project %tuple.1`.
    // Without this phase, the destructured key/val are NOT in iter_elems,
    // so AIMS emits spurious RcDec on them.
    loop {
        let prev_len = iter_elems.len();
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Project { dst, value, .. } = instr {
                    if iter_elems.contains(value) {
                        iter_elems.insert(*dst);
                    }
                }
            }
        }
        if iter_elems.len() == prev_len {
            break;
        }
    }

    // Phase 3: propagate through Let aliases and block params.
    propagate_borrowed_closure(func, &mut iter_elems);
    iter_elems
}

/// Collect variables borrowed via `Project` only (excludes function params).
///
/// Includes:
/// 1. `Project` instructions (borrowed views into structs/enums)
/// 2. `Let` aliases of project-borrowed variables (transitive closure)
///
/// This set identifies variables whose RC is managed by a parent aggregate.
/// Unlike [`collect_all_borrowed_defs`], function parameters with
/// `Ownership::Borrowed` are excluded — they can participate in `RcInc`
/// decisions (e.g., when a borrowed list parameter is used to create an
/// iterator that has its own reference).
pub(crate) fn collect_project_borrowed_defs(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    let mut borrowed = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { dst, .. } = instr {
                borrowed.insert(*dst);
            }
        }
    }
    propagate_borrowed_closure(func, &mut borrowed);
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
pub(crate) fn collect_all_borrowed_defs(func: &ArcFunction) -> FxHashSet<ArcVarId> {
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
    // Transitive closure: Let aliases AND block parameter flows.
    propagate_borrowed_closure(func, &mut borrowed);
    borrowed
}

/// Propagate borrowed-ness through Let aliases and block parameter flows.
///
/// Computes the transitive closure of the `borrowed` set by following:
/// 1. `Let { dst, value: Var(src) }` — pointer copy aliases
/// 2. `Jump { target, args }` — when a borrowed variable is passed as a Jump
///    argument, the corresponding block parameter inherits borrowed status
///
/// This handles the `__for_coll` pattern where a borrowed collection parameter
/// is threaded through loop headers and exit blocks via block parameter passing.
fn propagate_borrowed_closure(func: &ArcFunction, borrowed: &mut FxHashSet<ArcVarId>) {
    let mut changed = true;
    while changed {
        changed = false;
        for block in &func.blocks {
            // Let aliases: `let dst = borrowed_var`
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
            // Jump arg→param: borrowed variable passed to target block param.
            if let ArcTerminator::Jump { target, args } = &block.terminator {
                let target_idx = target.index();
                if target_idx < func.blocks.len() {
                    let target_params = &func.blocks[target_idx].params;
                    for (arg, (param_var, _)) in args.iter().zip(target_params) {
                        if borrowed.contains(arg) && borrowed.insert(*param_var) {
                            changed = true;
                        }
                    }
                }
            }
        }
    }
}

/// For each Project source variable in a block, compute the latest `LastUse`
/// of any borrowed child (direct Project destinations or their Let aliases).
///
/// A parent aggregate must not be decremented before all of its borrowed
/// children are dead. The AIMS backward analysis doesn't track this
/// relationship (it only propagates demand for direct uses), so the
/// emission phase extends parent lifetimes using this map.
pub(crate) fn compute_child_effective_last_use(
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
#[inline]
pub(crate) fn is_ownership_transfer(instr: &ArcInstr, func: &ArcFunction) -> bool {
    match instr {
        ArcInstr::Let {
            dst,
            value: ArcValue::Var(_),
            ..
        }
        | ArcInstr::Construct { dst, .. }
        | ArcInstr::PartialApply { dst, .. } => func.var_reprs[dst.index()] != ValueRepr::Scalar,
        _ => false,
    }
}
