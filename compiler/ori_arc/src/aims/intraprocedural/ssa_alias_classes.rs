//! SSA-alias equivalence-class computation (BUG-04-104).
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

use crate::ir::{
    is_transitive_drop_strategy, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership,
    RcStrategy, ValueRepr,
};

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
    /// PIN-6 inter-class payload-of (BUG-04-104 §2.6): class A id → set of
    /// class B ids whose drop transitively covers class A's RC slot.
    pub class_payload_of: FxHashMap<u32, FxHashSet<u32>>,
}

/// Compute SSA-alias equivalence classes via union-find over Let-Var aliases,
/// Jump-arg → block-param pairs, and apply-result aliases of `Direct` and
/// `Conditional` shape, AND the PIN-6 inter-class payload-of relation
/// (BUG-04-104 §2.6).
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
    clippy::cognitive_complexity,
    reason = "pre-existing; refactoring would risk RC emission regressions"
)]
#[expect(
    clippy::too_many_lines,
    reason = "pre-existing; ssa alias class computation is inherently complex"
)]
#[expect(
    clippy::match_same_arms,
    reason = "pre-existing; explicit arms document the intent"
)]
pub(crate) fn compute_ssa_alias_classes(
    func: &ArcFunction,
    apply_result_aliases: &FxHashMap<ArcVarId, ApplyAliasSource>,
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

    // PIN-6 inter-class payload-of population (BUG-04-104 §2.6.2). Walks every
    // block's instructions + terminator. For each Construct/Apply/Invoke/
    // PartialApply/Set whose dst's RcStrategy is transitive-drop, records
    // class(arg) → class(dst) for each [own]-consumed arg. ApplyIndirect /
    // InvokeIndirect are EXCLUDED (no static contract for closure dispatch).
    // Set's `value` is ALWAYS treated as [own]-consumed per R22 conservative
    // heuristic (aims-rules.md §3 TF-15 IA-5 step (1) backward promotion).
    //
    // `dst_strategy_of` returns `None` for scalar dsts (no RC, no transitive
    // drop). Self-loop entries (arg-class == dst-class from `Direct`
    // apply-aliases unioning into one class) are excluded — they reduce to
    // PIN-4 class-liveness and need no PIN-6 suppression.
    let mut class_payload_of: FxHashMap<u32, FxHashSet<u32>> = FxHashMap::default();

    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Construct { dst, args, .. }
                | ArcInstr::PartialApply { dst, args, .. } => {
                    let Some(strat) = dst_strategy_of(func, *dst) else {
                        continue;
                    };
                    if !is_transitive_drop_strategy(strat) {
                        continue;
                    }
                    for arg in args {
                        record_payload_edge(func, &mut uf, *arg, *dst, &mut class_payload_of);
                    }
                }
                ArcInstr::Apply {
                    dst,
                    args,
                    arg_ownership,
                    ..
                } => {
                    let Some(strat) = dst_strategy_of(func, *dst) else {
                        continue;
                    };
                    if !is_transitive_drop_strategy(strat) {
                        continue;
                    }
                    for (i, arg) in args.iter().enumerate() {
                        // Pre-walk default for Apply: arg_ownership empty OR
                        // arg_ownership[i] == Owned (matches `is_owned_position`'s
                        // is_none_or-Owned-default for Apply).
                        let owned = arg_ownership
                            .get(i)
                            .is_none_or(|o| matches!(o, ArgOwnership::Owned));
                        if !owned {
                            continue;
                        }
                        record_payload_edge(func, &mut uf, *arg, *dst, &mut class_payload_of);
                    }
                }
                ArcInstr::Set { base, value, .. } => {
                    let Some(strat) = dst_strategy_of(func, *base) else {
                        continue;
                    };
                    if !is_transitive_drop_strategy(strat) {
                        continue;
                    }
                    record_payload_edge(func, &mut uf, *value, *base, &mut class_payload_of);
                }
                _ => {}
            }
        }
        if let ArcTerminator::Invoke {
            dst,
            args,
            arg_ownership,
            ..
        } = &block.terminator
        {
            if let Some(strat) = dst_strategy_of(func, *dst) {
                if is_transitive_drop_strategy(strat) {
                    for (i, arg) in args.iter().enumerate() {
                        let owned = arg_ownership
                            .get(i)
                            .is_none_or(|o| matches!(o, ArgOwnership::Owned));
                        if !owned {
                            continue;
                        }
                        record_payload_edge(func, &mut uf, *arg, *dst, &mut class_payload_of);
                    }
                }
            }
        }
    }

    // Singleton-parent invariant per R19 Codex F1: ensure `class_members` has
    // an entry for every class id that appears as a parent (set value) in
    // `class_payload_of`. Multi-member parents are populated by the union-find
    // loop above; singleton parents need a defensive insert here so the
    // PIN-6 BFS predicate's `class_members(parent_class)` lookup succeeds.
    let parent_classes: FxHashSet<u32> = class_payload_of.values().flatten().copied().collect();
    for parent_class in parent_classes {
        class_members.entry(parent_class).or_insert_with(|| {
            // Singleton classes: union-find root for a non-unioned var equals
            // the var's index, so `ArcVarId::new(parent_class)` IS the
            // singleton's representative member.
            let mut s = FxHashSet::default();
            s.insert(ArcVarId::new(parent_class));
            s
        });
    }

    // Singleton-child invariant: ensure `class_table` + `class_members` carry
    // entries for every class id that appears as a CHILD (key) in
    // `class_payload_of`. Multi-member children are populated by the
    // union-find loop above; singleton children (e.g., the canonical
    // `apply_alias_result_strmap` `m` and `closure_env_alias` `captured`
    // cases — used once via Apply/PartialApply with no Let-Var/Jump aliases)
    // need a defensive insert so `walk_dec.rs::ssa_alias_class_of(var)`
    // returns `Some(class_id)` and the PIN-6 BFS predicate fires. Without
    // this, singleton sources whose payload-of parents have transitive-drop
    // strategy emit a redundant canonical dec → double-free (the parent's
    // transitive drop already dec's the child's RC slot).
    //
    // Symmetric with the parent population above per the same Round 19
    // Codex F1 reasoning extended to the source side of the relation.
    let child_classes: FxHashSet<u32> = class_payload_of.keys().copied().collect();
    for child_class in child_classes {
        let rep = ArcVarId::new(child_class);
        class_table.entry(rep).or_insert(child_class);
        class_members.entry(child_class).or_insert_with(|| {
            let mut s = FxHashSet::default();
            s.insert(rep);
            s
        });
    }

    SsaAliasClassesOutput {
        class_table,
        class_members,
        class_apply_alias_source_candidates,
        class_payload_of,
    }
}

