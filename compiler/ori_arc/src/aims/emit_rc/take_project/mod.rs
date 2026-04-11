//! Take-project facts: per-var lineage + per-source bypass-safe
//! reachability.
//!
//! A **take-project** is a `Project` instruction whose source is a sum
//! type (`Enum`/`Option`/`Result`) and whose projected payload is a
//! unique-owned Box (`Tag::Iterator` or `Tag::DoubleEndedIterator`).
//! The predicate lives in [`super::borrowed_defs::is_take_project`].
//! Once a take-project executes, the source enum has logically given
//! up its payload — any subsequent scope-exit `RcDec` that walks the
//! source's representation would re-free a pointer the consumer
//! already freed (double-free).
//!
//! # Two decoupled concepts
//!
//! Earlier iterations conflated two distinct notions into a single
//! "take-project class." showed that the conflation is
//! unsound on phi topologies where two unrelated take-project sources
//! merge into a shared block param. The fix splits them:
//!
//! ## 1. Membership class (over-approximating)
//!
//! A [`FxHashSet<ArcVarId>`] of every variable that may be an SSA
//! alias of a take-project source. Computed via a bidirectional
//! union-find over BOTH:
//!
//! - `Let { dst, Var(src) }` edges (true SSA aliases)
//! - `Jump arg → block-param` edges (forward alias flow INCLUDING
//!   multi-predecessor phi merges)
//!
//! The phi-inclusive union is deliberately **over-approximating**: it
//! treats a phi param as "may be aliased with any of its incoming
//! args," even though at runtime only one arg actually flows through
//! per path. Consumers of membership (`is_in_class`) use it as a skip
//! signal — [`super::edge_cleanup::collect_branch_edge_decs`] /
//! [`super::edge_cleanup::collect_invoke_edge_decs`] must skip every
//! in-class var so their edge decs never race source 1's per-lineage
//! emission. A tighter membership would let alias siblings fall out
//! of the skip and produce glibc-detected double-frees
//! (iteration 1 collapsed 9 `iterator_drop` pins this way).
//!
//! ## 2. Per-var lineage (precise forward reachability)
//!
//! For each variable in the membership class, the set of take-project
//! source indices whose runtime value that variable could hold.
//! Computed via forward BFS from each `tp_source` through a directed
//! alias graph:
//!
//! - `Let { dst, Var(src) }` → edge `src → dst`
//! - `Jump { target, args }` → edge `args[i] → target.params[i]`
//!
//! A singleton lineage `{i}` means "this variable definitely holds
//! source i's value at runtime." A mixed lineage `{i, j}` means "this
//! variable is phi-merged and could hold either source i's or source
//! j's value, depending on the runtime path."
//!
//! Lineage is the correct granularity for **bypass-safe emission**:
//! a drop at block `B` for variable `v` is sound iff `B` is bypass-
//! safe for EVERY source in `v`'s lineage. For singleton lineages,
//! this reduces to that source's own bypass-safe set (same as the
//! pre-lineage-split behavior for functions with disjoint alias classes).
//! For mixed lineages, the class-level set is the intersection of
//! per-source sets — emitting only where no source's take-project is
//! reachable on any path.
//!
//! ## Why lineage, not class
//!
//! The earlier class-merged code computed bypass-safe blocks from the
//! merged [`Self::tp_blocks`] of an entire class. That's equivalent
//! to taking the complement of the UNION of reachability from each
//! source's `tp_block`. On a function with two sources unioned via
//! phi, the merged bypass-safe set excludes blocks reachable from
//! EITHER source, which means a block bypass-safe for source `A`
//! alone (but reachable from an unrelated source `B` via phi) is
//! incorrectly excluded — a leak for `A`.
//!
//! Per-source reachability avoids the merge. Each source's own
//! `bypass_safe_blocks` is computed independently. A variable's
//! bypass-safe entries are derived from the intersection of its
//! lineage's per-source sets. For singleton lineages, this is exactly
//! that one source's set — no contamination from unrelated sources.
//!
//! # Consumer API surface
//!
//! Four methods on [`TakeMoveFacts`]:
//!
//! - [`TakeMoveFacts::is_in_class`] — membership check. Used by
//!   [`super::edge_cleanup`] and `emit_dead_block_param_decs` (source
//!   2) to skip every in-class var regardless of lineage.
//! - [`TakeMoveFacts::let_alias_rep`] — Let-alias representative used
//!   by `emit_dead_at_entry_decs` (source 1) for value-identity dedup.
//!   Variables with the same Let-alias rep are connected by a chain of
//!   `Let { dst, Var(src) }` instructions — true SSA aliases holding
//!   the same runtime value. Emitting drops for more than one would
//!   double-free. Phi-merged params with the same lineage source set
//!   but different Let-alias reps get separate drops.
//! - [`TakeMoveFacts::is_bypass_safe_entry_for_var`] — the central
//!   predicate. Returns true iff `blk` is the unique entry edge of
//!   the bypass-safe region for `var`'s lineage. Source 1 emits the
//!   scope-exit drop here exactly once per CFG path; downstream
//!   bypass-safe blocks inherit the dec via SSA flow.
//!
//! # Reference
//!
//!: initial function-global suppression, double-free fix.
//!: conditional-consume path-sensitivity fix.
//!: per-class partitioning via union-find.
//!: membership / lineage split — this module's current
//!   design. See `plans/repr-opt/section-07-enum-repr.md` §07.R.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_types::Pool;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

