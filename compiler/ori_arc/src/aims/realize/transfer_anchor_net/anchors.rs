//! Phase-6.99 transfer-anchor admission — the conservative over-fire boundary.
//!
//! Two anchor classes, each PROVEN by the callee contract + IR agreement:
//!
//! - **Direct** (RL-34): every aliasing param is `transfers_through_return ∧
//!   Owned ∧ return_alias == Direct` — the caller re-acquires the SAME
//!   allocation at `dst`.
//! - **Wrapped** (RL-1 / RL-2): every aliasing param is
//!   `return_payload_contains_param ∧ return_payload_contains_param_all_paths`
//!   (no Direct/Project alias, no ttr) and the RESULT type is a proven
//!   same-allocation wrapper (builtin `Option`/`Result`, `Aggregate` repr,
//!   non-recursive) — the returned wrapper carries exactly ONE reference on
//!   the payload's allocation: minted by the callee's RL-1 borrowed-store
//!   dup-inc (Borrowed param) or transferred through the store (Owned param,
//!   RL-2). The per-path proof excludes wrap-on-some-paths callees whose
//!   wrapper credit would be path-dependent (an over-count = double-free).
//!
//! Toggle: `ORI_DISABLE_WRAPPED_CREDIT_ANCHOR=1` declines the Wrapped class
//! (Direct admission unchanged).
//!
//! Spec: Annex E §AIMS RL-34 + RL-2 + RL-1 + TF-4.

use std::sync::LazyLock;

use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::FxHashMap;

use crate::aims::contract::{MemoryContract, ParamContract, ReturnAliasShape};
use crate::aims::lattice::AccessClass;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ValueRepr};

use super::super::emit_unified::is_burden_carrying_aggregate;
use super::views::same_alloc_wrapper_type;

/// `ORI_DISABLE_WRAPPED_CREDIT_ANCHOR=1` declines the WRAPPED transfer-anchor
/// class (the `Ok(m)`-style wrap-forwarder credit). Read once at first access.
static WRAPPED_CREDIT_ANCHOR_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_WRAPPED_CREDIT_ANCHOR").as_deref() == Ok("1"));

/// Anchor class — which contract proof admitted the call site.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AnchorKind {
    /// `transfers_through_return ∧ Owned ∧ Direct`: `dst` IS the transferred
    /// arg's allocation.
    Direct,
    /// `return_payload_contains_param` on every path: `dst` is a
    /// same-allocation wrapper CONTAINING the payload arg's allocation.
    Wrapped,
}

/// A proven transfer-anchor call site: the caller acquires a reference on the
/// payload's allocation at `dst` (the credit), the `payload_args` naming that
/// same allocation lineage.
pub(super) struct Anchor {
    pub(super) dst: ArcVarId,
    /// Args whose allocation `dst` re-names (Direct) or contains (Wrapped) —
    /// the lineage-union surface. Hand-off `-1` events come from the
    /// `[own]`-position use modeling, not from this list.
    pub(super) payload_args: Vec<ArcVarId>,
    /// Where the credit materializes.
    pub(super) credit: CreditSite,
    pub(super) kind: AnchorKind,
}

/// The point where the transferred-in `+1` becomes live in the caller.
pub(super) enum CreditSite {
    /// Body `Apply` anchor: live immediately after the call instruction.
    AfterBodyInstr { block: usize, instr_idx: usize },
    /// `Invoke` anchor: live at the NORMAL successor's entry (the unwind
    /// edge never binds `dst`).
    BlockEntry { block: usize },
}

