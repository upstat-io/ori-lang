//! FRESH-sum LIVE-EXTRACT same-alloc lineage treatment (RL-1 + RL-2): the
//! match-extract sibling of `compute_construct_fed_dead_param_lineage`.
//! Shape, over-emission mechanism, cure, and admission gates are documented
//! on [`compute_fresh_sum_live_extract_lineage`].

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;
use ori_types::TypeRegistry;

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::Uniqueness;
use crate::graph::compute_predecessors;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind, ValueRepr};

use crate::lower::burden::{Burden, TypeRef};
use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

use super::super::is_provably_scalar_repr;
use super::live_extract_site::{choose_release_site, release_site_sound};
use super::{function_used_vars, ForwarderReleasePos};

/// Result of [`compute_fresh_sum_live_extract_lineage`]: the same-alloc
/// closure to suppress (Part A) + the single placed release (Part B).
pub(in crate::lower::burden_lower) struct FreshSumLiveExtractLineage {
    /// Every var in an admitted fresh-sum live-extract closure. All carry
    /// spurious keep-alive incs / misplaced releases on ONE allocation;
    /// removed from `owned_vars_needing_rc` so the sole release is the
    /// placed dec below.
    pub suppressed_lineage_vars: FxHashSet<ArcVarId>,
    /// `(block_idx, pos) -> [dec var]` — exactly ONE whole-var `BurdenDec`
    /// per admitted closure, on the lineage var read at the final site,
    /// placed AFTER that read. Merged into the `forwarder_result_releases`
    /// emission surface (same `ForwarderReleasePos` placement contract).
    pub releases: FxHashMap<(usize, ForwarderReleasePos), Vec<ArcVarId>>,
}

/// RL-1 + RL-2 treatment for the FRESH-sum live-extract match shape
/// (`match result { Some(s) -> s, .. }` + downstream borrow-reads).
///
/// A FRESH niche-family sum allocation read through `Let { Var }` aliases +
/// a niche-payload `Project` borrow-view and EXTRACTED LIVE to a merge
/// block-param names ONE allocation across the whole closure (TF-4: `Project`
/// is a borrow; a niche-family sum carries no allocation of its own). Per
/// RL-1 NO duplication exists, yet the `use_counts >= 2` proxy classes the
/// Let-Var alias as a duplication and the FRESH-site result inc survives:
/// 2 spurious keep-alive incs + 2 misplaced releases (the sum's arm dec
/// before the live reads + the extract's last-use dec before its final
/// transitive read) net +1 — the caller-owned reference is never released
/// (`RL2_release_exactly_once` violated; leak).
///
/// The cure removes the WHOLE closure from `owned_vars_needing_rc` (both
/// incs + both misplaced releases) and emits EXACTLY ONE whole-var
/// `BurdenDec` on the lineage var read at the closure's execution-FINAL
/// borrow-read, placed AFTER that read (`RL2_dec_at_last_use` — no UAF). The
/// dec targets the final-read var (NOT the root): on a merge path where the
/// carrier param received a DIFFERENT allocation (the `None -> "fallback"`
/// arm's literal), the dec releases that path's allocation — per-path
/// correct where a root dec would no-op and leak the literal.
///
/// Admission gates (ALL must hold per closure; ANY failure declines the root
/// and keeps current behavior — the status-quo leak is FAR safer than an
/// over-fire UAF / double-free):
///  (a) FRESH sum root: a `Construct { ctor: EnumVariant }` dst, OR an
///      `Apply` / `Invoke` dst whose callee contract reports
///      `return_info.uniqueness ∈ {Unique, MaybeShared}` (the same condition
///      under which `fresh_site_burden_inc_dst` emits the spurious result
///      inc) with NO `transfers_through_return` param (a forwarder result is
///      the ARG's allocation — owned by the forwarder scans, disjoint family).
///  (b) root in `owned_vars_needing_rc` (heap-carrying; auto-declines roots
///      claimed by the construct-fed suppression, which runs first) with
///      `Aggregate` repr (an `RcPointer` sum is a BOXED wrapper needing its
///      own release — out of family).
///  (c) niche-family sum type ([`is_niche_family_sum`]): the wrapper's RC
///      identity IS the single live payload, so one release frees the web.
///  (d) vetted same-alloc closure ([`same_alloc_closure_vetted`]): every
///      member use is a borrow-read; any consume / store / capture /
///      `Select`-`IsShared`-`Reset`-`Reuse` use / `Return` / indirect-call
///      arg / non-scalar borrowed-call result declines.
///  (e) at least one closure member is a LIVE block-param (the live extract;
///      a dead-params-only closure is the RL-5 dead-param family).
///  (f) execution-final single release ([`choose_release_site`] +
///      [`release_site_sound`]): the dec lands after the closure's final
///      borrow-read on every normal exit; unwind paths (`Resume`) are exempt
///      (status-quo leak there, no new double-free — the closure carries no
///      other release).
pub(in crate::lower::burden_lower) fn compute_fresh_sum_live_extract_lineage(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    type_registry: &TypeRegistry,
) -> FreshSumLiveExtractLineage {
    let mut out = FreshSumLiveExtractLineage {
        suppressed_lineage_vars: FxHashSet::default(),
        releases: FxHashMap::default(),
    };
    let used = function_used_vars(func);
    let preds = compute_predecessors(func);

    for root in collect_fresh_sum_roots(func, contracts) {
        let decline = |gate: &str| {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                root = root.index(),
                gate,
                "fresh-sum live-extract root declined"
            );
        };
        // Gate (b): heap-carrying + not already claimed + by-value sum repr.
        if !owned_vars_needing_rc.contains(&root)
            || out.suppressed_lineage_vars.contains(&root)
            || !matches!(func.var_repr(root), Some(ValueRepr::Aggregate))
        {
            decline("b:owned/claimed/repr");
            continue;
        }
        // Gate (c): niche-family sum burden.
        if !is_niche_family_sum(func, root, type_registry) {
            decline("c:niche-family-sum");
            continue;
        }
        // Gate (d): vetted same-alloc closure.
        let Some(members) = same_alloc_closure_vetted(func, root) else {
            decline("d:closure-vet");
            continue;
        };
        // Gate (e): a LIVE block-param member (the live extract).
        let has_live_param = func.blocks.iter().any(|b| {
            b.params
                .iter()
                .any(|&(p, _)| members.contains(&p) && used.contains(&p))
        });
        if !has_live_param {
            decline("e:live-extract-param");
            continue;
        }
        // Gate (f): the placed single release.
        let Some((site_block, site_pos, dec_var)) = choose_release_site(func, &members) else {
            decline("f:release-site");
            continue;
        };
        if !release_site_sound(func, &members, root, site_block, site_pos, &preds) {
            decline("f:site-soundness");
            continue;
        }
        out.suppressed_lineage_vars.extend(members.iter().copied());
        out.releases
            .entry((site_block, site_pos))
            .or_default()
            .push(dec_var);
    }
    out
}