use super::borrowed_defs::is_take_project;

/// Per-lineage bypass-safe information.
///
/// A "lineage" is the set of take-project source indices that a
/// variable could represent at runtime. One [`LineageInfo`] is shared
/// by every variable with the same lineage set (deduped by
/// [`TakeMoveFacts::var_to_lineage`]). The actual lineage source set
/// is stored as the lookup key in the dedup table during
/// construction; only the precomputed bypass-safe-entry set survives
/// in this struct because that is the only field consumers query.
struct LineageInfo {
    /// Blocks where it is safe to emit a scope-exit drop for a
    /// variable with this lineage. Computed as the entries of the
    /// INTERSECTION of each source's own `bypass_safe_blocks` — a
    /// block where no source in the lineage is reachable from its
    /// take-project in either direction, AND where at least one
    /// predecessor is NOT bypass-safe for the lineage (so the drop
    /// fires exactly once per CFG path at the moment the source
    /// becomes definitively unreachable).
    bypass_safe_entries: FxHashSet<usize>,
}

/// Per-function take-project facts queried by RC cleanup sites.
pub(crate) struct TakeMoveFacts {
    /// Over-approximating membership set: every variable that may be
    /// an SSA alias of a take-project source, including phi-merged
    /// block params. Used by [`super::edge_cleanup`] and
    /// `emit_dead_block_param_decs` to skip all in-class vars so
    /// their edge/param decs never race source 1's per-lineage
    /// emission. The over-approximation is load-bearing:
    /// iteration 1 tried to tighten this and produced nine
    /// double-frees.
    in_class: FxHashSet<ArcVarId>,
    /// Per-variable lineage index (points into [`Self::lineages`]).
    /// Absent for vars outside every take-project class. Two vars
    /// with the same lineage index have the same take-project source
    /// set — they share bypass-safe entry computation. Note: same
    /// lineage does NOT mean same runtime value (phi-merged params
    /// can share a lineage source set but hold different values).
    var_to_lineage: FxHashMap<ArcVarId, usize>,
    /// Per-variable Let-alias representative. Two vars share the same
    /// representative iff connected by a chain of `Let { dst, Var(src) }`
    /// instructions (true SSA aliases — same runtime value). Used by
    /// source-1 dead-entry dedup: only the first var of each Let-alias
    /// group gets an `RcDec`, preventing double-free on alias siblings.
    ///
    /// Total for in-class vars: singletons (vars with no Let aliases)
    /// map to themselves. Absent for vars outside every take-project
    /// class.
    let_alias_rep: FxHashMap<ArcVarId, ArcVarId>,
    /// Deduplicated lineage table. Each entry is shared by every
    /// variable with the same `sources` set. Allocated once per
    /// distinct lineage, not per variable.
    lineages: Vec<LineageInfo>,
}

