//! Borrowed-position scans: borrowed terminator-`Invoke` args, borrowed-arg
//! `Let` aliases, borrowed projection dsts, and the RL-1 borrowed-store
//! duplication incs. Spec: Annex E §AIMS RL-1 + RL-2 + TF-4.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_types::TypeRegistry;

use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId};
use crate::lower::burden::TypeRef;
use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

use crate::ownership::Ownership;

use super::super::burden_carries_rc;
use super::instr_transfer_vars;

/// Vars consumed at a BORROWED arg position of any `Invoke` / `InvokeIndirect`
/// terminator. Mirrors the per-block `invoke_terminator_borrowed_args` in
/// `emit.rs` (which gates the terminator-last-use `BurdenDec` suppression) but
/// computed function-wide for the FRESH-site `BurdenInc` suppression. A FRESH
/// value passed at a borrowed Invoke arg position has its terminator dec
/// suppressed there; suppressing its FRESH inc here keeps the per-value burden
/// ledger empty so the predicate-stack edge cleanup fully owns its release.
pub(in crate::lower::burden_lower) fn compute_borrowed_terminator_invoke_args(
    func: &ArcFunction,
) -> FxHashSet<ArcVarId> {
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

/// `Let { Var(src) }` aliases whose SOLE use is a BORROWED terminator-Invoke arg
/// position. Such an alias is a borrow-view of `src`, NOT a new owned reference:
/// per RL-1 (`08-realization/RL-1.proof` + `AimsProof.Realization`
/// `RL1_emit_iff_not_elidable`) a duplication inc is emitted ONLY when a value is
/// passed to an OWNED param while still live, so `f(x, x)` over two Borrowed
/// params creates no reference at either arg — the owned SOURCE `x` carries the
/// sole inc + release. Without excluding these aliases the dup-alias FRESH-site
/// `BurdenInc` + borrowed-arg scope-exit `BurdenDec` pair on each alias leaves the
/// source's FRESH inc orphaned (VF-1 net=+1 leak). Excluded from
/// `owned_vars_needing_rc` (neither inc nor dec) — a borrowed alias gets
/// neither. Use-count gated to 1 so a value also
/// used at an OWNED position keeps its burden ops.
pub(in crate::lower::burden_lower) fn compute_borrowed_arg_let_aliases(
    func: &ArcFunction,
) -> FxHashSet<ArcVarId> {
    let borrowed_args = compute_borrowed_terminator_invoke_args(func);
    if borrowed_args.is_empty() {
        return FxHashSet::default();
    }
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
    let mut out: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                // dst is the sole-use borrow-view; src must STAY LIVE (used >= 2,
                // a genuine duplication) so the OWNED source carries the
                // release. A move source (src used once) makes the alias the
                // sole carrier — excluding it would drop the release (leak).
                if borrowed_args.contains(dst)
                    && use_counts.get(dst).copied().unwrap_or(0) == 1
                    && use_counts.get(src).copied().unwrap_or(0) >= 2
                {
                    out.insert(*dst);
                }
            }
        }
    }
    out
}

/// `Project` dsts whose EVERY use is at a BORROWED (non-owned) position — a pure
/// borrow-view of an owned aggregate's field per `Spec: Annex E §AIMS TF-4`
/// (Project produces `Borrowed`). The PARENT aggregate owns the projected field
/// and releases it via its whole-var scope-exit drop-glue, so the borrowed
/// projection gets NO dec. An RcPtr-typed Project dst (e.g. `Project box.0 :
/// [int]`) enters `owned_vars_needing_rc` by TYPE alone; without this exclusion
/// a borrowed field-read (`@len [borrow]` / `@__index [borrow]`) gets a spurious
/// last-use `BurdenDec`, over-releasing the field the parent drop already frees.
///
/// Over-fire gate (RL-15a project-escape boundary): excluded ONLY when never used
/// at an owned arg position (`instr_transfer_vars` honors `is_owned_position`)
/// AND never returned — an escaping / owned-position-transferred Project IS an
/// owned reference and KEEPS its burden RC.
pub(in crate::lower::burden_lower) fn compute_borrowed_projection_dsts(
    func: &ArcFunction,
) -> FxHashSet<ArcVarId> {
    let mut project_dsts: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let crate::ir::ArcInstr::Project { dst, .. } = instr {
                project_dsts.insert(*dst);
            }
        }
    }
    if project_dsts.is_empty() {
        return FxHashSet::default();
    }
    let mut transferred_or_escaped: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            for v in instr_transfer_vars(instr, func) {
                transferred_or_escaped.insert(v);
            }
        }
        let term = &block.terminator;
        for (pos, &v) in term.used_vars().iter().enumerate() {
            if term.is_owned_position(pos) {
                transferred_or_escaped.insert(v);
            }
        }
        if let crate::ir::ArcTerminator::Return { value } = term {
            transferred_or_escaped.insert(*value);
        }
    }
    project_dsts.retain(|d| !transferred_or_escaped.contains(d));
    project_dsts
}

