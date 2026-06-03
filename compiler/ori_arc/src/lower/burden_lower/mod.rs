//! Phase 5 trivial burden emission walker.
//!
//! Reads each owned non-scalar SSA value's `BurdenSpec` and emits `BurdenInc`
//! at every transfer point + `BurdenDec` at every last-use along every
//! reachable CFG path. Pure per-instruction emission driven by SSA def-use;
//! no global flow analysis, no fixpoint, no lattice consultation.
//!
//! Cluster layout: the analysis-assembly driver (`emit_burden_ops`),
//! `BurdenLowerCtx`, and the collect / detect / filter helpers live here.
//! `terminator` owns terminator-position transfer + inc precompute;
//! `moved_fields` owns the moved-out-fields forward dataflow + full/partial-move
//! partition; `emit` owns the per-instruction + per-terminator emission.

mod emit;
mod moved_fields;
mod terminator;

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId};
use crate::ownership::{DerivedOwnership, Ownership};
use ori_ir::Name;
use ori_types::TypeRegistry;
use rustc_hash::{FxHashMap, FxHashSet};

use super::burden::{Burden, BurdenRef, TypeRef};
use super::burden_lookup::{idx_to_type_ref, lookup_burden};

use emit::{emit_burden_ops_for_blocks, BurdenAnalysisCtx};
use moved_fields::{compute_full_move_vars, compute_partial_move_vars, populate_moved_out_fields};
use terminator::{compute_terminator_inc_per_block, compute_terminator_transfer_per_block};

/// True iff `burden` carries any RC-tracked dimension. Used by the filter at
/// `emit_burden_ops` to exclude scalars whose `lookup_burden` returns the empty
/// builtin burden. Defends VF-1 `RcOnScalar` invariant.
fn burden_carries_rc(burden: &BurdenRef<'_>) -> bool {
    burden.self_heap_alloc()
        || burden.element_burden().is_some()
        || burden.variant_burdens().next().is_some()
        || burden.owned_fields().next().is_some()
}

/// Per-instruction context accumulated by the emission walker.
///
/// Two storage axes (per-var and per-instruction transfer-point lookups
/// have distinct semantics):
/// - `collected` — per-`ArcVarId` `(var, BurdenSpec lookup)` from `var_types`
///   walk. Filtered by `ArcParam.ownership` for params.
/// - `transfer_points` — per-instruction `(consumed var, BurdenSpec lookup)`
///   for transfer points where ownership transfers (`Construct` with owned
///   arg; `Apply` / `Set` / etc.).
#[derive(Debug, Default)]
pub(crate) struct BurdenLowerCtx<'a> {
    collected: Vec<(ArcVarId, Option<BurdenRef<'a>>)>,
    transfer_points: Vec<(ArcVarId, Option<BurdenRef<'a>>)>,
    last_use_points: Vec<(ArcVarId, usize, usize)>,
    /// Per-block block-LOCAL moved-field bitsets indexed by `block_idx`.
    /// Each entry maps `ArcVarId → set of moved field indices` for
    /// projections that occur within THIS block's body or terminator (the
    /// per-block transfer function output). Filled by Pass 2 of
    /// `populate_moved_out_fields`. `FieldId` is `u32` per
    /// `ArcInstr::Project.field`.
    moved_out_fields_block_local: Vec<FxHashMap<ArcVarId, FxHashSet<u32>>>,
    /// Per-block ENTRY moved-field bitsets indexed by `block_idx`. Computed
    /// at fixpoint as `INTERSECT over P in predecessors(B): exit(P)` (or
    /// empty for entry block). Per `Spec: Annex E §AIMS RL-2`
    /// partial-transfer semantics, only fields moved on ALL incoming paths
    /// are "definitely moved" at block entry. When E2043 typeck rejection
    /// guarantees equal predecessor sets the INTERSECT degenerates to
    /// pick-any; INTERSECT remains the correct merge in both states.
    moved_out_fields_block_entry: Vec<FxHashMap<ArcVarId, FxHashSet<u32>>>,
    /// Per-block EXIT moved-field bitsets indexed by `block_idx`. Computed
    /// at fixpoint as `entry(B) ∪ block_local(B)` (pointwise union: for each
    /// var, union field sets). The flow function for "field moves accumulate
    /// forward along reachable paths".
    moved_out_fields_block_exit: Vec<FxHashMap<ArcVarId, FxHashSet<u32>>>,
    /// Cached union of `moved_out_fields_block_exit` populated at the end of
    /// `populate_moved_out_fields`. The accessor lends a reference into this
    /// field, preserving the `&FxHashMap<...>` accessor contract. Consumed
    /// by `compute_full_move_vars` / `compute_partial_move_vars`; both retain
    /// union-view semantics per `Spec: Annex E §AIMS RL-2` (a var's
    /// `BurdenDec` suppression / `BurdenDecPartial.skip_fields` is the union
    /// across all reachable CFG paths from definition to last use — exactly
    /// the exit-state union).
    moved_out_fields_union: FxHashMap<ArcVarId, FxHashSet<u32>>,
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "accessors consumed by tests only; the returned ctx's accessors \
                  are not yet read by the production pipeline (the class_covered \
                  consumer is pending) — the walk reads the fields directly"
    )
)]
impl<'a> BurdenLowerCtx<'a> {
    /// Construct a fresh `BurdenLowerCtx` sized for `func`'s block count.
    /// All three per-block maps (`moved_out_fields_block_local`,
    /// `moved_out_fields_block_entry`, `moved_out_fields_block_exit`) are
    /// pre-allocated with `func.blocks.len()` empty maps so
    /// `populate_moved_out_fields` can index by `block_idx` without bounds
    /// checking. Other Vec fields (`collected`, `transfer_points`,
    /// `last_use_points`) stay empty; downstream walks populate them via
    /// `push`.
    fn new(func: &ArcFunction) -> Self {
        let n = func.blocks.len();
        Self {
            collected: Vec::new(),
            transfer_points: Vec::new(),
            last_use_points: Vec::new(),
            moved_out_fields_block_local: vec![FxHashMap::default(); n],
            moved_out_fields_block_entry: vec![FxHashMap::default(); n],
            moved_out_fields_block_exit: vec![FxHashMap::default(); n],
            moved_out_fields_union: FxHashMap::default(),
        }
    }

