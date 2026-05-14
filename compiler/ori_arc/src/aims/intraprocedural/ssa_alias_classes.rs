//! SSA-alias equivalence-class computation ( ).
//!
//! Pre-walk pass that computes the `ssa_alias_classes` side-table on
//! [`AimsStateMap`] via union-find over Let-Var alias edges, Jump-arg →
//! block-param edges, and apply-result alias edges of `Direct` and
//! `Conditional` shape. Class membership encodes "these SSA names refer to the
//! same RC slot" — orthogonal to `borrow_sources` (per-Project borrow facts)
//! and to `project_alias_sources` (per-Project transitive alias chains).
//!
//! In-class membership: Let-Var aliases (transitively chained), Jump-arg →
//! block-param pairs, and apply-result aliases whose `ApplyAliasSource` is
//! `Direct(arg)` or `Conditional { candidates }`. NOT in-class:
//! - Project field borrows — different RC slot from the source's root; the
//!   `borrow_sources` side table covers DP-5 disjointness for them.
//! - Select operands — `Select(cond, true_val, false_val)` operands refer to
//!   different runtime RC slots; only one is selected at runtime, the
//!   unselected one needs an independent `RcDec`. Existing compensating `RcInc`
//!   at `walk.rs:141-195` handles Select correctly through per-source dec
//!   independence (PIN-2).
//! - `ApplyAliasSource::Project { arg, field }` — the apply returned `arg.field`
//!   (a different RC slot than `arg`'s root). Same "different RC slot"
//!   architectural concern as Select; unioning would conflate two distinct
//!   RC slots and reproduce the same double-dec / under-dec failure mode
//!   PIN-2 was created to prevent. The directional metadata is still recorded
//!   in `class_apply_alias_source_candidates` (PIN-3) so caller-side
//!   `should_suppress_apply_aliased_dec` continues to suppress the apply-source
//!   role's scope-exit dec.
//!
//! Three returned fields:
//! - `class_table: ArcVarId → u32` — the union-find result, keyed only by vars
//!   that participate in a multi-member class (singletons excluded — see
//!   Round 17 Codex F4 + Gemini F2 in §02 PIN material).
//! - `class_members: u32 → FxHashSet<ArcVarId>` — reverse index. Enables the
//!   PIN-4 class-liveness check `class_members(class_id).any(is_live_after)`
//!   in `walk_dec.rs::emit_last_use_decs`: skip `RcDec` emission unless no class
//!   member is live after the current instruction; emit at the class's
//!   absolute last use.
//! - `class_apply_alias_source_candidates: u32 → FxHashSet<ArcVarId>` — PIN-3
//!   directional metadata, keyed by the SOURCE arg's class. For `Direct` and
//!   `Conditional` shapes, source-class equals destination-class (via union).
//!   For `Project` shape (no union), source-class is the source arg's
//!   pre-existing class — keying by source-class is the only way the
//!   downstream `should_suppress_apply_aliased_dec` helper can find the apply
//!   source for a Project return.
//!
//! Pipeline ordering — PL-5 (no-stale-summary invariant):
//!
//! 1. `populate_apply_result_aliases(func, sigs)` — pre-walk (`apply_aliases.rs`).
//! 2. `compute_ssa_alias_classes(func, &apply_result_aliases)` — pre-walk (this file).
//! 3. `compute_project_alias_sources(func, &apply_result_aliases)` — pre-walk.
//! 4. `analyze_function` worklist — sees fully composed alias graph; backward
//!    walk never reads stale state.
//!
//! Read-only after step 2 completes; matches the `borrow_sources` and
//! `apply_result_aliases` invariants per §1.9 Side-Table Domains.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcTerminator, ArcVarId};
use ori_ir::Name;

use super::apply_aliases::build_let_alias_map;
use super::state_map::ApplyAliasSource;

