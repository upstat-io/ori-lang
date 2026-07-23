//! Whole-function logical owner-credit analysis and replacement dispatch.
//!
//! The planner consumes per-block ledger events, places `BurdenInc` and
//! `BurdenDec` operations, and verifies each class before committing the plan.
//! Its `owed` value is a logical credit multiplicity, not a runtime counter.
//! Every merge predecessor must agree on owed credit, and no release may move
//! past a merge.
//!
//! Replacement requires every readiness gate; Phase 7 materializes the
//! resulting operations in the transitional `Rc*` carrier. Production gate
//! failures are fail-loud unless the burden-op ablation is active.
//! Diagnostics use the shared `ori_arc::aims::class_ledger` target.

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::MemoryContract;
use crate::aims::intraprocedural::birth_site_partition::{BirthSitePartition, NodeIdx};
use crate::aims::intraprocedural::ledger_events::EventSite;
use crate::aims::intraprocedural::ledger_events::{BoundaryFacts, LedgerClassification};
use crate::aims::intraprocedural::AimsStateMap;
use crate::graph::compute_predecessors;
use crate::ir::ArcFunction;

use super::emit::ClassOutcome;
use super::replace::{attempt_replacement, EmissionMode, FallbackReason, ReplacementOutcome};
use super::verify::{ClassVerdict, ReadinessSummary};
use super::{copy_out, emit, events, hazard, replace, verify};

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
    /// A heap arg handed through an indirect call (unmodeled ownership;
    /// per the classification flag) — the replacement gate declines.
    pub(crate) indirect_arg_handoff: bool,
    /// Every variable excluded under the classifier's own semantics (per
    /// the classification flag) — the zero-class empty plan is admitted.
    pub(crate) all_vars_excluded: bool,
    /// Vars whose whole-var node's class carries a `Consume` event (per the
    /// classification): the reference transfers to an owner whose release
    /// chain runs the value's drop glue recursively.
    pub(crate) consume_covered: rustc_hash::FxHashSet<crate::ir::ArcVarId>,
    /// COPY-OUT covered user-drop vars (RL-DROP §8.1.1): the value is
    /// runtime-copied into a map/set at a borrowed `insert` arg; the class's
    /// placed releases were rewritten fields-only (`DecPartial` empty skip)
    /// and the stored copy carries the single `@drop` at teardown.
    pub(crate) copy_out_covered: rustc_hash::FxHashSet<crate::ir::ArcVarId>,
}

/// Run classification, planning, and per-class verification from the
/// converged state map — the analysis entry [`attempt_replacement`] and the
/// tests share.
pub(crate) fn analyze_from_state_map(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    contracts: &FxHashMap<Name, MemoryContract>,
    type_registry: &ori_types::TypeRegistry,
    interner: &ori_ir::StringInterner,
) -> ClassLedgerAnalysis {
    let boundary_facts: FxHashMap<Name, BoundaryFacts> = contracts
        .iter()
        .map(|(name, contract)| (*name, BoundaryFacts::from_contract(contract)))
        .collect();
    // Scalar-excluded vars whose type carries a user `@drop` participate:
    // they hold a drop OBLIGATION (RL-DROP, balance-neutral) the plan must
    // discharge with one whole-var release at the death point.
    let user_drop_admitted: FxHashSet<crate::ir::ArcVarId> = (0..func.var_types.len())
        .map(|i| {
            crate::ir::ArcVarId::new(
                u32::try_from(i).unwrap_or_else(|_| panic!("var index {i} fits in u32")),
            )
        })
        .filter(|&var| {
            // Scalars only — an IMMORTAL user-drop value never drops, so it
            // stays excluded (and the gate's residual decline covers it).
            state_map.is_scalar(var)
                && !state_map.is_immortal(var)
                && crate::lower::burden_lookup::type_has_user_drop(
                    func.var_types[var.index()],
                    type_registry,
                )
        })
        .collect();
    let mut partition =
        crate::aims::intraprocedural::birth_site_population::compute_birth_site_partition_with_admitted(
            func, state_map, &user_drop_admitted,
        );
    let classification =
        crate::aims::intraprocedural::ledger_events::classify_function_with_admitted(
            func,
            state_map,
            &mut partition,
            &boundary_facts,
            interner,
            user_drop_admitted,
        );
    let mut analysis = analyze_class_ledger(
        func,
        &classification,
        &mut partition,
        contracts,
        type_registry,
        interner,
    );
    copy_out::apply_copy_out_rewrite(func, type_registry, interner, &mut partition, &mut analysis);
    analysis
}

