//! Phase 5 trivial burden emission walker.
//!
//! Reads each owned non-scalar SSA value's `BurdenSpec` and emits `BurdenInc`
//! at every transfer point + `BurdenDec` at every last-use along every
//! reachable CFG path. Pure per-instruction emission driven by SSA def-use;
//! no global flow analysis, no fixpoint, no lattice consultation.

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::Uniqueness;
use crate::graph::{compute_postorder, compute_predecessors};
use crate::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, LitValue};
use crate::ownership::{DerivedOwnership, Ownership};
use ori_ir::Name;
use ori_types::TypeRegistry;
use rustc_hash::{FxHashMap, FxHashSet};

use super::burden::{Burden, BurdenRef, TypeRef};
use super::burden_lookup::{idx_to_type_ref, lookup_burden};

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

    /// Read-only access to the per-block entry-state moved-field map. Per
    /// `Spec: Annex E §AIMS RL-2` INTERSECT-merge semantics: `entry(B) =
    /// INTERSECT over P in predecessors(B): exit(P)` (empty for entry block).
    #[allow(
        dead_code,
        reason = "exposed for future per-block-aware consumers; existing consumers use the union view"
    )]
    pub(crate) fn moved_out_fields_block_entry(&self) -> &[FxHashMap<ArcVarId, FxHashSet<u32>>] {
        &self.moved_out_fields_block_entry
    }

    /// Read-only access to the per-block exit-state moved-field map.
    /// `exit(B) = entry(B) ∪ block_local(B)` per pointwise field-set union.
    #[allow(
        dead_code,
        reason = "exposed for future per-block-aware consumers; existing consumers use the union view"
    )]
    pub(crate) fn moved_out_fields_block_exit(&self) -> &[FxHashMap<ArcVarId, FxHashSet<u32>>] {
        &self.moved_out_fields_block_exit
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
    // Dead FRESH values (defined but never used — e.g. a shadowed `let`
    // rebind) receive a FRESH-site BurdenInc but no last-use BurdenDec (they
    // are never last-used). The predicate-stack emits their dead-value cleanup
    // RcDec per RL-2 (unused owned value -> immediate dec), so they are
    // predicate-stack-managed and must carry no burden ops; suppress the
    // orphaned inc symmetrically (it would otherwise net +1).
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

    let analysis = BurdenAnalysisCtx {
        owned_vars_needing_rc: &owned_vars_needing_rc,
        last_uses_at: &last_uses_at,
        full_move_vars: &full_move_vars,
        partial_move_vars: &partial_move_vars,
        inc_suppressed_vars: &inc_suppressed_vars,
        dup_alias_dsts: &dup_alias_dsts,
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
                    if let Some(slot) = func.burden_emitted.get_mut(var.index()) {
                        *slot = true;
                    }
                }
                ArcInstr::BurdenDecField { base, .. } => {
                    if let Some(slot) = func.burden_emitted.get_mut(base.index()) {
                        *slot = true;
                    }
                }
                _ => {}
            }
        }
    }
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

/// Terminator-transfer-var pre-computation. Computed against the IMMUTABLE
/// `func.blocks` borrow so subsequent mutable iteration can consume per-block
/// transfer sets without aliasing conflict (target-block lookup
/// `func.blocks[target.index()]` would otherwise collide with `iter_mut()`).
///
/// Per AIMS RL-2 ownership-transferring exception:
/// - `Return.value` transfers to caller.
/// - `Jump.args` at positions whose target-block params carry
///   `DerivedOwnership::Owned` transfer to the target block param.
/// - `Invoke`/`InvokeIndirect` arg-positions whose `arg_ownership[pos] ==
///   Owned` transfer ownership to the callee. The canonical helper
///   `ArcTerminator::is_owned_position(pos)` encodes empty-arg_ownership
///   defaults + closure-pos-0 Borrowed semantics in one place.
///
/// Empty `derived_ownership` or out-of-bounds index defaults to `Owned`. The
/// Jump-Borrowed case is structurally vacuous under that default.
fn compute_terminator_transfer_per_block(
    func: &ArcFunction,
    derived_ownership: &[DerivedOwnership],
) -> Vec<FxHashSet<ArcVarId>> {
    func.blocks
        .iter()
        .map(|block| terminator_transfer_vars(block, &func.blocks, derived_ownership))
        .collect()
}

/// Build the transfer-var set for a single block's terminator. Extracted from
/// `compute_terminator_transfer_per_block` to keep cognitive complexity per
/// function under workspace limits.
fn terminator_transfer_vars(
    block: &ArcBlock,
    all_blocks: &[ArcBlock],
    derived_ownership: &[DerivedOwnership],
) -> FxHashSet<ArcVarId> {
    let mut transfers: FxHashSet<ArcVarId> = FxHashSet::default();
    match &block.terminator {
        ArcTerminator::Return { value } => {
            transfers.insert(*value);
        }
        ArcTerminator::Jump { target, args } => {
            let Some(target_block) = all_blocks.get(target.index()) else {
                return transfers;
            };
            for (i, &arg) in args.iter().enumerate() {
                let Some(&(block_param_var, _)) = target_block.params.get(i) else {
                    continue;
                };
                let ownership = derived_ownership
                    .get(block_param_var.index())
                    .copied()
                    .unwrap_or(DerivedOwnership::Owned);
                if matches!(ownership, DerivedOwnership::Owned) {
                    transfers.insert(arg);
                }
            }
        }
        ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. } => {
            for (pos, &var) in block.terminator.used_vars().iter().enumerate() {
                if block.terminator.is_owned_position(pos) {
                    transfers.insert(var);
                }
            }
        }
        _ => {}
    }
    transfers
}

