//! Construct-fed dead-param lineage (RL-5 + RL-4 + RL-2): the
//! sum-aggregate-`Construct`-fed dead-block-param release + spurious-op
//! suppression, the nested-construct transitive lineages, and their
//! escape / borrow-view vetting gates. Spec: Annex E §AIMS RL-5 + RL-4 + RL-2.

use std::sync::LazyLock;

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

use super::call_arg_dup::compute_funded_call_arg_dup_aliases;
use super::dead_param::dead_param_single_feeding_rep;
use super::forwarder::{compute_alt_consumer_vars, rep_call_site_count};
use super::function_used_vars;
use super::store_dup::compute_funded_store_dup_aliases;
use super::union_find::ForwarderUnionFind;

/// `ORI_DISABLE_ALT_CONSUMED_DEAD_PARAM_RELEASE=1` declines the ALT-CONSUMED
/// mode of the construct-fed dead-param scan: a lineage with a NON-forwarder
/// owned-transfer consumer reverts to the unconditional gate-(d) decline (the
/// pre-cure arrangement), leaking the Jump-transferred birth reference at the
/// dead merge param when every lineage consume is FUNDED. Bisection surface:
/// isolates a dead-merge-param leak / double-free to the alt-consumed release
/// vs the rest of the Phase-5 walk. Spec: Annex E §AIMS RL-1 + RL-2 + RL-5.
static ALT_CONSUMED_DEAD_PARAM_RELEASE_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_ALT_CONSUMED_DEAD_PARAM_RELEASE").as_deref() == Ok("1")
});

/// Result of [`compute_construct_fed_dead_param_lineage`]: the dead-block-param
/// releases (Part A) + the lineage vars to suppress (Part B).
pub(in crate::lower::burden_lower) struct ConstructFedDeadParamLineage {
    /// `block_idx → [dead block-param var]` — one representative dead-param var
    /// per `(block, rep)` needing exactly ONE RL-5 dead-at-entry `BurdenDec`.
    pub releases: FxHashMap<usize, Vec<ArcVarId>>,
    /// Every var in an admitted construct-fed dead-param lineage class. These
    /// carry spurious keep-alive incs (FRESH-Construct + dup-alias) and a
    /// misplaced release that must be suppressed (removed from
    /// `owned_vars_needing_rc`) so the sole release is the dead-param dec.
    pub suppressed_lineage_vars: FxHashSet<ArcVarId>,
}