/// Returned data of `compute_ssa_alias_classes`.
#[expect(
    clippy::struct_field_names,
    reason = "pre-existing; struct field naming is consistent with domain model"
)]
pub(crate) struct SsaAliasClassesOutput {
    pub class_table: FxHashMap<ArcVarId, u32>,
    pub class_members: FxHashMap<u32, FxHashSet<ArcVarId>>,
    pub class_apply_alias_source_candidates: FxHashMap<u32, FxHashSet<ArcVarId>>,
    /// PIN-6 inter-class payload-of (): class A id → set of
    /// class B ids whose drop transitively covers class A's RC slot.
    pub class_payload_of: FxHashMap<u32, FxHashSet<u32>>,
}

/// Compute SSA-alias equivalence classes via union-find over Let-Var aliases,
/// Jump-arg → block-param pairs, and apply-result aliases of `Direct` and
/// `Conditional` shape, AND the PIN-6 inter-class payload-of relation
/// ().
///
/// Returns the four-field `SsaAliasClassesOutput` consumed by
/// `walk_dec.rs::emit_last_use_decs` (class-liveness check + same-instruction
/// batching), `should_suppress_apply_aliased_dec` (directional metadata),
/// and the PIN-6 ancestor-chain BFS in `walk_dec.rs` /
/// `edge_cleanup.rs` / `dead_cleanup`.
///
/// Complexity: O(N · α(N) + I) where N = total SSA vars and I = total
/// instructions+terminators (PIN-6 population pass walks each once).
#[expect(
    clippy::match_same_arms,
    reason = "pre-existing; explicit arms document the intent"
)]
pub(crate) fn compute_ssa_alias_classes(
    func: &ArcFunction,
    apply_result_aliases: &FxHashMap<ArcVarId, ApplyAliasSource>,
    sigs: &FxHashMap<Name, MemoryContract>,
) -> SsaAliasClassesOutput {
    let Ok(var_count) = u32::try_from(func.var_types.len()) else {
        unreachable!("ArcFunction var count exceeds u32::MAX");
    };
    let mut uf = UnionFind::new(var_count);

    // Edge type 1: Let Var aliases (single-hop, chained via union-find).
    let let_aliases = build_let_alias_map(func);
    for (&dst, &src) in &let_aliases {
        uf.union(dst, src);
    }

    // Edge type 2: Jump arg → successor block param.
    for block in &func.blocks {
        if let ArcTerminator::Jump { target, args } = &block.terminator {
            let target_block = &func.blocks[target.index()];
            for (arg, (param_var, _ty)) in args.iter().zip(target_block.params.iter()) {
                uf.union(*arg, *param_var);
            }
        }
    }

    // Edge type 3: Select operands — DROPPED per PIN-2 (different RC slot).

    // Edge type 4: Apply-result aliases. Direct + Conditional union; Project
    // excluded per PIN-2 / Round 10 Codex F3 (different RC slot — the apply
    // returns `arg.field`, not `arg` root).
    for (&dst, source) in apply_result_aliases {
        match source {
            ApplyAliasSource::Direct(arg) => uf.union(dst, *arg),
            ApplyAliasSource::Project { .. } => { /* no union — PIN-2 */ }
            ApplyAliasSource::Wrapped(_) => {
                // BUG-04-118 §05 Round 4 Option B: PIN-2 ANALOGOUS — no union.
                // Wrapped means dst CONTAINS arg as a transitive-drop variant
                // payload (e.g., `wrap_ok(m: T) -> Result<T, E> = Ok(m)`).
                // Result and the wrapped allocation are SEPARATE RC slots
                // (different identity), so unioning their classes would
                // collapse two distinct slots into one and over-suppress
                // downstream Project apply-aliased decs (the Round 2 install
                // failure mode).
            }
            ApplyAliasSource::Conditional { candidates } => {
                for &cand in candidates {
                    uf.union(dst, cand);
                }
            }
        }
    }

    // Materialize the three returned fields. Singletons (vars not involved in
    // any union edge) are excluded from class_table per Round 17 Codex F4 +
    // Gemini F2 — they continue to flow through the existing per-var dec
    // emission path unchanged (no class lookup).
    let mut class_table = FxHashMap::default();
    let mut class_members: FxHashMap<u32, FxHashSet<ArcVarId>> = FxHashMap::default();
    let mut class_apply_alias_source_candidates: FxHashMap<u32, FxHashSet<ArcVarId>> =
        FxHashMap::default();

    for var in 0..var_count {
        let var_id = ArcVarId::new(var);
        let class_id = uf.find(var_id);
        if uf.class_size(class_id) > 1 {
            class_table.insert(var_id, class_id);
            class_members.entry(class_id).or_default().insert(var_id);
        }
    }

    // PIN-3: record source-arg → class metadata, keyed by the SOURCE's class.
    // For Direct and Conditional, source-class equals destination-class via
    // union. For Project (no union), source-class is the source arg's
    // pre-existing class — keying by source-class is the only way
    // `should_suppress_apply_aliased_dec` can find the source for a Project
    // return.
    for source in apply_result_aliases.values() {
        let source_args: Vec<ArcVarId> = match source {
            ApplyAliasSource::Direct(arg) => vec![*arg],
            ApplyAliasSource::Project { arg, .. } => vec![*arg],
            // BUG-04-118 §05 Round 4 Option B: Wrapped contributes its arg
            // as a source candidate so class-keyed lookups (mirror of the
            // per-var `should_suppress_apply_aliased_dec` path) can find
            // arg too. Mirrors Direct/Project's single-arg pattern.
            ApplyAliasSource::Wrapped(arg) => vec![*arg],
            ApplyAliasSource::Conditional { candidates } => candidates.clone(),
        };
        for arg in source_args {
            let source_class = uf.find(arg);
            class_apply_alias_source_candidates
                .entry(source_class)
                .or_default()
                .insert(arg);
        }
    }

    // BUG-04-118 §05.6 — `class_payload_of` is now populated post-convergence
    // by `populate_class_payload_of_with_liveness` using path-sensitive
    // liveness from the converged AimsStateMap. The initial empty map here
    // is overwritten via `set_class_payload_of` at step 4.5 in
    // `analyze_function`. Singleton class materialization is ALSO moved
    // there (via `AimsStateMap::ensure_singleton_class`).
    let class_payload_of: FxHashMap<u32, FxHashSet<u32>> = FxHashMap::default();
    let _ = sigs;

    SsaAliasClassesOutput {
        class_table,
        class_members,
        class_apply_alias_source_candidates,
        class_payload_of,
    }
}

