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
    pub(crate) fallback_reason: Option<FallbackReason>,
}

/// Why a function fell back to the legacy Step-4b emission (the closed set
/// of `gate_rejection` outcomes plus the post-apply structural-verify gate).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FallbackReason {
    /// Step-4b emission disabled (`legacy_emission_enabled = false`).
    LegacyEmissionDisabled,
    /// The function evented no partition class.
    ZeroClasses,
    /// A class declined or verified non-`Clean`.
    ReadinessNotClean,
    /// The function carries a `Reset`/`Reuse`/`CollectionReuse` shape.
    ReuseShape,
    /// The function carries a TRMC `ContextHole`-shaped variable.
    TrmcContext,
    /// A heap arg handed through an indirect call — call-site ownership is
    /// unresolved at classification time (unmodeled hand-off).
    IndirectArgOwnership,
    /// An OWNED param whose own contract cardinality is `Absent`: the body
    /// must carry NO reference to it (VF-2 `AbsentParamHasUses`; the caller
    /// retains the release obligation).
    /// An endangered field-path view went uncured — both `hazard` cure-ladder
    /// rungs declined. Every hazard-bearing fixture in the current test
    /// corpus is cured (`!field_view_hazard` pinned in each), and each
    /// individual cure's own decline path is pinned where `plan_class`
    /// itself declines (`emit::DeclineReason`'s tests); no fixture yet
    /// forces both ladder rungs to decline for the SAME view.
    FieldViewLiveness,
    /// A variable's type carries a user `@drop`.
    UserDropGlue,
    /// A planned op's variable definition does not dominate its slot.
    /// Defense in depth over a correct-by-construction planner: every
    /// slot the emitter chooses derives from a real instruction position
    /// reached via the dominator tree, so this arm is not exercised by a
    /// planner-derived plan in the test corpus. The gate itself
    /// (`ops_placeable`) is pinned directly against hand-built off-path
    /// ops in `op_var_placement_requires_dominating_definition`.
    OpVarPlacement,
    /// A planned `DecPartial` skip set names a field outside the
    /// container's own named owned-field surface. Pinned end-to-end (both
    /// the clearing and the rejecting case) via
    /// `field_decomposition_cure_replaces_end_to_end_with_registered_burden`
    /// and `field_decomposition_cure_declines_replacement_on_skip_field_mismatch`.
    FieldDecompositionShape,
    /// The applied plan failed the post-apply VF-1 structural check.
    /// Defense in depth over the pre-apply gates above, which already rule
    /// out every ill-formed shape those gates recognize; `check_function`
    /// itself carries its own dedicated pin corpus in `verify::tests`.
    StructuralVerify,
}