/// Terminator-position `BurdenInc` pre-computation. Per RL-1 (RC inc emitted at
/// every ownership-transfer point on owned non-scalar SSA values), each
/// terminator-position Owned-arg gets a `BurdenInc` emitted before the
/// terminator. Mirrors `emit_instr_burdens` instruction-level behavior which
/// emits `BurdenInc` unconditionally at every `is_owned_position(pos)` position;
/// conservative Phase 5 emission — RC traffic is overcounted but balanced; the
/// lattice rewrite pass eliminates redundant Incs.
///
/// Ordered `Vec<Vec<ArcVarId>>` (NOT `FxHashSet` like the transfer-set), so
/// multi-position-same-var terminators emit one `BurdenInc` per occurrence
/// (e.g., Jump block1, args=[%0, %0] to 2 Owned params emits 2× `BurdenInc`).
///
/// Computed against the IMMUTABLE `func.blocks` borrow so subsequent mutable
/// iteration in `emit_burden_ops_for_blocks` can consume per-block Inc lists
/// without aliasing conflict. AIMS Invariant 5 preserved — `DerivedOwnership` is
/// existing analysis output, not a parallel ownership tracker.
fn compute_terminator_inc_per_block(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    derived_ownership: &[DerivedOwnership],
) -> Vec<Vec<ArcVarId>> {
    func.blocks
        .iter()
        .map(|block| {
            terminator_inc_vars(
                block,
                &func.blocks,
                owned_vars_needing_rc,
                derived_ownership,
            )
        })
        .collect()
}

/// Build the ordered `BurdenInc` list for a single block's terminator. Extracted
/// from `compute_terminator_inc_per_block` to mirror `terminator_transfer_vars`
/// extraction and keep cognitive complexity per function under workspace
/// limits.
///
/// Jump-to-Owned-param: per-position Owned check against `target_block.params[i]`'s
/// `DerivedOwnership`. Empty `derived_ownership` or out-of-bounds defaults to
/// Owned, preserving `terminator_transfer_vars` semantics.
///
/// Invoke / `InvokeIndirect`: per-position check against canonical SSOT helper
/// `ArcTerminator::is_owned_position(pos)`, which encodes empty `arg_ownership`
/// defaults + `InvokeIndirect` closure-pos-0 Borrowed semantics in one place.
///
/// `owned_vars_needing_rc` filter rejects EMPTY-spec scalars per VF-1 `RcOnScalar`.
fn terminator_inc_vars(
    block: &ArcBlock,
    all_blocks: &[ArcBlock],
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    derived_ownership: &[DerivedOwnership],
) -> Vec<ArcVarId> {
    let mut incs: Vec<ArcVarId> = Vec::new();
    match &block.terminator {
        ArcTerminator::Jump { target, args } => {
            let Some(target_block) = all_blocks.get(target.index()) else {
                return incs;
            };
            for (i, &arg) in args.iter().enumerate() {
                if !owned_vars_needing_rc.contains(&arg) {
                    continue;
                }
                let Some(&(block_param_var, _)) = target_block.params.get(i) else {
                    continue;
                };
                let ownership = derived_ownership
                    .get(block_param_var.index())
                    .copied()
                    .unwrap_or(DerivedOwnership::Owned);
                if matches!(ownership, DerivedOwnership::Owned) {
                    incs.push(arg);
                }
            }
        }
        ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. } => {
            for (pos, &var) in block.terminator.used_vars().iter().enumerate() {
                if owned_vars_needing_rc.contains(&var) && block.terminator.is_owned_position(pos) {
                    incs.push(var);
                }
            }
        }
        _ => {}
    }
    incs
}

/// Populate `ctx.moved_out_fields_{block_local,block_entry,block_exit}` per the
/// Non-Drop partial-move forward-flow rule. Three-pass walk over the CFG;
/// BOUNDED structural bookkeeping (finite field set per var, monotone field-set
/// growth → bounded fixpoint).
///
/// **Pass 1**: walk every block's body; record every `ArcInstr::Project
/// { dst, value, field, .. }` as a `dst → (value, field)` entry in a local
/// map.
///
/// **Pass 2**: walk every block's body + terminator; for each transferred
/// var (per `instr_transfer_vars` which honors `is_owned_position` + the
/// Set-value carve-out per `Spec: Annex E §AIMS TF-15` + IA-5 step (1), and
/// per the precomputed `terminator_transfer_per_block` set), if the
/// transferred var matches a `project_dst`, insert `(project_src, field)` into
/// `block_local[block_idx]`. This is the per-block transfer function output
/// ("what gets moved DURING this block").
///
/// Project ALONE does NOT set the bit (per `Spec: Annex E §AIMS TF-4` —
/// Project produces `Borrowed`; `is_owned_position`'s `_ => false`
/// excludes it). Project consumed at a borrowed position (e.g.,
/// `IsShared`) also leaves the bit unset — `IsShared` falls through
/// `_ => false` in `is_owned_position` and has no Set-value-style
/// carve-out.
///
/// **Pass 3 (X.2 merge)**: forward dataflow over the CFG. For each
/// block `B` in reverse-postorder:
///   - `entry(B) := INTERSECT over P in predecessors(B): exit(P)` (or
///     empty map for entry block); only fields moved on ALL incoming
///     paths are "definitely moved" at entry.
///   - `exit(B) := entry(B) ∪ block_local(B)` (pointwise union: for
///     each `(var, fields)` pair, merge field sets via set union).
///
/// Bounded fixpoint via worklist iteration to handle CFG back edges
/// (loops) — monotonicity of `∪` over a finite field-index set
/// (`burden.owned_fields()` is bounded by the struct's declared field
/// count, ≤ 256 in practice) guarantees termination in `O(N_BLOCKS *
/// MAX_FIELDS)` steps. Defensive iteration cap: `max(N_BLOCKS, 64) * 4`
/// rounds per `Spec: Annex E §AIMS IC-7` convergence-bound pattern.
///
/// When E2043 typeck rejection guarantees equal predecessor exit sets the
/// INTERSECT degenerates to pick-any; INTERSECT remains the correct merge —
/// robust across both rejection states and structurally simpler than
/// special-casing per typeck status.
///
/// **Union rebuild**: `moved_out_fields_union` rebuilt as the pointwise
/// union over every `block_exit[B]`. Preserves the `moved_out_fields()`
/// accessor contract; consumed by `compute_full_move_vars` /
/// `compute_partial_move_vars` per `Spec: Annex E §AIMS RL-2`
/// partial-transfer semantics.
fn populate_moved_out_fields(
    ctx: &mut BurdenLowerCtx<'_>,
    func: &ArcFunction,
    terminator_transfer_per_block: &[FxHashSet<ArcVarId>],
) {
    // Pass 1: collect (project_dst → (project_src, field)) tuples.
    let mut project_origins: FxHashMap<ArcVarId, (ArcVarId, u32)> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project {
                dst, value, field, ..
            } = instr
            {
                project_origins.insert(*dst, (*value, *field));
            }
        }
    }

    // Pass 2: walk instructions + terminators; check transfer-vars against
    // project_origins. instr_transfer_vars honors is_owned_position +
    // Set-value carve-out; terminator_transfer_per_block carries
    // Return / Jump-to-Owned-param / Invoke-Owned / InvokeIndirect-Owned
    // per `Spec: Annex E §AIMS RL-2`. Insertions land in
    // `block_local[block_idx]` — the per-block transfer function output
    // consumed by Pass 3's merge.
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for instr in &block.body {
            let transfer_vars = instr_transfer_vars(instr);
            for var in &transfer_vars {
                if let Some(&(src, field)) = project_origins.get(var) {
                    ctx.moved_out_fields_block_local[block_idx]
                        .entry(src)
                        .or_default()
                        .insert(field);
                }
            }
        }
        if let Some(term_transfers) = terminator_transfer_per_block.get(block_idx) {
            for var in term_transfers {
                if let Some(&(src, field)) = project_origins.get(var) {
                    ctx.moved_out_fields_block_local[block_idx]
                        .entry(src)
                        .or_default()
                        .insert(field);
                }
            }
        }
    }

    // Pass 3: forward dataflow with INTERSECT-at-entry merge.
    propagate_moved_out_fields(ctx, func);

    // Union step: rebuild the flat view from block_exit storage. Each
    // var's union is the set of fields moved on ANY reachable CFG path
    // from definition to last use — matches the partial-transfer
    // semantics consumed by `compute_full_move_vars` /
    // `compute_partial_move_vars`. Cleared first to keep
    // `populate_moved_out_fields` idempotent on repeat invocation.
    ctx.moved_out_fields_union.clear();
    for per_block in &ctx.moved_out_fields_block_exit {
        for (&src, fields) in per_block {
            let union_entry = ctx.moved_out_fields_union.entry(src).or_default();
            for &field in fields {
                union_entry.insert(field);
            }
        }
    }
}

