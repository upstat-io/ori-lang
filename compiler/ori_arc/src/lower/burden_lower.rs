//! Phase 5 trivial burden emission walker.
//!
//! Reads each owned non-scalar SSA value's `BurdenSpec` and emits `BurdenInc`
//! at every transfer point + `BurdenDec` at every last-use along every
//! reachable CFG path. Pure per-instruction emission driven by SSA def-use;
//! no global flow analysis, no fixpoint, no lattice consultation.

use crate::graph::{compute_postorder, compute_predecessors};
use crate::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use crate::ownership::{DerivedOwnership, Ownership};
use ori_types::TypeRegistry;
use rustc_hash::{FxHashMap, FxHashSet};

use super::burden::{Burden, BurdenRef, TypeRef};
use super::burden_lookup::{idx_to_type_ref, lookup_burden};

/// True iff `burden` carries any RC-tracked dimension. Used by the filter at
/// `emit_burden_ops` to exclude scalars whose `lookup_burden` returns the empty
/// builtin burden. Defends VF-1 `RcOnScalar` invariant.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "dead until pipeline wiring lands")
)]
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
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "dead until pipeline wiring lands")
)]
#[derive(Debug, Default)]
pub(crate) struct BurdenLowerCtx<'a> {
    collected: Vec<(ArcVarId, Option<BurdenRef<'a>>)>,
    transfer_points: Vec<(ArcVarId, Option<BurdenRef<'a>>)>,
    last_use_points: Vec<(ArcVarId, usize, usize)>,
    /// §03.4 per-block block-LOCAL moved-field bitsets indexed by
    /// `block_idx`. Each entry maps `ArcVarId → set of moved field indices`
    /// for projections that occur within THIS block's body or terminator
    /// (the per-block transfer function output). Filled by Pass 2 of
    /// `populate_moved_out_fields`.
    ///
    /// `FieldId` is `u32` per `ArcInstr::Project.field` at `instr.rs:76-81`.
    moved_out_fields_block_local: Vec<FxHashMap<ArcVarId, FxHashSet<u32>>>,
    /// §03.4 per-block ENTRY moved-field bitsets indexed by `block_idx`.
    /// Computed at fixpoint as `INTERSECT over P in predecessors(B): exit(P)`
    /// (or empty for entry block). Per `Spec: Annex E §AIMS RL-2`
    /// partial-transfer semantics, only fields moved on ALL incoming paths
    /// are "definitely moved" at block entry. Post-E2043 typeck rejection,
    /// predecessor sets are guaranteed equal so INTERSECT degenerates to
    /// pick-any; INTERSECT remains the architecturally-correct cure that
    /// works in both pre-rejection and post-rejection states.
    moved_out_fields_block_entry: Vec<FxHashMap<ArcVarId, FxHashSet<u32>>>,
    /// §03.4 per-block EXIT moved-field bitsets indexed by `block_idx`.
    /// Computed at fixpoint as `entry(B) ∪ block_local(B)` (pointwise
    /// union: for each var, union field sets). The flow function for
    /// "field moves accumulate forward along reachable paths".
    moved_out_fields_block_exit: Vec<FxHashMap<ArcVarId, FxHashSet<u32>>>,
    /// Cached union of `moved_out_fields_block_exit` populated at the end
    /// of `populate_moved_out_fields`. The accessor lends a reference into
    /// this field — preserves the pre-X.1 `&FxHashMap<...>` accessor
    /// contract. Consumed by `compute_full_move_vars` /
    /// `compute_partial_move_vars`; both retain union-view semantics per
    /// `Spec: Annex E §AIMS RL-2` (a var's `BurdenDec` suppression /
    /// `BurdenDecPartial.skip_fields` is the union across all reachable
    /// CFG paths from definition to last use — exactly the exit-state
    /// union).
    moved_out_fields_union: FxHashMap<ArcVarId, FxHashSet<u32>>,
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "dead until pipeline wiring lands")
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
    /// burden lookups. Cycle 5 ships the `Construct` axis; subsequent cycles
    /// extend with `Apply` / `ApplyIndirect` / `Invoke` / `InvokeIndirect` /
    /// `CollectionReuse` / `Set` / `PartialApply` per §03.2 enumeration.
    pub(crate) fn transfer_points(&self) -> &[(ArcVarId, Option<BurdenRef<'a>>)] {
        &self.transfer_points
    }

    /// Read-only access to per-block last-use positions: `(var, block_idx,
    /// instr_idx)`. Per §03.2 `success_criterion` 2 — `BurdenDec(v)` emits
    /// immediately following EVERY last-use of `v` along every reachable CFG
    /// path. Cycle 8 ships per-block backward-walk scaffold; cross-block
    /// liveness via block-param handoffs lands in §03.3.
    pub(crate) fn last_use_points(&self) -> &[(ArcVarId, usize, usize)] {
        &self.last_use_points
    }

    /// Read-only access to §03.4 moved-field bitset map (union-of-exit-
    /// states view). Populated at the end of `populate_moved_out_fields`
    /// from `moved_out_fields_block_exit`. Contract preserved across the
    /// X.2 refactor: returns the SAME `&FxHashMap<ArcVarId,
    /// FxHashSet<u32>>` shape consumers had pre-X.1.
    pub(crate) fn moved_out_fields(&self) -> &FxHashMap<ArcVarId, FxHashSet<u32>> {
        &self.moved_out_fields_union
    }

    /// Read-only access to the per-block entry-state moved-field map.
    /// Per `Spec: Annex E §AIMS RL-2` INTERSECT-merge semantics:
    /// `entry(B) = INTERSECT over P in predecessors(B): exit(P)` (empty
    /// for entry block). Exposed for future per-block-aware consumers;
    /// X.2 keeps existing consumers on the union view.
    #[allow(
        dead_code,
        reason = "exposed for future per-block-aware consumers; X.2 keeps existing consumers on union view"
    )]
    pub(crate) fn moved_out_fields_block_entry(&self) -> &[FxHashMap<ArcVarId, FxHashSet<u32>>] {
        &self.moved_out_fields_block_entry
    }

    /// Read-only access to the per-block exit-state moved-field map.
    /// `exit(B) = entry(B) ∪ block_local(B)` per pointwise field-set
    /// union. Exposed for future per-block-aware consumers; X.2 keeps
    /// existing consumers on the union view.
    #[allow(
        dead_code,
        reason = "exposed for future per-block-aware consumers; X.2 keeps existing consumers on union view"
    )]
    pub(crate) fn moved_out_fields_block_exit(&self) -> &[FxHashMap<ArcVarId, FxHashSet<u32>>] {
        &self.moved_out_fields_block_exit
    }
}

