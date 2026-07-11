//! Post-realize cleanup of redundant project-alias decs.
//!
//! The over-emission
//! shape for nested-Project chains over transitive-drop variant containers
//! cannot be cured at any single emission site — the failing decs arrive from
//! multiple producers with no shared per-class budget.
//!
//! Cure: a single emission-time service consumed indirectly via post-pass
//! cleanup. For each project-only class (no Direct/Conditional apply-result
//! union members, no parameter members), count `K` = explicit `RcInc` and `N`
//! = explicit `RcDec` on class members. When N > K, the source's drop chain
//! contributes 1 dec via type-driven walk, so the slot needs exactly K
//! explicit decs to balance — remove (N - K) latest-CFG-order decs.
//!
//! ## Why a post-pass
//!
//! The realize pipeline emits `RcDec` from multiple producers (body last-use
//! placement, entry-dead cleanup, per-edge dying-successor releases), each
//! owning its own gating. Coordinating a per-class budget across producers
//! at emission time requires shared state none has. A post-pass that runs
//! after all emission is the cleanest cure surface.
//!
//! ## Discriminator
//!
//! - Reject classes touched by Direct OR Conditional apply-result union
//!   (Wrapped is containment-only, not Project provenance).
//! - Reject parameter-member classes (param's drop is owned by caller, can't
//!   over-suppress).
//! - Require: at least one class member has a `project_alias_sources` entry
//!   pointing at a transitive-drop ancestor (so source's drop chain WILL
//!   walk this slot and provide the implicit dec).

use std::sync::LazyLock;

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::StringInterner;
use ori_types::Pool;

use crate::aims::intraprocedural::state_map::{AimsStateMap, ApplyAliasSource};
use crate::graph::DominatorTree;
use crate::ir::{is_transitive_drop_strategy, ArcBlockId, ArcFunction, ArcInstr, ArcVarId};

use super::super::emit_rc::rc_strategy;

/// `ORI_DISABLE_REDUNDANT_CLEANUP=1` disables the redundant project-alias dec
/// cleanup pass. Read once at first access; kept as a permanent diagnostic
/// bisection flag (RC-balance issues originating in the cleanup vs upstream
/// emission).
static REDUNDANT_CLEANUP_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_REDUNDANT_CLEANUP").is_ok());

