//! Ownership / transfer scans feeding `emit_burden_ops`: owned-burden
//! collection, transfer-point and last-use detection, move-alias chains,
//! use counts, and the live-out / gen-kill dataflow inputs.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId, PrimOp, ValueRepr};
use crate::ownership::Ownership;
use ori_types::TypeRegistry;

use super::super::burden::TypeRef;
use super::super::burden_lookup::{idx_to_type_ref, lookup_burden};
use super::burden_carries_rc;
use super::BurdenLowerCtx;

/// Phase 1 — per-`ArcVarId` ownership-filtered burden lookup walk.
///
/// Build `ArcVarId -> Ownership` map from `func.params`. Locals (vars not in
/// params) lack `ArcParam.ownership` and are collected unconditionally; params
/// with `Borrowed` ownership are skipped (only owned `ArcVarId`s carry RC).
pub(super) fn collect_owned_burdens<'a>(
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
pub(super) fn detect_transfer_points<'a>(
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
pub(super) fn detect_last_uses(ctx: &mut BurdenLowerCtx<'_>, func: &ArcFunction) {
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
pub(super) fn compute_owned_vars_needing_rc(ctx: &BurdenLowerCtx<'_>) -> FxHashSet<ArcVarId> {
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
pub(super) fn group_last_uses_filtered(
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
pub(super) fn compute_transfer_via_move_alias(
    func: &ArcFunction,
    terminator_transfer_per_block: &[FxHashSet<ArcVarId>],
    use_counts: &FxHashMap<ArcVarId, u32>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let mut transferred: FxHashSet<ArcVarId> = FxHashSet::default();
    // Seed: terminator-transferred vars.
    for set in terminator_transfer_per_block {
        transferred.extend(set.iter().copied());
    }
    // Seed: instruction owned-position transfers (Construct/Apply/Set/etc.).
    for block in &func.blocks {
        for instr in &block.body {
            transferred.extend(instr_transfer_vars(instr, func).iter().copied());
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
    // Seed: a move-alias source `%s` (`%d = %s`, `%s` used once) whose dst `%d`
    // is owned-RC has its SINGLE release discharged BY `%d` — either `%d`
    // transfers out (seeded above) OR `%d` gets its own last-use dec. The move
    // hands `%s`'s one allocation to `%d`; emitting `%s`'s own last-use dec
    // would double-release the shared buffer, and emitting `%s`'s FRESH-site inc
    // would orphan (the move is not a duplication — `%d` does NOT get a paired
    // inc, so the lineage carries exactly one inc+dec at the `%d` end). Both
    // halves suppressed via this set (dec here, inc via `inc_suppressed_vars`),
    // matching the transfer-out case. Witness: `coll_list_cow_concat_shared`
    // `%14 = %8` (fresh concat result moved to a borrow-used alias) — `%8`'s
    // scope-exit dec double-frees the buffer `%14` decs. Per AIMS RL-2
    // (move = ownership transfer, single release at the lineage's terminal owner).
    for &(dst, src) in &move_edges {
        if owned_vars_needing_rc.contains(&dst) {
            transferred.insert(src);
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
pub(super) fn compute_borrowed_terminator_invoke_args(func: &ArcFunction) -> FxHashSet<ArcVarId> {
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
///   (`use_counts` ≥ 2) — a duplication alias (RL-1). The burden path emits the
///   alias's own paired `BurdenInc dst` (alias site) + `BurdenDec dst`
///   (last-use); the alias owns its release, not the predicate stack.
pub(super) fn compute_use_counts_and_dup_aliases(
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
pub(super) fn compute_live_out_owned(
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
pub(super) fn compute_owned_gen_kill(
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
/// excludes it). A list-concat `PrimOp Binary(Add)` `RcPointer` operand is added
/// per the dual-consuming `ori_list_concat_cow` runtime contract (the helper
/// dec/frees BOTH input buffers;
/// `is_owned_position`'s `_ => false` excludes `Let { PrimOp }`). Shared by the
/// driver, `moved_fields`, and `emit` submodules.
pub(super) fn instr_transfer_vars(instr: &ArcInstr, func: &ArcFunction) -> FxHashSet<ArcVarId> {
    let mut transfer_vars = instr_owned_position_transfer_vars(instr);
    transfer_vars.extend(list_concat_consumed_operands(instr, func));
    transfer_vars
}

/// The `func`-free subset of `instr_transfer_vars`: vars consumed at an
/// `is_owned_position` arg plus the `Set.value` TF-15 carve-out. Used at the
/// `&mut`-walk emission site where `func` cannot be re-borrowed for `var_repr`;
/// the list-concat consume set is consulted there via the precomputed
/// `BurdenAnalysisCtx::list_concat_transfer_vars` instead.
pub(super) fn instr_owned_position_transfer_vars(instr: &ArcInstr) -> FxHashSet<ArcVarId> {
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

/// Operands consumed by the dual-consuming `ori_list_concat_cow` runtime helper:
/// the `RcPointer` operands of a `Let { PrimOp Binary(Add) }`. SSOT for the
/// list-concat consume set — consulted by `instr_transfer_vars` (last-use dec
/// suppression) AND precomputed function-wide in `emit_burden_ops_for_blocks`
/// (the `&mut` walk cannot re-borrow `func` for `var_repr`).
///
/// `RcPointer` is the list discriminator: `str`/closure operands are `FatValue`
/// and BORROWED by `ori_str_concat`'s `*const OriStr` contract (not consumed);
/// scalar `Add` operands carry no RC (filtered by `owned_vars_needing_rc`).
/// User `Add` impls dispatch via trait `Apply`/`Invoke`, never `PrimOp Binary`.
/// List + list → COW concat: both operands are consumed by the runtime.
pub(crate) fn list_concat_consumed_operands(
    instr: &ArcInstr,
    func: &ArcFunction,
) -> FxHashSet<ArcVarId> {
    let mut consumed = FxHashSet::default();
    if let ArcInstr::Let {
        value: ArcValue::PrimOp { op, args },
        ..
    } = instr
    {
        if matches!(op, PrimOp::Binary(ori_ir::BinaryOp::Add)) {
            for &arg in args {
                if matches!(func.var_repr(arg), Some(ValueRepr::RcPointer)) {
                    consumed.insert(arg);
                }
            }
        }
    }
    consumed
}
