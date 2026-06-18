//! Borrowed-position scans: borrowed terminator-`Invoke` args, borrowed-arg
//! `Let` aliases, borrowed projection dsts, and the RL-1 borrowed-store
//! duplication incs. Spec: Annex E §AIMS RL-1 + RL-2 + TF-4.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_types::TypeRegistry;

use ori_ir::StringInterner;

use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId};
use crate::lower::burden::TypeRef;
use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

use crate::ownership::Ownership;

use super::super::burden_carries_rc;
use super::instr_transfer_vars;

use std::sync::LazyLock;

/// `ORI_DISABLE_SOLE_CARRIER_BORROWED_INVOKE_CLAIM=1` restores the inline
/// last-use dec for a SOLE-CARRIER borrowed-`Invoke` alias
/// ([`compute_sole_carrier_borrowed_invoke_aliases`] returns empty): the
/// lineage's single release lands BEFORE the borrowed terminator that reads it
/// (use-after-free / double-free when the callee aliases the value into its
/// result — the `wrap_ok(m: m)` mint shape). Bisection surface: isolates a
/// borrowed-call-arg early-release to the edge-claim vs the rest of the
/// Phase-5 walk. Spec: Annex E §AIMS RL-2 + RL-4.
static SOLE_CARRIER_BORROWED_INVOKE_CLAIM_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_SOLE_CARRIER_BORROWED_INVOKE_CLAIM").as_deref() == Ok("1")
});

/// `ORI_DISABLE_YIELD_IDENTITY_PUSH_DUP_INC=1` restores the missing store-dup
/// inc on a yield-identity `@ori_list_push` element (the bisection surface that
/// isolates the RL-1 yield-identity duplication inc from the rest of the
/// Phase-5 walk). Default off (the inc is emitted). Spec: Annex E §AIMS RL-1.
static YIELD_IDENTITY_PUSH_DUP_INC_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_YIELD_IDENTITY_PUSH_DUP_INC").as_deref() == Ok("1")
});

