//! Per-instruction + per-terminator burden-op emission for the Phase 5 walk.
//!
//! Drives the single-forward-pass per-block emission: `BurdenInc` before owned
//! positions + FRESH-allocating definitions, `BurdenDec` / `BurdenDecPartial` /
//! `BurdenDecVariant` / `BurdenDecField` at last-use / transfer / mutation
//! sites per `Spec: Annex E §AIMS RL-1 / RL-2 / TF-15`. Consumes the
//! precomputed analysis maps via `BurdenAnalysisCtx`; emits no lattice
//! consultation of its own.

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::Uniqueness;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, LitValue};
use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use super::instr_transfer_vars;

/// Function-wide analysis results consumed by burden emission. Bundled into
/// one struct so per-instruction and per-terminator helpers share a single
/// reference — a domain newtype carrying the co-varying analysis fields at
/// their shared carrier.
pub(super) struct BurdenAnalysisCtx<'a> {
    pub(super) owned_vars_needing_rc: &'a FxHashSet<ArcVarId>,
    pub(super) last_uses_at: &'a FxHashMap<(usize, usize), Vec<ArcVarId>>,
    pub(super) full_move_vars: &'a FxHashSet<ArcVarId>,
    pub(super) partial_move_vars: &'a FxHashMap<ArcVarId, Vec<u32>>,
    // Vars whose paired BurdenDec is transfer-suppressed at their last-use
    // (transferred at last-use instr / terminator, or full-move). The
    // symmetric FRESH-site BurdenInc is suppressed for these in
    // emit_fresh_site_burden_inc to keep the per-value burden ledger balanced.
    pub(super) inc_suppressed_vars: &'a FxHashSet<ArcVarId>,
    // Let-Var alias dsts whose source stays live (duplication, RL-1). The burden
    // path emits the alias's own RcInc/RcDec pair: a `BurdenInc dst` at the alias
    // site (emit_fresh_site_burden_inc) balanced by a `BurdenDec dst` at the
    // alias's true last-use. The alias's release is the burden path's
    // responsibility, NOT the retiring predicate stack's.
    pub(super) dup_alias_dsts: &'a FxHashSet<ArcVarId>,
    // Move sources whose ownership transfers out through a Let-Var move-alias
    // chain to a terminator/owned-position transfer point (AIMS RL-2). Their
    // last-use BurdenDec is suppressed — the release is discharged at the
    // downstream transfer point, not at the move-alias site (else net -1).
    pub(super) transfer_via_move_alias: &'a FxHashSet<ArcVarId>,
    // Per-block live-out sets (owned-RC vars) per `Spec: Annex E §AIMS RL-4`.
    // A var live-out of block B has its in-block last-use `BurdenDec` suppressed
    // — the genuine release is the dead-out block's dec OR the predicate-stack
    // edge / dead-at-entry cleanup on the dying CFG edge, never an unconditional
    // dec in a block the value outlives.
    pub(super) live_out_per_block: &'a [FxHashSet<ArcVarId>],
    pub(super) contracts: &'a FxHashMap<Name, MemoryContract>,
}

/// Drive the unified single-forward-pass per-block emission. For each
/// instruction, `BurdenInc` is emitted BEFORE for every owned-position arg per
/// `ArcInstr::is_owned_position(pos)`; `BurdenDec` is emitted AFTER for each
/// last-use position EXCEPT when the instruction consumes the var at an owned
/// position (transfer point; ownership transferred per AIMS RL-2). `Set`/
/// `SetTag` carve-outs per AIMS TF-15 apply at both halves; `full_move_vars`
/// suppresses `BurdenDec` emission for vars whose entire owned-field set is
/// covered by `moved_out_fields`.
pub(super) fn emit_burden_ops_for_blocks(
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
        let terminator_borrowed_args = invoke_terminator_borrowed_args(&block.terminator);
        emit_terminator_burden_decs(
            &mut new_body,
            block_idx,
            terminator_idx,
            analysis,
            &terminator_borrowed_args,
            &terminator_transfer_per_block[block_idx],
            &inc_counts,
        );
        block.body = new_body;
    }
}

