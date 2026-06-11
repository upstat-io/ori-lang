//! Root collection + gate-(c)/(c2) helpers for the borrowed-`Invoke` lineage
//! scan: the FRESH collection-`Construct` + borrowed-call-RESULT root families
//! and the borrowed-arg / iter-consume member probes. Split from
//! `borrowed_invoke_lineage.rs` for the 500-line cap. Spec: Annex E §AIMS RL-2.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, CtorKind};

/// Gate (a): candidate roots — every FRESH heap-`Construct` dst:
///  - a collection buffer `Construct { ctor: ListLiteral | MapLiteral |
///    SetLiteral }`, OR
///  - a sum-aggregate `Construct { ctor: EnumVariant }` (Option / Result / user
///    sum) whose dst is in `owned_vars_needing_rc` — the same family the
///    construct-fed dead-param scan admits (`is_sum_aggregate_construct_rep`).
///    The `owned_vars_needing_rc` gate excludes all-scalar-payload sum
///    instantiations (`Option<int>`: niche-packed, no RC header), keeping the
///    no-sink / dead-param treatment scoped to genuine heap lineages.
///
/// Plain `Struct` ctors stay DECLINED (no failing cell; a wider admission is
/// over-fire surface without evidence). A `Let { Literal::String }` heap-str body
/// is NOT a candidate (no `Construct` definer; string-literal lineages decline
/// naturally).
pub(super) fn collect_fresh_construct_roots(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> Vec<ArcVarId> {
    let mut roots: Vec<ArcVarId> = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Construct {
                    dst,
                    ctor: CtorKind::ListLiteral | CtorKind::MapLiteral | CtorKind::SetLiteral,
                    ..
                } => roots.push(*dst),
                ArcInstr::Construct {
                    dst,
                    ctor: CtorKind::EnumVariant { .. },
                    ..
                } if owned_vars_needing_rc.contains(dst) => roots.push(*dst),
                _ => {}
            }
        }
    }
    roots
}

/// Candidate RESULT roots: a heap `Invoke` result of a may-unwind USER call
/// (`@first(xs) = xs[0]` returns a self-inc'd element) that the callee returns
/// owned (`return_info.uniqueness ∈ {Unique, MaybeShared}`) WITHOUT transferring
/// a param through the return (a forwarder result is the ARG's allocation, owned
/// by the forwarder scans) and WITHOUT iter-consuming a param (the iter-consume
/// transfer owns the release). The result's FRESH-site inc is spurious — the
/// callee already handed +1 — and the result, borrow-read-only, dies at a dead
/// block-param; the suppress + dead-param release frees the callee's +1 exactly
/// once. The death-point + vet gates ([`choose_dead_param_release_site`] +
/// [`same_alloc_closure_vetted`]) bound the over-fire surface.
pub(super) fn collect_borrowed_call_result_roots(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> (Vec<ArcVarId>, FxHashSet<ArcVarId>) {
    let owned_result_non_forwarder_non_iter = |callee: &Name| -> bool {
        contracts.get(callee).is_some_and(|c| {
            matches!(
                c.return_info.uniqueness,
                crate::aims::lattice::Uniqueness::Unique
                    | crate::aims::lattice::Uniqueness::MaybeShared
            ) && !c.params.iter().any(|p| p.transfers_through_return)
                && !c.params.iter().any(|p| p.iter_consumes)
        })
    };
    // PROVABLY-FRESH result: the callee's contract proves the returned value is
    // a NEW allocation (`Unique` + `preserves_freshness`), never a same-buffer
    // view (`@substring` / `@repeat` are ttr / non-fresh and stay excluded).
    // Only this subset is safe for the NO-SINK edge-death claim — an edge
    // release on a view double-frees the viewed buffer. Spec: Annex E §AIMS
    // RL-2.
    let provably_fresh = |callee: &Name| -> bool {
        contracts.get(callee).is_some_and(|c| {
            c.return_info.uniqueness == crate::aims::lattice::Uniqueness::Unique
                && c.return_info.preserves_freshness
                && !c.params.iter().any(|p| p.transfers_through_return)
                && !c.params.iter().any(|p| p.iter_consumes)
        })
    };
    let mut roots: Vec<ArcVarId> = Vec::new();
    let mut fresh: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        if let ArcTerminator::Invoke {
            dst, func: callee, ..
        } = &block.terminator
        {
            if owned_vars_needing_rc.contains(dst) && owned_result_non_forwarder_non_iter(callee) {
                roots.push(*dst);
                if provably_fresh(callee) {
                    fresh.insert(*dst);
                }
            }
        }
    }
    (roots, fresh)
}

/// Gate (c): true iff any closure member is used at a BORROWED (non-owned) arg
/// position of an `Invoke` / `InvokeIndirect` terminator — the carrier whose
/// inline-before-terminator `BurdenDec` is the use-after-free.
pub(super) fn closure_has_borrowed_invoke_arg(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
) -> bool {
    for block in &func.blocks {
        let term = &block.terminator;
        if !matches!(
            term,
            ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. }
        ) {
            continue;
        }
        for (pos, &v) in term.used_vars().iter().enumerate() {
            if members.contains(&v) && !term.is_owned_position(pos) {
                return true;
            }
        }
    }
    false
}

/// Gate (c2): true iff any closure member is used at a borrowed `Invoke` /
/// `InvokeIndirect` arg position whose callee `iter_consumes` that position (a
/// `for w in coll` inside the callee → its `ori_iter_drop` frees the buffer; an
/// RL-2 ownership transfer DESPITE the borrowed arg position, per
/// `ParamContract.iter_consumes`). The callee owns the release, so a dead-param
/// release on such a lineage double-frees. Mirrors the `iter_consumes` detection
/// in `compute_ttr_iter_consume_dup_aliases` (the same SSOT signal).
pub(super) fn closure_member_iter_consumed_at_invoke(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> bool {
    for block in &func.blocks {
        // Only a named-callee `Invoke` carries a contract with `iter_consumes`;
        // an `InvokeIndirect` (closure) has no contract — conservatively NOT an
        // iter-consume transfer here (it declines elsewhere via the `Apply
        // Indirect` vet anyway).
        if let ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            let Some(contract) = contracts.get(callee) else {
                continue;
            };
            for (pos, &arg) in args.iter().enumerate() {
                if members.contains(&arg)
                    && contract.params.get(pos).is_some_and(|p| p.iter_consumes)
                {
                    return true;
                }
            }
        }
    }
    false
}