impl TakeMoveFacts {
    /// Empty facts — used when the function has no take-projects at
    /// all. Avoids allocating the alias closure, lineage BFS, and
    /// per-lineage intersection.
    pub(crate) fn empty() -> Self {
        Self {
            in_class: FxHashSet::default(),
            var_to_lineage: FxHashMap::default(),
            let_alias_rep: FxHashMap::default(),
            lineages: Vec::new(),
        }
    }

    /// Test-only constructor for building `TakeMoveFacts` with specific
    /// bypass-safe entries and let-alias representatives. Used by
    /// consumer-level tests for `emit_dead_at_entry_decs`.
    #[cfg(test)]
    pub(crate) fn with_bypass_entries(
        vars: &[ArcVarId],
        let_reps: &[(ArcVarId, ArcVarId)],
        bypass_safe_entry_blocks: &[usize],
    ) -> Self {
        let in_class: FxHashSet<ArcVarId> = vars.iter().copied().collect();
        let mut var_to_lineage = FxHashMap::default();
        for &var in vars {
            var_to_lineage.insert(var, 0); // all share lineage 0
        }
        let let_alias_rep: FxHashMap<ArcVarId, ArcVarId> = let_reps.iter().copied().collect();
        let lineages = vec![LineageInfo {
            bypass_safe_entries: bypass_safe_entry_blocks.iter().copied().collect(),
        }];
        Self {
            in_class,
            var_to_lineage,
            let_alias_rep,
            lineages,
        }
    }

    /// Whether `var` participates in any take-project alias class.
    /// Uses the over-approximating membership set that includes
    /// phi-merged block params; consumers that need precise per-var
    /// reachability should use
    /// [`Self::is_bypass_safe_entry_for_var`] instead.
    pub(crate) fn is_in_class(&self, var: ArcVarId) -> bool {
        self.in_class.contains(&var)
    }

    /// Let-alias representative of `var`, or `None` if `var` is not in
    /// any take-project class.
    ///
    /// Two vars share the same representative iff connected by a chain
    /// of `Let { dst, Var(src) }` instructions — true SSA aliases that
    /// hold the same runtime value. Used by source-1 dead-entry dedup:
    /// only the first var of each Let-alias group gets an `RcDec`,
    /// preventing double-free on alias siblings.
    ///
    /// Total for in-class vars: singletons map to themselves.
    pub(crate) fn let_alias_rep(&self, var: ArcVarId) -> Option<ArcVarId> {
        self.let_alias_rep.get(&var).copied()
    }

    /// Whether `blk` is the bypass-safe entry for `var`'s lineage.
    /// Returns `false` if `var` is not in any take-project class or
    /// its lineage has no bypass-safe region.
    ///
    /// A bypass-safe entry is the first block on a CFG path where
    /// every take-project source in `var`'s lineage becomes
    /// definitively unreachable (not forward- or backward-reachable
    /// from any such source's take-project block). This is the
    /// canonical place to emit a scope-exit drop for `var`: it fires
    /// exactly once per CFG path, and downstream bypass-safe blocks
    /// inherit the dec via SSA flow.
    ///
    /// For singleton lineages (the common case of a function with
    /// one take-project, or two take-projects with disjoint alias
    /// chains), this query reduces to "is `blk` a bypass-safe entry
    /// for this source's own reachability?" — matching the original
    /// per-class semantics.
    ///
    /// For mixed lineages (phi-merged sources), this
    /// query uses the INTERSECTION of per-source bypass-safe sets.
    /// A contaminated block for even one source excludes the entire
    /// lineage from emitting a drop there; the specific-lineage vars
    /// cover cleanup at their own more-permissive entries instead.
    pub(crate) fn is_bypass_safe_entry_for_var(&self, var: ArcVarId, blk: usize) -> bool {
        let Some(&lineage_idx) = self.var_to_lineage.get(&var) else {
            return false;
        };
        self.lineages[lineage_idx]
            .bypass_safe_entries
            .contains(&blk)
    }
}

