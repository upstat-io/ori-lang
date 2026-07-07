//! Per-function replacement of the legacy Step-4b emission by the
//! class-ledger plan, behind the readiness gate.
//!
//! Replacement gate — ALL must hold, any failure falls back to the legacy
//! walk unchanged:
//!
//! - the analysis reports FULLY CLEAN readiness (no declined class, every
//!   class verdict `Clean`) with at least ONE class — a zero-class function
//!   falls back (the class model proves nothing about variables it never
//!   evented, and the legacy walk may still owe ops for them);
//! - no variable's type carries a user `@drop` (the RL-DROP user-drop call
//!   for scalar-repr values is a legacy-walk completeness pass the class
//!   model does not cover);
//! - every planned op's subject variable is defined at a site dominating
//!   the op's insertion slot (the plan's release-var selection is
//!   heuristic; the flat VF-1 defined-set check alone would not catch a
//!   non-dominating pick);
//! - the applied plan passes the VF-1 structural check
//!   (`verify::check_function`) on a CLONE — commit-or-discard.
//!
//! A committed replacement sets `ArcFunction::class_ledger_emission`, which
//! routes realization to mechanical Phase-7 lowering (per that field's
//! contract). `burden_emitted` stays unmarked: the plan places its own RL-4
//! edge releases, so the `carries_burden`-gated edge-cleanup machinery must
//! stay inert for plan variables.

use ori_ir::Name;
use ori_types::TypeRegistry;
use rustc_hash::FxHashMap;

use crate::aims::contract::MemoryContract;
use crate::aims::intraprocedural::AimsStateMap;
use crate::graph::DominatorTree;
use crate::ir::{ArcFunction, ArcTerminator, ArcVarId};
use crate::lower::burden_lookup::type_has_user_drop;

use super::apply::apply_plan;
use super::emit::{ClassOutcome, PlanSlot, PlannedOp};
use super::{analyze_from_state_map, ClassLedgerAnalysis};

/// Step-4b emission mode for one function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EmissionMode {
    /// The class-ledger plan replaced the legacy emission.
    Replaced,
    /// The legacy walk remains the emitter (gate failure; reason recorded).
    Fallback,
}

impl EmissionMode {
    /// Tracing label for the mode.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Replaced => "replaced",
            Self::Fallback => "fallback",
        }
    }
}

/// Outcome of one replacement attempt.
#[derive(Debug)]
pub(crate) struct ReplacementOutcome {
    pub(crate) mode: EmissionMode,
    pub(crate) analysis: ClassLedgerAnalysis,
    /// The failed gate on `Fallback`; `None` on `Replaced`.
    pub(crate) fallback_reason: Option<&'static str>,
}

/// Analyze `func` and commit the class-ledger plan when every replacement
/// gate holds; otherwise leave `func` byte-identical for the legacy walk.
///
/// `allow_replacement = false` runs the analysis only (readiness stays
/// reportable) and never mutates `func`.
pub(crate) fn attempt_replacement(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    contracts: &FxHashMap<Name, MemoryContract>,
    type_registry: &TypeRegistry,
    allow_replacement: bool,
) -> ReplacementOutcome {
    let analysis = analyze_from_state_map(func, state_map, contracts);
    let ops = planned_ops(&analysis);
    if let Some(reason) = gate_rejection(
        func,
        state_map,
        &analysis,
        &ops,
        type_registry,
        allow_replacement,
    ) {
        return ReplacementOutcome {
            mode: EmissionMode::Fallback,
            analysis,
            fallback_reason: Some(reason),
        };
    }
    let mut applied = func.clone();
    apply_plan(&mut applied, &ops);
    applied.class_ledger_emission = true;
    if !crate::verify::check_function(&applied).is_empty() {
        return ReplacementOutcome {
            mode: EmissionMode::Fallback,
            analysis,
            fallback_reason: Some("structural-verify"),
        };
    }
    *func = applied;
    ReplacementOutcome {
        mode: EmissionMode::Replaced,
        analysis,
        fallback_reason: None,
    }
}

/// Every planned op of every class, plan order.
pub(crate) fn planned_ops(analysis: &ClassLedgerAnalysis) -> Vec<PlannedOp> {
    analysis
        .plan
        .classes
        .iter()
        .flat_map(|plan| match &plan.outcome {
            ClassOutcome::Planned(ops) => ops.as_slice(),
            ClassOutcome::Declined(_) => &[],
        })
        .copied()
        .collect()
}

/// The first failed pre-apply gate, `None` when all hold.
fn gate_rejection(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    analysis: &ClassLedgerAnalysis,
    ops: &[PlannedOp],
    type_registry: &TypeRegistry,
    allow_replacement: bool,
) -> Option<&'static str> {
    if !allow_replacement {
        return Some("legacy-emission-disabled");
    }
    if analysis.plan.classes.is_empty() {
        return Some("zero-classes");
    }
    if !analysis.readiness.all_classes_clean {
        return Some("readiness-not-clean");
    }
    // Unmodeled shapes stay on the legacy walk: a Reset/Reuse pairing
    // rebirths the DYING value's allocation (no fresh birth site), and a
    // TRMC context hole threads a fill-at-recursive-call obligation —
    // neither is represented by the class model yet.
    if has_reuse_shape(func) {
        return Some("reuse-shape");
    }
    if has_context_hole(func, state_map) {
        return Some("trmc-context");
    }
    if analysis.field_view_hazard {
        return Some("field-view-liveness");
    }
    if func
        .var_types
        .iter()
        .any(|&ty| type_has_user_drop(ty, type_registry))
    {
        return Some("user-drop-glue");
    }
    if !ops_placeable(func, ops) {
        return Some("op-var-placement");
    }
    None
}