/// Walk `func` and accumulate `BurdenSpec` lookups per SSA variable. Cycle 2
/// ships the iteration scaffold + classifier wiring; cycles 3+ add the owned
/// filter (via `DerivedOwnership`) and replace accumulation with actual
/// `BurdenInc` / `BurdenDec` emission per `BurdenSpec` walks.
///
/// Invoked from the AIMS pipeline at Phase 5 (ARC lowering); see
/// `pipeline/aims_pipeline/`.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "dead until pipeline wiring lands")
)]
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
) -> BurdenLowerCtx<'a> {
    let mut ctx = BurdenLowerCtx::new(func);
    collect_owned_burdens(&mut ctx, func, type_registry);
    detect_transfer_points(&mut ctx, func, type_registry);
    detect_last_uses(&mut ctx, func);

    // `owned_vars_needing_rc` filters scalars whose `lookup_burden` returns
    // `Some(BurdenRef)` wrapping `BuiltinBurdenSpec::EMPTY` per `BURDEN_TABLE`
    // at `ori_registry/src/burden/table.rs:184-193` — required by `aims-rules.md
    // §4 DP-1` (`is_rc_needed: Owned ∧ ¬Dead ∧ ¬is_scalar`) + `§9 VF-1 RcOnScalar`.
    let owned_vars_needing_rc = compute_owned_vars_needing_rc(&ctx);
    let last_uses_at = group_last_uses_filtered(&ctx, &owned_vars_needing_rc);
    let terminator_transfer_per_block =
        compute_terminator_transfer_per_block(func, derived_ownership);
    let terminator_inc_per_block =
        compute_terminator_inc_per_block(func, &owned_vars_needing_rc, derived_ownership);

    // §03.4 cycle 42 — populate `moved_out_fields` per proposal §Non-Drop
    // Partial-Move two-stage rule. Pass 1 collects `(project_dst → (src, field))`;
    // Pass 2 walks instructions + terminators and sets the bit when a transferred
    // var matches a project_dst. Project alone leaves the bit unset (TF-4
    // Borrowed); `Set.value` carve-out applies via `instr_transfer_vars` (TF-15).
    populate_moved_out_fields(&mut ctx, func, &terminator_transfer_per_block);

    // §03.4 cycle 43 — derive the full-move var set: vars whose
    // `moved_out_fields[var]` covers every top-level field index of their
    // `Burden::owned_fields()`. BurdenDec emission is suppressed for these
    // per `aims-rules.md §8 RL-2` (full-move == complete ownership transfer at
    // field-projection grain → BurdenDec correctly suppressed). Partial-move
    // (some-but-not-all fields covered) still emits a CONSERVATIVE FULL
    // BurdenDec (over-emit; cycle 44 introduces partial-drop IR variant).
    let full_move_vars = compute_full_move_vars(
        func,
        &ctx.moved_out_fields_union,
        type_registry,
        &owned_vars_needing_rc,
    );

    // §03.4 cycle 46 — derive the partial-move var map: vars with non-empty
    // `moved_out_fields[var]` that are NOT in `full_move_vars`. Each entry's
    // `skip_fields: Vec<u32>` lists top-level field indices to skip during
    // drop-glue iteration at codegen (cycle 44c). `BurdenDecPartial` emission
    // gates on this map per `aims-rules.md §8 RL-2` partial-transfer semantics
    // (the non-moved fields still need their drop; skip_fields names the
    // transferred subset). AIMS Invariant 5 case (b) — extends ArcInstr enum
    // on the SAME var dimension; no parallel emission, no shadow tracker.
    let partial_move_vars = compute_partial_move_vars(
        &ctx.moved_out_fields_union,
        &full_move_vars,
        &owned_vars_needing_rc,
    );

    emit_burden_ops_for_blocks(
        func,
        &owned_vars_needing_rc,
        &last_uses_at,
        &terminator_transfer_per_block,
        &terminator_inc_per_block,
        &full_move_vars,
        &partial_move_vars,
    );
    ctx
}