/// Pass 3 — forward CFG dataflow propagating moved-field sets via
/// INTERSECT-at-entry merge.
///
/// Computes `entry(B) := INTERSECT over P in predecessors(B): exit(P)`
/// (empty for entry block) and `exit(B) := entry(B) ∪ block_local(B)`
/// for every block in reverse-postorder. Bounded fixpoint via worklist
/// iteration handles back edges; monotonicity of `∪` over a finite
/// field-index set guarantees termination.
///
/// INTERSECT semantics: for each var-key in the intersect result, the
/// field set is the intersection of fields moved on EVERY predecessor
/// path. Vars present in only one predecessor's exit set are DROPPED
/// from the intersect (NOT definitely-moved on this path). Per `Spec:
/// Annex E §AIMS RL-2`, this is the architecturally-correct merge —
/// emitting `BurdenDecPartial` with a field skipped only because ONE
/// of N predecessors moved it would be a use-after-free if the run-time
/// execution took a different predecessor.
fn propagate_moved_out_fields(ctx: &mut BurdenLowerCtx<'_>, func: &ArcFunction) {
    let n = func.blocks.len();
    if n == 0 {
        return;
    }

    let predecessors = compute_predecessors(func);

    // Optimistic-⊤ initialization for MUST-move INTERSECT fixpoint per
    // standard dataflow practice (Kildall 1973; Aho/Lam/Sethi/Ullman
    // chapter 9.3). For an INTERSECT (must-) analysis, non-entry blocks
    // are seeded with the lattice top ⊤ so that INTERSECT with ⊤ acts
    // as identity until each predecessor has been processed at least
    // once. Without ⊤ seeding, a back-edge predecessor's empty initial
    // exit would falsely "intersect away" a fact contributed by a
    // forward predecessor, yielding a strictly-weaker (incorrect)
    // fixpoint at loop-exit blocks.
    //
    // ⊤ here = the universe of all `(project_src, field)` pairs that
    // appear in ANY block_local; no block can possibly move a (var,
    // field) pair outside this universe, so this is a sound upper
    // bound. Entry block stays at ⊥ (empty) — the entry has no
    // predecessors and starts with "nothing yet moved".
    let universe = compute_block_local_universe(&ctx.moved_out_fields_block_local);
    let entry_idx = func.entry.index();
    for b in 0..n {
        if b != entry_idx {
            ctx.moved_out_fields_block_exit[b].clone_from(&universe);
        }
    }

    // Reverse-postorder traversal: visiting blocks in RPO converges on a
    // single pass for DAG IR. Back edges require the outer worklist loop
    // below.
    let mut rpo = compute_postorder(func);
    rpo.reverse();

    // Defensive iteration cap per `Spec: Annex E §AIMS IC-7`
    // convergence-bound pattern. Each round MAY shrink (or grow once at
    // most, on the first non-⊤ pass through a block) one (var, field)
    // pair per block; lattice height is bounded by `n_blocks *
    // max_fields_per_struct`. Practical cap is `max(N_BLOCKS, 64) * 4`
    // — far above any realistic loop-fixpoint depth.
    let iteration_cap = n.saturating_mul(4).max(64);
    let mut changed = true;
    let mut rounds = 0usize;
    while changed && rounds < iteration_cap {
        changed = false;
        rounds += 1;
        for &b in &rpo {
            // Compute new entry state: INTERSECT over predecessors' exits.
            // Entry block (or any unreachable block with no predecessors)
            // has empty entry.
            let new_entry =
                intersect_predecessor_exits(&predecessors[b], &ctx.moved_out_fields_block_exit);

            // Compute new exit state: entry ∪ block_local.
            let new_exit = union_entry_with_local(&new_entry, &ctx.moved_out_fields_block_local[b]);

            if ctx.moved_out_fields_block_entry[b] != new_entry {
                ctx.moved_out_fields_block_entry[b] = new_entry;
                changed = true;
            }
            if ctx.moved_out_fields_block_exit[b] != new_exit {
                ctx.moved_out_fields_block_exit[b] = new_exit;
                changed = true;
            }
        }
    }

    debug_assert!(
        !changed,
        "moved_out_fields fixpoint failed to converge in {iteration_cap} rounds — lattice height should be O(n_blocks * max_fields_per_struct)",
    );
}

