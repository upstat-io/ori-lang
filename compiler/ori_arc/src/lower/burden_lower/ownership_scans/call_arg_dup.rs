//! RL-1 owned-call-arg duplication scans: the genuine-duplication
//! OWNED-CALL-ARG alias admission (the multi-fork COW-lineage cure) and the
//! call-result-aggregate element FINAL-READ release designation. Spec:
//! Annex E §AIMS RL-1 + RL-2.

use std::sync::LazyLock;

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::dimensions::AccessClass;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr};

use super::dup_inc::{collect_use_sites, is_burden_op};
use super::successor_reachable_blocks;

/// `ORI_DISABLE_OWNED_CALL_ARG_DUP_INC=1` restores the symmetric-cancellation
/// treatment for a `Let { Var(src) }` dup-alias consumed at an OWNED call-arg
/// position while `src` stays live ([`compute_genuine_dup_call_arg_aliases`]
/// returns empty). Default (unset): the alias keeps its RL-1 duplication
/// `BurdenInc` — each owned-call-arg fork is funded by its own surviving inc
/// whose matched release is the consumer's (`RL1_duplication_balanced`).
/// Bisection surface: isolates a multi-fork COW-lineage double-free / wrong
/// value to the kept duplication inc vs the rest of the Phase-5 walk.
/// Spec: Annex E §AIMS RL-1 + RL-2.
static OWNED_CALL_ARG_DUP_INC_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_OWNED_CALL_ARG_DUP_INC").as_deref() == Ok("1"));

/// `ORI_DISABLE_RESULT_ELEM_FINAL_READ_RELEASE=1` skips the FINAL-READ release
/// designation for multi-read call-result-aggregate elements
/// ([`compute_call_result_element_final_read_releases`] returns empty),
/// restoring the keep-alive-pair arrangement (one leaked element per
/// multi-read lineage). Bisection surface: isolates a returned-tuple element
/// leak / double-free to the designated release vs the rest of the Phase-5
/// walk. Spec: Annex E §AIMS RL-2.
static RESULT_ELEM_FINAL_READ_RELEASE_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_RESULT_ELEM_FINAL_READ_RELEASE").as_deref() == Ok("1")
});

