//! RL-2 move-alias transfer-suppression analysis feeding `emit_burden_ops`:
//! the function-wide fixpoint that propagates ownership-transfer through
//! `Let { Var }` move-alias chains so the move source's last-use `BurdenDec`
//! is suppressed.

use ori_types::TypeRegistry;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::emit_rc::block_id;
use crate::aims::intraprocedural::state_map::ApplyAliasSource;
use crate::graph::DominatorTree;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};
use crate::lower::burden::{Burden, TypeRef};
use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

use super::{instr_transfer_vars, successor_reachable_blocks};

/// Compute the move-alias transfer-suppression set per AIMS RL-2.
///
/// Seed = every var transferred out at a terminator (`terminator_transfer`)
/// plus every var consumed at an owned instruction position. A value reaching
/// one of those transfer points THROUGH a `Let { Var }` move-alias chain
/// (`%dst = %src` where the alias is `%src`'s LAST use) also transfers out: that
/// terminal use forwards `%src`'s remaining ownership to `%dst`. Backward-
/// propagate: for every move-alias whose `dst` is in the set, add `src`. Iterate
/// to a fixpoint so multi-hop chains (`%2 = %1; %3 = %2; Return %3`) propagate.
/// The returned set suppresses the last-use `BurdenDec` of every move source in
/// a transfer chain.
///
/// Use-once vs dup'd-terminal-move gate. A use-once source (`use_counts == 1`)
/// is the unchanged pure-move case: its sole use forwards its only reference, so
/// its last-use dec is suppressed (the consumer discharges the release). A DUP'd
/// source (`%s` used >= 2 — earlier uses each consume a duplicate reference)
/// ALSO forwards its ORIGINAL allocation reference at its TERMINAL `Let { Var }`
/// use; emitting that terminal `BurdenDec` releases a reference the move hands to
/// `%dst`'s consuming lineage (RL-2 net=-1, an early over-release that collapses
/// a COW receiver's RC below the LIVE alias count — witness:
/// `let a; let b = a; let c = a.updated(..)` with `b` LIVE decs `a` before the
/// consuming `updated`, so `is_unique` takes the in-place path on a still-aliased
/// buffer). BUT the terminal dec is suppressed ONLY when every NON-terminal
/// duplicate use is LIVE — each LIVE alias releases its own reference at its own
/// last-use (RL-2 `LastReadBeforeScopeExit`), so the source's terminal dec is the
/// redundant double-release. If a duplicate alias is DEAD (`let b = a` where `b`
/// is never read), the source's terminal dec is that dead alias's
/// `ScopeExit` release (RL-2 non-transfer -> dec) — it is KEPT, not suppressed
/// (else the `updated_list_dead_alias_no_leak` case leaks the orphaned buffer).
/// Only the terminal-move source's last-use DEC is governed here; its FRESH inc
/// is KEPT for the dup case (mod.rs gates the symmetric inc-suppression on
/// `use_counts <= 1`), since that inc supplies the duplicate references the
/// non-terminal uses consume. Per AIMS RL-2 `TerminalUse`: a move IS an
/// ownership-transferring terminal use, a dead alias's `ScopeExit` is not
/// (`AimsProof.Realization::RL2_dec_at_last_use`).
pub(in crate::lower::burden_lower) fn compute_transfer_via_move_alias(
    func: &ArcFunction,
    terminator_transfer_per_block: &[FxHashSet<ArcVarId>],
    use_counts: &FxHashMap<ArcVarId, u32>,
    last_use_points: &[(ArcVarId, usize, usize)],
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    // `Invoke @callee(%arg [own]) -> %result` move-edges where `@callee`'s
    // `MemoryContract` transfers an owned param THROUGH its return: `%result` IS
    // the forwarded `%arg` (a move across the call), so the chain
    // `... -> %arg -> %result -> ...` collapses like a `Let { Var }` move. Without
    // this edge the move-chain breaks at the `Invoke` (the result is a fresh SSA
    // var, not a Let-alias of the arg), leaving the value-copy aggregate's
    // intermediate decs/incs un-suppressed (double-free on the forwarder lineage).
    invoke_ttr_edges: &[(ArcVarId, ArcVarId)],
    // Same-allocation identity inputs consumed by the surplus-dec suppression
    // arms (`collect_surplus_dec_srcs`): the `genuine_same_alloc_reps` union-find
    // (Let{Var} + apply-Direct/Conditional edges) + the `apply_result_aliases`
    // (`ApplyAliasSource::Project` for the joint borrow-projection arm) + the type
    // registry (sole-owned-field gate). Spec: Annex E §AIMS RL-2 + TF-4.
    same_alloc: &SameAllocIdentity<'_>,
) -> FxHashSet<ArcVarId> {
    // Global-last-use lookup: a var with exactly ONE `last_use_points` entry is
    // used in exactly one block, and that entry is its global last use. A var
    // used in >= 2 blocks has >= 2 entries (one per block) — its terminal use
    // is proven instead by the successor-reachability final-use proof
    // (`is_cross_block_final_use`), which admits the FIXPOINT-ONLY hand-off
    // edges below.
    let mut last_use_entry_count: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    let mut last_use_pos: FxHashMap<ArcVarId, (usize, usize)> = FxHashMap::default();
    let mut block_last_use: FxHashMap<(ArcVarId, usize), usize> = FxHashMap::default();
    let mut use_blocks: FxHashMap<ArcVarId, Vec<usize>> = FxHashMap::default();
    for &(var, b, i) in last_use_points {
        *last_use_entry_count.entry(var).or_default() += 1;
        last_use_pos.insert(var, (b, i));
        block_last_use.insert((var, b), i);
        use_blocks.entry(var).or_default().push(b);
    }
    let is_single_block_last_use = |src: &ArcVarId, b: usize, i: usize| -> bool {
        last_use_entry_count.get(src).copied().unwrap_or(0) == 1
            && last_use_pos.get(src) == Some(&(b, i))
    };
    let src_has_dead_alias = collect_dead_alias_sources(func, use_counts);

    let mut transferred: FxHashSet<ArcVarId> = FxHashSet::default();
    // Seed: terminator-transferred vars.
    for set in terminator_transfer_per_block {
        transferred.extend(set.iter().copied());
    }
    // Seed: instruction owned-position transfers (Construct/Apply/Set/etc.).
    for block in &func.blocks {
        for instr in &block.body {
            transferred.extend(instr_transfer_vars(instr, func).iter().copied());
        }
    }
    // Move-alias edges `dst -> src`. A use-once source is the unchanged pure-move
    // case. A dup'd source qualifies at its TERMINAL `Let { Var }` use AND
    // only when it has NO dead duplicate alias (a dead alias's reference is
    // discharged by the kept terminal dec, not by a downstream consumer).
    // Terminality is proven two ways:
    // - single-block source: its sole `last_use_points` entry is this site
    //   (`move_edges` — the legacy admitted set, seed + fixpoint);
    // - cross-block LOCAL source: the successor-reachability final-use proof
    //   (`handoff_edges` — FIXPOINT-ONLY conditional edges: the source is
    //   cancelled iff its terminal alias dst GENUINELY transfers out at an
    //   owned position per AIMS RL-2, AND every NON-terminal use of the source
    //   also discharges a duplicate reference — an owned-position consume, or
    //   a `Let { Var }` alias whose own lineage transfers — from a block that
    //   DOMINATES the terminal block (an alternative-arm use never runs on
    //   the terminal's path, so it discharges nothing there). A non-consuming
    //   non-terminal use (borrow read, slice view) leaves a duplicate
    //   reference whose only release IS the terminal dec, so cancellation
    //   declines. Function PARAMS are excluded: a param's last-use dec marker
    //   is load-bearing on the default coexistence path (`emit_last_use_decs`
    //   keeps it so the residual predicate-stack co-emission suppresses its
    //   own real dec); the param-transfers-through-return case is owned by the
    //   contract-driven `transfer_through_return_param_vars` strip instead.
    let param_vars: FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();
    let mut move_edges: Vec<(ArcVarId, ArcVarId)> = Vec::new();
    let mut handoff_edges: Vec<HandoffEdge> = Vec::new();
    let mut reach_cache: FxHashMap<usize, FxHashSet<usize>> = FxHashMap::default();
    // Built lazily — only a function with at least one cross-block final-use
    // candidate pays for the dominator tree.
    let mut dom_tree: Option<DominatorTree> = None;
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                let count = use_counts.get(src).copied().unwrap_or(0);
                let dup_no_dead_alias = count >= 2 && !src_has_dead_alias.contains(src);
                let use_once = count == 1;
                let dup_terminal_move =
                    dup_no_dead_alias && is_single_block_last_use(src, block_idx, instr_idx);
                if use_once || dup_terminal_move {
                    move_edges.push((*dst, *src));
                } else if dup_no_dead_alias
                    && !param_vars.contains(src)
                    && !super::super::cross_block_final_use_cancel_disabled()
                    && is_cross_block_final_use(
                        func,
                        &block_last_use,
                        &use_blocks,
                        &mut reach_cache,
                        *src,
                        block_idx,
                        instr_idx,
                    )
                {
                    let dom = dom_tree.get_or_insert_with(|| DominatorTree::build(func));
                    if let Some(required) =
                        non_terminal_uses_all_discharge(func, dom, *src, block_idx, instr_idx)
                    {
                        handoff_edges.push(HandoffEdge {
                            dst: *dst,
                            src: *src,
                            required,
                        });
                    }
                }
            }
        }
    }
    // Invoke transfer-through-return edges: `%result` is the moved `%arg`.
    move_edges.extend(invoke_ttr_edges.iter().copied());
    // Seed: a move-alias source `%s` (`%d = %s`, `%s` used once) whose dst `%d`
    // is owned-RC has its SINGLE release discharged BY `%d` — either `%d`
    // transfers out (seeded above) OR `%d` gets its own last-use dec. The move
    // hands `%s`'s one allocation to `%d`; emitting `%s`'s own last-use dec
    // would double-release the shared buffer, and emitting `%s`'s FRESH-site inc
    // would orphan (the move is not a duplication — `%d` does NOT get a paired
    // inc, so the lineage carries exactly one inc+dec at the `%d` end). Both
    // halves suppressed via this set (dec here, inc via `inc_suppressed_vars`),
    // matching the transfer-out case. Witness: `coll_list_cow_concat_shared`
    // `%14 = %8` (fresh concat result moved to a borrow-used alias) — `%8`'s
    // scope-exit dec double-frees the buffer `%14` decs. Per AIMS RL-2
    // (move = ownership transfer, single release at the lineage's terminal owner).
    for &(dst, src) in &move_edges {
        if owned_vars_needing_rc.contains(&dst) {
            transferred.insert(src);
        }
    }
    // Same-allocation surplus-dec suppression arms (both RL-2 release-once): the
    // borrow-view-dst keystone (`%6 = %4` forwarder-result) + the joint
    // borrow-projection arm (`edge_project_return_not_param`). Full rationale +
    // gates: `collect_surplus_dec_srcs`.
    transferred.extend(collect_surplus_dec_srcs(
        func,
        &SurplusDecInputs {
            owned_vars_needing_rc,
            use_counts,
            last_use_points,
            src_has_dead_alias: &src_has_dead_alias,
            param_vars: &param_vars,
            same_alloc,
        },
    ));
    // Fixpoint: a move source transfers out when its dst transfers out (monotone
    // over `transferred`; both move + cross-block hand-off edge kinds terminate).
    // See `run_move_transfer_fixpoint`.
    run_move_transfer_fixpoint(&mut transferred, &move_edges, &handoff_edges);
    transferred
}