/// SOLE-CARRIER borrowed-`Invoke` aliases (RL-2 + RL-4): `Let { Var(src) }`
/// dsts that carry their lineage's ONLY release while their single use is a
/// BORROWED arg of a may-unwind `Invoke` terminator. The base walk places the
/// dst's last-use `BurdenDec` INLINE before that terminator — releasing the
/// allocation BEFORE the borrowed call reads it (use-after-free; a double-free
/// when the callee aliases the value into its result). The cure removes the
/// dst from `owned_vars_needing_rc` (suppressing the early inline dec) and
/// CLAIMS it for the Category-2 `deadAtSucc` per-edge release (`edge_cleanup.rs`
/// via `claimed_no_sink_vars` / `burden_emitted`): the dst is dead at EVERY
/// successor of its carrier `Invoke`, so each executing path releases exactly
/// once AFTER the call completes.
///
/// Gates (ALL hold; a declined dst keeps the inline dec — status quo):
/// - dst is in `owned_vars_needing_rc` (it carries the lineage's release) and
///   its SOLE use is a borrowed `Invoke` / `InvokeIndirect` terminator arg;
/// - src is used EXACTLY once (the alias) — the lineage genuinely dies at the
///   carrier call; a forward-live src owns later reads (and the funded
///   duplication families require `src >= 2` uses — disjoint by construction);
/// - src is NOT a function param (param scope-exit accounting is its own
///   surface);
/// - src is DIRECTLY the dst of a USER-call `Invoke` / `InvokeIndirect`
///   terminator AND is itself still in `owned_vars_needing_rc`. Only that
///   shape guarantees src's own burden ops are a self-canceling
///   fresh-inc/last-use-dec pair at the alias site (net 0), leaving the
///   claimed dst the lineage's single effective release. Every other src
///   carries the lineage's release ITSELF — a `Project` src is a borrow-view
///   whose alias dec is the element's designated release (RL-2 final-read
///   release); a block-param src inherited its RC obligation through the
///   Jump transfer (RL-4 exemption); a builtin `Apply` / `Invoke` result
///   (`@__index` element copy, `@concat` buffer) and a `Let { Var }`
///   chain hop carry a bare last-use dec with no funding inc. Claiming an
///   alias of those relocates the bare dec INLINE before the borrowed
///   terminator (use-after-free) or drops the only release (leak).
pub(in crate::lower::burden_lower) fn compute_sole_carrier_borrowed_invoke_aliases(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    contracts: &FxHashMap<ori_ir::Name, crate::aims::contract::MemoryContract>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    if *SOLE_CARRIER_BORROWED_INVOKE_CLAIM_DISABLED {
        return FxHashSet::default();
    }
    // BORROWED at the carrier = the call-site annotation says Borrowed, OR a
    // USER callee's interprocedural contract proves the param a pure borrow
    // (the call-site `arg_ownership` of a named user call is unpopulated at
    // Phase 5 and `ArcTerminator::is_owned_position` defaults missing entries
    // to Owned; the contract is the proven signal — the `wrap_ok(m: m)`
    // borrowed-store-mint shape). The contract branch mirrors
    // `contract_consuming_arg_position`'s fences: KNOWN builtins keep their
    // precise structural annotations (a COW receiver's seeded contract would
    // over-admit); an iter-consuming / ttr / COW-consumed borrowed param is a
    // transfer or consume with its own release accounting — never claimed.
    let builtins = crate::borrow::BuiltinOwnershipSets::new(interner);
    let mut borrowed_args = compute_borrowed_terminator_invoke_args(func);
    for block in &func.blocks {
        if let crate::ir::ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            if builtins.is_builtin(*callee) {
                continue;
            }
            if let Some(contract) = contracts.get(callee) {
                for (pos, &arg) in args.iter().enumerate() {
                    let pure_borrow = contract.params.get(pos).is_some_and(|p| {
                        p.access == crate::aims::lattice::dimensions::AccessClass::Borrowed
                            && !p.iter_consumes
                            && !p.transfers_through_return
                            && !p.borrowed_cow_consumed
                    });
                    if pure_borrow {
                        borrowed_args.insert(arg);
                    }
                }
            }
        }
    }
    if borrowed_args.is_empty() {
        return FxHashSet::default();
    }
    let mut use_counts: FxHashMap<ArcVarId, u32> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            for &v in &instr.used_vars() {
                *use_counts.entry(v).or_default() += 1;
            }
        }
        for v in block.terminator.used_vars() {
            *use_counts.entry(v).or_default() += 1;
        }
    }
    let params: FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();
    let user_invoke_results = collect_user_invoke_result_dsts(func, &builtins);
    let mut out: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                let gates = [
                    ("owned-dst", owned_vars_needing_rc.contains(dst)),
                    ("borrowed-arg", borrowed_args.contains(dst)),
                    (
                        "dst-sole-use",
                        use_counts.get(dst).copied().unwrap_or(0) == 1,
                    ),
                    (
                        "src-sole-use",
                        use_counts.get(src).copied().unwrap_or(0) == 1,
                    ),
                    ("src-not-param", !params.contains(src)),
                    (
                        "src-user-invoke-result",
                        user_invoke_results.contains(src) && owned_vars_needing_rc.contains(src),
                    ),
                ];
                if let Some((gate, _)) = gates.iter().find(|(_, ok)| !ok) {
                    if owned_vars_needing_rc.contains(dst) && borrowed_args.contains(dst) {
                        tracing::trace!(
                            target: "ori_arc::aims::realize",
                            fn_name = ?func.name,
                            dst = dst.index(),
                            gate,
                            "sole-carrier borrowed-invoke claim declined"
                        );
                    }
                } else {
                    tracing::trace!(
                        target: "ori_arc::aims::realize",
                        fn_name = ?func.name,
                        dst = dst.index(),
                        "sole-carrier borrowed-invoke claim admitted"
                    );
                    out.insert(*dst);
                }
            }
        }
    }
    out
}

