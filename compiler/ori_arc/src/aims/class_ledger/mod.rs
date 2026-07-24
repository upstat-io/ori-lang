//! Logical owner-credit planning over the birth-site partition.
//!
//! The planner consumes per-block ledger events, places `BurdenInc` and
//! `BurdenDec` operations, and verifies each class before committing the plan.

macro_rules! class_ledger_trace {
    ($($fields:tt)*) => {
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            $($fields)*
        )
    };
}

macro_rules! class_ledger_debug {
    ($($fields:tt)*) => {
        tracing::debug!(
            target: "ori_arc::aims::class_ledger",
            $($fields)*
        )
    };
}

mod analysis;
mod apply;
mod copy_out;
mod emit;
mod events;
mod hazard;
mod placement;
mod replace;
mod verify;

#[cfg(test)]
pub(crate) use analysis::{analyze_from_state_map, apply_class_ledger_replacement};
pub(crate) use analysis::{
    apply_class_ledger_replacement_with_exact, ClassLedgerAnalysis, ClassPlan,
};

#[cfg(test)]
use analysis::analyze_class_ledger;
#[cfg(test)]
use emit::ClassOutcome;
#[cfg(test)]
use replace::{attempt_replacement, EmissionMode, FallbackReason};
#[cfg(test)]
use verify::ClassVerdict;

#[cfg(test)]
pub(crate) use emit::{DeclineReason, PlanSlot, PlannedOp, PlannedOpKind};

#[cfg(test)]
mod tests;