/// Transfer-out fixpoint over move + cross-block hand-off edges. A move source
/// transfers out when its dst does; a hand-off edge fires only when its dst
/// transfers AND every `required` non-terminal-alias lineage also transfers.
/// Both edge kinds are monotone over `transferred`, so the loop terminates.
fn run_move_transfer_fixpoint(
    transferred: &mut FxHashSet<ArcVarId>,
    move_edges: &[(ArcVarId, ArcVarId)],
    handoff_edges: &[HandoffEdge],
) {
    let mut changed = true;
    while changed {
        changed = false;
        for &(dst, src) in move_edges {
            if transferred.contains(&dst) && transferred.insert(src) {
                changed = true;
            }
        }
        for edge in handoff_edges {
            if transferred.contains(&edge.dst)
                && edge.required.iter().all(|d| transferred.contains(d))
                && transferred.insert(edge.src)
            {
                changed = true;
            }
        }
    }
}

/// Same-allocation identity inputs for the surplus-dec suppression arms — the
/// `genuine_same_alloc_reps` union-find + the `apply_result_aliases` Project
/// edges + the type registry (sole-owned-field gate). Spec: Annex E §AIMS RL-2.
pub(in crate::lower::burden_lower) struct SameAllocIdentity<'a> {
    pub(in crate::lower::burden_lower) genuine_same_alloc_reps: &'a FxHashMap<ArcVarId, ArcVarId>,
    pub(in crate::lower::burden_lower) apply_result_aliases:
        &'a FxHashMap<ArcVarId, ApplyAliasSource>,
    pub(in crate::lower::burden_lower) type_registry: &'a TypeRegistry,
}

