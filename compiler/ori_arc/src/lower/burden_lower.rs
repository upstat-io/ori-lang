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

    emit_burden_ops_for_blocks(
        func,
        &owned_vars_needing_rc,
        &last_uses_at,
        &terminator_transfer_per_block,
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

/// Drive the unified single-forward-pass per-block emission. For each instruction:
/// - `BurdenInc` emitted BEFORE for every owned-position arg per
///   `ArcInstr::is_owned_position(pos)` SSOT helper (§03.2 sc 1).
/// - `BurdenDec` emitted AFTER for each last-use position EXCEPT when the
///   instruction consumes the var at an owned position (transfer point;
///   ownership transferred per `aims-rules.md §8 RL-2`).
///
/// `Set`/`SetTag` carve-outs per `aims-rules.md §3 TF-15` apply at both halves.
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
) {
    for (block_idx, block) in func.blocks.iter_mut().enumerate() {
        let original = std::mem::take(&mut block.body);
        let terminator_idx = original.len();
        let mut new_body: Vec<ArcInstr> = Vec::with_capacity(original.len() * 2);
        for (instr_idx, instr) in original.into_iter().enumerate() {
            emit_instr_burdens(
                &mut new_body,
                instr,
                block_idx,
                instr_idx,
                owned_vars_needing_rc,
                last_uses_at,
            );
        }
        emit_terminator_burden_decs(
            &mut new_body,
            block_idx,
            terminator_idx,
            last_uses_at,
            &terminator_transfer_per_block[block_idx],
        );
        block.body = new_body;
    }
}

/// Emit `BurdenInc` ops before `instr`, push `instr` itself, then emit
/// `BurdenDec` ops at any last-use position for vars not consumed at an
/// owned position by this instruction. `Set` carve-outs (`value` is Owned
/// via IA-5 alias-transfer despite `is_owned_position`'s `_ => false`) are
/// applied symmetrically per `aims-rules.md §3 TF-15`.
fn emit_instr_burdens(
    new_body: &mut Vec<ArcInstr>,
    instr: ArcInstr,
    block_idx: usize,
    instr_idx: usize,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    last_uses_at: &FxHashMap<(usize, usize), Vec<ArcVarId>>,
) {
    for (pos, &arg) in instr.used_vars().iter().enumerate() {
        if instr.is_owned_position(pos) && owned_vars_needing_rc.contains(&arg) {
            new_body.push(ArcInstr::BurdenInc { var: arg });
        }
    }
    if let ArcInstr::Set { value, .. } = &instr {
        if owned_vars_needing_rc.contains(value) {
            new_body.push(ArcInstr::BurdenInc { var: *value });
        }
    }
    let transfer_vars = instr_transfer_vars(&instr);
    new_body.push(instr);
    if let Some(last_use_vars) = last_uses_at.get(&(block_idx, instr_idx)) {
        for &var in last_use_vars {
            if !transfer_vars.contains(&var) {
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
) {
    let Some(last_use_vars) = last_uses_at.get(&(block_idx, terminator_idx)) else {
        return;
    };
    for &var in last_use_vars {
        if !terminator_transfer_vars.contains(&var) {
            new_body.push(ArcInstr::BurdenDec { var });
        }
    }
}

#[cfg(test)]
mod tests;