/// Whether any instruction is a `Reset` / `Reuse` / `CollectionReuse`
/// (FBIP allocation-reuse pairing).
fn has_reuse_shape(func: &ArcFunction) -> bool {
    func.blocks.iter().any(|block| {
        block.body.iter().any(|instr| {
            matches!(
                instr,
                crate::ir::ArcInstr::Reset { .. }
                    | crate::ir::ArcInstr::Reuse { .. }
                    | crate::ir::ArcInstr::CollectionReuse { .. }
            )
        })
    })
}

/// Whether any variable carries the TRMC `ContextHole` shape.
fn has_context_hole(func: &ArcFunction, state_map: &AimsStateMap) -> bool {
    (0..func.var_types.len()).any(|raw| {
        let Ok(raw) = u32::try_from(raw) else {
            return false;
        };
        state_map.var_shape(ArcVarId::new(raw)) == crate::aims::lattice::ShapeClass::ContextHole
    })
}

/// The definition point of a variable, for slot-dominance checking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DefPoint {
    /// A function param — defined at entry, dominates every slot.
    Entry,
    /// Defined at a block's front: a block param, or an `Invoke` /
    /// `InvokeIndirect` result (materialized on entry to the NORMAL
    /// successor — the unwind successor never sees it).
    BlockEntry(usize),
    /// Defined by the body instruction at `(block, index)`.
    Body(usize, usize),
}

/// Whether every planned op's variable is defined at a point that dominates
/// (and, same-block, precedes) the op's insertion slot.
pub(super) fn ops_placeable(func: &ArcFunction, ops: &[PlannedOp]) -> bool {
    if ops.is_empty() {
        return true;
    }
    let defs = collect_def_points(func);
    let dom = DominatorTree::build(func);
    ops.iter().all(|op| {
        let placeable = defs
            .get(&op.var)
            .is_some_and(|&def| def_reaches_slot(func, &dom, def, op.slot));
        if !placeable {
            tracing::trace!(
                target: "ori_arc::aims::class_ledger",
                gate = "op-var-placement",
                var = ?op.var,
                slot = ?op.slot,
                def = ?defs.get(&op.var),
                "planned op's variable definition does not dominate its insertion slot"
            );
        }
        placeable
    })
}

/// Definition points of every variable in `func`.
pub(super) fn collect_def_points(func: &ArcFunction) -> FxHashMap<ArcVarId, DefPoint> {
    let mut defs: FxHashMap<ArcVarId, DefPoint> = FxHashMap::default();
    for param in &func.params {
        defs.insert(param.var, DefPoint::Entry);
    }
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for &(var, _) in &block.params {
            defs.insert(var, DefPoint::BlockEntry(block_idx));
        }
        for (instr_idx, instr) in block.body.iter().enumerate() {
            if let Some(dst) = instr.defined_var() {
                defs.insert(dst, DefPoint::Body(block_idx, instr_idx));
            }
        }
        if let ArcTerminator::Invoke { dst, normal, .. }
        | ArcTerminator::InvokeIndirect { dst, normal, .. } = &block.terminator
        {
            defs.insert(*dst, DefPoint::BlockEntry(normal.index()));
        }
    }
    defs
}

/// Whether `def` dominates `slot` — cross-block via the dominator tree
/// (blocks execute atomically, so a dominating block's whole body precedes
/// the slot), same-block via body position.
pub(super) fn def_reaches_slot(
    func: &ArcFunction,
    dom: &DominatorTree,
    def: DefPoint,
    slot: PlanSlot,
) -> bool {
    let slot_block = slot.block();
    if slot_block >= func.blocks.len() {
        return false;
    }
    match def {
        DefPoint::Entry => true,
        DefPoint::BlockEntry(def_block) => {
            def_block == slot_block || dominates(func, dom, def_block, slot_block)
        }
        DefPoint::Body(def_block, def_idx) => {
            if def_block == slot_block {
                match slot {
                    PlanSlot::BlockFront { .. } => false,
                    PlanSlot::BeforeBody { index, .. } => index > def_idx,
                    PlanSlot::AfterBody { index, .. } => index >= def_idx,
                    PlanSlot::BeforeTerminator { .. } => true,
                }
            } else {
                dominates(func, dom, def_block, slot_block)
            }
        }
    }
}

/// Block-index dominance via block ids (block ids equal block indices in
/// pipeline IR; a mismatch is conservative — an unreachable block never
/// dominates).
fn dominates(func: &ArcFunction, dom: &DominatorTree, a: usize, b: usize) -> bool {
    let (Some(block_a), Some(block_b)) = (func.blocks.get(a), func.blocks.get(b)) else {
        return false;
    };
    dom.dominates(block_a.id, block_b.id)
}
