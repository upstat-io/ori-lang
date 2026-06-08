//! Phase 5 trivial burden emission walker.
//!
//! Reads each owned non-scalar SSA value's `BurdenSpec` and emits `BurdenInc`
//! at every transfer point + `BurdenDec` at every last-use along every
//! reachable CFG path. Pure per-instruction emission driven by SSA def-use;
//! no global flow analysis, no fixpoint, no lattice consultation.
//!
//! Cluster layout: the analysis-assembly driver (`emit_burden_ops`),
//! `BurdenLowerCtx`, and the collect / detect / filter helpers live here.
//! `terminator` owns terminator-position transfer + inc precompute;
//! `moved_fields` owns the moved-out-fields forward dataflow + full/partial-move
//! partition; `emit` owns the per-instruction + per-terminator emission.

mod cow_aliases;
mod ctx;
mod emit;
mod moved_fields;
mod ownership_scans;
mod terminator;

pub(crate) use ctx::BurdenLowerCtx;

pub(crate) use cow_aliases::{compute_borrowed_alias_vars, compute_cow_inc_borrowed_aliases};
use cow_aliases::{compute_cow_inc_and_mutators, compute_scalar_literal_vars};
pub(crate) use ownership_scans::list_concat_consumed_operands;
use ownership_scans::{
    collect_owned_burdens, compute_borrowed_arg_let_aliases, compute_borrowed_projection_dsts,
    compute_borrowed_terminator_invoke_args, compute_live_out_owned, compute_owned_vars_needing_rc,
    compute_transfer_through_return_param_vars, compute_transfer_through_return_results,
    compute_transfer_via_move_alias, compute_use_counts_and_dup_aliases, detect_last_uses,
    detect_transfer_points, group_last_uses_filtered,
};
use ownership_scans::{instr_owned_position_transfer_vars, instr_transfer_vars};

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcInstr, ArcVarId};
use crate::ownership::DerivedOwnership;
use ori_ir::Name;
use ori_types::TypeRegistry;
use rustc_hash::{FxHashMap, FxHashSet};

use super::burden::{Burden, BurdenRef};

use emit::{emit_burden_ops_for_blocks, BurdenAnalysisCtx};
use moved_fields::{compute_full_move_vars, compute_partial_move_vars, populate_moved_out_fields};
use terminator::{compute_terminator_inc_per_block, compute_terminator_transfer_per_block};

/// True iff `burden` carries any RC-tracked dimension. Used by the filter at
/// `emit_burden_ops` to exclude scalars whose `lookup_burden` returns the empty
/// builtin burden. Defends VF-1 `RcOnScalar` invariant.
pub(super) fn burden_carries_rc(burden: &BurdenRef<'_>) -> bool {
    burden.self_heap_alloc()
        || burden.element_burden().is_some()
        || burden.variant_burdens().next().is_some()
        || burden.owned_fields().next().is_some()
}