/// Owned-call-arg consume positions admissible as RL-1 duplication chain
/// endpoints: `(var, is_rc_reading)` membership for every var consumed at an
/// OWNED arg position of an `Apply` / `ApplyIndirect` body instruction or an
/// `Invoke` / `InvokeIndirect` terminator.
///
/// "Owned" is BOTH structural (`is_owned_position` over the Phase-5
/// `arg_ownership` annotations — the builtin COW receivers `push` / `insert` /
/// `set` / `remove` carry `[own]` at lowering) AND contract-derived
/// (`ParamContract.access == Owned` — a user callee's call-site annotation is
/// still the pre-`realize_rc_reuse` borrowed default at Phase 5, while borrow
/// inference already proved the param Owned).
///
/// EXCLUSIONS (each a recorded over-fire fence):
/// - the `iter` protocol builtin's owned args (iter-consume transfer — its own
///   RL-2 accounting; admitting it over-counts +1 per loop on the iterator
///   corpus);
/// - `__`-prefixed protocol builtins (`__iter_next`, `__index` — compiler-
///   interceptable with their own ownership protocol);
/// - positions whose `ParamContract.iter_consumes` is set (user-callee
///   iter-consume transfer, same fence as the `iter` builtin).
fn collect_owned_call_arg_consumes(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    let iter_name =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Iter.name());
    let callee_excluded = |callee: Name| -> bool {
        callee == iter_name
            || interner
                .try_lookup(callee)
                .is_none_or(|n| n.starts_with("__"))
    };
    // KNOWN builtins carry precise structural `[own]`/`[borrow]` call-site
    // annotations from lowering (`BuiltinOwnershipSets`) — the contract-derived
    // borrowed-consuming class applies ONLY to user callees (their call-site
    // annotation is the pre-`realize` borrowed default; seeded builtin
    // contracts leave `borrowed_read_only` at its conservative `false` default,
    // which would over-admit pure-read receivers like `@len`/`@concat`).
    let builtins = crate::borrow::BuiltinOwnershipSets::new(interner);
    let contract_owned = |callee: Name, pos: usize| {
        contract_consuming_arg_position(contracts, &builtins, interner, callee, pos)
    };
    // Per-position exclusion fences applying to BOTH the structural and the
    // contract-derived admission: iter-consuming positions (own RL-2
    // accounting) and `transfers_through_return` PASS-THROUGH positions (a
    // ttr forwarder hands the same allocation back through the result — the
    // lineage continues per RL-34; the hand-in is never a consumed
    // duplication).
    let contract_excluded = |callee: Name, pos: usize| -> bool {
        contracts.get(&callee).is_some_and(|c| {
            c.params
                .get(pos)
                .is_some_and(|p| p.iter_consumes || p.transfers_through_return)
        })
    };
    let mut consumed: FxHashSet<ArcVarId> = FxHashSet::default();
    // Named-callee admission: structural `[own]` OR contract-proven Owned,
    // minus the iter-consume / protocol-builtin fences. `args` are positional
    // user args for `Apply` / `Invoke`, so the arg index IS the param index.
    let admit_named = |out: &mut FxHashSet<ArcVarId>,
                       callee: Name,
                       args: &[ArcVarId],
                       owned_at: &dyn Fn(usize) -> bool| {
        if callee_excluded(callee) {
            return;
        }
        for (pos, &arg) in args.iter().enumerate() {
            if contract_excluded(callee, pos) {
                continue;
            }
            if owned_at(pos) || contract_owned(callee, pos) {
                out.insert(arg);
            }
        }
    };
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Apply {
                    func: callee, args, ..
                } => {
                    admit_named(&mut consumed, *callee, args, &|pos| {
                        instr.is_owned_position(pos)
                    });
                }
                // Indirect calls have no callee identity (no contract, no
                // protocol-builtin risk): structural owned positions only.
                // `used_vars()` is `[closure, ...args]`, position 0 borrowed.
                ArcInstr::ApplyIndirect { .. } => {
                    for (pos, &var) in instr.used_vars().iter().enumerate() {
                        if instr.is_owned_position(pos) {
                            consumed.insert(var);
                        }
                    }
                }
                _ => {}
            }
        }
        match &block.terminator {
            ArcTerminator::Invoke {
                func: callee, args, ..
            } => {
                let term = &block.terminator;
                admit_named(&mut consumed, *callee, args, &|pos| {
                    term.is_owned_position(pos)
                });
            }
            ArcTerminator::InvokeIndirect { .. } => {
                let term = &block.terminator;
                for (pos, &var) in term.used_vars().iter().enumerate() {
                    if term.is_owned_position(pos) {
                        consumed.insert(var);
                    }
                }
            }
            _ => {}
        }
    }
    consumed
}

