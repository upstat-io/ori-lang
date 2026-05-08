//! Project alias analysis for backward demand propagation.
//!
//! Computes a function-wide map from Project destinations (and their transitive
//! aliases through Let instructions and Jump-arg → block-param edges) to the
//! original Project source variables. At CFG merge points a block param may
//! receive projected values from *multiple* predecessor aggregates, so the map
//! value is a `SmallVec` of sources (not a single `ArcVarId`).
//!
//! This enables [`propagate_project_source_demand`] to keep *all* possible
//! parent aggregates alive when borrowed children are used cross-block.
//!
//! # Example
//!
//! ```text
//! %3 = Project %2.0
//! %4 = Let Var(%3)
//! Jump block1, args=[%4]
//! // block1 params: [%5]
//! ```
//!
//! Maps: `%3 → [%2]`, `%4 → [%2]`, `%5 → [%2]`

use rustc_hash::FxHashMap;
use smallvec::{smallvec, SmallVec};

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

use super::super::lattice::{AccessClass, AimsState, Cardinality};
use super::state_map::{AimsStateMap, ApplyAliasSource};

/// Per-variable set of possible Project source aggregates.
///
/// `SmallVec<[ArcVarId; 1]>` avoids heap allocation for the common
/// single-predecessor case while supporting multi-predecessor merges.
pub(crate) type ProjectSources = SmallVec<[ArcVarId; 1]>;