/// Walk `func` and emit `BurdenInc` / `BurdenDec` ops per SSA variable from
/// `BurdenSpec` lookups, filtered to owned positions via `DerivedOwnership`.
///
/// Invoked from the AIMS pipeline at Phase 5 (ARC lowering); see
/// `pipeline/aims_pipeline/`.
#[expect(
    clippy::too_many_lines,
    reason = "single Phase-5 burden-emission orchestration: the owned-burden \
              collection, transfer-point detection, last-use detection, the \
              suppression-set computations (borrowed-alias / move-alias / \
              COW-inc / terminator-borrowed-arg), and the per-block emit walk \
              are one cohesive pass; splitting mid-sequence fragments the \
              load-bearing emission order"
)]
pub(crate) fn emit_burden_ops<'a>(
    func: &mut ArcFunction,
    type_registry: &'a TypeRegistry,
    // Block-param ownership lookup for Jump-to-Owned-param transfer detection.
    // DerivedOwnership side-table threaded as typed pre-pass input — slice
    // indexed by ArcVarId::raw() matches infer_derived_ownership() return shape.
    // Empty &[] semantically safe — out-of-bounds defaults to Owned. AIMS
    // Invariant 5 (unified model) preserved — DerivedOwnership is existing
    // analysis output, not a parallel ownership tracker.
    derived_ownership: &[DerivedOwnership],
    // Per-function MemoryContracts from interprocedural analysis. Consumed by
    // FRESH-site BurdenInc emission for Apply/Invoke whose callee
    // `ReturnContract.uniqueness ∈ {Unique, MaybeShared}` (i.e., return value
    // is a FRESH allocation owned by caller). AIMS Invariant 5 — read
    // unchanged, no parallel emission.
    // Immortal var bitvector (`detect_immortals`): empty-string literals carry
    // no RC, so they receive NO burden ops (the predicate-stack emits none) —
    // else the FRESH-site inc orphans (VF-1 net=+1). Tests pass `&[]`.
    immortals: &[bool],
    contracts: &FxHashMap<Name, MemoryContract>,
    // When true (`ORI_DISABLE_PREDICATE_STACK_RC=1`), the predicate-stack edge
    // cleanup is OFF, so the burden walk is the sole RC emitter. The
    // borrowed-Invoke-arg scope-exit `BurdenDec` (normally deferred to the
    // predicate stack's `release_with_burden_edge`) MUST be emitted by the
    // burden walk instead — the completeness pass per `emit.rs`
    // `emit_terminator_burden_decs`. On the default path (false) the dec stays
    // deferred so the two paths do not double-count.
    predicate_stack_rc_disabled: bool,
    // String interner — threaded so the iterator-element exclusion can resolve
    // the `__iter_next` protocol-builtin name via `collect_iter_element_defs`
    // (`Spec: Annex E §AIMS Protocol Builtins`). The SSOT element-classification
    // set is the predicate stack's `collect_iter_element_defs`; the burden walk
    // consumes it (AIMS Invariant 5 — no parallel iterator-element tracker).
    interner: &ori_ir::StringInterner,
) -> BurdenLowerCtx<'a> {
    let mut ctx = BurdenLowerCtx::new(func);
    collect_owned_burdens(&mut ctx, func, type_registry);
    detect_transfer_points(&mut ctx, func, type_registry);
    detect_last_uses(&mut ctx, func);

    // `owned_vars_needing_rc` filters scalars whose `lookup_burden` returns
    // `Some(BurdenRef)` wrapping the empty builtin burden — required by AIMS
    // DP-1 (`is_rc_needed: Owned ∧ ¬Dead ∧ ¬is_scalar`) + VF-1 `RcOnScalar`.
    let mut owned_vars_needing_rc = compute_owned_vars_needing_rc(&ctx);
    {
        let collected: Vec<(u32, String, bool)> = ctx
            .collected
            .iter()
            .map(|(v, b)| {
                let ty = func
                    .var_types
                    .get(v.index())
                    .map(|i| format!("{i:?}"))
                    .unwrap_or_default();
                (
                    u32::try_from(v.index()).unwrap_or(u32::MAX),
                    ty,
                    b.is_some(),
                )
            })
            .collect();
        tracing::trace!(
            target: "ori_arc::aims::realize",
            fn_name = ?func.name,
            ?collected,
            "burden ctx.collected (var, has_burden, carries_rc)"
        );
        let mut initial: Vec<u32> = owned_vars_needing_rc
            .iter()
            .map(|v| u32::try_from(v.index()).unwrap_or(u32::MAX))
            .collect();
        initial.sort_unstable();
        tracing::trace!(
            target: "ori_arc::aims::realize",
            fn_name = ?func.name,
            ?initial,
            "burden owned-vars INITIAL (collected, pre-retain)"
        );
    }
    // Exclude immortals (empty-string literals) — no RC, so no burden ops at all.
    owned_vars_needing_rc.retain(|v| !immortals.get(v.index()).copied().unwrap_or(false));
    // Exclude borrowed-derived locals: a `Let { Var(src) }` alias of a borrowed
    // value is itself a borrow (TF-2 propagates the source's Access; a borrowed
    // source yields a borrowed alias). Borrowed values carry NO RC obligation
    // (RL-2: "Borrowed variables do NOT receive decs"), so an alias of a
    // borrowed param MUST get no burden ops — `collect_owned_burdens` filters
    // borrowed PARAMS but a local alias of one is not a param and slips through,
    // producing an orphan last-use BurdenDec (VF-1 net=-1). Propagate the
    // borrow forward through every Let-Var hop to a fixpoint and exclude the set.
    let borrowed_aliases = compute_borrowed_alias_vars(func);
    owned_vars_needing_rc.retain(|v| !borrowed_aliases.contains(v));
    // A `Let { Var(src) }` alias whose sole use is a BORROWED terminator-Invoke
    // arg is a borrow-view of an owned source: per RL-1 the dup-inc is
    // Owned-param-only, so `f(x, x)` over Borrowed params creates no reference at
    // either arg — the owned source carries the inc+release, the alias gets
    // neither (else the source's FRESH inc orphans, VF-1 net=+1 leak).
    let borrowed_arg_aliases = compute_borrowed_arg_let_aliases(func);
    owned_vars_needing_rc.retain(|v| !borrowed_arg_aliases.contains(v));
    // RL-2 / TF-4: a `Project` dst used only at borrow positions is a borrow-view
    // of the parent aggregate's field — the parent owns + drops the field via
    // whole-var drop-glue, so the borrowed projection gets NO dec. Pairs with the
    // generic-user-struct burden composition (`compose_for_idx`) that makes the
    // owning aggregate carry RC: without the parent drop, excluding the projection
    // would leak; without this exclusion, both would dec and double-free.
    let borrowed_projection_dsts = compute_borrowed_projection_dsts(func);
    owned_vars_needing_rc.retain(|v| !borrowed_projection_dsts.contains(v));
    // Exclude scalar-`Literal`-defined vars: a var whose definition is a
    // `Let { value: Literal(lit) }` with `lit != String` is a scalar sentinel
    // (`Int`/`Float`/`Bool`/`Char`/`Duration`/`Size`/`Unit`/`Null`) carrying NO
    // RC burden regardless of its declared `var_types[v]` (`Spec: Annex E §AIMS
    // L-9` scalar exclusion; TF-1 `Let { Literal } -> SCALAR`). `collect_owned_burdens`
    // keys membership on the declared TYPE, so a var typed as a heap aggregate but
    // defined `Literal(Int(0))` (the `__iter_next` element-type-marker scratch slot)
    // is over-collected and receives an unbalanced `BurdenDec` the inc side never
    // emitted (`fresh_site_burden_inc_dst` emits an inc ONLY for `Literal::String`).
    // The exclusion restores INC/DEC symmetry on the DEFINITION grain.
    let scalar_literal_vars = compute_scalar_literal_vars(func);
    owned_vars_needing_rc.retain(|v| !scalar_literal_vars.contains(v));
    // Exclude iterator-element borrow-views: a `Project { field: 1 }` of an
    // `Apply @__iter_next` result (and its Let/Project/block-param closure) is
    // a BORROWED view into the collection buffer (`Spec: Annex E §AIMS Protocol
    // Builtins`: `IterNext` yields a borrowed element-type marker; the element
    // itself is owned by the collection, freed by `elem_dec_fn` when the
    // collection / iterator handle drops via `ori_iter_drop` / `CollectSet`).
    // The burden walk classifies such a `[str]`/`str` projection as owned (its
    // declared `var_types` carries RC burden) and would emit a last-use
    // `BurdenDec` — a double-free under the standalone ledger (the element view
    // owns no allocation). `compute_borrowed_alias_vars` only source-gates on
    // borrowed PARAMS, and the `__iter_next` result is a scalar handle (not a
    // borrowed param), so the projection slips through. Consume the predicate
    // stack's `collect_iter_element_defs` SSOT (AIMS Invariant 5 — no parallel
    // iterator-element tracker) to exclude the element-view lineage.
    let iter_element_defs = crate::aims::emit_rc::collect_iter_element_defs(func, interner);
    owned_vars_needing_rc.retain(|v| !iter_element_defs.contains(v));
    let last_uses_at = group_last_uses_filtered(&ctx, &owned_vars_needing_rc);
    {
        let mut owned: Vec<u32> = owned_vars_needing_rc
            .iter()
            .map(|v| u32::try_from(v.index()).unwrap_or(u32::MAX))
            .collect();
        owned.sort_unstable();
        let dec_sites: Vec<u32> = last_uses_at
            .values()
            .flatten()
            .map(|v| u32::try_from(v.index()).unwrap_or(u32::MAX))
            .collect();
        tracing::trace!(
            target: "ori_arc::aims::realize",
            fn_name = ?func.name,
            ?owned,
            ?dec_sites,
            "burden owned-vars-needing-rc + last-use dec sites"
        );
    }
    let terminator_transfer_per_block =
        compute_terminator_transfer_per_block(func, derived_ownership);
    let terminator_inc_per_block =
        compute_terminator_inc_per_block(func, &owned_vars_needing_rc, derived_ownership);

    // Populate `moved_out_fields` per the Non-Drop partial-move two-stage rule.
    // Pass 1 collects `(project_dst → (src, field))`; Pass 2 walks instructions
    // + terminators and sets the bit when a transferred var matches a
    // project_dst. Project alone leaves the bit unset (TF-4 Borrowed);
    // `Set.value` carve-out applies via `instr_transfer_vars` (TF-15).
    populate_moved_out_fields(
        &mut ctx,
        func,
        &terminator_transfer_per_block,
        type_registry,
    );

    // Derive the full-move var set: vars whose `moved_out_fields[var]` covers
    // every top-level field index of their `Burden::owned_fields()`. BurdenDec
    // emission is suppressed for these per AIMS RL-2 (full-move == complete
    // ownership transfer at field-projection grain → BurdenDec correctly
    // suppressed). Partial-move (some-but-not-all fields covered) still emits a
    // CONSERVATIVE FULL BurdenDec (over-emit, refined by the partial-drop IR
    // variant).
    let full_move_vars = compute_full_move_vars(
        func,
        &ctx.moved_out_fields_union,
        type_registry,
        &owned_vars_needing_rc,
    );

    // Derive the partial-move var map: vars with non-empty
    // `moved_out_fields[var]` that are NOT in `full_move_vars`. Each entry's
    // `skip_fields: Vec<u32>` lists top-level field indices to skip during
    // drop-glue iteration at codegen. `BurdenDecPartial` emission gates on this
    // map per AIMS RL-2 partial-transfer semantics (the non-moved fields still
    // need their drop; skip_fields names the transferred subset). AIMS
    // Invariant 5 case (b) — extends ArcInstr enum on the SAME var dimension;
    // no parallel emission, no shadow tracker.
    let partial_move_vars = compute_partial_move_vars(
        &ctx.moved_out_fields_union,
        &full_move_vars,
        &owned_vars_needing_rc,
    );

    // RL-2 transfer-suppression symmetry: a fresh value whose paired BurdenDec
    // is transfer-suppressed at its LAST-USE must have its FRESH-site BurdenInc
    // suppressed too, else the inc is orphaned and the per-value burden ledger
    // nets +1 (VF-1 imbalance). Mirror the EXACT instruction-level
    // dec-suppression condition in emit_instr_burdens (line ~1221): dec
    // suppressed iff the var is transferred at its last-use instr OR its whole
    // owned-field set was moved (full_move_vars). Terminator-position transfers
    // are NOT included — their decs are emitted by emit_terminator_burden_decs
    // and balanced by emit_terminator_burden_incs, a separate inc/dec pair from
    // the FRESH-site inc. A value transferred at a NON-last use (aliased, still
    // live) keeps its Inc — its dec is emitted at the later non-transfer use.
    let mut inc_suppressed_vars: FxHashSet<ArcVarId> = full_move_vars.clone();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            let Some(last_used) = last_uses_at.get(&(block_idx, instr_idx)) else {
                continue;
            };
            let tv = instr_transfer_vars(instr, func);
            for &var in last_used {
                if tv.contains(&var) {
                    inc_suppressed_vars.insert(var);
                }
            }
        }
    }

    // RL-1 duplication-alias emission for Let-Var aliases: a `Let { Var(src) }`
    // alias whose SOURCE stays live after the alias is a genuine duplication —
    // a new reference to `src`'s allocation. The burden path emits the alias's
    // own paired RC: a FRESH-site `BurdenInc dst` at the alias site
    // (emit_fresh_site_burden_inc) balanced by a `BurdenDec dst` at the alias's
    // true last-use (emit_last_use_decs / emit_terminator_burden_decs). Net 0.
    // A move-alias (source used only at the alias) is NOT a dup_alias_dst — its
    // ownership forwards through the move chain (transfer_via_move_alias) and the
    // source's own FRESH-site inc covers the lineage. "Source stays live" =
    // source appears in >= 2 used-var positions (the alias use plus at least one
    // more downstream).
    let (use_counts, dup_alias_dsts) =
        compute_use_counts_and_dup_aliases(func, &mut inc_suppressed_vars);

    // RL-2 transitive-transfer suppression through Let-Var MOVE-alias chains.
    // A value whose ownership transfers out (Return value, owned call arg,
    // owned Jump arg) discharges its release at the transfer point — RL-2's
    // ownership-transferring-use exception. When a value reaches that transfer
    // point THROUGH a `Let { Var }` move-alias chain (`%d = %s` with `%s` used
    // exactly once — a move, not a duplication), the MOVE SOURCE also transfers
    // out: its only use forwards ownership to `%d`, which forwards to the
    // transfer point. Emitting a last-use BurdenDec on the move source then
    // releases a reference already handed off downstream — a VF-1 net=-1 orphan
    // dec (`id<T>(x) = x` over an owned param is the minimal witness:
    // `%1 = %0; Return %1` would dec `%0` though its ownership returns to the
    // caller). Backward-propagate transfer-suppression through every move-alias
    // hop to a fixpoint; the source set suppresses the source's last-use dec.
    // Invoke / Apply transfer-through-return result→arg move-edges: when a callee
    // transfers an owned param THROUGH its return, the call result IS the
    // forwarded arg (a move across the call), so the move-chain must span it.
    let invoke_ttr_edges: Vec<(ArcVarId, ArcVarId)> = {
        let mut edges = Vec::new();
        let mut collect = |dst: ArcVarId, callee: &Name, args: &[ArcVarId]| {
            if let Some(contract) = contracts.get(callee) {
                for (i, param) in contract.params.iter().enumerate() {
                    if param.transfers_through_return {
                        if let Some(&arg) = args.get(i) {
                            edges.push((dst, arg));
                        }
                    }
                }
            }
        };
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Apply {
                    dst,
                    func: callee,
                    args,
                    ..
                } = instr
                {
                    collect(*dst, callee, args);
                }
            }
            if let crate::ir::ArcTerminator::Invoke {
                dst,
                func: callee,
                args,
                ..
            } = &block.terminator
            {
                collect(*dst, callee, args);
            }
        }
        edges
    };
    let transfer_via_move_alias = compute_transfer_via_move_alias(
        func,
        &terminator_transfer_per_block,
        &use_counts,
        ctx.last_use_points(),
        &owned_vars_needing_rc,
        &invoke_ttr_edges,
    );

    // Symmetry: a FRESH move source whose last-use dec is suppressed (it
    // transfers out via the alias chain) must also have its FRESH-site inc
    // suppressed, else the inc orphans (VF-1 net=+1). A param move source has no
    // FRESH inc, so the union is a no-op for it; only FRESH sources gain the
    // inc-suppression. Keeps the per-value ledger at net 0 for both shapes.
    // Both halves are suppressed on BOTH the default and probe paths: the value
    // genuinely moves through the alias chain to a real transfer point where its
    // single release is discharged (suppressing here prevents a double dec on the
    // shared allocation under the probe — the move-alias dec stays suppressed in
    // emit_last_use_decs on both paths).
    for &var in &transfer_via_move_alias {
        // Pure move source (used <= once): the value flows straight through to
        // the consumer, so BOTH halves of its FRESH pair are suppressed. A DUP'd
        // move source (used >= 2) forwards only its ORIGINAL allocation reference
        // at its terminal move; its FRESH inc supplies the DUPLICATE references
        // the non-terminal uses consume and MUST be kept — only the terminal
        // last-use dec is suppressed (in `transfer_via_move_alias`). Suppressing
        // the inc here would under-count and collapse a COW receiver's RC below
        // the live alias count. Per AIMS RL-1/RL-2.
        if use_counts.get(&var).copied().unwrap_or(0) <= 1 {
            inc_suppressed_vars.insert(var);
        }
    }

    // Symmetry for borrowed terminator-Invoke args: `invoke_terminator_borrowed_args`
    // (emit.rs) suppresses the terminator-last-use `BurdenDec` for a BORROWED
    // `Invoke`/`InvokeIndirect` arg — the value survives the borrowed call and
    // the predicate-stack edge cleanup discharges its release. A FRESH value
    // created in the terminator block solely to pass borrowed (e.g.
    // `%0 = "hello"; Invoke @f(%0 [borrow])` where the callee stores `%0` into
    // its result) would otherwise keep its FRESH-site `BurdenInc` with the dec
    // suppressed → net=+1 on the path where the value lives on into the result.
    // Suppress the FRESH inc symmetrically so the var carries NO burden ops
    // (`burden_emitted` stays false → edge cleanup emits no paired `BurdenDec`)
    // and is fully predicate-stack-managed (`Spec: Annex E §AIMS RL-2`). A
    // genuine borrow (param / alias, no FRESH inc) gains nothing — no-op.
    //
    // Probe gate: under `predicate_stack_rc_disabled` the terminator-last-use
    // BurdenDec is un-suppressed (emit_burden_ops_for_blocks passes an empty
    // borrowed-arg set), so the FRESH inc must survive symmetrically — the
    // burden path carries the full paired inc+dec for the fresh-owned arg the
    // callee stores. Keep the inc suppressed only on the default path.
    let borrowed_terminator_args = compute_borrowed_terminator_invoke_args(func);
    if !predicate_stack_rc_disabled {
        for &var in &borrowed_terminator_args {
            inc_suppressed_vars.insert(var);
        }
    }

    // RL-4 live-out suppression set (`Spec: Annex E §AIMS RL-4`): a per-block
    // last-use `BurdenDec` is a genuine release only when the var is dead at
    // block exit; live-out vars are released on dying CFG edges. See
    // `compute_live_out_owned`.
    let live_out_per_block = compute_live_out_owned(func, &owned_vars_needing_rc);

    // Step-1 COW-inc set + step-2 COW-mutator-release-gate names. Probe-only —
    // empty on the default path (the predicate stack emits the equivalent RcInc,
    // so default AOT codegen is byte-identical). Detail: `compute_cow_inc_and_mutators`.
    let (cow_inc_borrowed_aliases, cow_mutator_names) = compute_cow_inc_and_mutators(
        func,
        &borrowed_aliases,
        interner,
        predicate_stack_rc_disabled,
    );

    // Function-wide list-concat consume set (precomputed: the `&mut` emit walk
    // cannot re-borrow `func` for `var_repr`). SSOT: `list_concat_consumed_operands`.
    let mut list_concat_transfer_vars: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            list_concat_transfer_vars.extend(list_concat_consumed_operands(instr, func));
        }
    }

    // RL-1 result-inc elision for transfer-through-return forwarders: a callee
    // returning its owned param unchanged hands the SAME allocation back, so the
    // result is not a fresh value and its result-inc is elidable. SSOT:
    // `compute_transfer_through_return_results`.
    let transfer_through_return_results = compute_transfer_through_return_results(func, contracts);

    // RL-2 callee transfer-source-dec strip: a param of THIS function that flows
    // to a `Return` terminator (per its own `MemoryContract.transfers_through_return`)
    // transfers ownership back to the caller — its scope-exit `BurdenDec` is
    // suppressed (the caller decs the bound result). The interprocedural contract
    // is the SSOT for the proven Return-flow fact; the structural move-alias scan
    // conservatively keeps the dec when the param is multi-block-used, so consult
    // the contract directly for the param case. SSOT:
    // `compute_transfer_through_return_param_vars`.
    let transfer_through_return_param_vars =
        compute_transfer_through_return_param_vars(func, contracts);

    // RL-1 inc-elision callee identity: `__index` codegen self-increments its
    // extracted non-scalar result, so the burden path elides the result-inc
    // (AIMS emits only the balancing dec). Interned once; idempotent.
    let index_builtin_name =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Index.name());

    let analysis = BurdenAnalysisCtx {
        owned_vars_needing_rc: &owned_vars_needing_rc,
        last_uses_at: &last_uses_at,
        full_move_vars: &full_move_vars,
        partial_move_vars: &partial_move_vars,
        inc_suppressed_vars: &inc_suppressed_vars,
        dup_alias_dsts: &dup_alias_dsts,
        transfer_via_move_alias: &transfer_via_move_alias,
        live_out_per_block: &live_out_per_block,
        contracts,
        predicate_stack_rc_disabled,
        list_concat_transfer_vars: &list_concat_transfer_vars,
        cow_inc_borrowed_aliases: &cow_inc_borrowed_aliases,
        cow_mutator_names: &cow_mutator_names,
        transfer_through_return_results: &transfer_through_return_results,
        transfer_through_return_param_vars: &transfer_through_return_param_vars,
        index_builtin_name,
    };
    emit_burden_ops_for_blocks(
        func,
        &analysis,
        &terminator_transfer_per_block,
        &terminator_inc_per_block,
    );
    populate_burden_emitted(func);
    ctx
}

