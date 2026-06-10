//! Phase-6.99 lineage model — ordered per-block credit-net events.
//!
//! Builds the [`LineageModel`] for one anchored rep: fresh births (+1),
//! surviving whole-var ops (±1), `[own]` hand-offs (-1), the transfer-anchor
//! CREDIT (+1 at its [`CreditSite`]), member/view value-reads
//! (aliveness-required), or `None` when any member/view use falls outside the
//! modeled vocabulary (the prove-or-change-nothing boundary).
//!
//! WRAPPED-anchor reps additionally model: a member bound by a NON-anchor
//! call whose contract proves a FRESH OWNED result (+1 acquisition), a
//! same-allocation wrapper EXTRACTION (`Project` re-naming the payload —
//! Neutral, TF-4), and a Scalar literal member (a dead-arm merge
//! placeholder — Neutral). Direct-only reps model EXACTLY as before.
//!
//! Spec: Annex E §AIMS RL-34 + RL-2 + RL-1.

use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::Uniqueness;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr};

use super::super::emit_unified::is_burden_carrying_aggregate;
use super::anchors::{Anchor, AnchorKind, CreditSite};
use super::uses::{model_body_uses, model_terminator_uses};
use super::views::{compute_view_chain, view_ops_balanced};

/// One ordered net event inside a block.
///
/// `alive: true` events REQUIRE the running net `>= 1` before they apply (a
/// member must be alive when read / transferred / inc'd / dec'd); the
/// intra-block walk in [`classify_model`] enforces this (RL-2: release at
/// last use, never before).
#[derive(Clone, Copy)]
pub(super) struct Event {
    pub(super) alive: bool,
    pub(super) delta: i64,
}

pub(super) const READ_EVENT: Event = Event {
    alive: true,
    delta: 0,
};
const ALLOC_EVENT: Event = Event {
    alive: false,
    delta: 1,
};
const CREDIT_EVENT: Event = Event {
    alive: false,
    delta: 1,
};
pub(super) const INC_EVENT: Event = Event {
    alive: true,
    delta: 1,
};
pub(super) const DEC_EVENT: Event = Event {
    alive: true,
    delta: -1,
};
pub(super) const OWN_ARG_EVENT: Event = Event {
    alive: true,
    delta: -1,
};

/// Ordered per-block events: `entry_credit` applies at block ENTRY (an
/// `Invoke` anchor's normal successor or a fresh `Invoke` acquisition),
/// then `body` in instruction order, then `term`.
#[derive(Default, Clone)]
pub(super) struct BlockEvents {
    pub(super) entry_credit: i64,
    pub(super) body: Vec<Event>,
    pub(super) term: Vec<Event>,
}

/// A surviving whole-var inc on a FRESH-SITE-defined member (removal candidate).
pub(super) struct FreshSiteInc {
    pub(super) block: usize,
    pub(super) instr_idx: usize,
    pub(super) event_idx: usize,
}

/// Per-sink value-read record for one block.
#[derive(Default)]
pub(super) struct BlockReads {
    /// Last member value-read by a BODY instruction (the var read).
    pub(super) last_body: Option<ArcVarId>,
    /// Member value-read at the block TERMINATOR (borrowed Invoke arg).
    pub(super) terminator: Option<ArcVarId>,
    /// A VIEW value-read at the block TERMINATOR.
    pub(super) view_terminator: bool,
}

/// The modeled lineage: ordered per-block credit-net events + repair surfaces.
pub(super) struct LineageModel {
    pub(super) events: Vec<BlockEvents>,
    pub(super) fresh_site_incs: Vec<FreshSiteInc>,
    /// Blocks with member value-reads (placement sink candidates).
    pub(super) read_blocks: FxHashMap<usize, BlockReads>,
}

/// Member-definition kind for the fresh-carry / decline classification.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DefKind {
    FreshSite,
    Neutral,
}