/// Shared inputs for both surplus-dec suppression arms (borrow-view-dst +
/// joint-borrow-projection). Bundles the per-var last-use / use-count / dead-alias
/// facts with the same-allocation identity.
struct SurplusDecInputs<'a> {
    owned_vars_needing_rc: &'a FxHashSet<ArcVarId>,
    use_counts: &'a FxHashMap<ArcVarId, u32>,
    last_use_points: &'a [(ArcVarId, usize, usize)],
    src_has_dead_alias: &'a FxHashSet<ArcVarId>,
    param_vars: &'a FxHashSet<ArcVarId>,
    same_alloc: &'a SameAllocIdentity<'a>,
}

/// Run both same-allocation surplus-dec suppression arms (each RL-2
/// release-once), each behind its own toggle. Returns the union of `%s` (surplus
/// source) sets to mark transferred. Spec: Annex E §AIMS RL-2 + TF-4.
fn collect_surplus_dec_srcs(func: &ArcFunction, inp: &SurplusDecInputs<'_>) -> FxHashSet<ArcVarId> {
    let mut out: FxHashSet<ArcVarId> = FxHashSet::default();
    if !super::super::borrow_view_dst_surplus_dec_suppress_disabled() {
        out.extend(collect_borrow_view_dst_surplus_dec_srcs(func, inp));
    }
    if !super::super::project_return_surplus_owner_dec_suppress_disabled() {
        out.extend(collect_project_return_surplus_owner_dec_srcs(func, inp));
    }
    out
}

/// Borrow-view-dst surplus-dec suppression source set (RL-2 release-once). A
/// use-once owned source `%s` whose sole `Let { Var }` alias `%d` is a
/// same-allocation borrow-view (`genuine_same_alloc_reps` proves identity) that
/// is live downstream: `%s`'s own scope-exit dec is the SURPLUS same-allocation
/// dec (the lineage's single release is the edge-cleanup dec on the shared rep).
/// Returns the `%s` set to mark transferred. Spec: Annex E §AIMS RL-2.
fn collect_borrow_view_dst_surplus_dec_srcs(
    func: &ArcFunction,
    inp: &SurplusDecInputs<'_>,
) -> FxHashSet<ArcVarId> {
    let owned_vars_needing_rc = inp.owned_vars_needing_rc;
    let use_counts = inp.use_counts;
    let last_use_points = inp.last_use_points;
    let src_has_dead_alias = inp.src_has_dead_alias;
    let param_vars = inp.param_vars;
    let genuine_same_alloc_reps = inp.same_alloc.genuine_same_alloc_reps;
    let mut entry_count: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    let mut last_pos: FxHashMap<ArcVarId, (usize, usize)> = FxHashMap::default();
    for &(var, b, i) in last_use_points {
        *entry_count.entry(var).or_default() += 1;
        last_pos.insert(var, (b, i));
    }
    let mut out: FxHashSet<ArcVarId> = FxHashSet::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            else {
                continue;
            };
            let (dst, src) = (*dst, *src);
            // `%s` owned-RC + use-once + this alias is its single-block last use +
            // no dead alias (a dead alias's ref is discharged by the kept terminal
            // dec, never the borrow-view path) + not a param.
            let single_block_last_use = entry_count.get(&src).copied().unwrap_or(0) == 1
                && last_pos.get(&src) == Some(&(block_idx, instr_idx));
            if !owned_vars_needing_rc.contains(&src)
                || use_counts.get(&src).copied().unwrap_or(0) != 1
                || !single_block_last_use
                || src_has_dead_alias.contains(&src)
                || param_vars.contains(&src)
            {
                continue;
            }
            // `%d` is a SAME-ALLOCATION borrow-view that is LIVE downstream.
            let same_alloc = genuine_same_alloc_reps.get(&dst).copied()
                == genuine_same_alloc_reps.get(&src).copied()
                && genuine_same_alloc_reps.contains_key(&dst);
            let dst_is_borrow_view = !owned_vars_needing_rc.contains(&dst);
            let dst_live = use_counts.get(&dst).copied().unwrap_or(0) >= 1;
            if same_alloc && dst_is_borrow_view && dst_live {
                out.insert(src);
            }
        }
    }
    out
}

