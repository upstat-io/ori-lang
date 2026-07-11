//! RL-1 store-family duplication funding on FRESH-local lineages: the FUNDED
//! aggregate-STORE duplication-alias set shared by the Phase-6 pair-atomic
//! collector and the Phase-7 lineage-net machinery. Spec: Annex E §AIMS RL-1 +
//! RL-2.

use std::sync::LazyLock;

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId};

use super::dup_inc::{collect_use_sites, is_burden_op};
use super::{collect_def_blocks, successor_reachable_blocks_with_cut};

/// `ORI_DISABLE_STORE_FAMILY_FUNDING=1` reverts the RL-1 store-family
/// duplication funding to the pre-cure arrangement
/// ([`compute_funded_store_dup_aliases`] returns empty): the Phase-6 per-var
/// DP-3 split strips the kept store-site dup incs of a FRESH-local lineage
/// whose source stays live past the store, and the Phase-7 debited net never
/// prices the store hand-offs. Bisection surface: isolates a fresh-local
/// multi-store double-free / store-lineage leak to the funded store treatment
/// vs the rest of the Phase-6 elimination + Phase-7 lowering. Spec: Annex E
/// §AIMS RL-1 + RL-2.
static STORE_FAMILY_FUNDING_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_STORE_FAMILY_FUNDING").as_deref() == Ok("1"));

/// Genuine-duplication STORE-out aliases (standalone form): `Let { Var(src) }`
/// dsts whose single-use move chain terminates at an aggregate-STORE consume
/// (`Construct` / `Reuse` / `CollectionReuse` arg or `Set.value` — NEVER an
/// `Apply` / `Invoke` arg, per the [`collect_move_edges_and_store_consumes`]
/// SSOT) while a use of `src` is forward-REACHABLE past the alias site. The
/// store creates a real SECOND reference to `src`'s allocation while the first
/// stays live, so the alias-site `BurdenInc` is LOAD-BEARING
/// (`AimsProof.Realization::RL1_duplication_balanced` — the container drop is
/// the matched release).
///
/// The store twin of [`compute_genuine_dup_call_arg_aliases`]: same
/// recomputed-from-`func` inputs (use counting skips burden ops so the set is
/// in lock-step pre- and post-emission), same forward-reachability
/// discriminator with the source-definition CUT (a loop-variant reassignment
/// re-defines the binding — the re-reached "use" belongs to the NEW binding,
/// so a per-iteration TERMINAL store declines while a loop-INVARIANT source
/// stored per iteration admits), same branch-exclusive decline (each path's
/// alias is the terminal move of the one reference).
///
/// RAW classification only — consumers take the FUNDED set
/// ([`compute_funded_store_dup_aliases`]). Empty when the store-family funding
/// toggle OR the master genuine-dup pair-coupling toggle is disabled (without
/// the Phase-5 pair coupling no store dup inc is emitted, so nothing is
/// funded).
fn compute_genuine_dup_store_aliases(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    if *STORE_FAMILY_FUNDING_DISABLED || super::super::genuine_dup_pair_coupling_disabled() {
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
    let (alias_next, store_consumed) = super::dup_inc::collect_move_edges_and_store_consumes(func);
    if store_consumed.is_empty() {
        return FxHashSet::default();
    }
    // Walk the single-use move chain from `dst`; true iff it ends at an
    // aggregate-store consume (mirrors `chain_ends_in_store` / the call-arg
    // twin's `chain_ends_in_call`).
    let chain_ends_in_store = |start: ArcVarId| -> bool {
        let mut cur = start;
        for _ in 0..func.var_types.len() {
            if use_sites.get(&cur).map(Vec::len) != Some(1) {
                return false;
            }
            if store_consumed.contains(&cur) {
                return true;
            }
            match alias_next.get(&cur) {
                Some(&next) => cur = next,
                None => return false,
            }
        }
        false
    };
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
            if use_counts.get(src).copied().unwrap_or(0) < 2 || !chain_ends_in_store(*dst) {
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

/// FUNDED store-family duplication aliases — the ONE SSOT every consumer of
/// the fresh-local aggregate-STORE duplication family reads: the RAW
/// classification ([`compute_genuine_dup_store_aliases`]) filtered to the
/// members whose alias-site `BurdenInc` Phase-5 emission actually KEEPS. Two
/// gates, mirroring [`compute_funded_call_arg_dup_aliases`]:
///
/// - `dup_alias_dsts` membership: forwarder-identity transparent aliases carry
///   no alias-site inc (skipped at the duplication classification), so they
///   fund nothing. Same `ORI_DISABLE_FORWARDER_IDENTITY_ALIAS_DEDUP` toggle
///   parity as the Phase-5 classification.
/// - `full_move_vars` exclusion via the pure project-source superset: every
///   full-move member is a `Project` source, and a raw member's single-use
///   move chain admits no `Project` use, so the superset and the exact set
///   agree on this family.
///
/// Standalone-callable (recomputes its gates from `func` + `contracts`): the
/// Phase-6 pair-atomic collector (`aims::realize::burden_elim`) and the
/// Phase-7 lineage-net machinery (`aims::realize::emit_unified`) consume this
/// ONE set. Per-alias membership only — NEVER the alias-chain ROOT (root-inc
/// suppression over-fires; the per-alias dst entry is the discriminator).
/// Empty when `ORI_DISABLE_STORE_FAMILY_FUNDING=1` (the raw scan owns the
/// toggle).
pub(crate) fn compute_funded_store_dup_aliases(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> FxHashSet<ArcVarId> {
    let mut funded = compute_genuine_dup_store_aliases(func);
    if funded.is_empty() {
        return funded;
    }
    // Gate 1 — dup_alias_dsts: drop forwarder-identity transparent aliases.
    if !*super::super::FORWARDER_IDENTITY_ALIAS_DEDUP_DISABLED {
        let forwarder_transparent =
            super::forwarder::compute_forwarder_identity_transparent_aliases(func, contracts);
        funded.retain(|v| !forwarder_transparent.contains(v));
    }
    // Gate 2 — full_move_vars, via the pure project-source superset.
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
