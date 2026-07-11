//! Root collection + gate-(c)/(c2) helpers for the borrowed-`Invoke` lineage
//! scan: the FRESH collection-`Construct` + borrowed-call-RESULT root families
//! and the borrowed-arg / iter-consume member probes. Spec: Annex E §AIMS RL-2.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind, LitValue};

/// Gate (a): candidate roots — every FRESH heap rc=1 dst:
///  - a collection buffer `Construct { ctor: ListLiteral | MapLiteral |
///    SetLiteral }`, OR
///  - a sum-aggregate `Construct { ctor: EnumVariant }` (Option / Result / user
///    sum) whose dst is in `owned_vars_needing_rc` — the same family the
///    construct-fed dead-param scan admits (`is_sum_aggregate_construct_rep`).
///    The `owned_vars_needing_rc` gate excludes all-scalar-payload sum
///    instantiations (`Option<int>`: niche-packed, no RC header), keeping the
///    no-sink / dead-param treatment scoped to genuine heap lineages.
///  - a HEAP string literal `Let { value: Literal::String }` whose dst is in
///    `owned_vars_needing_rc` (heap-backed `OriStr`, not SSO — the membership
///    gate IS the heap discriminator, mirroring the `EnumVariant` arm). A heap
///    str literal borrow-read through a chain of borrowed-`Invoke` args
///    (`base.substring(..).split(..)` — every part is a seamless slice co-owning
///    the base buffer) then dead at the normal-exit `Return` is the SAME
///    receiver-lineage shape as a collection `Construct`: the base walk places
///    the whole-var release on the dying unwind edges (the per-edge `deadAtSucc` release) but
///    NOT on the normal `Return`, leaking the base allocation on the normal path
///    (`RL2_release_exactly_once`: every concrete path nets to 0; the normal
///    path nets +1). The same death-point treatment + same-alloc vetting +
///    overlap gate place EXACTLY ONE release after the closure's final
///    borrow-read on the normal exit. Spec: Annex E §AIMS RL-2 + RL-4.
///
/// Plain `Struct` ctors are admitted under the SAME `owned_vars_needing_rc`
/// heap-burden discriminator as the `EnumVariant` arm (a let-bound user-drop
/// struct key borrowed by a map-insert `Invoke` terminator is the failing cell:
/// the base walk's inline `[AggFields]` dec runs the key's drop glue BEFORE the
/// call reads it). Struct roots are returned in the second set — the caller
/// restricts them to the DEAD-PARAM + NO-SINK death-point modes (loop-exit +
/// carrier-succ stay excluded until a failing cell exists) and declines any
/// struct root carrying a live field extract (a struct field `Project` IS a
/// same-alloc view; a whole-var release would double-free its backing). An SSO
/// str literal (absent from `owned_vars_needing_rc`) declines naturally — no
/// RC burden.
pub(super) fn collect_fresh_construct_roots(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> (Vec<ArcVarId>, FxHashSet<ArcVarId>) {
    let mut roots: Vec<ArcVarId> = Vec::new();
    let mut struct_roots: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                // Collection buffers: always heap rc=1, no `owned_vars_needing_rc` gate.
                ArcInstr::Construct { dst, ctor, .. } if ctor.is_collection_literal() => {
                    roots.push(*dst);
                }
                // Sum-aggregate `Construct` + heap str-literal `Let`: heap rc=1
                // ONLY when the `owned_vars_needing_rc` membership proves an RC
                // burden (excludes niche-packed scalar sums + SSO strings).
                ArcInstr::Construct {
                    dst,
                    ctor: CtorKind::EnumVariant { .. },
                    ..
                }
                | ArcInstr::Let {
                    dst,
                    value: ArcValue::Literal(LitValue::String(_)),
                    ..
                } if owned_vars_needing_rc.contains(dst) => roots.push(*dst),
                // Plain `Struct` Construct: same membership gate; tagged for the
                // caller's mode restriction + field-extract decline.
                ArcInstr::Construct {
                    dst,
                    ctor: CtorKind::Struct(_),
                    ..
                } if owned_vars_needing_rc.contains(dst) => {
                    roots.push(*dst);
                    struct_roots.insert(*dst);
                }
                _ => {}
            }
        }
    }
    (roots, struct_roots)
}