/// USER-call Invoke / `InvokeIndirect` terminator dsts — the sole src shape
/// whose burden ops self-cancel (the sole-carrier gate table's
/// src-user-invoke-result row). Builtin Invoke results are excluded: their
/// single dec is the lineage's release. Spec: Annex E §AIMS RL-2 + RL-4.
fn collect_user_invoke_result_dsts(
    func: &ArcFunction,
    builtins: &crate::borrow::BuiltinOwnershipSets,
) -> FxHashSet<ArcVarId> {
    let mut user_invoke_results: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        match &block.terminator {
            crate::ir::ArcTerminator::Invoke {
                dst, func: callee, ..
            } => {
                if !builtins.is_builtin(*callee) {
                    user_invoke_results.insert(*dst);
                }
            }
            crate::ir::ArcTerminator::InvokeIndirect { dst, .. } => {
                user_invoke_results.insert(*dst);
            }
            _ => {}
        }
    }
    user_invoke_results
}

/// Vars consumed at a BORROWED arg position of any `Invoke` / `InvokeIndirect`
/// terminator. Mirrors the per-block `invoke_terminator_borrowed_args` in
/// `emit.rs` (which gates the terminator-last-use `BurdenDec` suppression) but
/// computed function-wide for the FRESH-site `BurdenInc` suppression. A FRESH
/// value passed at a borrowed Invoke arg position has its terminator dec
/// suppressed there; suppressing its FRESH inc here keeps the per-value burden
/// ledger empty so the predicate-stack edge cleanup fully owns its release.
pub(in crate::lower::burden_lower) fn compute_borrowed_terminator_invoke_args(
    func: &ArcFunction,
) -> FxHashSet<ArcVarId> {
    let mut borrowed: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        let term = &block.terminator;
        if !matches!(
            term,
            crate::ir::ArcTerminator::Invoke { .. }
                | crate::ir::ArcTerminator::InvokeIndirect { .. }
        ) {
            continue;
        }
        for (pos, &var) in term.used_vars().iter().enumerate() {
            if !term.is_owned_position(pos) {
                borrowed.insert(var);
            }
        }
    }
    borrowed
}

/// `Let { Var(src) }` aliases whose SOLE use is a BORROWED terminator-Invoke arg
/// position. Such an alias is a borrow-view of `src`, NOT a new owned reference:
/// per RL-1 (`08-realization/RL-1.proof` + `AimsProof.Realization`
/// `RL1_emit_iff_not_elidable`) a duplication inc is emitted ONLY when a value is
/// passed to an OWNED param while still live, so `f(x, x)` over two Borrowed
/// params creates no reference at either arg — the owned SOURCE `x` carries the
/// sole inc + release. Without excluding these aliases the dup-alias FRESH-site
/// `BurdenInc` + borrowed-arg scope-exit `BurdenDec` pair on each alias leaves the
/// source's FRESH inc orphaned (VF-1 net=+1 leak). Excluded from
/// `owned_vars_needing_rc` (neither inc nor dec) — a borrowed alias gets
/// neither. Use-count gated to 1 so a value also
/// used at an OWNED position keeps its burden ops.
pub(in crate::lower::burden_lower) fn compute_borrowed_arg_let_aliases(
    func: &ArcFunction,
) -> FxHashSet<ArcVarId> {
    let borrowed_args = compute_borrowed_terminator_invoke_args(func);
    if borrowed_args.is_empty() {
        return FxHashSet::default();
    }
    let mut use_counts: FxHashMap<ArcVarId, u32> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            for &v in &instr.used_vars() {
                *use_counts.entry(v).or_default() += 1;
            }
        }
        for v in block.terminator.used_vars() {
            *use_counts.entry(v).or_default() += 1;
        }
    }
    let mut out: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                // dst is the sole-use borrow-view; src must STAY LIVE (used >= 2,
                // a genuine duplication) so the OWNED source carries the
                // release. A move source (src used once) makes the alias the
                // sole carrier — excluding it would drop the release (leak).
                if borrowed_args.contains(dst)
                    && use_counts.get(dst).copied().unwrap_or(0) == 1
                    && use_counts.get(src).copied().unwrap_or(0) >= 2
                {
                    out.insert(*dst);
                }
            }
        }
    }
    out
}

