//! Phase A and dead Invoke result cleanup for RC emission.
//!
//! - **Phase A** ([`emit_dead_at_entry_decs`]): emits `RcDec` for variables
//!   that are live at block entry, not used anywhere in the block, and dead
//!   at block exit.
//! - **Dead Invoke sweep** ([`emit_dead_invoke_dsts`]): scans Invoke
//!   terminators for result variables that escaped Phase A–C entirely (their
//!   backward demand was never propagated) and prepends the missing `RcDec`
//!   into the normal successor block.

use rustc_hash::FxHashSet;

use ori_types::Pool;

use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::Cardinality;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, RcStrategy};

use super::helpers::{is_live_at_exit, is_owned_at_entry, BlockCtx, LastUse};
use super::{block_id, rc_strategy};

pub(crate) mod emission_site;
use emission_site::{canonical_rep_for, pin4_class_emits_dec_set, Pin4EmitsByClass};

/// Phase A: `RcDec` for variables live at entry, unused in block, dead at exit.
///
/// Two sources of dead-at-entry variables:
/// 1. Variables tracked by the backward analysis (`entry_states`) that have
///    non-Absent cardinality but are unused and dead at exit.
/// 2. Block parameters that generated no backward demand (absent from
///    `entry_states` entirely) — e.g., mutable-scope variables threaded
///    through loop exit blocks but never actually read. These are `RcPtr`
///    parameters that need cleanup even though the analysis didn't track them.
///
/// Returns `(deferred_parents, merge_edge_decs)`.
#[expect(
    clippy::type_complexity,
    reason = "two-part return: deferred parents + merge-edge decs"
)]
pub(crate) fn emit_dead_at_entry_decs(
    ctx: &BlockCtx<'_>,
    new_body: &mut Vec<ArcInstr>,
) -> (
    Vec<(ArcVarId, RcStrategy, LastUse)>,
    Vec<(ArcVarId, RcStrategy)>,
) {
    let mut deferred_parents = Vec::new();
    let mut merge_edge_decs = Vec::new();

    // Precompute predecessor count for merge-block filtering.
    let predecessors = crate::graph::compute_predecessors(ctx.func);
    let pred_count = predecessors.get(ctx.blk.index()).map_or(0, Vec::len);

    // BUG-04-090 §05 Step 8: pre-compute whether this block is an unwind
    // block. Unwind paths always emit cleanup decs per `arc.md §RL-4`; the
    // suppression helper short-circuits when `is_unwind_block = true`.
    let is_unwind_block = matches!(
        ctx.func.blocks[ctx.blk.index()].terminator,
        crate::ir::ArcTerminator::Resume
    );
    let is_block_param = |v: ArcVarId| -> bool {
        ctx.func.blocks[ctx.blk.index()]
            .params
            .iter()
            .any(|&(p, _)| p == v)
    };

    // track which
    // Let-alias groups have already received a bypass-safe scope-exit
    // dec in this block. Multiple Let-alias siblings (e.g., `%5` and
    // its `Let` alias `%19`) in `entry_states` share the same runtime
    // value — emitting one RcDec per alias would double-free.
    //
    // Dedup is per-LET-ALIAS-REPRESENTATIVE, not per-lineage-index.
    // Two vars share a Let-alias rep iff connected by a chain of
    // `Let { dst, Var(src) }` instructions (true SSA aliases — same
    // runtime value). Phi-merged block params that share the same
    // lineage source set but hold DIFFERENT runtime values get
    // DIFFERENT Let-alias reps and thus separate RcDecs — this is
    // the fix.
    let mut let_reps_dec_emitted: FxHashSet<ArcVarId> = FxHashSet::default();

    // BUG-04-111 §05 Step 2: pre-compute the per-class emission map for this
    // block. Consumed by the PIN-4 canonical-rep query at Source 1 (below)
    // and at Source 2 (`emit_dead_block_param_decs` — passed in).
    let pin4_emits: Pin4EmitsByClass = pin4_class_emits_dec_set(ctx, is_unwind_block);

    // Source 1: variables in entry_states.
    if let Some(entry_states) = ctx.state_map.block_entry_states(ctx.blk) {
        for (&var, &state) in entry_states {
            if state.is_scalar() || state.cardinality == Cardinality::Absent {
                continue;
            }
            // Use is_owned_at_entry to handle cross-block variables whose
            // access dimension is stuck at BOTTOM (Borrowed) due to backward
            // demand propagation not updating access.
            if !is_owned_at_entry(
                ctx.state_map,
                ctx.blk,
                var,
                ctx.defined_in_block,
                ctx.borrowed_defs,
                ctx.all_borrowed_defs,
            ) {
                continue;
            }
            // take-project alias-class
            // members get a single scope-exit drop at the entry edge
            // of the per-class bypass-safe region.
            //
            // A block is BYPASS-SAFE for a class iff it is NEITHER
            // forward- nor backward-reachable from any take-project
            // in that class. A block is a BYPASS-SAFE ENTRY iff it
            // is bypass-safe AND at least one of its CFG predecessors
            // is NOT bypass-safe — i.e., it is the first block on a
            // CFG path where the source enum becomes definitively
            // unreachable from the take-project. The dec is emitted
            // exactly here: downstream bypass-safe blocks inherit the
            // drop via SSA flow and emitting at every bypass-safe
            // block would produce N duplicate decs for sequential
            // bypass-safe regions, double-freeing the shared payload.
            //
            // The check is per-class (not function-global): a block
            // can be a bypass-safe entry for class `A` while being
            // reachable from an unrelated class `B`. Per-class
            // partitioning is what added on top of the
            // initial fix.
            //
            // The check fires BEFORE `use_info`/`is_live_at_exit`
            // because alias-chain "uses" on bypass-safe blocks are
            // necessarily SSA-only (Let alias / Jump-arg propagation)
            // — they don't dereference the value. The dec walks the
            // tagged-pointer encoding (`ori_iter_drop` on the
            // payload) without invalidating the source variable's
            // bit pattern, so subsequent alias reads stay safe.
            // Edge cleanup (`collect_branch_edge_decs` /
            // `collect_invoke_edge_decs`) is taught via
            // `take_move_facts.is_in_class` to skip in-class vars
            // entirely, so it never produces a duplicate dec.
            //
            // Per-Let-alias dedup: only the FIRST
            // variable of each Let-alias group encountered in
            // entry_states gets a dec — emitting for subsequent Let
            // aliases (e.g., `%5` then its `Let` alias `%19`) would
            // double-free the same underlying value. Phi-merged params
            // with the same lineage but different Let-alias reps get
            // separate decs (they hold different values at runtime).
            if ctx
                .take_move_facts
                .is_bypass_safe_entry_for_var(var, ctx.blk.index())
            {
                // `let_alias_rep` is total for in-class vars (returns
                // `Some(var)` for singletons), so `if let` is
                // defensive only.
                if let Some(rep) = ctx.take_move_facts.let_alias_rep(var) {
                    if let_reps_dec_emitted.insert(rep) {
                        if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                            // BUG-04-090 §05 Step 8: suppress dec on params
                            // that flow to a Return on this path.
                            if !super::should_suppress_return_transfer_dec(
                                ctx,
                                var,
                                ctx.blk,
                                is_unwind_block,
                            ) {
                                new_body.push(ArcInstr::RcDec { var, strategy });
                            }
                        }
                    }
                }
                continue;
            }
            // In-class but NOT bypass-safe-entry: the dec is handled
            // either upstream at the bypass-safe-region entry, or by
            // the take-project's `is_ownership_transfer` at the
            // `Project` site. Skip here.
            if ctx.take_move_facts.is_in_class(var) {
                continue;
            }
            if ctx.use_info.contains_key(&var) || is_live_at_exit(ctx.state_map, ctx.blk, var) {
                continue;
            }
            // Merge-block filter: at a block with >1 predecessor, variables
            // that are NOT block params may come from project-source demand
            // propagation and exist only on some predecessor paths. Emitting
            // a block-level RcDec for them would fire on paths where they
            // don't exist (double-free or undefined-var skip). Check that
            // the variable is defined in ALL predecessor blocks (or received
            // as their block param); if not, skip — the defining predecessor's
            // forward walk handles cleanup via the deferred parent mechanism.
            if pred_count > 1 && !is_block_param(var) && !ctx.defined_in_block.contains(&var) {
                let all_preds_define_it = predecessors[ctx.blk.index()]
                    .iter()
                    .all(|&pred_idx| ctx.func.blocks[pred_idx].defines_var(var));
                if !all_preds_define_it {
                    // Route to per-predecessor edge cleanup instead of
                    // block-level RcDec. The edge cleanup will use
                    // trampolines to emit RcDec only on the edges where
                    // the variable actually exists.
                    if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                        merge_edge_decs.push((var, strategy));
                    }
                    continue;
                }
            }
            // Defer if this variable has live borrowed children (projections
            // or their aliases still used in this block). The deferred dec is
            // seeded into the forward walk, which emits it after the children die.
            if let Some(&child_last) = ctx.child_effective_last_use.get(&var) {
                if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                    deferred_parents.push((var, strategy, child_last));
                }
                continue;
            }
            // BUG-04-090 §05 Hypothesis D component #4 (Session H wiring):
            // suppress dec on vars whose ownership was transferred to an
            // Apply/Invoke result via `apply_result_aliases`. Without this
            // gate, downstream block-entry dead-cleanup emits decs on
            // already-consumed args (helper at edge_cleanup wires this for
            // its sites; emit_dead_at_entry_decs is the missing consumer).
            // Helper filters Let-Var aliases out at population time, so the
            // correct dec on a Let-alias of the result still fires via the
            // body walk's last-use path.
            if super::should_suppress_apply_aliased_dec(ctx.state_map, var, is_unwind_block) {
                continue;
            }
            // BUG-04-111 §05 Step 3 (replaces BUG-04-104 PIN-4): canonical-rep
            // selection per gemini Round 2 F1's latest-emission-site rule.
            // Suppress this var's dec if a different class member is the
            // canonical-rep emitter (per `pin4_class_emits_dec_set` SSOT).
            // Pre-fix logic checked "any other class member is USED in body
            // OR live at exit" — but a use is a borrow that emits no dec, so
            // the suppression fired even when no class member would actually
            // emit, leaving the dec unbalanced (BUG-04-111 leak shape).
            if let Some(class_id) = ctx.state_map.ssa_alias_class_of(var) {
                if let Some(canonical) = canonical_rep_for(class_id, &pin4_emits) {
                    if canonical != var {
                        continue;
                    }
                }
            }
            if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                if !super::should_suppress_return_transfer_dec(ctx, var, ctx.blk, is_unwind_block) {
                    new_body.push(ArcInstr::RcDec { var, strategy });
                }
            }
        }
    }

    // Source 2: block parameters absent from entry_states.
    emit_dead_block_param_decs(ctx, new_body, &pin4_emits);

    (deferred_parents, merge_edge_decs)
}

