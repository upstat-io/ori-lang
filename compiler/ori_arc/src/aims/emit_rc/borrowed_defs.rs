//! Borrowed-definition collection for RC emission.
//!
//! Identifies variables whose RC is managed by a parent aggregate or iterator,
//! suppressing independent `RcDec` for borrowed views. Also collects
//! COW-borrowed receiver variables that need pre-call `RcInc` guards.

use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

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

    // Find all Apply @__iter_next calls. Collect:
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

    // Find Project at field index 1 from __iter_next results. This is the
    // yielded element (field 0 is the Option tag).
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

    // Joint fixpoint: transitive Project chains INTERLEAVED with Let-alias /
    // block-param propagation. A compound source element
    // (`[Option<str>]` / `[Item]` / `[[int]]`) yields an iter-element-view whose
    // INTERIOR sub-value the body projects out (`match opt { Some(s) -> .. }` =>
    // `Project (variant payload).1`; `item.name` => `Project (struct).0`; the
    // inner list of a nested loop => `Project (outer elem).1`). That nested
    // projection's SOURCE often reaches the iter-element set only through a
    // Let-alias hop (`%view = %iter_elem; %inner = Project %view.field`), so a
    // Project-chain pass that completes BEFORE the Let-alias pass misses it ->
    // the interior view slips through and the burden walk emits a spurious
    // `BurdenDec` on a BORROW into the collection buffer -> double-free (the
    // source's `elem_dec_fn` already frees the interior element via
    // `ori_iter_drop` / `CollectSet`). The two propagation kinds must reach a
    // SINGLE fixpoint together: each Project-chain step can expose a new alias
    // source and each alias step can expose a new Project source. Both kinds
    // monotonically GROW the borrow-view set (RL-2: "Borrowed variables do NOT
    // receive decs"; the interior projection of a borrowed compound element is
    // itself borrowed). Spec: Annex E §AIMS Protocol Builtins + RL-2.
    loop {
        let prev_len = iter_elems.len();
        // Project-chain step: a Project of any member is a member.
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Project { dst, value, .. } = instr {
                    if iter_elems.contains(value) {
                        iter_elems.insert(*dst);
                    }
                }
            }
        }
        // Let-alias / block-param step.
        propagate_borrowed_closure(func, &mut iter_elems, &FxHashSet::default());
        if iter_elems.len() == prev_len {
            break;
        }
    }
    iter_elems
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
///
/// `move_out_args` are Jump args that transfer ownership to the successor param
/// (tagless-enum full-payload move-outs per `tagless_move_out_projections`,
/// RL-2). They break unanimity exactly like a `Construct` arg: a param fed such
/// an arg is OWNED and gets its own dec. Pass an empty set to disable.
fn propagate_borrowed_closure(
    func: &ArcFunction,
    borrowed: &mut FxHashSet<ArcVarId>,
    move_out_args: &FxHashSet<ArcVarId>,
) {
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
            let all_borrowed = incoming_args
                .iter()
                .all(|arg| borrowed.contains(arg) && !move_out_args.contains(arg));
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