/// Compute a function-wide map from (Project destination + transitive Let
/// aliases + block param aliases + Apply-result alias roots) to the set of
/// possible Project / Apply-aliased source variables.
///
/// For `%3 = Project %2.0` followed by `%4 = Let Var(%3)`, maps both
/// `%3 → [%2]` and `%4 → [%2]`. For `Jump block1, args=[%3]` where block1
/// has param `%5`, maps `%5 → [%2]`. At CFG merges where multiple
/// predecessors jump to the same block with projections from different
/// aggregates, the param maps to the union of all sources.
///
/// BUG-04-090: also folds in [`ApplyAliasSource`] entries as Step 1b roots so
/// shipped transitivity Rules 2/3/4/6 (§1.9 `project_alias_sources` preamble —
/// Rules 5/7 are unshipped, folding through them is dead code) carry the
/// Apply-result allocation-identity through Let/Jump/CFG-merge/nested-Project
/// alias chains. When `%4 = Apply ...` and `apply_result_aliases[%4] =
/// Direct(%3)`, Step 1b seeds `alias_sources[%4] = [%3]`; downstream `%5 = Let
/// Var(%4)` then transitively maps `%5 → [%3]` via the existing Step 2
/// worklist without any per-rule code change.
///
/// This enables `propagate_project_source_demand` to detect demand on any
/// alias of a projected or Apply-aliased value — including block params that
/// receive projected values from multiple predecessors via Jump arguments.
///
/// Precomputed once before the worklist loop (the alias structure is static).
pub(crate) fn compute_project_alias_sources(
    func: &ArcFunction,
    apply_result_aliases: &FxHashMap<ArcVarId, ApplyAliasSource>,
) -> FxHashMap<ArcVarId, ProjectSources> {
    // Step 1: Direct Project destinations → sources.
    //
    // note: take-projects (unique-owned payloads moved out
    // of sum types, e.g., iterators projected from a tagged-pointer
    // enum) *could* also be skipped here to avoid keeping the parent
    // alive, but that would require plumbing a `Pool` through the
    // public `analyze_function` API. Instead, is fixed at
    // the borrowed_defs + is_ownership_transfer layer, which is
    // sufficient: the take-project's projected payload is no longer
    // classified as borrowed, so no spurious `RcInc` fires at the
    // owned-arg call site, and the Project itself is treated as an
    // ownership transfer, so walk_dec skips the source enum's
    // scope-exit last-use dec.
    let mut alias_sources: FxHashMap<ArcVarId, ProjectSources> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { dst, value, .. } = instr {
                alias_sources.insert(*dst, smallvec![*value]);
            }
        }
    }

    // Step 1b (BUG-04-090): seed alias graph with Apply-result allocation-identity
    // entries. Each `apply_result_aliases[apply_dst]` records that `apply_dst`
    // shares an allocation with one or more consumed args; representing those
    // arg(s) as the alias source lets Step 2's Let/Jump/CFG-merge transitivity
    // propagate the alias through downstream chains without re-coding the
    // worklist. Variants:
    //   - Direct(arg)        → seed alias_sources[apply_dst] = [arg]
    //   - Project { arg, .. } → seed alias_sources[apply_dst] = [arg]
    //                          (variable-level only — field info not tracked
    //                          here; field-level discrimination happens at
    //                          File 13 walk.rs realization)
    //   - Conditional { candidates } → seed alias_sources[apply_dst] = candidates
    //                          (multi-root union — dst aliases ONE of N at
    //                          runtime; analysis keeps ALL parents alive)
    for (apply_dst, alias_source) in apply_result_aliases {
        let roots: ProjectSources = match alias_source {
            ApplyAliasSource::Direct(arg) | ApplyAliasSource::Project { arg, .. } => {
                smallvec![*arg]
            }
            ApplyAliasSource::Wrapped(_) => {
                // BUG-04-118 §05 Round 4 Option B: Wrapped is NOT seeded into
                // project_alias_sources. Containment (`wrap_ok(m) = Ok(m)`)
                // is NOT a projection-derived alias chain — `extracted = inner
                // = Project result.payload[0]` accesses dst's payload via
                // structural projection, but `extracted`'s class must NOT
                // inherit `m`'s alias chain (which would over-suppress
                // extracted's canonical dec, the Round 2 failure mode).
                // Wrapped's only effect is per-var dec-suppression on arg
                // via `should_suppress_apply_aliased_dec`; downstream
                // projections of dst follow Step 1 (Project-from-dst → dst)
                // and Step 2 (Let alias) without a Step 1b containment seed.
                continue;
            }
            ApplyAliasSource::Conditional { candidates } => SmallVec::from_slice(candidates),
        };
        // SSA invariant: each var defined exactly once. An Apply-defined dst
        // cannot also be a Project dst, so this insert never collides with a
        // Step 1 entry. Defensive merge anyway in case of unforeseen overlap.
        merge_sources(&mut alias_sources, *apply_dst, &roots);
    }

    // Step 2: Extend transitively through Let aliases AND Jump arg → block
    // param edges. Both are simple variable renaming (phi-like), so the
    // same fixed-point closure applies.
    //
    // `Let { dst, Var(src) }` where `src` has Project sources means `dst`
    // inherits the same sources. Similarly, `Jump { target, args }` where
    // arg N has Project sources means the target's param N inherits them.
    //
    // At CFG merges, multiple predecessors may jump to the same block with
    // projections from different parent aggregates. The param must map to
    // the UNION of all predecessor sources. Using `Entry::Vacant` (as the
    // old code did) would drop all but the first predecessor — unsound.
    let mut changed = true;
    while changed {
        changed = false;
        for block in &func.blocks {
            // Let aliases within the block body.
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } = instr
                {
                    if let Some(sources) = alias_sources.get(src).cloned() {
                        changed |= merge_sources(&mut alias_sources, *dst, &sources);
                    }
                }
            }

            // BUG-04-118 §05 Round 5 Rule 6 (per aims-rules.md §1.9 nested
            // Project transitivity): `%dst = Project %src.field`, where %src
            // has sources S → `%dst → [%src] ∪ S`. Step 1 only seeded
            // `%dst → [%src]` (the direct value); the transitive ∪ S
            // extension happens here in the fixed-point loop so multi-hop
            // Project chains (e.g., `let inner = Project result.payload[0];
            // let extracted = Let Var(inner); let next = Project extracted.X`)
            // correctly carry the original aggregate's source through.
            //
            // Without this rule, `class_lifetime_extends_past_path_sensitive`
            // cannot see that `next`'s liveness extends `result`'s lifetime,
            // and the m→result edge fires PIN-6 over-suppression on the
            // canonical apply-aliased dec for nested-projection shapes.
            for instr in &block.body {
                if let ArcInstr::Project {
                    dst, value: src, ..
                } = instr
                {
                    if let Some(src_sources) = alias_sources.get(src).cloned() {
                        changed |= merge_sources(&mut alias_sources, *dst, &src_sources);
                    }
                }
            }

            // BUG-04-118 §05 Round 5 Rule 5 (Select alias propagation per
            // aims-rules.md §1.9): DEFERRED. Initial implementation
            // empirically regressed `generics::test_path_sensitive_select_*`
            // tests (12 failures vs 11 baseline). Select propagation
            // over-extends the alias chain in path-sensitive shapes where
            // the operands are mutually-exclusive aliases of different
            // aggregates; the predicate then over-triggers SKIP on edges
            // that should record. Needs narrower trigger condition (e.g.,
            // restrict to Selects whose operands trace to the same parent
            // aggregate). Tracked in §05 HISTORY for follow-up.

            // Jump arg → target block param edges (phi-like renaming).
            if let ArcTerminator::Jump { target, args } = &block.terminator {
                let target_idx = target.index();
                if target_idx < func.blocks.len() {
                    for (i, &arg) in args.iter().enumerate() {
                        if let Some(sources) = alias_sources.get(&arg).cloned() {
                            if let Some(&(param_var, _)) = func.blocks[target_idx].params.get(i) {
                                changed |= merge_sources(&mut alias_sources, param_var, &sources);
                            }
                        }
                    }
                }
            }
        }
    }

    alias_sources
}