/// Emit `RcDec` for block parameters that generated no backward demand
/// (never used in this block or any successor) but still carry RC-managed
/// values (e.g., mutable-scope list variables passed through loop exit
/// blocks). Variables that the take-project dataflow proves moved are
/// skipped per In-class block params are SKIPPED entirely
/// rather than routed: they are SSA aliases of the take-project source
/// (via Jump-arg → block-param propagation), and the source enum's
/// natural scope-exit drops in the predecessors already handle cleanup.
/// Routing the param's `ArcVarId` to a predecessor would emit an `RcDec`
/// using a name that has no SSA definition reachable from the
/// predecessor — the LLVM emitter resolves the param ID to the merge
/// block's phi node, producing a phi-dominance violation.
#[expect(
    clippy::cognitive_complexity,
    reason = "pre-existing; refactoring would risk RC emission regressions"
)]
fn emit_dead_block_param_decs(
    ctx: &BlockCtx<'_>,
    new_body: &mut Vec<ArcInstr>,
    pin4_emits: &Pin4EmitsByClass,
) {
    let entry_states = ctx.state_map.block_entry_states(ctx.blk);
    let block = &ctx.func.blocks[ctx.blk.index()];
    // BUG-04-090 §05 Hypothesis D component #4 (Session H wiring):
    // unwind-block awareness for the apply-aliased gate. Resume blocks
    // always emit cleanup decs per `arc.md §RL-4`.
    let is_unwind_block = matches!(block.terminator, crate::ir::ArcTerminator::Resume,);

    // BUG-04-104 PIN-6 (§2.6.3) cross-param same-emission: when MULTIPLE block
    // params land here as candidates for canonical dec, they all fire at the
    // same conceptual boundary (block entry). PIN-6's same_emission check
    // needs to see all classes whose params will fire here so a child class's
    // dec is correctly suppressed when its transitive-drop parent class is
    // ALSO firing at this same boundary. Pre-compute the set BEFORE the per-
    // param loop runs (mirrors the cross-deferred fix in walk.rs).
    let pin6_classes_dying_here: rustc_hash::FxHashMap<
        u32,
        rustc_hash::FxHashSet<crate::ir::ArcVarId>,
    > = {
        let mut m = rustc_hash::FxHashMap::default();
        for &(param_var, _param_ty) in &block.params {
            if entry_states.is_some_and(|es| es.contains_key(&param_var)) {
                continue;
            }
            if ctx.state_map.is_excluded(param_var) {
                continue;
            }
            if ctx.use_info.contains_key(&param_var) {
                continue;
            }
            if is_live_at_exit(ctx.state_map, ctx.blk, param_var) {
                continue;
            }
            // Apply ALL same gates as the per-param emission loop. Without
            // these, classes whose params would actually be FILTERED OUT
            // (apply-aliased, take-move, iter-element, PIN-4-skipped) are
            // incorrectly recorded as "will dec here", causing PIN-6 to
            // suppress legitimate child decs whose parents won't actually
            // fire → leak.
            if ctx.iter_element_defs.contains(&param_var) {
                continue;
            }
            if ctx.take_move_facts.is_in_class(param_var) {
                continue;
            }
            if super::should_suppress_apply_aliased_dec(ctx.state_map, param_var, is_unwind_block) {
                continue;
            }
            if let Some(class_id) = ctx.state_map.ssa_alias_class_of(param_var) {
                if let Some(members) = ctx.state_map.class_members(class_id) {
                    if members.iter().any(|&mem| {
                        mem != param_var
                            && (ctx.use_info.contains_key(&mem)
                                || crate::aims::emit_rc::is_live_at_exit(
                                    ctx.state_map,
                                    ctx.blk,
                                    mem,
                                ))
                    }) {
                        continue;
                    }
                }
            }
            if let Some(class_id) = ctx.state_map.ssa_alias_class_of(param_var) {
                m.entry(class_id)
                    .or_insert_with(rustc_hash::FxHashSet::default)
                    .insert(param_var);
            }
        }
        m
    };

    for &(param_var, _param_ty) in &block.params {
        // Skip if already handled by Source 1.
        if entry_states.is_some_and(|es| es.contains_key(&param_var)) {
            continue;
        }
        // Skip scalars and excluded variables.
        if ctx.state_map.is_excluded(param_var) {
            continue;
        }
        // Skip if the parameter is actually used in this block.
        if ctx.use_info.contains_key(&param_var) {
            continue;
        }
        // Skip if live at exit (used in successor blocks).
        if is_live_at_exit(ctx.state_map, ctx.blk, param_var) {
            continue;
        }
        // Skip iterator-element variables. These are borrowed from the
        // collection buffer — elem_dec_fn handles cleanup when the
        // collection is freed. Without this check, mutable-scope
        // threading of borrowed elements through inner loop exit blocks
        // generates spurious RcDec on the block param.
        if ctx.iter_element_defs.contains(&param_var) {
            continue;
        }
        // take-project must-move handling for block params.
        //
        // Skip in-class block params entirely. A block param that
        // belongs to a take-project alias class is just an SSA name
        // for whichever incoming Jump arg (also in the class) flows
        // through this merge. The source enum's actual scope-exit
        // drops are emitted at natural death sites in the
        // predecessors (e.g., the non-projecting branch's
        // `RcDec %5`), and the take-project at its `Project` site
        // is already an `is_ownership_transfer` so its source isn't
        // re-dropped. Emitting another drop here would
        // be a redundant double-free on the bypass path.
        if ctx.take_move_facts.is_in_class(param_var) {
            continue;
        }
        // BUG-04-090 §05 Hypothesis D component #4 (Session H wiring):
        // suppress dec on block params whose ownership was transferred
        // to an Apply/Invoke result via `apply_result_aliases`. Same
        // rationale as the Source 1 gate above.
        if super::should_suppress_apply_aliased_dec(ctx.state_map, param_var, is_unwind_block) {
            continue;
        }
        // BUG-04-111 §05 Step 4 (replaces BUG-04-104 PIN-4): canonical-rep
        // selection consuming the SSOT `pin4_class_emits_dec_set`. Same
        // rationale as Source 1's refactor — see §05 Step 3 comment in
        // `emit_dead_at_entry_decs`.
        if let Some(class_id) = ctx.state_map.ssa_alias_class_of(param_var) {
            if let Some(canonical) = canonical_rep_for(class_id, pin4_emits) {
                if canonical != param_var {
                    continue;
                }
            }
        }
        // BUG-04-104 PIN-6 (§2.6.3): inter-class payload-of suppression at
        // the dead-block-param emission site, restricted to SAME-EMISSION
        // only (NOT alive-after). The dead-block-param case fires at block
        // entry; any "alive_after" parent class member is just "used in the
        // block somewhere", which doesn't guarantee its transitive drop
        // covers this child's RC slot at this specific boundary. Restricting
        // to same-emission means: only suppress when the parent class is
        // ALSO firing a dead-block-param dec at this exact boundary. This
        // is the canonical apply_alias_result_strmap shape (multiple block
        // params, one in each of class A and class B, all dying here, B's
        // transitive-drop covering A's slot via the same Phase-A emission).
        // Additional gate: require ≥2 distinct classes firing at this
        // boundary (the apply_alias_result_strmap shape with simultaneous
        // parent + child decs). Single-class cases don't have double-
        // emission risk and would over-suppress legitimate decs (e.g.,
        // generic forwarder cases where the parent's drop is the ONLY dec).
        if pin6_classes_dying_here.len() >= 2 {
            if let Some(class_id) = ctx.state_map.ssa_alias_class_of(param_var) {
                if pin6_same_emission_covers(ctx, class_id, param_var, &pin6_classes_dying_here) {
                    continue;
                }
            }
        }
        if let Some(strategy) = rc_strategy(ctx.func, param_var, ctx.pool) {
            new_body.push(ArcInstr::RcDec {
                var: param_var,
                strategy,
            });
        }
    }
}