/// Joint borrow-projection surplus-owner-dec suppression source set (RL-2
/// release-once + TF-4). A borrowed-receiver callee that returns `arg.field`
/// (apply-result `ApplyAliasSource::Project { arg, field }` — `@unwrap(b) =
/// b.value`) hands the caller's OWNED result `%d` a borrow-view of the SAME
/// single allocation as `%s = arg`'s SOLE owned RC field. The base walk emits
/// BOTH `%s`'s scope-exit dec (its drop recurses into the field) AND `%d`'s own
/// scope-exit dec at the joint lineage's last use — two releases of one
/// allocation (the joint borrow-projection double-free, exit 134). The proven
/// joint release theorem: one allocation, exactly one release. The owned result
/// `%d` carries the single release at its TRUE last use (which dominates `%s`'s
/// premature drop); `%s`'s own dec is the SURPLUS same-allocation release —
/// suppress it. Returns the `%s` set to mark transferred. Spec: Annex E §AIMS
/// RL-2 (`RL2_release_exactly_once`) + TF-4 (Project borrow-view identity).
///
/// SAME-ALLOCATION-IDENTITY discriminators bound the family (NOT a use-count /
/// type-membership proxy — each is a structural property of the lineage):
///
/// - **`return_alias = Project { field }`**: the callee's PROVEN contract that
///   the result IS `arg.field` (TF-4 borrow-view of `arg`'s field). Direct /
///   Conditional / Wrapped aliases are handled by other arms; only the
///   `Project` apply-result alias keys this arm.
/// - **SOLE owned RC field**: `arg`'s burden has EXACTLY ONE owned RC field, and
///   it is the returned `field`. A multi-field struct returning one field would
///   LEAK its other owned fields if `arg`'s whole-var dec were suppressed —
///   declined here (a `BurdenDecPartial skip=[field]` is the multi-field
///   extension, deferred to its own cycle). The single-owned-field gate makes
///   `arg`'s whole-var drop a no-op once the field transfers out, so whole-var
///   suppression is sound.
/// - **`%d` live downstream**: the result is used past the call (`use_count ≥
///   1`), so it carries the joint lineage's SINGLE release at its true last use
///   (whether `%d` is a fresh owned result or released by the projected-result
///   borrow-view machinery). A dead result aliases no surviving reference — the
///   surplus would be the result's, not `%s`'s; declined.
/// - **`%s` use-once owned non-param, no dead alias**: matches the keystone's
///   surplus-source gates — a use-once owned source whose single use is the
///   borrowed-receiver call, with no dead duplicate alias whose ref the kept
///   terminal dec must discharge.
fn collect_project_return_surplus_owner_dec_srcs(
    func: &ArcFunction,
    inp: &SurplusDecInputs<'_>,
) -> FxHashSet<ArcVarId> {
    let owned_vars_needing_rc = inp.owned_vars_needing_rc;
    let use_counts = inp.use_counts;
    let src_has_dead_alias = inp.src_has_dead_alias;
    let param_vars = inp.param_vars;
    let apply_result_aliases = inp.same_alloc.apply_result_aliases;
    let type_registry = inp.same_alloc.type_registry;
    let mut out: FxHashSet<ArcVarId> = FxHashSet::default();
    for (&dst, source) in apply_result_aliases {
        let ApplyAliasSource::Project { arg, field } = source else {
            continue;
        };
        let (arg, field) = (*arg, *field);
        // `%d` (the call result) is the joint lineage's release carrier: it is
        // live downstream (used past the call), so it carries the SINGLE release
        // at its true last use. Whether `%d` is tracked in `owned_vars_needing_rc`
        // (a fresh owned result) or released by the projected-result borrow-view
        // machinery, the buffer's one release lands at `%d`'s last use — `%s`'s
        // own drop is the surplus regardless. Require only `%d` live (used ≥ 1).
        if use_counts.get(&dst).copied().unwrap_or(0) == 0 {
            continue;
        }
        // `%s` (= the consumed `arg`) is a use-once owned non-param source whose
        // surplus drop is the box's recursive field-drop. No dead alias (its ref
        // would otherwise be discharged by the kept terminal dec).
        if !owned_vars_needing_rc.contains(&arg)
            || use_counts.get(&arg).copied().unwrap_or(0) != 1
            || src_has_dead_alias.contains(&arg)
            || param_vars.contains(&arg)
        {
            continue;
        }
        // SOLE-owned-RC-field gate: `arg`'s burden has exactly ONE owned RC
        // field, and it is the returned `field`. Whole-var suppression is sound
        // only when no OTHER owned field would leak. A multi-field struct
        // returning one field declines (deferred to a partial-dec extension).
        if !arg_sole_owned_rc_field_is(func, arg, field, type_registry) {
            continue;
        }
        out.insert(arg);
    }
    out
}