    /// Read-only access to the accumulated `(var, burden lookup)` pairs.
    pub(crate) fn collected_burdens(&self) -> &[(ArcVarId, Option<BurdenRef<'a>>)] {
        &self.collected
    }

    /// Read-only access to the accumulated per-instruction transfer-point
    /// burden lookups for `Construct`, `Apply`, `ApplyIndirect`, `Invoke`,
    /// `InvokeIndirect`, `CollectionReuse`, `Set`, and `PartialApply` owned
    /// positions.
    pub(crate) fn transfer_points(&self) -> &[(ArcVarId, Option<BurdenRef<'a>>)] {
        &self.transfer_points
    }

    /// Read-only access to per-block last-use positions: `(var, block_idx,
    /// instr_idx)`. `BurdenDec(v)` emits immediately following EVERY last-use
    /// of `v` along every reachable CFG path; cross-block liveness flows via
    /// block-param handoffs.
    pub(crate) fn last_use_points(&self) -> &[(ArcVarId, usize, usize)] {
        &self.last_use_points
    }

    /// Read-only access to the moved-field bitset map (union-of-exit-states
    /// view). Populated at the end of `populate_moved_out_fields` from
    /// `moved_out_fields_block_exit`.
    pub(crate) fn moved_out_fields(&self) -> &FxHashMap<ArcVarId, FxHashSet<u32>> {
        &self.moved_out_fields_union
    }
}

/// Walk `func` and emit `BurdenInc` / `BurdenDec` ops per SSA variable from
/// `BurdenSpec` lookups, filtered to owned positions via `DerivedOwnership`.
///
/// Invoked from the AIMS pipeline at Phase 5 (ARC lowering); see
/// `pipeline/aims_pipeline/`.
pub(crate) fn emit_burden_ops<'a>(
    func: &mut ArcFunction,
    type_registry: &'a TypeRegistry,
    // Block-param ownership lookup for Jump-to-Owned-param transfer detection.
    // DerivedOwnership side-table threaded as typed pre-pass input — slice
    // indexed by ArcVarId::raw() matches infer_derived_ownership() return shape.
    // Empty &[] semantically safe — out-of-bounds defaults to Owned. AIMS
    // Invariant 5 (unified model) preserved — DerivedOwnership is existing
    // analysis output, not a parallel ownership tracker.
    derived_ownership: &[DerivedOwnership],
    // Per-function MemoryContracts from interprocedural analysis. Consumed by
    // FRESH-site BurdenInc emission for Apply/Invoke whose callee
    // `ReturnContract.uniqueness ∈ {Unique, MaybeShared}` (i.e., return value
    // is a FRESH allocation owned by caller). AIMS Invariant 5 — read
    // unchanged, no parallel emission.
    // Immortal var bitvector (`detect_immortals`): empty-string literals carry
    // no RC, so they receive NO burden ops (the predicate-stack emits none) —
    // else the FRESH-site inc orphans (VF-1 net=+1). Tests pass `&[]`.
    immortals: &[bool],
    contracts: &FxHashMap<Name, MemoryContract>,
) -> BurdenLowerCtx<'a> {
    let mut ctx = BurdenLowerCtx::new(func);
    collect_owned_burdens(&mut ctx, func, type_registry);
    detect_transfer_points(&mut ctx, func, type_registry);
    detect_last_uses(&mut ctx, func);