/// Vars used at a BORROWED (non-owned) arg position of an `Invoke` /
/// `InvokeIndirect` terminator. A borrowed Invoke arg is NOT consumed by the
/// callee — the value survives to the `normal` / `unwind` successors, where the
/// predicate-stack edge cleanup (`emit_rc::release_with_burden`) releases it and
/// co-emits the paired scope-exit `BurdenDec`. Emitting the burden-walk's own
/// terminator-last-use `BurdenDec` for such an arg double-counts the release
/// (VF-1 net=-1 per terminal path). Used by `emit_terminator_burden_decs` to
/// suppress the redundant terminator dec. Non-Invoke terminators (Return /
/// Jump / Branch) return empty — their owned transfers are handled by
/// `terminator_transfer_vars` and their non-arg last-uses are genuine
/// scope-exit releases the burden walk owns.
fn invoke_terminator_borrowed_args(term: &ArcTerminator) -> FxHashSet<ArcVarId> {
    let mut borrowed: FxHashSet<ArcVarId> = FxHashSet::default();
    if matches!(
        term,
        ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. }
    ) {
        for (pos, &var) in term.used_vars().iter().enumerate() {
            if !term.is_owned_position(pos) {
                borrowed.insert(var);
            }
        }
    }
    borrowed
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
    emit_owned_position_incs(new_body, &instr, ctx);
    emit_in_place_mutation_drops(new_body, &instr, ctx);
    let transfer_vars = instr_transfer_vars(&instr);
    new_body.push(instr);
    emit_last_use_decs(new_body, ctx, &transfer_vars);
}

/// Emit `BurdenInc` for every owned-position arg of `instr` whose last-use is
/// NOT this instruction. Skip owned-position `BurdenInc` when the arg's
/// last-use is THIS instruction: the matching `BurdenDec` would be
/// transfer-suppressed per AIMS RL-2, producing a `Σ Inc - Σ Dec = +1` VF-1
/// imbalance in `aims/verify/burden_balance.rs`. Suppressing both Inc + Dec
/// keeps the coexistence handshake clean: vars whose physical RC is owned by
/// the `aims/realize/walk.rs` predicate-stack stay OUT of `func.burden_emitted`,
/// preventing `populate_class_covered` from spuriously suppressing
/// predicate-stack RC. Burden* are no-op codegen markers; the predicate-stack
/// realize walk owns the real codegen RC for transferred-out vars.
fn emit_owned_position_incs(
    new_body: &mut Vec<ArcInstr>,
    instr: &ArcInstr,
    ctx: &BurdenEmitCtx<'_>,
) {
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
}