/// Compute the universe of `(project_src, field)` pairs that appear in
/// ANY block-local moved-field map. This is the lattice top ⊤ for the
/// MUST-move INTERSECT analysis: a sound upper bound on what any block
/// could possibly move.
fn compute_block_local_universe(
    block_local: &[FxHashMap<ArcVarId, FxHashSet<u32>>],
) -> FxHashMap<ArcVarId, FxHashSet<u32>> {
    let mut universe: FxHashMap<ArcVarId, FxHashSet<u32>> = FxHashMap::default();
    for per_block in block_local {
        for (&src, fields) in per_block {
            let dest = universe.entry(src).or_default();
            for &f in fields {
                dest.insert(f);
            }
        }
    }
    universe
}

/// INTERSECT field-sets across predecessors' exit states.
///
/// For each var-key present in ALL predecessor exit sets, take the
/// intersection of field sets. Var-keys present in only a strict subset
/// of predecessors are dropped (NOT definitely-moved at this entry).
/// Empty predecessor list (entry block) returns an empty map.
fn intersect_predecessor_exits(
    preds: &[usize],
    block_exits: &[FxHashMap<ArcVarId, FxHashSet<u32>>],
) -> FxHashMap<ArcVarId, FxHashSet<u32>> {
    let mut result: FxHashMap<ArcVarId, FxHashSet<u32>> = FxHashMap::default();
    let Some((&first, rest)) = preds.split_first() else {
        return result;
    };
    // Seed from first predecessor.
    for (&src, fields) in &block_exits[first] {
        result.insert(src, fields.clone());
    }
    // Intersect against each remaining predecessor.
    for &p in rest {
        let other = &block_exits[p];
        result.retain(|src, fields| {
            let Some(other_fields) = other.get(src) else {
                return false;
            };
            fields.retain(|f| other_fields.contains(f));
            !fields.is_empty()
        });
    }
    result
}

/// Pointwise union of `entry` and `local`. For each var present in
/// either, the result's field set is the union. Pure function — the
/// per-block transfer function `exit(B) = entry(B) ∪ block_local(B)`.
fn union_entry_with_local(
    entry: &FxHashMap<ArcVarId, FxHashSet<u32>>,
    local: &FxHashMap<ArcVarId, FxHashSet<u32>>,
) -> FxHashMap<ArcVarId, FxHashSet<u32>> {
    let mut result = entry.clone();
    for (&src, fields) in local {
        let dest = result.entry(src).or_default();
        for &f in fields {
            dest.insert(f);
        }
    }
    result
}

/// Derive the full-move var set. For each `var` in `owned_vars_needing_rc`,
/// the full-move criterion holds when every `Burden::owned_fields()` entry's
/// `field_path[0]` (top-level field index) is contained in
/// `moved_out_fields[var]`. Vacuously true for vars with empty
/// `owned_fields()` (treated as not-full-move because such a var would not be
/// in `owned_vars_needing_rc` per the `burden_carries_rc` filter — the vacuous
/// case is unreachable in practice).
///
/// Returns a set of vars whose `BurdenDec` emission is SUPPRESSED at last-use
/// sites + terminator-positions per AIMS RL-2 (`BurdenDec` SHALL be emitted at
/// last use of an owned value UNLESS the last use is ownership-transferring;
/// full-move == complete field-projection transfer).
///
/// Partial-move (some-but-not-all fields covered by `moved_out_fields`) is NOT
/// in the full-move set — those vars still emit a conservative FULL `BurdenDec`
/// at last-use; field-aware partial-drop emission is handled by the
/// `BurdenDecPartial` IR variant.
fn compute_full_move_vars(
    func: &ArcFunction,
    moved_out_fields: &FxHashMap<ArcVarId, FxHashSet<u32>>,
    type_registry: &TypeRegistry,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let mut full_move_vars: FxHashSet<ArcVarId> = FxHashSet::default();
    for &var in owned_vars_needing_rc {
        let Some(moved_fields) = moved_out_fields.get(&var) else {
            continue;
        };
        let var_type = func.var_types[var.index()];
        let ty: TypeRef = idx_to_type_ref(var_type, type_registry);
        let Some(burden) = lookup_burden(ty, type_registry) else {
            continue;
        };
        // Empty owned_fields → vacuous all() returns true; guard against
        // false-positive by requiring at least one owned field. Vars in
        // owned_vars_needing_rc pass burden_carries_rc which excludes EMPTY
        // burdens, so this guard is defensive (catches future edge cases).
        let mut has_owned_field = false;
        let all_top_level_moved = burden.owned_fields().all(|of| {
            has_owned_field = true;
            of.field_path
                .first()
                .is_some_and(|f| moved_fields.contains(f))
        });
        if has_owned_field && all_top_level_moved {
            full_move_vars.insert(var);
        }
    }
    full_move_vars
}

