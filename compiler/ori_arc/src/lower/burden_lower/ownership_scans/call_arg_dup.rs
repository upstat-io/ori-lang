//! RL-1 owned-call-arg duplication scans: the genuine-duplication
//! OWNED-CALL-ARG alias admission (the multi-fork COW-lineage cure). The
//! call-result-aggregate element FINAL-READ release designation lives in
//! [`result_elem_release`]. Spec: Annex E §AIMS RL-1 + RL-2.

mod result_elem_release;

use std::sync::LazyLock;

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::dimensions::AccessClass;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

use super::dup_inc::{collect_use_sites, is_burden_op};
use super::{collect_def_blocks, successor_reachable_blocks_with_cut};

pub(in crate::lower::burden_lower) use result_elem_release::compute_call_result_element_final_read_releases;

/// `ORI_DISABLE_OWNED_CALL_ARG_DUP_INC=1` restores the symmetric-cancellation
/// treatment for a `Let { Var(src) }` dup-alias consumed at an OWNED call-arg
/// position while `src` stays live ([`compute_genuine_dup_call_arg_aliases`]
/// returns empty). Default (unset): the alias keeps its RL-1 duplication
/// `BurdenInc` — each owned-call-arg fork is funded by its own surviving inc
/// whose matched release is the consumer's (`RL1_duplication_balanced`).
/// Bisection surface: isolates a multi-fork COW-lineage double-free / wrong
/// value to the kept duplication inc vs the rest of the Phase-5 walk.
/// Spec: Annex E §AIMS RL-1 + RL-2.
static OWNED_CALL_ARG_DUP_INC_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_OWNED_CALL_ARG_DUP_INC").as_deref() == Ok("1"));

/// Owned-call-arg consume positions admissible as RL-1 duplication chain
/// endpoints: `(var, is_rc_reading)` membership for every var consumed at an
/// OWNED arg position of an `Apply` / `ApplyIndirect` body instruction or an
/// `Invoke` / `InvokeIndirect` terminator.
///
/// "Owned" is BOTH structural (`is_owned_position` over the Phase-5
/// `arg_ownership` annotations — the builtin COW receivers `push` / `insert` /
/// `set` / `remove` carry `[own]` at lowering) AND contract-derived
/// (`ParamContract.access == Owned` — a user callee's call-site annotation is
/// still the pre-`realize_rc_reuse` borrowed default at Phase 5, while borrow
/// inference already proved the param Owned).
///
/// EXCLUSIONS (each a recorded over-fire fence):
/// - the `iter` protocol builtin's owned args (iter-consume transfer — its own
///   RL-2 accounting; admitting it over-counts +1 per loop on the iterator
///   corpus);
/// - `__`-prefixed protocol builtins (`__iter_next`, `__index` — compiler-
///   interceptable with their own ownership protocol);
/// - positions whose `ParamContract.iter_consumes` is set (user-callee
///   iter-consume transfer, same fence as the `iter` builtin).
fn collect_owned_call_arg_consumes(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    let iter_name =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Iter.name());
    let callee_excluded = |callee: Name| -> bool {
        callee == iter_name
            || interner
                .try_lookup(callee)
                .is_none_or(|n| n.starts_with("__"))
    };
    // KNOWN builtins carry precise structural `[own]`/`[borrow]` call-site
    // annotations from lowering (`BuiltinOwnershipSets`) — the contract-derived
    // borrowed-consuming class applies ONLY to user callees (their call-site
    // annotation is the pre-`realize` borrowed default; seeded builtin
    // contracts leave `borrowed_read_only` at its conservative `false` default,
    // which would over-admit pure-read receivers like `@len`/`@concat`).
    let builtins = crate::borrow::BuiltinOwnershipSets::new(interner);
    let contract_owned = |callee: Name, pos: usize| {
        contract_consuming_arg_position(contracts, &builtins, interner, callee, pos)
    };
    // Per-position exclusion fences applying to BOTH the structural and the
    // contract-derived admission: iter-consuming positions (own RL-2
    // accounting) and `transfers_through_return` PASS-THROUGH positions (a
    // ttr forwarder hands the same allocation back through the result — the
    // lineage continues per RL-34; the hand-in is never a consumed
    // duplication).
    let contract_excluded = |callee: Name, pos: usize| -> bool {
        contracts.get(&callee).is_some_and(|c| {
            c.params
                .get(pos)
                .is_some_and(|p| p.iter_consumes || p.transfers_through_return)
        })
    };
    let mut consumed: FxHashSet<ArcVarId> = FxHashSet::default();
    // Named-callee admission: structural `[own]` OR contract-proven Owned,
    // minus the iter-consume / protocol-builtin fences. `args` are positional
    // user args for `Apply` / `Invoke`, so the arg index IS the param index.
    let admit_named = |out: &mut FxHashSet<ArcVarId>,
                       callee: Name,
                       args: &[ArcVarId],
                       owned_at: &dyn Fn(usize) -> bool| {
        if callee_excluded(callee) {
            return;
        }
        for (pos, &arg) in args.iter().enumerate() {
            if contract_excluded(callee, pos) {
                continue;
            }
            if owned_at(pos) || contract_owned(callee, pos) {
                out.insert(arg);
            }
        }
    };
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Apply {
                    func: callee, args, ..
                } => {
                    admit_named(&mut consumed, *callee, args, &|pos| {
                        instr.is_owned_position(pos)
                    });
                }
                // Indirect calls have no callee identity (no contract, no
                // protocol-builtin risk): structural owned positions only.
                // `used_vars()` is `[closure, ...args]`, position 0 borrowed.
                ArcInstr::ApplyIndirect { .. } => {
                    for (pos, &var) in instr.used_vars().iter().enumerate() {
                        if instr.is_owned_position(pos) {
                            consumed.insert(var);
                        }
                    }
                }
                _ => {}
            }
        }
        match &block.terminator {
            ArcTerminator::Invoke {
                func: callee, args, ..
            } => {
                let term = &block.terminator;
                admit_named(&mut consumed, *callee, args, &|pos| {
                    term.is_owned_position(pos)
                });
            }
            ArcTerminator::InvokeIndirect { .. } => {
                let term = &block.terminator;
                for (pos, &var) in term.used_vars().iter().enumerate() {
                    if term.is_owned_position(pos) {
                        consumed.insert(var);
                    }
                }
            }
            _ => {}
        }
    }
    consumed
}

