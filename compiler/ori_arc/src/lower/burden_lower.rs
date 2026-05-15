//! Phase 5 trivial burden emission walker.
//!
//! Reads each owned non-scalar SSA value's `BurdenSpec` and emits `BurdenInc`
//! at every transfer point + `BurdenDec` at every last-use along every
//! reachable CFG path. Pure per-instruction emission driven by SSA def-use;
//! no global flow analysis, no fixpoint, no lattice consultation.
//!
//! Subsequent cycles author the actual transfer-point detection, last-use
//! detection, and `BurdenInc` / `BurdenDec` emission.

use crate::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use crate::ownership::{DerivedOwnership, Ownership};
use ori_types::TypeRegistry;
use rustc_hash::{FxHashMap, FxHashSet};

use super::burden::{Burden, BurdenRef, TypeRef};
use super::burden_lookup::{idx_to_type_ref, lookup_burden};

/// True iff `burden` carries any RC-tracked dimension. Used by the filter at
/// `emit_burden_ops` to exclude scalars whose `lookup_burden` returns
/// `Some(BurdenRef)` wrapping `BuiltinBurdenSpec::EMPTY` (per `BURDEN_TABLE`
/// at `ori_registry/src/burden/table.rs:184-193`). Defends `VF-1 RcOnScalar`
/// per `aims-rules.md §9`.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "wired into AIMS pipeline by §03.N pipeline-wiring migration; until then only test-callers exist"
    )
)]
fn burden_carries_rc(burden: &BurdenRef<'_>) -> bool {
    burden.self_heap_alloc()
        || burden.element_burden().is_some()
        || burden.variant_burdens().next().is_some()
        || burden.owned_fields().next().is_some()
}

/// Per-cycle context accumulated by the emission walker.
///
/// Two storage axes (kept separate per cycle 5 navigator note — per-var and
/// per-instruction transfer-point lookups have distinct semantics):
/// - `collected` — per-`ArcVarId` `(var, BurdenSpec lookup)` from `var_types`
///   walk (cycle 2-4 axis). Filtered by `ArcParam.ownership` for params.
/// - `transfer_points` — per-instruction `(consumed var, BurdenSpec lookup)`
///   for transfer points where ownership transfers (`Construct` with owned
///   arg per cycle 5; `Apply` / `Set` / etc. in subsequent cycles per §03.2
///   `success_criterion` enumeration).
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "wired into AIMS pipeline by §03.N pipeline-wiring migration; until then only test-callers exist"
    )
)]
#[derive(Debug, Default)]
pub(crate) struct BurdenLowerCtx<'a> {
    collected: Vec<(ArcVarId, Option<BurdenRef<'a>>)>,
    transfer_points: Vec<(ArcVarId, Option<BurdenRef<'a>>)>,
    last_use_points: Vec<(ArcVarId, usize, usize)>,
    /// §03.4 Mikado-leaf: per-aggregate-var moved-field bitset. Populated
    /// per proposal §Non-Drop Partial-Move two-stage rule: bit set when
    /// `let f = v.field` (Project) AND `f` is THEN consumed at a transfer
    /// point — NOT on every `Project` (Project produces Borrowed per
    /// `aims-rules.md §3 TF-4` and is not itself an ownership-transfer
    /// site per `instr.rs:391 _ => false`). Population logic lands in a
    /// sibling cycle gated on transfer-point consumption of the projection
    /// destination; this cycle (40) introduces the empty data structure
    /// and accessor only, deferring semantics per Mikado-leaf discipline.
    ///
    /// `FieldId` is `u32` per `ArcInstr::Project.field` at `instr.rs:76-81`.
    /// CFG-join semantics (per-predecessor lookup) deferred per §03.4
    /// framing line 1641.
    moved_out_fields: FxHashMap<ArcVarId, FxHashSet<u32>>,
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "wired into AIMS pipeline by §03.N pipeline-wiring migration; until then only test-callers exist"
    )
)]
impl<'a> BurdenLowerCtx<'a> {
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

