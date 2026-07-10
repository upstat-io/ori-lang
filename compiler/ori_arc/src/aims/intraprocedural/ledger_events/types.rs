//! Ledger-event type vocabulary consumed by [`super::classify_function`].
//!
//! Mirrors the proven ledger calculus's data shapes: the RL-2 terminal-use
//! grid, per-class origin attribution, the classified event stream, and the
//! boundary-contract projection consumed at call sites.

use rustc_hash::FxHashMap;

use crate::aims::contract::MemoryContract;
use crate::ir::ArcVarId;

use super::super::birth_site_partition::NodeIdx;

/// RL-2 terminal-use kinds — the committed 12-row coverage grid.
///
/// MUST match `AimsProof.Realization::TerminalUse` member-for-member; the
/// transfer split is [`TerminalUse::transfers_ownership`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "test-pinned mirror of the compiled AimsProof.Realization::TerminalUse grid (verified by terminal_use_table_matches_committed_rl2_grid); the class-ledger emitter derives its own deltas in class_ledger::events, never through this table"
    )
)]
pub(crate) enum TerminalUse {
    Return,
    ConstructArg,
    ReuseArg,
    CollectionReuseArg,
    SetValue,
    PartialApplyCapture,
    ApplyToOwnedParam,
    JumpArg,
    ApplyToIterConsumingParam,
    LastReadBeforeScopeExit,
    ScopeExit,
    ApplyToBorrowedParam,
}

impl TerminalUse {
    /// The 12-kind transfer partition: 9 transfer kinds hand the reference
    /// off (CONSUME); 3 non-transfer kinds are the terminal READ the placed
    /// dec must follow. Mirrors `rl2_use_transfers_ownership` exactly.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "test-pinned mirror of the compiled AimsProof.Realization::TerminalUse grid (verified by terminal_use_table_matches_committed_rl2_grid); the class-ledger emitter derives its own deltas in class_ledger::events, never through this table"
        )
    )]
    pub(crate) fn transfers_ownership(self) -> bool {
        !matches!(
            self,
            Self::LastReadBeforeScopeExit | Self::ScopeExit | Self::ApplyToBorrowedParam
        )
    }

    /// All 12 kinds, for exhaustiveness tests over the grid.
    #[cfg(test)]
    pub(crate) const ALL: [Self; 12] = [
        Self::Return,
        Self::ConstructArg,
        Self::ReuseArg,
        Self::CollectionReuseArg,
        Self::SetValue,
        Self::PartialApplyCapture,
        Self::ApplyToOwnedParam,
        Self::JumpArg,
        Self::ApplyToIterConsumingParam,
        Self::LastReadBeforeScopeExit,
        Self::ScopeExit,
        Self::ApplyToBorrowedParam,
    ];
}

/// Class-origin attribution — WHERE a partition class's tracked reference
/// enters the function.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ClassOrigin {
    /// A local allocation site (`Construct` / `Reuse` / `CollectionReuse` /
    /// `PartialApply`).
    Fresh,
    /// A callee-produced owned allocation (owned function param, or a call
    /// result whose contract proves an owned fresh return).
    Foreign,
    /// A borrowed function param — the caller retains ownership; the emitter
    /// treats borrowed-rooted classes as always-inc-at-consume.
    Borrowed,
    /// A call result with no contract knowledge — conservative unknown.
    Opaque,
    /// A refused phi/Select merge — the class starts at the merge point and
    /// is funded per-edge by cross-class jump-arg credits, never by a birth
    /// event (one predecessor edge executes per walk).
    Merge,
}

/// A class-resolved instruction event — the Rust mirror of the calculus's
/// `LedgerInstr` with variables resolved to partition-class representatives.
///
/// `Read` / `Mutate` carry the member variable so walk-level derivation can
/// compute the dynamic-COW live-sibling floor from the path suffix
/// (`sibReadCount`), exactly as the calculus computes it at derivation time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClassInstr {
    /// The class's tracked reference enters: +1 (except `Merge`, which is
    /// funded per-edge and never emits a birth).
    Birth { class: NodeIdx, origin: ClassOrigin },
    /// RL-1 duplication inc / RL-34 return-leg / sharing-view producer /
    /// cross-class jump-arg funding: +1.
    Credit { class: NodeIdx },
    /// A `Select` acquisition: the dst conditionally holds ONE operand's
    /// allocation; the planner REALIZES the acquired reference with an
    /// RL-1 duplication inc placed after the select (the event itself is
    /// delta-0 — an unplanned class stays honestly unfunded).
    SelectCredit { class: NodeIdx, var: ArcVarId },
    /// Ownership hand-off out (a transfer terminal use) or a placed
    /// release: -1.
    Consume { class: NodeIdx },
    /// A borrow-view or terminal read: running count >= 1.
    Read { class: NodeIdx, value: ArcVarId },
    /// A COW-mutating use: running count >= 1 + live same-class siblings.
    Mutate { class: NodeIdx, value: ArcVarId },
}