/// RL-5 dead-at-entry release for a SUM-AGGREGATE-Construct-fed allocation
/// reaching a merge/return block's DEAD block-params, PLUS the spurious-op
/// suppression that the over-emitting lineage requires.
///
/// Shape (the `for x in Some(str) yield { break }` lineage): a sum-aggregate
/// `Construct` (`%1 = Construct Variant(Option.0)(%0)`, an `Option<str>` owning
/// the heap str `%0`) is threaded — possibly through `Let { Var }` aliases
/// (`%3 = %1`) — as a `Jump` arg to a merge/return block's DEAD block-param
/// (`Jump bb3(%1)` → `%7: str?`, `Cardinality = Absent`). The Jump-arg → Owned-
/// param handoff (RL-4 exemption) defers `%1`'s release to the dead successor
/// param `%7`, which the Phase-5 walk never released → the str backing leaks
/// (`RL5_cleanup_balanced` violated).
///
/// Distinct from [`compute_dead_forwarder_block_param_releases`] in TWO ways:
///   1. The feeding allocation is a `Construct` (not an Invoke/Apply forwarder
///      identity), so the rep is gated by `is_sum_aggregate_construct_rep`, NOT
///      `is_forwarder_rep`.
///   2. The lineage OVER-emits — the Construct gets a FRESH-site `BurdenInc`
///      (TF-3) AND its Let-Var alias gets a dup-alias `BurdenInc` (the
///      `use_counts >= 2` cardinality proxy mis-classes the same-alloc alias
///      `%3 = %1` as a duplication: `%1` is "live" only because it ALSO feeds the
///      `Jump bb3(%1)` handoff), plus a misplaced alias `BurdenDec` in the Some
///      arm. RL-2 (`RL2_release_exactly_once`) requires ONE allocation released
///      EXACTLY once with ZERO keep-alive incs; the lineage's two incs + one
///      misplaced dec net +1 (leak). The cure removes the whole lineage from
///      `owned_vars_needing_rc` (suppressing both incs + the misplaced dec) and
///      supplies the sole release at the dead param `%7`.
///
/// Gates (the over-fire boundary — a double-free is FAR worse than the leak):
///   (a) FRESH heap allocation: the rep's allocation root is a sum-aggregate
///       `Construct` (`is_sum_aggregate_construct_rep`). A non-Construct lineage
///       (forwarder, plain param) is NOT admitted here.
///   (b) Heap element: the Construct dst is in `owned_vars_needing_rc` (the
///       burden machinery proved the sum payload carries RC). An `int?`
///       (`[Scalar]` repr) Construct is absent from `owned_vars_needing_rc` → not
///       admitted (the int variant's burden ops are codegen no-ops anyway, but
///       gating here keeps the suppression scoped to genuine heap lineages).
///   (c) Dead merge/return param: the param is `Cardinality = Absent` (used
///       nowhere) and every feeding `Jump` edge resolves to the ONE rep
///       (`dead_param_single_feeding_rep`).
///   (d) No alternate release — OR the ALT-CONSUMED mode admits: the rep has
///       no member used at a NON-forwarder owned transfer position
///       (`compute_alt_consumer_vars`). When an arm consumed the lineage at an
///       owned call/Construct/Set the per-var path owns THAT release and the
///       dead-param dec would double it — UNLESS every such consume is a
///       FUNDED duplication site (member of the `compute_funded_store_dup_aliases`
///       / `compute_funded_call_arg_dup_aliases` SSOTs: each funded consume
///       carries its OWN kept inc whose matched release is the container drop
///       / consumer), exactly one rep feeds the dead param, no lineage member
///       is used at or forward of the merge block, and no LIVE sibling param
///       of the same block is fed by the same rep. Then the Jump-transferred
///       birth reference is still unmatched (the funded incs balance the
///       consumes, never the birth ref) and the dead param owes its RL-5 dec.
///       The ALT-CONSUMED emission is RELEASES-ONLY: the funded-duplication
///       machinery stays intact (`suppressed_lineage_vars` gains nothing —
///       suppressing a funded store inc would double-free the container's
///       drop). Root-kind gate (a) applies UNCHANGED in this mode: a struct
///       `Construct` (partial-move territory) stays declined. Toggle
///       `ORI_DISABLE_ALT_CONSUMED_DEAD_PARAM_RELEASE=1` restores the
///       unconditional decline.
///
/// SAFE for the both-paths-fail shape (verified `ORI_DISABLE_BURDEN_OPS=1`
/// emits zero Option release): the predicate stack emits no normal-path release
/// for this lineage, so suppressing the burden ops + supplying the dead-param
/// release does not race a predicate-stack release. Spec: Annex E §AIMS RL-5 +
/// RL-4 + RL-2.
pub(in crate::lower::burden_lower) fn compute_construct_fed_dead_param_lineage(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    interner: &ori_ir::StringInterner,
) -> ConstructFedDeadParamLineage {
    let mut uf = ForwarderUnionFind::build(func, contracts);
    let used = function_used_vars(func);
    let alt_consumer_vars = compute_alt_consumer_vars(func, contracts, &mut uf);
    // FUNDED duplication sites (store-family + owned-call-arg SSOTs), computed
    // lazily — only an ALT-CONSUMED candidate pays for the scans.
    let mut funded_sites: Option<FxHashSet<ArcVarId>> = None;

    let mut releases: FxHashMap<usize, Vec<ArcVarId>> = FxHashMap::default();
    let mut suppressed_lineage_vars: FxHashSet<ArcVarId> = FxHashSet::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let mut seen_reps: FxHashSet<ArcVarId> = FxHashSet::default();
        // Collect the dead params + their feeding reps first, then apply gates,
        // to avoid borrow conflicts on `uf` inside the loop.
        let dead_params: Vec<(ArcVarId, ArcVarId)> = block
            .params
            .iter()
            .filter(|(p, _)| !used.contains(p))
            .filter_map(|&(param_var, _)| {
                dead_param_single_feeding_rep(func, &mut uf, block_idx, param_var)
                    .map(|rep| (param_var, rep))
            })
            .collect();
        // The set of ALL reps feeding a DEAD block-param in THIS block. A nested
        // heap-aggregate Construct whose rep is in this set has its own copy dead
        // (used nowhere but its dead-param handoff) — the gate for nested-lineage
        // suppression below (Part B (cont. 2)).
        let block_dead_reps: FxHashSet<ArcVarId> =
            dead_params.iter().map(|&(_, rep)| rep).collect();
        for (param_var, rep) in dead_params {
            let alt_consumed = alt_consumer_vars.contains_key(&rep);
            if alt_consumed && *ALT_CONSUMED_DEAD_PARAM_RELEASE_DISABLED {
                continue;
            }
            // Disjointness gate: a FORWARDER-identity rep is owned by
            // `compute_dead_forwarder_block_param_releases` (which KEEPS the
            // keep-alive inc + adds the dead-param dec — net 0 for a transferred-in
            // allocation whose `+1` came from the forwarded arg). This pass's Part-B
            // suppression would strip that keep-alive inc and double-free. The two
            // passes target DISJOINT reps: forwarder-fed (`@id(x)=x` result) vs
            // construct-fed (a local `Construct` not forwarded through a call). A
            // lineage that is BOTH (a forwarded Construct) stays with the forwarder
            // pass — its `+1` is the forwarded arg's alloc, not a spurious keep-alive.
            if uf.is_forwarder_rep.contains(&rep) {
                continue;
            }
            // Gate (a): rep's allocation root is a FRESH heap Construct — a
            // sum-aggregate `EnumVariant` (Option/Result/user sum) OR a
            // collection-buffer `ListLiteral`/`MapLiteral`/`SetLiteral`. The
            // collection arm cures the borrowed-Invoke-arg fresh collection whose
            // lineage dies at a merge/return DEAD block-param (the
            // `catch(expr: callee(coll))` shape: `%3 = Construct List(..)` borrowed
            // into `Invoke @get(%3 [borrow])`, threaded via the catch's match arms
            // to a dead block-param — the base walk's fresh-site keep-alive inc has
            // no executing-path release, so the buffer leaks). Toggle
            // `ORI_DISABLE_FRESH_COLLECTION_DEAD_PARAM_RELEASE=1` restricts gate (a)
            // to the sum-aggregate root (legacy behaviour). Spec: Annex E §AIMS
            // RL-5 + RL-2.
            let collection_admitted = std::env::var_os("ORI_DISABLE_FRESH_COLLECTION_DEAD_PARAM_RELEASE").is_none()
                    && uf.is_fresh_collection_construct_rep(rep)
                    // Over-fire boundary (collection arm only): the lineage must be
                    // consumed at AT MOST ONE call site. A fresh collection borrowed
                    // into a SECOND call after the catch (`catch(f(coll)); g(coll)`)
                    // is LIVE-ACROSS — the value survives past the dead-param point,
                    // so supplying a dead-param release here frees it before the
                    // second borrowed use → use-after-free / double-free. A
                    // single-call-site lineage genuinely dies at the dead param.
                    // STRUCTURAL discriminator (call-site count, NOT a use-count
                    // cardinality proxy). Spec: Annex E
                    // §AIMS RL-2.
                    && rep_call_site_count(func, &mut uf, rep) <= 1;
            if !uf.is_sum_aggregate_construct_rep(rep) && !collection_admitted {
                continue;
            }
            // Gate (b): heap element — at least one class member carries RC.
            let members = uf.class_members(rep);
            if !members.iter().any(|m| owned_vars_needing_rc.contains(m)) {
                continue;
            }
            if alt_consumed {
                // ALT-CONSUMED mode (gate (d) admission by FUNDED-consume
                // proof): every NON-forwarder owned-transfer consume of the
                // lineage must be a FUNDED duplication site — its kept inc is
                // matched by the consumer's release, so the Jump-transferred
                // birth reference remains the dead param's unmatched RL-5
                // obligation. One unfunded consume = the birth reference was
                // transferred INTO that consumer; releasing the dead param
                // would double-free → decline (the leak persists, never a
                // double-free).
                let funded = funded_sites.get_or_insert_with(|| {
                    let mut f = compute_funded_store_dup_aliases(func, contracts);
                    f.extend(compute_funded_call_arg_dup_aliases(
                        func, contracts, interner,
                    ));
                    f
                });
                let all_consumes_funded = alt_consumer_vars
                    .get(&rep)
                    .is_some_and(|consumes| consumes.iter().all(|v| funded.contains(v)));
                if !all_consumes_funded {
                    continue;
                }
                // No forward use: a lineage member read at or past the merge
                // still observes the allocation the dead-param dec would free.
                if lineage_used_at_or_past_merge(func, &members, block_idx) {
                    continue;
                }
                // No LIVE same-rep sibling param: a live sibling fed by the
                // same rep keeps observing the allocation past the merge — its
                // own release path owns it (`dead_param_single_feeding_rep` is
                // the gate-free position resolver; param liveness is checked
                // here).
                let live_same_rep_sibling = block.params.iter().any(|&(p, _)| {
                    p != param_var
                        && used.contains(&p)
                        && dead_param_single_feeding_rep(func, &mut uf, block_idx, p) == Some(rep)
                });
                if live_same_rep_sibling {
                    continue;
                }
                // RELEASES-ONLY: one RL-5 dec at the dead param; the funded
                // duplication machinery (incs + container-drop releases) stays
                // untouched — `suppressed_lineage_vars` gains nothing.
                if seen_reps.insert(rep) {
                    releases.entry(block_idx).or_default().push(param_var);
                }
                continue;
            }
            if seen_reps.insert(rep) {
                releases.entry(block_idx).or_default().push(param_var);
                // Part B: suppress the spurious keep-alive incs + misplaced
                // release on the whole lineage class. The dead param itself is
                // NOT suppressed — it carries the sole RL-5 release.
                for m in &members {
                    if *m != param_var {
                        suppressed_lineage_vars.insert(*m);
                    }
                }
                // Part B (cont.): the heap ELEMENT borrow-views projected out of
                // the lineage (`%11 = Project %3.1` extracting the `str` payload,
                // plus their `Let { Var }` alias closure `%12 = %11`) are BORROWS
                // of the lineage's heap element (TF-4 `Project` is Borrowed). The
                // lineage's sole release at the dead param frees that element, so a
                // borrow-view release double-frees it. A `Project`-dst escapes
                // `compute_borrowed_projection_dsts` once it is re-aliased by a
                // Let-Var hop. Suppress the projected-element borrow-view closure.
                for view in collect_lineage_element_borrow_views(func, &members) {
                    suppressed_lineage_vars.insert(view);
                }
                // Part B (cont. 2): NESTED heap-aggregate Construct lineages
                // transitively OWNED by this released sum-aggregate. The matched
                // sum payload may itself be a heap Construct (`Some(Node{..})`)
                // that wraps deeper Constructs (`Node{ next: Some(Node{..}) }`).
                // Each such nested Construct is moved into its parent via an OWNING
                // `Construct`-arg (RL-2 transfer), so this sum-aggregate's
                // dead-param release transitively frees it. The base walk
                // nonetheless gives each nested Construct a spurious FRESH-site
                // keep-alive `BurdenInc` (its rep ALSO feeds a Jump-arg to its own
                // dead block-param, so `use_counts >= 2` mis-classes it as a
                // duplication) → +1 leak per nested level. Suppress those whole
                // lineage classes too. NO separate release: this aggregate's dec
                // already owns them (a second dec would double-free — the over-fire
                // boundary). Gate: the nested Construct's rep ALSO feeds a dead
                // block-param in THIS block (`block_dead_reps`), so its own copy is
                // dead (sole escapes = into the released parent + the dead param).
                let nested = collect_nested_construct_owned_dead_lineages(
                    func,
                    &mut uf,
                    &members,
                    &block_dead_reps,
                    rep,
                    owned_vars_needing_rc,
                );
                for v in nested {
                    suppressed_lineage_vars.insert(v);
                }
            }
        }
    }
    ConstructFedDeadParamLineage {
        releases,
        suppressed_lineage_vars,
    }
}

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
fn collect_nested_construct_owned_dead_lineages(
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
fn collect_lineage_element_borrow_views(
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