/// Hazard facts for one planned class: a view is ENDANGERED by independent
/// DEMAND (`Read` / `Mutate` / `SelectCredit`) OR by a Consume that is NOT the
/// move-in store at the released container's own Construct site — an
/// extract-then-move-out member is freed by the original container's
/// recursive release AND owned by its new container (the aliased-subtree
/// corruption). Only the move-in store (a Consume at the container's
/// Construct) is the lifecycle the container's release legitimately pays
/// for. Locally released = a PLANNED whole-var dec; a transfer-out consume
/// hands the container to a new owner and frees nothing here.
fn hazard_facts_for(
    class: NodeIdx,
    class_events: &events::ClassEvents,
    outcome: &ClassOutcome,
    verdict: ClassVerdict,
) -> hazard::ClassHazardFacts {
    let released = matches!(outcome, ClassOutcome::Planned(ops)
            if ops.iter().any(|op| op.kind == emit::PlannedOpKind::Dec));
    let has_demand = class_events.per_block.iter().flatten().any(|ev| {
        matches!(
            ev.kind,
            events::EventKind::Read | events::EventKind::Mutate | events::EventKind::SelectCredit
        )
    });
    let mut consume_events = Vec::new();
    for (block, evs) in class_events.per_block.iter().enumerate() {
        for ev in evs {
            if ev.kind == events::EventKind::Consume {
                consume_events.push((block, ev.site, ev.var));
            }
        }
    }
    let self_funded_clean = !class_events.is_externally_funded() && verdict == ClassVerdict::Clean;
    let borrowed_rooted_clean = class_events.origin
        == Some(crate::aims::intraprocedural::ledger_events::ClassOrigin::Borrowed)
        && class_events.is_externally_funded()
        && verdict == ClassVerdict::Clean;
    let has_credit = class_events.per_block.iter().flatten().any(|ev| {
        matches!(
            ev.kind,
            events::EventKind::Credit | events::EventKind::SelectCredit
        )
    });
    let planned_inc_count = match outcome {
        ClassOutcome::Planned(ops) => ops
            .iter()
            .filter(|op| op.kind == emit::PlannedOpKind::Inc)
            .count(),
        ClassOutcome::Declined(_) => 0,
    };
    hazard::ClassHazardFacts::new(
        class,
        hazard::ClassHazardFlags::EMPTY
            .with_released(released)
            .with_demand(has_demand)
            .with_self_funded_clean(self_funded_clean)
            .with_credit(has_credit)
            .with_borrowed_rooted_clean(borrowed_rooted_clean)
            .with_verified_clean(verdict == ClassVerdict::Clean),
        planned_inc_count,
        consume_events,
    )
}

/// Plan and verify every partition class named by `classification`.
///
/// A declined class contributes no ops; it is verified against the bare
/// event stream (its verdict stays honest) and reported in the readiness
/// summary. The plan is trusted only when NO class declined and every class
/// verifies `Clean`.
struct InitialClassPlans {
    classes: Vec<ClassPlan>,
    verdicts: Vec<(NodeIdx, ClassVerdict)>,
    declined: Vec<(NodeIdx, emit::DeclineReason)>,
    facts: Vec<hazard::ClassHazardFacts>,
}

struct ClassPlanningInput<'a> {
    func: &'a ArcFunction,
    classification: &'a LedgerClassification,
    partition: &'a mut BirthSitePartition,
    preds: &'a [Vec<usize>],
    regions: &'a emit::CycleRegions,
    full_move_arms: &'a [events::FullMoveArm],
}

