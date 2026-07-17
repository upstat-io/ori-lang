//! Unwind cleanup for Invoke terminators.
//!
//! When a callee panics, iterator handles and unfinished for-yield scratch
//! lists created before the Invoke are not automatically cleaned up — the
//! unwind path must explicitly drop them. This pass adds
//! `Apply @ori_list_free(list, elem_size)` and `Apply @ori_iter_drop(iter)`
//! instructions to distinct unwind blocks before they resume propagation or
//! transfer control to an enclosing catch handler so that:
//!
//! 1. Iterator handles release their reference to the source collection.
//! 2. A scratch list releases its initialized elements and backing buffer.
//! 3. The `downgrade_trivial_invokes` pass in `merge_blocks()` won't
//!    collapse the Invoke into an `Apply` (losing the unwind path).
//! 4. Every physical projection preserves a cleanup edge; LLVM materializes
//!    a dedicated cleanup landing block for checked operations rather than
//!    turning a shared catch handler into an invalid mixed-predecessor pad.
//!
//! # Pipeline placement
//!
//! Runs after `emit_rc_ops()` / `realize_rc_reuse()` (RC instructions
//! present) and before `merge_blocks()` (which would downgrade the
//! Invoke). Specifically, it runs in `verify_and_merge()` right before
//! the `merge_blocks()` call.

use ori_types::Idx;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership};

#[derive(Clone, Copy)]
struct ProgramPoint {
    block: usize,
    instr: usize,
}

#[derive(Clone, Copy)]
struct YieldScratch {
    list: ArcVarId,
    elem_size: ArcVarId,
    created_at: ProgramPoint,
}

#[derive(Clone, Copy)]
struct CleanupNames {
    iter: ori_ir::Name,
    iter_drop: ori_ir::Name,
    list_new: ori_ir::Name,
    list_take: ori_ir::Name,
    list_free: ori_ir::Name,
}

#[derive(Default)]
struct ResourceEvents {
    iter_create: Vec<(ArcVarId, ProgramPoint)>,
    iter_drops: Vec<(ArcVarId, ProgramPoint)>,
    yield_scratch: Vec<YieldScratch>,
    scratch_consumes: Vec<(ArcVarId, ProgramPoint)>,
    invokes: Vec<(ProgramPoint, usize)>,
    definitions: FxHashMap<ArcVarId, ProgramPoint>,
}

struct LiveResources {
    scratch: Vec<(ArcVarId, ArcVarId)>,
    iters: Vec<ArcVarId>,
}

impl LiveResources {
    fn is_empty(&self) -> bool {
        self.scratch.is_empty() && self.iters.is_empty()
    }
}

/// Add resource cleanup to distinct Invoke and checked-operation unwind edges.
///
/// For each site, find every live compiler-owned iterator and unfinished
/// for-yield scratch list. Invoke destinations receive cleanup inline; checked
/// operations are retargeted through a synthesized cleanup-only landing block.
pub(crate) fn add_invoke_unwind_cleanup(func: &mut ArcFunction, interner: &ori_ir::StringInterner) {
    let names = cleanup_names(interner);
    let events = collect_resource_events(func, names);
    let successors = compute_block_successors(func);
    add_invoke_edge_cleanup(func, names, &events, &successors);
    add_checked_operation_cleanup(func, names, &events, &successors);
}

fn cleanup_names(interner: &ori_ir::StringInterner) -> CleanupNames {
    CleanupNames {
        iter: interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Iter.name()),
        iter_drop: interner.intern("ori_iter_drop"),
        list_new: interner.intern("ori_list_new"),
        list_take: interner.intern("ori_list_take"),
        list_free: interner.intern("ori_list_free"),
    }
}

fn collect_resource_events(func: &ArcFunction, names: CleanupNames) -> ResourceEvents {
    let mut events = ResourceEvents::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            let point = ProgramPoint {
                block: block_idx,
                instr: instr_idx,
            };
            if let Some(dst) = instr.defined_var() {
                events.definitions.insert(dst, point);
            }
            collect_instruction_event(&mut events, instr, point, names);
        }
        collect_invoke_event(&mut events, func.blocks.len(), block_idx, block);
    }
    events
}