/// Post-realize cleanup pass — remove redundant project-alias decs.
///
/// Runs AFTER `realize_annotations` (Phase 2) so it sees the final realized
/// IR including all four `RcDec` emission paths (body walk, dead-cleanup,
/// edge-cleanup invoke, edge-cleanup branch) plus any annotations.
///
/// Idempotent: removing decs never adds new ones, and a second pass would
/// find K = N (we just balanced) so make no further changes.
#[expect(
    clippy::too_many_lines,
    reason = "Exhaustive K/N inventory + chain walk + dominator removal across one logical pass — splitting fragments the algorithm without isolating reusable units."
)]
pub(crate) fn cleanup_redundant_project_alias_decs(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
    interner: &StringInterner,
) {
    // `ORI_DISABLE_REDUNDANT_CLEANUP=1` disables the cure for diagnostic
    // regression isolation; kept as a permanent diagnostic bisection flag.
    if *REDUNDANT_CLEANUP_DISABLED {
        return;
    }
    // Skip cure when function contains Apply
    // @ori_catch_recover. Catch lowering in lower/constructs.rs
    // returns a fresh str (the panic msg) via this builtin; the str's RC
    // trajectory interacts with class_outer's transitive drops AND the
    // for-loop iter chain in ways the simple K/N/source-dec model
    // miscounts. Conservative skip preserves baseline correctness for
    // catch-using functions (the canonical regression: result_str_letvar
    // _forloop_over_alias) at the cost of leaving match_arm_alias_result
    // _str + unwind_path_alias unfixed by this cure, left for separate
    // path-sensitive cure work.
    let catch_recover_name = interner.intern("ori_catch_recover");
    let has_catch_recover = func.blocks.iter().any(|block| {
        block.body.iter().any(|instr| {
            matches!(instr, ArcInstr::Apply { func: callee, .. } if *callee == catch_recover_name)
        })
    });
    if has_catch_recover {
        return;
    }

    // Step 1: identify project-only classes that qualify for cleanup.
    let qualified_classes: FxHashSet<u32> = identify_qualified_classes(func, state_map, pool);
    if qualified_classes.is_empty() {
        return;
    }

    // Step 2: per qualified class, inventory RcInc count (K) AND RcDec
    // locations (N). Cure surplus: when N > K, the source's drop chain
    // provides one type-driven dec, so K explicit decs balance the ledger.
    // Surplus = N - K decs to suppress. Dominance picks WHICH ones.
    let mut class_inc_count: FxHashMap<u32, usize> = FxHashMap::default();
    let mut class_decs: FxHashMap<u32, Vec<(usize, usize)>> = FxHashMap::default();

    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            match instr {
                ArcInstr::RcInc { var, count, .. } => {
                    if let Some(class_id) = state_map.ssa_alias_class_of(*var) {
                        if qualified_classes.contains(&class_id) {
                            *class_inc_count.entry(class_id).or_default() += *count as usize;
                        }
                    }
                }
                ArcInstr::RcDec { var, .. } => {
                    if let Some(class_id) = state_map.ssa_alias_class_of(*var) {
                        if qualified_classes.contains(&class_id) {
                            class_decs
                                .entry(class_id)
                                .or_default()
                                .push((block_idx, instr_idx));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    if class_decs.is_empty() {
        return;
    }

    // Step 3: dominator-based redundancy detection — for each class where
    // N > K, suppress decs that are strictly cross-block-dominated by another
    // dec. Within-block ordering does NOT imply redundancy (multiple decs
    // in the same block balance multiple intra-block RcIncs). Cross-block
    // dominance proves execution-order coverage on every reaching path.
    //
    // Surplus cap: stop after suppressing (N - K) decs per class. Without
    // this cap, a class with N=K (balanced) but multiple dominated chains
    // would over-suppress. Suppress in CFG-late-first order so that the
    // EARLIEST kept decs match the original PIN-4 canonical-rep selection.
    let dom = DominatorTree::build(func);
    let mut removals: FxHashSet<(usize, usize)> = FxHashSet::default();

    // Pre-compute decs-per-class function-wide to count source-class decs
    // (each walks payload via type-driven drop, contributing one cover per
    // dec).
    let mut all_class_dec_counts: FxHashMap<u32, usize> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::RcDec { var, .. } = instr {
                if let Some(class_id) = state_map.ssa_alias_class_of(*var) {
                    *all_class_dec_counts.entry(class_id).or_default() += 1;
                }
            }
        }
    }

    for (class_id, decs) in &class_decs {
        let n = decs.len();
        // Chain-aware K/N counting. Walk transitively
        // through project_alias_sources to find ALL classes that share the
        // underlying physical storage. Aggregate K (RcInc) and N (RcDec)
        // across the chain. The class being decided (class_id) contributes
        // K_self and its decs are the candidates; other chain classes
        // contribute K and N_transitive (transitive walks via type-driven
        // drop).
        //
        // RC ledger for the shared slot:
        //   alloc(1) + total_K (sum of K across chain) -
        //   N_self (explicit decs on class_id) -
        //   N_transitive (decs on other chain classes) = 0
        // → N_self_needed = total_K + 1 - N_transitive
        //
        // surplus = N_self - N_self_needed (only when positive).
        let project_alias_sources_map = state_map.project_alias_sources();
        let mut chain_classes: FxHashSet<u32> = FxHashSet::default();
        chain_classes.insert(*class_id);
        let mut frontier: Vec<u32> = vec![*class_id];
        while let Some(c) = frontier.pop() {
            if let Some(members) = state_map.class_members(c) {
                for m in members {
                    let Some(sources) = project_alias_sources_map.get(m) else {
                        continue;
                    };
                    for &source_var in sources {
                        if let Some(source_class) = state_map.ssa_alias_class_of(source_var) {
                            if chain_classes.insert(source_class) {
                                frontier.push(source_class);
                            }
                        }
                    }
                }
            }
        }
        // Filter chain to transitive-drop classes only — non-transitive-drop
        // classes don't walk the slot via drop chain.
        let chain_transitive: Vec<u32> = chain_classes
            .iter()
            .copied()
            .filter(|&c| {
                if c == *class_id {
                    return true;
                }
                let Some(members) = state_map.class_members(c) else {
                    return false;
                };
                members.iter().next().is_some_and(|&rep| {
                    rc_strategy(func, rep, pool).is_some_and(is_transitive_drop_strategy)
                })
            })
            .collect();
        let total_k: usize = chain_transitive
            .iter()
            .map(|c| class_inc_count.get(c).copied().unwrap_or(0))
            .sum();
        let n_transitive: usize = chain_transitive
            .iter()
            .filter(|&&c| c != *class_id)
            .map(|c| all_class_dec_counts.get(c).copied().unwrap_or(0))
            .sum();
        let n_self_needed = (total_k + 1).saturating_sub(n_transitive);
        if n <= n_self_needed {
            continue;
        }
        let surplus = n - n_self_needed;
        // Identify candidates for removal: decs strictly cross-block-
        // dominated by another dec.
        let mut candidates: Vec<(usize, usize)> = decs
            .iter()
            .copied()
            // Every surviving dec is a placed RL-2/RL-4 release, so only the
            // pass's namesake redundancy — a dec on a var that IS a Project
            // alias of a transitive-drop source — is removable; a root /
            // Let-alias dec is the lineage's genuine release. The ledger
            // (N/K/surplus) stays class-wide; this restricts only WHICH decs
            // the surplus may remove.
            .filter(|&(b_idx, i_idx)| {
                let Some(ArcInstr::RcDec { var, .. }) =
                    func.blocks.get(b_idx).and_then(|b| b.body.get(i_idx))
                else {
                    return false;
                };
                is_project_alias_of_transitive_drop(func, state_map, pool, *var)
            })
            .filter(|&(b2_idx, _i2_idx)| {
                let Ok(b2_u32) = u32::try_from(b2_idx) else {
                    return false;
                };
                let b2 = ArcBlockId::new(b2_u32);
                decs.iter().any(|&(b1_idx, _i1_idx)| {
                    if b1_idx == b2_idx {
                        return false;
                    }
                    let Ok(b1_u32) = u32::try_from(b1_idx) else {
                        return false;
                    };
                    let b1 = ArcBlockId::new(b1_u32);
                    dom.dominates(b1, b2)
                })
            })
            .collect();
        // Sort ascending by (block_idx, instr_idx); take the LATEST `surplus`
        // candidates. Latest in CFG order = the redundant ones (drop chain
        // already fired before them).
        candidates.sort_unstable();
        for &loc in candidates.iter().rev().take(surplus) {
            removals.insert(loc);
        }
    }

    if removals.is_empty() {
        return;
    }
    log_cleanup_removals(func, &removals);

    // Step 4: rebuild each block's body without the marked decs. Iterate
    // blocks in reverse so removed indices don't shift unprocessed entries
    // — but since we rebuild via filter into a fresh Vec, index shifting
    // is moot. The `removals` set is keyed by ORIGINAL (block_idx, instr_idx)
    // captured during inventory, so we use the same order here.
    for (block_idx, block) in func.blocks.iter_mut().enumerate() {
        let new_body: Vec<ArcInstr> = block
            .body
            .iter()
            .enumerate()
            .filter_map(|(instr_idx, instr)| {
                if removals.contains(&(block_idx, instr_idx)) {
                    None
                } else {
                    Some(instr.clone())
                }
            })
            .collect();
        block.body = new_body;
    }
}

/// Emit the post-cleanup diagnostic event listing removed dec sites.
#[cold]
fn log_cleanup_removals(func: &ArcFunction, removals: &FxHashSet<(usize, usize)>) {
    tracing::debug!(
        target: "ori_arc::aims::realize",
        func = ?func.name,
        removals = ?removals,
        "[CLEANUP] removing"
    );
}

/// Identify project-only classes qualifying for redundant-dec cleanup.
///
/// Per the discriminator:
/// - REJECT classes with any member in `apply_result_aliases` as Direct or
///   Conditional (Wrapped is containment-only, allowed).
/// - REJECT classes with any member that is a function parameter.
/// - REQUIRE at least one member has a `project_alias_sources` entry pointing
///   at a transitive-drop ancestor (else source's drop chain doesn't cover).
fn identify_qualified_classes(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
) -> FxHashSet<u32> {
    let param_set: FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();
    let project_alias_sources_map = state_map.project_alias_sources();
    let apply_result_aliases_map = state_map.apply_result_aliases();

    // Compute set of vars that are Apply/Invoke destinations across the
    // function. Any class containing such a var has Apply provenance and
    // is excluded — only real Project-origin chains qualify for this cure.
    // Apply destinations may inherit project_alias
    // _sources entries via Step 1b (Apply-aliased seed) but those are NOT
    // real Project provenance and the cure must skip them.
    let mut apply_dst_set: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply { dst, .. } | ArcInstr::ApplyIndirect { dst, .. } = instr {
                apply_dst_set.insert(*dst);
            }
        }
        if let crate::ir::ArcTerminator::Invoke { dst, .. }
        | crate::ir::ArcTerminator::InvokeIndirect { dst, .. } = &block.terminator
        {
            apply_dst_set.insert(*dst);
        }
    }

    let mut qualified: FxHashSet<u32> = FxHashSet::default();

    // Collect all unique class IDs from class_members + ssa_alias_class_of
    // for vars in project_alias_sources (covers singleton non-materialized).
    let mut candidate_classes: FxHashSet<u32> = FxHashSet::default();
    for var in project_alias_sources_map.keys() {
        if let Some(class_id) = state_map.ssa_alias_class_of(*var) {
            candidate_classes.insert(class_id);
        }
    }

    for class_id in candidate_classes {
        let Some(members) = state_map.class_members(class_id) else {
            continue;
        };
        // REJECT: any member is parameter.
        if members.iter().any(|m| param_set.contains(m)) {
            continue;
        }
        // REJECT: any member in apply_result_aliases as Direct or Conditional.
        let has_direct_or_conditional = members.iter().any(|m| {
            matches!(
                apply_result_aliases_map.get(m),
                Some(ApplyAliasSource::Direct(_) | ApplyAliasSource::Conditional { .. })
            )
        });
        if has_direct_or_conditional {
            continue;
        }
        // REJECT: any member is an Apply/Invoke destination. Apply provenance
        // (including Wrapped variants) is NOT real Project provenance —
        // Step 1b seeds project_alias_sources with Apply
        // arg, but suppression must NOT fire on Apply-derived classes
        // (catch lowering, generic forwarders, etc. — these have their own
        // cleanup machinery).
        if members.iter().any(|m| apply_dst_set.contains(m)) {
            continue;
        }
        // REQUIRE: at least one member has project_alias_sources entry pointing
        // at a transitive-drop ancestor.
        let has_transitive_drop_source = members.iter().any(|m| {
            let Some(sources) = project_alias_sources_map.get(m) else {
                return false;
            };
            sources.iter().any(|&source_var| {
                rc_strategy(func, source_var, pool).is_some_and(is_transitive_drop_strategy)
            })
        });
        if !has_transitive_drop_source {
            continue;
        }
        qualified.insert(class_id);
    }
    qualified
}

/// Whether `var` is STRUCTURALLY the dst of a `Project` instruction whose
/// projected source carries a transitive-drop RC strategy. Only such decs are
/// burden-sole suppression candidates: the source's drop glue walks the
/// projected slot, making the explicit dec the namesake redundancy. The
/// `project_alias_sources` map is NOT consulted (its Apply-arg seeding
/// pollutes it with non-Project provenance).
fn is_project_alias_of_transitive_drop(
    func: &ArcFunction,
    _state_map: &AimsStateMap,
    pool: &Pool,
    var: ArcVarId,
) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { dst, value, .. } = instr {
                if *dst == var {
                    return rc_strategy(func, *value, pool)
                        .is_some_and(is_transitive_drop_strategy);
                }
            }
        }
    }
    false
}