    // `owned_vars_needing_rc` filters scalars whose `lookup_burden` returns
    // `Some(BurdenRef)` wrapping the empty builtin burden — required by AIMS
    // DP-1 (`is_rc_needed: Owned ∧ ¬Dead ∧ ¬is_scalar`) + VF-1 `RcOnScalar`.
    let mut owned_vars_needing_rc = compute_owned_vars_needing_rc(&ctx);
    // Exclude immortals (empty-string literals) — no RC, so no burden ops at all.
    owned_vars_needing_rc.retain(|v| !immortals.get(v.index()).copied().unwrap_or(false));
    // Exclude borrowed-derived locals: a `Let { Var(src) }` alias of a borrowed
    // value is itself a borrow (TF-2 propagates the source's Access; a borrowed
    // source yields a borrowed alias). Borrowed values carry NO RC obligation
    // (RL-2: "Borrowed variables do NOT receive decs"), so an alias of a
    // borrowed param MUST get no burden ops — `collect_owned_burdens` filters
    // borrowed PARAMS but a local alias of one is not a param and slips through,
    // producing an orphan last-use BurdenDec (VF-1 net=-1). Propagate the
    // borrow forward through every Let-Var hop to a fixpoint and exclude the set.
    let borrowed_aliases = compute_borrowed_alias_vars(func);
    owned_vars_needing_rc.retain(|v| !borrowed_aliases.contains(v));
    let last_uses_at = group_last_uses_filtered(&ctx, &owned_vars_needing_rc);
    let terminator_transfer_per_block =
        compute_terminator_transfer_per_block(func, derived_ownership);
    let terminator_inc_per_block =
        compute_terminator_inc_per_block(func, &owned_vars_needing_rc, derived_ownership);

    // Populate `moved_out_fields` per the Non-Drop partial-move two-stage rule.
    // Pass 1 collects `(project_dst → (src, field))`; Pass 2 walks instructions
    // + terminators and sets the bit when a transferred var matches a
    // project_dst. Project alone leaves the bit unset (TF-4 Borrowed);
    // `Set.value` carve-out applies via `instr_transfer_vars` (TF-15).
    populate_moved_out_fields(&mut ctx, func, &terminator_transfer_per_block);

    // Derive the full-move var set: vars whose `moved_out_fields[var]` covers
    // every top-level field index of their `Burden::owned_fields()`. BurdenDec
    // emission is suppressed for these per AIMS RL-2 (full-move == complete
    // ownership transfer at field-projection grain → BurdenDec correctly
    // suppressed). Partial-move (some-but-not-all fields covered) still emits a
    // CONSERVATIVE FULL BurdenDec (over-emit, refined by the partial-drop IR
    // variant).
    let full_move_vars = compute_full_move_vars(
        func,
        &ctx.moved_out_fields_union,
        type_registry,
        &owned_vars_needing_rc,
    );

    // Derive the partial-move var map: vars with non-empty
    // `moved_out_fields[var]` that are NOT in `full_move_vars`. Each entry's
    // `skip_fields: Vec<u32>` lists top-level field indices to skip during
    // drop-glue iteration at codegen. `BurdenDecPartial` emission gates on this
    // map per AIMS RL-2 partial-transfer semantics (the non-moved fields still
    // need their drop; skip_fields names the transferred subset). AIMS
    // Invariant 5 case (b) — extends ArcInstr enum on the SAME var dimension;
    // no parallel emission, no shadow tracker.
    let partial_move_vars = compute_partial_move_vars(
        &ctx.moved_out_fields_union,
        &full_move_vars,
        &owned_vars_needing_rc,
    );