fn collect_instruction_event(
    events: &mut ResourceEvents,
    instr: &ArcInstr,
    point: ProgramPoint,
    names: CleanupNames,
) {
    match instr {
        ArcInstr::Apply { dst, func, .. } if *func == names.iter => {
            events.iter_create.push((*dst, point));
        }
        ArcInstr::Apply { func, args, .. } if *func == names.iter_drop => {
            if let Some(&iter_var) = args.first() {
                events.iter_drops.push((iter_var, point));
            }
        }
        ArcInstr::Apply {
            dst, func, args, ..
        } if *func == names.list_new && args.len() >= 2 => {
            events.yield_scratch.push(YieldScratch {
                list: *dst,
                elem_size: args[1],
                created_at: point,
            });
        }
        ArcInstr::Apply { func, args, .. }
            if *func == names.list_take || *func == names.list_free =>
        {
            if let Some(&list) = args.first() {
                events.scratch_consumes.push((list, point));
            }
        }
        _ => {}
    }
}

fn collect_invoke_event(
    events: &mut ResourceEvents,
    block_count: usize,
    block_idx: usize,
    block: &ArcBlock,
) {
    if let ArcTerminator::Invoke { unwind, normal, .. }
    | ArcTerminator::InvokeIndirect { unwind, normal, .. } = &block.terminator
    {
        let unwind_idx = unwind.index();
        if unwind_idx < block_count && normal != unwind {
            events.invokes.push((
                ProgramPoint {
                    block: block_idx,
                    instr: block.body.len(),
                },
                unwind_idx,
            ));
        }
    }
}

fn add_invoke_edge_cleanup(
    func: &mut ArcFunction,
    names: CleanupNames,
    events: &ResourceEvents,
    successors: &[Vec<usize>],
) {
    for &(site, unwind_idx) in &events.invokes {
        let live = live_resources_at(events, successors, site);
        if live.is_empty() {
            continue;
        }
        let cleanup = build_cleanup_instrs(
            func,
            names.list_free,
            names.iter_drop,
            &live.scratch,
            &live.iters,
        );
        let span_count = cleanup.len();
        func.blocks[unwind_idx].body.extend(cleanup);
        func.spans[unwind_idx].extend(std::iter::repeat_n(None, span_count));

        tracing::debug!(
            invoke_block = site.block,
            unwind_block = unwind_idx,
            live_iters = live.iters.len(),
            live_yield_scratch = live.scratch.len(),
            "added resource cleanup to invoke unwind block"
        );
    }
}

fn add_checked_operation_cleanup(
    func: &mut ArcFunction,
    names: CleanupNames,
    events: &ResourceEvents,
    successors: &[Vec<usize>],
) {
    let checked_ops = func.catch_scoped_checked_ops.clone();
    for (metadata_idx, (checked_var, catch_handler)) in checked_ops.into_iter().enumerate() {
        let site = *events.definitions.get(&checked_var).unwrap_or_else(|| {
            panic!(
                "catch-scoped checked op v{} has no defining instruction",
                checked_var.raw()
            )
        });
        assert!(
            catch_handler.index() < func.blocks.len(),
            "catch-scoped checked op v{} targets missing block {}",
            checked_var.raw(),
            catch_handler.raw()
        );
        assert!(
            func.blocks[catch_handler.index()].params.is_empty(),
            "catch handler block {} must not require jump arguments",
            catch_handler.raw()
        );
        let live = live_resources_at(events, successors, site);
        let body = build_cleanup_instrs(
            func,
            names.list_free,
            names.iter_drop,
            &live.scratch,
            &live.iters,
        );
        let landing = func.next_block_id();
        func.push_block(ArcBlock {
            id: landing,
            params: Vec::new(),
            body,
            terminator: ArcTerminator::Jump {
                target: catch_handler,
                args: Vec::new(),
            },
        });
        func.catch_scoped_checked_ops[metadata_idx].1 = landing;

        tracing::debug!(
            checked_var = checked_var.raw(),
            checked_block = site.block,
            landing_block = landing.raw(),
            catch_block = catch_handler.raw(),
            live_iters = live.iters.len(),
            live_yield_scratch = live.scratch.len(),
            "retargeted checked-op unwind through cleanup landing block"
        );
    }
}