/// Merge `new_sources` into the source set for `var`, returning true if
/// any new source was added. Deduplicates via linear scan — source sets
/// are almost always 1–2 elements, so this is cheaper than a `HashSet`.
fn merge_sources(
    alias_sources: &mut FxHashMap<ArcVarId, ProjectSources>,
    var: ArcVarId,
    new_sources: &[ArcVarId],
) -> bool {
    let entry = alias_sources.entry(var);
    match entry {
        std::collections::hash_map::Entry::Vacant(e) => {
            e.insert(SmallVec::from_slice(new_sources));
            true
        }
        std::collections::hash_map::Entry::Occupied(mut e) => {
            let existing = e.get_mut();
            let mut added = false;
            for &src in new_sources {
                if !existing.contains(&src) {
                    existing.push(src);
                    added = true;
                }
            }
            added
        }
    }
}

/// Propagate Project source demand: when a Project result (or its alias)
/// has demand, ALL possible Project sources must also have demand to prevent
/// premature `RcDec`.
///
/// Handles four cases:
/// 1. Direct: `%3 = Project %2.0` in block A, `%3` used in block B
/// 2. Aliased: `%3 = Project %2.0`, `%4 = Let %3`, `%4` used in block B
/// 3. Transitive: chains of `Let` aliases from a Project destination
/// 4. Block params (single predecessor): `Jump block1, args=[%3]`
///
/// Without this, parent aggregates would have no demand at block B's entry,
/// causing edge cleanup to emit premature `RcDec` — use-after-free when
/// the borrowed child is still alive.
///
/// Note: for block params at multi-predecessor merge points, this propagates
/// demand for ALL possible sources (from different predecessors). This is
/// correct for the backward analysis (keeps parent aggregates alive on each
/// predecessor's path) but the emission side must filter: variables that
/// exist only on one predecessor path must NOT get block-level `RcDec` at the
/// merge point. See `emit_dead_at_entry_decs` and `collect_branch_edge_decs`.
pub(super) fn propagate_project_source_demand(
    current: &mut FxHashMap<ArcVarId, AimsState>,
    state_map: &AimsStateMap,
    project_alias_sources: &FxHashMap<ArcVarId, ProjectSources>,
    _block_params: &[(ArcVarId, ori_types::Idx)],
) {
    let mut extra_demand: Vec<(ArcVarId, AimsState)> = Vec::new();
    for (&var, &state) in current.iter() {
        if state.cardinality == Cardinality::Absent {
            continue;
        }
        if let Some(sources) = project_alias_sources.get(&var) {
            let mut src_demand = AimsState {
                access: AccessClass::Borrowed,
                cardinality: state.cardinality,
                ..state
            };
            // Canonicalize: Rule 8 enforces Borrowed → locality <=
            // FunctionLocal. Without this, a returned child's
            // HeapEscaping locality would create an infeasible state.
            src_demand.canonicalize();

            for &source in sources {
                if !state_map.is_excluded(source) {
                    extra_demand.push((source, src_demand));
                }
            }
        }
    }
    for (var, demand) in extra_demand {
        let joined = current
            .get(&var)
            .map_or(demand, |existing| existing.join(&demand));
        current.insert(var, joined);
    }
}
