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
//! (classification + insertion plan + per-class readiness verdict) and, per
//! function, REPLACES the legacy Step-4b emission with the applied plan when
//! the replacement gate holds (`replace` module: FULLY CLEAN readiness with
//! one class or more, no user-`@drop` type in the function, dominance-checked
//! op placement, and a VF-1 structural check on a clone — commit-or-discard).
//! Any function failing a gate falls back to the legacy walk unchanged; the
//! per-function mode + readiness verdict are reported on the
//! `ori_arc::aims::class_ledger` tracing target. Toggle off, the pipeline is
//! byte-identical to the legacy path. Corpus-level default-on cutover is
//! deferred to the differential harness. A class whose per-class net
//! dataflow cannot be proven (non-converged, merge-disagreeing, or an
//! inexpressible release) is DECLINED — no ops are planned for it and the
//! readiness summary reports it (fail-closed, never a wrong placement).

mod apply;
mod emit;
mod events;
mod replace;
mod verify;

pub(crate) use emit::{ClassOutcome, PlannedOp};
pub(crate) use replace::{attempt_replacement, EmissionMode, ReplacementOutcome};
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
// emitter (insertion plan + per-class verification + readiness report +
// per-function replacement of the legacy Step-4b emission behind the
// readiness gate; non-clean functions fall back to the legacy walk),
// experimental.
static CLASS_LEDGER_EMITTER: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_CLASS_LEDGER_EMITTER").as_deref() == Ok("1"));

/// Whether the class-ledger emitter analysis is enabled for this process.
pub(crate) fn class_ledger_emitter_enabled() -> bool {
    *CLASS_LEDGER_EMITTER
}

/// One class's planning outcome, keyed by its partition representative.
#[derive(Debug)]
pub(crate) struct ClassPlan {
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
    /// A locally-released container class (a planned dec or a consume event)
    /// has a SIBLING field-path view class with events of its own: the
    /// container's recursive release and the view's cross-class liveness are
    /// not modeled together — the replacement gate declines.
    pub(crate) field_view_hazard: bool,
}

/// Run classification, planning, and per-class verification from the
/// converged state map — the analysis entry [`attempt_replacement`] and the
/// tests share.
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
    let regions = emit::CycleRegions::compute(func);
    let mut classes = Vec::new();
    let mut verdicts = Vec::new();
    let mut declined = Vec::new();
    let mut class_facts: Vec<(NodeIdx, bool, bool)> = Vec::new();
    for class in events::collect_classes(classification) {
        let class_events = events::extract_class_events(func, classification, partition, class);
        let outcome = emit::plan_class(func, &preds, &regions, &class_events, &[]);
        let planned_ops: &[PlannedOp] = match &outcome {
            ClassOutcome::Planned(ops) => ops,
            ClassOutcome::Declined(reason) => {
                declined.push((class, *reason));
                &[]
            }
        };
        let verdict = verify::verify_class(func, &preds, &class_events, planned_ops);
        let decline = declined.iter().find(|&&(c, _)| c == class).map(|&(_, r)| r);
        if verdict != ClassVerdict::Clean || decline.is_some() {
            tracing::debug!(
                target: "ori_arc::aims::class_ledger",
                class = ?partition.node_key(class),
                verdict = ?verdict,
                declined = ?decline,
                origin = ?class_events.origin,
                container_held = class_events.container_held,
                threads_back_edge = class_events.threads_back_edge,
                events = ?class_events.per_block,
                planned = ?planned_ops,
                "class not clean (run ORI_LOG=ori_arc::aims::class_ledger=trace for the failing gate)"
            );
        }
        verdicts.push((class, verdict));
        // Locally released = a PLANNED whole-var dec (an actual local free
        // whose recursion reaches the fields). A transfer-out consume
        // (Return / store) hands the container to a new owner and frees
        // nothing here.
        let released = matches!(&outcome, ClassOutcome::Planned(ops)
                if ops.iter().any(|op| op.kind == emit::PlannedOpKind::Dec));
        let eventful = class_events.per_block.iter().any(|evs| !evs.is_empty());
        class_facts.push((class, released, eventful));
        classes.push(ClassPlan { class, outcome });
    }
    let hazard_views = field_view_hazard_classes(partition, &class_facts);
    let mut uncured = Vec::new();
    for view in hazard_views {
        if cure_view_with_extraction_funding(
            func,
            classification,
            partition,
            &preds,
            &regions,
            view,
            &mut classes,
            &mut verdicts,
            &mut declined,
        ) {
            continue;
        }
        uncured.push(view);
    }
    let field_view_hazard = !uncured.is_empty();
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
        field_view_hazard,
    }
}

/// The field-path VIEW classes endangered by a locally-released container:
/// the container's recursive release would free the view's allocation while
/// the view still uses it. Deduplicated, deterministic order.
fn field_view_hazard_classes(
    partition: &mut BirthSitePartition,
    class_facts: &[(NodeIdx, bool, bool)],
) -> Vec<NodeIdx> {
    let released: Vec<NodeIdx> = class_facts
        .iter()
        .filter(|&&(_, released, _)| released)
        .map(|&(class, _, _)| class)
        .collect();
    if released.is_empty() {
        return Vec::new();
    }
    let nodes = partition.nodes_snapshot();
    let mut views: Vec<NodeIdx> = Vec::new();
    for &container in &released {
        let container_rep = partition.rep_of(container);
        // Member vars of the container class (whole-var nodes).
        let member_vars: Vec<_> = nodes
            .iter()
            .filter(|(_, path, _)| path.is_whole_var())
            .filter(|&&(_, _, node)| partition.rep_of(node) == container_rep)
            .map(|&(var, _, _)| var)
            .collect();
        for (var, path, node) in &nodes {
            if path.is_whole_var() || !member_vars.contains(var) {
                continue;
            }
            let view_rep = partition.rep_of(*node);
            if view_rep == container_rep {
                continue;
            }
            let view_eventful = class_facts
                .iter()
                .any(|&(class, _, eventful)| eventful && partition.rep_of(class) == view_rep);
            if view_eventful && !views.contains(&view_rep) {
                views.push(view_rep);
            }
        }
    }
    views.sort_unstable();
    views
}

