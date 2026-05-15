//! Phase 5 trivial burden emission walker.
//!
//! Reads each owned non-scalar SSA value's `BurdenSpec` and emits `BurdenInc`
//! at every transfer point + `BurdenDec` at every last-use along every
//! reachable CFG path. Pure per-instruction emission driven by SSA def-use;
//! no global flow analysis, no fixpoint, no lattice consultation.
//!
//! Subsequent cycles author the actual transfer-point detection, last-use
//! detection, and `BurdenInc` / `BurdenDec` emission.

use crate::ir::{ArcFunction, ArcInstr, ArcVarId};
use crate::ownership::Ownership;
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
) -> BurdenLowerCtx<'a> {
    let mut ctx = BurdenLowerCtx::default();
    // Build ArcVarId -> Ownership map from func.params. Locals (vars not in
    // params) lack ArcParam.ownership; cycle 5+ wires DerivedOwnership for
    // per-local ownership filtering. Until then, locals are NOT filtered
    // (collected unconditionally) — params with Borrowed ownership ARE
    // skipped per §03.2 checkbox 1 "For each owned ArcVarId v".
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
    // Transfer-point detection — generic walk via canonical SSOT helpers
    // `ArcInstr::used_vars()` + `ArcInstr::is_owned_position(pos)` per
    // `instr.rs:330-393` + `ir/mod.rs::used_vars`. Mechanically covers
    // Construct + PartialApply + CollectionReuse (positions 1..=args.len)
    // + ApplyIndirect (positions 1..= for Owned args) + Apply (positions
    // 0..args.len with arg_ownership filter) via the canonical helper —
    // single source of truth per `impl-hygiene.md §SSOT`. Set / SetTag use
    // the IA-5 alias-transfer model (NOT covered by `is_owned_position`'s
    // `_ => false` catch-all per `aims-rules.md §3 TF-15`); those land in
    // cycles dedicated to Set/SetTag handling per §03.2 plan body. Terminator
    // transfer points (Invoke / InvokeIndirect / Jump-to-Owned-param) require
    // `ArcTerminator::is_owned_position` walks landed in §03.3.
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
            // TF-15 carve-out: ArcInstr::Set has Owned `value` via IA-5
            // step (1) alias-transfer (`value.access := Owned` unconditional)
            // — NOT covered by `is_owned_position` per its `_ => false`
            // catch-all. `base` gets direct demand only (consumed but not
            // an ownership-transfer point); only `value` is the transfer
            // point per `aims-rules.md §3 TF-15`.
            if let ArcInstr::Set { value, .. } = instr {
                let value_idx = func.var_types[value.index()];
                let ty: TypeRef = idx_to_type_ref(value_idx, type_registry);
                let burden = lookup_burden(ty, type_registry);
                ctx.transfer_points.push((*value, burden));
            }
        }
    }
    // Last-use detection — per-block backward walk per §03.2 success_criterion
    // 2 ("BurdenDec(v) emits immediately following EVERY last-use of v along
    // EVERY reachable CFG path"). Per-block linear scan satisfies the §03.2
    // goal ban on global flow analysis / fixpoint / lattice consultation;
    // cross-block liveness via block-param handoffs lands in §03.3 terminator
    // burden-op ordering. Walk each block backward over instructions; first
    // sighting of an `ArcVarId` (in reverse order) is its last use in the
    // block.
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let mut seen: FxHashSet<ArcVarId> = FxHashSet::default();
        for (instr_idx, instr) in block.body.iter().enumerate().rev() {
            for &arg in &instr.used_vars() {
                if seen.insert(arg) {
                    ctx.last_use_points.push((arg, block_idx, instr_idx));
                }
            }
        }
    }
    // BurdenInc + BurdenDec instruction emission — unified single forward
    // pass per block (avoids index-shift fragility). For each instruction:
    // - BurdenInc emitted BEFORE for every owned-position arg per
    //   `ArcInstr::is_owned_position(pos)` SSOT helper (§03.2 sc 1).
    // - BurdenDec emitted AFTER for each last-use position EXCEPT when the
    //   instruction consumes the var at an owned position (transfer point;
    //   ownership transferred per `aims-rules.md §8 RL-2` — emitting
    //   BurdenDec at the transfer would double-release).
    // - BurdenDec is filtered to vars in `ctx.collected` carrying a
    //   non-empty `BurdenRef` (excludes scalars per `aims-rules.md §4 DP-1`
    //   `is_rc_needed: Owned ∧ ¬Dead ∧ ¬is_scalar` + `§9 VF-1 RcOnScalar`).
    //   Note: `lookup_burden(Idx::INT, ...)` returns `Some(BurdenRef)`
    //   carrying `BuiltinBurdenSpec::EMPTY` (per `BURDEN_TABLE` at
    //   `ori_registry/src/burden/table.rs:184-193`), so the filter MUST
    //   reject EMPTY specs via `burden_carries_rc` helper vs naively
    //   admitting any `Some(_)`.
    // Set/SetTag remain TF-15 carve-outs deferred to subsequent cycles.
    let owned_vars_needing_rc: FxHashSet<ArcVarId> = ctx
        .collected
        .iter()
        .filter_map(|(var, burden)| {
            burden
                .as_ref()
                .filter(|b| burden_carries_rc(b))
                .map(|_| *var)
        })
        .collect();
    let mut last_uses_at: FxHashMap<(usize, usize), Vec<ArcVarId>> = FxHashMap::default();
    for &(var, b, i) in &ctx.last_use_points {
        if !owned_vars_needing_rc.contains(&var) {
            continue;
        }
        last_uses_at.entry((b, i)).or_default().push(var);
    }
    for (block_idx, block) in func.blocks.iter_mut().enumerate() {
        let original = std::mem::take(&mut block.body);
        let mut new_body: Vec<ArcInstr> = Vec::with_capacity(original.len() * 2);
        for (instr_idx, instr) in original.into_iter().enumerate() {
            // BurdenInc BEFORE the instruction. Gated by `owned_vars_needing_rc`
            // per cycle-24 VF-1 RcOnScalar mirror — burdens with EMPTY spec
            // (scalars) MUST NOT receive BurdenInc, symmetric to cycle-21's
            // BurdenDec filter. Per `aims-rules.md §9 VF-1` + IR variant doc
            // (`instr.rs` BurdenInc: "parallel to RcInc but tracks burden lattice").
            for (pos, &arg) in instr.used_vars().iter().enumerate() {
                if instr.is_owned_position(pos) && owned_vars_needing_rc.contains(&arg) {
                    new_body.push(ArcInstr::BurdenInc { var: arg });
                }
            }
            // TF-15 carve-out: ArcInstr::Set's `value` is owned via IA-5
            // alias-transfer (NOT covered by `is_owned_position`'s
            // `_ => false` catch-all). Symmetric BurdenInc emission — also
            // gated by owned_vars_needing_rc per cycle-24 VF-1 mirror.
            if let ArcInstr::Set { value, .. } = &instr {
                if owned_vars_needing_rc.contains(value) {
                    new_body.push(ArcInstr::BurdenInc { var: *value });
                }
            }
            // Snapshot transfer vars (consumed at owned position by THIS
            // instr) before moving — used to skip BurdenDec at transfers
            // per `aims-rules.md §8 RL-2` ownership-transferring exception.
            let mut transfer_vars: FxHashSet<ArcVarId> = instr
                .used_vars()
                .iter()
                .enumerate()
                .filter_map(|(pos, &arg)| {
                    if instr.is_owned_position(pos) {
                        Some(arg)
                    } else {
                        None
                    }
                })
                .collect();
            // TF-15 carve-out symmetric: Set's `value` is a transfer point
            // too. Without this, BurdenDec at Set value's last use would
            // double-release per RL-2.
            if let ArcInstr::Set { value, .. } = &instr {
                transfer_vars.insert(*value);
            }
            new_body.push(instr);
            // BurdenDec AFTER, filtered against transfer vars per RL-2.
            if let Some(last_use_vars) = last_uses_at.get(&(block_idx, instr_idx)) {
                for &var in last_use_vars {
                    if !transfer_vars.contains(&var) {
                        new_body.push(ArcInstr::BurdenDec { var });
                    }
                }
            }
        }
        block.body = new_body;
    }
    ctx
}

#[cfg(test)]
mod tests;