/// Build the ordered per-block credit-net events for `rep`'s lineage, or
/// `None` when any member definition / use falls outside the modeled
/// vocabulary (the prove-or-change-nothing boundary).
pub(super) fn build_lineage_model(
    func: &ArcFunction,
    pool: &Pool,
    contracts: &FxHashMap<Name, MemoryContract>,
    rep_of: &dyn Fn(ArcVarId) -> ArcVarId,
    rep: ArcVarId,
    anchors: &[&Anchor],
) -> Option<LineageModel> {
    let n = func.blocks.len();
    let mut model = LineageModel {
        events: vec![BlockEvents::default(); n],
        fresh_site_incs: Vec::new(),
        read_blocks: FxHashMap::default(),
    };
    // Wrapped-anchor reps admit the extraction / fresh-acquisition /
    // dead-arm-scalar shapes; Direct-only reps model EXACTLY as before.
    let wrapped = anchors.iter().any(|a| a.kind == AnchorKind::Wrapped);
    let anchor_dsts: FxHashSet<ArcVarId> = anchors.iter().map(|a| a.dst).collect();
    // Apply-anchor credits keyed by (block, instr_idx) — the credit event is
    // emitted immediately AFTER that instruction's own events.
    let mut apply_credits: FxHashSet<(usize, usize)> = FxHashSet::default();
    for anchor in anchors {
        match anchor.credit {
            CreditSite::AfterBodyInstr { block, instr_idx } => {
                apply_credits.insert((block, instr_idx));
            }
            CreditSite::BlockEntry { block } => {
                if block < n {
                    model.events[block].entry_credit += 1;
                }
            }
        }
    }

    let is_member = |v: ArcVarId| rep_of(v) == rep;
    // Function-param member = caller-retained allocation, NOT caller-fresh.
    if func.params.iter().any(|p| is_member(p.var)) {
        return None;
    }

    // Same-allocation VIEW chain (TF-4 — a niche-payload borrow-view shares
    // the member's allocation). SAME-ALLOC-proven view RC ops join the
    // unified ledger at `±1` (counted in `model_body_uses`); OPAQUE view RC
    // ops MUST be per-block balanced pairs (validated here, then excluded as
    // net-0) — a lone opaque half means the allocation's accounting extends
    // beyond this model, so decline.
    let views = compute_view_chain(func, pool, &is_member);
    if !view_ops_balanced(func, &views.opaque) {
        return None;
    }

    // Member def-kinds (for fresh-carry +1 and removal-candidate gating).
    let mut def_kind: FxHashMap<ArcVarId, DefKind> = FxHashMap::default();
    // Fresh NON-anchor Apply acquisitions whose +1 lands AFTER the call's
    // own use events (the result exists only once the call returns).
    let mut post_instr_births: FxHashSet<(usize, usize)> = FxHashSet::default();

    for (b, block) in func.blocks.iter().enumerate() {
        for (i, instr) in block.body.iter().enumerate() {
            if let Some(dst) = instr.defined_var() {
                if is_member(dst) {
                    if wrapped
                        && !anchor_dsts.contains(&dst)
                        && matches!(instr, ArcInstr::Apply { .. })
                    {
                        // Non-anchor Apply binding a member: admit ONLY a
                        // proven fresh owned acquisition (+1); anything
                        // else is a foreign acquisition — unmodeled.
                        let ArcInstr::Apply { func: callee, .. } = instr else {
                            unreachable!("guarded by the matches! above")
                        };
                        if !fresh_owned_result(contracts, *callee) || !rc_bearing(func, pool, dst) {
                            return None;
                        }
                        def_kind.insert(dst, DefKind::FreshSite);
                        post_instr_births.insert((b, i));
                    } else {
                        model_member_def(
                            func,
                            pool,
                            instr,
                            dst,
                            b,
                            wrapped,
                            &is_member,
                            &anchor_dsts,
                            &mut model,
                            &mut def_kind,
                        )?;
                    }
                }
            }
            model_body_uses(
                contracts, &is_member, &views, instr, b, i, &def_kind, &mut model,
            )?;
            // Fresh-acquisition / Apply-anchor credit: live immediately
            // after the call.
            if post_instr_births.contains(&(b, i)) {
                model.events[b].body.push(ALLOC_EVENT);
            }
            if apply_credits.contains(&(b, i)) {
                model.events[b].body.push(CREDIT_EVENT);
            }
        }
        model_invoke_member_dst(
            func,
            pool,
            contracts,
            &block.terminator,
            wrapped,
            &is_member,
            &anchor_dsts,
            &mut model,
            &mut def_kind,
        )?;
        model_terminator_uses(
            contracts,
            &is_member,
            &views,
            &block.terminator,
            b,
            &mut model,
        )?;
    }

    Some(model)
}

/// Classify a member bound by a block's `Invoke` terminator: an anchor dst
/// is the credit (modeled separately); a NON-anchor binding is a foreign
/// acquisition — unmodeled (`None`) — UNLESS (wrapped reps) the callee
/// contract proves a fresh owned result: +1 acquired at the NORMAL
/// successor's entry (the unwind edge never binds `dst`).
#[expect(
    clippy::too_many_arguments,
    reason = "single classification seam threading the lineage-model build \
              context; bundling into a struct fragments the one-walk shape"
)]
fn model_invoke_member_dst(
    func: &ArcFunction,
    pool: &Pool,
    contracts: &FxHashMap<Name, MemoryContract>,
    terminator: &ArcTerminator,
    wrapped: bool,
    is_member: &dyn Fn(ArcVarId) -> bool,
    anchor_dsts: &FxHashSet<ArcVarId>,
    model: &mut LineageModel,
    def_kind: &mut FxHashMap<ArcVarId, DefKind>,
) -> Option<()> {
    let ArcTerminator::Invoke {
        dst,
        func: callee,
        normal,
        ..
    } = terminator
    else {
        return Some(());
    };
    if !is_member(*dst) || anchor_dsts.contains(dst) {
        return Some(());
    }
    if !(wrapped && fresh_owned_result(contracts, *callee) && rc_bearing(func, pool, *dst)) {
        return None;
    }
    def_kind.insert(*dst, DefKind::FreshSite);
    if normal.index() < model.events.len() {
        model.events[normal.index()].entry_credit += 1;
    }
    Some(())
}