/// Genuine-duplication OWNED-CALL-ARG aliases per AIMS RL-1: `Let { Var(src) }`
/// dup-aliases whose single-use move chain terminates at an owned call-arg
/// consume ([`collect_owned_call_arg_consumes`]) while a use of `src` is
/// forward-REACHABLE past the alias site — the call consumes a real SECOND
/// reference to `src`'s allocation while the first stays live, so the
/// alias-site `BurdenInc` is LOAD-BEARING (`RL1_duplication_balanced`: the
/// consumer's release — the COW helper's copy-source dec, the callee's
/// owned-param release — is the matched release). The kept inc MUST escape the
/// `emit_terminator_burden_decs` symmetric same-block cancellation AND the
/// Phase-5 inc-suppression scans; consumers gate those on this set.
///
/// The call-arg twin of [`compute_genuine_dup_move_aliases`] (which admits
/// ONLY aggregate-STORE endpoints): same dup-alias classification, same
/// forward-reachability discriminator (branch-exclusive aliases decline; loop
/// back-edges admit), same `full_move_vars` exclusion — only the chain
/// endpoint differs. The iter-consume / protocol-builtin fences live in the
/// endpoint collection.
///
/// RAW classification only — consumers take the FUNDED set
/// ([`compute_funded_call_arg_dup_aliases`]), which additionally applies the
/// Phase-5 emission gates (a raw member outside `dup_alias_dsts` or inside
/// `full_move_vars` never receives the alias-site inc, so it funds nothing).
/// Empty when `ORI_DISABLE_OWNED_CALL_ARG_DUP_INC=1`.
pub(in crate::lower::burden_lower) fn compute_genuine_dup_call_arg_aliases(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    if *OWNED_CALL_ARG_DUP_INC_DISABLED {
        return FxHashSet::default();
    }
    let mut use_counts: FxHashMap<ArcVarId, u32> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if is_burden_op(instr) {
                continue;
            }
            for &v in &instr.used_vars() {
                *use_counts.entry(v).or_default() += 1;
            }
        }
        for v in block.terminator.used_vars() {
            *use_counts.entry(v).or_default() += 1;
        }
    }
    let use_sites = collect_use_sites(func);
    let (alias_next, _) = super::dup_inc::collect_move_edges_and_store_consumes(func);
    let call_consumed = collect_owned_call_arg_consumes(func, contracts, interner);
    if call_consumed.is_empty() {
        return FxHashSet::default();
    }
    // Walk the single-use move chain from `dst`; true iff it ends at an owned
    // call-arg consume (mirrors `chain_ends_in_store`).
    let chain_ends_in_call = |start: ArcVarId| -> bool {
        let mut cur = start;
        for _ in 0..func.var_types.len() {
            if use_sites.get(&cur).map(Vec::len) != Some(1) {
                return false;
            }
            if call_consumed.contains(&cur) {
                return true;
            }
            match alias_next.get(&cur) {
                Some(&next) => cur = next,
                None => return false,
            }
        }
        false
    };
    // Source-definition CUT (a loop-variant reassignment `xs = xs.push(i)`
    // threads the CONSUME RESULT into the next iteration's block-param; the
    // re-reached "use" belongs to the NEW binding, not the consumed
    // allocation). A loop-INVARIANT source (defined OUTSIDE the cycle, e.g.
    // `base` forked per iteration by `base.set(0, i)`) is unaffected — its
    // defining block is never re-reached from the alias's successors, so
    // in-loop and post-loop uses still admit. Shared helpers: `collect_def_blocks`
    // + `successor_reachable_blocks_with_cut` (`ownership_scans` hub).
    let def_block = collect_def_blocks(func);
    let mut reachable_cache: FxHashMap<(usize, Option<usize>), FxHashSet<usize>> =
        FxHashMap::default();
    let mut genuine: FxHashSet<ArcVarId> = FxHashSet::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            else {
                continue;
            };
            // Dup-alias classification: `src` stays live (>= 2 used-var
            // positions). The Phase-5 `dup_alias_dsts` refinement
            // (forwarder-transparent exclusion) is param-rooted, disjoint
            // from the fresh-local call-arg family admitted here.
            if use_counts.get(src).copied().unwrap_or(0) < 2 || !chain_ends_in_call(*dst) {
                continue;
            }
            let Some(sites) = use_sites.get(src) else {
                continue;
            };
            let cut = def_block.get(src).copied();
            let reachable = reachable_cache
                .entry((block_idx, cut))
                .or_insert_with(|| successor_reachable_blocks_with_cut(func, block_idx, cut));
            let src_used_after = sites.iter().any(|&(ub, ui)| {
                (ub == block_idx && ui > instr_idx) || (reachable.contains(&ub) && cut != Some(ub))
            });
            if src_used_after {
                genuine.insert(*dst);
            }
        }
    }
    genuine
}