/// Candidate FRESH-sum roots per gate (a): sum-aggregate `Construct` dsts +
/// `Apply` / `Invoke` results whose callee contract hands the caller an owned
/// reference (`uniqueness ∈ {Unique, MaybeShared}`) without transferring an
/// arg through the return.
fn collect_fresh_sum_roots(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> Vec<ArcVarId> {
    let owned_result_non_forwarder = |callee: &Name| -> bool {
        contracts.get(callee).is_some_and(|c| {
            matches!(
                c.return_info.uniqueness,
                Uniqueness::Unique | Uniqueness::MaybeShared
            ) && !c.params.iter().any(|p| p.transfers_through_return)
        })
    };
    let mut roots: Vec<ArcVarId> = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Construct {
                    dst,
                    ctor: CtorKind::EnumVariant { .. },
                    ..
                } => roots.push(*dst),
                ArcInstr::Apply {
                    dst, func: callee, ..
                } if owned_result_non_forwarder(callee) => roots.push(*dst),
                _ => {}
            }
        }
        if let ArcTerminator::Invoke {
            dst, func: callee, ..
        } = &block.terminator
        {
            if owned_result_non_forwarder(callee) {
                roots.push(*dst);
            }
        }
    }
    roots
}

/// Gate (c): the root's type is a niche-family sum — variant entries present,
/// no self heap allocation, no struct fields, no element burden, every variant
/// at most ONE owned payload (transfer-on-match binding or retained field).
/// The wrapper then carries no allocation of its own and its RC identity is
/// the single live payload — one release frees the whole web.
fn is_niche_family_sum(func: &ArcFunction, root: ArcVarId, type_registry: &TypeRegistry) -> bool {
    let ty: TypeRef = idx_to_type_ref(func.var_types[root.index()], type_registry);
    let Some(burden) = lookup_burden(ty, type_registry) else {
        return false;
    };
    if burden.self_heap_alloc()
        || burden.owned_fields().next().is_some()
        || burden.element_burden().is_some()
    {
        return false;
    }
    let mut any_variant = false;
    for v in burden.variant_burdens() {
        any_variant = true;
        if v.transfers_on_match.len() + v.retained_owned.len() > 1 {
            return false;
        }
    }
    any_variant
}