/// Classify a member DEFINITION: fresh births emit `ALLOC_EVENT`; aliases and
/// anchor results are neutral; wrapped reps additionally admit
/// same-allocation extractions (`Project` of a member) and Scalar literal
/// placeholders as neutral; anything else is unmodeled (`None`).
#[expect(
    clippy::too_many_arguments,
    reason = "single classification seam threading the lineage-model build \
              context; bundling into a struct fragments the one-walk shape"
)]
fn model_member_def(
    func: &ArcFunction,
    pool: &Pool,
    instr: &ArcInstr,
    dst: ArcVarId,
    b: usize,
    wrapped: bool,
    is_member: &dyn Fn(ArcVarId) -> bool,
    anchor_dsts: &FxHashSet<ArcVarId>,
    model: &mut LineageModel,
    def_kind: &mut FxHashMap<ArcVarId, DefKind>,
) -> Option<()> {
    match instr {
        ArcInstr::Construct { .. } => {
            if rc_bearing(func, pool, dst) {
                model.events[b].body.push(ALLOC_EVENT);
            }
            def_kind.insert(dst, DefKind::FreshSite);
            Some(())
        }
        ArcInstr::Let {
            value: ArcValue::Literal(crate::ir::LitValue::String(_)),
            ..
        } => {
            model.events[b].body.push(ALLOC_EVENT);
            def_kind.insert(dst, DefKind::FreshSite);
            Some(())
        }
        // Wrapped reps: a Scalar literal member is a dead-arm merge
        // placeholder (a unit/int phi filler jump-threaded into the rep) —
        // no allocation, no refs (ledger-inert).
        ArcInstr::Let {
            value: ArcValue::Literal(_),
            ..
        } if wrapped && matches!(func.var_repr(dst), Some(ValueRepr::Scalar)) => {
            def_kind.insert(dst, DefKind::Neutral);
            Some(())
        }
        // Wrapped reps: a non-scalar Project of a member re-names the SAME
        // allocation (the extract-union admitted it) — the destructure
        // re-transfer, TF-4. The projection's READ of the source member is
        // modeled by `model_body_uses`.
        ArcInstr::Project { value, .. } if wrapped && is_member(*value) => {
            def_kind.insert(dst, DefKind::Neutral);
            Some(())
        }
        ArcInstr::Let {
            value: ArcValue::Var(_),
            ..
        }
        // An ANCHOR Apply's result acquisition is the credit (modeled
        // separately); a non-anchor Apply result is a foreign acquisition
        // (wrapped fresh acquisitions are pre-routed by the build loop).
        | ArcInstr::Apply { .. }
            if !matches!(instr, ArcInstr::Apply { .. }) || anchor_dsts.contains(&dst) =>
        {
            def_kind.insert(dst, DefKind::Neutral);
            Some(())
        }
        // Foreign acquisition / reuse / projection / scalar literal / PrimOp
        // defining a member — unmodeled.
        _ => None,
    }
}

/// Whether `callee`'s contract proves its result is a FRESH OWNED
/// acquisition for the caller: a unique, freshness-preserving return with
/// NO param aliasing the result (the result is not derived from any arg).
fn fresh_owned_result(contracts: &FxHashMap<Name, MemoryContract>, callee: Name) -> bool {
    let Some(c) = contracts.get(&callee) else {
        return false;
    };
    c.return_info.uniqueness == Uniqueness::Unique
        && c.return_info.preserves_freshness
        && c.params
            .iter()
            .all(|p| p.return_alias.is_none() && !p.return_payload_contains_param)
}

/// Whether `v`'s repr carries RC (`RcPointer` / `FatValue` / burden-carrying
/// `Aggregate`).
fn rc_bearing(func: &ArcFunction, pool: &Pool, v: ArcVarId) -> bool {
    matches!(
        func.var_repr(v),
        Some(ValueRepr::RcPointer | ValueRepr::FatValue)
    ) || is_burden_carrying_aggregate(v, func, pool)
}