/// `Project` dsts whose EVERY use is at a BORROWED (non-owned) position — a pure
/// borrow-view of an owned aggregate's field per `Spec: Annex E §AIMS TF-4`
/// (Project produces `Borrowed`). The PARENT aggregate owns the projected field
/// and releases it via its whole-var scope-exit drop-glue, so the borrowed
/// projection gets NO dec. An RcPtr-typed Project dst (e.g. `Project box.0 :
/// [int]`) enters `owned_vars_needing_rc` by TYPE alone; without this exclusion
/// a borrowed field-read (`@len [borrow]` / `@__index [borrow]`) gets a spurious
/// last-use `BurdenDec`, over-releasing the field the parent drop already frees.
///
/// Over-fire gate (RL-15a project-escape boundary): excluded ONLY when never used
/// at an owned arg position (`instr_transfer_vars` honors `is_owned_position`)
/// AND never returned — an escaping / owned-position-transferred Project IS an
/// owned reference and KEEPS its burden RC.
pub(in crate::lower::burden_lower) fn compute_borrowed_projection_dsts(
    func: &ArcFunction,
) -> FxHashSet<ArcVarId> {
    let mut project_dsts: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let crate::ir::ArcInstr::Project { dst, .. } = instr {
                project_dsts.insert(*dst);
            }
        }
    }
    if project_dsts.is_empty() {
        return FxHashSet::default();
    }
    let mut transferred_or_escaped: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            for v in instr_transfer_vars(instr, func) {
                transferred_or_escaped.insert(v);
            }
        }
        let term = &block.terminator;
        for (pos, &v) in term.used_vars().iter().enumerate() {
            if term.is_owned_position(pos) {
                transferred_or_escaped.insert(v);
            }
        }
        if let crate::ir::ArcTerminator::Return { value } = term {
            transferred_or_escaped.insert(*value);
        }
    }
    project_dsts.retain(|d| !transferred_or_escaped.contains(d));
    project_dsts
}

/// Borrowed-store duplication args per AIMS RL-1: vars whose `Let { Var }`
/// alias-chain ROOT is a BORROWED param and that are consumed at an
/// aggregate-STORE position (`Construct` / `Reuse` / `CollectionReuse` arg or
/// `Set.value`).
///
/// A borrow proves the CALLER retains a live reference across the whole call
/// (RL-2: the caller emits the borrowed param's release —
/// `AimsProof.Realization::RL2_borrowed_param_emits_caller_dec`), so storing
/// the borrowed value into an aggregate ALWAYS creates a real SECOND
/// reference: the caller's reference persists and the aggregate takes a new
/// one. The store-site `BurdenInc` is LOAD-BEARING
/// (`AimsProof.Realization::RL1_duplication_balanced` — the duplication's inc
/// is matched by exactly one release: the container's drop); without it the
/// container's drop releases a reference no inc supplied (the caller's
/// still-live value frees at live=1 — use-after-free).
///
/// THREE exclusions bound the family:
///
/// - Call-arg consumers: a borrowed value passed at an `Apply` / `Invoke` arg
///   is OUT — call consumes have their own interprocedural / protocol
///   ownership accounting; injecting an inc there over-counts. Only the
///   aggregate-STORE family is classified.
/// - Non-param roots: only a chain rooted at a BORROWED param carries the
///   borrow proof. Owned roots keep their existing treatment (terminal move /
///   genuine-dup per [`compute_genuine_dup_move_aliases`]); over-adding on an
///   owned source leaks.
/// - RC-free types: a stored value whose burden carries no RC dimension needs
///   no inc (nothing to release).
///
/// Membership is per-VAR; the emission walk emits one inc per store SITE the
/// member is consumed at (each store duplicates). No paired dec is emitted in
/// this function — the container's drop is the matched release wherever the
/// container dies.
pub(in crate::lower::burden_lower) fn compute_borrowed_store_dup_args(
    func: &ArcFunction,
    type_registry: &TypeRegistry,
) -> FxHashSet<ArcVarId> {
    if super::super::borrowed_store_dup_inc_disabled() {
        return FxHashSet::default();
    }
    let mut borrowed_rooted: FxHashSet<ArcVarId> = func
        .params
        .iter()
        .filter(|p| p.ownership == Ownership::Borrowed)
        .map(|p| p.var)
        .collect();
    if borrowed_rooted.is_empty() {
        return FxHashSet::default();
    }
    // Vars are defined before use within the function walk, so a single
    // forward pass resolves each `Let { Var }` chain to its root (same
    // discipline as `collect_use_sites`' single pass).
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                if borrowed_rooted.contains(src) {
                    borrowed_rooted.insert(*dst);
                }
            }
        }
    }
    let mut dup_args: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut admit = |var: ArcVarId| {
        if !borrowed_rooted.contains(&var) || dup_args.contains(&var) {
            return;
        }
        // L-9 repr-aware admission gate (same SSOT as `owned_vars_needing_rc`):
        // a provably-Scalar-repr stored value is a niche-packed copy — no
        // second reference exists, and a store-site inc on it can never lower.
        if super::super::is_provably_scalar_repr(func, var) {
            return;
        }
        let ty: TypeRef = idx_to_type_ref(func.var_types[var.index()], type_registry);
        if lookup_burden(ty, type_registry)
            .as_ref()
            .is_some_and(burden_carries_rc)
        {
            dup_args.insert(var);
        }
    };
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Construct { args, .. }
                | ArcInstr::Reuse { args, .. }
                | ArcInstr::CollectionReuse { args, .. } => {
                    for &arg in args {
                        admit(arg);
                    }
                }
                ArcInstr::Set { value, .. } => admit(*value),
                _ => {}
            }
        }
    }
    dup_args
}