/// `arg`'s monomorphized burden has EXACTLY ONE owned RC field, and it is
/// `field`. Used by `collect_project_return_surplus_owner_dec_srcs` to gate
/// whole-var dec suppression on the single-owned-field case (whole-var drop
/// becomes a no-op once that field transfers out via the projected return) AND by
/// `walk::extend_owner_last_use_for_borrow_views` to gate the owner-drop
/// borrow-view-liveness placement relocation on the same discriminator.
pub(in crate::lower::burden_lower) fn arg_sole_owned_rc_field_is(
    func: &ArcFunction,
    arg: ArcVarId,
    field: u32,
    type_registry: &TypeRegistry,
) -> bool {
    let arg_ty: TypeRef = idx_to_type_ref(func.var_types[arg.index()], type_registry);
    let Some(burden) = lookup_burden(arg_ty, type_registry) else {
        return false;
    };
    // Each owned field carries a `field_path` (Cow<[u32]>); a TOP-LEVEL owned
    // field has a single-element path `[idx]`. The sole-owned-field gate requires
    // exactly one owned field, at the top level, equal to the returned `field`.
    let owned: Vec<Vec<u32>> = burden
        .owned_fields()
        .map(|f| f.field_path.to_vec())
        .collect();
    owned.len() == 1 && owned[0].as_slice() == [field]
}

/// Whether every use of `alias` across the function is a `Project` that reads
/// `alias` as its source (a borrow-read field extraction, TF-4) whose projected
/// dst is a pure BORROW-VIEW — NOT in `owned_vars_needing_rc`, so it is never
/// independently released (the `.name` / `.settings` read passed straight to a
/// `[borrow]` call). Returns false when `alias` is consumed at any other position
/// (owned call arg, Construct arg, Set, Return, another Let alias, terminator
/// operand) OR when ANY projected field is OWNED-RC (an extracted heap field with
/// its OWN release lineage — `let s_field = m.s` over `Mixed { s: str, xs: [int] }`
/// where `s_field` / `xs_field` get their own scope-exit decs). In the owned-field
/// case the owner's drop is NOT surplus — the fields transferred out and the owner
/// drop is suppressed by other arms; suppressing it here loses a release (leak).
fn alias_use_is_borrow_view_project_only(
    func: &ArcFunction,
    alias: ArcVarId,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> bool {
    let mut saw_use = false;
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Project { dst, value, .. } if *value == alias => {
                    // The projected field must be a pure borrow-view: NOT
                    // independently owned-RC, AND consumed only as a borrow-read
                    // (no `Let { Var }` extraction alias of it, which would move
                    // the field out into its own released lineage — the `Mixed`
                    // `let s_field = m.s` shape where the owner's drop is NOT
                    // surplus because the fields transferred out).
                    if owned_vars_needing_rc.contains(dst)
                        || !projected_field_is_borrow_read_only(func, *dst)
                    {
                        return false;
                    }
                    saw_use = true;
                }
                other => {
                    if other.used_vars().contains(&alias) {
                        return false;
                    }
                }
            }
        }
        if block.terminator.used_vars().contains(&alias) {
            return false;
        }
    }
    saw_use
}

/// Whether the projected field `field_dst` is consumed ONLY as a borrow-read
/// (used at an Invoke/Apply borrowed-arg position, or not at all), with NO
/// `Let { Var }` extraction alias and NO owned-position consume. A field extracted
/// into a downstream `Let` alias (`let s_field = m.s`) becomes its own released
/// lineage, so the owner's whole-var drop is NOT surplus over the borrow-view
/// aliases — declines the multi-borrow-view-alias arm.
fn projected_field_is_borrow_read_only(func: &ArcFunction, field_dst: ArcVarId) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                // A `Let { Var(field_dst) }` is an extraction alias — the field is
                // moved into its own released lineage.
                ArcInstr::Let {
                    value: ArcValue::Var(src),
                    ..
                } if *src == field_dst => return false,
                // Any owned-position consume of the field (Construct arg / owned
                // call arg / `Set` value) transfers it out.
                other => {
                    if instr_transfer_vars(other, func).contains(&field_dst) {
                        return false;
                    }
                }
            }
        }
        // A terminator owned-move (Return value / Jump arg) transfers the field
        // out; a borrowed Invoke arg (`Invoke @length(%f [borrow])`) does NOT and
        // is the permitted borrow-read consume.
        match &block.terminator {
            ArcTerminator::Return { value } if *value == field_dst => return false,
            ArcTerminator::Jump { args, .. } if args.contains(&field_dst) => return false,
            ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. } => {
                let used = block.terminator.used_vars();
                for (pos, &v) in used.iter().enumerate() {
                    if v == field_dst && block.terminator.is_owned_position(pos) {
                        return false;
                    }
                }
            }
            _ => {}
        }
    }
    true
}

