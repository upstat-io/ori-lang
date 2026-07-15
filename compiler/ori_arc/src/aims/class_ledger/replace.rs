//! Per-function application of the class-ledger plan at Step-4b, behind the
//! readiness gate — the sole burden-op producer for the current
//! compiled-counter adapter.
//!
//! Replacement gate — ALL must hold; any failure declines replacement, which
//! is fail-loud (an ICE — no fallback adapter input exists) except when burden-op
//! emission itself is disabled via `ORI_DISABLE_BURDEN_OPS=1`:
//!
//! - the analysis reports FULLY CLEAN readiness (no declined class, every
//!   class verdict `Clean`) with at least ONE class, UNLESS every variable
//!   is excluded (scalar or immortal) — that empty-surface case is ADMITTED
//!   with the empty plan (zero RC-bearing values, so zero placement
//!   obligations hold vacuously). A zero-class function with any
//!   non-excluded variable still declines (the class model proves nothing
//!   about variables it never evented; a live non-excluded variable with no
//!   evented class is a classifier coverage gap, not a genuine zero-RC
//!   function);
//! - no variable's type carries a user `@drop` (the RL-DROP user-drop call
//!   for scalar-repr values is a completeness case the class model does not
//!   yet cover);
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
use crate::ir::{ArcFunction, ArcVarId};
use crate::lower::burden_lookup::type_has_user_drop;

use super::apply::apply_plan;
use super::emit::{ClassOutcome, PlannedOp};
use super::placement::ops_placeable;
use super::{analyze_from_state_map, ClassLedgerAnalysis};

/// Step-4b emission mode for one function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EmissionMode {
    /// The class-ledger plan replaced the standard burden-op emission.
    Replaced,
    /// Replacement declined (gate failure; reason recorded) — fail-loud
    /// downstream unless burden-op emission itself is disabled.
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

/// Why a function's class-ledger replacement was declined (the closed set
/// of `gate_rejection` outcomes plus the post-apply structural-verify gate).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FallbackReason {
    /// Step-4b burden-op emission disabled (`burden_ops_enabled = false`).
    BurdenEmissionDisabled,
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
    /// Defense in depth over the other pre-apply gates in this enum, which
    /// already rule out every ill-formed shape those gates recognize;
    /// `check_function` itself carries its own dedicated pin corpus in
    /// `verify::tests`.
    StructuralVerify,
}