/// Phase 1 — per-`ArcVarId` ownership-filtered burden lookup walk.
///
/// Build `ArcVarId -> Ownership` map from `func.params`. Locals (vars not in
/// params) lack `ArcParam.ownership`; cycle 5+ wires `DerivedOwnership` for
/// per-local ownership filtering. Until then, locals are NOT filtered
/// (collected unconditionally) — params with `Borrowed` ownership ARE
/// skipped per §03.2 checkbox 1 ("For each owned `ArcVarId` v").
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "dead until pipeline wiring lands")
)]
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

/// Phase 2 — transfer-point detection via canonical SSOT helpers
/// `ArcInstr::used_vars()` and `ArcInstr::is_owned_position(pos)` per
/// `instr.rs:330-393` and `ir/mod.rs::used_vars`. Mechanically covers
/// `Construct`, `PartialApply`, `CollectionReuse` (positions 1..=args.len),
/// `ApplyIndirect` (positions 1..= for Owned args), and `Apply` (positions
/// 0..args.len with `arg_ownership` filter) via the canonical helper —
/// single source of truth per `impl-hygiene.md §SSOT`. `Set`/`SetTag` use
/// the IA-5 alias-transfer model (NOT covered by `is_owned_position`'s
/// `_ => false` catch-all per `aims-rules.md §3 TF-15`); `Set`'s `value`
/// is handled explicitly. Terminator transfer points land in
/// `compute_terminator_transfer_per_block`.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "dead until pipeline wiring lands")
)]
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