/// Return `Some(RcStrategy)` for non-scalar `dst`, `None` for scalar or when
/// `var_rc_strategies` has not been populated yet (test fixtures that
/// Rank-based union-find with path compression and class-size tracking.
///
/// Operations are amortized O(α(N)) per union/find; `class_size(root)` is O(1).
/// `find` uses path-halving compression; `union` uses rank for tree balance.
struct UnionFind {
    parents: Vec<u32>,
    ranks: Vec<u8>,
    sizes: Vec<u32>,
}

impl UnionFind {
    fn new(parent_count: u32) -> Self {
        Self {
            parents: (0..parent_count).collect(),
            ranks: vec![0; parent_count as usize],
            sizes: vec![1; parent_count as usize],
        }
    }

    fn find(&mut self, var: ArcVarId) -> u32 {
        let Ok(idx) = u32::try_from(var.index()) else {
            unreachable!("ArcVarId index exceeds u32::MAX");
        };
        let mut x = idx;
        while self.parents[x as usize] != x {
            let parent = self.parents[x as usize];
            let grandparent = self.parents[parent as usize];
            self.parents[x as usize] = grandparent;
            x = grandparent;
        }
        x
    }

    #[expect(
        clippy::comparison_chain,
        reason = "pre-existing; if-chain is clearer for this domain logic"
    )]
    fn union(&mut self, a: ArcVarId, b: ArcVarId) {
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return;
        }
        let rank_a = self.ranks[root_a as usize];
        let rank_b = self.ranks[root_b as usize];
        let (kept, merged) = if rank_a < rank_b {
            (root_b, root_a)
        } else if rank_a > rank_b {
            (root_a, root_b)
        } else {
            self.ranks[root_a as usize] += 1;
            (root_a, root_b)
        };
        self.parents[merged as usize] = kept;
        self.sizes[kept as usize] += self.sizes[merged as usize];
    }

    fn class_size(&self, root: u32) -> usize {
        self.sizes[root as usize] as usize
    }
}