/// Find compiler-owned resources created but not consumed before `site`.
fn live_resources_at(
    events: &ResourceEvents,
    successors: &[Vec<usize>],
    site: ProgramPoint,
) -> LiveResources {
    let dropped_iters: FxHashSet<ArcVarId> = events
        .iter_drops
        .iter()
        .filter(|&&(_, point)| event_precedes_site(successors, point, site))
        .map(|&(var, _)| var)
        .collect();
    let iters = events
        .iter_create
        .iter()
        .filter(|&&(var, point)| {
            event_precedes_site(successors, point, site) && !dropped_iters.contains(&var)
        })
        .map(|&(var, _)| var)
        .collect();

    let consumed_scratch: FxHashSet<ArcVarId> = events
        .scratch_consumes
        .iter()
        .filter(|&&(_, point)| event_precedes_site(successors, point, site))
        .map(|&(list, _)| list)
        .collect();
    let scratch = events
        .yield_scratch
        .iter()
        .filter(|scratch| {
            event_precedes_site(successors, scratch.created_at, site)
                && !consumed_scratch.contains(&scratch.list)
        })
        .map(|scratch| (scratch.list, scratch.elem_size))
        .collect();

    LiveResources { scratch, iters }
}

/// Build cleanup in reverse allocation order: scratch outputs are created
/// after iterator state and therefore release before the iterator handle.
fn build_cleanup_instrs(
    func: &mut ArcFunction,
    list_free_name: ori_ir::Name,
    iter_drop_name: ori_ir::Name,
    live_scratch: &[(ArcVarId, ArcVarId)],
    live_iters: &[ArcVarId],
) -> Vec<ArcInstr> {
    let Some(cleanup_capacity) = live_scratch.len().checked_add(live_iters.len()) else {
        panic!("unwind cleanup resource count must fit usize");
    };
    let mut cleanup = Vec::with_capacity(cleanup_capacity);

    for &(list, elem_size) in live_scratch.iter().rev() {
        cleanup.push(ArcInstr::Apply {
            dst: func.fresh_scalar_var(Idx::UNIT),
            ty: Idx::UNIT,
            func: list_free_name,
            args: vec![list, elem_size],
            arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Borrowed],
            mono_instance_id: None,
        });
    }

    // `ori_iter_drop` consumes the iterator handle. Its ownership marker must
    // match `ProtocolBuiltin::IterDrop.arg_ownership()` rather than becoming a
    // second, unwind-only source of truth.
    for &iter in live_iters.iter().rev() {
        cleanup.push(ArcInstr::Apply {
            dst: func.fresh_scalar_var(Idx::UNIT),
            ty: Idx::UNIT,
            func: iter_drop_name,
            args: vec![iter],
            arg_ownership: vec![ArgOwnership::Owned],
            mono_instance_id: None,
        });
    }

    cleanup
}

/// Whether an event occurs before a cleanup site under the pass's existing
/// forward-reachability model. Instruction coordinates make checked operations
/// precise when creation/finalization and the checked op share one ARC block.
fn event_precedes_site(successors: &[Vec<usize>], event: ProgramPoint, site: ProgramPoint) -> bool {
    if event.block == site.block {
        return event.instr < site.instr;
    }
    can_reach(successors, event.block, site.block)
}

/// Build a successor list for each block from the function's terminators.
fn compute_block_successors(func: &ArcFunction) -> Vec<Vec<usize>> {
    func.blocks
        .iter()
        .map(|block| {
            crate::graph::successor_block_ids(&block.terminator)
                .iter()
                .map(|id| id.index())
                .collect()
        })
        .collect()
}

/// Check if `from` can reach `to` via forward edges (BFS).
fn can_reach(successors: &[Vec<usize>], from: usize, to: usize) -> bool {
    if from == to {
        return true;
    }
    let mut visited = FxHashSet::default();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(from);
    visited.insert(from);
    while let Some(current) = queue.pop_front() {
        for &succ in &successors[current] {
            if succ == to {
                return true;
            }
            if visited.insert(succ) {
                queue.push_back(succ);
            }
        }
    }
    false
}

#[cfg(test)]
mod tests;