/// Phase 3 — per-block backward last-use detection per §03.2 `success_criterion`
/// 2 ("BurdenDec(v) emits immediately following EVERY last-use of v along
/// EVERY reachable CFG path"). Per-block linear scan satisfies the §03.2 goal
/// ban on global flow analysis / fixpoint / lattice consultation. Terminator
/// last-uses register at sentinel idx = `body.len()` so §03.3 terminator-
/// ordering rules can distinguish them.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "dead until pipeline wiring lands")
)]
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
/// carrying `BuiltinBurdenSpec::EMPTY` (per `BURDEN_TABLE` at
/// `ori_registry/src/burden/table.rs:184-193`); the filter MUST reject EMPTY
/// specs via `burden_carries_rc` vs naively admitting any `Some(_)`.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "dead until pipeline wiring lands")
)]
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
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "dead until pipeline wiring lands")
)]
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

/// §03.3 terminator-transfer-var pre-computation. Computed against the
/// IMMUTABLE `func.blocks` borrow so subsequent mutable iteration can consume
/// per-block transfer sets without aliasing conflict (target-block lookup
/// `func.blocks[target.index()]` would otherwise collide with `iter_mut()`).
///
/// Per `aims-rules.md §8 RL-2` ownership-transferring exception:
/// - `Return.value` transfers to caller.
/// - `Jump.args` at positions whose target-block params carry
///   `DerivedOwnership::Owned` transfer to the target block param (rule 3).
/// - `Invoke`/`InvokeIndirect` arg-positions whose `arg_ownership[pos] ==
///   Owned` transfer ownership to the callee (rule 5). Canonical SSOT helper
///   `ArcTerminator::is_owned_position(pos)` at `compiler_repo/compiler/
///   ori_arc/src/ir/terminator.rs:100-129` encodes empty-arg_ownership
///   defaults + closure-pos-0 Borrowed semantics in one place per
///   `impl-hygiene.md §SSOT`.
///
/// Empty `derived_ownership` or out-of-bounds index defaults to `Owned` per
/// `borrow/derived.rs:60`. Rule 4 (Jump-Borrowed) is structurally vacuous
/// under that semantic — verified at cycle 36 batch-flip.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "dead until pipeline wiring lands")
)]
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
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "dead until pipeline wiring lands")
)]
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

/// §03.4 populate `ctx.moved_out_fields_{block_local,block_entry,block_exit}`
/// per proposal §Non-Drop Partial-Move forward-flow rule. Three-pass walk
/// over the CFG; per §03.4 framing, BOUNDED structural bookkeeping (finite
/// field set per var, monotone field-set growth → bounded fixpoint).
///
/// **Pass 1**: walk every block's body; record every `ArcInstr::Project
/// { dst, value, field, .. }` as a `dst → (value, field)` entry in a local
/// map.
///
/// **Pass 2**: walk every block's body + terminator; for each transferred
/// var (per `instr_transfer_vars` which honors `is_owned_position` + the
/// Set-value carve-out per `Spec: Annex E §AIMS TF-15` + IA-5 step (1),
/// and per the precomputed `terminator_transfer_per_block` set), if the
/// transferred var matches a `project_dst`, insert `(project_src, field)`
/// into `block_local[block_idx]`. This is the per-block transfer
/// function output ("what gets moved DURING this block").
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
/// Post-E2043 typeck rejection (line 2371 SHIPPED), predecessor exit
/// sets are guaranteed equal so the INTERSECT degenerates to pick-any;
/// implementing INTERSECT remains architecturally-correct — robust to
/// future typeck-rejection bugs AND structurally simpler than special-
/// casing per typeck status.
///
/// **Union rebuild**: `moved_out_fields_union` rebuilt as the pointwise
/// union over every `block_exit[B]`. Preserves the `moved_out_fields()`
/// accessor contract; consumed by `compute_full_move_vars` /
/// `compute_partial_move_vars` per `Spec: Annex E §AIMS RL-2`
/// partial-transfer semantics.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "dead until pipeline wiring lands")
)]
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

