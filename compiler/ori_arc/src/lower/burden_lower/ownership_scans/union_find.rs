//! Forwarder-identity union-find: unions a forwarder call's consumed arg and
//! result (plus `Let { Var }` aliases and Jump-arg block-param handoffs) into
//! one same-allocation class. Spec: Annex E §AIMS RL-34 + RL-2.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind, LitValue};

use super::forwarder::arg_owned_transfers_through_return;

/// Structural forwarder-identity union-find over Let{Var} aliases (`%6 = %4`) +
/// forwarder result→arg edges (Invoke/Apply `transfers_through_return ∧ Owned`,
/// `%7 = id(%6 [own])`). Mirrors `compute_same_alloc_reps` (`edge_cleanup.rs`) EXCEPT it
/// is structural (no `state_map.apply_result_aliases()` — unavailable at Phase 5) and
/// intentionally EXCLUDES the Jump-arg → block-param edge (a phi merge over DISTINCT
/// allocations is NOT a same-allocation relation). `is_forwarder_rep` records reps that
/// participate in a forwarder edge (the over-fire gate).
pub(super) struct ForwarderUnionFind {
    pub(super) parent: FxHashMap<ArcVarId, ArcVarId>,
    pub(super) is_forwarder_rep: FxHashSet<ArcVarId>,
    /// Vars defined by a sum-aggregate `Construct` (`EnumVariant`) — the FRESH
    /// heap allocation root of an `Option<T>` / `Result<T, E>` / user sum lineage.
    /// Tracked per-var (resolved to rep via `find`) so the construct-fed
    /// dead-block-param scan can gate on "this rep's allocation root is a sum
    /// Construct" without re-walking the body.
    pub(super) sum_aggregate_construct_dsts: FxHashSet<ArcVarId>,
    /// Vars defined by a FRESH heap-buffer allocation — a collection-buffer
    /// `Construct` (`ListLiteral`/`MapLiteral`/`SetLiteral`) OR a `Let { Literal::
    /// String }` (heap str body). Both share the dead-block-param over-emission
    /// shape with the sum-aggregate Construct, but allocate a `CollectionBuffer` /
    /// str body. Tracked per-var (resolved to rep via `find`) so the construct-fed
    /// dead-block-param scan admits a borrowed-Invoke-arg fresh list/map/set/str
    /// whose lineage dies at a merge/return DEAD block-param (the
    /// `catch(expr: callee(coll))` shape). Spec: Annex E §AIMS RL-5 + RL-2.
    pub(super) fresh_collection_construct_dsts: FxHashSet<ArcVarId>,
}

impl ForwarderUnionFind {
    pub(super) fn build(func: &ArcFunction, contracts: &FxHashMap<Name, MemoryContract>) -> Self {
        let mut uf = ForwarderUnionFind {
            parent: FxHashMap::default(),
            is_forwarder_rep: FxHashSet::default(),
            sum_aggregate_construct_dsts: FxHashSet::default(),
            fresh_collection_construct_dsts: FxHashSet::default(),
        };
        // Edge type 1: Let{Var} aliases.
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } = instr
                {
                    uf.union(*dst, *src);
                }
                // Record sum-aggregate `Construct` dsts (the FRESH allocation
                // root of an Option / Result / user sum lineage). Recorded on
                // the raw dst; resolved to rep at query time via `find`.
                if let ArcInstr::Construct {
                    dst,
                    ctor: CtorKind::EnumVariant { .. },
                    ..
                } = instr
                {
                    uf.sum_aggregate_construct_dsts.insert(*dst);
                }
                // Record FRESH heap-buffer dsts: list/map/set literal Constructs
                // (CollectionBuffer) + `Let { Literal::String }` (heap str body) —
                // the same dead-block-param over-emission shape, buffer-rooted.
                if let ArcInstr::Construct {
                    dst,
                    ctor: CtorKind::ListLiteral | CtorKind::MapLiteral | CtorKind::SetLiteral,
                    ..
                } = instr
                {
                    uf.fresh_collection_construct_dsts.insert(*dst);
                }
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Literal(LitValue::String(_)),
                    ..
                } = instr
                {
                    uf.fresh_collection_construct_dsts.insert(*dst);
                }
            }
        }
        // Edge type 4 (forwarder only): Invoke/Apply result `dst` ← every owned
        // transfer-through-return arg. The result IS the forwarded arg's allocation.
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Apply {
                    dst,
                    func: callee,
                    args,
                    ..
                } = instr
                {
                    uf.record_forwarder(contracts, *dst, *callee, args);
                }
            }
            if let ArcTerminator::Invoke {
                dst,
                func: callee,
                args,
                ..
            } = &block.terminator
            {
                uf.record_forwarder(contracts, *dst, *callee, args);
            }
        }
        uf
    }

    /// True iff `rep` (a union-find representative) has a member defined by a
    /// sum-aggregate `Construct` — the construct-fed lineage gate. Resolves every
    /// recorded sum-Construct dst to its rep and checks for a match.
    pub(super) fn is_sum_aggregate_construct_rep(&mut self, rep: ArcVarId) -> bool {
        let dsts: Vec<ArcVarId> = self.sum_aggregate_construct_dsts.iter().copied().collect();
        dsts.into_iter().any(|d| self.find(d) == rep)
    }

    /// True iff `rep`'s allocation root is a FRESH heap-buffer allocation — a
    /// collection-buffer `Construct` (`ListLiteral`/`MapLiteral`/`SetLiteral`) OR
    /// a `Let { Literal::String }` (heap str body).
    pub(super) fn is_fresh_collection_construct_rep(&mut self, rep: ArcVarId) -> bool {
        let dsts: Vec<ArcVarId> = self
            .fresh_collection_construct_dsts
            .iter()
            .copied()
            .collect();
        dsts.into_iter().any(|d| self.find(d) == rep)
    }

    /// Every member var of `rep`'s class, drawn from the recorded edge endpoints
    /// (`parent` keys + values). Used to suppress the spurious keep-alive incs +
    /// misplaced release on the construct-fed dead-param lineage.
    pub(super) fn class_members(&mut self, rep: ArcVarId) -> FxHashSet<ArcVarId> {
        let endpoints: Vec<ArcVarId> = self
            .parent
            .keys()
            .chain(self.parent.values())
            .copied()
            .collect();
        let mut out: FxHashSet<ArcVarId> = FxHashSet::default();
        for v in endpoints {
            if self.find(v) == rep {
                out.insert(v);
            }
        }
        out
    }

    pub(super) fn record_forwarder(
        &mut self,
        contracts: &FxHashMap<Name, MemoryContract>,
        dst: ArcVarId,
        callee: Name,
        args: &[ArcVarId],
    ) {
        for &arg in args {
            if arg_owned_transfers_through_return(contracts, callee, arg, args) {
                self.union(dst, arg);
                let rep = self.find(dst);
                self.is_forwarder_rep.insert(rep);
            }
        }
    }

    pub(super) fn find(&mut self, v: ArcVarId) -> ArcVarId {
        let p = *self.parent.get(&v).unwrap_or(&v);
        if p == v {
            return v;
        }
        let r = self.find(p);
        self.parent.insert(v, r);
        r
    }

    pub(super) fn union(&mut self, a: ArcVarId, b: ArcVarId) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra != rb {
            self.parent.insert(ra, rb);
        }
    }
}
