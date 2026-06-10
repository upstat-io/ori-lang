//! Phase-6.99 same-allocation view chain (TF-4 niche-payload borrow-views).
//!
//! A non-scalar `Project` dst of a member shares the member's allocation.
//! Views split into two classes:
//!
//! - **Same-alloc** views: every projection hop is off a PROVEN single-payload
//!   niche-family wrapper (`Option` / `Result` — at most one RC-bearing payload
//!   per variant, `Aggregate` repr, non-recursive), so a whole-var RC op on the
//!   view and on the member change the SAME allocation count. These ops join
//!   the unified per-rep ledger at `±1` (the lone niche-payload release IS the
//!   lineage's release).
//! - **Opaque** views: any hop whose source type is not a proven single-payload
//!   wrapper (collection-element extraction, multi-field aggregates). Their RC
//!   ops MUST be per-block balanced pairs or the lineage is declined —
//!   field-grain attribution is not provable at this layer.
//!
//! Toggle: `ORI_DISABLE_VIEW_LEDGER_ADMISSION=1` forces every view Opaque
//! (the balanced-pair-or-decline treatment).
//!
//! Spec: Annex E §AIMS RL-1 + RL-2 + TF-4.

use std::sync::LazyLock;

use ori_types::{Pool, Tag};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId, ValueRepr};

/// `ORI_DISABLE_VIEW_LEDGER_ADMISSION=1` forces every same-allocation view
/// into the Opaque (balanced-pair-or-decline) class. Read once at first
/// access.
static VIEW_LEDGER_ADMISSION_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_VIEW_LEDGER_ADMISSION").as_deref() == Ok("1"));

/// The classified same-allocation view chain of one rep's members.
#[derive(Default)]
pub(super) struct ViewChain {
    /// Views provably naming the member's own allocation (unified ledger).
    pub(super) same_alloc: FxHashSet<ArcVarId>,
    /// Views whose allocation identity is unproven (balanced-pair-or-decline).
    pub(super) opaque: FxHashSet<ArcVarId>,
}

impl ViewChain {
    /// Whether `v` is in either view class.
    pub(super) fn contains(&self, v: ArcVarId) -> bool {
        self.same_alloc.contains(&v) || self.opaque.contains(&v)
    }
}

/// The view chain of `rep`'s members: non-scalar `Project` dsts whose value is
/// a member or a view, plus `Let { Var }` aliases of views, to a fixpoint.
/// Scalar projections (tag / int extracts) drop out. Each view carries its
/// class per the module-level same-alloc proof; a var reachable as both
/// classes is Opaque (conservative). MEMBERS never enter the chain —
/// membership takes precedence (a wrapped-rep extraction unioned into the
/// lineage is a member, not a view).
pub(super) fn compute_view_chain(
    func: &ArcFunction,
    pool: &Pool,
    is_member: &dyn Fn(ArcVarId) -> bool,
) -> ViewChain {
    let mut views = ViewChain::default();
    loop {
        let mut grew = false;
        for block in &func.blocks {
            for instr in &block.body {
                match instr {
                    ArcInstr::Project { dst, value, .. }
                        if (is_member(*value) || views.contains(*value))
                            && !matches!(func.var_repr(*dst), Some(ValueRepr::Scalar))
                            && !is_member(*dst)
                            && !views.contains(*dst) =>
                    {
                        let source_same_alloc =
                            is_member(*value) || views.same_alloc.contains(value);
                        if source_same_alloc && is_same_alloc_wrapper(func, *value, pool) {
                            views.same_alloc.insert(*dst);
                        } else {
                            views.opaque.insert(*dst);
                        }
                        grew = true;
                    }
                    ArcInstr::Let {
                        dst,
                        value: ArcValue::Var(src),
                        ..
                    } if views.contains(*src) && !is_member(*dst) && !views.contains(*dst) => {
                        if views.same_alloc.contains(src) {
                            views.same_alloc.insert(*dst);
                        } else {
                            views.opaque.insert(*dst);
                        }
                        grew = true;
                    }
                    _ => {}
                }
            }
        }
        if !grew {
            return views;
        }
    }
}

/// Whether a whole-var RC op on a non-scalar projection of `var` provably
/// changes the SAME allocation count as a whole-var op on `var` itself
/// ([`same_alloc_wrapper_type`]), gated by the view-ledger admission toggle.
fn is_same_alloc_wrapper(func: &ArcFunction, var: ArcVarId, pool: &Pool) -> bool {
    if *VIEW_LEDGER_ADMISSION_DISABLED {
        return false;
    }
    same_alloc_wrapper_type(func, var, pool)
}

/// The toggle-free same-allocation wrapper proof: `var`'s type is a
/// niche-family single-payload wrapper — builtin `Option` / `Result` (at
/// most ONE payload field per variant by construction), inline `Aggregate`
/// repr (no self allocation), non-recursive. User enums and multi-RC-field
/// aggregates are NOT admitted (field-grain attribution is unprovable at
/// this layer); collection element extraction never qualifies (a `List` is
/// not a wrapper). Consumed by the view-chain classification (through the
/// toggled [`is_same_alloc_wrapper`]) AND by the wrapped transfer-anchor
/// admission + extract-union (toggle-free — those carry their own
/// `ORI_DISABLE_WRAPPED_CREDIT_ANCHOR` gate). Spec: Annex E §AIMS TF-4 +
/// RL-2.
pub(super) fn same_alloc_wrapper_type(func: &ArcFunction, var: ArcVarId, pool: &Pool) -> bool {
    if !matches!(func.var_repr(var), Some(ValueRepr::Aggregate)) {
        return false;
    }
    if var.index() >= func.var_types.len() {
        return false;
    }
    let ty = pool.resolve_fully(func.var_type(var));
    matches!(pool.tag(ty), Tag::Option | Tag::Result) && !pool.aggregate_type_is_recursive(ty)
}

/// Whether every OPAQUE view RC op forms a per-block, per-var, inc-first
/// balanced pair (net 0 inside one block). A lone half (the niche-payload
/// release on a `Some` arm) or a field-grain dec fails — the allocation's
/// accounting extends beyond the member model.
pub(super) fn view_ops_balanced(func: &ArcFunction, opaque: &FxHashSet<ArcVarId>) -> bool {
    for block in &func.blocks {
        let mut running: FxHashMap<ArcVarId, i64> = FxHashMap::default();
        for instr in &block.body {
            match instr {
                ArcInstr::BurdenInc { var } | ArcInstr::RcInc { var, .. } => {
                    if opaque.contains(var) {
                        *running.entry(*var).or_insert(0) += 1;
                    }
                }
                ArcInstr::BurdenDec { var }
                | ArcInstr::BurdenDecPartial { var, .. }
                | ArcInstr::BurdenDecVariant { var }
                | ArcInstr::RcDec { var, .. }
                | ArcInstr::RcDecPartial { var, .. }
                | ArcInstr::RcDecVariant { var } => {
                    if opaque.contains(var) {
                        let r = running.entry(*var).or_insert(0);
                        if *r < 1 {
                            return false;
                        }
                        *r -= 1;
                    }
                }
                ArcInstr::RcDecField { base, .. } | ArcInstr::BurdenDecField { base, .. } => {
                    if opaque.contains(base) {
                        return false;
                    }
                }
                _ => {}
            }
        }
        if running.values().any(|&r| r != 0) {
            return false;
        }
    }
    true
}