    // RL-2 transfer-suppression symmetry: a fresh value whose paired BurdenDec
    // is transfer-suppressed at its LAST-USE must have its FRESH-site BurdenInc
    // suppressed too, else the inc is orphaned and the per-value burden ledger
    // nets +1 (VF-1 imbalance). Mirror the EXACT instruction-level
    // dec-suppression condition in emit_instr_burdens (line ~1221): dec
    // suppressed iff the var is transferred at its last-use instr OR its whole
    // owned-field set was moved (full_move_vars). Terminator-position transfers
    // are NOT included — their decs are emitted by emit_terminator_burden_decs
    // and balanced by emit_terminator_burden_incs, a separate inc/dec pair from
    // the FRESH-site inc. A value transferred at a NON-last use (aliased, still
    // live) keeps its Inc — its dec is emitted at the later non-transfer use.
    let mut inc_suppressed_vars: FxHashSet<ArcVarId> = full_move_vars.clone();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            let Some(last_used) = last_uses_at.get(&(block_idx, instr_idx)) else {
                continue;
            };
            let tv = instr_transfer_vars(instr);
            for &var in last_used {
                if tv.contains(&var) {
                    inc_suppressed_vars.insert(var);
                }
            }
        }
    }

    // RL-2 dec-fidelity for Let-Var aliases: a `Let { Var(src) }` alias whose
    // SOURCE stays live after the alias is a duplication — the predicate-stack
    // emits the alias's real RcInc/RcDec, so the alias carries NO burden ops.
    // It never received a matching FRESH-site BurdenInc (Var aliases are not a
    // FRESH-allocating instr), so emitting its last-use BurdenDec would net the
    // alias's burden ledger to -1 (VF-1 imbalance). Suppress that dec. A
    // move-alias (source used only at the alias) keeps its dec to balance the
    // source's FRESH-site inc. "Source stays live" = source appears in >= 2
    // used-var positions (the alias use plus at least one more downstream).
    let (use_counts, dup_alias_dsts) =
        compute_use_counts_and_dup_aliases(func, &mut inc_suppressed_vars);

    // RL-2 transitive-transfer suppression through Let-Var MOVE-alias chains.
    // A value whose ownership transfers out (Return value, owned call arg,
    // owned Jump arg) discharges its release at the transfer point — RL-2's
    // ownership-transferring-use exception. When a value reaches that transfer
    // point THROUGH a `Let { Var }` move-alias chain (`%d = %s` with `%s` used
    // exactly once — a move, not a duplication), the MOVE SOURCE also transfers
    // out: its only use forwards ownership to `%d`, which forwards to the
    // transfer point. Emitting a last-use BurdenDec on the move source then
    // releases a reference already handed off downstream — a VF-1 net=-1 orphan
    // dec (`id<T>(x) = x` over an owned param is the minimal witness:
    // `%1 = %0; Return %1` would dec `%0` though its ownership returns to the
    // caller). Backward-propagate transfer-suppression through every move-alias
    // hop to a fixpoint; the source set suppresses the source's last-use dec.
    let transfer_via_move_alias =
        compute_transfer_via_move_alias(func, &terminator_transfer_per_block, &use_counts);

    // Symmetry: a FRESH move source whose last-use dec is suppressed (it
    // transfers out via the alias chain) must also have its FRESH-site inc
    // suppressed, else the inc orphans (VF-1 net=+1). A param move source has no
    // FRESH inc, so the union is a no-op for it; only FRESH sources gain the
    // inc-suppression. Keeps the per-value ledger at net 0 for both shapes.
    for &var in &transfer_via_move_alias {
        inc_suppressed_vars.insert(var);
    }

    // Symmetry for borrowed terminator-Invoke args: `invoke_terminator_borrowed_args`
    // (emit.rs) suppresses the terminator-last-use `BurdenDec` for a BORROWED
    // `Invoke`/`InvokeIndirect` arg — the value survives the borrowed call and
    // the predicate-stack edge cleanup discharges its release. A FRESH value
    // created in the terminator block solely to pass borrowed (e.g.
    // `%0 = "hello"; Invoke @f(%0 [borrow])` where the callee stores `%0` into
    // its result) would otherwise keep its FRESH-site `BurdenInc` with the dec
    // suppressed → net=+1 on the path where the value lives on into the result.
    // Suppress the FRESH inc symmetrically so the var carries NO burden ops
    // (`burden_emitted` stays false → edge cleanup emits no paired `BurdenDec`)
    // and is fully predicate-stack-managed (`Spec: Annex E §AIMS RL-2`). A
    // genuine borrow (param / alias, no FRESH inc) gains nothing — no-op.
    let borrowed_terminator_args = compute_borrowed_terminator_invoke_args(func);
    for &var in &borrowed_terminator_args {
        inc_suppressed_vars.insert(var);
    }

    // RL-4 live-out suppression set (`Spec: Annex E §AIMS RL-4`): a per-block
    // last-use `BurdenDec` is a genuine release only when the var is dead at
    // block exit; live-out vars are released on dying CFG edges. See
    // `compute_live_out_owned`.
    let live_out_per_block = compute_live_out_owned(func, &owned_vars_needing_rc);

    let analysis = BurdenAnalysisCtx {
        owned_vars_needing_rc: &owned_vars_needing_rc,
        last_uses_at: &last_uses_at,
        full_move_vars: &full_move_vars,
        partial_move_vars: &partial_move_vars,
        inc_suppressed_vars: &inc_suppressed_vars,
        dup_alias_dsts: &dup_alias_dsts,
        transfer_via_move_alias: &transfer_via_move_alias,
        live_out_per_block: &live_out_per_block,
        contracts,
    };
    emit_burden_ops_for_blocks(
        func,
        &analysis,
        &terminator_transfer_per_block,
        &terminator_inc_per_block,
    );
    populate_burden_emitted(func);
    ctx
}