    /// Read-only access to §03.4 moved-field bitset map. Empty by default
    /// (cycle 40 skeleton); population logic lands in a sibling cycle.
    pub(crate) fn moved_out_fields(&self) -> &FxHashMap<ArcVarId, FxHashSet<u32>> {
        &self.moved_out_fields
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
    allow(
        dead_code,
        reason = "wired into AIMS pipeline by §03.N pipeline-wiring migration; until then only test-callers exist"
    )
)]
pub(crate) fn emit_burden_ops<'a>(
    func: &mut ArcFunction,
    type_registry: &'a TypeRegistry,
    // Mikado-leaf prerequisite for §03.3 rule 3 (Jump-to-Owned-param): block-
    // param ownership lookup per `aims-rules.md §invariant 5 case (c)` requires
    // DerivedOwnership side-table threaded as typed pre-pass input. Slice
    // indexed by `ArcVarId::raw()` matches `infer_derived_ownership()` return
    // shape per `compiler_repo/compiler/ori_arc/src/borrow/derived.rs:31-36`.
    // Empty `&[]` semantically safe — out-of-bounds defaults to `Owned` per
    // `borrow/derived.rs:60`. AIMS Invariant 5 (unified model) preserved per
    // `canon.md §7.1` — DerivedOwnership is existing analysis output, not a
    // parallel ownership tracker.
    derived_ownership: &[DerivedOwnership],
) -> BurdenLowerCtx<'a> {
    let mut ctx = BurdenLowerCtx::default();
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
        &ctx.moved_out_fields,
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
        &ctx.moved_out_fields,
        &full_move_vars,
        &owned_vars_needing_rc,
    );

    emit_burden_ops_for_blocks(
        func,
        &owned_vars_needing_rc,
        &last_uses_at,
        &terminator_transfer_per_block,
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
    allow(
        dead_code,
        reason = "wired into AIMS pipeline by §03.N pipeline-wiring migration; until then only test-callers exist"
    )
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
    allow(
        dead_code,
        reason = "wired into AIMS pipeline by §03.N pipeline-wiring migration; until then only test-callers exist"
    )
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
    allow(
        dead_code,
        reason = "wired into AIMS pipeline by §03.N pipeline-wiring migration; until then only test-callers exist"
    )
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
    allow(
        dead_code,
        reason = "wired into AIMS pipeline by §03.N pipeline-wiring migration; until then only test-callers exist"
    )
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
    allow(
        dead_code,
        reason = "wired into AIMS pipeline by §03.N pipeline-wiring migration; until then only test-callers exist"
    )
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
    allow(
        dead_code,
        reason = "wired into AIMS pipeline by §03.N pipeline-wiring migration; until then only test-callers exist"
    )
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

/// §03.4 cycle 42 — populate `ctx.moved_out_fields` per proposal §Non-Drop
/// Partial-Move two-stage rule. Two-pass linear scan; per §03.4 framing,
/// BOUNDED structural bookkeeping (no fixpoint, no lattice consultation).
///
/// **Pass 1**: walk every block's body; record every `ArcInstr::Project { dst,
/// value, field, .. }` as a `dst → (value, field)` entry in a local map.
///
/// **Pass 2**: walk every block's body + terminator; for each transferred var
/// (per `instr_transfer_vars` which honors `is_owned_position` + the Set-value
/// carve-out per `aims-rules.md §3 TF-15` + IA-5 step (1), and per the
/// precomputed `terminator_transfer_per_block` set), if the transferred var
/// matches a `project_dst`, insert `(project_src, field)` into
/// `moved_out_fields[project_src]`.
///
/// Project ALONE does NOT set the bit (per `aims-rules.md §3 TF-4` — Project
/// produces `Borrowed`; `is_owned_position`'s `_ => false` excludes it). Project
/// consumed at a borrowed position (e.g., `IsShared`) also leaves the bit
/// unset — `IsShared` falls through `_ => false` in `is_owned_position` and
/// has no Set-value-style carve-out.
///
/// CFG-join semantics (per-predecessor lookup) deferred per §03.4 framing line 1641.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "wired into AIMS pipeline by §03.N pipeline-wiring migration; until then only test-callers exist"
    )
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
    // Return/Jump-to-Owned-param/Invoke-Owned/InvokeIndirect-Owned per
    // `aims-rules.md §8 RL-2`.
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for instr in &block.body {
            let transfer_vars = instr_transfer_vars(instr);
            for var in &transfer_vars {
                if let Some(&(src, field)) = project_origins.get(var) {
                    ctx.moved_out_fields.entry(src).or_default().insert(field);
                }
            }
        }
        if let Some(term_transfers) = terminator_transfer_per_block.get(block_idx) {
            for var in term_transfers {
                if let Some(&(src, field)) = project_origins.get(var) {
                    ctx.moved_out_fields.entry(src).or_default().insert(field);
                }
            }
        }
    }
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
    allow(
        dead_code,
        reason = "wired into AIMS pipeline by §03.N pipeline-wiring migration; until then only test-callers exist"
    )
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
    allow(
        dead_code,
        reason = "wired into AIMS pipeline by §03.N pipeline-wiring migration; until then only test-callers exist"
    )
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
    allow(
        dead_code,
        reason = "wired into AIMS pipeline by §03.N pipeline-wiring migration; until then only test-callers exist"
    )
)]
fn emit_burden_ops_for_blocks(
    func: &mut ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    last_uses_at: &FxHashMap<(usize, usize), Vec<ArcVarId>>,
    terminator_transfer_per_block: &[FxHashSet<ArcVarId>],
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