/// Multi-borrow-view-alias surplus suppression (RL-2 release-once + TF-4 + DP-3).
/// The N-alias generalization of the single-alias
/// [`collect_borrow_view_dst_surplus_dec_srcs`] keystone: a fresh `Construct`
/// owner `%s` consumed ONLY through >= 2 same-allocation whole-var `Let { Var }`
/// aliases (`%6 = %s`, `%11 = %s`), each of which is a borrow-view (not owned-RC,
/// same `genuine_same_alloc_reps` rep) projected for a borrow-read
/// (`Project %6.0`, `Project %11.1`). The base walk emits a surplus whole-var
/// `BurdenDec` at EACH alias's last use AND keeps `%s`'s own scope-exit /
/// edge-cleanup dec — N+1 releases of ONE allocation (cleanup-on: the
/// redundant-cleanup pass over-strips toward a leak; cleanup-off: the surplus
/// decs over-release → double-free). It ALSO keeps `%s`'s spurious keep-alive
/// FRESH inc (the `use_counts >= 2` proxy mis-classes the borrow-view aliases as
/// duplication).
///
/// The proven correction (`RL2_release_exactly_once`): one allocation, exactly
/// one release. `%s`'s born-at-alloc reference (+1) is released by `%s`'s
/// surviving scope-exit / edge-cleanup dec (exactly one per CFG path); each
/// alias's whole-var dec is the SURPLUS same-allocation release — suppress it.
/// The keep-alive FRESH inc is spurious (the lineage is borrow-read-only, no
/// duplication, DP-3 `is_rc_inc_elidable`) — suppress it. Returns
/// `(alias_dec_suppress, owner_inc_suppress)`: the alias dsts whose surplus decs
/// are suppressed (marked `transfer_via_move_alias`) and the owners whose
/// keep-alive inc is suppressed (marked `inc_suppressed_vars`).
///
/// SAME-ALLOCATION-IDENTITY discriminators bound the family (NOT a use-count /
/// type-membership proxy — each is a structural property of the lineage):
///
/// - **fresh `Construct` owner in `owned_vars_needing_rc`, non-param**: the
///   allocation born here carries the single release.
/// - **EVERY direct use is a same-allocation whole-var borrow-view alias**: each
///   direct use is a `Let { Var(%s) }` whose dst is NOT owned-RC (a borrow-view)
///   and shares `%s`'s `genuine_same_alloc_reps` rep. A use-once MOVE alias (one
///   alias, owned dst) is the single-alias keystone's territory; an owned-consume
///   (Construct arg / `[own]` call arg / `Set` / Return) declines (the consume IS
///   the transfer — `%s`'s ownership leaves, not a borrow).
/// - **>= 2 such aliases**: the single-alias case is the existing keystone arm;
///   this arm owns the multi-alias generalization the keystone's
///   `use_counts(%s) == 1` gate declines.
/// - **whole same-allocation lineage is borrow-read-only**: no member (alias OR
///   projected field) is owned-consumed (`owned_consumed`). A field moved out at
///   an owned position keeps the owner's drop load-bearing — declined.
pub(in crate::lower::burden_lower) fn compute_multi_borrow_view_alias_surplus(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    owned_consumed: &FxHashSet<ArcVarId>,
    genuine_same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) -> (FxHashSet<ArcVarId>, FxHashSet<ArcVarId>) {
    let mut alias_dec_suppress: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut owner_inc_suppress: FxHashSet<ArcVarId> = FxHashSet::default();
    let param_vars: FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();

    // Forward whole-var alias edges + projected-field edges (the same-allocation
    // flow), and per-owner direct-use lists.
    let mut whole_var_aliases: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    let mut flow_dsts: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    let mut direct_uses: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } => {
                    whole_var_aliases.entry(*src).or_default().push(*dst);
                    flow_dsts.entry(*src).or_default().push(*dst);
                    direct_uses.entry(*src).or_default().push(*dst);
                }
                ArcInstr::Project { dst, value, .. } => {
                    flow_dsts.entry(*value).or_default().push(*dst);
                    direct_uses.entry(*value).or_default().push(*dst);
                }
                _ => {}
            }
        }
    }

    for block in &func.blocks {
        for instr in &block.body {
            let ArcInstr::Construct { dst: owner, .. } = instr else {
                continue;
            };
            let owner = *owner;
            // Owner is a fresh owned-RC non-param allocation.
            if !owned_vars_needing_rc.contains(&owner) || param_vars.contains(&owner) {
                continue;
            }
            let Some(uses) = direct_uses.get(&owner) else {
                continue;
            };
            // EVERY direct use is a same-allocation whole-var `Let { Var }` alias
            // whose own sole downstream use is a `Project` borrow-read, and there
            // are >= 2 of them (the single-alias case is the keystone). Each alias
            // is the same allocation as the owner (`genuine_same_alloc_reps` rep,
            // unwrap-or-self: the owner is its own rep, never a key). Whether the
            // alias is tracked owned-RC or not, its whole-var dec at the borrow
            // point is the surplus same-allocation release.
            let owner_rep = genuine_same_alloc_reps
                .get(&owner)
                .copied()
                .unwrap_or(owner);
            let mut alias_count = 0usize;
            let mut all_borrow_view_aliases = true;
            for &u in uses {
                let is_whole_var_alias = whole_var_aliases
                    .get(&owner)
                    .is_some_and(|a| a.contains(&u));
                let same_alloc = genuine_same_alloc_reps.get(&u).copied().unwrap_or(u) == owner_rep;
                // The alias's own sole use is a `Project` borrow-read (it never
                // consumes the aggregate at an owned position itself).
                let alias_borrow_projects_only =
                    alias_use_is_borrow_view_project_only(func, u, owned_vars_needing_rc);
                if is_whole_var_alias && same_alloc && alias_borrow_projects_only {
                    alias_count += 1;
                } else {
                    all_borrow_view_aliases = false;
                    break;
                }
            }
            if !all_borrow_view_aliases || alias_count < 2 {
                continue;
            }
            // The whole same-allocation lineage is borrow-read-only: no member
            // (alias OR projected field) is owned-consumed. A field moved out at
            // an owned position keeps the owner's drop load-bearing — decline.
            let mut stack = vec![owner];
            let mut seen: FxHashSet<ArcVarId> = FxHashSet::default();
            seen.insert(owner);
            let mut borrow_only = true;
            while let Some(v) = stack.pop() {
                // The owner itself is the released root, never an "owned consume"
                // for this gate; only its descendants count.
                if v != owner && owned_consumed.contains(&v) {
                    borrow_only = false;
                    break;
                }
                if let Some(children) = flow_dsts.get(&v) {
                    for &child in children {
                        if seen.insert(child) {
                            stack.push(child);
                        }
                    }
                }
            }
            if !borrow_only {
                continue;
            }
            tracing::trace!(
                target: "ori_arc::lower::burden_lower",
                owner = owner.index(),
                alias_count,
                "multi-borrow-view-alias surplus: owner keep-alive inc + N alias \
                 decs suppressed (RL-2 release-once over N borrow-view aliases)"
            );
            owner_inc_suppress.insert(owner);
            for &u in uses {
                alias_dec_suppress.insert(u);
            }
        }
    }
    (alias_dec_suppress, owner_inc_suppress)
}

