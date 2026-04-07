//! Borrowed-definition collection for RC emission.
//!
//! Identifies variables whose RC is managed by a parent aggregate or iterator,
//! suppressing independent `RcDec` for borrowed views. Also collects
//! COW-borrowed receiver variables that need pre-call `RcInc` guards.
//!
//! Extracted from `helpers.rs` to respect the 500-line file size limit.

use rustc_hash::FxHashSet;

use ori_types::{Pool, Tag};

use crate::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr};
use crate::ownership::Ownership;

/// Classify a `Project` instruction as a "take" (consuming
/// move) rather than a borrow.
///
/// For the normal refcounted case (str, [T], structs containing RC
/// children), `Project` produces a borrowed view into the parent
/// aggregate: the projected child shares a refcount-bumped reference
/// to the same heap data, and the parent's scope-exit `RcDec` is the
/// canonical drop site. This works because `RcInc` on the projected
/// child at the point of consumption gives the child its own
/// independent refcount, balancing the parent's later `RcDec`.
///
/// For **unique-owned Box payloads** (specifically `Tag::Iterator` and
/// `Tag::DoubleEndedIterator`, which are `MachineRepr::UnmanagedPtr`
/// with no RC header), there is no refcount to bump — `RcInc` on the
/// projected iterator is intentionally a no-op. This breaks the
/// balance: the projected child is consumed by the method call
/// (`ori_iter_count`, `ori_iter_map`, etc. via `Box::from_raw`), but
/// the parent enum's scope-exit `RcDec` still walks the tagged-pointer
/// encoding and calls `ori_iter_drop` on the same (now-freed) pointer
/// → double-free.
///
/// The architectural fix (Codex / prior art from Swift SIL, Lean 4
/// LCNF, Koka Perceus) is to distinguish "borrow projection" from
/// "consuming projection" at the AIMS classification layer. A take-
/// project:
///
/// 1. Does **not** propagate borrowed-parent demand (the child does
///    not "keep the parent alive" — the parent has logically given
///    up its payload).
/// 2. Is **not** a borrowed def (so no spurious `RcInc` fires at the
///    owned-arg call site — there is nothing to refcount anyway).
/// 3. **Is** an ownership transfer (the parent enum has no payload
///    left to drop, so its scope-exit `RcDec` is suppressed).
///
/// # Current scope
///
/// This predicate only fires for `Tag::Iterator` and
/// `Tag::DoubleEndedIterator` payloads projected from sum types
/// (`Enum`, `Option`, `Result`). Those are the only types that
/// (a) are `UnmanagedPtr` (no refcount, move-only) and
/// (b) can reach a `Project` instruction as a variant payload. Other
/// unique-owned types (channels via `OpaquePtr` — though channels
/// currently have no RC header dropping pipeline at all) can be added
/// here when they grow `Project` call sites.
#[inline]
pub(crate) fn is_take_project(instr: &ArcInstr, func: &ArcFunction, pool: &Pool) -> bool {
    let ArcInstr::Project {
        dst, value, field, ..
    } = instr
    else {
        return false;
    };
    // Only consider non-tag fields (field 0 = tag discriminant for
    // tagged-ptr / inline enums, never the payload).
    if *field == 0 {
        return false;
    }
    // Source must be a sum type that can carry a payload.
    let src_ty = func.var_type(*value);
    let src_resolved = pool.resolve_fully(src_ty);
    let src_tag = pool.tag(src_resolved);
    if !matches!(src_tag, Tag::Enum | Tag::Option | Tag::Result) {
        return false;
    }
    // Destination (the projected payload) must be a unique-owned
    // Box type — currently only iterators. `Tag::Channel` uses a
    // different representation (`OpaquePtr`) and has its own drop
    // model, so it is excluded for now.
    let dst_ty = func.var_type(*dst);
    let dst_resolved = pool.resolve_fully(dst_ty);
    let dst_tag = pool.tag(dst_resolved);
    matches!(dst_tag, Tag::Iterator | Tag::DoubleEndedIterator)
}