/// §03.4 Pass 3 — forward CFG dataflow propagating moved-field sets via
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

/// §03.4 cycle 43 — derive the full-move var set. For each `var` in
/// `owned_vars_needing_rc`, the full-move criterion holds when every
/// `Burden::owned_fields()` entry's `field_path[0]` (top-level field index)
/// is contained in `moved_out_fields[var]`. Vacuously true for vars with
/// empty `owned_fields()` (treated as not-full-move because the var would
/// not be in `owned_vars_needing_rc` per `burden_carries_rc` filter — the
/// vacuous case is unreachable in practice).
///
/// Returns a set of vars whose `BurdenDec` emission is SUPPRESSED at last-use
/// sites + terminator-positions per `aims-rules.md §8 RL-2` ("`BurdenDec`
/// SHALL be emitted at last use of owned value... UNLESS last use is
/// ownership-transferring"; full-move == complete field-projection
/// transfer).
///
/// Partial-move (some-but-not-all fields covered by `moved_out_fields`) is
/// NOT in the full-move set — those vars still emit a conservative FULL
/// `BurdenDec` at last-use (cycle 43 baseline). Field-aware partial-drop
/// emission lands in cycle 44 via IR variant evolution.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "dead until pipeline wiring lands")
)]
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

/// §03.4 cycle 46 — derive the partial-move var map. For each `var` in
/// `owned_vars_needing_rc` whose `moved_out_fields[var]` is non-empty AND
/// `var` is NOT in `full_move_vars`, collect a sorted `Vec<u32>` of the
/// moved-out top-level field indices. This is the `skip_fields` payload
/// for the `BurdenDecPartial { var, skip_fields }` IR variant.
///
/// Sorted-Vec encoding satisfies determinism (`impl-hygiene.md §Pass
/// Composition — Pass determinism`); `moved_out_fields[var]` is a
/// `FxHashSet<u32>` whose iteration order is non-deterministic. Sorting at
/// emission time yields byte-identical IR across runs.
///
/// Returns a map from `ArcVarId` to its sorted `skip_fields`. Vars in
/// `full_move_vars` are excluded (suppression branch handles them); vars
/// with empty `moved_out_fields` are excluded (no skip required → emit full
/// `BurdenDec`). The result feeds the three-way branch in
/// `emit_instr_burdens` and `emit_terminator_burden_decs` at last-use sites.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "dead until pipeline wiring lands")
)]
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

/// Drive the unified single-forward-pass per-block emission. For each instruction:
/// - `BurdenInc` emitted BEFORE for every owned-position arg per
///   `ArcInstr::is_owned_position(pos)` SSOT helper (§03.2 sc 1).
/// - `BurdenDec` emitted AFTER for each last-use position EXCEPT when the
///   instruction consumes the var at an owned position (transfer point;
///   ownership transferred per `aims-rules.md §8 RL-2`).
///
/// `Set`/`SetTag` carve-outs per `aims-rules.md §3 TF-15` apply at both halves.
/// §03.4 cycle 43: `full_move_vars` suppresses `BurdenDec` emission for vars
/// whose entire owned-field set is covered by `moved_out_fields`.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "dead until pipeline wiring lands")
)]
fn emit_burden_ops_for_blocks(
    func: &mut ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    last_uses_at: &FxHashMap<(usize, usize), Vec<ArcVarId>>,
    terminator_transfer_per_block: &[FxHashSet<ArcVarId>],
    terminator_inc_per_block: &[Vec<ArcVarId>],
    full_move_vars: &FxHashSet<ArcVarId>,
    partial_move_vars: &FxHashMap<ArcVarId, Vec<u32>>,
) {
    for (block_idx, block) in func.blocks.iter_mut().enumerate() {
        let original = std::mem::take(&mut block.body);
        let terminator_idx = original.len();
        let mut new_body: Vec<ArcInstr> = Vec::with_capacity(original.len() * 2);
        for (instr_idx, instr) in original.into_iter().enumerate() {
            let ctx = BurdenEmitCtx {
                block_idx,
                instr_idx,
                owned_vars_needing_rc,
                last_uses_at,
                full_move_vars,
                partial_move_vars,
            };
            emit_instr_burdens(&mut new_body, instr, &ctx);
        }
        emit_terminator_burden_incs(&mut new_body, &terminator_inc_per_block[block_idx]);
        emit_terminator_burden_decs(
            &mut new_body,
            block_idx,
            terminator_idx,
            last_uses_at,
            &terminator_transfer_per_block[block_idx],
            full_move_vars,
            partial_move_vars,
        );
        block.body = new_body;
    }
}