/// Candidate RESULT roots: a heap `Invoke` result of a may-unwind USER call
/// (`@first(xs) = xs[0]` returns a self-inc'd element) that the callee returns
/// owned (`return_info.uniqueness ∈ {Unique, MaybeShared}`) WITHOUT transferring
/// a param through the return (a forwarder result is the ARG's allocation, owned
/// by the forwarder scans) and WITHOUT iter-consuming a param (the iter-consume
/// transfer owns the release). The result's FRESH-site inc is spurious — the
/// callee already handed +1 — and the result, borrow-read-only, dies at a dead
/// block-param; the suppress + dead-param release frees the callee's +1 exactly
/// once. The death-point + vet gates ([`choose_dead_param_release_site`] +
/// [`same_alloc_closure_vetted`]) bound the over-fire surface.
pub(super) fn collect_borrowed_call_result_roots(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> (Vec<ArcVarId>, FxHashSet<ArcVarId>) {
    let owned_result_non_forwarder_non_iter = |callee: &Name| -> bool {
        contracts.get(callee).is_some_and(|c| {
            matches!(
                c.return_info.uniqueness,
                crate::aims::lattice::Uniqueness::Unique
                    | crate::aims::lattice::Uniqueness::MaybeShared
            ) && !c.params.iter().any(|p| p.transfers_through_return)
                && !c.params.iter().any(|p| p.iter_consumes)
        })
    };
    // PROVABLY-FRESH result: the callee's contract proves the returned value is
    // a NEW allocation (`Unique` + `preserves_freshness`), never a same-buffer
    // view (`@substring` / `@repeat` are ttr / non-fresh and stay excluded).
    // Only this subset is safe for the NO-SINK edge-death claim — an edge
    // release on a view double-frees the viewed buffer. Spec: Annex E §AIMS
    // RL-2.
    let provably_fresh = |callee: &Name| -> bool {
        contracts.get(callee).is_some_and(|c| {
            c.return_info.uniqueness == crate::aims::lattice::Uniqueness::Unique
                && c.return_info.preserves_freshness
                && !c.params.iter().any(|p| p.transfers_through_return)
                && !c.params.iter().any(|p| p.iter_consumes)
        })
    };
    let mut roots: Vec<ArcVarId> = Vec::new();
    let mut fresh: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        if let ArcTerminator::Invoke {
            dst, func: callee, ..
        } = &block.terminator
        {
            if owned_vars_needing_rc.contains(dst) && owned_result_non_forwarder_non_iter(callee) {
                roots.push(*dst);
                if provably_fresh(callee) {
                    fresh.insert(*dst);
                }
            }
        }
    }
    (roots, fresh)
}

/// Candidate BUILTIN-RESULT roots (the third root family): a terminator
/// `Invoke` whose callee is a known SELF-ALLOCATING builtin returning a fresh
/// rc=1 heap buffer (`@concat` template-chain links, heap-producing `@to_str` /
/// `@debug`, `@split` / `@keys` conversions, COW-method results — the
/// [`crate::aims::realize::fresh_rc_alloc_dst_terminator`] SSOT classification,
/// which excludes the sharing views `@slice` / `@substring`). The result is a
/// fresh allocation whose Phase-5 fresh-site inc / inline last-use dec pair
/// splits across blocks when the result is live across a later may-unwind
/// `Invoke` (the next interpolation's `@to_str`) — the Phase-7 position-blind
/// fresh-inc elision then strips the inc keeping the pre-call dec, freeing the
/// buffer BEFORE the borrowed `@concat` reads it (debug `copy_nonoverlapping`
/// abort / release silent use-after-free). The lineage treatment suppresses the
/// pair and places ONE release at the consuming `Invoke`'s normal-successor
/// entry (the CARRIER-SUCC death-point mode). Spec: Annex E §AIMS RL-1 + RL-2.
pub(super) fn collect_fresh_builtin_invoke_result_roots(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    // `__collect_set` (`xs.iter().collect()`) yields a fresh rc=1 set buffer as
    // a body-form `Apply` result; admitted by exact protocol name so its
    // release lands after the borrowed-use `Invoke`. Spec: Annex E §AIMS RL-2 + RL-4.
    let collect_set_protocol =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::CollectSet.name());
    let mut roots: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst, func: callee, ..
            } = instr
            {
                if *callee == collect_set_protocol && owned_vars_needing_rc.contains(dst) {
                    roots.insert(*dst);
                }
            }
        }
        if let Some((dst, _normal)) =
            crate::aims::realize::fresh_rc_alloc_dst_terminator(&block.terminator, func, interner)
        {
            if owned_vars_needing_rc.contains(&dst) {
                roots.insert(dst);
            }
        }
    }
    roots
}