/// Genuine-duplication OWNED-CALL-ARG aliases per AIMS RL-1: `Let { Var(src) }`
/// dup-aliases whose single-use move chain terminates at an owned call-arg
/// consume ([`collect_owned_call_arg_consumes`]) while a use of `src` is
/// forward-REACHABLE past the alias site — the call consumes a real SECOND
/// reference to `src`'s allocation while the first stays live, so the
/// alias-site `BurdenInc` is LOAD-BEARING (`RL1_duplication_balanced`: the
/// consumer's release — the COW helper's copy-source dec, the callee's
/// owned-param release — is the matched release). The kept inc MUST escape the
/// `emit_terminator_burden_decs` symmetric same-block cancellation AND the
/// Phase-5 inc-suppression scans; consumers gate those on this set.
///
/// The call-arg twin of [`compute_genuine_dup_move_aliases`] (which admits
/// ONLY aggregate-STORE endpoints): same dup-alias classification, same
/// forward-reachability discriminator (branch-exclusive aliases decline; loop
/// back-edges admit), same `full_move_vars` exclusion — only the chain
/// endpoint differs. The iter-consume / protocol-builtin fences live in the
/// endpoint collection.
///
/// Standalone-callable (recomputes its inputs from `func`): the Phase-6
/// pair-atomic collector (`aims::realize::burden_elim`) and the Phase-7
/// lineage-net machinery (`aims::realize::emit_unified`) consume the same set
/// — ONE SSOT for the owned-call-arg duplication family. Empty when
/// `ORI_DISABLE_OWNED_CALL_ARG_DUP_INC=1`.
pub(crate) fn compute_genuine_dup_call_arg_aliases(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    if *OWNED_CALL_ARG_DUP_INC_DISABLED {
        return FxHashSet::default();
    }
    let mut use_counts: FxHashMap<ArcVarId, u32> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if is_burden_op(instr) {
                continue;
            }
            for &v in &instr.used_vars() {
                *use_counts.entry(v).or_default() += 1;
            }
        }
        for v in block.terminator.used_vars() {
            *use_counts.entry(v).or_default() += 1;
        }
    }
    let use_sites = collect_use_sites(func);
    let (alias_next, _) = super::dup_inc::collect_move_edges_and_store_consumes(func);
    let call_consumed = collect_owned_call_arg_consumes(func, contracts, interner);
    if call_consumed.is_empty() {
        return FxHashSet::default();
    }
    // Walk the single-use move chain from `dst`; true iff it ends at an owned
    // call-arg consume (mirrors `chain_ends_in_store`).
    let chain_ends_in_call = |start: ArcVarId| -> bool {
        let mut cur = start;
        for _ in 0..func.var_types.len() {
            if use_sites.get(&cur).map(Vec::len) != Some(1) {
                return false;
            }
            if call_consumed.contains(&cur) {
                return true;
            }
            match alias_next.get(&cur) {
                Some(&next) => cur = next,
                None => return false,
            }
        }
        false
    };
    // Per-var defining block: instruction dsts + block params. The
    // forward-reachability discriminator CUTS at the source's defining block —
    // re-reaching the definition via a back-edge re-DEFINES the binding (a
    // loop-variant reassignment `xs = xs.push(i)` threads the CONSUME RESULT
    // into the next iteration's block-param; the re-reached "use" belongs to
    // the NEW binding, not the consumed allocation). A loop-INVARIANT source
    // (defined OUTSIDE the cycle, e.g. `base` forked per iteration by
    // `base.set(0, i)`) is unaffected — its defining block is never re-reached
    // from the alias's successors, so in-loop and post-loop uses still admit.
    let mut def_block: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for &(param, _) in &block.params {
            def_block.insert(param, block_idx);
        }
        for instr in &block.body {
            if let Some(dst) = instr.defined_var() {
                def_block.insert(dst, block_idx);
            }
        }
    }
    let mut reachable_cache: FxHashMap<(usize, Option<usize>), FxHashSet<usize>> =
        FxHashMap::default();
    let mut genuine: FxHashSet<ArcVarId> = FxHashSet::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            else {
                continue;
            };
            // Dup-alias classification: `src` stays live (>= 2 used-var
            // positions). The Phase-5 `dup_alias_dsts` refinement
            // (forwarder-transparent exclusion) is param-rooted, disjoint
            // from the fresh-local call-arg family admitted here.
            if use_counts.get(src).copied().unwrap_or(0) < 2 || !chain_ends_in_call(*dst) {
                continue;
            }
            let Some(sites) = use_sites.get(src) else {
                continue;
            };
            let cut = def_block.get(src).copied();
            let reachable = reachable_cache
                .entry((block_idx, cut))
                .or_insert_with(|| successor_reachable_blocks_with_cut(func, block_idx, cut));
            let src_used_after = sites.iter().any(|&(ub, ui)| {
                (ub == block_idx && ui > instr_idx) || (reachable.contains(&ub) && cut != Some(ub))
            });
            if src_used_after {
                genuine.insert(*dst);
            }
        }
    }
    genuine
}

