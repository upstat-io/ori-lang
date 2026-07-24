//! SSA alias equivalence classes.
//!
//! Union-find groups `Let` aliases, jump arguments with block parameters, and
//! `Direct` or `Conditional` apply-result aliases. Project, wrapped, and
//! `Select` values remain separate ownership identities. `class_table` maps
//! non-singleton members, `class_members` supports last-use checks, and
//! `class_apply_alias_source_candidates` preserves directional suppression.
//!
//! # Ordering
//!
//! Apply-result aliases must exist before class construction. The completed
//! table is read-only before backward demand analysis begins.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcTerminator, ArcVarId};

use super::apply_aliases::build_let_alias_map;
use super::state_map::ApplyAliasSource;

/// Returned data of `compute_ssa_alias_classes`.
#[expect(
    clippy::struct_field_names,
    reason = "all fields expose distinct projections of one SSA alias class"
)]
pub(crate) struct SsaAliasClassesOutput {
    pub class_table: FxHashMap<ArcVarId, u32>,
    pub class_members: FxHashMap<u32, FxHashSet<ArcVarId>>,
    pub class_apply_alias_source_candidates: FxHashMap<u32, FxHashSet<ArcVarId>>,
}

/// Compute SSA-alias equivalence classes via union-find over Let-Var aliases,
/// Jump-arg → block-param pairs, and apply-result aliases of `Direct` and
/// `Conditional` shape.
///
/// Returns the three-field `SsaAliasClassesOutput` consumed by
/// `walk_dec.rs::emit_last_use_decs` (class-liveness check + same-instruction
/// batching) and `should_suppress_apply_aliased_dec` (directional metadata).
///
/// Complexity: O(N · α(N) + I) where N = total SSA vars and I = total
/// instructions+terminators.
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
    // excluded per PIN-2 (different RC slot — the apply returns
    // `arg.field`, not `arg` root).
    for (&dst, source) in apply_result_aliases {
        match source {
            ApplyAliasSource::Direct(arg) => uf.union(dst, *arg),
            ApplyAliasSource::Project { .. } | ApplyAliasSource::Wrapped(_) => {}
            ApplyAliasSource::Conditional { candidates } => {
                for &cand in candidates {
                    uf.union(dst, cand);
                }
            }
        }
    }

    // Materialize the three returned fields. Singletons (vars not involved
    // in any union edge) are excluded from class_table — they continue to
    // flow through the existing per-var dec emission path unchanged (no
    // class lookup).
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
    // existing class — keying by source-class is the only way
    // `should_suppress_apply_aliased_dec` can find the source for a Project
    // return.
    for source in apply_result_aliases.values() {
        let source_args: Vec<ArcVarId> = match source {
            ApplyAliasSource::Direct(arg)
            | ApplyAliasSource::Project { arg, .. }
            | ApplyAliasSource::Wrapped(arg) => vec![*arg],
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

    SsaAliasClassesOutput {
        class_table,
        class_members,
        class_apply_alias_source_candidates,
    }
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

    fn union(&mut self, a: ArcVarId, b: ArcVarId) {
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return;
        }
        let rank_a = self.ranks[root_a as usize];
        let rank_b = self.ranks[root_b as usize];
        let (kept, merged) = match rank_a.cmp(&rank_b) {
            std::cmp::Ordering::Less => (root_b, root_a),
            std::cmp::Ordering::Greater => (root_a, root_b),
            std::cmp::Ordering::Equal => {
                self.ranks[root_a as usize] += 1;
                (root_a, root_b)
            }
        };
        self.parents[merged as usize] = kept;
        self.sizes[kept as usize] += self.sizes[merged as usize];
    }

    fn class_size(&self, root: u32) -> usize {
        self.sizes[root as usize] as usize
    }
}