/// Compute take-project facts for `func`.
///
/// Returns [`TakeMoveFacts::empty`] when the function has no
/// take-projects, avoiding both the alias closure and the per-source
/// reachability sweep.
pub(crate) fn analyze(func: &ArcFunction, pool: &Pool) -> TakeMoveFacts {
    // 1. Find take-project sites: each is (block_idx, source_var).
    let tp_sites = collect_take_project_sites(func, pool);
    if tp_sites.is_empty() {
        return TakeMoveFacts::empty();
    }

    // 2. Compute CFG predecessors once (shared between membership
    //    union-find via forward-jump edges and per-source bypass-safe
    //    reachability).
    let predecessors = crate::graph::compute_predecessors(func);

    // 3. Build the over-approximating membership set via bidirectional
    //    union-find over both Let-alias and Jump-arg edges. This is the
    //    same structure introduced; only adds a
    //    per-var lineage layer on top, leaving membership unchanged so
    //    edge cleanup's skip invariant survives.
    let in_class = compute_membership(func, &tp_sites);

    // 4. Compute per-source bypass-safe blocks. Each take-project
    //    source gets its OWN bypass-safe set from its own `tp_block`
    //    alone — no cross-source contamination. This is the key
    //    change versus which merged `tp_blocks`
    //    across every source in the same class and took the complement
    //    of the union, shrinking bypass-safe sets incorrectly.
    let source_bypass: Vec<FxHashSet<usize>> = tp_sites
        .iter()
        .map(|&(block_idx, _)| compute_bypass_safe_blocks(func, &[block_idx], &predecessors))
        .collect();

    // 5. Compute per-variable lineage via forward BFS from each
    //    take-project source through the directed alias graph
    //    (`src → dst` for Let aliases, `arg → param[i]` for Jump).
    //    Singleton lineage `{i}` = definitely source `i`; mixed
    //    lineage `{i, j}` = phi-merged.
    let var_to_sources = compute_lineage(func, &tp_sites);

    // 6. Deduplicate lineages and precompute per-lineage bypass-safe
    //    entries. Each unique `sources` set becomes one `LineageInfo`
    //    entry shared by every variable with that lineage.
    let entry_block = func.entry.index();
    let mut lineage_table: FxHashMap<Vec<usize>, usize> = FxHashMap::default();
    let mut lineages: Vec<LineageInfo> = Vec::new();
    let mut var_to_lineage: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    for (var, sources) in var_to_sources {
        let idx = if let Some(&existing) = lineage_table.get(&sources) {
            existing
        } else {
            let bypass_safe_entries = compute_lineage_bypass_safe_entries(
                &sources,
                &source_bypass,
                entry_block,
                &predecessors,
            );
            let new_idx = lineages.len();
            lineages.push(LineageInfo {
                bypass_safe_entries,
            });
            lineage_table.insert(sources, new_idx);
            new_idx
        };
        var_to_lineage.insert(var, idx);
    }

    // 7. Compute Let-alias representatives for source-1 dedup.
    //    Two vars share a rep iff connected by `Let { dst, Var(src) }`
    //    chains only (no Jump edges). This distinguishes "same runtime
    //    value" (Let alias) from "same source set" (lineage) — fixing
    //    where phi-merged params with the same lineage were
    //    incorrectly deduped.
    let let_alias_rep = compute_let_alias_reps(func, &in_class);

    TakeMoveFacts {
        in_class,
        var_to_lineage,
        let_alias_rep,
        lineages,
    }
}