/// Yield-identity push duplication args per AIMS RL-1: element vars consumed at
/// the OWNED element-store position (`args[1]`) of an `Apply @ori_list_push`
/// whose stored element is an iterator-element borrow-view (`Project field:1`
/// of `Apply @__iter_next`, + its `Let`-alias closure) AND whose iteration
/// SOURCE roots at a BORROWED parameter of THIS function.
///
/// `for w in borrowed_words yield w` lowers to `@ori_list_push(fresh_list,
/// w [own], elem_size)` where `w` is a BORROW into the source's buffer. The
/// source is a borrowed param, so the CALLER retains it across the whole call
/// (RL-2: `AimsProof.Realization::RL2_borrowed_param_emits_caller_dec`); the
/// push copies the element bytes into the FRESH result buffer, creating a real
/// SECOND reference. Per RL-1 (`RL1_duplication_balanced`) this duplicating
/// store MUST emit one `BurdenInc` on the element — matched by the result
/// collection's `elem_dec_fn` drop. Without it both the source (via the
/// caller's retained reference) and the result list dec the shared element ->
/// double-free (single call + source reuse) or under-count -> leak (two calls).
///
/// The iterator-element-view exclusion (`collect_iter_element_defs` keeps the
/// view OUT of `owned_vars_needing_rc` to suppress a spurious last-use dec)
/// correctly drops the DEC side but also suppressed this load-bearing store-dup
/// INC; this scan restores only the inc on the genuine duplication.
///
/// Source-ownership discriminator: only a BORROWED-param-rooted source duplicates
/// (the source survives in the caller). An OWNED / freshly-`Construct`ed source
/// (e.g. `for w in [..] yield w`) is consumed by its own iteration and needs no
/// inc — its element references transfer; firing there over-counts -> leak.
/// No paired dec is emitted here — the result collection's drop is the match.
pub(in crate::lower::burden_lower) fn compute_yield_identity_push_dup_args(
    func: &ArcFunction,
    type_registry: &TypeRegistry,
    interner: &StringInterner,
) -> FxHashSet<ArcVarId> {
    if *YIELD_IDENTITY_PUSH_DUP_INC_DISABLED {
        return FxHashSet::default();
    }
    let elem_views = compute_borrowed_source_iter_element_views(func, interner);
    if elem_views.is_empty() {
        return FxHashSet::default();
    }
    let list_push_name = interner.intern("ori_list_push");

    // Admit element views consumed at the OWNED element-store position
    // (`args[1]`) of `@ori_list_push`, gated on RC-carrying burden.
    let mut dup_args: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            let ArcInstr::Apply { func: f, args, .. } = instr else {
                continue;
            };
            if *f != list_push_name {
                continue;
            }
            let Some(&elem) = args.get(1) else { continue };
            if !elem_views.contains(&elem) || dup_args.contains(&elem) {
                continue;
            }
            let ty: TypeRef = idx_to_type_ref(func.var_types[elem.index()], type_registry);
            if lookup_burden(ty, type_registry)
                .as_ref()
                .is_some_and(burden_carries_rc)
            {
                dup_args.insert(elem);
            }
        }
    }
    dup_args
}