/// PIN-6 same-emission-only check for the dead-block-param emission site.
///
/// Returns true iff some ancestor class B (transitively reachable from
/// `class_id` via `class_payload_of`) has transitive-drop strategy AND is
/// in `classes_dying_here`. Distinct from
/// [`crate::aims::realize::walk_dec::pin6_any_ancestor_will_cover`]: that
/// predicate ALSO accepts `alive_after` as a sufficient condition. The
/// `alive_after` check is too loose for dead-block-param emission — a parent
/// class member being "alive somewhere in the block" doesn't guarantee its
/// transitive drop covers the child's RC slot at this specific block-entry
/// boundary, leading to over-suppression and leaks (BUG-04-104 regression
/// fix on `fat_ptr_iter` / generics tests).
///
/// Restricting to same-emission preserves the canonical apply-alias case
/// (multiple block params dying at the same boundary, parent's drop covering
/// child's slot via the same Phase-A emission group) while avoiding false
/// suppression when the alive parent member's drop fires elsewhere.
fn pin6_same_emission_covers(
    ctx: &super::helpers::BlockCtx<'_>,
    class_id: u32,
    var: crate::ir::ArcVarId,
    classes_dying_here: &rustc_hash::FxHashMap<u32, rustc_hash::FxHashSet<crate::ir::ArcVarId>>,
) -> bool {
    let mut visited: rustc_hash::FxHashSet<u32> = rustc_hash::FxHashSet::default();
    let mut queue: std::collections::VecDeque<u32> = std::collections::VecDeque::new();
    let existing_parents = ctx.state_map.class_payload_of(class_id);
    if let Some(parents) = existing_parents {
        queue.extend(parents.iter().copied());
    }
    // BUG-04-118 §05 — Project-alias seed (companion to walk_dec.rs).
    // Narrowed trigger matches body walker: class_payload_of empty AND
    // var is the dead-block-param being decremented. Strategy gate at
    // line 484 (InlineEnum-only) preserved. PIN-2 preserved (no class
    // merge). See walk_dec.rs::pin6_any_ancestor_will_cover for the full
    // soundness rationale.
    if existing_parents.is_none() {
        let is_singleton = match ctx.state_map.class_members(class_id) {
            Some(members) => members.len() == 1,
            None => class_id == var.raw(),
        };
        if is_singleton {
            if let Some(sources) = ctx.state_map.project_alias_sources().get(&var) {
                for &source_var in sources {
                    if let Some(source_class) = ctx.state_map.ssa_alias_class_of(source_var) {
                        queue.push_back(source_class);
                        if let Some(source_parents) = ctx.state_map.class_payload_of(source_class) {
                            queue.extend(source_parents.iter().copied());
                        }
                    }
                }
            }
        }
    }
    while let Some(parent_class) = queue.pop_front() {
        if !visited.insert(parent_class) {
            continue;
        }
        if classes_dying_here.contains_key(&parent_class) {
            // Verify parent's strategy is transitive-drop. Resolve via
            // class_members representative (singleton-class invariant per
            // §2.6.2 ensures class_members has an entry).
            //
            // Strategy gate: dead-block-param scope intentionally narrower than
            // body-level pin6_any_ancestor_will_cover (which uses the full
            // is_transitive_drop_strategy predicate). HeapPointer / Closure /
            // AggregateFields coverage at the dead-block-param boundary is the
            // forward-looking strategy expansion deferred to the §1.2 row 9
            // helper-split anchor; broadening here without the matching matrix
            // coverage would ship unverified behavior.
            if let Some(members) = ctx.state_map.class_members(parent_class) {
                if let Some(&parent_rep) = members.iter().next() {
                    if let Some(parent_strategy) = rc_strategy(ctx.func, parent_rep, ctx.pool) {
                        if matches!(parent_strategy, crate::ir::RcStrategy::InlineEnum) {
                            return true;
                        }
                    }
                }
            }
        }
        // Continue chain walk for nested transitive-drop coverage.
        if let Some(gps) = ctx.state_map.class_payload_of(parent_class) {
            queue.extend(gps.iter().copied());
        }
    }
    false
}