/// Populate `func.burden_emitted` from the just-emitted burden ops. Walks
/// every block's body once after `emit_burden_ops_for_blocks` completes and
/// sets `burden_emitted[var.index()] = true` for every var targeted by
/// `BurdenInc` / `BurdenDec` / `BurdenDecPartial` / `BurdenDecField` /
/// `BurdenDecVariant`. One linear pass per function, no per-var hash-map churn.
///
/// Coexistence-handshake input consumed downstream by the AIMS
/// post-convergence `class_covered` computation, which gates predicate-stack
/// realization deferral.
fn populate_burden_emitted(func: &mut ArcFunction) {
    if func.burden_emitted.len() != func.var_types.len() {
        func.burden_emitted = vec![false; func.var_types.len()];
    }
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::BurdenInc { var }
                | ArcInstr::BurdenDec { var }
                | ArcInstr::BurdenDecPartial { var, .. }
                | ArcInstr::BurdenDecVariant { var } => {
                    mark_emitted(&mut func.burden_emitted, var.index());
                }
                ArcInstr::BurdenDecField { base, .. } => {
                    mark_emitted(&mut func.burden_emitted, base.index());
                }
                _ => {}
            }
        }
    }
}

/// Set `emitted[idx] = true` when `idx` is in bounds; out-of-bounds is a no-op.
fn mark_emitted(emitted: &mut [bool], idx: usize) {
    if let Some(slot) = emitted.get_mut(idx) {
        *slot = true;
    }
}

/// Compute the set of borrowed-derived vars: borrowed parameters plus every
/// `Let { Var(src) }` alias transitively reachable from one. A borrowed value
/// carries no RC obligation (`Spec: Annex E §AIMS RL-2`), and TF-2 propagates a
/// source's borrow to its Let-Var alias, so an alias of a borrowed param is
/// itself borrowed and must carry no burden ops. `collect_owned_burdens` already
/// excludes borrowed PARAMS; this set additionally excludes their local aliases.
/// Forward fixpoint over Let-Var edges (a 1-hop alias of a borrowed alias is
/// borrowed too).
fn compute_borrowed_alias_vars(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    let mut borrowed: FxHashSet<ArcVarId> = func
        .params
        .iter()
        .filter(|p| matches!(p.ownership, Ownership::Borrowed))
        .map(|p| p.var)
        .collect();
    if borrowed.is_empty() {
        return borrowed;
    }
    let mut changed = true;
    while changed {
        changed = false;
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } = instr
                {
                    if borrowed.contains(src) && borrowed.insert(*dst) {
                        changed = true;
                    }
                }
            }
        }
    }
    borrowed
}

/// Phase 1 — per-`ArcVarId` ownership-filtered burden lookup walk.
///
/// Build `ArcVarId -> Ownership` map from `func.params`. Locals (vars not in
/// params) lack `ArcParam.ownership` and are collected unconditionally; params
/// with `Borrowed` ownership are skipped (only owned `ArcVarId`s carry RC).
fn collect_owned_burdens<'a>(
    ctx: &mut BurdenLowerCtx<'a>,
    func: &ArcFunction,
    type_registry: &'a TypeRegistry,
) {
    let param_ownership: FxHashMap<ArcVarId, Ownership> =
        func.params.iter().map(|p| (p.var, p.ownership)).collect();
    for (raw, &idx) in func.var_types.iter().enumerate() {
        let var = ArcVarId::new(
            u32::try_from(raw).unwrap_or_else(|_| panic!("var index {raw} fits in u32")),
        );
        if matches!(param_ownership.get(&var), Some(Ownership::Borrowed)) {
            continue;
        }
        let ty: TypeRef = idx_to_type_ref(idx, type_registry);
        let burden = lookup_burden(ty, type_registry);
        ctx.collected.push((var, burden));
    }
}