/// §03.3 rule 3 + rule 5 emission-side: emit `BurdenInc` for each owned
/// terminator-position arg pre-computed by `compute_terminator_inc_per_block`.
/// Mirrors `emit_instr_burdens`'s instruction-level `BurdenInc` loop (line ~966)
/// — conservative Phase 5 emission at every transfer point per `aims-rules.md
/// §8 RL-1`; lattice rewrite in §05 eliminates redundant Incs.
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
/// position (`block_idx`/`instr_idx`) plus four loop-invariant analysis maps
/// (`owned_vars_needing_rc`, `last_uses_at`, `full_move_vars`,
/// `partial_move_vars`) consumed by `emit_instr_burdens` per `aims-rules.md
/// §8 RL-2`. Domain newtype per `impl-hygiene.md §PARAM_SPRAWL Cure hierarchy
/// item 3`.
struct BurdenEmitCtx<'a> {
    block_idx: usize,
    instr_idx: usize,
    owned_vars_needing_rc: &'a FxHashSet<ArcVarId>,
    last_uses_at: &'a FxHashMap<(usize, usize), Vec<ArcVarId>>,
    full_move_vars: &'a FxHashSet<ArcVarId>,
    partial_move_vars: &'a FxHashMap<ArcVarId, Vec<u32>>,
}