impl FallbackReason {
    /// Tracing label for the reason.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::BurdenEmissionDisabled => "burden-emission-disabled",
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
/// gate holds; otherwise leave `func` byte-identical (no burden ops emitted
/// for this function).
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
        return Some(FallbackReason::BurdenEmissionDisabled);
    }
    // Empty-surface admission: a function whose EVERY variable is excluded
    // (scalar or immortal) carries no RC-bearing value anywhere — no births,
    // no param lifecycle, no call-result entries — so the empty plan is the
    // correct emission (zero classes -> zero placement obligations; the
    // three placement clauses hold vacuously per
    // `AimsProof.Ledger::three_clauses_iff_ledger_safe`). The later gates
    // (`UserDropGlue` for a scalar-repr type carrying a user `@drop`,
    // `ReuseShape`, ...) still run and decline the shapes an empty
    // emission would mis-handle. A zero-class function with ANY non-excluded
    // variable stays declined — the classifier missing a live heap
    // value is a coverage gap, not a genuine zero-RC function.
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
    // Unmodeled shape declines: a Reset/Reuse pairing rebirths the DYING
    // value's allocation (no fresh birth site).
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
    // point per RL-DROP (`userDrop` is balance-neutral). Declines:
    // (a) a FIELD-GRAIN release on a user-drop type — a partial dec
    //     releases fields around the type's own drop glue, so `@drop`
    //     would run never or on a gutted value;
    // (b) a user-drop-typed var with NO whole-var planned release of its
    //     own — an excluded scalar or a suppressed/transferred shape whose
    //     `@drop` the plan does not carry.
    let user_drop_var = |var: crate::ir::ArcVarId| {
        func.var_types
            .get(var.index())
            .is_some_and(|&ty| type_has_user_drop(ty, type_registry))
    };
    if ops.iter().any(|op| {
        matches!(op.kind, super::emit::PlannedOpKind::DecPartial { .. })
            && user_drop_var(op.var)
            && !analysis.copy_out_covered.contains(&op.var)
    }) {
        return Some(FallbackReason::UserDropGlue);
    }
    // ADMITTED scalar user-drop vars (booked drop obligations) are verified
    // by their class verdicts: Clean = every path discharges, by planned
    // Dec or transfer-consume. Every OTHER user-drop var needs one of:
    // a whole-var planned release of its own; a BORROWED-rooted alias of a
    // borrowed param (the caller owns the release; the `@drop` impl body's
    // own `self` and its aliases are the canonical case); or a CONSUME
    // discharge on its class (the reference transfers to an owner — a
    // `Construct` arg, an owned call arg, a `Return` — whose release chain
    // runs the drop glue recursively, releasing the moved value through the
    // owner's teardown).
    let admitted_scalar =
        |var: crate::ir::ArcVarId| state_map.is_scalar(var) && !state_map.is_immortal(var);
    let var_has_own_dec = |var: crate::ir::ArcVarId| {
        ops.iter()
            .any(|op| op.var == var && matches!(op.kind, super::emit::PlannedOpKind::Dec))
    };
    // An OWNED param's `Let { Var }` alias closure names ONE caller-handed
    // allocation; a whole-var planned release on ANY closure member is the
    // param's own release (the impl-method `@eq(self [own], other [own])`
    // bodies release through the alias the walk decs). Purely structural
    // Let-alias lineage — no store/copy-out entanglement.
    let mut owned_alias_dec_covered: rustc_hash::FxHashSet<crate::ir::ArcVarId> =
        rustc_hash::FxHashSet::default();
    for param in func
        .params
        .iter()
        .filter(|p| p.ownership == crate::ownership::Ownership::Owned)
    {
        let closure =
            super::emit::close_over_let_aliases(func, std::iter::once(param.var).collect());
        if closure.iter().any(|&member| var_has_own_dec(member)) {
            owned_alias_dec_covered.extend(closure);
        }
    }
    // Borrowed-ROOTED, not just the param var: a `Let { Var }` alias of a
    // borrowed param names the SAME caller-released allocation (the @drop
    // body's `%2 = %0` self alias is the canonical case), and a `Project`
    // of one is a borrow-view of the same caller-owned allocation tree
    // (the recursive-`@drop` body's field views).
    let borrowed_rooted = super::emit::close_over_borrow_views(
        func,
        func.params
            .iter()
            .filter(|p| p.ownership == crate::ownership::Ownership::Borrowed)
            .map(|p| p.var)
            .collect(),
    );
    if (0..func.var_types.len()).any(|i| {
        let var = crate::ir::ArcVarId::new(
            u32::try_from(i).unwrap_or_else(|_| panic!("var index {i} fits in u32")),
        );
        user_drop_var(var)
            && !admitted_scalar(var)
            && !var_has_own_dec(var)
            && !owned_alias_dec_covered.contains(&var)
            && !borrowed_rooted.contains(&var)
            && !analysis.consume_covered.contains(&var)
            && !analysis.copy_out_covered.contains(&var)
    }) {
        return Some(FallbackReason::UserDropGlue);
    }
    if !ops_placeable(func, ops) {
        return Some(FallbackReason::OpVarPlacement);
    }
    if !dec_partial_skips_valid(func, ops, type_registry, &analysis.copy_out_covered) {
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
    copy_out_covered: &rustc_hash::FxHashSet<ArcVarId>,
) -> bool {
    use crate::lower::burden::Burden;
    use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

    ops.iter().all(|op| {
        let super::emit::PlannedOpKind::DecPartial { skip_fields } = &op.kind else {
            return true;
        };
        if skip_fields.is_empty() {
            // The copy-out rewrite's fields-only release (RL-DROP §8.1.1):
            // release EVERY field, run no user `@drop` (the stored copy's
            // teardown carries it). Any other empty skip stays malformed.
            return copy_out_covered.contains(&op.var);
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