/// FUNDED owned-call-arg duplication aliases — the ONE SSOT every consumer of
/// the owned-call-arg duplication family reads: the RAW classification
/// ([`compute_genuine_dup_call_arg_aliases`]) filtered to the members whose
/// alias-site `BurdenInc` is actually KEPT by Phase-5 emission. A raw member
/// outside the funded set receives NO alias-site inc, so treating it as
/// dup-funded in the Phase-6 pair-atomicity guard or the Phase-7 lineage-net
/// accounting debits a reference no inc supplied (emission vs accounting
/// drift). Two gates, mirroring the Phase-5 emission filter:
///
/// - `dup_alias_dsts` membership: forwarder-identity transparent aliases
///   ([`compute_forwarder_identity_transparent_aliases`]) are skipped at the
///   duplication classification (`compute_use_counts_and_dup_aliases`) AND
///   excluded from `owned_vars_needing_rc`, so they carry no alias-site inc.
///   The raw set's own `use_counts >= 2` admission matches the remaining
///   `dup_alias_dsts` condition, so the gate reduces to the forwarder
///   exclusion (same `ORI_DISABLE_FORWARDER_IDENTITY_ALIAS_DEDUP` toggle).
/// - `full_move_vars` exclusion: a full-move var's whole-var ops are owned by
///   the field-projection machinery (`inc_suppressed_vars` coupling). Every
///   `full_move_vars` member — initial (`compute_full_move_vars` over the
///   `moved_out_fields` keys), sibling-union-absorbed, or
///   extract-transfer-widened — is a `Project` SOURCE, so the gate is applied
///   via the pure project-source superset (recomputable from `func` alone; a
///   raw member's single-use move chain admits no `Project` use, so the
///   superset and the exact set agree on this family).
///
/// Standalone-callable (recomputes its gates from `func` + `contracts`): the
/// Phase-5 emission walk (`scan_orchestration` / `scan_helpers`), the Phase-6
/// pair-atomic collector (`aims::realize::burden_elim`), and the Phase-7
/// lineage-net machinery (`aims::realize::emit_unified`) all consume this ONE
/// set. Use counting inside the raw scan skips burden ops, keeping the set in
/// lock-step pre- and post-emission. Empty when
/// `ORI_DISABLE_OWNED_CALL_ARG_DUP_INC=1` (the raw scan owns the toggle).
pub(crate) fn compute_funded_call_arg_dup_aliases(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    let mut funded = compute_genuine_dup_call_arg_aliases(func, contracts, interner);
    if funded.is_empty() {
        return funded;
    }
    // Gate 1 — dup_alias_dsts: drop forwarder-identity transparent aliases
    // (they are skipped at the duplication classification, so no alias-site
    // inc exists for them). Toggle parity with the Phase-5 classification.
    if !*super::super::FORWARDER_IDENTITY_ALIAS_DEDUP_DISABLED {
        let forwarder_transparent =
            super::forwarder::compute_forwarder_identity_transparent_aliases(func, contracts);
        funded.retain(|v| !forwarder_transparent.contains(v));
    }
    // Gate 2 — full_move_vars, via the pure project-source superset: every
    // full-move member is a `Project` source (see fn doc), so excluding the
    // superset excludes the exact set on this family.
    let mut project_sources: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { value, .. } = instr {
                project_sources.insert(*value);
            }
        }
    }
    funded.retain(|v| !project_sources.contains(v));
    funded
}

