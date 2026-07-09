//! Nested-construct transitive lineages consumed by
//! [`super::compute_construct_fed_dead_param_lineage`]: the transitive
//! nested-heap-aggregate suppression walk, its payload-escapes-live over-fire
//! guard, and the lineage element borrow-view closure. Spec: Annex E §AIMS
//! RL-5 + RL-4 + RL-2.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

use super::super::function_used_vars;
use super::super::union_find::ForwarderUnionFind;

/// The lineage members of every NESTED heap-aggregate `Construct` transitively
/// owned (via owning `Construct`-args + `Let { Var }` aliases) by the admitted
/// sum-aggregate `released_rep`, whose own rep ALSO feeds a dead block-param in
/// the same block (`block_dead_reps`).
///
/// Shape (the `Node { value, next: Option<Node> }` recursion matched out of an
/// `Option`/`Result`): the matched aggregate `%10 = Construct Variant(Option.0)(%9)`
/// (the released `released_rep`) owns `%9 = %7`, where `%7 = Construct Struct(Node)
/// (%4, %6)` is a nested heap Construct that ITSELF feeds a dead block-param
/// (`Jump bb1(.., %7, ..)` → dead `%15`). `%7` in turn owns `%6 = Construct
/// Variant(Option.0)(%5)` owning `%5 = %2 = Construct Struct(Node)(%0, %1)`, also
/// dead-param-fed. The base walk emits a spurious FRESH-site `BurdenInc` on each
/// of `%2`/`%7` (their rep feeds a Jump-arg to a dead param → `use_counts >= 2`
/// dup-proxy) → +1 leak per nested level (`RL2_release_exactly_once` violated:
/// the parent's single release already frees the whole tree).
///
/// Traversal: BFS from `released_rep`'s class members, following OWNING
/// `Construct`-arg edges (`Construct { dst, args }` → each `arg` whose repr is
/// heap and whose rep is in `block_dead_reps`) and `Let { Var }` aliases. Each
/// discovered nested rep contributes its whole class to the suppression set. The
/// `released_rep` itself is excluded (handled by the caller's Part-B). NO release
/// is emitted for the nested reps — the parent's dead-param dec owns them.
///
/// Over-fire boundary (a double-free is FAR worse than the leak):
///   - PAYLOAD-EXTRACTED-LIVE precondition (`released_payload_escapes_live`): if
///     the released aggregate's heap payload is PROJECTED out to a LIVE block-param
///     destination (`Some(node) -> node` extracting + returning the inner heap
///     Node, vs `Some(node) -> node.value` reading only a scalar field), the
///     extracted copy holds a live reference to the same allocation as the nested
///     Construct. The parent's dead-param dec frees the tree AND the live extract's
///     own release frees its copy → double-free. The WHOLE nested suppression is
///     aborted in that case (the parent release + the live extract's release are
///     both correct as-is; the nested keep-alive incs are NOT spurious then).
///   - `block_dead_reps` membership: a nested Construct whose own copy is LIVE
///     (used outside the dead-param handoff) is NOT suppressed — it has a genuine
///     independent reference needing its own release. Only a dead-copy nested
///     Construct (sole escapes = into the released parent + its own dead param) is
///     transitively freed by the parent's dec.
///   - heap repr (`owned_vars_needing_rc` membership of a class member): a scalar
///     nested aggregate carries no RC, nothing to suppress.
///   - owning-`Construct`-arg edges only: a `Project` (Borrowed, TF-4) or a
///     non-owning position does NOT establish transitive ownership; the parent's
///     release does not reach a borrowed view (handled separately by
///     `collect_lineage_element_borrow_views`).
pub(super) fn collect_nested_construct_owned_dead_lineages(
    func: &ArcFunction,
    uf: &mut ForwarderUnionFind,
    released_members: &FxHashSet<ArcVarId>,
    block_dead_reps: &FxHashSet<ArcVarId>,
    released_rep: ArcVarId,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> FxHashSet<ArcVarId> {
    // PAYLOAD-EXTRACTED-LIVE precondition: abort the whole nested suppression when
    // the released aggregate's heap payload escapes live (its `Project` view reaches
    // a live block-param). See the over-fire boundary in the doc comment.
    if released_payload_escapes_live(func, released_members, owned_vars_needing_rc) {
        return FxHashSet::default();
    }
    // Map each var defined by a `Construct` to its owning args (heap-aggregate
    // construction transfers each arg inward). Built once per call.
    let mut construct_owned_args: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Construct { dst, args, .. } = instr {
                construct_owned_args.insert(*dst, args.clone());
            }
        }
    }
    let mut suppressed: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut visited_reps: FxHashSet<ArcVarId> = FxHashSet::default();
    visited_reps.insert(released_rep);
    // BFS frontier of vars whose owning-Construct args we still expand. Seed with
    // every member of the released aggregate's class (the Construct dst + its
    // Let-Var aliases).
    let mut frontier: Vec<ArcVarId> = released_members.iter().copied().collect();
    while let Some(var) = frontier.pop() {
        let Some(args) = construct_owned_args.get(&var).cloned() else {
            continue;
        };
        for arg in args {
            let arg_rep = uf.find(arg);
            if visited_reps.contains(&arg_rep) {
                continue;
            }
            // Gate: the nested arg's rep must ALSO feed a dead block-param in this
            // block (its own copy is dead) AND carry RC (heap aggregate). A live or
            // scalar arg is NOT transitively suppressible.
            let nested_members = uf.class_members(arg_rep);
            let is_dead = block_dead_reps.contains(&arg_rep);
            let is_heap = nested_members
                .iter()
                .any(|m| owned_vars_needing_rc.contains(m));
            if is_dead && is_heap {
                visited_reps.insert(arg_rep);
                for m in &nested_members {
                    suppressed.insert(*m);
                }
                // Recurse into the nested Construct's class to reach deeper levels.
                frontier.extend(nested_members.iter().copied());
            }
        }
    }
    suppressed
}