/// Compute the over-approximating membership set.
///
/// A variable is "in class" iff its union-find representative equals
/// some take-project source's representative. The union-find walks
/// BOTH `Let` (bidirectional) and `Jump arg → block-param` (treated
/// as bidirectional for the membership class only; forward-only for
/// lineage below). The Jump edge is intentionally over-approximating
/// — it treats every phi param as aliased with every incoming arg —
/// so that edge cleanup's `is_in_class` skip covers the full alias
/// chain. iteration 1 tried to tighten this to single-
/// predecessor targets only and broke nine regression pins.
fn compute_membership(func: &ArcFunction, tp_sites: &[(usize, ArcVarId)]) -> FxHashSet<ArcVarId> {
    let mut parent: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    union_alias_edges(func, &mut parent);
    // Ensure every take-project source has a singleton entry even
    // when it participates in no alias edges.
    for &(_, src) in tp_sites {
        parent.entry(src).or_insert(src);
    }
    // Collect the representatives of every take-project source.
    let mut tp_source_reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for &(_, src) in tp_sites {
        tp_source_reps.insert(find(&mut parent, src));
    }
    // Any var whose representative matches a take-project rep is in
    // some alias class.
    let mut in_class: FxHashSet<ArcVarId> = FxHashSet::default();
    let vars: Vec<ArcVarId> = parent.keys().copied().collect();
    for v in vars {
        let rep = find(&mut parent, v);
        if tp_source_reps.contains(&rep) {
            in_class.insert(v);
        }
    }
    in_class
}

/// Compute per-variable lineage via BFS from each take-project source
/// through the alias graph.
///
/// Unlike [`compute_membership`]'s fully bidirectional union-find,
/// this graph treats edge kinds asymmetrically:
///
/// - **`Let { dst, value: Var(src) }` is BIDIRECTIONAL** (`src ↔ dst`).
///   `dst` and `src` are SSA-equivalent — they name the same runtime
///   value at every point where both are in scope. The take-project
///   source variable for a `Project` instruction may be the
///   downstream Let alias (`%19 = %5`, `Project %19.1`), in which
///   case the upstream `%5` is the actual enum that needs the
///   bypass-safe scope-exit drop. Forward-only Let propagation would
///   miss `%5` and leak it.
///
/// - **`Jump { target, args }` is FORWARD ONLY**
///   (`args[i] → target.params[i]`). A phi-merge param inherits the
///   lineage of every incoming arg (could-be-either at runtime), but
///   the args do NOT inherit each other's lineage via the phi —
///   that would falsely conflate unrelated `tp_sources`, which is
///   exactly the bug we are fixing.
///
/// Returns a map from each variable to a **sorted, deduplicated**
/// vector of source indices. Variables outside every lineage (no
/// take-project source reaches them) are absent from the map.
fn compute_lineage(
    func: &ArcFunction,
    tp_sites: &[(usize, ArcVarId)],
) -> FxHashMap<ArcVarId, Vec<usize>> {
    // Precompute the alias graph once.
    let alias_graph = build_alias_graph(func);

    // BFS from each take-project source through the alias graph.
    let mut var_to_sources: FxHashMap<ArcVarId, Vec<usize>> = FxHashMap::default();
    for (src_idx, &(_, src_var)) in tp_sites.iter().enumerate() {
        let mut reachable: FxHashSet<ArcVarId> = FxHashSet::default();
        let mut work: Vec<ArcVarId> = vec![src_var];
        while let Some(v) = work.pop() {
            if !reachable.insert(v) {
                continue;
            }
            if let Some(successors) = alias_graph.get(&v) {
                for &s in successors {
                    work.push(s);
                }
            }
        }
        for v in reachable {
            var_to_sources.entry(v).or_default().push(src_idx);
        }
    }

    // Sort + dedup for determinism and stable hashing as lineage keys.
    for lineage in var_to_sources.values_mut() {
        lineage.sort_unstable();
        lineage.dedup();
    }

    var_to_sources
}

/// Enumerate all `Let { dst, Var(src) }` edges in `func`.
///
/// Returns `(src, dst)` pairs — one per Let-alias instruction. This is
/// the SSOT for Let-edge enumeration: both [`build_alias_graph`] (which
/// adds bidirectional Let edges + forward-only Jump edges) and
/// [`compute_let_alias_reps`] (which builds a Let-only union-find)
/// consume this helper to avoid duplicating the iteration over blocks
/// and instructions.
fn collect_let_edges(func: &ArcFunction) -> Vec<(ArcVarId, ArcVarId)> {
    let mut edges = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                edges.push((*src, *dst));
            }
        }
    }
    edges
}

