//! Take-project alias-class computation (TPR-07-011 / TPR-07-016 / TPR-07-017).
//!
//! A **take-project** is a `Project` instruction whose source is a sum
//! type (`Enum`/`Option`/`Result`) and whose projected payload is a
//! unique-owned Box (`Tag::Iterator` or `Tag::DoubleEndedIterator`).
//! The predicate lives in [`super::borrowed_defs::is_take_project`].
//! Once a take-project executes, the source enum has logically given
//! up its payload — any subsequent scope-exit `RcDec` that walks the
//! source's representation would re-free a pointer the consumer
//! already freed (TPR-07-011 double-free).
//!
//! # The fix surface
//!
//! TPR-07-011 used a function-global suppression in `is_owned_at_entry`
//! / `is_owned_for_rc`: if ANY take-project reached a variable in the
//! alias chain, every cleanup site skipped that variable. That was too
//! coarse — TPR-07-016 showed that on runtime paths which bypass the
//! projection (e.g. the `else` arm of an `if` that guards the `match`),
//! the source enum was NEVER dropped and its Box-allocated payload
//! leaked.
//!
//! The fix replaces the global suppression with **per-class CFG
//! reachability**: a block is "bypass-safe for variable `v`" iff it is
//! NEITHER forward- nor backward-reachable from the take-projects that
//! consume `v`'s alias class. On a bypass-safe block, the source enum
//! is still owned AND will never be consumed by `v`'s take-projects on
//! any reachable path → it is the canonical place to emit the
//! scope-exit drop. On non-bypass-safe blocks, existing mechanisms
//! handle cleanup: the take-project's `is_ownership_transfer` check
//! suppresses the source's last-use drop at the `Project` site (TPR-
//! 07-011), and natural scope-exit drops in non-projecting predecessors
//! (e.g., the empty arm of a `match`) cover their own paths.
//!
//! # Per-class partitioning (TPR-07-017)
//!
//! Earlier iterations collapsed every take-project in a function into
//! one global alias class with one global bypass-safe block set. That
//! was correct for functions with a single take-project but broke
//! functions with two unrelated take-projects: a block bypass-safe for
//! source `A` could be forward/backward reachable from unrelated source
//! `B`, so it was no longer "bypass-safe globally" and `A`'s scope-exit
//! drop never fired.
//!
//! The fix is union-find: each take-project source seeds its own
//! component, two seeds end up in the same component iff they share
//! an alias chain (Let alias / Jump-arg propagation). Each component
//! has its own `tp_blocks` set (the take-projects whose source belongs
//! to this component) and its own `bypass_safe_blocks` set (computed
//! as the complement of forward∪backward reachability from THIS
//! component's `tp_blocks`).
//!
//! `is_bypass_safe_for_var(var, blk)` returns true iff `var` is in some
//! class AND `blk` is bypass-safe for that specific class — independent
//! of any other class in the function.
//!
//! # Consumer call sites
//!
//! Two cleanup sites in [`super::dead_cleanup`] need this information:
//!
//! 1. `emit_dead_at_entry_decs` source 1: vars at entry that are
//!    unused-and-dead-at-exit. The in-class branch routes the drop via
//!    `merge_edge_decs` only when the block is bypass-safe for the var.
//!    On non-bypass-safe blocks the in-class branch is skipped and the
//!    `use_info` / `is_ownership_transfer` path takes over.
//!
//! 2. `emit_dead_block_param_decs` (source 2): block params that are
//!    SSA aliases of a take-project source via Jump-arg propagation.
//!    These are SKIPPED entirely (no routing) — routing them via the
//!    merge-block param ID would emit an `RcDec` using a name with no
//!    SSA definition reachable from the predecessor, producing a phi-
//!    dominance violation (the LLVM emitter resolves the param ID to
//!    the merge block's phi). Source 2 doesn't need bypass-safety
//!    info — the natural scope-exit drops on bypass-safe predecessors
//!    cover the param's underlying value.
//!
//! # Reference
//!
//! Codex iteration 6 TPR review finding (TPR-07-016) and iteration 7
//! follow-up (TPR-07-017). Discussion in `plans/repr-opt/section-07-
//! enum-repr.md` §07.R.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_types::Pool;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