/// Source position of one classified event inside its block — the
/// placement anchor the class-ledger emitter plans insertions against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EventSite {
    /// Block-entry events, before any body instruction: function-param
    /// births in the entry block; `Invoke`/`InvokeIndirect` result events
    /// materialized at a NORMAL successor's entry (the result never exists
    /// on the unwind edge — PV-4: the boundary credit lands where the
    /// return lands).
    BlockEntry,
    /// The body instruction at this index.
    Body(usize),
    /// The block terminator.
    Terminator,
}

/// A derived per-class ledger event — mirror of the calculus `LedgerEvent`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LedgerEvent {
    Birth,
    Credit,
    Consume,
    Read,
    Mutate { live_siblings: usize },
}

/// PV-4 boundary-contract projection for one callee — the contract facts
/// call-site classification consumes (`BoundaryContract.ofParamContract`).
#[derive(Clone, Debug, Default)]
pub(crate) struct BoundaryFacts {
    /// Per-param: the callee iter-consumes this argument (RL-2 inward
    /// transfer despite a Borrowed annotation).
    pub(crate) param_iter_consumes: Vec<bool>,
    /// Per-param: the callee transfers this argument through its return
    /// (RL-34 passthrough — consume at call, credit at return, net 0).
    pub(crate) param_transfers_through_return: Vec<bool>,
    /// Per-param: the contract cardinality is `Absent` (zero live-path
    /// demand). VF-2 requires the function body carry NO reference to such
    /// a param — the caller retains the release obligation.
    pub(crate) param_cardinality_absent: Vec<bool>,
    /// The callee's return is a sharing view of an argument's allocation
    /// (seamless slice family): the result carries a CREDIT.
    pub(crate) returns_sharing_view: bool,
    /// The callee's return is a fresh owned allocation the caller now owns.
    pub(crate) returns_owned_fresh: bool,
}

impl BoundaryFacts {
    /// Project the classification-relevant facts out of a full
    /// `MemoryContract` (the production adapter; PV-4's
    /// `BoundaryContract.ofParamContract` composed per param).
    pub(crate) fn from_contract(contract: &MemoryContract) -> Self {
        Self {
            param_iter_consumes: contract.params.iter().map(|p| p.iter_consumes).collect(),
            param_transfers_through_return: contract
                .params
                .iter()
                .map(|p| p.transfers_through_return)
                .collect(),
            param_cardinality_absent: contract
                .params
                .iter()
                .map(|p| p.cardinality == crate::aims::lattice::Cardinality::Absent)
                .collect(),
            returns_sharing_view: contract.return_info.returns_sharing_view,
            returns_owned_fresh: contract.return_info.preserves_freshness,
        }
    }

    /// Whether argument `position` is an RL-2 iter-consume inward transfer:
    /// the callee iter-consumes it AND does not pass it back through the
    /// return.
    pub(super) fn iter_consume_transfer(&self, position: usize) -> bool {
        self.param_iter_consumes
            .get(position)
            .copied()
            .unwrap_or(false)
            && !self
                .param_transfers_through_return
                .get(position)
                .copied()
                .unwrap_or(false)
    }
}

/// Classifier output: per-block class-instruction streams (block order mirrors
/// `func.blocks`) plus the per-class origin attribution map.
#[derive(Debug, Default)]
pub(crate) struct LedgerClassification {
    /// One ordered stream per block, indexed by block position.
    pub(crate) blocks: Vec<Vec<ClassInstr>>,
    /// Per-event source site, parallel to `blocks` (`sites[b][k]` locates
    /// `blocks[b][k]` within block `b`).
    pub(crate) sites: Vec<Vec<EventSite>>,
    /// Class representative -> origin kind.
    pub(crate) class_origins: FxHashMap<NodeIdx, ClassOrigin>,
    /// A non-excluded HEAP arg was handed through an indirect call
    /// (`ApplyIndirect` / `InvokeIndirect` arg position, receiver excluded).
    /// Call-site `arg_ownership` is populated during realization — AFTER
    /// this classification runs — and the callee is unresolved, so the
    /// consumed-vs-borrowed hand-off is UNMODELED: the readiness gate falls
    /// back (READ double-frees a consuming callee; CONSUME leaks a
    /// borrowing one).
    pub(crate) indirect_arg_handoff: bool,
    /// Every variable is excluded under the CLASSIFIER's own semantics
    /// (state-map scalar/immortal OR placeholder alias-closure): a
    /// zero-class function with this set carries no RC-bearing value and
    /// the empty plan is the correct emission.
    pub(crate) all_vars_excluded: bool,
}