/// Emit `BurdenInc` ops before `instr`, push `instr` itself, then emit
/// `BurdenDec` ops at any last-use position for vars not consumed at an
/// owned position by this instruction. `Set` carve-outs (`value` is Owned
/// via IA-5 alias-transfer despite `is_owned_position`'s `_ => false`) are
/// applied symmetrically per `aims-rules.md §3 TF-15`.
fn emit_instr_burdens(new_body: &mut Vec<ArcInstr>, instr: ArcInstr, ctx: &BurdenEmitCtx<'_>) {
    for (pos, &arg) in instr.used_vars().iter().enumerate() {
        if instr.is_owned_position(pos) && ctx.owned_vars_needing_rc.contains(&arg) {
            new_body.push(ArcInstr::BurdenInc { var: arg });
        }
    }
    if let ArcInstr::Set { base, field, value } = &instr {
        // §03.4 cycle 47 — Set old-value drop emission per plan body line 1943
        // ("`BurdenDec(base.field.old_value)` BEFORE Set mutation"). Emit when
        // base carries any burden (owned_vars_needing_rc.contains(base)) — the
        // codegen layer at cycle 48 walks `Burden::owned_fields()` to filter
        // which field positions actually need a drop. Mirrors symmetric
        // BurdenInc(value) at the same site (cycle 12+24 below): BurdenInc
        // transfers ownership INTO the field, BurdenDecField releases prior
        // value OUT. Ordering invariant: BurdenDecField BEFORE BurdenInc(value)
        // BEFORE Set — old release precedes new acquire precedes mutation, so
        // codegen can read prior value via GEP+load BEFORE the store clobbers
        // it.
        if ctx.owned_vars_needing_rc.contains(base) {
            new_body.push(ArcInstr::BurdenDecField {
                base: *base,
                field: *field,
            });
        }
        if ctx.owned_vars_needing_rc.contains(value) {
            new_body.push(ArcInstr::BurdenInc { var: *value });
        }
    }
    if let ArcInstr::SetTag { base, .. } = &instr {
        // §03.4 cycle 50b — SetTag old-variant drop emission per
        // `aims-rules.md §3 TF-15a` + `§8 RL-10`. Whole-var pattern (NOT
        // field-positional): the tag change invalidates ALL payload
        // fields of the OLD variant. Emit BurdenDecVariant BEFORE the
        // SetTag so codegen at cycle 50c can GEP+load the current
        // discriminant + dispatch per-variant burden walk BEFORE the
        // store clobbers the tag. SetTag has no value operand (TF-15a
        // backward demand is `(base, Once)` only), so no symmetric
        // BurdenInc(value) — parallel to cycle 47 BurdenDecField's
        // role for Set, scoped to the whole variant per RL-10.
        if ctx.owned_vars_needing_rc.contains(base) {
            new_body.push(ArcInstr::BurdenDecVariant { var: *base });
        }
    }
    let transfer_vars = instr_transfer_vars(&instr);
    new_body.push(instr);
    if let Some(last_use_vars) = ctx.last_uses_at.get(&(ctx.block_idx, ctx.instr_idx)) {
        for &var in last_use_vars {
            // §03.4 cycle 46 three-way branch per `aims-rules.md §8 RL-2`:
            // (a) suppress entirely when var is ownership-transferred at this
            //     instr OR var's entire owned-field set was moved (full-move
            //     case from cycle 43);
            // (b) emit `BurdenDecPartial { var, skip_fields }` when some-but-
            //     not-all owned fields were moved via field-projection
            //     transfers (partial-move case from cycle 46; codegen at
            //     cycle 44c walks owned_fields minus skip_fields);
            // (c) emit standard `BurdenDec { var }` for the no-projection
            //     baseline (cycle 42 conservative case retained).
            if transfer_vars.contains(&var) || ctx.full_move_vars.contains(&var) {
                continue;
            }
            if let Some(skip_fields) = ctx.partial_move_vars.get(&var) {
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

/// Snapshot vars consumed at an owned position by `instr`, used to suppress
/// `BurdenDec` at transfer points per `aims-rules.md §8 RL-2`. `Set.value`
/// is added explicitly per `aims-rules.md §3 TF-15` (`is_owned_position`'s
/// `_ => false` catch-all excludes it).
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

/// §03.3 terminator-position emission. Per `aims-rules.md §8 RL-2`, Return
/// transfers ownership to caller — Return's `value` is a terminator-transfer
/// point. Vars whose terminator-position last-use is the transferred value
/// MUST NOT receive `BurdenDec`; owned locals whose terminator-position last-
/// use is NOT transferred get `BurdenDec` emitted immediately before the
/// terminator.
fn emit_terminator_burden_decs(
    new_body: &mut Vec<ArcInstr>,
    block_idx: usize,
    terminator_idx: usize,
    last_uses_at: &FxHashMap<(usize, usize), Vec<ArcVarId>>,
    terminator_transfer_vars: &FxHashSet<ArcVarId>,
    full_move_vars: &FxHashSet<ArcVarId>,
    partial_move_vars: &FxHashMap<ArcVarId, Vec<u32>>,
) {
    let Some(last_use_vars) = last_uses_at.get(&(block_idx, terminator_idx)) else {
        return;
    };
    for &var in last_use_vars {
        // §03.4 cycle 46 three-way branch — symmetric with `emit_instr_burdens`
        // per `aims-rules.md §8 RL-2` terminator + instruction equivalence:
        // (a) suppress on transfer OR full-move; (b) BurdenDecPartial for
        // partial-move; (c) standard BurdenDec for no-projection baseline.
        if terminator_transfer_vars.contains(&var) || full_move_vars.contains(&var) {
            continue;
        }
        if let Some(skip_fields) = partial_move_vars.get(&var) {
            new_body.push(ArcInstr::BurdenDecPartial {
                var,
                skip_fields: skip_fields.clone(),
            });
        } else {
            new_body.push(ArcInstr::BurdenDec { var });
        }
    }
}

#[cfg(test)]
mod tests;