/// Derive the partial-move var map. For each `var` in `owned_vars_needing_rc`
/// whose `moved_out_fields[var]` is non-empty AND `var` is NOT in
/// `full_move_vars`, collect a sorted `Vec<u32>` of the moved-out top-level
/// field indices. This is the `skip_fields` payload for the
/// `BurdenDecPartial { var, skip_fields }` IR variant.
///
/// Sorted-Vec encoding makes pass output deterministic: `moved_out_fields[var]`
/// is a `FxHashSet<u32>` whose iteration order is non-deterministic, so sorting
/// at emission time yields byte-identical IR across runs.
///
/// Returns a map from `ArcVarId` to its sorted `skip_fields`. Vars in
/// `full_move_vars` are excluded (suppression branch handles them); vars
/// with empty `moved_out_fields` are excluded (no skip required → emit full
/// `BurdenDec`). The result feeds the three-way branch in
/// `emit_instr_burdens` and `emit_terminator_burden_decs` at last-use sites.
fn compute_partial_move_vars(
    moved_out_fields: &FxHashMap<ArcVarId, FxHashSet<u32>>,
    full_move_vars: &FxHashSet<ArcVarId>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> FxHashMap<ArcVarId, Vec<u32>> {
    let mut partial: FxHashMap<ArcVarId, Vec<u32>> = FxHashMap::default();
    for (&var, fields) in moved_out_fields {
        if fields.is_empty() {
            continue;
        }
        if !owned_vars_needing_rc.contains(&var) {
            continue;
        }
        if full_move_vars.contains(&var) {
            continue;
        }
        let mut sorted: Vec<u32> = fields.iter().copied().collect();
        sorted.sort_unstable();
        partial.insert(var, sorted);
    }
    partial
}

/// Function-wide analysis results consumed by burden emission. Bundled into
/// one struct so per-instruction and per-terminator helpers share a single
/// reference — a domain newtype carrying the co-varying analysis fields at
/// their shared carrier.
struct BurdenAnalysisCtx<'a> {
    owned_vars_needing_rc: &'a FxHashSet<ArcVarId>,
    last_uses_at: &'a FxHashMap<(usize, usize), Vec<ArcVarId>>,
    full_move_vars: &'a FxHashSet<ArcVarId>,
    partial_move_vars: &'a FxHashMap<ArcVarId, Vec<u32>>,
    // Vars whose paired BurdenDec is transfer-suppressed at their last-use
    // (transferred at last-use instr / terminator, or full-move). The
    // symmetric FRESH-site BurdenInc is suppressed for these in
    // emit_fresh_site_burden_inc to keep the per-value burden ledger balanced.
    inc_suppressed_vars: &'a FxHashSet<ArcVarId>,
    // Let-Var alias dsts whose source stays live (duplication). The
    // predicate-stack owns their RC (RcInc/RcDec); they carry no FRESH-site
    // BurdenInc, so their last-use BurdenDec is suppressed to keep the ledger
    // balanced (it would otherwise net -1).
    dup_alias_dsts: &'a FxHashSet<ArcVarId>,
    contracts: &'a FxHashMap<Name, MemoryContract>,
}

/// Drive the unified single-forward-pass per-block emission. For each
/// instruction, `BurdenInc` is emitted BEFORE for every owned-position arg per
/// `ArcInstr::is_owned_position(pos)`; `BurdenDec` is emitted AFTER for each
/// last-use position EXCEPT when the instruction consumes the var at an owned
/// position (transfer point; ownership transferred per AIMS RL-2). `Set`/
/// `SetTag` carve-outs per AIMS TF-15 apply at both halves; `full_move_vars`
/// suppresses `BurdenDec` emission for vars whose entire owned-field set is
/// covered by `moved_out_fields`.
fn emit_burden_ops_for_blocks(
    func: &mut ArcFunction,
    analysis: &BurdenAnalysisCtx<'_>,
    terminator_transfer_per_block: &[FxHashSet<ArcVarId>],
    terminator_inc_per_block: &[Vec<ArcVarId>],
) {
    // Per-block Inc count map for symmetric Dec emission at terminator-transfer
    // points. Populated DURING the emit walk so the Dec emission sees every Inc
    // actually pushed (FRESH-site Incs from `emit_fresh_site_burden_inc`,
    // instruction-level owned-position Incs from `emit_instr_burdens`, and
    // terminator-position Incs from `emit_terminator_burden_incs`). The
    // terminator Dec emission then emits one BurdenDec per Inc for vars whose
    // last-use is terminator-transferred, preserving VF-1 intraprocedural
    // balance. FRESH-site BurdenInc for Invoke/InvokeIndirect results is indexed
    // by the `normal` successor block where the result `dst` is bound.
    let invoke_result_incs = compute_invoke_result_incs(func, analysis);
    for (block_idx, block) in func.blocks.iter_mut().enumerate() {
        let original = std::mem::take(&mut block.body);
        let terminator_idx = original.len();
        let mut new_body: Vec<ArcInstr> = Vec::with_capacity(original.len() * 2);
        let mut inc_counts: FxHashMap<ArcVarId, usize> = FxHashMap::default();
        // Prepend the Invoke-result FRESH-site Incs bound on this block's
        // normal-entry edge, before any body instruction.
        for &dst in &invoke_result_incs[block_idx] {
            new_body.push(ArcInstr::BurdenInc { var: dst });
            *inc_counts.entry(dst).or_insert(0) += 1;
        }
        for (instr_idx, instr) in original.into_iter().enumerate() {
            let ctx = BurdenEmitCtx {
                block_idx,
                instr_idx,
                analysis,
            };
            let before = new_body.len();
            emit_instr_burdens(&mut new_body, instr, &ctx);
            // Tally every BurdenInc the instruction emitted into this block.
            for emitted in &new_body[before..] {
                if let ArcInstr::BurdenInc { var } = emitted {
                    *inc_counts.entry(*var).or_insert(0) += 1;
                }
            }
        }
        let before_term_incs = new_body.len();
        emit_terminator_burden_incs(&mut new_body, &terminator_inc_per_block[block_idx]);
        for emitted in &new_body[before_term_incs..] {
            if let ArcInstr::BurdenInc { var } = emitted {
                *inc_counts.entry(*var).or_insert(0) += 1;
            }
        }
        emit_terminator_burden_decs(
            &mut new_body,
            block_idx,
            terminator_idx,
            analysis,
            &terminator_transfer_per_block[block_idx],
            &inc_counts,
        );
        block.body = new_body;
    }
}