/// Emit the old-value drop ops that precede an in-place mutation (`Set` /
/// `SetTag`) per AIMS TF-15 / TF-15a + RL-10.
///
/// `Set`: emit `BurdenDecField(base.field)` BEFORE the `Set` mutation when
/// `base` carries any burden. The codegen layer walks `Burden::owned_fields()`
/// to filter which field positions actually need a drop. Mirrors the symmetric
/// `BurdenInc(value)` — `BurdenInc` transfers ownership INTO the field,
/// `BurdenDecField` releases the prior value OUT. Ordering invariant:
/// `BurdenDecField` BEFORE `BurdenInc(value)` BEFORE `Set` — old release
/// precedes new acquire precedes mutation, so codegen can read the prior value
/// via `GEP`+load BEFORE the store clobbers it.
///
/// `SetTag`: whole-var pattern (NOT field-positional) — the tag change
/// invalidates ALL payload fields of the OLD variant. Emit `BurdenDecVariant`
/// BEFORE the `SetTag` so codegen can `GEP`+load the current discriminant +
/// dispatch the per-variant burden walk BEFORE the store clobbers the tag.
/// `SetTag` has no value operand (TF-15a backward demand is `(base, Once)`
/// only), so no symmetric `BurdenInc(value)` — parallel to `Set`'s
/// `BurdenDecField`, scoped to the whole variant per RL-10.
fn emit_in_place_mutation_drops(
    new_body: &mut Vec<ArcInstr>,
    instr: &ArcInstr,
    ctx: &BurdenEmitCtx<'_>,
) {
    if let ArcInstr::Set { base, field, value } = instr {
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
    if let ArcInstr::SetTag { base, .. } = instr {
        if ctx.analysis.owned_vars_needing_rc.contains(base) {
            new_body.push(ArcInstr::BurdenDecVariant { var: *base });
        }
    }
}

/// Emit the last-use `BurdenDec` / `BurdenDecPartial` for vars whose last-use
/// is at this instruction position, via the three-way branch per AIMS RL-2:
/// (a) suppress entirely when var is ownership-transferred at this instr OR
///     var's entire owned-field set was moved (full-move) OR var's ownership
///     transfers out through a move-alias chain (`transfer_via_move_alias`);
///     a duplication Let-Var alias (`dup_alias_dsts`) is NOT suppressed — its
///     RL-1 alias-site `BurdenInc` (`emit_fresh_site_burden_inc`) is balanced by
///     the last-use `BurdenDec` emitted here;
/// (b) emit `BurdenDecPartial { var, skip_fields }` when some-but-not-all owned
///     fields were moved via field-projection transfers (partial-move; codegen
///     walks `owned_fields` minus `skip_fields`);
/// (c) emit standard `BurdenDec { var }` for the no-projection conservative
///     baseline.
///
/// Instruction-level transfer suppression is preserved per the coexistence
/// handshake. The owned-position `BurdenInc` deposited by `emit_instr_burdens`
/// is a VF-1 accounting marker, NOT a real `RcInc`; codegen's predicate-stack
/// realize walk (consulting `class_covered`) owns the physical RC management for
/// vars consumed at instruction-level owned positions (`Apply`/`PartialApply`/
/// `Construct`/etc.). Adding a symmetric `BurdenDec` here would mark the var in
/// `func.burden_emitted`, propagate through `populate_class_covered`, and
/// suppress predicate-stack RC emission — causing real-world RC leaks observed
/// in `match_alias::test_closure_*` AOT tests. For VF-1 balance, the legacy
/// owned-position Inc/transfer-Dec pattern is rebalanced separately by
/// `emit_terminator_burden_decs` and by `eliminate_burden_ops` paired elision.
fn emit_last_use_decs(
    new_body: &mut Vec<ArcInstr>,
    ctx: &BurdenEmitCtx<'_>,
    transfer_vars: &FxHashSet<ArcVarId>,
) {
    let Some(last_use_vars) = ctx
        .analysis
        .last_uses_at
        .get(&(ctx.block_idx, ctx.instr_idx))
    else {
        return;
    };
    for &var in last_use_vars {
        if transfer_vars.contains(&var)
            || ctx.analysis.full_move_vars.contains(&var)
            || ctx.analysis.transfer_via_move_alias.contains(&var)
        {
            continue;
        }
        // RL-4: a per-block last-use is a genuine release only when the var is
        // dead at block exit. Live-out vars are released on the dying CFG edge
        // (predicate-stack edge cleanup) or at the dead-out block — not here.
        if ctx
            .analysis
            .live_out_per_block
            .get(ctx.block_idx)
            .is_some_and(|s| s.contains(&var))
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

/// Emit FRESH-site `BurdenInc dst` for instructions that define a
/// freshly-allocated owned value per AIMS TF-3 / TF-5 / TF-6 / TF-7 / TF-9 /
/// TF-9a, AND the duplication-alias `BurdenInc dst` for a `Let { Var(src) }`
/// whose `src` stays live (RL-1: a value duplicated to a still-live source is a
/// genuine new reference). Symmetric with the scope-exit `BurdenDec` at
/// last-use; both gated on `owned_vars_needing_rc.contains(&dst)` per the
/// coexistence handshake (scalars excluded by the `burden_carries_rc` filter in
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
/// Duplication-alias definition site (RL-1):
///   (g) `Let { Var(src) }` where `dst ∈ dup_alias_dsts` (`src` use-count ≥ 2,
///       so `src` stays live past the alias). The alias is a genuine
///       duplication of `src`'s allocation; the new reference owes a paired
///       `BurdenInc dst` here and `BurdenDec dst` at `dst`'s true last-use.
///       A MOVE-alias (`src` used exactly once) is NOT in `dup_alias_dsts` and
///       emits no inc — the existing `src` FRESH-site inc covers the lineage.
///
/// Other definitions (TF-1 scalar Literal, TF-2 MOVE-alias Var, TF-2a `PrimOp`,
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
            value: ArcValue::Var(_),
            ..
        } if ctx.analysis.dup_alias_dsts.contains(dst) => *dst,
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
    terminator_borrowed_args: &FxHashSet<ArcVarId>,
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
    // Vars whose terminator-position last-use is NOT a transfer point follow the
    // last-use emission path: emit one BurdenDec per last-use entry unless
    // full_move OR move-alias-transfer suppresses. A duplication Let-Var alias
    // (`dup_alias_dsts`) is NOT suppressed: its RL-1 alias-site `BurdenInc`
    // (emit_fresh_site_burden_inc) is balanced by this last-use `BurdenDec`
    // (net 0). A MOVE-alias source transferring out is in `full_move_vars` or
    // `transfer_via_move_alias` and stays suppressed.
    if let Some(last_use_vars) = analysis.last_uses_at.get(&(block_idx, terminator_idx)) {
        for &var in last_use_vars {
            if terminator_transfer_vars.contains(&var) {
                continue;
            }
            if analysis.full_move_vars.contains(&var) {
                continue;
            }
            if analysis.transfer_via_move_alias.contains(&var) {
                continue;
            }
            // RL-4: a terminator-position last-use whose var is live-out of the
            // block is not a genuine release — the value flows on to a
            // successor (the dec belongs on the dying edge / dead-out block).
            if analysis
                .live_out_per_block
                .get(block_idx)
                .is_some_and(|s| s.contains(&var))
            {
                continue;
            }
            // Borrowed Invoke arg: the value survives the borrowed call and is
            // released at the successor by the predicate-stack edge cleanup,
            // which co-emits the paired scope-exit BurdenDec. A terminator dec
            // here too double-counts (VF-1 net=-1 per terminal path).
            if terminator_borrowed_args.contains(&var) {
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