/// Forward-reachable block set from `start`'s SUCCESSORS, NOT expanding
/// through `cut` (the source's defining block): blocks beyond a re-definition
/// belong to the NEXT binding instance. `cut` itself may appear in the set
/// (callers exclude its use sites separately).
fn successor_reachable_blocks_with_cut(
    func: &ArcFunction,
    start: usize,
    cut: Option<usize>,
) -> FxHashSet<usize> {
    let mut visited: FxHashSet<usize> = FxHashSet::default();
    let mut stack: Vec<usize> = func
        .blocks
        .get(start)
        .map(|b| {
            crate::graph::successor_block_ids(&b.terminator)
                .into_iter()
                .map(crate::ir::ArcBlockId::index)
                .collect()
        })
        .unwrap_or_default();
    while let Some(b) = stack.pop() {
        if !visited.insert(b) {
            continue;
        }
        if cut == Some(b) {
            continue;
        }
        if let Some(block) = func.blocks.get(b) {
            for s in crate::graph::successor_block_ids(&block.terminator) {
                stack.push(s.index());
            }
        }
    }
    visited
}

/// FINAL-READ release designation for multi-read elements of a caller-owned
/// call-result aggregate (RL-2 `RL2_release_exactly_once`).
///
/// Shape: `let (a, b) = make(); ... a.len() ... a[0]` — the call result `%0`
/// is an `Aggregate`-repr tuple carrying NO burden ops (tuple types compose no
/// whole-var burden, so the aggregate emits no `AggFields` drop), its element
/// `Project` dsts are borrowed views (excluded from `owned_vars_needing_rc`),
/// and every read goes through a fresh `Let { Var }` alias of the projection.
/// A SINGLE-read element's alias keeps a lone last-use `BurdenDec` (the
/// element's release). A MULTI-read element's aliases are dup-aliases
/// (`use_counts >= 2`): each gets a net-0 keep-alive pair (or no ops at all
/// for a sole-use borrowed-`Invoke`-arg alias), so the element's caller-owned
/// reference is NEVER released — one leaked element per multi-read lineage.
///
/// The cure designates the lineage's EXECUTION-FINAL read alias as the
/// release carrier: returned aliases are (a) re-included in
/// `owned_vars_needing_rc` (the borrowed-arg-alias exclusion would drop a
/// final `@len`-read alias) and (b) inserted into `inc_suppressed_vars` (the
/// dup-alias keep-alive inc is suppressed), leaving exactly ONE last-use
/// `BurdenDec` on the lineage — the transferred-in element's single release.
///
/// Decline fences (each leaves the lineage on today's arrangement):
/// - the call result has no contract, a `transfers_through_return` param (the
///   result aliases an arg — not a fresh acquisition), or a non-`Unique`
///   return (the caller may not own the only reference);
/// - the aggregate base or the projection carries burden ops of its own
///   (membership in `owned_vars_needing_rc` — a whole-var release exists, a
///   designated alias dec would double-free);
/// - any lineage member is consumed at an owned position, returned, or passed
///   as a `Jump` arg (the element escapes — its release belongs elsewhere);
/// - a use of the projection that is not a `Let { Var }` alias;
/// - no UNIQUE execution-final alias (loop-carried reads re-reach every site
///   via the back-edge; branch forks have per-arm finals) — conservative.
pub(in crate::lower::burden_lower) fn compute_call_result_element_final_read_releases(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> FxHashSet<ArcVarId> {
    if *RESULT_ELEM_FINAL_READ_RELEASE_DISABLED {
        return FxHashSet::default();
    }
    // Fresh caller-owned call-result aggregates: contract-vetted Apply /
    // Invoke dsts with Aggregate repr and no whole-var burden membership.
    let fresh_result = |callee: Name, dst: ArcVarId| -> bool {
        matches!(func.var_repr(dst), Some(ValueRepr::Aggregate))
            && !owned_vars_needing_rc.contains(&dst)
            && contracts.get(&callee).is_some_and(|c| {
                c.return_info.uniqueness == crate::aims::lattice::Uniqueness::Unique
                    && !c.params.iter().any(|p| p.transfers_through_return)
            })
    };
    let mut fresh_aggregates: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst, func: callee, ..
            } = instr
            {
                if fresh_result(*callee, *dst) {
                    fresh_aggregates.insert(*dst);
                }
            }
        }
        if let ArcTerminator::Invoke {
            dst, func: callee, ..
        } = &block.terminator
        {
            if fresh_result(*callee, *dst) {
                fresh_aggregates.insert(*dst);
            }
        }
    }
    if fresh_aggregates.is_empty() {
        return FxHashSet::default();
    }
    // Element projections of those aggregates: RcPtr/FatValue borrowed views
    // with no burden membership of their own.
    let mut elem_sources: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { dst, value, .. } = instr {
                if fresh_aggregates.contains(value)
                    && !owned_vars_needing_rc.contains(dst)
                    && matches!(
                        func.var_repr(*dst),
                        Some(ValueRepr::RcPointer | ValueRepr::FatValue)
                    )
                {
                    elem_sources.insert(*dst);
                }
            }
        }
    }
    if elem_sources.is_empty() {
        return FxHashSet::default();
    }
    let use_sites = collect_use_sites(func);
    let (aliases_of, declined) = vet_element_read_lineages(func, &elem_sources);
    // Execution-final unique alias per multi-read lineage.
    let mut reachable_cache: FxHashMap<usize, FxHashSet<usize>> = FxHashMap::default();
    let mut releases: FxHashSet<ArcVarId> = FxHashSet::default();
    for (&src, aliases) in &aliases_of {
        if declined.contains(&src) {
            continue;
        }
        // Multi-read lineages only: the single-read alias already carries its
        // lone last-use dec on today's arrangement.
        if use_sites.get(&src).map_or(0, Vec::len) < 2 || aliases.len() < 2 {
            continue;
        }
        // Lineage use-site union (source + aliases).
        let mut lineage_sites: Vec<(usize, usize)> = Vec::new();
        for member in std::iter::once(&src).chain(aliases.iter()) {
            if let Some(sites) = use_sites.get(member) {
                lineage_sites.extend(sites.iter().copied());
            }
        }
        let mut finals: Vec<ArcVarId> = Vec::new();
        for &a in aliases {
            let Some(sites) = use_sites.get(&a) else {
                continue;
            };
            // The alias's LAST use site: later-in-block beats earlier;
            // terminator sites are usize::MAX. Cross-block ordering is settled
            // by the execution-final check below, not here.
            let Some(&(lb, li)) = sites.last() else {
                continue;
            };
            let reachable = reachable_cache
                .entry(lb)
                .or_insert_with(|| successor_reachable_blocks(func, lb));
            let used_after = lineage_sites
                .iter()
                .any(|&(ub, ui)| (ub == lb && ui > li) || reachable.contains(&ub));
            if !used_after {
                finals.push(a);
            }
        }
        if let [one] = finals.as_slice() {
            releases.insert(*one);
        }
    }
    releases
}