/// Collect the admitted transfer-anchor call sites (Direct then Wrapped per
/// site; a site admitted by neither contributes no anchor).
///
/// Direct admission (ALL required):
/// - the callee has a contract whose ALIASING params (`return_alias.is_some()
///   ∨ return_payload_contains_param`) are EVERY one
///   `transfers_through_return ∧ access == Owned ∧ return_alias == Direct`;
/// - at least one aliasing param exists (the result IS a transferred arg);
/// - the IR annotates each aliasing position `[own]` (contract/IR agreement);
/// - the result repr is RC-bearing (`RcPointer` / `FatValue` /
///   burden-carrying `Aggregate`).
///
/// Wrapped admission (ALL required):
/// - every aliasing param proves `return_payload_contains_param ∧
///   return_payload_contains_param_all_paths ∧ return_alias == None ∧
///   ¬transfers_through_return ∧ ¬iter_consumes`;
/// - the IR position agrees with the param access (`Owned` ↔ `[own]`,
///   `Borrowed` ↔ borrow);
/// - the result is a burden-carrying same-allocation wrapper
///   ([`same_alloc_wrapper_type`]) — a BOXED wrapper (own allocation)
///   declines (two-allocation field-drop coupling is not modeled).
pub(super) fn collect_anchors(
    func: &ArcFunction,
    pool: &Pool,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> Vec<Anchor> {
    let mut anchors = Vec::new();
    let rc_bearing = |v: ArcVarId| {
        matches!(
            func.var_repr(v),
            Some(ValueRepr::RcPointer | ValueRepr::FatValue)
        ) || is_burden_carrying_aggregate(v, func, pool)
    };
    for (b, block) in func.blocks.iter().enumerate() {
        for (i, instr) in block.body.iter().enumerate() {
            if let ArcInstr::Apply {
                dst,
                func: callee,
                args,
                ..
            } = instr
            {
                if !rc_bearing(*dst) {
                    continue;
                }
                let owned_at = |p: usize| instr.is_owned_position(p);
                let credit = CreditSite::AfterBodyInstr {
                    block: b,
                    instr_idx: i,
                };
                if let Some(anchor) = admit_site(
                    func, pool, contracts, *callee, *dst, args, &owned_at, credit,
                ) {
                    anchors.push(anchor);
                }
            }
        }
        if let ArcTerminator::Invoke {
            dst,
            func: callee,
            args,
            normal,
            ..
        } = &block.terminator
        {
            if !rc_bearing(*dst) {
                continue;
            }
            let owned_at = |p: usize| block.terminator.is_owned_position(p);
            let credit = CreditSite::BlockEntry {
                block: normal.index(),
            };
            if let Some(anchor) = admit_site(
                func, pool, contracts, *callee, *dst, args, &owned_at, credit,
            ) {
                anchors.push(anchor);
            }
        }
    }
    anchors
}

/// Admit one call site as Direct, else Wrapped, else `None`.
#[expect(
    clippy::too_many_arguments,
    reason = "single admission seam shared by the Apply and Invoke call \
              shapes; bundling into a struct fragments the per-site check"
)]
fn admit_site(
    func: &ArcFunction,
    pool: &Pool,
    contracts: &FxHashMap<Name, MemoryContract>,
    callee: Name,
    dst: ArcVarId,
    args: &[ArcVarId],
    owned_at: &dyn Fn(usize) -> bool,
    credit: CreditSite,
) -> Option<Anchor> {
    if let Some(positions) = admitted_transfer_positions(contracts, callee, args.len()) {
        if positions.iter().all(|&p| owned_at(p)) {
            return Some(Anchor {
                dst,
                payload_args: positions.iter().map(|&p| args[p]).collect(),
                credit,
                kind: AnchorKind::Direct,
            });
        }
        return None;
    }
    let positions = admitted_wrapped_positions(contracts, callee, args.len())?;
    if !same_alloc_wrapper_type(func, dst, pool) || !is_burden_carrying_aggregate(dst, func, pool) {
        return None;
    }
    let ir_agrees = positions.iter().all(|&(p, access)| match access {
        AccessClass::Owned => owned_at(p),
        AccessClass::Borrowed => !owned_at(p),
    });
    if !ir_agrees {
        return None;
    }
    Some(Anchor {
        dst,
        payload_args: positions.iter().map(|&(p, _)| args[p]).collect(),
        credit,
        kind: AnchorKind::Wrapped,
    })
}

/// The arg positions whose params prove `transfers_through_return ∧ Owned ∧
/// Direct`, or `None` when the site is not an admitted Direct anchor.
fn admitted_transfer_positions(
    contracts: &FxHashMap<Name, MemoryContract>,
    callee: Name,
    n_args: usize,
) -> Option<Vec<usize>> {
    let contract = contracts.get(&callee)?;
    let mut positions = Vec::new();
    for (i, p) in contract.params.iter().enumerate().take(n_args) {
        if !is_aliasing(p) {
            continue;
        }
        if !(p.transfers_through_return
            && p.access == AccessClass::Owned
            && p.return_alias == Some(ReturnAliasShape::Direct))
        {
            return None;
        }
        positions.push(i);
    }
    if positions.is_empty() {
        None
    } else {
        Some(positions)
    }
}

/// The `(position, access)` pairs whose params prove the all-paths WRAPPED
/// containment shape, or `None` when the site is not an admitted wrapped
/// anchor (any non-wrapped aliasing param declines the whole site).
fn admitted_wrapped_positions(
    contracts: &FxHashMap<Name, MemoryContract>,
    callee: Name,
    n_args: usize,
) -> Option<Vec<(usize, AccessClass)>> {
    if *WRAPPED_CREDIT_ANCHOR_DISABLED {
        return None;
    }
    let contract = contracts.get(&callee)?;
    let mut positions = Vec::new();
    for (i, p) in contract.params.iter().enumerate().take(n_args) {
        if !is_aliasing(p) {
            continue;
        }
        if !(p.return_payload_contains_param
            && p.return_payload_contains_param_all_paths
            && p.return_alias.is_none()
            && !p.transfers_through_return
            && !p.iter_consumes)
        {
            return None;
        }
        positions.push((i, p.access));
    }
    if positions.is_empty() {
        None
    } else {
        Some(positions)
    }
}

/// Whether the param's contract claims its allocation reaches the result
/// (alias or containment) — the anchor-relevance test.
fn is_aliasing(p: &ParamContract) -> bool {
    p.return_alias.is_some() || p.return_payload_contains_param
}