/// Gate (d): grow the same-alloc closure from `root` (Let-Var aliases +
/// non-scalar `Project` borrow-views + `Jump`-arg → block-param hops), then
/// vet every member use as a pure borrow-read. `None` on any vet failure.
fn same_alloc_closure_vetted(func: &ArcFunction, root: ArcVarId) -> Option<FxHashSet<ArcVarId>> {
    let mut members: FxHashSet<ArcVarId> = FxHashSet::default();
    members.insert(root);
    loop {
        let mut grew = false;
        for block in &func.blocks {
            for instr in &block.body {
                match instr {
                    ArcInstr::Let {
                        dst,
                        value: ArcValue::Var(src),
                        ..
                    } if members.contains(src) && members.insert(*dst) => grew = true,
                    // Non-scalar Project dsts are the niche payload
                    // borrow-views sharing the allocation (TF-4); scalar tag
                    // reads drop out of the closure.
                    ArcInstr::Project { dst, value, .. }
                        if members.contains(value)
                            && !is_provably_scalar_repr(func, *dst)
                            && members.insert(*dst) =>
                    {
                        grew = true;
                    }
                    _ => {}
                }
            }
            if let ArcTerminator::Jump { target, args } = &block.terminator {
                for (pos, &arg) in args.iter().enumerate() {
                    if members.contains(&arg) {
                        if let Some(&(param, _)) = func.blocks[target.index()].params.get(pos) {
                            if members.insert(param) {
                                grew = true;
                            }
                        }
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }

    // Vet every member use as a borrow-read.
    for block in &func.blocks {
        for instr in &block.body {
            let touches_member = instr.used_vars().iter().any(|v| members.contains(v));
            if !touches_member {
                continue;
            }
            match instr {
                // Alias hops + borrow-view projections are the closure's own
                // edges (scalar tag projections are borrow-reads too).
                ArcInstr::Let {
                    value: ArcValue::Var(_),
                    ..
                }
                | ArcInstr::Project { .. } => {}
                // COW / conditional-alias / mutation / reuse machinery on a
                // member is a distinct sub-root (the Select-branch shape
                // double-frees under a single-release treatment) — decline.
                ArcInstr::Select { .. }
                | ArcInstr::IsShared { .. }
                | ArcInstr::Reset { .. }
                | ArcInstr::Set { .. }
                | ArcInstr::SetTag { .. }
                | ArcInstr::Reuse { .. }
                | ArcInstr::CollectionReuse { .. } => return None,
                // A closure capture retains a reference — a genuine
                // duplication out of family.
                ArcInstr::PartialApply { .. } => return None,
                ArcInstr::Apply { dst, .. } => {
                    // Owned-position consume = transfer out of family.
                    for (pos, v) in instr.used_vars().iter().enumerate() {
                        if members.contains(v) && instr.is_owned_position(pos) {
                            return None;
                        }
                    }
                    // A borrowed read must provably NOT alias the member into
                    // its result: require a provably-scalar result. The
                    // protocol builtins (`__index` self-inc, `iter` consume)
                    // and user callees returning heap all decline.
                    if !is_provably_scalar_repr(func, *dst) {
                        return None;
                    }
                }
                ArcInstr::ApplyIndirect { .. } => return None,
                // Owned-position consume at any other instruction = transfer;
                // a list-concat `PrimOp Binary(Add)` consumes its `RcPointer`
                // operands (the dual-consuming runtime contract) — decline.
                _ => {
                    for (pos, v) in instr.used_vars().iter().enumerate() {
                        if members.contains(v) && instr.is_owned_position(pos) {
                            return None;
                        }
                    }
                    if super::list_concat_consumed_operands(instr, func)
                        .iter()
                        .any(|v| members.contains(v))
                    {
                        return None;
                    }
                }
            }
        }
        let term = &block.terminator;
        let term_touches = term.used_vars().iter().any(|v| members.contains(v));
        if !term_touches {
            continue;
        }
        match term {
            // Jump hops are the closure's own param edges.
            ArcTerminator::Jump { .. } => {}
            ArcTerminator::Invoke { dst, .. } => {
                for (pos, &v) in term.used_vars().iter().enumerate() {
                    if members.contains(&v) && term.is_owned_position(pos) {
                        return None;
                    }
                }
                // Borrowed terminator read: same provably-scalar-result vet
                // as the body `Apply` arm.
                if !is_provably_scalar_repr(func, *dst) {
                    return None;
                }
            }
            // Escapes / unvettable consumers.
            ArcTerminator::InvokeIndirect { .. }
            | ArcTerminator::Return { .. }
            | ArcTerminator::Branch { .. }
            | ArcTerminator::Switch { .. } => return None,
            ArcTerminator::Resume | ArcTerminator::Unreachable => {}
        }
    }
    Some(members)
}