/// Sweep for dead Invoke result variables across all blocks.
///
/// An Invoke terminator's `dst` variable is "born" at the edge between the
/// predecessor (where the Invoke lives) and the successor blocks. The backward
/// analysis may never see demand for this variable (e.g., if the result is
/// unused or only used in the defining block), causing it to be absent from
/// all entry/exit state maps. Phases A–C only process variables they find in
/// `entry_states` or `use_info`, so they miss these orphaned definitions entirely.
///
/// This function scans every Invoke/InvokeIndirect terminator and checks whether its `dst`
/// variable has an `RcDec` in the normal successor block. If not, and the
/// variable is an `RcPointer`, it prepends one. The same check is done for
/// the unwind successor.
pub(crate) fn emit_dead_invoke_dsts(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
    all_borrowed_defs: &FxHashSet<ArcVarId>,
) {
    // Collect (successor_block_idx, var, strategy) tuples to avoid borrowing
    // func mutably while iterating.
    let mut pending_decs = Vec::new();

    for block in &func.blocks {
        let (ArcTerminator::Invoke { dst, normal, .. }
        | ArcTerminator::InvokeIndirect { dst, normal, .. }) = &block.terminator
        else {
            continue;
        };

        // Skip scalars and borrowed defs.
        if state_map.is_excluded(*dst) || all_borrowed_defs.contains(dst) {
            continue;
        }

        let Some(strategy) = rc_strategy(func, *dst, pool) else {
            continue;
        };

        // Only check the NORMAL successor. On the unwind path, the Invoke's
        // dst variable is never populated (the call threw), so decrementing it
        // would be a use-after-free of uninitialized memory.
        let succ_idx = normal.index();
        if succ_idx >= func.blocks.len() {
            continue;
        }

        let has_dec = func.blocks[succ_idx]
            .body
            .iter()
            .any(|instr| matches!(instr, ArcInstr::RcDec { var, .. } if *var == *dst));
        if has_dec {
            continue;
        }

        // Skip if the variable is used in the successor (Phase B/C
        // will have emitted appropriate RcDec already).
        let used_in_succ = func.blocks[succ_idx]
            .body
            .iter()
            .any(|instr| instr.used_vars().contains(dst))
            || func.blocks[succ_idx].terminator.used_vars().contains(dst);
        if used_in_succ {
            continue;
        }

        // Skip if the variable is live at the exit of the successor block
        // (it's still needed in downstream blocks).
        let succ_blk = block_id(succ_idx);
        if is_live_at_exit(state_map, succ_blk, *dst) {
            continue;
        }

        pending_decs.push((succ_idx, *dst, strategy));
    }

    // Prepend RcDec at the start of each successor block.
    for (succ_idx, var, strategy) in pending_decs {
        func.blocks[succ_idx]
            .body
            .insert(0, ArcInstr::RcDec { var, strategy });
    }
}

#[cfg(test)]
mod tests;