/// Per-source alias collection + decline vetting for
/// [`compute_call_result_element_final_read_releases`]: returns
/// `(aliases_of, declined)` — the `Let { Var }` read aliases per element
/// source, plus the sources DECLINED by an escaping shape (a non-alias source
/// use, an owned-position alias consume, a `Set.value`, a re-alias of an
/// alias, or any member at a Return / Jump / owned terminator position).
fn vet_element_read_lineages(
    func: &ArcFunction,
    elem_sources: &FxHashSet<ArcVarId>,
) -> (FxHashMap<ArcVarId, Vec<ArcVarId>>, FxHashSet<ArcVarId>) {
    // Per-source alias collection + vetting.
    let mut aliases_of: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    let mut alias_src: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    let mut declined: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if is_burden_op(instr) {
                continue;
            }
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                if elem_sources.contains(src) {
                    aliases_of.entry(*src).or_default().push(*dst);
                    alias_src.insert(*dst, *src);
                    continue;
                }
                // A re-alias of an alias hides downstream reads from the
                // lineage-site union — decline the lineage (conservative).
                if let Some(&s) = alias_src.get(src) {
                    declined.insert(s);
                }
            }
            // A non-alias use of a source declines its lineage; an
            // owned-position consume of an alias declines its lineage.
            for (pos, &v) in instr.used_vars().iter().enumerate() {
                if elem_sources.contains(&v) {
                    declined.insert(v);
                }
                if let Some(&s) = alias_src.get(&v) {
                    if instr.is_owned_position(pos) {
                        declined.insert(s);
                    }
                }
            }
            if let ArcInstr::Set { value, .. } = instr {
                if let Some(&s) = alias_src.get(value) {
                    declined.insert(s);
                }
            }
        }
        // Terminator vetting: a lineage member at a Return / Jump / owned
        // Invoke position escapes — decline. Borrowed Invoke args are reads.
        let term = &block.terminator;
        let term_is_call = matches!(
            term,
            ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. }
        );
        for (pos, &v) in term.used_vars().iter().enumerate() {
            let member_src = if elem_sources.contains(&v) {
                Some(v)
            } else {
                alias_src.get(&v).copied()
            };
            if let Some(s) = member_src {
                if elem_sources.contains(&v) || !term_is_call || term.is_owned_position(pos) {
                    declined.insert(s);
                }
            }
        }
    }
    (aliases_of, declined)
}