/// Populate `func.burden_emitted` from the just-emitted burden ops. Walks
/// every block's body once after `emit_burden_ops_for_blocks` completes and
/// sets `burden_emitted[var.index()] = true` for every var targeted by
/// `BurdenInc` / `BurdenDec` / `BurdenDecPartial` / `BurdenDecField` /
/// `BurdenDecVariant`. One linear pass per function, no per-var hash-map churn.
///
/// Coexistence-handshake input consumed downstream by the AIMS
/// post-convergence `class_covered` computation, which gates predicate-stack
/// realization deferral.
fn populate_burden_emitted(func: &mut ArcFunction) {
    if func.burden_emitted.len() != func.var_types.len() {
        func.burden_emitted = vec![false; func.var_types.len()];
    }
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::BurdenInc { var }
                | ArcInstr::BurdenDec { var }
                | ArcInstr::BurdenDecPartial { var, .. }
                | ArcInstr::BurdenDecVariant { var } => {
                    mark_emitted(&mut func.burden_emitted, var.index());
                }
                ArcInstr::BurdenDecField { base, .. } => {
                    mark_emitted(&mut func.burden_emitted, base.index());
                }
                _ => {}
            }
        }
    }
}

/// Set `emitted[idx] = true` when `idx` is in bounds; out-of-bounds is a no-op.
fn mark_emitted(emitted: &mut [bool], idx: usize) {
    if let Some(slot) = emitted.get_mut(idx) {
        *slot = true;
    }
}

#[cfg(test)]
mod tests;