/// Borrowed-store duplication args per AIMS RL-1: vars whose `Let { Var }`
/// alias-chain ROOT is a BORROWED param and that are consumed at an
/// aggregate-STORE position (`Construct` / `Reuse` / `CollectionReuse` arg or
/// `Set.value`).
///
/// A borrow proves the CALLER retains a live reference across the whole call
/// (RL-2: the caller emits the borrowed param's release —
/// `AimsProof.Realization::RL2_borrowed_param_emits_caller_dec`), so storing
/// the borrowed value into an aggregate ALWAYS creates a real SECOND
/// reference: the caller's reference persists and the aggregate takes a new
/// one. The store-site `BurdenInc` is LOAD-BEARING
/// (`AimsProof.Realization::RL1_duplication_balanced` — the duplication's inc
/// is matched by exactly one release: the container's drop); without it the
/// container's drop releases a reference no inc supplied (the caller's
/// still-live value frees at live=1 — use-after-free).
///
/// THREE exclusions bound the family:
///
/// - Call-arg consumers: a borrowed value passed at an `Apply` / `Invoke` arg
///   is OUT — call consumes have their own interprocedural / protocol
///   ownership accounting; injecting an inc there over-counts. Only the
///   aggregate-STORE family is classified.
/// - Non-param roots: only a chain rooted at a BORROWED param carries the
///   borrow proof. Owned roots keep their existing treatment (terminal move /
///   genuine-dup per [`compute_genuine_dup_move_aliases`]); over-adding on an
///   owned source leaks.
/// - RC-free types: a stored value whose burden carries no RC dimension needs
///   no inc (nothing to release).
///
/// Membership is per-VAR; the emission walk emits one inc per store SITE the
/// member is consumed at (each store duplicates). No paired dec is emitted in
/// this function — the container's drop is the matched release wherever the
/// container dies.
pub(in crate::lower::burden_lower) fn compute_borrowed_store_dup_args(
    func: &ArcFunction,
    type_registry: &TypeRegistry,
) -> FxHashSet<ArcVarId> {
    if super::super::borrowed_store_dup_inc_disabled() {
        return FxHashSet::default();
    }
    let mut borrowed_rooted: FxHashSet<ArcVarId> = func
        .params
        .iter()
        .filter(|p| p.ownership == Ownership::Borrowed)
        .map(|p| p.var)
        .collect();
    if borrowed_rooted.is_empty() {
        return FxHashSet::default();
    }
    // Vars are defined before use within the function walk, so a single
    // forward pass resolves each `Let { Var }` chain to its root (same
    // discipline as `collect_use_sites`' single pass).
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                if borrowed_rooted.contains(src) {
                    borrowed_rooted.insert(*dst);
                }
            }
        }
    }
    let mut dup_args: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut admit = |var: ArcVarId| {
        if !borrowed_rooted.contains(&var) || dup_args.contains(&var) {
            return;
        }
        // L-9 repr-aware admission gate (same SSOT as `owned_vars_needing_rc`):
        // a provably-Scalar-repr stored value is a niche-packed copy — no
        // second reference exists, and a store-site inc on it can never lower.
        if super::super::is_provably_scalar_repr(func, var) {
            return;
        }
        let ty: TypeRef = idx_to_type_ref(func.var_types[var.index()], type_registry);
        if lookup_burden(ty, type_registry)
            .as_ref()
            .is_some_and(burden_carries_rc)
        {
            dup_args.insert(var);
        }
    };
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Construct { args, .. }
                | ArcInstr::Reuse { args, .. }
                | ArcInstr::CollectionReuse { args, .. } => {
                    for &arg in args {
                        admit(arg);
                    }
                }
                ArcInstr::Set { value, .. } => admit(*value),
                _ => {}
            }
        }
    }
    dup_args
}
