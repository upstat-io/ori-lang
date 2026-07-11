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
//! The pipeline runs the analysis (classification + insertion plan +
//! per-class readiness verdict) and, per function, REPLACES the legacy
//! Step-4b emission with the applied plan when the replacement gate holds
//! (`replace` module: FULLY CLEAN readiness with one class or more, no
//! user-`@drop` type in the function, dominance-checked op placement, and a
//! VF-1 structural check on a clone — commit-or-discard). Any function
//! failing a gate falls back to the legacy walk unchanged; the per-function
//! mode + readiness verdict are reported on the
//! `ori_arc::aims::class_ledger` tracing target. A class whose per-class
//! net dataflow cannot be proven (non-converged, merge-disagreeing, or an
//! inexpressible release) is DECLINED — no ops are planned for it and the
//! readiness summary reports it (fail-closed, never a wrong placement).

mod apply;
mod emit;
mod events;
mod hazard;
mod replace;
mod verify;

pub(crate) use emit::{ClassOutcome, PlannedOp};
pub(crate) use replace::{attempt_replacement, EmissionMode, FallbackReason, ReplacementOutcome};
pub(crate) use verify::{ClassVerdict, ReadinessSummary};

#[cfg(test)]
pub(crate) use emit::{DeclineReason, PlanSlot, PlannedOpKind};

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::MemoryContract;
use crate::aims::intraprocedural::birth_site_partition::{BirthSitePartition, NodeIdx};
use crate::aims::intraprocedural::ledger_events::EventSite;
use crate::aims::intraprocedural::ledger_events::{BoundaryFacts, LedgerClassification};
use crate::aims::intraprocedural::AimsStateMap;
use crate::graph::compute_predecessors;
use crate::ir::ArcFunction;

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

/// Borrowed store args whose value is RUNTIME-COPIED into a map/set: a
/// borrowed arg (position > 0) of an `insert` terminator-`Invoke` whose
/// receiver's definer chain resolves to a `Construct` with a
/// `MapLiteral`/`SetLiteral` ctor. The stored copy carries the value's
/// `@drop` at the container's teardown (RL-DROP §8.1.1 copy-out); the local
/// site owes a fields-only release. Receivers without a local
/// map/set-literal `Construct` definer stay unmarked (conservative).
fn copy_out_store_args(
    func: &ArcFunction,
    type_registry: &ori_types::TypeRegistry,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<crate::ir::ArcVarId> {
    use crate::ir::{ArcInstr, ArcValue, CtorKind};

    let insert_name = interner.intern("insert");
    let definer_is_map_or_set = |mut var: crate::ir::ArcVarId| loop {
        let mut definer = None;
        for arc_block in &func.blocks {
            for instr in &arc_block.body {
                match instr {
                    ArcInstr::Let {
                        dst,
                        value: ArcValue::Var(src),
                        ..
                    } if *dst == var => definer = Some(Err(*src)),
                    ArcInstr::Construct { dst, ctor, .. } if *dst == var => {
                        definer = Some(Ok(matches!(
                            ctor,
                            CtorKind::MapLiteral | CtorKind::SetLiteral
                        )));
                    }
                    _ => {}
                }
            }
        }
        match definer {
            Some(Ok(verdict)) => return verdict,
            Some(Err(src)) => var = src,
            None => return false,
        }
    };
    let mut out = FxHashSet::default();
    for arc_block in &func.blocks {
        let crate::ir::ArcTerminator::Invoke {
            func: callee,
            args,
            arg_ownership,
            ..
        } = &arc_block.terminator
        else {
            continue;
        };
        if *callee != insert_name || args.is_empty() {
            continue;
        }
        if !definer_is_map_or_set(args[0]) {
            continue;
        }
        for (position, &arg) in args.iter().enumerate().skip(1) {
            let borrowed = arg_ownership
                .get(position)
                .is_some_and(|o| matches!(o, crate::ir::ArgOwnership::Borrowed));
            let user_drop = func.var_types.get(arg.index()).is_some_and(|&ty| {
                crate::lower::burden_lookup::type_has_user_drop(ty, type_registry)
            });
            if borrowed && user_drop {
                out.insert(arg);
            }
        }
    }
    out
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
    // discharge with one whole-var release at the death point — the legacy
    // walk emits the same dec on these shapes.
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
        type_registry,
        interner,
    );
    apply_copy_out_rewrite(func, type_registry, interner, &mut partition, &mut analysis);
    analysis
}