/// Compute Let-alias representatives for in-class variables.
///
/// Two variables share the same representative iff they are connected
/// by a chain of `Let { dst, Var(src) }` instructions — true SSA
/// aliases that hold the same runtime value. Jump-arg → block-param
/// edges are excluded: phi-merged params hold different values on
/// different CFG paths and must NOT share a representative.
///
/// Every in-class variable gets an entry (total for in-class):
/// singletons (vars with no Let aliases within `in_class`) map to
/// themselves, avoiding `unwrap_or(var)` fallback policy at consumers.
fn compute_let_alias_reps(
    func: &ArcFunction,
    in_class: &FxHashSet<ArcVarId>,
) -> FxHashMap<ArcVarId, ArcVarId> {
    // Union-find: parent[v] = v initially (each var is its own rep).
    // Reuses the module-level iterative `find`/`union` (no duplication).
    let mut parent: FxHashMap<ArcVarId, ArcVarId> = in_class.iter().map(|&v| (v, v)).collect();

    // Union over Let edges only — no Jump edges.
    for (src, dst) in collect_let_edges(func) {
        if in_class.contains(&src) && in_class.contains(&dst) {
            union(&mut parent, src, dst);
        }
    }

    // Path-compress all entries to canonical representatives.
    let vars: Vec<ArcVarId> = parent.keys().copied().collect();
    for v in vars {
        find(&mut parent, v);
    }

    parent
}

/// Build the alias graph used by [`compute_lineage`].
///
/// - `Let { dst, value: Var(src) }` produces BIDIRECTIONAL edges
///   `src ↔ dst` (SSA aliases — `dst` and `src` name the same
///   runtime value, so lineage must propagate in both directions).
/// - `Jump { target, args }` produces FORWARD-ONLY edges
///   `args[i] → target.params[i]` (phi params inherit lineage from
///   their incoming args, but args do NOT inherit each other's
///   lineage via the phi — that would conflate distinct
///   take-project sources, which is precisely the bug).
///
/// The Jump-forward-only direction is the critical difference from
/// [`union_alias_edges`], which is fully bidirectional and produces
/// the over-approximating membership set.
fn build_alias_graph(func: &ArcFunction) -> FxHashMap<ArcVarId, Vec<ArcVarId>> {
    let mut graph: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    // Let edges are bidirectional (SSOT: sourced from collect_let_edges).
    for (src, dst) in collect_let_edges(func) {
        graph.entry(src).or_default().push(dst);
        graph.entry(dst).or_default().push(src);
    }
    for block in &func.blocks {
        if let ArcTerminator::Jump { target, args } = &block.terminator {
            let target_idx = target.index();
            if target_idx < func.blocks.len() {
                for (i, &arg) in args.iter().enumerate() {
                    if let Some(&(param, _)) = func.blocks[target_idx].params.get(i) {
                        // Forward-only: phi params inherit from their
                        // incoming args, but the reverse direction
                        // would conflate unrelated tp_sources.
                        graph.entry(arg).or_default().push(param);
                    }
                }
            }
        }
    }
    graph
}

/// Compute the bypass-safe entries for a specific lineage.
///
/// Takes the INTERSECTION of `source_bypass[i]` for every `i ∈
/// lineage`, then computes the entries of that intersection via
/// [`compute_bypass_safe_entries`]. For singleton lineages, this
/// reduces to that one source's own bypass-safe entries (matching
/// behavior for the common case). For mixed lineages
/// (phi-merged), it is strictly tighter than either source's set,
/// preventing contamination from an unrelated source in the same
/// membership class.
fn compute_lineage_bypass_safe_entries(
    lineage_sources: &[usize],
    source_bypass: &[FxHashSet<usize>],
    entry_block: usize,
    predecessors: &[Vec<usize>],
) -> FxHashSet<usize> {
    let mut iter = lineage_sources.iter();
    let Some(&first) = iter.next() else {
        return FxHashSet::default();
    };
    let mut intersection: FxHashSet<usize> = source_bypass[first].clone();
    for &src_idx in iter {
        intersection = intersection
            .intersection(&source_bypass[src_idx])
            .copied()
            .collect();
        if intersection.is_empty() {
            break;
        }
    }
    compute_bypass_safe_entries(entry_block, &intersection, predecessors)
}