/// Contract-derived CONSUMING user-callee arg position — the SSOT predicate
/// shared by the owned-call-arg dup admission, the Phase-7 hand-off debits,
/// and the dup-funded COW clearance. TWO classes (each minus `iter_consumes`):
///  - `access == Owned`: the callee own-consumes outright;
///  - `access == Borrowed && !borrowed_read_only`: a borrowed param with an
///    owned-position use inside the callee (the COW-consume class —
///    `@grow(xs) = xs.push(v)`). The realization convention makes this an
///    effective consume: the call-site annotation falls back to Owned and the
///    callee's COW-inc edge release nets -1 on the caller's lineage when the
///    param dies at the consume, so the caller funds one reference per call. A
///    `borrowed_read_only` param (pure reads, `@total(xs) = xs.fold(..)`)
///    nets 0 — never a consume.
///
/// KNOWN builtins are excluded — their call-site `[own]`/`[borrow]`
/// annotations from lowering are the precise structural source (seeded
/// builtin contracts leave `borrowed_read_only` at its conservative `false`
/// default, which would mis-class pure-read receivers like `@len`/`@concat`).
pub(crate) fn contract_consuming_arg_position(
    contracts: &FxHashMap<Name, MemoryContract>,
    builtins: &crate::borrow::BuiltinOwnershipSets,
    interner: &ori_ir::StringInterner,
    callee: Name,
    pos: usize,
) -> bool {
    let owned = !builtins.is_builtin(callee)
        && contracts.get(&callee).is_some_and(|c| {
            c.params.get(pos).is_some_and(|p| {
                // `transfers_through_return` positions are PASS-THROUGHS, not
                // consumes: a ttr forwarder (`@id<T>(x) = x`) hands the same
                // allocation back through the result — the lineage continues
                // (RL-34); treating the hand-in as a consumed duplication
                // over-incs the moved-through allocation. The Borrowed class
                // requires the precise `borrowed_cow_consumed` fact (COW
                // consume at the lineage's death) — a borrowed STORE
                // (`Inner { items: xs }`) or a live-past COW read nets 0 in
                // the callee and never obligates caller funding.
                !p.iter_consumes
                    && !p.transfers_through_return
                    && (p.access == AccessClass::Owned
                        || (p.access == AccessClass::Borrowed && p.borrowed_cow_consumed))
            })
        });
    if tracing::enabled!(target: "ori_arc::aims::realize", tracing::Level::TRACE) {
        let probe = contracts.get(&callee).and_then(|c| c.params.get(pos));
        tracing::trace!(
            target: "ori_arc::aims::realize",
            callee = ?interner.try_lookup(callee),
            pos,
            param = ?probe,
            owned,
            "owned-call-arg admission contract probe"
        );
    }
    owned
}