/// Collect variables defined by borrowing instructions (`Project`).
///
/// These create borrowed references that do NOT need independent RC
/// management — the source variable's RC covers the borrowed ref.
///
/// take-projects (`is_take_project`) are excluded — they
/// transfer ownership rather than borrow, so they must participate in
/// normal RC decisions for the projected payload.
pub(crate) fn collect_borrowed_defs(
    block: &ArcBlock,
    func: &ArcFunction,
    pool: &Pool,
) -> FxHashSet<ArcVarId> {
    let mut borrowed = FxHashSet::default();
    for instr in &block.body {
        if let ArcInstr::Project { dst, .. } = instr {
            if !is_take_project(instr, func, pool) {
                borrowed.insert(*dst);
            }
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

/// Collect variables projected from inline-enum sources (`Option`, `Result`, `Enum`).
///
/// These variables' RC is managed by the parent inline-enum's `RcDec` — no
/// separate per-field `RcDec` is needed. This prevents double-free when the
/// AIMS emits both inline-enum `RcDec` and per-field `RcDec` for the same value.
///
/// Includes transitive `Let` aliases (e.g., `%12 = %11` where `%11` is projected
/// from an `Option`).
///
/// take-projects (see `is_take_project`) are EXCLUDED. For a
/// take-project, the parent sum type has logically given up its payload
/// (suppresses its scope-exit drop), so the projected
/// iterator is no longer "managed by the parent" — it must participate
/// in its own RC lifecycle and drop at its own scope exit when unused.
/// Without this exclusion, `walk_dec`'s `emit_defined_dead` skips the
/// inline-enum-projected iterator unconditionally (treating it as a
/// borrow managed by the parent), and the iterator leaks.
pub(crate) fn collect_inline_enum_projected_defs(
    func: &ArcFunction,
    pool: &Pool,
) -> FxHashSet<ArcVarId> {
    use ori_types::Tag;

    let mut projected = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { dst, value, .. } = instr {
                // Take-projects transfer ownership; the projected var
                // is not managed by the parent and must get its own
                // drop. Skip it from the borrowed-by-parent set.
                if is_take_project(instr, func, pool) {
                    continue;
                }
                let src_ty = func.var_type(*value);
                let resolved = pool.resolve_fully(src_ty);
                let tag = pool.tag(resolved);
                if matches!(tag, Tag::Option | Tag::Result | Tag::Enum) {
                    projected.insert(*dst);
                }
            }
        }
    }
    propagate_borrowed_closure(func, &mut projected);
    projected
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
///
/// take-projects (`is_take_project`) are excluded — they
/// transfer ownership rather than borrow, so they must participate in
/// normal RC decisions for the projected payload.
///
/// take-project SOURCES are NOT added here. Adding the
/// backward alias chain was a function-global suppression and leaked
/// on paths that never executed the projection. Path-sensitive
/// suppression of source drops is handled by the separate
/// `take_project` must-analysis threaded through `BlockCtx` and
/// consumed by dead/edge cleanup.
pub(crate) fn collect_project_borrowed_defs(
    func: &ArcFunction,
    pool: &Pool,
) -> FxHashSet<ArcVarId> {
    let mut borrowed = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { dst, .. } = instr {
                if !is_take_project(instr, func, pool) {
                    borrowed.insert(*dst);
                }
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
///
/// take-projects (`is_take_project`) are excluded — they
/// transfer ownership of a unique-owned payload rather than borrow it.
///
/// take-project SOURCES are NOT added here. The backward
/// alias chain propagation (earlier attempt) was a function-global
/// suppression that leaked on paths that never executed the
/// projection. Path-sensitive suppression of the source enum's
/// scope-exit drop is handled by the separate `take_project`
/// must-analysis, threaded through `BlockCtx` and consumed by the
/// dead/edge cleanup sites.
pub(crate) fn collect_all_borrowed_defs(func: &ArcFunction, pool: &Pool) -> FxHashSet<ArcVarId> {
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
                if !is_take_project(instr, func, pool) {
                    borrowed.insert(*dst);
                }
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
/// 2. `Jump { target, args }` — when ALL predecessors pass borrowed values for
///    a block parameter, it inherits borrowed status
///
/// Rule 2 requires unanimity: a merge block param is only borrowed when every
/// predecessor's Jump arg at that position is borrowed. If ANY predecessor
/// brings an owned value (e.g., from Construct), the param stays owned so that
/// edge cleanup emits `RcDec` for it. The borrowed-path predecessors rely on
/// `emit_project_escape_incs` to add compensating `RcInc`.
fn propagate_borrowed_closure(func: &ArcFunction, borrowed: &mut FxHashSet<ArcVarId>) {
    // Pre-collect all Jump predecessors for each (target_block, param_position).
    // Key: (target_block_idx, param_pos) → Vec<Jump_arg_var>
    let mut param_incoming: rustc_hash::FxHashMap<(usize, usize), Vec<ArcVarId>> =
        rustc_hash::FxHashMap::default();
    for block in &func.blocks {
        if let ArcTerminator::Jump { target, args } = &block.terminator {
            let target_idx = target.index();
            if target_idx < func.blocks.len() {
                for (pos, &arg) in args.iter().enumerate() {
                    param_incoming
                        .entry((target_idx, pos))
                        .or_default()
                        .push(arg);
                }
            }
        }
    }

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
        }

        // Jump arg→param: only mark param borrowed when ALL incoming args are
        // borrowed. This prevents merge block params from being treated as
        // borrowed when some predecessors bring owned values (e.g., coalesce ??
        // where the Some path projects from Option and the None path constructs
        // a new value).
        for (&(target_idx, pos), incoming_args) in &param_incoming {
            let all_borrowed = incoming_args.iter().all(|arg| borrowed.contains(arg));
            if all_borrowed {
                if let Some(&(param_var, _)) = func.blocks[target_idx].params.get(pos) {
                    if borrowed.insert(param_var) {
                        changed = true;
                    }
                }
            }
        }
    }
}

/// Collect borrowed-parameter variables that are receivers of MUTATING COW
/// calls (push, set, insert, remove, etc.). Excludes `iter` — which takes
/// ownership of the buffer for iteration but never reallocs/frees it.
///
/// Only includes receivers with [`RcPointer`](crate::ir::ValueRepr::RcPointer)
/// representation (lists, maps, sets). String `add`/`concat` are borrowing
/// operations (not COW) — see `borrow/builtins/mod.rs` type-qualification docs.
pub(crate) fn collect_cow_borrowed_receivers(
    func: &ArcFunction,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    let cow_names = crate::borrow::all_cow_method_names(interner);
    // iter takes ownership but never mutates/frees — exclude from guard.
    let iter_name = interner.intern("iter");
    let param_borrowed = collect_param_borrowed_vars(func);
    if param_borrowed.is_empty() {
        return FxHashSet::default();
    }

    let mut result = FxHashSet::default();

    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                func: callee, args, ..
            } = instr
            {
                if *callee != iter_name && cow_names.contains(callee) && !args.is_empty() {
                    let receiver = args[0];
                    // COW semantics only apply to heap-pointer collections (lists, maps,
                    // sets). Strings are FatValue and use borrowing semantics for
                    // add/concat — including them here would emit invalid RcInc with
                    // HeapPointer strategy on a FatPointer variable.
                    if param_borrowed.contains(&receiver)
                        && func.var_repr(receiver) == Some(ValueRepr::RcPointer)
                    {
                        result.insert(receiver);
                    }
                }
            }
        }
        if let ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            if *callee != iter_name && cow_names.contains(callee) && !args.is_empty() {
                let receiver = args[0];
                if param_borrowed.contains(&receiver)
                    && func.var_repr(receiver) == Some(ValueRepr::RcPointer)
                {
                    result.insert(receiver);
                }
            }
        }
    }

    result
}

fn collect_param_borrowed_vars(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    crate::aims::emit_rc::queries::collect_param_borrowed_vars(func)
}