use super::borrowed_defs::is_take_project;

/// Per-class take-project information.
struct ClassInfo {
    /// Block indices containing at least one take-project whose
    /// source variable belongs to this connected-component class.
    tp_blocks: Vec<usize>,
    /// Block indices that are NEITHER forward- nor backward-reachable
    /// from `tp_blocks`. Computed only against this class's
    /// take-projects, so a block bypass-safe for class A is independent
    /// of an unrelated class B in the same function.
    bypass_safe_blocks: FxHashSet<usize>,
    /// Subset of [`Self::bypass_safe_blocks`]: the "entry edge" of
    /// the bypass-safe region. A block is a bypass-safe entry iff
    /// it is bypass-safe AND at least one of its CFG predecessors is
    /// NOT bypass-safe (or it has no predecessors). This identifies
    /// the canonical place to emit the scope-exit drop on a bypass
    /// path: exactly once per CFG path, at the moment the source
    /// enum first becomes "definitely not consumed by this class's
    /// take-projects on any reachable path." Downstream bypass-safe
    /// blocks already inherit the dec from upstream and need
    /// nothing.
    bypass_safe_entries: FxHashSet<usize>,
}

/// Per-function take-project facts queried by RC cleanup sites.
pub(crate) struct TakeMoveFacts {
    /// Maps each `ArcVarId` that participates in a take-project alias
    /// class to its class index in [`Self::classes`]. Variables outside
    /// every class are absent from the map.
    var_to_class: FxHashMap<ArcVarId, usize>,
    /// One entry per connected-component class that contains at least
    /// one take-project source.
    classes: Vec<ClassInfo>,
}

impl TakeMoveFacts {
    /// Empty facts — used when the function has no take-projects at
    /// all. Avoids allocating both the alias closure and the
    /// reachability sets.
    pub(crate) fn empty() -> Self {
        Self {
            var_to_class: FxHashMap::default(),
            classes: Vec::new(),
        }
    }

    /// Whether `var` participates in any take-project alias class.
    /// Variables outside every class are cleaned up normally.
    pub(crate) fn is_in_class(&self, var: ArcVarId) -> bool {
        self.var_to_class.contains_key(&var)
    }

    /// The class index of `var` if it participates in any take-project
    /// alias class, otherwise `None`. Cleanup sites use the index to
    /// dedup drops across alias siblings — a single dec on any var in
    /// a class drops the underlying value, so emitting one per alias
    /// would double-free.
    pub(crate) fn class_of(&self, var: ArcVarId) -> Option<usize> {
        self.var_to_class.get(&var).copied()
    }

    /// Whether `blk` is the **entry edge** of the bypass-safe region
    /// for the alias class containing `var`. Returns `false` if
    /// `var` is not in any class.
    ///
    /// A bypass-safe entry is the first block on a CFG path where
    /// the take-project becomes definitively unreachable: it is
    /// bypass-safe AND at least one of its predecessors is NOT
    /// bypass-safe. This is the canonical place to emit a scope-exit
    /// drop for `var` — emitting at every bypass-safe block instead
    /// would produce N duplicate decs for sequential bypass-safe
    /// regions and double-free the shared underlying value.
    pub(crate) fn is_bypass_safe_entry_for_var(&self, var: ArcVarId, blk: usize) -> bool {
        let Some(&idx) = self.var_to_class.get(&var) else {
            return false;
        };
        self.classes[idx].bypass_safe_entries.contains(&blk)
    }
}