/// DP-3 + RL-1 + RL-2 read-only-borrow orphan-inc suppression. A fresh
/// `Construct` dst whose FRESH-site `BurdenInc` is ORPHANED — its scope-exit dec
/// is transfer-suppressed (`transferred`, via the owned-RC-dst move-edge seed)
/// AND its every direct use is a `Let { Var }` DUP-alias (`dup_alias_dsts`: the
/// source stays live, `use_count >= 2`, so each alias carries its OWN paired
/// FRESH inc + last-use dec and self-balances) — has NO paired dec for its root
/// inc and needs none. Per DP-3 (`is_rc_inc_elidable`: `Once ∧ Affine` borrow →
/// the duplicate-inc is elidable, the upstream alias entries DECOUPLED;
/// `AimsProof.Decision::DP3_is_rc_inc_elidable_table` admits the `Affine` row)
/// the root construct's FRESH inc is the orphan (VF-1 net=+1 leak). Suppressing
/// it restores RL-2 single-release balance
/// (`AimsProof.Realization::RL2_dec_at_last_use`).
///
/// TWO same-allocation-identity discriminators bound the family (NOT a use-count
/// proxy — each is a structural property of the construct's lineage):
///
/// - **every-use-is-dup-alias**: EXCLUDES the use-once MOVE case
///   (`let h = Holder { kept: s }`: `%30 -> %38` where `%38` carries the
///   move-dec that PAIRS with `%30`'s FRESH inc — not an orphan). A use-once
///   move dst is not in `dup_alias_dsts`, so the gate declines and the balanced
///   FRESH inc + move-dec pair is preserved.
/// - **Project-aware read-only-borrow-only closure**: EXCLUDES the
///   struct-self-rebuild field-move-out (`r = Pair { a: r.a, b: r.b }`: the heap
///   field `Project r.a` is owned-consumed at the rebuild `Construct`, so the
///   `Project` dst is in `owned_consumed` and the gate declines — the kept inc
///   stays load-bearing per `RL1_duplication_balanced`). The closure follows
///   BOTH `Let { Var }` alias edges AND `Project` field-extraction edges, so a
///   heap field moved out of the lineage at an owned position is caught.
///
/// Spec: Annex E §AIMS DP-3 + RL-1 + RL-2.
pub(in crate::lower::burden_lower) fn compute_readonly_borrow_orphan_inc_suppression(
    func: &ArcFunction,
    transferred: &FxHashSet<ArcVarId>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    owned_consumed: &FxHashSet<ArcVarId>,
    dup_alias_dsts: &FxHashSet<ArcVarId>,
) -> FxHashSet<ArcVarId> {
    // Forward same-allocation flow edges: a `Let { Var }` alias OR a `Project`
    // field-extraction (the projected value names the same heap object's
    // sub-allocation). src -> [dst, ...].
    let mut flow_dsts: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    // Per-var direct uses, to enforce every-use-is-dup-alias.
    let mut direct_uses: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } => {
                    flow_dsts.entry(*src).or_default().push(*dst);
                    direct_uses.entry(*src).or_default().push(*dst);
                }
                ArcInstr::Project { dst, value, .. } => {
                    flow_dsts.entry(*value).or_default().push(*dst);
                }
                _ => {}
            }
        }
    }
    let mut result: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            let ArcInstr::Construct { dst, args, .. } = instr else {
                continue;
            };
            // Act only on a transfer-suppressed (orphaned) owned-RC construct.
            if !owned_vars_needing_rc.contains(dst) || !transferred.contains(dst) {
                continue;
            }
            // LEAF-allocation gate: the construct holds NO owned-RC argument, so
            // it is a single leaf heap object with no heap sub-allocations to
            // recursively drop (a scalar-element collection buffer like `[int]`).
            // EXCLUDES recursive-drop aggregates — a struct/tuple holding a heap
            // field, OR a heap-element collection (`[str]` / `[[int]]`) — whose
            // FRESH inc is BALANCED by its own drop-dec (the drop recursively
            // frees the children), so suppressing it over-releases the children
            // (the `narrowed_list_derived_eq` Container / `Holder { kept: s }`
            // shapes). Same-allocation-identity based (leaf vs recursive-owner),
            // NOT a type proxy.
            if args.iter().any(|a| owned_vars_needing_rc.contains(a)) {
                continue;
            }
            // every-use-is-dup-alias: EVERY direct use of the root is a
            // `Let { Var }` dup-alias (`dup_alias_dsts`). A use-once move dst (not
            // a dup) carries an UNPAIRED move-dec that pairs with the root inc —
            // not an orphan; decline.
            let root_uses = direct_uses.get(dst);
            let every_use_dup_alias = root_uses.is_some_and(|uses| {
                !uses.is_empty() && uses.iter().all(|u| dup_alias_dsts.contains(u))
            });
            if !every_use_dup_alias {
                continue;
            }
            // Project-aware read-only-borrow-only closure: NO same-allocation
            // member (alias OR projected field) is owned-consumed.
            let mut stack = vec![*dst];
            let mut seen: FxHashSet<ArcVarId> = FxHashSet::default();
            seen.insert(*dst);
            let mut borrow_only = true;
            while let Some(v) = stack.pop() {
                if owned_consumed.contains(&v) {
                    borrow_only = false;
                    break;
                }
                if let Some(children) = flow_dsts.get(&v) {
                    for &child in children {
                        if seen.insert(child) {
                            stack.push(child);
                        }
                    }
                }
            }
            if borrow_only {
                tracing::trace!(
                    target: "ori_arc::lower::burden_lower",
                    root = dst.index(),
                    "readonly-borrow orphan-inc suppression: fresh Construct \
                     fresh-site BurdenInc suppressed (DP-3 borrow lineage)"
                );
                result.insert(*dst);
            }
        }
    }
    result
}

