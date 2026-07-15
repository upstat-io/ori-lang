//! Local jump-threaded same-allocation rep tracking, consumed by codegen's
//! push-result element-escape keep-alive gate.
//!
//! Split out of [`super`] to keep both files under the 500-line hygiene cap.

use rustc_hash::FxHashMap;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

/// Whether `recv`'s jump-threaded collection lineage is RETURNED from `func` —
/// the SAME discriminator the element-escape keep-alive gates on
/// ([`collection_receiver_returned`]). This is a logical element-ownership fact
/// for every physical projection. LLVM currently maps it to the paired
/// elem-header store on push results: the keep-alive inc's balancing release is
/// the receiving collection's `elem_dec_fn`, which exists only when the result
/// buffer's header is populated; an in-scope (non-returned) receiver holds
/// UNFUNDED element views and must NOT dec them at free.
/// Spec: Annex E §AIMS RL-1 + RL-2.
pub fn push_receiver_lineage_returned(func: &ArcFunction, recv: ArcVarId) -> bool {
    let jt_reps = compute_jump_threaded_reps(func, None);
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    collection_receiver_returned(rep_of(recv), func, &rep_of)
}

/// Whether collection-receiver lineage `recv_rep` (a jump-threaded rep of a
/// `@push`/`ori_list_push` arg-0 collection) is RETURNED from the function — its
/// lineage flows to a `Return` value.
///
/// This is the precise over-fire boundary for the element-escape keep-alive
/// ([`emit_iter_element_pushed_into_returned_collection_keepalive_inc`]): the
/// keep-alive is needed ONLY when the receiving collection survives the SOURCE
/// collection's in-function `ori_iter_drop`. A RETURNED collection's drop runs in
/// the CALLER — a SECOND `elem_dec_fn` over the borrowed element backing, distinct
/// from the in-callee `ori_iter_drop`, so the rc-1 backing is decced twice and
/// needs the keep-alive (the receiving collection's `elem_dec_fn` is the inc's
/// balancing release).
///
/// An IN-SCOPE receiver — iterated (`for w in r` lowers to `@iter(r [own])` ->
/// `ori_iter_drop`, an in-scope consume, NOT an escape), or dropped at scope exit —
/// is sequenced with the source iter-drop WITHIN one function, where the base
/// accounting already balances them; a keep-alive there orphans a +1 -> leak. The
/// in-scope `@iter [own]` consume is exactly why this gate is RETURN-reachability,
/// NOT a generic owned-position transfer-out test (which would mis-classify the
/// in-scope iterate as an escape).
///
/// `rep_of` is the jump-threaded rep map: the COW append (`%out = push(r [own],
/// ..)`) re-binds the loop-carried collection to its result `dst`, and the
/// loop-back `Jump` merges `%out` into the receiver's rep, so the finally-returned
/// value shares `recv_rep`. Spec: Annex E §AIMS RL-2 (`RL2_transfer_kinds_no_dec`).
fn collection_receiver_returned(
    recv_rep: ArcVarId,
    func: &ArcFunction,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
) -> bool {
    func.blocks.iter().any(|block| {
        matches!(&block.terminator, ArcTerminator::Return { value } if rep_of(*value) == recv_rep)
    })
}

/// LOCAL jump-threaded same-allocation reps: `Let{Var}` aliases PLUS the
/// Jump-arg→block-param POSITIONAL SSA-rename edges `compute_same_alloc_reps`
/// excludes BY DESIGN. When `apply_direct_seed` is `Some`, the union-find is ALSO
/// seeded with that map's equivalences (the apply-Direct/Conditional transfer
/// edges from `compute_same_alloc_reps`), so a forwarder result (`%dst = Apply
/// @id(%arg [own])` where the callee param `transfers_through_return ∧
/// ReturnAliasShape::Direct`) merges with its transferred owned arg — the result
/// IS the same allocation. With `None` the result is byte-identical to the
/// Let+Jump-only threading.
///
/// Kept local: threading the phi globally would widen `same_alloc_reps`, changing
/// the fresh-inc-elision and alloc-aware-net verdicts that depend on the
/// unthreaded reps. The apply-Direct seed is wired ONLY for the dead-owned-
/// collection pass (the forwarder-RESULT leak), where the threaded rep attributes
/// the result's borrowed-read scope-exit sink to its allocation source.
pub(super) fn compute_jump_threaded_reps(
    func: &ArcFunction,
    apply_direct_seed: Option<&FxHashMap<ArcVarId, ArcVarId>>,
) -> FxHashMap<ArcVarId, ArcVarId> {
    fn find(parent: &mut FxHashMap<ArcVarId, ArcVarId>, v: ArcVarId) -> ArcVarId {
        let p = *parent.get(&v).unwrap_or(&v);
        if p == v {
            return v;
        }
        let r = find(parent, p);
        parent.insert(v, r);
        r
    }
    fn union(parent: &mut FxHashMap<ArcVarId, ArcVarId>, a: ArcVarId, b: ArcVarId) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent.insert(ra, rb);
        }
    }
    let mut parent: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    // Apply-Direct/Conditional transfer-edge seed (forwarder result ↔ owned arg).
    // Reconstructs the `compute_same_alloc_reps` equivalences (member → rep) so a
    // `transfers_through_return ∧ Direct` forwarder result shares its arg's
    // allocation rep. `None` for every caller except the dead-owned-collection
    // pass — see the doc comment. Iterate in SORTED key order: `apply_direct_seed`
    // is an `FxHashMap`, and a HashMap-order union sequence yields a
    // nondeterministic union-find rep (the downstream net + sink would flip across
    // runs — codegen must be deterministic: same inputs, byte-identical output).
    // Sorting fixes the rep.
    if let Some(seed) = apply_direct_seed {
        let mut edges: Vec<(ArcVarId, ArcVarId)> = seed.iter().map(|(&m, &r)| (m, r)).collect();
        edges.sort_unstable();
        for (member, rep) in edges {
            union(&mut parent, member, rep);
        }
    }
    // Edge type 1: Let{Var} aliases.
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                union(&mut parent, *dst, *src);
            }
        }
    }
    // Edge type 2: Jump-arg → successor block-param POSITIONAL rename. This is
    // the edge `compute_same_alloc_reps` omits; threading it locally lets the
    // dead-block-param at the loop-exit trace back to its allocation source.
    for block in &func.blocks {
        let (target, args) = match &block.terminator {
            ArcTerminator::Jump { target, args } => (*target, args),
            _ => continue,
        };
        let Some(succ) = func.blocks.get(target.index()) else {
            continue;
        };
        for (pos, &arg) in args.iter().enumerate() {
            if let Some(&(param, _)) = succ.params.get(pos) {
                union(&mut parent, param, arg);
            }
        }
    }
    let keys: Vec<ArcVarId> = parent.keys().copied().collect();
    let mut reps = FxHashMap::default();
    for v in keys {
        let r = find(&mut parent, v);
        reps.insert(v, r);
    }
    reps
}