/// Iterator-element borrow-views (`Project field:1` of `@__iter_next`, + the
/// `Let { Var }` alias closure) whose iteration SOURCE roots at a BORROWED
/// parameter of `func`. The borrowed-param root is the survives-in-caller
/// discriminator (RL-2): only such an element duplicates when stored. An
/// OWNED / freshly-`Construct`ed source is consumed by its own iteration and
/// is intentionally NOT collected (no `@iter` over a borrowed-rooted source).
fn compute_borrowed_source_iter_element_views(
    func: &ArcFunction,
    interner: &StringInterner,
) -> FxHashSet<ArcVarId> {
    let iter_name =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Iter.name());
    let iter_next_name =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::IterNext.name());

    // Borrowed-param roots (+ `Let { Var }` alias closure), the same source-gate
    // as `compute_borrowed_store_dup_args`.
    let mut borrowed_rooted: FxHashSet<ArcVarId> = func
        .params
        .iter()
        .filter(|p| p.ownership == Ownership::Borrowed)
        .map(|p| p.var)
        .collect();
    if borrowed_rooted.is_empty() {
        return FxHashSet::default();
    }
    propagate_let_var_closure(func, &mut borrowed_rooted);

    // `@iter` results whose source `args[0]` roots at a borrowed param, then
    // `__iter_next` results whose iterator `args[0]` is such a borrowed iter.
    let borrowed_iter_dsts = apply_dsts_with_first_arg_in(func, iter_name, &borrowed_rooted);
    if borrowed_iter_dsts.is_empty() {
        return FxHashSet::default();
    }
    let borrowed_iter_next_dsts =
        apply_dsts_with_first_arg_in(func, iter_next_name, &borrowed_iter_dsts);
    if borrowed_iter_next_dsts.is_empty() {
        return FxHashSet::default();
    }

    // Element views: `Project field:1` of a borrowed-source `__iter_next`, plus
    // the `Let { Var }` alias closure (the lowering emits `%elem = %projected`).
    let mut elem_views: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project {
                dst,
                value,
                field: 1,
                ..
            } = instr
            {
                if borrowed_iter_next_dsts.contains(value) {
                    elem_views.insert(*dst);
                }
            }
        }
    }
    propagate_let_var_closure(func, &mut elem_views);
    elem_views
}

/// Grow `set` by the `Let { Var(src) }` alias closure: `dst` joins when `src`
/// is a member (single forward pass — vars are defined before use).
fn propagate_let_var_closure(func: &ArcFunction, set: &mut FxHashSet<ArcVarId>) {
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                if set.contains(src) {
                    set.insert(*dst);
                }
            }
        }
    }
}

/// `Apply @name` result dsts whose first arg (`args[0]`) is in `sources`.
fn apply_dsts_with_first_arg_in(
    func: &ArcFunction,
    name: ori_ir::Name,
    sources: &FxHashSet<ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let mut dsts: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst, func: f, args, ..
            } = instr
            {
                if *f == name && args.first().is_some_and(|a| sources.contains(a)) {
                    dsts.insert(*dst);
                }
            }
        }
    }
    dsts
}