/// Emit `BurdenInc` for each owned terminator-position arg pre-computed by
/// `compute_terminator_inc_per_block`. Mirrors `emit_instr_burdens`'s
/// instruction-level `BurdenInc` loop — conservative Phase 5 emission at every
/// transfer point per AIMS RL-1; the lattice rewrite eliminates redundant Incs.
///
/// Lands BEFORE `emit_terminator_burden_decs` so the emitted IR sequence at
/// terminator position is `[terminator BurdenIncs] [terminator BurdenDecs]`
/// before the terminator itself (which lives in `block.terminator`, not
/// `block.body`). Decs suppress transfer vars per the existing transfer-set
/// gate; the symmetric Inc emission balances duplication arising from
/// multi-position-same-var terminators.
fn emit_terminator_burden_incs(new_body: &mut Vec<ArcInstr>, incs: &[ArcVarId]) {
    for &var in incs {
        new_body.push(ArcInstr::BurdenInc { var });
    }
}

/// Read-only context bundle for per-instruction burden emission. Carries the
/// position (`block_idx`/`instr_idx`) plus the loop-invariant analysis maps
/// (`owned_vars_needing_rc`, `last_uses_at`, `full_move_vars`,
/// `partial_move_vars`) consumed by `emit_instr_burdens` per AIMS RL-2. Domain
/// newtype bundling the co-varying emission inputs.
struct BurdenEmitCtx<'a> {
    block_idx: usize,
    instr_idx: usize,
    analysis: &'a BurdenAnalysisCtx<'a>,
}

/// Emit `BurdenInc` ops before `instr`, push `instr` itself, then emit
/// `BurdenDec` ops at any last-use position for vars not consumed at an owned
/// position by this instruction. `Set` carve-outs (`value` is Owned via IA-5
/// alias-transfer despite `is_owned_position`'s `_ => false`) are applied
/// symmetrically per AIMS TF-15.
///
/// FRESH-allocating instructions (`Construct`, `PartialApply`, `Reuse`,
/// `CollectionReuse`, `Apply`/`Invoke` with Owned-return contract,
/// `Let { Literal::String }`) emit `BurdenInc dst` at definition site per AIMS
/// TF-3 / TF-5 / TF-6 / TF-7 / TF-9 / TF-9a ("FRESH starts Owned"), symmetric
/// with the scope-exit `BurdenDec` at last-use. Gated on
/// `owned_vars_needing_rc.contains(&dst)` per the coexistence handshake —
/// scalars naturally excluded per the `burden_carries_rc` filter.
fn emit_instr_burdens(new_body: &mut Vec<ArcInstr>, instr: ArcInstr, ctx: &BurdenEmitCtx<'_>) {
    emit_fresh_site_burden_inc(new_body, &instr, ctx);
    // Skip owned-position BurdenInc when the arg's last-use is THIS
    // instruction. The matching BurdenDec would be transfer-suppressed per AIMS
    // RL-2, producing a `Σ Inc - Σ Dec = +1` VF-1 imbalance in
    // `aims/verify/burden_balance.rs`. Suppressing both Inc + Dec keeps the
    // coexistence handshake clean: vars whose physical RC is owned by the
    // `aims/realize/walk.rs` predicate-stack stay OUT of `func.burden_emitted`,
    // preventing `populate_class_covered` from spuriously suppressing
    // predicate-stack RC. Burden* are no-op codegen markers; the predicate-stack
    // realize walk owns the real codegen RC for transferred-out vars.
    let last_use_at_this_instr: &[ArcVarId] = ctx
        .analysis
        .last_uses_at
        .get(&(ctx.block_idx, ctx.instr_idx))
        .map_or(&[], Vec::as_slice);
    for (pos, &arg) in instr.used_vars().iter().enumerate() {
        if instr.is_owned_position(pos) && ctx.analysis.owned_vars_needing_rc.contains(&arg) {
            if last_use_at_this_instr.contains(&arg) {
                continue;
            }
            new_body.push(ArcInstr::BurdenInc { var: arg });
        }
    }
    if let ArcInstr::Set { base, field, value } = &instr {
        // Set old-value drop: emit `BurdenDecField(base.field)` BEFORE the Set
        // mutation when base carries any burden. The codegen layer walks
        // `Burden::owned_fields()` to filter which field positions actually need
        // a drop. Mirrors the symmetric `BurdenInc(value)` below — BurdenInc
        // transfers ownership INTO the field, BurdenDecField releases the prior
        // value OUT. Ordering invariant: BurdenDecField BEFORE BurdenInc(value)
        // BEFORE Set — old release precedes new acquire precedes mutation, so
        // codegen can read the prior value via GEP+load BEFORE the store clobbers
        // it.
        if ctx.analysis.owned_vars_needing_rc.contains(base) {
            new_body.push(ArcInstr::BurdenDecField {
                base: *base,
                field: *field,
            });
        }
        if ctx.analysis.owned_vars_needing_rc.contains(value) {
            new_body.push(ArcInstr::BurdenInc { var: *value });
        }
    }
    if let ArcInstr::SetTag { base, .. } = &instr {
        // SetTag old-variant drop per AIMS TF-15a + RL-10. Whole-var pattern
        // (NOT field-positional): the tag change invalidates ALL payload fields
        // of the OLD variant. Emit BurdenDecVariant BEFORE the SetTag so codegen
        // can GEP+load the current discriminant + dispatch the per-variant
        // burden walk BEFORE the store clobbers the tag. SetTag has no value
        // operand (TF-15a backward demand is `(base, Once)` only), so no
        // symmetric BurdenInc(value) — parallel to Set's BurdenDecField, scoped
        // to the whole variant per RL-10.
        if ctx.analysis.owned_vars_needing_rc.contains(base) {
            new_body.push(ArcInstr::BurdenDecVariant { var: *base });
        }
    }
    let transfer_vars = instr_transfer_vars(&instr);
    new_body.push(instr);
    if let Some(last_use_vars) = ctx
        .analysis
        .last_uses_at
        .get(&(ctx.block_idx, ctx.instr_idx))
    {
        for &var in last_use_vars {
            // Three-way branch per AIMS RL-2:
            // (a) suppress entirely when var is ownership-transferred at this
            //     instr OR var's entire owned-field set was moved (full-move);
            // (b) emit `BurdenDecPartial { var, skip_fields }` when some-but-
            //     not-all owned fields were moved via field-projection
            //     transfers (partial-move; codegen walks owned_fields minus
            //     skip_fields);
            // (c) emit standard `BurdenDec { var }` for the no-projection
            //     conservative baseline.
            //
            // Instruction-level transfer suppression is preserved per the
            // coexistence handshake. The owned-position `BurdenInc` deposited by
            // `emit_instr_burdens` is a VF-1 accounting marker, NOT a real RcInc;
            // codegen's predicate-stack realize walk (consulting `class_covered`)
            // owns the physical RC management for vars consumed at
            // instruction-level owned positions (Apply/PartialApply/Construct/
            // etc.). Adding a symmetric BurdenDec here would mark the var in
            // `func.burden_emitted`, propagate through `populate_class_covered`,
            // and suppress predicate-stack RC emission — causing real-world RC
            // leaks observed in `match_alias::test_closure_*` AOT tests. For VF-1
            // balance, the legacy owned-position Inc/transfer-Dec pattern is
            // rebalanced separately by `emit_terminator_burden_decs` and by
            // `eliminate_burden_ops` paired elision.
            if transfer_vars.contains(&var)
                || ctx.analysis.full_move_vars.contains(&var)
                || ctx.analysis.dup_alias_dsts.contains(&var)
            {
                continue;
            }
            if let Some(skip_fields) = ctx.analysis.partial_move_vars.get(&var) {
                new_body.push(ArcInstr::BurdenDecPartial {
                    var,
                    skip_fields: skip_fields.clone(),
                });
            } else {
                new_body.push(ArcInstr::BurdenDec { var });
            }
        }
    }
}