/// Compute the entry-edge subset of `bypass_safe_blocks`.
///
/// A block is an entry iff it is bypass-safe AND at least one of its
/// CFG predecessors is NOT bypass-safe (or it has no predecessors at
/// all, or it IS the function entry block). This identifies the
/// canonical "first block" of each maximal bypass-safe region — the
/// place where the bypass-safe region first becomes definitively
/// unreachable from the take-projects of the corresponding lineage
/// on the CFG path. RC cleanup emits the scope-exit drop here
/// exactly once; downstream bypass-safe blocks inherit the dec via
/// SSA flow.
///
/// **Function entry handling**: the function entry block
/// has an implicit "outside caller" predecessor that is non-bypass-safe
/// by definition. Without this special case, a loop-headed function
/// whose entire loop body is bypass-safe would have NO entry block
/// selected: the entry block has at least one predecessor (the loop
/// latch back-edge) and that predecessor is bypass-safe, so the
/// `preds.is_empty() || any non-bypass pred` predicate is false. The
/// reachable bypass-safe region would then receive no class drop at
/// all, with edge cleanup also skipping in-class vars — a real leak.
fn compute_bypass_safe_entries(
    entry_block: usize,
    bypass_safe_blocks: &FxHashSet<usize>,
    predecessors: &[Vec<usize>],
) -> FxHashSet<usize> {
    let mut entries = FxHashSet::default();
    for &b in bypass_safe_blocks {
        let preds = predecessors.get(b).map_or(&[][..], Vec::as_slice);
        let is_function_entry = b == entry_block;
        let has_non_bypass_pred = preds.is_empty()
            || preds.iter().any(|&p| !bypass_safe_blocks.contains(&p))
            || is_function_entry;
        if has_non_bypass_pred {
            entries.insert(b);
        }
    }
    entries
}

/// Collect every take-project `Project` instruction in the function as
/// a `(block_idx, source_var)` pair.
fn collect_take_project_sites(func: &ArcFunction, pool: &Pool) -> Vec<(usize, ArcVarId)> {
    let mut sites = Vec::new();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for instr in &block.body {
            if is_take_project(instr, func, pool) {
                if let ArcInstr::Project { value, .. } = instr {
                    sites.push((block_idx, *value));
                }
            }
        }
    }
    sites
}

/// Walk every alias edge in the function and union the two endpoints
/// in the union-find. Adds vars to `parent` lazily as they appear.
///
/// Alias edges:
/// - bidirectional `Let { dst, value: Var(src) }` (`dst ↔ src`)
/// - bidirectional `Jump arg → block param` at matching positions
///
/// The bidirectional Jump union is DELIBERATELY over-approximating —
/// it collapses every phi-merged param with all of its incoming args,
/// even though at runtime only one arg actually flows through per
/// path. This is load-bearing for [`super::edge_cleanup`]'s
/// `is_in_class` skip: a tighter union lets alias siblings fall out
/// of the skip and race source 1's per-lineage emission, producing
/// glibc-detected double-frees. iteration 1 tightened it
/// to single-predecessor targets only and collapsed nine
/// `iterator_drop` regression pins. The final fix keeps
/// this union unchanged and instead adds per-source lineage tracking
/// (via [`build_alias_graph`] + [`compute_lineage`]) for the
/// bypass-safe query, which is the only consumer that needs precise
/// per-var reachability.
fn union_alias_edges(func: &ArcFunction, parent: &mut FxHashMap<ArcVarId, ArcVarId>) {
    // Let edges: bidirectional (SSOT via collect_let_edges).
    for (src, dst) in collect_let_edges(func) {
        union(parent, dst, src);
    }
    // Jump edges: bidirectional (over-approximating for membership).
    for block in &func.blocks {
        if let ArcTerminator::Jump { target, args } = &block.terminator {
            let target_idx = target.index();
            if target_idx < func.blocks.len() {
                for (i, &arg) in args.iter().enumerate() {
                    if let Some(&(param_var, _)) = func.blocks[target_idx].params.get(i) {
                        union(parent, arg, param_var);
                    }
                }
            }
        }
    }
}