fn plan_initial_classes(input: &mut ClassPlanningInput<'_>) -> InitialClassPlans {
    let mut classes = Vec::new();
    let mut verdicts = Vec::new();
    let mut declined = Vec::new();
    let mut facts = Vec::new();
    for class in events::collect_classes(input.classification) {
        let credit_sites =
            events::full_move_credit_sites(input.partition, input.full_move_arms, class);
        let mut class_events = if credit_sites.is_empty() {
            events::extract_class_events(input.func, input.classification, input.partition, class)
        } else {
            events::extract_class_events_with_extraction_credits(
                input.func,
                input.classification,
                input.partition,
                class,
                &credit_sites,
                events::EventFunding::Classified,
            )
        };
        events::apply_full_move_rebook(
            input.partition,
            input.full_move_arms,
            class,
            &mut class_events,
        );
        let outcome = emit::plan_class(input.func, input.preds, input.regions, &class_events, &[]);
        class_ledger_trace!(
            class = ?input.partition.node_key(class),
            events = ?class_events.per_block,
            outcome = ?outcome,
            "class plan probe"
        );
        let (planned_ops, decline) = match &outcome {
            ClassOutcome::Planned(ops) => (ops.as_slice(), None),
            ClassOutcome::Declined(reason) => {
                declined.push((class, *reason));
                (&[][..], Some(*reason))
            }
        };
        let verdict = verify::verify_class(input.func, input.preds, &class_events, planned_ops);
        if verdict != ClassVerdict::Clean || decline.is_some() {
            class_ledger_debug!(
                class = ?input.partition.node_key(class),
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
        facts.push(hazard_facts_for(class, &class_events, &outcome, verdict));
        classes.push(ClassPlan { class, outcome });
    }
    InitialClassPlans {
        classes,
        verdicts,
        declined,
        facts,
    }
}

pub(crate) fn analyze_class_ledger(
    func: &ArcFunction,
    classification: &LedgerClassification,
    partition: &mut BirthSitePartition,
    contracts: &FxHashMap<Name, MemoryContract>,
    type_registry: &ori_types::TypeRegistry,
    interner: &ori_ir::StringInterner,
) -> ClassLedgerAnalysis {
    let preds = compute_predecessors(func);
    let regions = emit::CycleRegions::compute(func);
    let full_move_arms = events::detect_full_move_arms(func, partition, type_registry, contracts);
    let InitialClassPlans {
        mut classes,
        mut verdicts,
        mut declined,
        facts: class_facts,
    } = plan_initial_classes(&mut ClassPlanningInput {
        func,
        classification,
        partition,
        preds: &preds,
        regions: &regions,
        full_move_arms: &full_move_arms,
    });
    let full_move_construct_sites: Vec<(usize, EventSite)> = full_move_arms
        .iter()
        .map(|arm| (arm.block, EventSite::Body(arm.construct_index)))
        .collect();
    let hazards = hazard::field_view_hazard_classes(
        func,
        partition,
        &class_facts,
        &full_move_construct_sites,
        &classification.user_drop_admitted,
    );
    let cure_inputs = hazard::HazardCureInputs::new(
        func,
        classification,
        &preds,
        &regions,
        type_registry,
        interner,
        &full_move_arms,
    );
    let mut cure_state =
        hazard::HazardCureState::new(partition, &mut classes, &mut verdicts, &mut declined);
    let uncured = hazard::cure_endangered_views(&cure_inputs, &mut cure_state, &hazards);
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
        indirect_arg_handoff: classification.indirect_arg_handoff,
        all_vars_excluded: classification.all_vars_excluded,
        consume_covered: classification.consume_covered.clone(),
        copy_out_covered: rustc_hash::FxHashSet::default(),
    }
}

/// Pipeline Step-4b dispatch: attempt the per-function replacement, report
/// the readiness verdict + emission mode on the `ori_arc::aims::class_ledger`
/// tracing target, and return whether the plan replaced the standard
/// burden-op emission.
///
/// `burden_ops_enabled = false` (Step-4b emission disabled)
/// keeps the analysis-only readiness report and never replaces.
pub(crate) fn apply_class_ledger_replacement(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    contracts: &FxHashMap<Name, MemoryContract>,
    type_registry: &ori_types::TypeRegistry,
    interner: &ori_ir::StringInterner,
    burden_ops_enabled: bool,
) -> bool {
    let outcome = attempt_replacement(
        func,
        state_map,
        contracts,
        type_registry,
        interner,
        burden_ops_enabled,
    );
    report_readiness(func, interner, &outcome);
    assert!(
        !burden_ops_enabled || outcome.mode == EmissionMode::Replaced,
        "class-ledger replacement declined for `{}`: {} — every production shape must replace (the legacy Phase-5/6 walk was deleted; no fallback emitter exists)",
        interner.lookup(func.name),
        outcome
            .fallback_reason
            .map_or("<no reason>", replace::FallbackReason::as_str),
    );
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
    class_ledger_debug!(
        function = interner.lookup(func.name),
        classes = analysis.plan.classes.len(),
        planned_ops,
        clean = verdict_count(ClassVerdict::Clean),
        leak_only = verdict_count(ClassVerdict::LeakOnly),
        unprovable = verdict_count(ClassVerdict::Unprovable),
        declined = analysis.readiness.declined.len(),
        all_classes_clean = analysis.readiness.all_classes_clean,
        mode = outcome.mode.as_str(),
        fallback_reason = outcome.fallback_reason.map_or("", FallbackReason::as_str),
        "class-ledger readiness"
    );
}