/// Emit FRESH-site `BurdenInc dst` for instructions that define a
/// freshly-allocated owned value per AIMS TF-3 / TF-5 / TF-6 / TF-7 / TF-9 /
/// TF-9a. Symmetric with the scope-exit `BurdenDec` at last-use; both gated on
/// `owned_vars_needing_rc.contains(&dst)` per the coexistence handshake
/// (scalars excluded by the `burden_carries_rc` filter in
/// `compute_owned_vars_needing_rc`).
///
/// FRESH-allocating definition sites:
///   (a) `Let { Literal::String(_) }` — heap-allocated str body.
///   (b) `Construct` — TF-3 FRESH (`Owned`, `Unique`, `BlockLocal`).
///   (c) `Apply` / `Invoke` with callee `ReturnContract.uniqueness ∈
///       {Unique, MaybeShared}` — TF-6 refined to callee's return shape.
///       Conservative for unknown callees (no contract) — emits the Inc;
///       balanced by the existing terminator/last-use `BurdenDec`.
///   (d) `PartialApply` — TF-7 FRESH(NonReusable).
///   (e) `Reuse` — TF-9 FRESH (inherited shape).
///   (f) `CollectionReuse` — TF-9a FRESH(CollectionBuffer).
///
/// Other definitions (TF-1 scalar Literal, TF-2 Var alias, TF-2a `PrimOp`,
/// TF-4 Project (Borrowed), TF-8 Select (alias-transfer), TF-10 `IsShared`
/// (scalar), TF-10a Reset (scalar)) emit no Inc. Scalars naturally drop out
/// via the `owned_vars_needing_rc` gate.
///
/// `Apply` / `Invoke` with no contract: conservative emission (treat as
/// `MaybeShared` return). Indirect calls (`ApplyIndirect` / `InvokeIndirect`)
/// have no callee identity, so their dst is treated as `MaybeShared` per AIMS
/// TF-5a / TF-6c — also emits the Inc when dst is in `owned_vars_needing_rc`.
fn emit_fresh_site_burden_inc(
    new_body: &mut Vec<ArcInstr>,
    instr: &ArcInstr,
    ctx: &BurdenEmitCtx<'_>,
) {
    let dst = match instr {
        ArcInstr::Let {
            dst,
            value: ArcValue::Literal(LitValue::String(_)),
            ..
        }
        | ArcInstr::Construct { dst, .. }
        | ArcInstr::PartialApply { dst, .. }
        | ArcInstr::Reuse { dst, .. }
        | ArcInstr::CollectionReuse { dst, .. } => *dst,
        ArcInstr::Apply { dst, func, .. } => {
            // TF-6: when the callee has a known contract, gate on its
            // ReturnContract.uniqueness. For Unique / MaybeShared returns,
            // the callee hands an owned reference to the caller — caller
            // owes a BurdenDec at last-use, which the existing emission
            // already covers. The Inc here closes the inc/dec pair.
            // No contract: conservative — treat as MaybeShared (matches
            // TF-5's CONSERVATIVE default of MaybeShared).
            match ctx.analysis.contracts.get(func) {
                Some(c) => match c.return_info.uniqueness {
                    Uniqueness::Unique | Uniqueness::MaybeShared => *dst,
                    Uniqueness::Shared => return,
                },
                None => *dst,
            }
        }
        ArcInstr::ApplyIndirect { dst, .. } => {
            // TF-5a: indirect calls have no callee identity; treated as
            // MaybeShared. Emit FRESH-site Inc to balance the last-use Dec.
            *dst
        }
        _ => return,
    };
    // Suppress the FRESH-site Inc when dst is move-transferred into an owned
    // position: its paired BurdenDec is transfer-suppressed (RL-2), so emitting
    // the Inc would orphan it (VF-1 net=+1). The container's own drop owns the
    // released reference. Non-transferred fresh values keep the paired Inc+Dec.
    if ctx.analysis.owned_vars_needing_rc.contains(&dst)
        && !ctx.analysis.inc_suppressed_vars.contains(&dst)
    {
        new_body.push(ArcInstr::BurdenInc { var: dst });
    }
}