impl FallbackReason {
    /// Tracing label for the reason.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::LegacyEmissionDisabled => "legacy-emission-disabled",
            Self::ZeroClasses => "zero-classes",
            Self::ReadinessNotClean => "readiness-not-clean",
            Self::ReuseShape => "reuse-shape",
            Self::TrmcContext => "trmc-context",
            Self::IndirectArgOwnership => "indirect-arg-ownership",
            Self::FieldViewLiveness => "field-view-liveness",
            Self::UserDropGlue => "user-drop-glue",
            Self::OpVarPlacement => "op-var-placement",
            Self::FieldDecompositionShape => "field-decomposition-shape",
            Self::StructuralVerify => "structural-verify",
        }
    }
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
    interner: &ori_ir::StringInterner,
    allow_replacement: bool,
) -> ReplacementOutcome {
    let analysis = analyze_from_state_map(func, state_map, contracts, type_registry, interner);
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
            fallback_reason: Some(FallbackReason::StructuralVerify),
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
        .cloned()
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
) -> Option<FallbackReason> {
    if !allow_replacement {
        return Some(FallbackReason::LegacyEmissionDisabled);
    }
    // Empty-surface admission: a function whose EVERY variable is excluded
    // (scalar or immortal) carries no RC-bearing value anywhere — no births,
    // no param lifecycle, no call-result entries — so the empty plan is the
    // correct emission (zero classes -> zero placement obligations; the
    // three placement clauses hold vacuously per
    // `AimsProof.Ledger::three_clauses_iff_ledger_safe`). The later gates
    // (`UserDropGlue` for a scalar-repr type carrying a user `@drop`,
    // `ReuseShape`, ...) still run below and decline the shapes an empty
    // emission would mis-handle. A zero-class function with ANY non-excluded
    // variable stays on the fallback — the classifier missing a live heap
    // value is a coverage gap the legacy walk must keep owning.
    if analysis.plan.classes.is_empty() && !analysis.all_vars_excluded {
        if tracing::enabled!(target: "ori_arc::aims::class_ledger", tracing::Level::TRACE) {
            let non_excluded: Vec<u32> = (0..func.var_types.len())
                .filter_map(|raw| u32::try_from(raw).ok())
                .filter(|&raw| !state_map.is_excluded(ArcVarId::new(raw)))
                .collect();
            tracing::trace!(
                target: "ori_arc::aims::class_ledger",
                ?non_excluded,
                "zero-classes decline: classifier evented no class but these vars are not state-map-excluded"
            );
        }
        return Some(FallbackReason::ZeroClasses);
    }
    if !analysis.readiness.all_classes_clean {
        return Some(FallbackReason::ReadinessNotClean);
    }
    // Unmodeled shape stays on the legacy walk: a Reset/Reuse pairing
    // rebirths the DYING value's allocation (no fresh birth site).
    if has_reuse_shape(func) {
        return Some(FallbackReason::ReuseShape);
    }
    // The TRMC ContextHole fill-at-recursive-call IS modeled: the fill's
    // `Set` classifies as mutate(context) + consume(filled value) — the K3
    // derivation (`AimsProof.Ledger::holeFill_is_the_release`; a release
    // placed after the fill is rejected, the fill IS the filled value's
    // release). Spec: Annex E §AIMS §12 (compositional placement, K3).
    // Env: ORI_DISABLE_TRMC_CONTEXT_LEDGER — restores the pre-K3
    // conservative TrmcContext decline for bisection, debug-only
    if trmc_context_ledger_disabled() && has_context_hole(func, state_map) {
        return Some(FallbackReason::TrmcContext);
    }
    if analysis.indirect_arg_handoff {
        return Some(FallbackReason::IndirectArgOwnership);
    }
    if analysis.field_view_hazard {
        return Some(FallbackReason::FieldViewLiveness);
    }
    // User `@drop` admission: a WHOLE-VAR planned release lowers to the
    // standard drop glue (heap repr) or `RcStrategy::UserDrop` (scalar
    // repr), running the user `@drop` exactly once at the class's death
    // point — same observable discipline as the legacy walk (RL-DROP:
    // `userDrop` is balance-neutral). Declines:
    // (a) a FIELD-GRAIN release on a user-drop type — a partial dec
    //     releases fields around the type's own drop glue, so `@drop`
    //     would run never or on a gutted value;
    // (b) a user-drop-typed var with NO whole-var planned release of its
    //     own — an excluded scalar or a suppressed/transferred shape whose
    //     `@drop` the plan does not carry (the legacy walk's RL-DROP
    //     completeness pass covers those).
    let user_drop_var = |var: crate::ir::ArcVarId| {
        func.var_types
            .get(var.index())
            .is_some_and(|&ty| type_has_user_drop(ty, type_registry))
    };
    if ops.iter().any(|op| {
        matches!(op.kind, super::emit::PlannedOpKind::DecPartial { .. }) && user_drop_var(op.var)
    }) {
        return Some(FallbackReason::UserDropGlue);
    }
    let var_has_own_dec = |var: crate::ir::ArcVarId| {
        ops.iter()
            .any(|op| op.var == var && matches!(op.kind, super::emit::PlannedOpKind::Dec))
    };
    // A BORROWED param's release belongs to the caller (RL-2 borrowed
    // discipline) — a user `@drop` impl body's own `self` is the canonical
    // case: the drop glue calls the body with `self` borrowed and runs the
    // release AFTER it returns, so the plan correctly carries no dec.
    let borrowed_param = |var: crate::ir::ArcVarId| {
        func.params
            .iter()
            .any(|p| p.var == var && p.ownership == crate::ownership::Ownership::Borrowed)
    };
    if (0..func.var_types.len()).any(|i| {
        let var = crate::ir::ArcVarId::new(
            u32::try_from(i).unwrap_or_else(|_| panic!("var index {i} fits in u32")),
        );
        user_drop_var(var) && !var_has_own_dec(var) && !borrowed_param(var)
    }) {
        return Some(FallbackReason::UserDropGlue);
    }
    if !ops_placeable(func, ops) {
        return Some(FallbackReason::OpVarPlacement);
    }
    if !dec_partial_skips_valid(func, ops, type_registry) {
        return Some(FallbackReason::FieldDecompositionShape);
    }
    None
}

/// Whether every planned `DecPartial`'s skip set names only indices the
/// container's drop glue walks: the type's OWN named top-level owned fields
/// (struct field indices), or — for a variant-carrying sum burden — its
/// variant ORDINALS (the tag-switched enum glue skips by ordinal; the sum
/// cure produces exactly that grain per the uniform single-payload-variant
/// admission). A skip index outside the walked surface (or a container with
/// no burden) declines the function; the interior walk would silently
/// mis-skip at runtime.
fn dec_partial_skips_valid(
    func: &ArcFunction,
    ops: &[PlannedOp],
    type_registry: &TypeRegistry,
) -> bool {
    use crate::lower::burden::Burden;
    use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

    ops.iter().all(|op| {
        let super::emit::PlannedOpKind::DecPartial { skip_fields } = &op.kind else {
            return true;
        };
        if skip_fields.is_empty() {
            return false;
        }
        let Some(&ty) = func.var_types.get(op.var.index()) else {
            return false;
        };
        let Some(burden) = lookup_burden(idx_to_type_ref(ty, type_registry), type_registry) else {
            return false;
        };
        // Variant IDs are 1-indexed (NonZeroU32); the glue's skip grain is
        // the 0-based ordinal.
        let variant_ordinals: Vec<u32> = burden
            .variant_burdens()
            .map(|variant| variant.variant_id.get().get().saturating_sub(1))
            .collect();
        if !variant_ordinals.is_empty() {
            return skip_fields
                .iter()
                .all(|field| variant_ordinals.contains(field));
        }
        let named: Vec<u32> = burden
            .owned_fields()
            .filter_map(|field| field.field_path.first().copied())
            .collect();
        skip_fields.iter().all(|&field| named.contains(&field))
    })
}

/// Whether every variable of the function is excluded from RC accounting
/// (scalar or immortal) — the empty-surface admission predicate for the
/// zero-classes gate. Params are variables too (`ArcParam.var` indexes the
/// same space), so the sweep covers them.
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
fn trmc_context_ledger_disabled() -> bool {
    std::env::var("ORI_DISABLE_TRMC_CONTEXT_LEDGER").as_deref() == Ok("1")
}

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