/// Sources with at least one DEAD `Let { Var }` duplicate alias (`%d = %src`,
/// `%d` never used). The dead alias's reference is released by the source's
/// terminal dec (RL-2 `ScopeExit`); suppressing it would leak.
fn collect_dead_alias_sources(
    func: &ArcFunction,
    use_counts: &FxHashMap<ArcVarId, u32>,
) -> FxHashSet<ArcVarId> {
    let mut src_has_dead_alias: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                if use_counts.get(dst).copied().unwrap_or(0) == 0 {
                    src_has_dead_alias.insert(*src);
                }
            }
        }
    }
    src_has_dead_alias
}

/// Conditional cross-block hand-off edge: `src`'s terminal `Let { Var }` alias
/// is `dst`; cancellation fires in the fixpoint only when `dst` AND every
/// `required` non-terminal alias dst transfer out.
struct HandoffEdge {
    dst: ArcVarId,
    src: ArcVarId,
    required: Vec<ArcVarId>,
}

/// Classify every NON-terminal use of `src` (every use site except the
/// terminal `Let { Var }` at `(terminal_block, terminal_idx)`) per AIMS RL-2:
///
/// - owned-position consume (`instr_transfer_vars`: Construct / Reuse /
///   `CollectionReuse` / owned Apply arg / `Set.value` / list-concat operand,
///   or an owned terminator position) — discharges its duplicate reference at
///   the consume; nothing further required.
/// - `Let { Var }` alias — discharges only when the alias's own lineage
///   transfers out; the alias dst joins the returned `required` set and is
///   resolved in the fixpoint.
/// - anything else (borrow read, `Project`, `IsShared`, slice view, `Set`
///   base, borrowed terminator arg) — does NOT consume a reference; the
///   terminal dec is that duplicate's only release, so cancellation DECLINES
///   (`None`).
///
/// Every non-terminal use's block MUST additionally DOMINATE the terminal
/// block (same-block uses are earlier-in-block by the caller's finality
/// proof). A discharge counts only when it executes on EVERY path reaching
/// the terminal; an ALTERNATIVE-arm use (sibling `Branch`/`Switch` arm — the
/// per-arm sum-rebuild shape `if c then Left(v, next: t) else
/// Right(v, next: t)`) never runs on this arm's path, so the kept per-arm dec
/// is that path's only release for the path-insensitive duplication inc —
/// cancellation DECLINES (`None`).
fn non_terminal_uses_all_discharge(
    func: &ArcFunction,
    dom: &DominatorTree,
    src: ArcVarId,
    terminal_block: usize,
    terminal_idx: usize,
) -> Option<Vec<ArcVarId>> {
    let terminal_block_id = block_id(terminal_block);
    let mut required: Vec<ArcVarId> = Vec::new();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            if block_idx == terminal_block && instr_idx == terminal_idx {
                continue;
            }
            if !instr.used_vars().contains(&src) {
                continue;
            }
            if block_idx != terminal_block && !dom.dominates(block_id(block_idx), terminal_block_id)
            {
                return None;
            }
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(s),
                ..
            } = instr
            {
                if *s == src {
                    required.push(*dst);
                    continue;
                }
            }
            if let ArcInstr::Set { base, .. } = instr {
                if *base == src {
                    return None;
                }
            }
            if instr_transfer_vars(instr, func).contains(&src) {
                continue;
            }
            return None;
        }
        let term_uses = block.terminator.used_vars();
        for (pos, &var) in term_uses.iter().enumerate() {
            if var != src {
                continue;
            }
            if !block.terminator.is_owned_position(pos)
                || !dom.dominates(block_id(block_idx), terminal_block_id)
            {
                return None;
            }
        }
    }
    Some(required)
}

/// Successor-reachability final-use proof for a dup'd CROSS-BLOCK move source
/// per AIMS RL-2: the `Let { Var(src) }` at `(b, i)` is `src`'s global final
/// use iff (a) it is `src`'s last use within block `b` (a terminator use of
/// `src` registers at the sentinel index and fails this), and (b) no block
/// reachable from `b`'s successors uses `src`. The walk follows EVERY CFG
/// edge, so a block re-reached through a loop back-edge — including `b`
/// itself — is in the set: a back-edge re-use is a later use of the same
/// lineage and DECLINES the proof (the next iteration still consumes the
/// reference). Reachability sets are memoized per block in `reach_cache`.
fn is_cross_block_final_use(
    func: &ArcFunction,
    block_last_use: &FxHashMap<(ArcVarId, usize), usize>,
    use_blocks: &FxHashMap<ArcVarId, Vec<usize>>,
    reach_cache: &mut FxHashMap<usize, FxHashSet<usize>>,
    src: ArcVarId,
    b: usize,
    i: usize,
) -> bool {
    if block_last_use.get(&(src, b)) != Some(&i) {
        return false;
    }
    let reach = reach_cache
        .entry(b)
        .or_insert_with(|| successor_reachable_blocks(func, b));
    use_blocks
        .get(&src)
        .is_none_or(|blocks| blocks.iter().all(|ub| !reach.contains(ub)))
}