/// Gate (c): true iff any closure member is used at a BORROWED (non-owned) arg
/// position of an `Invoke` / `InvokeIndirect` terminator — the carrier whose
/// inline-before-terminator `BurdenDec` is the use-after-free.
pub(super) fn closure_has_borrowed_invoke_arg(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
) -> bool {
    for block in &func.blocks {
        let term = &block.terminator;
        if !matches!(
            term,
            ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. }
        ) {
            continue;
        }
        for (pos, &v) in term.used_vars().iter().enumerate() {
            if members.contains(&v) && !term.is_owned_position(pos) {
                return true;
            }
        }
    }
    false
}

/// Gate (c2): true iff any closure member is used at a borrowed `Invoke` /
/// `InvokeIndirect` arg position whose callee `iter_consumes` that position (a
/// `for w in coll` inside the callee → its `ori_iter_drop` frees the buffer; an
/// RL-2 ownership transfer DESPITE the borrowed arg position, per
/// `ParamContract.iter_consumes`). The callee owns the release, so a dead-param
/// release on such a lineage double-frees. Mirrors the `iter_consumes` detection
/// in `compute_ttr_iter_consume_dup_aliases` (the same SSOT signal).
pub(super) fn closure_member_iter_consumed_at_invoke(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> bool {
    for block in &func.blocks {
        // Only a named-callee `Invoke` carries a contract with `iter_consumes`;
        // an `InvokeIndirect` (closure) has no contract — conservatively NOT an
        // iter-consume transfer here (it declines elsewhere via the `Apply
        // Indirect` vet anyway).
        if let ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            let Some(contract) = contracts.get(callee) else {
                continue;
            };
            for (pos, &arg) in args.iter().enumerate() {
                if members.contains(&arg)
                    && contract.params.get(pos).is_some_and(|p| p.iter_consumes)
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Gate (c3): true iff any closure member is a borrowed call arg (body `Apply`
/// or terminator `Invoke`) at a position whose callee param carries
/// `borrowed_cow_mutated` — the callee forwards the borrow into a COW-MUTATOR
/// owned position (`@fork (base) = base.push(v)`) and nets -1 on the caller's
/// allocation per call. The RL-1 caller funding inc (one duplication inc per
/// call site, `RL1_duplication_balanced`) is load-bearing for such a callee;
/// this claim's inc-suppression would free the buffer during the first call.
/// The MUTATOR-only fact (not `borrowed_cow_consumed`) keeps iter-consuming
/// callees (`c.iter().count()`) admitted — their accounting stays with the
/// iter-consume machinery, and declining them re-opens the set-collect leak.
/// Builtin seed contracts never carry the fact, so pure builtin reads
/// (`@len` / `@__index`) stay admitted. Spec: Annex E §AIMS RL-1 + RL-2.
pub(super) fn closure_member_cow_consumed_at_call(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> bool {
    let cow_consumed_at = |callee: &Name, pos: usize| -> bool {
        contracts
            .get(callee)
            .and_then(|c| c.params.get(pos))
            .is_some_and(|p| p.borrowed_cow_mutated)
    };
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                func: callee, args, ..
            } = instr
            {
                for (pos, &arg) in args.iter().enumerate() {
                    if members.contains(&arg) && cow_consumed_at(callee, pos) {
                        return true;
                    }
                }
            }
        }
        if let ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            for (pos, &arg) in args.iter().enumerate() {
                if members.contains(&arg) && cow_consumed_at(callee, pos) {
                    return true;
                }
            }
        }
    }
    false
}