/// Contract-derived CONSUMING user-callee arg position — the SSOT predicate
/// shared by the owned-call-arg dup admission, the Phase-7 hand-off debits,
/// and the dup-funded COW clearance. TWO classes (each minus `iter_consumes`):
///  - `access == Owned`: the callee own-consumes outright;
///  - `access == Borrowed && !borrowed_read_only`: a borrowed param with an
///    owned-position use inside the callee (the COW-consume class —
///    `@grow(xs) = xs.push(v)`). The realization convention makes this an
///    effective consume: the call-site annotation falls back to Owned and the
///    callee's COW-inc edge release nets -1 on the caller's lineage when the
///    param dies at the consume, so the caller funds one reference per call. A
///    `borrowed_read_only` param (pure reads, `@total(xs) = xs.fold(..)`)
///    nets 0 — never a consume.
///
/// KNOWN builtins are excluded — their call-site `[own]`/`[borrow]`
/// annotations from lowering are the precise structural source (seeded
/// builtin contracts leave `borrowed_read_only` at its conservative `false`
/// default, which would mis-class pure-read receivers like `@len`/`@concat`).
pub(crate) fn contract_consuming_arg_position(
    contracts: &FxHashMap<Name, MemoryContract>,
    builtins: &crate::borrow::BuiltinOwnershipSets,
    interner: &ori_ir::StringInterner,
    callee: Name,
    pos: usize,
) -> bool {
    let owned = !builtins.is_builtin(callee)
        && contracts.get(&callee).is_some_and(|c| {
            c.params.get(pos).is_some_and(|p| {
                // `transfers_through_return` positions are PASS-THROUGHS, not
                // consumes: a ttr forwarder (`@id<T>(x) = x`) hands the same
                // allocation back through the result — the lineage continues
                // (RL-34); treating the hand-in as a consumed duplication
                // over-incs the moved-through allocation. The Borrowed class
                // requires the precise `borrowed_cow_consumed` fact (COW
                // consume at the lineage's death) — a borrowed STORE
                // (`Inner { items: xs }`) or a live-past COW read nets 0 in
                // the callee and never obligates caller funding.
                !p.iter_consumes
                    && !p.transfers_through_return
                    && (p.access == AccessClass::Owned
                        || (p.access == AccessClass::Borrowed && p.borrowed_cow_consumed))
            })
        });
    if tracing::enabled!(target: "ori_arc::aims::realize", tracing::Level::TRACE) {
        let probe = contracts.get(&callee).and_then(|c| c.params.get(pos));
        tracing::trace!(
            target: "ori_arc::aims::realize",
            callee = ?interner.try_lookup(callee),
            pos,
            param = ?probe,
            owned,
            "owned-call-arg admission contract probe"
        );
    }
    owned
}