/// Cure one endangered view class by funding it at its extraction sites: an
/// RL-1 dup `BurdenInc` right after each `Project` that defines a member
/// var, with the class re-planned and re-verified under OWNED semantics
/// (the container's recursive release and the view's independent reference
/// each balance). Returns `true` when the cured plan verifies `Clean`;
/// `false` leaves the original outcome in place (the replacement gate then
/// declines the function).
#[expect(
    clippy::too_many_arguments,
    reason = "internal cure pass over analyze_class_ledger's own accumulators"
)]
fn cure_view_with_extraction_funding(
    func: &ArcFunction,
    classification: &LedgerClassification,
    partition: &mut BirthSitePartition,
    preds: &[Vec<usize>],
    regions: &emit::CycleRegions,
    view: NodeIdx,
    classes: &mut [ClassPlan],
    verdicts: &mut [(NodeIdx, ClassVerdict)],
    declined: &mut Vec<(NodeIdx, emit::DeclineReason)>,
) -> bool {
    use crate::aims::intraprocedural::birth_site_partition::FieldPath;
    use crate::ir::ArcInstr;

    let mut seeds = Vec::new();
    for (block_idx, arc_block) in func.blocks.iter().enumerate() {
        for (index, instr) in arc_block.body.iter().enumerate() {
            let ArcInstr::Project { dst, .. } = instr else {
                continue;
            };
            let node = partition.register_node(*dst, FieldPath::whole_var());
            if partition.rep_of(node) != view {
                continue;
            }
            seeds.push(emit::PlannedOp {
                slot: emit::PlanSlot::AfterBody {
                    block: block_idx,
                    index,
                },
                kind: emit::PlannedOpKind::Inc,
                var: *dst,
            });
        }
    }
    if seeds.is_empty() {
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            view = ?partition.node_key(view),
            "view cure declined: no member-defining Project seeds"
        );
        return false;
    }
    let funded_events =
        events::extract_class_events_with(func, classification, partition, view, true);
    let outcome = emit::plan_class(func, preds, regions, &funded_events, &seeds);
    let planned: &[PlannedOp] = match &outcome {
        ClassOutcome::Planned(ops) => ops,
        ClassOutcome::Declined(reason) => {
            tracing::trace!(
                target: "ori_arc::aims::class_ledger",
                view = ?partition.node_key(view),
                declined = ?reason,
                seeds = seeds.len(),
                events = ?funded_events.per_block,
                "view cure declined: funded plan declined"
            );
            return false;
        }
    };
    let verdict = verify::verify_class(func, preds, &funded_events, planned);
    if verdict != ClassVerdict::Clean {
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            view = ?partition.node_key(view),
            verdict = ?verdict,
            planned = ?planned,
            events = ?funded_events.per_block,
            "view cure declined: funded plan verifies non-Clean"
        );
        return false;
    }
    let Some(entry) = classes.iter_mut().find(|plan| plan.class == view) else {
        return false;
    };
    entry.outcome = outcome;
    if let Some(slot) = verdicts.iter_mut().find(|(class, _)| *class == view) {
        slot.1 = ClassVerdict::Clean;
    }
    declined.retain(|&(class, _)| class != view);
    true
}

/// Pipeline Step-4b dispatch: attempt the per-function replacement, report
/// the readiness verdict + emission mode on the `ori_arc::aims::class_ledger`
/// tracing target, and return whether the plan replaced the legacy emission.
///
/// `class_ledger_enabled` carries the toggle read at the pipeline's outer
/// entry (`class_ledger_emitter_enabled`); off = no analysis, no report,
/// `false`. `legacy_emission_enabled = false` (Step-4b emission disabled)
/// keeps the analysis-only readiness report and never replaces.
pub(crate) fn apply_class_ledger_replacement(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    contracts: &FxHashMap<Name, MemoryContract>,
    type_registry: &ori_types::TypeRegistry,
    interner: &ori_ir::StringInterner,
    class_ledger_enabled: bool,
    legacy_emission_enabled: bool,
) -> bool {
    if !class_ledger_enabled {
        return false;
    }
    let outcome = attempt_replacement(
        func,
        state_map,
        contracts,
        type_registry,
        legacy_emission_enabled,
    );
    report_readiness(func, interner, &outcome);
    outcome.mode == EmissionMode::Replaced
}

/// Report one function's readiness verdict + Step-4b emission mode on the
/// `ori_arc::aims::class_ledger` tracing target.
fn report_readiness(
    func: &ArcFunction,
    interner: &ori_ir::StringInterner,
    outcome: &ReplacementOutcome,
) {
    let analysis = &outcome.analysis;
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
        mode = outcome.mode.as_str(),
        fallback_reason = outcome.fallback_reason.unwrap_or(""),
        "class-ledger readiness"
    );
}

#[cfg(test)]
mod tests;
