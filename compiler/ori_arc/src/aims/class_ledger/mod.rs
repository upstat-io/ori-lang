//! Class-ledger owed-invariant emitter over the birth-site partition.
//!
//! Alternate Phase-5 emission path: consumes the class-ledger classifier's
//! per-block event streams (`ledger_events`), plans per-class `BurdenInc` /
//! `BurdenDec` insertions under the owed-count invariant (the references a
//! class holds that must be released), and verifies every class per path
//! before the plan is trusted. Placement per the compositional-placement
//! calculus (`AimsProof.Ledger`): the owed count agrees on every edge into
//! every merge block; a release is never hoisted past a merge point.
//!
//! Staging: with `ORI_CLASS_LEDGER_EMITTER=1` the pipeline runs the analysis
//! (classification + insertion plan + per-class readiness verdict) and
//! reports it on the `ori_arc::aims::class_ledger` tracing target. The
//! existing burden path still performs ALL emission, so compiled output is
//! byte-identical with the toggle on or off. Op materialization into the
//! instruction stream is exercised in unit tests through the plan-application
//! helper (`apply`); pipeline cutover is deferred to the differential
//! harness. A class whose per-class net dataflow cannot be proven
//! (non-converged, merge-disagreeing, or an inexpressible release) is
//! DECLINED — no ops are planned for it and the readiness summary reports it
//! (fail-closed, never a wrong placement).

mod apply;
mod emit;
mod events;
mod verify;

#[cfg(test)]
mod tests;

pub(crate) use emit::{ClassOutcome, PlannedOp};
pub(crate) use verify::{ClassVerdict, ReadinessSummary};

#[cfg(test)]
pub(crate) use emit::{DeclineReason, PlanSlot, PlannedOpKind};

use std::sync::LazyLock;

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::aims::contract::MemoryContract;
use crate::aims::intraprocedural::birth_site_partition::{BirthSitePartition, NodeIdx};
use crate::aims::intraprocedural::birth_site_population::compute_birth_site_partition;
use crate::aims::intraprocedural::ledger_events::{
    classify_function, BoundaryFacts, LedgerClassification,
};
use crate::aims::intraprocedural::AimsStateMap;
use crate::graph::compute_predecessors;
use crate::ir::ArcFunction;

// Env: ORI_CLASS_LEDGER_EMITTER — enables the class-ledger alternate Phase-5
// emitter analysis (insertion plan + per-class verification + readiness
// report; no emission cutover), experimental.
static CLASS_LEDGER_EMITTER: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_CLASS_LEDGER_EMITTER").as_deref() == Ok("1"));

/// Whether the class-ledger emitter analysis is enabled for this process.
pub(crate) fn class_ledger_emitter_enabled() -> bool {
    *CLASS_LEDGER_EMITTER
}

/// One class's planning outcome, keyed by its partition representative.
#[derive(Debug)]
pub(crate) struct ClassPlan {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "consumed by the class-ledger emitter cutover; test-pinned until the differential harness lands"
        )
    )]
    pub(crate) class: NodeIdx,
    pub(crate) outcome: ClassOutcome,
}

/// The whole-function insertion plan, one entry per partition class.
#[derive(Debug, Default)]
pub(crate) struct ClassLedgerPlan {
    pub(crate) classes: Vec<ClassPlan>,
}

/// Insertion plan plus the per-class readiness verdicts for one function.
#[derive(Debug)]
pub(crate) struct ClassLedgerAnalysis {
    pub(crate) plan: ClassLedgerPlan,
    pub(crate) readiness: ReadinessSummary,
}

/// Toggle-gated pipeline entry: a no-op (`None`) while the toggle is off.
pub(crate) fn pipeline_analysis(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> Option<ClassLedgerAnalysis> {
    if !class_ledger_emitter_enabled() {
        return None;
    }
    Some(analyze_from_state_map(func, state_map, contracts))
}

/// Run classification, planning, and per-class verification from the
/// converged state map — the enabled path of [`pipeline_analysis`].
pub(crate) fn analyze_from_state_map(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> ClassLedgerAnalysis {
    let boundary_facts: FxHashMap<Name, BoundaryFacts> = contracts
        .iter()
        .map(|(name, contract)| (*name, BoundaryFacts::from_contract(contract)))
        .collect();
    let mut partition = compute_birth_site_partition(func, state_map);
    let classification = classify_function(func, state_map, &mut partition, &boundary_facts);
    analyze_class_ledger(func, &classification, &mut partition)
}

/// Plan and verify every partition class named by `classification`.
///
/// A declined class contributes no ops; it is verified against the bare
/// event stream (its verdict stays honest) and reported in the readiness
/// summary. The plan is trusted only when NO class declined and every class
/// verifies `Clean`.
pub(crate) fn analyze_class_ledger(
    func: &ArcFunction,
    classification: &LedgerClassification,
    partition: &mut BirthSitePartition,
) -> ClassLedgerAnalysis {
    let preds = compute_predecessors(func);
    let mut classes = Vec::new();
    let mut verdicts = Vec::new();
    let mut declined = Vec::new();
    for class in events::collect_classes(classification) {
        let class_events = events::extract_class_events(func, classification, partition, class);
        let outcome = emit::plan_class(func, &preds, &class_events);
        let planned_ops: &[PlannedOp] = match &outcome {
            ClassOutcome::Planned(ops) => ops,
            ClassOutcome::Declined(reason) => {
                declined.push((class, *reason));
                &[]
            }
        };
        let verdict = verify::verify_class(func, &preds, &class_events, planned_ops);
        verdicts.push((class, verdict));
        classes.push(ClassPlan { class, outcome });
    }
    let all_classes_clean = declined.is_empty()
        && verdicts
            .iter()
            .all(|&(_, verdict)| verdict == ClassVerdict::Clean);
    ClassLedgerAnalysis {
        plan: ClassLedgerPlan { classes },
        readiness: ReadinessSummary {
            all_classes_clean,
            verdicts,
            declined,
        },
    }
}

/// Pipeline step: run the gated analysis and report the readiness verdict
/// on the `ori_arc::aims::class_ledger` tracing target. Mutates nothing —
/// the burden path remains the sole emitter until cutover.
pub(crate) fn report_pipeline_readiness(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
) {
    let Some(analysis) = pipeline_analysis(func, state_map, contracts) else {
        return;
    };
    let planned_ops: usize = analysis
        .plan
        .classes
        .iter()
        .map(|plan| match &plan.outcome {
            ClassOutcome::Planned(ops) => ops.len(),
            ClassOutcome::Declined(_) => 0,
        })
        .sum();
    let verdict_count = |wanted: ClassVerdict| {
        analysis
            .readiness
            .verdicts
            .iter()
            .filter(|&&(_, verdict)| verdict == wanted)
            .count()
    };
    tracing::debug!(
        target: "ori_arc::aims::class_ledger",
        function = interner.lookup(func.name),
        classes = analysis.plan.classes.len(),
        planned_ops,
        clean = verdict_count(ClassVerdict::Clean),
        leak_only = verdict_count(ClassVerdict::LeakOnly),
        unprovable = verdict_count(ClassVerdict::Unprovable),
        declined = analysis.readiness.declined.len(),
        all_classes_clean = analysis.readiness.all_classes_clean,
        "class-ledger readiness"
    );
}