/// Phase 2 — transfer-point detection via the canonical helpers
/// `ArcInstr::used_vars()` and `ArcInstr::is_owned_position(pos)`. Covers
/// `Construct`, `PartialApply`, `CollectionReuse` (positions 1..=args.len),
/// `ApplyIndirect` (positions 1..= for Owned args), and `Apply` (positions
/// 0..args.len with `arg_ownership` filter) through the one canonical helper.
/// `Set`/`SetTag` use the IA-5 alias-transfer model (NOT covered by
/// `is_owned_position`'s `_ => false` catch-all per AIMS TF-15); `Set`'s
/// `value` is handled explicitly. Terminator transfer points land in
/// `compute_terminator_transfer_per_block`.
fn detect_transfer_points<'a>(
    ctx: &mut BurdenLowerCtx<'a>,
    func: &ArcFunction,
    type_registry: &'a TypeRegistry,
) {
    for block in &func.blocks {
        for instr in &block.body {
            for (pos, &arg) in instr.used_vars().iter().enumerate() {
                if instr.is_owned_position(pos) {
                    let arg_idx = func.var_types[arg.index()];
                    let ty: TypeRef = idx_to_type_ref(arg_idx, type_registry);
                    let burden = lookup_burden(ty, type_registry);
                    ctx.transfer_points.push((arg, burden));
                }
            }
            if let ArcInstr::Set { value, .. } = instr {
                let value_idx = func.var_types[value.index()];
                let ty: TypeRef = idx_to_type_ref(value_idx, type_registry);
                let burden = lookup_burden(ty, type_registry);
                ctx.transfer_points.push((*value, burden));
            }
        }
    }
}

/// Phase 3 — per-block backward last-use detection: `BurdenDec(v)` emits
/// immediately following EVERY last-use of `v` along EVERY reachable CFG path.
/// Per-block linear scan, no global flow analysis / fixpoint / lattice
/// consultation. Terminator last-uses register at sentinel idx = `body.len()`
/// so terminator-ordering rules can distinguish them.
fn detect_last_uses(ctx: &mut BurdenLowerCtx<'_>, func: &ArcFunction) {
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let mut seen: FxHashSet<ArcVarId> = FxHashSet::default();
        let terminator_idx = block.body.len();
        for arg in block.terminator.used_vars() {
            if seen.insert(arg) {
                ctx.last_use_points.push((arg, block_idx, terminator_idx));
            }
        }
        for (instr_idx, instr) in block.body.iter().enumerate().rev() {
            for &arg in &instr.used_vars() {
                if seen.insert(arg) {
                    ctx.last_use_points.push((arg, block_idx, instr_idx));
                }
            }
        }
    }
}

/// Filter `ctx.collected` to vars whose burden carries any RC-tracked
/// dimension. `lookup_burden(Idx::INT, ...)` returns `Some(BurdenRef)`
/// carrying the empty builtin burden; the filter MUST reject EMPTY specs via
/// `burden_carries_rc` vs naively admitting any `Some(_)`.
fn compute_owned_vars_needing_rc(ctx: &BurdenLowerCtx<'_>) -> FxHashSet<ArcVarId> {
    ctx.collected
        .iter()
        .filter_map(|(var, burden)| {
            burden
                .as_ref()
                .filter(|b| burden_carries_rc(b))
                .map(|_| *var)
        })
        .collect()
}

/// Group `ctx.last_use_points` by `(block_idx, instr_idx)`, retaining only
/// vars that need RC. Output is consumed by the emission loop to position
/// `BurdenDec` ops at last-use sites.
fn group_last_uses_filtered(
    ctx: &BurdenLowerCtx<'_>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> FxHashMap<(usize, usize), Vec<ArcVarId>> {
    let mut last_uses_at: FxHashMap<(usize, usize), Vec<ArcVarId>> = FxHashMap::default();
    for &(var, b, i) in &ctx.last_use_points {
        if !owned_vars_needing_rc.contains(&var) {
            continue;
        }
        last_uses_at.entry((b, i)).or_default().push(var);
    }
    last_uses_at
}