/// Return `Some(RcStrategy)` for non-scalar `dst`, `None` for scalar or when
/// `var_rc_strategies` has not been populated yet (test fixtures that
/// construct an `ArcFunction` without running `compute_var_reprs`).
///
/// Looks up the cached strategy on `func.var_rc_strategies` — pre-walk does
/// not hold a `&Pool` reference, so the strategy classification must be
/// pre-computed alongside `var_reprs` at the AIMS pipeline's Step 3.
fn dst_strategy_of(func: &ArcFunction, dst: ArcVarId) -> Option<RcStrategy> {
    *func.var_rc_strategies.get(dst.index())?
}

/// Record `class(arg) → class(dst)` in `class_payload_of`, skipping scalar
/// args (no RC slot to share) and self-loop edges (arg-class == dst-class
/// from `Direct` apply-aliases unioning into one class).
fn record_payload_edge(
    func: &ArcFunction,
    uf: &mut UnionFind,
    arg: ArcVarId,
    dst: ArcVarId,
    class_payload_of: &mut FxHashMap<u32, FxHashSet<u32>>,
) {
    if matches!(func.var_reprs.get(arg.index()), Some(&ValueRepr::Scalar)) {
        return;
    }
    let arg_class = uf.find(arg);
    let dst_class = uf.find(dst);
    if arg_class == dst_class {
        return;
    }
    class_payload_of
        .entry(arg_class)
        .or_default()
        .insert(dst_class);
}

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