/// Per-block entry `BurdenInc` list for FRESH-allocating `Invoke` /
/// `InvokeIndirect` results. A may-unwind call binds its result `dst` on the
/// `normal` successor edge, so its FRESH-site `BurdenInc` — the terminator
/// analogue of `emit_fresh_site_burden_inc`'s `Apply` / `ApplyIndirect` arms —
/// lands at the TOP of the `normal` successor block. Gated identically per AIMS
/// TF-6 / TF-6a / TF-6c: `Invoke` consults the callee
/// `ReturnContract.uniqueness` (`Unique` / `MaybeShared` emit; `Shared` skips;
/// no contract is conservative `MaybeShared`); `InvokeIndirect` is always
/// conservative. The `owned_vars_needing_rc` + `!inc_suppressed_vars` filter
/// mirrors the final push gate so a transfer-suppressed dst gets no orphan inc.
/// Result indexed by successor block index; consumed by
/// `emit_burden_ops_for_blocks`.
fn compute_invoke_result_incs(
    func: &ArcFunction,
    analysis: &BurdenAnalysisCtx<'_>,
) -> Vec<Vec<ArcVarId>> {
    let mut per_succ: Vec<Vec<ArcVarId>> = vec![Vec::new(); func.blocks.len()];
    for block in &func.blocks {
        let (dst, normal) = match &block.terminator {
            ArcTerminator::Invoke {
                dst,
                func: callee,
                normal,
                ..
            } => {
                let shared_return = matches!(
                    analysis.contracts.get(callee),
                    Some(c) if matches!(c.return_info.uniqueness, Uniqueness::Shared)
                );
                if shared_return {
                    continue;
                }
                (*dst, *normal)
            }
            ArcTerminator::InvokeIndirect { dst, normal, .. } => (*dst, *normal),
            _ => continue,
        };
        if analysis.owned_vars_needing_rc.contains(&dst)
            && !analysis.inc_suppressed_vars.contains(&dst)
        {
            if let Some(slot) = per_succ.get_mut(normal.index()) {
                slot.push(dst);
            }
        }
    }
    per_succ
}

/// Snapshot vars consumed at an owned position by `instr`, used to suppress
/// `BurdenDec` at transfer points per AIMS RL-2. `Set.value` is added
/// explicitly per AIMS TF-15 (`is_owned_position`'s `_ => false` catch-all
/// excludes it).
fn instr_transfer_vars(instr: &ArcInstr) -> FxHashSet<ArcVarId> {
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

/// Terminator-position emission. Per AIMS RL-2, Return transfers ownership to
/// the caller — Return's `value` is a terminator-transfer point. Owned locals
/// whose terminator-position last-use is NOT transferred get `BurdenDec`
/// emitted immediately before the terminator.
///
/// Transfer-suppression preserves Dec emission for vars that received a
/// `BurdenInc` earlier in the block. The FRESH-site Inc emission + the
/// owned-position Inc emission both deposit Incs that need balancing Decs at the
/// transfer point to preserve VF-1's intraprocedural net-zero invariant in
/// `aims/verify/burden_balance.rs`. The Decs are TF-N/A metadata annotations in
/// `aims/realize/walk.rs` — they do NOT drive real `RcDec` emission; they exist
/// solely for VF-1 accounting. The realize walk preserves the transfer semantic
/// (no real `RcDec` on transferred-out values) by treating Burden* instructions
/// as transparent.
///
/// One `BurdenDec` per `BurdenInc` per var: the `inc_counts` map records every
/// `BurdenInc` the emit walk pushed for this block (FRESH-site, owned-position,
/// terminator-position), so multi-position-same-var terminators (e.g., Jump
/// with `args=[%v, %v]` to two Owned params) get matching multi-emit Decs.
fn emit_terminator_burden_decs(
    new_body: &mut Vec<ArcInstr>,
    block_idx: usize,
    terminator_idx: usize,
    analysis: &BurdenAnalysisCtx<'_>,
    terminator_transfer_vars: &FxHashSet<ArcVarId>,
    inc_counts: &FxHashMap<ArcVarId, usize>,
) {
    // Emit symmetric Dec at the terminator for every Inc the block deposited on
    // a transferred-out var. Walk transfer_vars instead of last_uses_at because
    // some vars receive Inc but are NOT in last_uses_at at terminator_idx — a
    // var with a FRESH-Inc at definition whose last-use is the Return terminator
    // is in BOTH last_uses_at AND terminator_transfer_vars, so a last_uses_at
    // walk would `continue` and emit no Dec.
    for &var in terminator_transfer_vars {
        let inc_count = inc_counts.get(&var).copied().unwrap_or(0);
        if inc_count == 0 {
            continue;
        }
        let dec_template = if let Some(skip_fields) = analysis.partial_move_vars.get(&var) {
            ArcInstr::BurdenDecPartial {
                var,
                skip_fields: skip_fields.clone(),
            }
        } else {
            ArcInstr::BurdenDec { var }
        };
        for _ in 0..inc_count {
            new_body.push(dec_template.clone());
        }
    }
    // Vars whose terminator-position last-use is NOT a transfer point still
    // follow the legacy emission path: emit one BurdenDec per last-use entry
    // unless full_move OR dup-alias suppresses. The dup_alias_dsts suppression
    // is symmetric with emit_instr_burdens: a Let-Var alias whose source stays
    // live is predicate-stack-managed (real RcInc/RcDec emitted there) and
    // received no FRESH-site BurdenInc, so a terminator-position BurdenDec would
    // net the alias ledger to -1 per AIMS RL-2 dec-fidelity (VF-1 imbalance).
    // Both positions consume the same `analysis.dup_alias_dsts` set — one
    // suppression source, no parallel computation.
    if let Some(last_use_vars) = analysis.last_uses_at.get(&(block_idx, terminator_idx)) {
        for &var in last_use_vars {
            if terminator_transfer_vars.contains(&var) {
                continue;
            }
            if analysis.full_move_vars.contains(&var) {
                continue;
            }
            if analysis.dup_alias_dsts.contains(&var) {
                continue;
            }
            if let Some(skip_fields) = analysis.partial_move_vars.get(&var) {
                new_body.push(ArcInstr::BurdenDecPartial {
                    var,
                    skip_fields: skip_fields.clone(),
                });
            } else {
                new_body.push(ArcInstr::BurdenDec { var });
            }
        }
    }
}

#[cfg(test)]
mod tests;