/// Union-find `find` with path compression.
///
/// Returns `v` itself when `v` is not yet in the union-find (treats it
/// as a singleton component). Path compression flattens the tree on
/// access for amortized near-constant lookup.
fn find(parent: &mut FxHashMap<ArcVarId, ArcVarId>, v: ArcVarId) -> ArcVarId {
    if !parent.contains_key(&v) {
        return v;
    }
    // Walk to root.
    let mut root = v;
    while let Some(&p) = parent.get(&root) {
        if p == root {
            break;
        }
        root = p;
    }
    // Path compression: re-walk and point every intermediate node at
    // the root.
    let mut x = v;
    loop {
        let next = match parent.get(&x).copied() {
            Some(p) if p != root => p,
            _ => break,
        };
        parent.insert(x, root);
        x = next;
    }
    root
}

/// Union-find `union`. Lazily inserts both endpoints and links the
/// representative of `a` under the representative of `b`.
fn union(parent: &mut FxHashMap<ArcVarId, ArcVarId>, a: ArcVarId, b: ArcVarId) {
    parent.entry(a).or_insert(a);
    parent.entry(b).or_insert(b);
    let ra = find(parent, a);
    let rb = find(parent, b);
    if ra != rb {
        parent.insert(ra, rb);
    }
}

/// Compute the bypass-safe block set for one take-project source
/// given that source's `tp_block`.
///
/// A block is bypass-safe iff CFG reachability from any take-project
/// in `tp_blocks` does NOT touch it in either direction. The set is
/// the complement of `(forward ∪ backward)` reachability where:
///
/// - Forward reachability is the closure of CFG successor edges from
///   `tp_blocks`. Includes the take-project blocks themselves and
///   every post-projection block.
/// - Backward reachability is the closure of CFG predecessor edges
///   from `tp_blocks`. Includes the take-project blocks themselves
///   and every pre-projection block.
///
/// The take-project blocks themselves are NOT bypass-safe (they're
/// in both reachable sets) — `is_ownership_transfer` at the `Project`
/// site handles their drop per and emitting an entry-time
/// dec there would walk the tagged-pointer encoding before the
/// projection reads its payload.
///
/// **Per-source, not per-class**: requires this function
/// to run against a single source's `tp_block` at a time. Merging
/// multiple sources into one call (as did) produces the
/// complement of the UNION of per-source reachability, which shrinks
/// the bypass-safe set incorrectly when two unrelated sources are
/// unioned via a phi merge.
fn compute_bypass_safe_blocks(
    func: &ArcFunction,
    tp_blocks: &[usize],
    predecessors: &[Vec<usize>],
) -> FxHashSet<usize> {
    // Forward-reachable blocks (post-move).
    let mut forward_reachable: FxHashSet<usize> = FxHashSet::default();
    let mut work: Vec<usize> = tp_blocks.to_vec();
    while let Some(b) = work.pop() {
        if !forward_reachable.insert(b) {
            continue;
        }
        for succ in crate::graph::successor_block_ids(&func.blocks[b].terminator) {
            work.push(succ.index());
        }
    }
    // Backward-reachable blocks (pre-move).
    let mut backward_reachable: FxHashSet<usize> = FxHashSet::default();
    let mut work: Vec<usize> = tp_blocks.to_vec();
    while let Some(b) = work.pop() {
        if !backward_reachable.insert(b) {
            continue;
        }
        if let Some(preds) = predecessors.get(b) {
            for &pred in preds {
                work.push(pred);
            }
        }
    }
    // Bypass-safe = complement of (forward ∪ backward).
    (0..func.blocks.len())
        .filter(|b| !forward_reachable.contains(b) && !backward_reachable.contains(b))
        .collect()
}

#[cfg(test)]
mod tests;