/// Compute the move-alias transfer-suppression set per AIMS RL-2.
///
/// Seed = every var transferred out at a terminator (`terminator_transfer`)
/// plus every var consumed at an owned instruction position. A value reaching
/// one of those transfer points THROUGH a `Let { Var }` move-alias chain
/// (`%dst = %src` where `%src` is used exactly once) also transfers out: its
/// sole use forwards ownership to `%dst`. Backward-propagate: for every
/// move-alias whose `dst` is in the set, add `src`. Iterate to a fixpoint so
/// multi-hop chains (`%2 = %1; %3 = %2; Return %3`) propagate. The returned set
/// suppresses the last-use `BurdenDec` of every move source in a transfer chain.
fn compute_transfer_via_move_alias(
    func: &ArcFunction,
    terminator_transfer_per_block: &[FxHashSet<ArcVarId>],
    use_counts: &FxHashMap<ArcVarId, u32>,
) -> FxHashSet<ArcVarId> {
    let mut transferred: FxHashSet<ArcVarId> = FxHashSet::default();
    // Seed: terminator-transferred vars.
    for set in terminator_transfer_per_block {
        transferred.extend(set.iter().copied());
    }
    // Seed: instruction owned-position transfers (Construct/Apply/Set/etc.).
    for block in &func.blocks {
        for instr in &block.body {
            transferred.extend(instr_transfer_vars(instr).iter().copied());
        }
    }
    // Move-alias edges `dst -> src` (src used exactly once = a move).
    let mut move_edges: Vec<(ArcVarId, ArcVarId)> = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                if use_counts.get(src).copied().unwrap_or(0) == 1 {
                    move_edges.push((*dst, *src));
                }
            }
        }
    }
    // Fixpoint: a move source transfers out when its dst transfers out.
    let mut changed = true;
    while changed {
        changed = false;
        for &(dst, src) in &move_edges {
            if transferred.contains(&dst) && transferred.insert(src) {
                changed = true;
            }
        }
    }
    transferred
}

/// Vars consumed at a BORROWED arg position of any `Invoke` / `InvokeIndirect`
/// terminator. Mirrors the per-block `invoke_terminator_borrowed_args` in
/// `emit.rs` (which gates the terminator-last-use `BurdenDec` suppression) but
/// computed function-wide for the FRESH-site `BurdenInc` suppression. A FRESH
/// value passed at a borrowed Invoke arg position has its terminator dec
/// suppressed there; suppressing its FRESH inc here keeps the per-value burden
/// ledger empty so the predicate-stack edge cleanup fully owns its release.
fn compute_borrowed_terminator_invoke_args(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    let mut borrowed: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        let term = &block.terminator;
        if !matches!(
            term,
            crate::ir::ArcTerminator::Invoke { .. }
                | crate::ir::ArcTerminator::InvokeIndirect { .. }
        ) {
            continue;
        }
        for (pos, &var) in term.used_vars().iter().enumerate() {
            if !term.is_owned_position(pos) {
                borrowed.insert(var);
            }
        }
    }
    borrowed
}

/// Compute function-wide use counts and the duplication-alias dst set, and
/// extend `inc_suppressed_vars` with dead FRESH values per AIMS RL-2.
///
/// - `use_counts`: total used-var occurrences per `ArcVarId` (body + terminator).
/// - dead-FRESH inc-suppression: a var never used receives a FRESH-site
///   `BurdenInc` but no last-use `BurdenDec`; the predicate-stack emits its
///   dead-value cleanup `RcDec` (RL-2 unused-owned dec), so it carries no
///   burden ops — suppress the orphaned inc (else net +1).
/// - `dup_alias_dsts`: `Let { Var(src) }` dsts whose `src` stays live
///   (`use_counts` ≥ 2) — a duplication alias the predicate-stack manages; its
///   last-use `BurdenDec` is suppressed downstream (else net -1).
fn compute_use_counts_and_dup_aliases(
    func: &ArcFunction,
    inc_suppressed_vars: &mut FxHashSet<ArcVarId>,
) -> (FxHashMap<ArcVarId, u32>, FxHashSet<ArcVarId>) {
    let mut use_counts: FxHashMap<ArcVarId, u32> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            for &v in &instr.used_vars() {
                *use_counts.entry(v).or_default() += 1;
            }
        }
        for v in block.terminator.used_vars() {
            *use_counts.entry(v).or_default() += 1;
        }
    }
    for raw in 0..func.var_types.len() {
        let var = ArcVarId::new(
            u32::try_from(raw).unwrap_or_else(|_| panic!("var index {raw} fits in u32")),
        );
        if !use_counts.contains_key(&var) {
            inc_suppressed_vars.insert(var);
        }
    }
    let mut dup_alias_dsts: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                if use_counts.get(src).copied().unwrap_or(0) >= 2 {
                    dup_alias_dsts.insert(*dst);
                }
            }
        }
    }
    (use_counts, dup_alias_dsts)
}