/// True iff the released aggregate's heap payload is PROJECTED out to a LIVE
/// block-param destination — the `Some(node) -> node` extract-and-return shape
/// (vs `Some(node) -> node.value` reading only a scalar field).
///
/// When the heap payload escapes live, the extracted copy holds a live reference
/// to the SAME allocation as the nested Construct, with its OWN release. Suppressing
/// the nested Construct's keep-alive inc then double-frees (the parent's dead-param
/// dec frees the tree AND the live extract's release frees its copy). This is the
/// over-fire boundary that aborts the whole nested-lineage suppression.
///
/// Detection: a `Project { dst, value }` whose `value` is a released-lineage member
/// AND whose `dst` is a HEAP aggregate (`owned_vars_needing_rc`) AND whose
/// `Let { Var }` / `Jump`-arg closure reaches a block-param that is USED (live). A
/// scalar projection (`Project %node.0` → an `int .value`) is NOT heap, so it does
/// not trip this gate — that is the `node.value` case that stays suppressible.
fn released_payload_escapes_live(
    func: &ArcFunction,
    released_members: &FxHashSet<ArcVarId>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> bool {
    let used = function_used_vars(func);
    // ALL Project views of the released lineage's payload (owned or borrowed — the
    // extracted Node flows out as a borrowed `Project` view re-bound to an OWNED
    // live param). The discriminator is whether the closure reaches a USED param
    // that is ALSO owned (a live heap reference), NOT whether the Project dst
    // itself is owned.
    let mut payload_views: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { dst, value, .. } = instr {
                if released_members.contains(value) {
                    payload_views.insert(*dst);
                }
            }
        }
    }
    if payload_views.is_empty() {
        return false;
    }
    // Forward `Let { Var }` alias + nested-`Project` + `Jump`-arg → block-param
    // closure of the payload views: if any reaches a USED block-param that is ALSO
    // an owned heap var (a live heap extract — `Some(node) -> node`), the payload
    // escapes live. A scalar `.value` projection reaches a USED but NON-owned int
    // param, which does NOT abort (the suppressible `node.value` case).
    loop {
        let mut grew = false;
        for block in &func.blocks {
            for instr in &block.body {
                match instr {
                    ArcInstr::Let {
                        dst,
                        value: ArcValue::Var(src),
                        ..
                    } if payload_views.contains(src) => {
                        if payload_views.insert(*dst) {
                            grew = true;
                        }
                    }
                    // A `Project` of a payload view that is ITSELF a heap aggregate
                    // stays in the closure (the extracted Node's own field, still the
                    // released allocation's identity); a scalar field projection is a
                    // distinct scalar value and drops out of the closure.
                    ArcInstr::Project { dst, value, .. }
                        if payload_views.contains(value) && owned_vars_needing_rc.contains(dst) =>
                    {
                        if payload_views.insert(*dst) {
                            grew = true;
                        }
                    }
                    _ => {}
                }
            }
            if let ArcTerminator::Jump { target, args } = &block.terminator {
                for (pos, &arg) in args.iter().enumerate() {
                    if payload_views.contains(&arg) {
                        if let Some(&(param, _)) = func.blocks[target.index()].params.get(pos) {
                            // A USED (live) destination param that is ALSO an owned
                            // heap var means the heap payload escapes live → abort.
                            if used.contains(&param) && owned_vars_needing_rc.contains(&param) {
                                return true;
                            }
                            if payload_views.insert(param) {
                                grew = true;
                            }
                        }
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }
    false
}

/// The heap-element borrow-view closure of a suppressed construct-fed lineage:
/// every `Project { value, .. }` dst whose `value` is a lineage member (the
/// element extracted out of the Option / sum payload), PLUS the `Let { Var }`
/// alias closure of those projection dsts. These are BORROWS of the lineage's
/// heap element (TF-4), so they carry no release — the lineage's sole dead-param
/// release frees the element. Excluded from `owned_vars_needing_rc` alongside the
/// lineage class itself.
pub(super) fn collect_lineage_element_borrow_views(
    func: &ArcFunction,
    lineage_members: &FxHashSet<ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let mut views: FxHashSet<ArcVarId> = FxHashSet::default();
    // Seed: Project dsts whose projected value is a lineage member.
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { dst, value, .. } = instr {
                if lineage_members.contains(value) {
                    views.insert(*dst);
                }
            }
        }
    }
    if views.is_empty() {
        return views;
    }
    // Fixpoint: a `Let { Var(src) }` whose `src` is already a view makes `dst` a
    // view too (the alias of a borrow is a borrow).
    loop {
        let mut grew = false;
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } = instr
                {
                    if views.contains(src) && views.insert(*dst) {
                        grew = true;
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }
    views
}