/// RL-DROP §8.1.1 copy-out: rewrite the placed WHOLE-VAR releases of a
/// copy-out class to fields-only (`DecPartial` with an empty skip set) —
/// the local site is not the value's death point (the stored copy's
/// teardown glue carries the single `@drop`); the local's funded reference
/// still releases its fields (`RLDROP_copyout_balanced`).
fn apply_copy_out_rewrite(
    func: &ArcFunction,
    type_registry: &ori_types::TypeRegistry,
    interner: &ori_ir::StringInterner,
    partition: &mut BirthSitePartition,
    analysis: &mut ClassLedgerAnalysis,
) {
    let store_args = copy_out_store_args(func, type_registry, interner);
    if store_args.is_empty() {
        return;
    }
    let nodes = partition.nodes_snapshot();
    let copy_out_reps: rustc_hash::FxHashSet<_> = nodes
        .iter()
        .filter(|(var, path, _)| path.is_whole_var() && store_args.contains(var))
        .map(|(_, _, node)| partition.rep_of(*node))
        .collect();
    if copy_out_reps.is_empty() {
        return;
    }
    let var_rep: FxHashMap<crate::ir::ArcVarId, _> = nodes
        .iter()
        .filter(|(_, path, _)| path.is_whole_var())
        .map(|(var, _, node)| (*var, partition.rep_of(*node)))
        .collect();
    for entry in &mut analysis.plan.classes {
        let ClassOutcome::Planned(ops) = &mut entry.outcome else {
            continue;
        };
        for op in ops.iter_mut() {
            let rewrite = matches!(op.kind, emit::PlannedOpKind::Dec)
                && var_rep
                    .get(&op.var)
                    .is_some_and(|rep| copy_out_reps.contains(rep));
            if rewrite {
                op.kind = emit::PlannedOpKind::DecPartial {
                    skip_fields: Vec::new(),
                };
            }
        }
    }
    analysis.copy_out_covered = var_rep
        .iter()
        .filter(|(_, rep)| copy_out_reps.contains(*rep))
        .map(|(var, _)| *var)
        .collect();
}

/// Plan and verify every partition class named by `classification`.
///
/// A declined class contributes no ops; it is verified against the bare
/// event stream (its verdict stays honest) and reported in the readiness
/// summary. The plan is trusted only when NO class declined and every class
/// verifies `Clean`.
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
    let mut consume_sites: Vec<(usize, EventSite)> = Vec::new();
    for (block, evs) in class_events.per_block.iter().enumerate() {
        for ev in evs {
            if ev.kind == events::EventKind::Consume {
                consume_sites.push((block, ev.site));
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
    hazard::ClassHazardFacts {
        class,
        released,
        has_demand,
        self_funded_clean,
        has_credit,
        borrowed_rooted_clean,
        planned_inc_count,
        consume_sites,
    }
}

pub(crate) fn analyze_class_ledger(
    func: &ArcFunction,
    classification: &LedgerClassification,
    partition: &mut BirthSitePartition,
    type_registry: &ori_types::TypeRegistry,
    interner: &ori_ir::StringInterner,
) -> ClassLedgerAnalysis {
    let preds = compute_predecessors(func);
    let regions = emit::CycleRegions::compute(func);
    let mut classes = Vec::new();
    let mut verdicts = Vec::new();
    let mut declined = Vec::new();
    let mut class_facts: Vec<hazard::ClassHazardFacts> = Vec::new();
    let full_move_arms = events::detect_full_move_arms(func, partition, type_registry);
    for class in events::collect_classes(classification) {
        let credit_sites = events::full_move_credit_sites(partition, &full_move_arms, class);
        let mut class_events = if credit_sites.is_empty() {
            events::extract_class_events(func, classification, partition, class)
        } else {
            events::extract_class_events_with_extraction_credits(
                func,
                classification,
                partition,
                class,
                &credit_sites,
                false,
            )
        };
        events::apply_full_move_rebook(partition, &full_move_arms, class, &mut class_events);
        let outcome = emit::plan_class(func, &preds, &regions, &class_events, &[]);
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            class = ?partition.node_key(class),
            events = ?class_events.per_block,
            outcome = ?outcome,
            "class plan probe"
        );
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
        class_facts.push(hazard_facts_for(class, &class_events, &outcome, verdict));
        classes.push(ClassPlan { class, outcome });
    }
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
    let uncured = hazard::cure_endangered_views(
        func,
        classification,
        partition,
        &preds,
        &regions,
        type_registry,
        interner,
        &full_move_arms,
        &hazards,
        &mut classes,
        &mut verdicts,
        &mut declined,
    );
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
/// tracing target, and return whether the plan replaced the legacy emission.
///
/// `legacy_emission_enabled = false` (Step-4b emission disabled)
/// keeps the analysis-only readiness report and never replaces.
pub(crate) fn apply_class_ledger_replacement(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    contracts: &FxHashMap<Name, MemoryContract>,
    type_registry: &ori_types::TypeRegistry,
    interner: &ori_ir::StringInterner,
    legacy_emission_enabled: bool,
) -> bool {
    let outcome = attempt_replacement(
        func,
        state_map,
        contracts,
        type_registry,
        interner,
        legacy_emission_enabled,
    );
    report_readiness(func, interner, &outcome);
    // Fail-loud (migration gate), scoped to the BURDEN-SOLE path — the sole
    // RC path of record: a decline there is an ICE naming the function +
    // failed gate, never a silent legacy fallback. The default coexistence
    // path (predicate stack live) still changes upstream IR shapes and
    // declines shapes the sole path replaces; it retires with the walk.
    assert!(
        !legacy_emission_enabled
            || !crate::aims::realize::rc_remark::on_burden_sole_path()
            || outcome.mode == EmissionMode::Replaced,
        "class-ledger replacement declined for `{}`: {} — every production shape must replace (the legacy Step-4b walk is being removed)",
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
        fallback_reason = outcome.fallback_reason.map_or("", FallbackReason::as_str),
        "class-ledger readiness"
    );
}

#[cfg(test)]
mod tests;