/// Compute take-project facts for `func`.
///
/// Returns empty facts when the function has no take-projects,
/// avoiding both the alias closure and the reachability sweep.
pub(crate) fn analyze(func: &ArcFunction, pool: &Pool) -> TakeMoveFacts {
    // 1. Find take-project sites: each is (block_idx, source_var).
    let tp_sites = collect_take_project_sites(func, pool);
    if tp_sites.is_empty() {
        return TakeMoveFacts::empty();
    }

    // 2. Union-find over every alias edge in the function.
    //    Aliases come from `Let { dst, Var(src) }` (bidirectional)
    //    and `Jump arg → block param` (forward). Both relations are
    //    equivalence-class-forming, so union-find captures them
    //    naturally.
    let mut parent: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    union_alias_edges(func, &mut parent);

    // 3. Ensure every take-project source has a singleton class entry,
    //    even when it has no alias edges (a sourceless take-project
    //    still needs a class for the consumer queries).
    for &(_, src) in &tp_sites {
        parent.entry(src).or_insert(src);
    }

    // 4. Group take-project sites by their class representative. Two
    //    sites that share a representative are in the same connected
    //    component → they belong to the same alias class.
    let mut rep_to_idx: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    let mut classes: Vec<ClassInfo> = Vec::new();
    for &(block_idx, src) in &tp_sites {
        let rep = find(&mut parent, src);
        let idx = if let Some(&i) = rep_to_idx.get(&rep) {
            i
        } else {
            let i = classes.len();
            rep_to_idx.insert(rep, i);
            classes.push(ClassInfo {
                tp_blocks: Vec::new(),
                bypass_safe_blocks: FxHashSet::default(),
                bypass_safe_entries: FxHashSet::default(),
            });
            i
        };
        if !classes[idx].tp_blocks.contains(&block_idx) {
            classes[idx].tp_blocks.push(block_idx);
        }
    }

    // 5. Map every in-class variable to its class index. Vars whose
    //    representative is NOT a class representative (i.e., they
    //    aren't reachable from any take-project source via alias
    //    edges) are absent from the map and reported as not-in-class
    //    by `is_in_class`.
    let mut var_to_class: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    let vars: Vec<ArcVarId> = parent.keys().copied().collect();
    for v in vars {
        let rep = find(&mut parent, v);
        if let Some(&idx) = rep_to_idx.get(&rep) {
            var_to_class.insert(v, idx);
        }
    }

    // 6. Compute bypass-safe blocks for each class independently
    //    using CFG reachability from that class's take-project blocks.
    //    Two unrelated classes never share each other's reachability
    //    sets — this is the per-class partitioning that TPR-07-017
    //    requires.
    let predecessors = crate::graph::compute_predecessors(func);
    for class in &mut classes {
        class.bypass_safe_blocks =
            compute_bypass_safe_blocks(func, &class.tp_blocks, &predecessors);
        class.bypass_safe_entries =
            compute_bypass_safe_entries(&class.bypass_safe_blocks, &predecessors);
    }

    TakeMoveFacts {
        var_to_class,
        classes,
    }
}

/// Compute the entry-edge subset of `bypass_safe_blocks`.
///
/// A block is an entry iff it is bypass-safe AND at least one of its
/// CFG predecessors is NOT bypass-safe (or it has no predecessors at
/// all). This identifies the canonical "first block" of each maximal
/// bypass-safe region — the place where the source enum first becomes
/// definitively unreachable from this class's take-projects on the
/// CFG path. RC cleanup emits the scope-exit drop here exactly once;
/// downstream bypass-safe blocks inherit the dec via SSA flow.
fn compute_bypass_safe_entries(
    bypass_safe_blocks: &FxHashSet<usize>,
    predecessors: &[Vec<usize>],
) -> FxHashSet<usize> {
    let mut entries = FxHashSet::default();
    for &b in bypass_safe_blocks {
        let preds = predecessors.get(b).map_or(&[][..], Vec::as_slice);
        let has_non_bypass_pred =
            preds.is_empty() || preds.iter().any(|&p| !bypass_safe_blocks.contains(&p));
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
/// - forward `Jump arg → block param` at matching positions
fn union_alias_edges(func: &ArcFunction, parent: &mut FxHashMap<ArcVarId, ArcVarId>) {
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                union(parent, *dst, *src);
            }
        }
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

/// Compute the bypass-safe block set for one alias class given that
/// class's `tp_blocks`.
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
/// site handles their drop per TPR-07-011, and emitting an entry-time
/// dec there would walk the tagged-pointer encoding before the
/// projection reads its payload.
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