/// Compute per-block live-out sets restricted to `owned_vars_needing_rc`.
///
/// Standard backward liveness (`live_out(B) = ∪ live_in(S)` over successors `S`;
/// `live_in(B) = gen(B) ∪ (live_out(B) − kill(B))`), filtered to vars that carry
/// an owned-heap burden. Mirrors `crate::liveness::compute_liveness`'s gen/kill
/// shape (an `Invoke` `dst` is a definition at its `normal` successor's entry,
/// like a block param) but is keyed on the burden walk's own
/// `owned_vars_needing_rc` set rather than the `ArcClassification` `needs_rc`
/// predicate — no parallel ownership tracker (AIMS Invariant 5): the set is the
/// burden walk's existing owned-RC classification.
///
/// Consumed by `emit_last_use_decs` + `emit_terminator_burden_decs` to suppress
/// the in-block last-use `BurdenDec` for a var live-out of the block per
/// `Spec: Annex E §AIMS RL-4` (the dec belongs on the dying CFG edge / at the
/// dead-out block, not unconditionally in a block the value outlives).
fn compute_live_out_owned(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> Vec<FxHashSet<ArcVarId>> {
    let n = func.blocks.len();
    let (gen, kill) = compute_owned_gen_kill(func, owned_vars_needing_rc);

    // Fixed-point backward dataflow: `live_out(B) = ∪ live_in(S)`,
    // `live_in(B) = gen(B) ∪ (live_out(B) − kill(B))`.
    let mut live_in: Vec<FxHashSet<ArcVarId>> = vec![FxHashSet::default(); n];
    let mut live_out: Vec<FxHashSet<ArcVarId>> = vec![FxHashSet::default(); n];
    let mut changed = true;
    while changed {
        changed = false;
        for b in (0..n).rev() {
            let mut new_out: FxHashSet<ArcVarId> = FxHashSet::default();
            for succ in crate::graph::successor_block_ids(&func.blocks[b].terminator) {
                let si = succ.index();
                if si < n {
                    new_out.extend(live_in[si].iter().copied());
                }
            }
            let mut new_in = gen[b].clone();
            for &var in &new_out {
                if !kill[b].contains(&var) {
                    new_in.insert(var);
                }
            }
            if new_in != live_in[b] || new_out != live_out[b] {
                changed = true;
                live_in[b] = new_in;
                live_out[b] = new_out;
            }
        }
    }
    live_out
}

/// Per-block `(gen, kill)` sets for `compute_live_out_owned`, restricted to
/// `owned_vars_needing_rc`. `gen` = vars used before any definition in the
/// block; `kill` = vars defined in the block (incl. block params + the
/// `Invoke` `dst` bound at the normal-successor entry).
fn compute_owned_gen_kill(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> (Vec<FxHashSet<ArcVarId>>, Vec<FxHashSet<ArcVarId>>) {
    let n = func.blocks.len();
    let invoke_defs = crate::graph::collect_invoke_defs(func);
    let mut gen: Vec<FxHashSet<ArcVarId>> = vec![FxHashSet::default(); n];
    let mut kill: Vec<FxHashSet<ArcVarId>> = vec![FxHashSet::default(); n];
    for (b, block) in func.blocks.iter().enumerate() {
        let g = &mut gen[b];
        let k = &mut kill[b];
        for &(param_var, _) in &block.params {
            if owned_vars_needing_rc.contains(&param_var) {
                k.insert(param_var);
            }
        }
        if let Some(dsts) = invoke_defs.get(&block.id) {
            for &dst in dsts {
                if owned_vars_needing_rc.contains(&dst) {
                    k.insert(dst);
                }
            }
        }
        for instr in &block.body {
            for var in instr.used_vars() {
                if owned_vars_needing_rc.contains(&var) && !k.contains(&var) {
                    g.insert(var);
                }
            }
            if let Some(dst) = instr.defined_var() {
                if owned_vars_needing_rc.contains(&dst) {
                    k.insert(dst);
                }
            }
        }
        for var in block.terminator.used_vars() {
            if owned_vars_needing_rc.contains(&var) && !k.contains(&var) {
                g.insert(var);
            }
        }
    }
    (gen, kill)
}

/// Snapshot vars consumed at an owned position by `instr`, used to suppress
/// `BurdenDec` at transfer points per AIMS RL-2. `Set.value` is added
/// explicitly per AIMS TF-15 (`is_owned_position`'s `_ => false` catch-all
/// excludes it). Shared by the driver, `moved_fields`, and `emit` submodules.
pub(super) fn instr_transfer_vars(instr: &ArcInstr) -> FxHashSet<ArcVarId> {
    let mut transfer_vars: FxHashSet<ArcVarId> = instr
        .used_vars()
        .iter()
        .enumerate()
        .filter_map(|(pos, &arg)| instr.is_owned_position(pos).then_some(arg))
        .collect();
    if let ArcInstr::Set { value, .. } = instr {
        transfer_vars.insert(*value);
    }
    transfer_vars
}

#[cfg(test)]
mod tests;
